use anyhow::{anyhow, bail, Result};
use flate2::read::GzDecoder;
use log::info;
use sevenz_rust::{Password, SevenZReader};
use std::ffi::OsStr;
use std::fs::File;
use std::io::{BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use xz2::read::XzDecoder;
use zip::ZipArchive;

/// Buffer size for I/O operations (1MB)
const BUFFER_SIZE: usize = 1024 * 1024;

/// Progress update threshold (4MB) to throttle progress callbacks
const PROGRESS_THRESHOLD: u64 = 4 * 1024 * 1024;

/// Zip progress update batch size (update every N entries)
const ZIP_PROGRESS_BATCH_SIZE: usize = 100;
/// Zip extraction progress threshold (512KB) for more responsive UI.
const ZIP_PROGRESS_THRESHOLD: u64 = 512 * 1024;

use crate::setter;
use crate::utils::{ProgressHandler, ProgressKind};

use super::file_system::{ensure_dir, ensure_parent_dir, walk_dir};
use super::progress_bar::CliProgress;

struct CountingReader<R> {
    inner: R,
    consumed: Arc<AtomicU64>,
}

impl<R> CountingReader<R> {
    fn new(inner: R, consumed: Arc<AtomicU64>) -> Self {
        Self { inner, consumed }
    }
}

impl<R: Read> Read for CountingReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let n = self.inner.read(buf)?;
        if n > 0 {
            self.consumed.fetch_add(n as u64, Ordering::Relaxed);
        }
        Ok(n)
    }
}

enum ExtractableKind {
    /// `7-zip` compressed files, ended with `.7z`
    SevenZ(SevenZReader<File>),
    Gz {
        archive: tar::Archive<GzDecoder<CountingReader<File>>>,
        compressed_total: u64,
        consumed: Arc<AtomicU64>,
    },
    Xz {
        archive: tar::Archive<XzDecoder<CountingReader<File>>>,
        compressed_total: u64,
        consumed: Arc<AtomicU64>,
    },
    Zip(ZipArchive<File>),
}

pub struct Extractable<'a> {
    path: &'a Path,
    kind: ExtractableKind,
    quiet: bool,
    progress_handler: Option<Box<dyn ProgressHandler>>,
}

impl<'a> Extractable<'a> {
    pub fn is_supported(path: &'a Path) -> bool {
        // First try extension-based detection
        if let Ok(ext) = file_extension(path) {
            if matches!(ext, "7z" | "zip" | "gz" | "xz" | "crate") {
                return true;
            }
        }
        // Fallback to content-based detection
        detect_format_from_content(path).is_some()
    }

