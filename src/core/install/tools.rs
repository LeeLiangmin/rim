use super::errors::InstallationErrors;
use super::InstallConfiguration;
use crate::core::dependency_handler::DependencyHandler;
use crate::core::parser::fingerprint::ToolRecord;
use crate::core::tools::Tool;
use crate::core::GlobalOpts;
use anyhow::{anyhow, bail, Context, Result};
use log::{info, warn};
use rim_common::types::{ToolInfo, ToolKind, ToolMap, ToolSource};
use rim_common::utils;
use rim_common::utils::ProgressHandler;
use std::path::{Path, PathBuf};
use tempfile::TempDir;
use url::Url;

impl<'a, T: ProgressHandler + Clone + 'static> InstallConfiguration<'a, T> {
    /// Check if a file is a nested archive that needs extraction.
    /// Returns Some(archive_path) if the file is an archive, None otherwise.
    ///
    /// This function uses multiple detection methods for resilience:
    /// 1. Extension-based detection (fastest)
    /// 2. Content-based detection (magic bytes)
    /// 3. Load test (most reliable)
    /// 4. Path comparison with original file
    fn detect_nested_archive(&self, file_path: &Path, original_file: &Path) -> Option<PathBuf> {
        if !file_path.exists() || !file_path.is_file() {
            return None;
        }

        let is_archive_by_ext = utils::Extractable::is_supported(file_path);
        let is_archive_by_content =
            utils::Extractable::detect_format_from_content(file_path).is_some();

        let is_archive_by_load = if !is_archive_by_ext && !is_archive_by_content {
            utils::Extractable::load(file_path, None).is_ok()
        } else {
            false
        };

        if is_archive_by_ext || is_archive_by_content || is_archive_by_load {
            let detection_method = if is_archive_by_ext {
                "extension"
            } else if is_archive_by_content {
                "content (magic bytes)"
            } else {
                "load test"
            };
            info!(
                "Found nested archive file '{}' (detected by {}), attempting extraction",
                file_path.display(),
                detection_method
            );
            return Some(file_path.to_path_buf());
        }

        if let (Ok(file_canon), Ok(orig_canon)) =
            (file_path.canonicalize(), original_file.canonicalize())
        {
            if file_canon == orig_canon {
                info!("Extracted path is the original archive file (canonicalized match), attempting extraction");
                return Some(file_path.to_path_buf());
            }
        }

        if let Some(file_name) = file_path.file_name().and_then(|n| n.to_str()) {
            let archive_extensions = [".zip", ".tar.gz", ".tar.xz", ".tgz", ".7z", ".gz", ".xz"];
            if archive_extensions
                .iter()
                .any(|ext| file_name.ends_with(ext))
            {
                if let Some(ext) = archive_extensions
                    .iter()
                    .find(|ext| file_name.ends_with(*ext))
                {
                    let format = ext.strip_prefix('.').unwrap_or(ext);
                    if utils::Extractable::load(file_path, Some(format)).is_ok() {
                        info!(
                            "Found nested archive '{}' by filename pattern ({}), attempting extraction",
                            file_path.display(),
                            format
                        );
                        return Some(file_path.to_path_buf());
                    }
                }
            }
        }

        None
    }

    /// Find the tool directory in a parent directory.
    /// Looks for directories containing "bin" subdirectory or Cargo.toml (for crates).
    fn find_tool_directory_in_parent(&self, parent: &Path) -> Result<Option<PathBuf>> {
        if !parent.is_dir() {
            return Ok(None);
        }

        let entries: Vec<_> = std::fs::read_dir(parent)?
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .collect();

        for entry in &entries {
            if entry.is_dir() {
                let bin_dir = entry.join("bin");
                let cargo_toml = entry.join("Cargo.toml");
                if (bin_dir.exists() && bin_dir.is_dir()) || cargo_toml.exists() {
                    info!("Found tool directory in parent: {}", entry.display());
                    return Ok(Some(entry.to_path_buf()));
                }
            }
        }

        let dirs: Vec<_> = entries.iter().filter(|p| p.is_dir()).collect();
        if dirs.len() == 1 {
            info!(
                "Using single directory found in parent: {}",
                dirs[0].display()
            );
            return Ok(Some(dirs[0].to_path_buf()));
        }

        Ok(None)
    }

    /// Perform extraction or copy action base on the given path, with progress reporting.
    ///
    /// If `maybe_file` is a path to compressed file, this will try to extract it to `dest`;
    /// otherwise this will copy that file into dest.
    ///
    /// `stop_keyword` is used to determine when to stop skipping solo directories during extraction.
    /// For tools with bin/ directory, use "bin". For crate tools, use None to skip all solo dirs.
    fn extract_or_copy_to_with_progress(
        &self,
        maybe_file: &Path,
        dest: &Path,
        stop_keyword: Option<&str>,
    ) -> Result<PathBuf> {
        fn extract_nested_archive(
            archive_path: &Path,
            dest: &Path,
            stop_keyword: Option<&str>,
            quiet: bool,
            progress_handler: Box<dyn rim_common::utils::ProgressHandler>,
        ) -> Result<PathBuf> {
            let mut extractable = utils::Extractable::load(archive_path, None)?;
            extractable = extractable
                .quiet(quiet)
                .with_progress_handler(progress_handler);
            extractable.extract_to(dest)?;

            let entries: Vec<_> = std::fs::read_dir(dest)?
                .filter_map(|e| e.ok())
                .map(|e| e.path())
                .filter(|p| p != archive_path)
                .collect();

            for entry in &entries {
                if entry.is_file() && utils::Extractable::is_supported(entry) {
                    let nested_handler = Box::new(rim_common::utils::HiddenProgress);
                    return extract_nested_archive(entry, dest, stop_keyword, quiet, nested_handler);
                }
            }

            for entry in &entries {
                if entry.is_dir() {
                    let bin_dir = entry.join("bin");
                    let cargo_toml = entry.join("Cargo.toml");
                    if (bin_dir.exists() && bin_dir.is_dir()) || cargo_toml.exists() {
                        return Ok(entry.to_path_buf());
                    }
                }
            }

            let dirs: Vec<_> = entries.iter().filter(|p| p.is_dir()).collect();
            if dirs.len() == 1 {
                return Ok(dirs[0].to_path_buf());
            }

            Ok(dest.to_path_buf())
        }

        if let Ok(mut extractable) = utils::Extractable::load(maybe_file, None) {
            extractable = extractable
                .quiet(GlobalOpts::get().quiet)
                .with_progress_handler(Box::new(self.progress_handler.clone()));
            let extracted_path =
                extractable.extract_then_skip_solo_dir(dest, stop_keyword)?;

            if extracted_path.is_file() {
                warn!(
                    "extract_then_skip_solo_dir returned a file instead of directory: {}",
                    extracted_path.display()
                );

                if let Some(archive_path) =
                    self.detect_nested_archive(&extracted_path, maybe_file)
                {
                    info!("Detected nested archive in extracted_path, attempting recursive extraction");
                    let handler = Box::new(self.progress_handler.clone());
                    match extract_nested_archive(
                        &archive_path,
                        dest,
                        stop_keyword,
                        GlobalOpts::get().quiet,
                        handler,
                    ) {
                        Ok(result) => return Ok(result),
                        Err(e) => {
                            warn!(
                                "Failed to extract nested archive '{}': {}",
                                archive_path.display(),
                                e
                            );
                        }
                    }
                }

                if dest.is_dir() {
                    if let Ok(entries) = std::fs::read_dir(dest) {
                        for entry in entries.filter_map(|e| e.ok()) {
                            let entry_path = entry.path();
                            if entry_path == extracted_path || entry_path == maybe_file {
                                continue;
                            }

                            if entry_path.is_file() {
                                if let Some(archive_path) =
                                    self.detect_nested_archive(&entry_path, maybe_file)
                                {
                                    warn!(
                                        "Found undetected nested archive '{}' in dest directory, attempting extraction",
                                        entry_path.display()
                                    );
                                    let handler = Box::new(self.progress_handler.clone());
                                    match extract_nested_archive(
                                        &archive_path,
                                        dest,
                                        stop_keyword,
                                        GlobalOpts::get().quiet,
                                        handler,
                                    ) {
                                        Ok(result) => return Ok(result),
                                        Err(e) => {
                                            warn!(
                                                "Failed to extract nested archive '{}': {}",
                                                archive_path.display(),
                                                e
                                            );
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                if utils::is_executable(&extracted_path) {
                    return Ok(extracted_path);
                }

                if extracted_path != maybe_file
                    && !utils::Extractable::is_supported(&extracted_path)
                {
                    return Ok(extracted_path);
                }

                if let Some(parent) = extracted_path.parent() {
                    let bin_in_parent = parent.join("bin");
                    if bin_in_parent.exists() && bin_in_parent.is_dir() {
                        info!(
                            "Found bin directory in parent, using parent directory: {}",
                            parent.display()
                        );
                        return Ok(parent.to_path_buf());
                    }
                    if let Ok(Some(tool_dir)) = self.find_tool_directory_in_parent(parent) {
                        return Ok(tool_dir);
                    }
                }

                if dest.is_dir() {
                    if let Ok(Some(tool_dir)) = self.find_tool_directory_in_parent(dest) {
                        return Ok(tool_dir);
                    }
                }

                anyhow::bail!(
                    "Extracted path is not a directory: {} (exists: {}, is_file: {}). \
                    Tried 6 layers of detection and recovery (nested archive detection, dest directory scan, \
                    executable check, parent directory search, dest directory search), but all failed. \
                    This may indicate a corrupted or unexpected archive structure. \
                    Please check the archive manually or report this issue.",
                    extracted_path.display(),
                    extracted_path.exists(),
                    extracted_path.is_file()
                );
            }

            if !extracted_path.is_dir() {
                if let Some(parent) = extracted_path.parent() {
                    let bin_in_parent = parent.join("bin");
                    if bin_in_parent.exists() && bin_in_parent.is_dir() {
                        info!(
                            "Found bin directory in parent, using parent directory: {}",
                            parent.display()
                        );
                        return Ok(parent.to_path_buf());
                    }
                    if let Ok(Some(tool_dir)) = self.find_tool_directory_in_parent(parent) {
                        return Ok(tool_dir);
                    }
                }

                if dest.is_dir() {
                    if let Ok(Some(tool_dir)) = self.find_tool_directory_in_parent(dest) {
                        return Ok(tool_dir);
                    }
                }

                if extracted_path.is_file() {
                    if let Some(archive_path) =
                        self.detect_nested_archive(&extracted_path, maybe_file)
                    {
                        warn!(
                            "Late detection: Found nested archive '{}', attempting extraction",
                            archive_path.display()
                        );
                        let handler = Box::new(self.progress_handler.clone());
                        if let Ok(result) = extract_nested_archive(
                            &archive_path,
                            dest,
                            stop_keyword,
                            GlobalOpts::get().quiet,
                            handler,
                        ) {
                            return Ok(result);
                        }
                    }
                }

                anyhow::bail!(
                    "Extracted path is not a directory: {} (exists: {}, is_file: {}). \
                    Tried multiple recovery methods but failed.",
                    extracted_path.display(),
                    extracted_path.exists(),
                    extracted_path.is_file()
                );
            }

            Ok(extracted_path)
        } else {
            // File is not a supported archive format
            if maybe_file.is_file() {
                if let Some(detected_format) =
                    utils::Extractable::detect_format_from_content(maybe_file)
                {
                    if let Ok(mut extractable) =
                        utils::Extractable::load(maybe_file, Some(detected_format))
                    {
                        extractable = extractable
                            .quiet(GlobalOpts::get().quiet)
                            .with_progress_handler(Box::new(self.progress_handler.clone()));
                        let extracted_path =
                            extractable.extract_then_skip_solo_dir(dest, stop_keyword)?;

                        if extracted_path.is_file() {
                            if utils::is_executable(&extracted_path) {
                                return Ok(extracted_path);
                            }
                            if let Some(parent) = extracted_path.parent() {
                                if let Ok(Some(tool_dir)) =
                                    self.find_tool_directory_in_parent(parent)
                                {
                                    return Ok(tool_dir);
                                }
                            }
                        }

                        if extracted_path.is_dir() {
                            return Ok(extracted_path);
                        }
                    }
                }

                if utils::is_executable(maybe_file) || maybe_file.extension().is_none() {
                    let dest_file = dest.join(
                        maybe_file
                            .file_name()
                            .unwrap_or_else(|| std::ffi::OsStr::new("executable")),
                    );
                    std::fs::copy(maybe_file, &dest_file)?;
                    #[cfg(unix)]
                    {
                        use std::os::unix::fs::PermissionsExt;
                        let mut perms = std::fs::metadata(&dest_file)?.permissions();
                        perms.set_mode(0o755);
                        std::fs::set_permissions(&dest_file, perms)?;
                    }
                    return Ok(dest_file);
                }
            }

            utils::copy_into(maybe_file, dest)
        }
    }

    /// Infer the filename from URL path segments.
    /// Returns the last non-empty path segment, or an error if none found.
    fn infer_filename_from_url(&self, url: &Url) -> Result<String> {
        url.path_segments()
            .ok_or_else(|| anyhow!("unsupported url format '{url}'"))?
            .next_back()
            .filter(|seg| !seg.is_empty())
            .ok_or_else(|| anyhow!("'{url}' doesn't appear to be a downloadable file"))
            .map(|s| s.to_string())
    }

    /// Infer file extension from URL or tool name if filename lacks extension.
    /// Returns the filename with inferred extension if needed.
    fn infer_file_extension(&self, filename: &str, url: &Url, tool_name: &str) -> String {
        if filename.contains('.') {
            return filename.to_string();
        }

        let url_str = url.as_str();

        if url_str.contains("win32-x64-archive")
            || url_str.contains("linux-x64")
            || url_str.contains("linux-arm64")
            || url_str.contains(".zip")
            || url_str.contains("archive")
        {
            return format!("{}.zip", filename);
        }

        if url_str.contains(".tar.gz") || url_str.contains(".tgz") {
            return format!("{}.tar.gz", filename);
        }

        if url_str.contains(".tar.xz") {
            return format!("{}.tar.xz", filename);
        }

        if url_str.contains(".7z") {
            return format!("{}.7z", filename);
        }

        if matches!(tool_name, "vscode" | "vscodium" | "codearts-rust") {
            return format!("{}.zip", filename);
        }

        filename.to_string()
    }

    /// Detect file format from content and rename if needed.
    /// Returns the final file path (possibly renamed).
    fn detect_and_rename_file_format(&self, file_path: &Path) -> Result<PathBuf> {
        if !file_path.is_file() || utils::Extractable::is_supported(file_path) {
            return Ok(file_path.to_path_buf());
        }

        let Some(detected_format) = utils::Extractable::detect_format_from_content(file_path)
        else {
            return Ok(file_path.to_path_buf());
        };

        let base_name = file_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or_else(|| {
                file_path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("download")
            });

        let new_name = if detected_format == "gz" {
            if base_name.ends_with(".tar") {
                format!(
                    "{}.tar.gz",
                    base_name.strip_suffix(".tar").unwrap_or(base_name)
                )
            } else {
                format!("{}.tar.gz", base_name)
            }
        } else {
            format!("{}.{}", base_name, detected_format)
        };

        let new_path = file_path
            .parent()
            .ok_or_else(|| anyhow!("file path has no parent: {}", file_path.display()))?
            .join(&new_name);

        if new_path != file_path {
            std::fs::rename(file_path, &new_path)?;
            info!(
                "Detected file format as '{}', renamed '{}' to '{}'",
                detected_format,
                file_path.display(),
                new_path.display()
            );
        }

        Ok(new_path)
    }

    /// Download a tool from URL and install it.
    ///
    /// This function handles:
    /// 1. Creating a temporary directory for download
    /// 2. Determining the filename (from tool info or URL)
    /// 3. Inferring file extension if missing
    /// 4. Downloading the file
    /// 5. Detecting and correcting file format if needed
    /// 6. Installing the tool from the downloaded file
    async fn download_and_try_install(
        &self,
        name: &str,
        url: &Url,
        info: &ToolInfo,
    ) -> Result<ToolRecord> {
        let temp_dir = self.create_temp_dir("download")?;

        let filename = if let Some(fname) = info.filename() {
            fname.to_string()
        } else {
            self.infer_filename_from_url(url)?
        };

        let filename_with_ext = self.infer_file_extension(&filename, url, name);

        let dest = temp_dir.path().join(&filename_with_ext);
        utils::DownloadOpt::new(name, Box::new(self.progress_handler.clone()))
            .with_proxy(self.manifest.proxy_config().cloned())
            .download(url, &dest)
            .await?;

        let final_dest = self.detect_and_rename_file_format(&dest)?;

        self.try_install_from_path(name, &final_dest, info, Some(temp_dir))
    }

    fn try_install_from_path(
        &self,
        name: &str,
        path: &Path,
        info: &ToolInfo,
        dl_temp: Option<TempDir>,
    ) -> Result<ToolRecord> {
        let mut maybe_temp = dl_temp;
        let tool_installer_path = if path.is_dir() {
            path.to_path_buf()
        } else if utils::Extractable::is_supported(path) {
            let extract_temp = self.create_temp_dir(name)?;
            let stop_keyword = match info.kind() {
                Some(rim_common::types::ToolKind::Crate) => None,
                _ => Some("bin"),
            };
            let tool_installer_path =
                self.extract_or_copy_to_with_progress(path, extract_temp.path(), stop_keyword)?;

            let tool_kind = info.kind();
            if !tool_installer_path.is_dir() {
                let is_executable_file =
                    tool_installer_path.is_file() && utils::is_executable(&tool_installer_path);
                if !matches!(tool_kind, Some(ToolKind::Executables)) && !is_executable_file {
                    anyhow::bail!(
                        "Extracted path for '{}' is not a directory: {} (exists: {}, is_file: {})",
                        name,
                        tool_installer_path.display(),
                        tool_installer_path.exists(),
                        tool_installer_path.is_file()
                    );
                }
            }
            maybe_temp = Some(extract_temp);
            tool_installer_path
        } else if path.is_file() {
            path.to_path_buf()
        } else {
            bail!(
                "unable to install '{name}' because the path to it's installer '{}' does not exist.",
                path.display()
            );
        };

        let tool_installer = if let Some(kind) = info.kind() {
            Tool::new(name.into(), kind).with_path(tool_installer_path.as_path())
        } else {
            Tool::from_path(name, &tool_installer_path)
                .with_context(|| format!("no install method for tool '{name}'"))?
        };

        let res = tool_installer.install(self, info);
        drop(maybe_temp);
        res
    }

    pub(crate) async fn install_tool(&mut self, name: &str, tool: &ToolInfo) -> Result<()> {
        self.remove_obsoleted_tools(tool)?;

        let record = match tool {
            ToolInfo::Basic(version) => {
                Tool::cargo_tool(name, Some(vec![name, "--version", version]))
                    .install(self, tool)?
            }
            ToolInfo::Complex(details) => match details.source.as_ref().with_context(|| {
                format!("tool '{name}' cannot be installed because it's lacking a package source")
            })? {
                ToolSource::Version { version } => {
                    Tool::cargo_tool(name, Some(vec![name, "--version", version]))
                        .install(self, tool)?
                }
                ToolSource::Git {
                    git,
                    branch,
                    tag,
                    rev,
                } => {
                    let mut args = vec!["--git", git.as_str()];
                    if let Some(s) = &branch {
                        args.extend(["--branch", s]);
                    }
                    if let Some(s) = &tag {
                        args.extend(["--tag", s]);
                    }
                    if let Some(s) = &rev {
                        args.extend(["--rev", s]);
                    }

                    Tool::cargo_tool(name, Some(args)).install(self, tool)?
                }
                ToolSource::Path { path, .. } => {
                    self.try_install_from_path(name, path, tool, None)?
                }
                ToolSource::Url { url, .. } => {
                    self.download_and_try_install(name, url, tool).await?
                }
                ToolSource::Restricted { source, .. } => {
                    let real_source = source
                        .as_deref()
                        .with_context(|| t!("missing_restricted_source", name = name))?;
                    let maybe_path = PathBuf::from(real_source);
                    if maybe_path.exists() {
                        self.try_install_from_path(name, &maybe_path, tool, None)?
                    } else {
                        self.download_and_try_install(
                            name,
                            &real_source.parse().with_context(|| {
                                format!("'{real_source}' is not an existing path nor a valid URL")
                            })?,
                            tool,
                        )
                        .await?
                    }
                }
            },
        };

        self.install_record.add_tool_record(name, record);

        Ok(())
    }

    pub(crate) async fn install_tools_(
        &mut self,
        use_rust: bool,
        tools: &ToolMap,
        weight: u64,
        errors: &mut InstallationErrors,
    ) -> Result<()> {
        let mut to_install = tools
            .iter()
            .filter(|(_, t)| {
                let requires_toolchain =
                    t.is_cargo_tool() || t.dependencies().iter().any(|s| s == "rust");
                if use_rust {
                    requires_toolchain
                } else {
                    !requires_toolchain
                }
            })
            .collect::<Vec<_>>();

        if use_rust && !self.toolchain_is_installed && !to_install.is_empty() {
            warn!("{}", tl!("skip_tools_no_toolchain"));
            for (name, _) in &to_install {
                errors.add_tool_error(
                    name.to_string(),
                    anyhow::anyhow!(t!("no_toolchain_installed")),
                );
            }
            return self.inc_progress(weight);
        }

        if to_install.is_empty() {
            return self.inc_progress(weight);
        }

        // Safety: We've already checked that to_install is not empty above,
        // but we use checked division for resilience against future code changes.
        let sub_progress_delta = weight.checked_div(to_install.len() as u64).unwrap_or(1);

        to_install = to_install.topological_sorted();
        to_install.reverse();

        for (name, tool) in to_install {
            info!("{}", tl!("installing_tool_info", name = name));
            match self.install_tool(name, tool).await {
                Ok(()) => {
                    self.inc_progress(sub_progress_delta)?;
                }
                Err(e) => {
                    errors.add_tool_error(name.to_string(), e);
                    self.inc_progress(sub_progress_delta)?;
                }
            }
        }

        if let Err(e) = self.install_record.write() {
            errors.add_step_error("保存安装记录".to_string(), e);
        }

        Ok(())
    }

    pub(crate) async fn install_tools(
        &mut self,
        tools: &ToolMap,
        errors: &mut InstallationErrors,
    ) -> Result<()> {
        info!("{}", tl!("install_tools"));
        self.install_tools_(false, tools, 30, errors).await
    }

    /// A step to include `cargo install`, and any tools that requires rust to be installed
    pub(crate) async fn install_tools_late(
        &mut self,
        tools: &ToolMap,
        errors: &mut InstallationErrors,
    ) -> Result<()> {
        info!("{}", tl!("install_via_cargo"));
        self.install_tools_(true, tools, 30, errors).await
    }

    pub(crate) async fn update_tools(
        &mut self,
        tools: &ToolMap,
        errors: &mut InstallationErrors,
    ) -> Result<()> {
        info!("{}", tl!("update_tools"));
        self.install_tools_(false, tools, 15, errors).await?;
        self.install_tools_(true, tools, 15, errors).await?;
        Ok(())
    }

    pub(super) fn remove_obsoleted_tools(&mut self, tool: &ToolInfo) -> Result<()> {
        let obsoleted_tool_names = tool.obsoletes();
        for obsolete in obsoleted_tool_names {
            let Some(rec) = self.install_record.tools.get(obsolete) else {
                continue;
            };
            let Some(tool) = Tool::from_installed(obsolete, rec) else {
                continue;
            };

            info!("{}", tl!("removing_obsolete_tool", name = obsolete));
            tool.uninstall(&*self)?;
            self.install_record.remove_tool_record(obsolete);
        }

        Ok(())
    }
}