    /// Detect file format from content (magic bytes) instead of extension
    pub fn detect_format_from_content(path: &Path) -> Option<&'static str> {
        detect_format_from_content(path)
    }

    pub fn load(path: &'a Path, custom_kind: Option<&str>) -> Result<Self> {
        let ext = if let Some(custom) = custom_kind {
            custom
        } else {
            // Try extension first
            match file_extension(path) {
                Ok(ext) if matches!(ext, "7z" | "zip" | "gz" | "xz" | "crate") => ext,
                _ => {
                    // Fallback to content-based detection
                    detect_format_from_content(path)
                        .ok_or_else(|| {
                            anyhow!(
                                "unable to determine file format for '{}': neither extension nor content match supported formats",
                                path.display()
                            )
                        })?
                }
            }
        };

        let kind = match ext {
            "7z" => {
                info!("extracting {} archive '{}'", ext, path.display());
                ExtractableKind::SevenZ(SevenZReader::open(path, Password::empty())?)
            }
            "zip" => {
                info!("extracting {} archive '{}'", ext, path.display());
                ExtractableKind::Zip(ZipArchive::new(File::open(path)?)?)
            }
            "gz" | "crate" => {
                info!("extracting {} archive '{}'", ext, path.display());
                let file = File::open(path)?;
                let compressed_total = file.metadata()?.len();
                let consumed = Arc::new(AtomicU64::new(0));
                let tar_gz = GzDecoder::new(CountingReader::new(file, consumed.clone()));
                ExtractableKind::Gz {
                    archive: tar::Archive::new(tar_gz),
                    compressed_total,
                    consumed,
                }
            }
            "xz" => {
                info!("extracting {} archive '{}'", ext, path.display());
                let file = File::open(path)?;
                let compressed_total = file.metadata()?.len();
                let consumed = Arc::new(AtomicU64::new(0));
                let tar_xz = XzDecoder::new(CountingReader::new(file, consumed.clone()));
                ExtractableKind::Xz {
                    archive: tar::Archive::new(tar_xz),
                    compressed_total,
                    consumed,
                }
            }
            _ => bail!("'{ext}' is not a supported extractable file format"),
        };

        Ok(Self {
            path,
            kind,
            quiet: false,
            progress_handler: None,
        })
    }

    setter!(quiet(self.quiet, bool));
    
    /// Set a custom progress handler for extraction progress.
    pub fn with_progress_handler(mut self, handler: Box<dyn ProgressHandler>) -> Self {
        self.progress_handler = Some(handler);
        self
    }

    /// Extract current file into a specific directory.
    ///
    /// This will extract file under the `root`, make sure it's an empty folder before using this function.
    pub fn extract_to(&mut self, root: &Path) -> Result<()> {
        // Ensure root is a directory, not a file
        if root.is_file() {
            bail!(
                "extraction target '{}' is a file, not a directory",
                root.display()
            );
        }
        // Ensure the directory exists
        ensure_dir(root)?;
        
        let handler: Box<dyn ProgressHandler> = if let Some(ref mut ph) = self.progress_handler {
            // Take the handler if available
            std::mem::replace(ph, Box::new(CliProgress::default()))
        } else {
            Box::new(CliProgress::default())
        };
        
        let mut helper = ExtractHelperBoxed {
            file_path: self.path,
            output_dir: root,
            handler,
        };

        match &mut self.kind {
            ExtractableKind::Zip(archive) => helper.extract_zip(archive),
            ExtractableKind::SevenZ(archive) => helper.extract_7z(archive),
            ExtractableKind::Gz {
                archive,
                compressed_total,
                consumed,
            } => helper.extract_tar(archive, *compressed_total, consumed),
            ExtractableKind::Xz {
                archive,
                compressed_total,
                consumed,
            } => helper.extract_tar(archive, *compressed_total, consumed),
        }
    }

    /// Extract file into a specific root like [`extract_to`](Extractable::extract_to),
    /// then look for **solo** nested directory and return the last one.
    ///
    /// This works similar to skipping common prefixes, except this does not
    /// handle common prefixes when extracting. ~~(because I don't know how)~~
    ///
    /// If `stop` contains a folder name, this function will stop when encountered that folder and
    /// return the full extracted path of **its parent** instead.
    ///
    /// # Example:
    /// Suppose we have an archive with entries like this:
    /// ```text
    /// Foo
    ///  |- a
    ///     |- b
    ///        |- c
    ///           |- d1
    ///           |- d2
    /// ```
    /// Then after calling this function, the path to `c` will be returned,
    /// because it's the last solo directory in the archive
    /// ```ignore
    /// let dir = Extractable::load("/path/to/foo.tar.gz")?
    ///     .extract_then_skip_solo_dir("/path/to/foo", None)?;
    /// assert_eq!(dir, PathBuf::from("/path/to/foo/a/b/c"));
    /// ```
    pub fn extract_then_skip_solo_dir<S: AsRef<OsStr>>(
        &mut self,
        root: &Path,
        stop: Option<S>,
    ) -> Result<PathBuf> {
        fn inner_<S: AsRef<OsStr>>(root: &Path, stop: Option<S>) -> Result<PathBuf> {
            // If root is not a directory, this is an error - extraction should have created a directory
            if !root.is_dir() {
                bail!(
                    "extraction target '{}' is not a directory after extraction (exists: {}, is_file: {})",
                    root.display(),
                    root.exists(),
                    root.is_file()
                );
            }
            
            let sub_entries = walk_dir(root, false)?;
            // Filter out files, only keep directories
            let sub_dirs: Vec<_> = sub_entries.iter()
                .filter(|p| p.is_dir())
                .collect();

            if let [sub_dir] = sub_dirs.as_slice() {
                if matches!(stop, Some(ref keyword) if filename_matches_keyword(sub_dir, keyword)) {
                    Ok(root.to_path_buf())
                } else {
                    inner_(sub_dir, stop)
                }
            } else {
                Ok(root.to_path_buf())
            }
        }

        // first we need to extract the tarball
        // extract_to will ensure root is a directory
        self.extract_to(root)?;
        // then find the last solo dir recursively
        inner_(root, stop)
    }
}

fn file_extension(path: &Path) -> Result<&str> {
    path.extension()
        .ok_or_else(|| {
            anyhow!(
                "'{}' is not extractable because it appears to have no file extension",
                path.display()
            )
        })?
        .to_str()
        .ok_or_else(|| {
            anyhow!(
                "'{}' is not extractable because its extension contains \
                invalid unicode characters",
                path.display()
            )
        })
}

fn filename_matches_keyword<S: AsRef<OsStr>>(path: &Path, keyword: S) -> bool {
    if let Some(name) = path.file_name() {
        name == keyword.as_ref()
    } else {
        false
    }
}

/// Detect file format from content (magic bytes)
/// Returns the format extension if detected, None otherwise
fn detect_format_from_content(path: &Path) -> Option<&'static str> {
    let mut file = match File::open(path) {
        Ok(f) => f,
        Err(_) => return None,
    };

    let mut header = [0u8; 16];
    match file.read_exact(&mut header) {
        Ok(_) => {}
        Err(_) => return None,
    }

    // Check magic bytes for different formats
    // ZIP: starts with PK (0x50 0x4B) - either "PK\x03\x04" (local file header) or "PK\x05\x06" (empty archive) or "PK\x07\x08" (spanned archive)
    if header.starts_with(b"PK\x03\x04") || header.starts_with(b"PK\x05\x06") || header.starts_with(b"PK\x07\x08") {
        return Some("zip");
    }

    // 7z: starts with "7z\xBC\xAF\x27\x1C"
    if header.starts_with(b"7z\xBC\xAF\x27\x1C") {
        return Some("7z");
    }

    // GZIP: starts with 0x1F 0x8B
    if header.starts_with(&[0x1F, 0x8B]) {
        return Some("gz");
    }

    // XZ: starts with 0xFD 0x37 0x7A 0x58 0x5A 0x00
    if header.starts_with(&[0xFD, 0x37, 0x7A, 0x58, 0x5A, 0x00]) {
        return Some("xz");
    }

    None
}

struct ExtractHelperBoxed<'a> {
    file_path: &'a Path,
    output_dir: &'a Path,
    handler: Box<dyn ProgressHandler>,
}

impl ExtractHelperBoxed<'_> {
    fn start_progress_bar(&mut self, style: ProgressKind) {
        let file_name = self.file_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_else(|| self.file_path.to_str().unwrap_or("unknown"));
        if let Err(e) = self.handler.as_mut().start(
            format!("extracting file '{}'", file_name),
            style,
        ) {
            log::debug!("progress start failed: {e}");
        }
    }

    fn update_progress_bar(&self, prog: Option<u64>) {
        if let Err(e) = self.handler.update(prog) {
            log::trace!("progress update failed: {e}");
        }
    }

    fn end_progress_bar(&self) {
        if let Err(e) = self.handler.finish("extraction complete".to_string()) {
            log::debug!("progress finish failed: {e}");
        }
    }

    /// Create a buffered writer with standard buffer size
    fn create_buf_writer(&self, file: File) -> BufWriter<File> {
        BufWriter::with_capacity(BUFFER_SIZE, file)
    }

    /// Throttled progress update - only updates when threshold is reached
    fn throttled_progress_update(
        &self,
        current: u64,
        last_reported: &mut u64,
        threshold: u64,
    ) {
        if current - *last_reported >= threshold {
            self.update_progress_bar(Some(current));
            *last_reported = current;
        }
    }

    fn extract_zip(&mut self, archive: &mut ZipArchive<File>) -> Result<()> {
        let zip_len = archive.len();
        let mut total_uncompressed: u64 = 0;

        // Pre-calculate total uncompressed size from zip metadata so progress can move
        // during large single-file extraction (instead of waiting for per-entry completion).
        for i in 0..zip_len {
            let zip_file = archive.by_index(i)?;
            total_uncompressed = total_uncompressed.saturating_add(zip_file.size());
        }

        let total = total_uncompressed.max(1);
        self.start_progress_bar(ProgressKind::Bytes(total));

        let mut extracted_len: u64 = 0;
        let mut last_reported_progress: u64 = 0;
        let mut buf = vec![0_u8; BUFFER_SIZE];

        for i in 0..zip_len {
            let mut zip_file = archive.by_index(i)?;
            let Some(out_path) = zip_file
                .enclosed_name()
                .map(|path| self.output_dir.join(path))
            else {
                continue;
            };

            if zip_file.is_dir() {
                ensure_dir(&out_path)?;
            } else {
                ensure_parent_dir(&out_path)?;
                let out_file = std::fs::File::create(&out_path)?;
                let mut writer = self.create_buf_writer(out_file);

                loop {
                    let read_size = zip_file.read(&mut buf)?;
                    if read_size == 0 {
                        writer.flush()?;
                        break;
                    }
                    writer.write_all(&buf[..read_size])?;
                    extracted_len += read_size as u64;

                    self.throttled_progress_update(
                        extracted_len,
                        &mut last_reported_progress,
                        ZIP_PROGRESS_THRESHOLD,
                    );
                }
            }

            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                if let Some(mode) = zip_file.unix_mode() {
                    std::fs::set_permissions(&out_path, std::fs::Permissions::from_mode(mode))?;
                }
            }

            // Ensure periodic updates even for metadata-only entries.
            if i % ZIP_PROGRESS_BATCH_SIZE == 0 {
                self.update_progress_bar(Some(extracted_len.min(total)));
                last_reported_progress = extracted_len;
            }
        }

        self.update_progress_bar(Some(total));
        self.end_progress_bar();

        Ok(())
    }

    fn extract_7z(&mut self, archive: &mut SevenZReader<File>) -> Result<()> {
        let entries = &archive.archive().files;
        let sz_len: u64 = entries
            .iter()
            .filter_map(|e| e.has_stream().then_some(e.size()))
            .sum();
        let mut extracted_len: u64 = 0;
        let mut last_reported_progress: u64 = 0;
        let mut buf = vec![0_u8; BUFFER_SIZE];

        self.start_progress_bar(ProgressKind::Bytes(sz_len));

        archive.for_each_entries(|entry, reader| {
            let entry_path = PathBuf::from(entry.name());
            let out_path = self.output_dir.join(&entry_path);

            if entry.is_directory() {
                ensure_dir(&out_path).map_err(|_| {
                    sevenz_rust::Error::other(format!(
                        "unable to create entry directory '{}'",
                        out_path.display()
                    ))
                })?;
                Ok(true)
            } else {
                ensure_parent_dir(&out_path).map_err(|_| {
                    sevenz_rust::Error::other(format!(
                        "unable to create parent directory for '{}'",
                        out_path.display()
                    ))
                })?;

                let out_file = std::fs::File::create(&out_path)?;
                let mut writer = self.create_buf_writer(out_file);

                loop {
                    let read_size = reader.read(&mut buf)?;
                    if read_size == 0 {
                        writer.flush()?;
                        break Ok(true);
                    }
                    writer.write_all(&buf[..read_size])?;
                    extracted_len += read_size as u64;

                    self.throttled_progress_update(
                        extracted_len,
                        &mut last_reported_progress,
                        PROGRESS_THRESHOLD,
                    );
                }
            }
        })?;

        self.end_progress_bar();
        Ok(())
    }

    fn extract_tar<R: Read>(
        &mut self,
        archive: &mut tar::Archive<R>,
        compressed_total: u64,
        consumed: &Arc<AtomicU64>,
    ) -> Result<()> {
        #[cfg(unix)]
        archive.set_preserve_permissions(true);

        let total = compressed_total.max(1);
        self.start_progress_bar(ProgressKind::Bytes(total));

        let mut entries = archive.entries()?;
        let mut last_reported_progress: u64 = 0;

        while let Some(mut entry) = entries.next().transpose()? {
            entry.unpack_in(self.output_dir)?;

            let current = consumed.load(Ordering::Relaxed).min(total);
            if current.saturating_sub(last_reported_progress) >= PROGRESS_THRESHOLD {
                self.update_progress_bar(Some(current));
                last_reported_progress = current;
            }
        }

        self.update_progress_bar(Some(total));
        self.end_progress_bar();
        Ok(())
    }
}
