use super::components::{split_components, ToolchainComponent};
use super::dependency_handler::DependencyHandler;
use super::{
    components::Component,
    directories::RimDir,
    parser::{
        cargo_config::CargoConfig,
        fingerprint::{InstallationRecord, ToolRecord},
    },
    rustup::ToolchainInstaller,
    tools::Tool,
    GlobalOpts, CARGO_HOME, RUSTUP_DIST_SERVER, RUSTUP_HOME, RUSTUP_UPDATE_ROOT,
};
use crate::core::baked_in_manifest_raw;
use crate::core::os::add_to_path;
use crate::{default_cargo_registry, default_rustup_dist_server, default_rustup_update_root};
use anyhow::{anyhow, bail, Context, Result};
use log::{error, info, warn};
use rim_common::types::{
    CargoRegistry, TomlParser, ToolInfo, ToolKind, ToolMap, ToolSource, ToolkitManifest,
};
use rim_common::utils::ProgressHandler;
use rim_common::{build_config, utils};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use tempfile::TempDir;
use url::Url;

/// 收集安装过程中的错误信息
#[derive(Debug, Default)]
pub(crate) struct InstallationErrors {
    /// 工具安装错误: (工具名, 错误信息)
    tool_errors: Vec<(String, String)>,
    /// Rust工具链安装错误
    rust_error: Option<String>,
    /// 其他步骤的错误: (步骤名, 错误信息)
    step_errors: Vec<(String, String)>,
}

impl InstallationErrors {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn add_tool_error(&mut self, tool_name: String, error: anyhow::Error) {
        let error_msg = format!("{error:?}");
        error!("安装工具 '{}' 失败: {}", tool_name, error_msg);
        self.tool_errors.push((tool_name, error_msg));
    }

    pub(crate) fn add_rust_error(&mut self, error: anyhow::Error) {
        let error_msg = format!("{error:?}");
        error!("安装Rust工具链失败: {}", error_msg);
        self.rust_error = Some(error_msg);
    }

    pub(crate) fn add_step_error(&mut self, step_name: String, error: anyhow::Error) {
        let error_msg = format!("{error:?}");
        warn!("步骤 '{}' 失败: {}", step_name, error_msg);
        self.step_errors.push((step_name, error_msg));
    }

    pub(crate) fn has_errors(&self) -> bool {
        !self.tool_errors.is_empty() || self.rust_error.is_some() || !self.step_errors.is_empty()
    }

    pub(crate) fn report(&self) {
        if !self.has_errors() {
            return;
        }

        error!("安装过程中发生了一些错误:");
        
        if !self.tool_errors.is_empty() {
            error!("工具安装错误 ({} 个):", self.tool_errors.len());
            for (name, err) in &self.tool_errors {
                error!("  - {}: {}", name, err);
            }
        }

        if let Some(ref err) = self.rust_error {
            error!("Rust工具链安装错误: {}", err);
        }

        if !self.step_errors.is_empty() {
            error!("其他步骤错误 ({} 个):", self.step_errors.len());
            for (step, err) in &self.step_errors {
                error!("  - {}: {}", step, err);
            }
        }
    }
}

const DEFAULT_FOLDER_NAME: &str = "rust";

/// Contains definition of installation steps, including pre-install configs.
pub trait EnvConfig {
    /// Configure environment variables.
    ///
    /// This will set persistent environment variables including
    /// `RUSTUP_DIST_SERVER`, `RUSTUP_UPDATE_ROOT`, `CARGO_HOME`, `RUSTUP_HOME`, etc.
    fn config_env_vars(&self) -> Result<()>;
}

/// Contains every information that the installation process needs.
pub struct InstallConfiguration<'a, T> {
    /// Path to install everything.
    ///
    /// Note that this folder will includes `cargo` and `rustup` folders as well.
    /// And the default location will under `$HOME` directory (`%USERPROFILE%` on windows).
    /// So, even if the user didn't specify any install path, a pair of env vars will still
    /// be written (CARGO_HOME and RUSTUP_HOME), which will be under the default location
    /// defined by [`default_install_dir`].
    pub install_dir: PathBuf,
    /// cargo registry config via user input.
    cargo_registry: Option<CargoRegistry>,
    /// rustup dist server via user input.
    rustup_dist_server: Option<Url>,
    /// rustup update root via user input.
    rustup_update_root: Option<Url>,
    /// Indicates whether the rust toolchain was already installed,
    /// useful when installing third-party tools.
    pub toolchain_is_installed: bool,
    install_record: InstallationRecord,
    pub(crate) progress_handler: T,
    pub(crate) manifest: &'a ToolkitManifest,
    insecure: bool,
}

impl<T> RimDir for &InstallConfiguration<'_, T> {
    fn install_dir(&self) -> &Path {
        self.install_dir.as_path()
    }
}

impl<T> RimDir for InstallConfiguration<'_, T> {
    fn install_dir(&self) -> &Path {
        self.install_dir.as_path()
    }
}

// Basic impl that doesn't require progress handler
impl<T> InstallConfiguration<'_, T> {
    /// Getting the server url that used to download toolchain packages using rustup.
    ///
    /// This is guaranteed to return a value, and it has a fallback order as below:
    /// 1. `rustup-dist-server` from [`ToolkitManifest`]'s config.
    /// 2. `rustup-dist-server` from user input (`self.rustup_dist_server`), such as CLI options.
    /// 3. Default value that is configured through `./configuration.toml`,
    ///    and returned by [`default_rustup_dist_server`].
    pub(crate) fn rustup_dist_server(&self) -> &Url {
        self.manifest
            .config
            .rustup_dist_server
            .as_ref()
            .or(self.rustup_dist_server.as_ref())
            .unwrap_or_else(|| default_rustup_dist_server())
    }

    /// Getting the server url that used to download rustup update.
    ///
    /// This is guaranteed to return a value, and it has a fallback order as below:
    /// 1. `rustup-update-root` from [`ToolkitManifest`]'s config.
    /// 2. `rustup-update-root` from user input (`self.rustup_update_root`), such as CLI options.
    /// 3. Default value that is configured through `./configuration.toml`,
    ///    and returned by [`default_rustup_update_root`].
    pub(crate) fn rustup_update_root(&self) -> &Url {
        self.manifest
            .config
            .rustup_update_root
            .as_ref()
            .or(self.rustup_update_root.as_ref())
            .unwrap_or_else(|| default_rustup_update_root())
    }

    /// Getting the cargo registry config.
    ///
    /// This is guaranteed to return a value, and it has a fallback order as below:
    /// 1. `cargo_registry` from [`ToolkitManifest`]'s config.
    /// 2. `cargo_registry` from user input (`self.cargo_registry`), such as CLI options.
    /// 3. Default value that is configured through `./configuration.toml`,
    ///    and returned by [`default_cargo_registry`].
    pub(crate) fn cargo_registry(&self) -> CargoRegistry {
        self.manifest
            .config
            .cargo_registry
            .clone()
            .or(self.cargo_registry.clone())
            .unwrap_or_else(|| default_cargo_registry().into())
    }

    setter!(
        with_cargo_registry(self.cargo_registry, registry: Option<impl Into<CargoRegistry>>) {
            registry.map(|r| r.into())
        }
    );
    setter!(with_rustup_dist_server(self.rustup_dist_server, Option<Url>));
    setter!(with_rustup_update_root(self.rustup_update_root, Option<Url>));
    setter!(insecure(self.insecure, bool));

    pub(crate) fn env_vars(&self) -> Result<Vec<(&'static str, String)>> {
        let cargo_home = self
            .cargo_home()
            .to_str()
            .map(ToOwned::to_owned)
            .context("`install-dir` cannot contains invalid unicode")?;
        // This `unwrap` is safe here because we've already make sure the `install_dir`'s path can be
        // converted to string with the `cargo_home` variable.
        let rustup_home = self.rustup_home().to_str().unwrap().to_string();

        let mut env_vars = Vec::from([
            (RUSTUP_DIST_SERVER, self.rustup_dist_server().to_string()),
            (RUSTUP_UPDATE_ROOT, self.rustup_update_root().to_string()),
            (RUSTUP_DIST_SERVER, self.rustup_dist_server().to_string()),
            (RUSTUP_UPDATE_ROOT, self.rustup_update_root().to_string()),
            (RUSTUP_DIST_SERVER, self.rustup_dist_server().to_string()),
            (RUSTUP_UPDATE_ROOT, self.rustup_update_root().to_string()),
            (RUSTUP_DIST_SERVER, self.rustup_dist_server().to_string()),
            (RUSTUP_UPDATE_ROOT, self.rustup_update_root().to_string()),
            (CARGO_HOME, cargo_home),
            (RUSTUP_HOME, rustup_home),
        ]);

        // Add proxy settings if has
        if let Some(proxy) = self.manifest.proxy_config() {
            if let Some(url) = &proxy.http {
                env_vars.push(("http_proxy", url.to_string()));
            }
            if let Some(url) = &proxy.https {
                env_vars.push(("https_proxy", url.to_string()));
            }
            if let Some(s) = &proxy.no_proxy {
                // keep use's original no_proxy var.
                #[cfg(windows)]
                let prev_np = std::env::var("no_proxy").unwrap_or_default();
                #[cfg(unix)]
                let prev_np = "$no_proxy";

                let no_proxy = if prev_np.is_empty() {
                    s.to_string()
                } else {
                    format!("{s},{prev_np}")
                };
                env_vars.push(("no_proxy", no_proxy));
            }
        }

        Ok(env_vars)
    }

    /// Creates a temporary directory under `install_dir/temp`, with a certain prefix.
    pub(crate) fn create_temp_dir(&self, prefix: &str) -> Result<TempDir> {
        let root = self.temp_dir();

        tempfile::Builder::new()
            .prefix(&format!("{prefix}_"))
            .tempdir_in(root)
            .with_context(|| format!("unable to create temp directory under '{}'", root.display()))
    }

    /// Perform extraction or copy action base on the given path.
    ///
    /// If `maybe_file` is a path to compressed file, this will try to extract it to `dest`;
    /// otherwise this will copy that file into dest.
    #[allow(dead_code)]
    fn extract_or_copy_to(&self, maybe_file: &Path, dest: &Path) -> Result<PathBuf> {
        if let Ok(extractable) = utils::Extractable::load(maybe_file, None) {
            // For VSCode and similar tools, we want to skip solo directories until we find
            // the actual tool directory (which contains bin/, Code.exe, etc.)
            // Using "bin" as stop keyword will stop at the directory containing bin/
            // Note: extract_then_skip_solo_dir internally calls extract_to, so we don't need to call it separately
            let mut extractable = extractable.quiet(GlobalOpts::get().quiet);
            let extracted_path = extractable.extract_then_skip_solo_dir(dest, Some("bin"))?;
            
            // Ensure the extracted path is actually a directory
            if !extracted_path.is_dir() {
                // If it's not a directory, try to find the parent directory that contains bin/
                if let Some(parent) = extracted_path.parent() {
                    let bin_in_parent = parent.join("bin");
                    if bin_in_parent.exists() && bin_in_parent.is_dir() {
                        info!("Found bin directory in parent, using parent directory: {}", parent.display());
                        return Ok(parent.to_path_buf());
                    }
                }
                anyhow::bail!(
                    "Extracted path is not a directory: {} (exists: {}, is_file: {})",
                    extracted_path.display(),
                    extracted_path.exists(),
                    extracted_path.is_file()
                );
            }
            
            Ok(extracted_path)
        } else {
            utils::copy_into(maybe_file, dest)
        }
    }
}

impl<'a, T: ProgressHandler + Clone + 'static> InstallConfiguration<'a, T> {
    /// Perform extraction or copy action base on the given path, with progress reporting.
    ///
    /// If `maybe_file` is a path to compressed file, this will try to extract it to `dest`;
    /// otherwise this will copy that file into dest.
    /// 
    /// `stop_keyword` is used to determine when to stop skipping solo directories during extraction.
    /// For tools with bin/ directory, use "bin". For crate tools, use None to skip all solo dirs.
    fn extract_or_copy_to_with_progress(&self, maybe_file: &Path, dest: &Path, stop_keyword: Option<&str>) -> Result<PathBuf> {
        // Helper function to recursively extract nested archives
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
            
            // Find the actual directory after extraction
            let entries: Vec<_> = std::fs::read_dir(dest)?
                .filter_map(|e| e.ok())
                .map(|e| e.path())
                .filter(|p| p != archive_path)
                .collect();
            
            // First, check for nested archives that need further extraction
            for entry in &entries {
                if entry.is_file() && utils::Extractable::is_supported(entry) {
                    // Found a nested archive, extract it recursively
                    let nested_handler = Box::new(rim_common::utils::HiddenProgress);
                    return extract_nested_archive(entry, dest, stop_keyword, quiet, nested_handler);
                }
            }
            
            // Look for directories with bin/ or Cargo.toml
            for entry in &entries {
                if entry.is_dir() {
                    let bin_dir = entry.join("bin");
                    let cargo_toml = entry.join("Cargo.toml");
                    if (bin_dir.exists() && bin_dir.is_dir()) || cargo_toml.exists() {
                        return Ok(entry.to_path_buf());
                    }
                }
            }
            
            // If no bin/Cargo.toml found, but there's only one directory, use that
            let dirs: Vec<_> = entries.iter().filter(|p| p.is_dir()).collect();
            if dirs.len() == 1 {
                return Ok(dirs[0].to_path_buf());
            }
            
            // If still no directory found, return dest itself
            Ok(dest.to_path_buf())
        }
        
        if let Ok(mut extractable) = utils::Extractable::load(maybe_file, None) {
            // For VSCode and similar tools, we want to skip solo directories until we find
            // the actual tool directory (which contains bin/, Code.exe, etc.)
            // Using "bin" as stop keyword will stop at the directory containing bin/
            // For crate tools, use None to skip all solo directories, then find Cargo.toml
            // Note: extract_then_skip_solo_dir internally calls extract_to, so we don't need to call it separately
            extractable = extractable
                .quiet(GlobalOpts::get().quiet)
                .with_progress_handler(Box::new(self.progress_handler.clone()));
            let extracted_path = extractable.extract_then_skip_solo_dir(dest, stop_keyword)?;
            
            // Check if extract_then_skip_solo_dir returned a file instead of a directory
            // This can happen if the archive structure is unexpected or extraction returned the original file
            if extracted_path.is_file() {
                // Check if extracted_path is a zip/tar file that needs extraction
                // Don't rely on direct path comparison as paths may differ in format (absolute vs relative, etc.)
                let mut needs_extraction = false;
                
                // If extracted_path is in dest directory, check if it's a supported archive format
                if extracted_path.parent() == Some(dest) {
                    // First try extension-based detection
                    if utils::Extractable::is_supported(&extracted_path) {
                        needs_extraction = true;
                    } else {
                        // Try content-based detection
                        if let Some(detected_format) = utils::Extractable::detect_format_from_content(&extracted_path) {
                            info!("Detected archive format '{}' for file '{}', attempting extraction", 
                                detected_format, extracted_path.display());
                            needs_extraction = true;
                        } else {
                            // Last resort: try to load it as an archive
                            if utils::Extractable::load(&extracted_path, None).is_ok() {
                                info!("Found archive file '{}' in dest directory, attempting extraction", 
                                    extracted_path.display());
                                needs_extraction = true;
                            }
                        }
                    }
                }
                
                // Also check if it's the original file (using canonicalize for reliable comparison)
                // This handles cases where the file was copied/moved but path format changed
                if !needs_extraction {
                    if let (Ok(extracted_canon), Ok(maybe_canon)) = (
                        extracted_path.canonicalize(),
                        maybe_file.canonicalize()
                    ) {
                        if extracted_canon == maybe_canon {
                            needs_extraction = true;
                        }
                    }
                }
                
                if needs_extraction {
                    // Extract nested archive recursively
                    let handler = Box::new(self.progress_handler.clone());
                    return extract_nested_archive(
                        &extracted_path,
                        dest,
                        stop_keyword,
                        GlobalOpts::get().quiet,
                        handler,
                    );
                }
                // If it's not extractable or doesn't need extraction, and it's a file,
                // check if it's an executable file (like cargo-nextest)
                // Also, if it's a file that was extracted (not a compressed archive),
                // it might be an executable that just doesn't have the exec bit set yet
                // In this case, we should allow it and let the caller handle it
                if utils::is_executable(&extracted_path) {
                    return Ok(extracted_path);
                }
                // If it's a file that was extracted from an archive (not the original archive file),
                // it's likely an executable file that needs exec permissions set
                // Allow it to proceed - Tool::from_path will handle it
                if extracted_path != maybe_file && !utils::Extractable::is_supported(&extracted_path) {
                    return Ok(extracted_path);
                }
                // If it's not an executable and not an extracted file, we can't handle it here
                // Fall through to the directory check below which will provide a better error message
            }
            
            // Ensure the extracted path is actually a directory
            if !extracted_path.is_dir() {
                // If it's not a directory, try to find the parent directory that contains bin/
                if let Some(parent) = extracted_path.parent() {
                    let bin_in_parent = parent.join("bin");
                    if bin_in_parent.exists() && bin_in_parent.is_dir() {
                        info!("Found bin directory in parent, using parent directory: {}", parent.display());
                        return Ok(parent.to_path_buf());
                    }
                    // If the extracted path is a file but parent is a directory, check if parent contains the actual tool directory
                    if parent.is_dir() {
                        // List contents of parent directory to find the actual tool directory
                        let entries: Vec<_> = std::fs::read_dir(parent)?
                            .filter_map(|e| e.ok())
                            .map(|e| e.path())
                            .collect();
                        // Look for a directory that contains "bin" subdirectory or Cargo.toml (for crates)
                        for entry in &entries {
                            if entry.is_dir() {
                                let bin_dir = entry.join("bin");
                                let cargo_toml = entry.join("Cargo.toml");
                                if (bin_dir.exists() && bin_dir.is_dir()) || cargo_toml.exists() {
                                    info!("Found tool directory in parent: {}", entry.display());
                                    return Ok(entry.to_path_buf());
                                }
                            }
                        }
                        // If no bin directory found, but there's only one directory in parent, use that
                        let dirs: Vec<_> = entries.iter().filter(|p| p.is_dir()).collect();
                        if dirs.len() == 1 {
                            info!("Using single directory found in parent: {}", dirs[0].display());
                            return Ok(dirs[0].to_path_buf());
                        }
                    }
                }
                anyhow::bail!(
                    "Extracted path is not a directory: {} (exists: {}, is_file: {})",
                    extracted_path.display(),
                    extracted_path.exists(),
                    extracted_path.is_file()
                );
            }
            
            Ok(extracted_path)
        } else {
            // File is not a supported archive format
            // Check if it's an executable file (like cargo-nextest on Linux)
            if maybe_file.is_file() {
                // Try to detect format from content first
                if let Some(detected_format) = utils::Extractable::detect_format_from_content(maybe_file) {
                    // File is actually a compressed archive, but extension was wrong
                    // Try to load it with the detected format
                    if let Ok(mut extractable) = utils::Extractable::load(maybe_file, Some(detected_format)) {
                        extractable = extractable
                            .quiet(GlobalOpts::get().quiet)
                            .with_progress_handler(Box::new(self.progress_handler.clone()));
                        let extracted_path = extractable.extract_then_skip_solo_dir(dest, stop_keyword)?;
                        
                        // Handle the extracted path (same logic as above)
                        if extracted_path.is_file() {
                            // Check if it's an executable file
                            if utils::is_executable(&extracted_path) {
                                return Ok(extracted_path);
                            }
                            // If it's not an executable, check if parent contains the tool directory
                            if let Some(parent) = extracted_path.parent() {
                                if parent.is_dir() {
                                    let entries: Vec<_> = std::fs::read_dir(parent)?
                                        .filter_map(|e| e.ok())
                                        .map(|e| e.path())
                                        .collect();
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
                                }
                            }
                        }
                        
                        if extracted_path.is_dir() {
                            return Ok(extracted_path);
                        }
                    }
                }
                
                // If it's not a compressed archive, check if it's an executable file
                // For executables, we should copy it to dest and return the copied file path
                if utils::is_executable(maybe_file) || maybe_file.extension().is_none() {
                    // It might be an executable file, copy it to dest directory
                    let dest_file = dest.join(maybe_file.file_name().unwrap_or_else(|| std::ffi::OsStr::new("executable")));
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
            
            // For other files, try to copy into dest
            utils::copy_into(maybe_file, dest)
        }
    }

    pub fn new(install_dir: &'a Path, manifest: &'a ToolkitManifest, handler: T) -> Result<Self> {
        let install_record = if InstallationRecord::exists() {
            // TODO: handle existing record, maybe we want to enter manager mode directly?
            InstallationRecord::load_from_config_dir()?
        } else {
            InstallationRecord {
                install_dir: install_dir.to_path_buf(),
                ..Default::default()
            }
        };
        Ok(Self {
            install_dir: install_dir.to_path_buf(),
            install_record,
            cargo_registry: None,
            rustup_dist_server: None,
            rustup_update_root: None,
            toolchain_is_installed: false,
            progress_handler: handler,
            manifest,
            insecure: false,
        })
    }
    /// Creating install directory and other preparations related to filesystem.
    ///
    /// This is suitable for first-time installation.
    pub fn setup(&mut self) -> Result<()> {
        let install_dir = &self.install_dir;
        info!("{}", t!("install_init", dir = install_dir.display()));
        utils::ensure_dir(install_dir)?;

        // Create a copy of the manifest which is later used for component management.
        // NB: This `setup` function only gets called during the first installation,
        // which means this manifest should always loaded from the baked-in one.
        // NB: If this is an offline build, meaning the manifest is likely to contain
        // local paths, which is not useful for adding components afterwards, therefore
        // we better store the online version instead,
        if self.manifest.is_offline {
            ToolkitManifest::from_str(baked_in_manifest_raw(false))?.write_to_dir(install_dir)?;
        } else {
            self.manifest.write_to_dir(install_dir)?;
        }

        // rename this installer to 'xxx-manager' and copy it into installer dir
        let self_exe = std::env::current_exe()?;
        let app_name = build_config().app_name();
        let manager_name = exe!(&app_name);
        let manager_exe = install_dir.join(&manager_name);
        utils::copy_as(self_exe, &manager_exe)?;

        // Write application icon (name: <APP_NAME>.ico) to the install dir for shortcut.
        // Note that this file currently have no use for CLI version, but we still put
        // it there to be future-proof.
        let ico_content = include_bytes!("../../rim_gui/public/favicon.ico");
        let ico_file_dest = install_dir.join(format!("{app_name}.ico"));
        utils::write_bytes(ico_file_dest, ico_content, false)?;

        // soft-link this binary into cargo bin, so it will be in th PATH
        // Note: we are creating two symlinks binary, one have the fullname,
        // and one with shorter name (rim)
        let link_full = self.cargo_bin().join(manager_name);
        let link_short = self.cargo_bin().join(exe!("rim"));
        utils::create_link(&manager_exe, &link_full)
            .with_context(|| format!("unable to create a link as '{}'", link_full.display()))?;
        utils::create_link(&manager_exe, &link_short)
            .with_context(|| format!("unable to create a link as '{}'", link_short.display()))?;

        #[cfg(windows)]
        // Create registry entry to add this program into "installed programs".
        super::os::windows::do_add_to_programs(&manager_exe)?;

        self.inc_progress(5)?;

        Ok(())
    }

    pub async fn install(mut self, components: Vec<Component>) -> Result<()> {
        let mut errors = InstallationErrors::new();
        
        let (tc_components, tools) = split_components(components);
        reject_conflicting_tools(&tools)?;

        self.progress_handler
            .start_master(t!("installing").into(), utils::ProgressKind::Len(100))?;

        // setup 是关键步骤，如果失败应该停止
        self.setup()?;
        
        // 其他步骤失败时记录错误但继续
        if let Err(e) = self.config_env_vars() {
            errors.add_step_error("配置环境变量".to_string(), e);
        }
        
        if let Err(e) = self.config_cargo() {
            errors.add_step_error("配置Cargo".to_string(), e);
        }
        
        // This step taking cares of requirements, such as `MSVC`, also third-party app such as `VS Code`.
        if let Err(e) = self.install_tools(&tools, &mut errors).await {
            errors.add_step_error("安装工具（早期）".to_string(), e);
        }
        
        if let Err(e) = self.install_rust(&tc_components, &mut errors).await {
            errors.add_step_error("安装Rust工具链".to_string(), e);
        }
        
        if let Err(e) = self.install_tools_late(&tools, &mut errors).await {
            errors.add_step_error("安装工具（后期）".to_string(), e);
        }

        // 报告所有错误
        errors.report();

        // 如果有错误，在日志中记录，但不阻止安装流程完成
        if errors.has_errors() {
            warn!("安装完成，但部分组件安装失败。请查看上面的错误信息。");
        }

        self.progress_handler
            .finish_master(t!("install_finished").into())?;
        
        Ok(())
    }

    pub(crate) fn inc_progress(&self, val: u64) -> Result<()> {
        self.progress_handler.update_master(Some(val))
    }

    async fn install_tools_(&mut self, use_rust: bool, tools: &ToolMap, weight: u64, errors: &mut InstallationErrors) -> Result<()> {
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

        // 如果需要 Rust 工具链但工具链未安装，跳过这些工具并记录错误
        if use_rust && !self.toolchain_is_installed && !to_install.is_empty() {
            warn!("Rust 工具链未安装，跳过需要 Rust 的工具安装");
            for (name, _) in &to_install {
                errors.add_tool_error(
                    name.to_string(),
                    anyhow::anyhow!(t!("no_toolchain_installed"))
                );
            }
            return self.inc_progress(weight);
        }

        if to_install.is_empty() {
            return self.inc_progress(weight);
        }

        let sub_progress_delta = weight / to_install.len() as u64;

        to_install = to_install.topological_sorted();
        // topological sort place the tool with more dependencies at the back,
        // which is what we need to install first, therefore we need to reverse it.
        to_install.reverse();

        for (name, tool) in to_install {
            info!("{}", t!("installing_tool_info", name = name));
            match self.install_tool(name, tool).await {
                Ok(()) => {
                    self.inc_progress(sub_progress_delta)?;
                }
                Err(e) => {
                    errors.add_tool_error(name.to_string(), e);
                    // 即使安装失败，也更新进度，避免进度条卡住
                    self.inc_progress(sub_progress_delta)?;
                }
            }
        }

        // 即使有错误，也尝试保存安装记录（已成功安装的工具）
        if let Err(e) = self.install_record.write() {
            errors.add_step_error("保存安装记录".to_string(), e);
        }

        Ok(())
    }

    pub(crate) async fn install_tools(&mut self, tools: &ToolMap, errors: &mut InstallationErrors) -> Result<()> {
        info!("{}", t!("install_tools"));
        self.install_tools_(false, tools, 30, errors).await
    }

    /// A step to include `cargo install`, and any tools that requires rust to be installed
    pub(crate) async fn install_tools_late(&mut self, tools: &ToolMap, errors: &mut InstallationErrors) -> Result<()> {
        info!("{}", t!("install_via_cargo"));
        self.install_tools_(true, tools, 30, errors).await
    }

    /// Install Rust toolchain with a list of components
    pub(crate) async fn install_rust(&mut self, components: &[ToolchainComponent], errors: &mut InstallationErrors) -> Result<()> {
        info!("{}", t!("install_toolchain"));

        let manifest = self.manifest;

        match ToolchainInstaller::init(&*self)
            .insecure(self.insecure)
            .rustup_dist_server(Some(self.rustup_dist_server().clone()))
            .install(self, components)
            .await
        {
            Ok(()) => {
                // 安装成功，继续后续步骤
                if let Err(e) = add_to_path(&*self, self.cargo_bin()) {
                    errors.add_step_error("添加到PATH".to_string(), e);
                } else {
                    self.toolchain_is_installed = true;
                }

                // Add the rust info to the fingerprint.
                self.install_record
                    .add_rust_record(&manifest.toolchain.channel, components);
                // record meta info
                // TODO(?): Maybe this should be moved as a separate step?
                self.install_record
                    .clone_toolkit_meta_from_manifest(manifest);
                // write changes
                if let Err(e) = self.install_record.write() {
                    errors.add_step_error("保存安装记录".to_string(), e);
                }

                self.inc_progress(30)?;
            }
            Err(e) => {
                errors.add_rust_error(e);
                // 即使安装失败，也更新进度
                self.inc_progress(30)?;
            }
        }

        Ok(())
    }

    /// Add toolchain components separately, typically used in `component add`.
    pub async fn install_toolchain_components(
        &mut self,
        components: &[ToolchainComponent],
    ) -> Result<()> {
        ToolchainInstaller::init(&*self)
            .insecure(self.insecure)
            .rustup_dist_server(Some(self.rustup_dist_server().clone()))
            .add_components(self, components)
            .await?;

        self.install_record
            .add_rust_record(&self.manifest.toolchain.channel, components);
        self.install_record.write()?;
        Ok(())
    }

    async fn install_tool(&mut self, name: &str, tool: &ToolInfo) -> Result<()> {
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
                    // the source should be filled before installation, if not, then it means
                    // the program hasn't ask for user input yet, which we should through an error.
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

    async fn download_and_try_install(
        &self,
        name: &str,
        url: &Url,
        info: &ToolInfo,
    ) -> Result<ToolRecord> {
        let temp_dir = self.create_temp_dir("download")?;
        let mut downloaded_file_name: String = if let Some(name) = info.filename() {
            name.to_string()
        } else {
            url.path_segments()
                .ok_or_else(|| anyhow!("unsupported url format '{url}'"))?
                .next_back()
                // Sadly, a path segment could be empty string, so we need to filter that out
                .filter(|seg| !seg.is_empty())
                .ok_or_else(|| anyhow!("'{url}' doesn't appear to be a downloadable file"))?
                .to_string()
        };
        
        // If the downloaded file name doesn't have an extension, try to infer it from the URL or tool name
        if !downloaded_file_name.contains('.') {
            // Check if URL contains hints about file type
            let url_str = url.as_str();
            if url_str.contains("win32-x64-archive") || url_str.contains("linux-x64") || url_str.contains("linux-arm64") {
                // VSCode archives are zip files
                downloaded_file_name = format!("{}.zip", downloaded_file_name);
            } else if url_str.contains(".zip") || url_str.contains("archive") {
                downloaded_file_name = format!("{}.zip", downloaded_file_name);
            } else if url_str.contains(".tar.gz") || url_str.contains(".tgz") {
                downloaded_file_name = format!("{}.tar.gz", downloaded_file_name);
            } else if url_str.contains(".tar.xz") {
                downloaded_file_name = format!("{}.tar.xz", downloaded_file_name);
            } else if url_str.contains(".7z") {
                downloaded_file_name = format!("{}.7z", downloaded_file_name);
            }
            // If still no extension and it's a known tool, try to infer from tool name
            else if name == "vscode" || name == "vscodium" || name == "codearts-rust" {
                downloaded_file_name = format!("{}.zip", downloaded_file_name);
            }
        }
        
        let dest = temp_dir.path().join(downloaded_file_name);
        utils::DownloadOpt::new(name, Box::new(self.progress_handler.clone()))
            .with_proxy(self.manifest.proxy_config().cloned())
            .download(url, &dest)
            .await?;

        // After download, check if the file format can be detected from content
        // If the extension doesn't match the actual format, try to detect and rename
        let final_dest = if dest.is_file() && !utils::Extractable::is_supported(&dest) {
            // File exists but extension-based detection failed, try content-based detection
            if let Some(detected_format) = utils::Extractable::detect_format_from_content(&dest) {
                // Get the base name without extension
                let base_name = dest.file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or_else(|| dest.file_name().and_then(|n| n.to_str()).unwrap_or("download"));
                
                // Construct new filename with detected format
                let new_name = if detected_format == "gz" {
                    // For .gz, check if it's actually .tar.gz by looking at the filename
                    if base_name.ends_with(".tar") {
                        format!("{}.tar.gz", base_name.strip_suffix(".tar").unwrap_or(base_name))
                    } else {
                        format!("{}.tar.gz", base_name)
                    }
                } else {
                    format!("{}.{}", base_name, detected_format)
                };
                
                let new_dest = dest.parent().unwrap().join(&new_name);
                
                // Only rename if the new name is different
                if new_dest != dest {
                    std::fs::rename(&dest, &new_dest)?;
                    info!("Detected file format as '{}', renamed '{}' to '{}'", 
                        detected_format, dest.display(), new_dest.display());
                    new_dest
                } else {
                    dest
                }
            } else {
                dest
            }
        } else {
            dest
        };

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
            // Determine stop keyword based on tool kind
            // For crate tools, use None to skip all solo directories, then find Cargo.toml
            // For other tools, use "bin" to stop at directory containing bin/
            let stop_keyword = match info.kind() {
                Some(rim_common::types::ToolKind::Crate) => None,
                _ => Some("bin"),
            };
            let tool_installer_path = self.extract_or_copy_to_with_progress(path, extract_temp.path(), stop_keyword)?;
            
            // For Executables kind, the extracted path can be a file (single executable)
            // Also, if kind is not specified but the extracted path is a single executable file,
            // we should allow it (it will be auto-detected as Executables by Tool::from_path)
            let tool_kind = info.kind();
            if !tool_installer_path.is_dir() {
                let is_executable_file = tool_installer_path.is_file() && utils::is_executable(&tool_installer_path);
                if matches!(tool_kind, Some(ToolKind::Executables)) || is_executable_file {
                    // For executables, if it's a file, that's fine - it's a single executable
                    // The Tool::new with Executables or Tool::from_path can handle files via PathExt
                    // Continue with the file path
                } else {
                    // For other kinds (like Custom for vscode, Crate for ylong_*), it must be a directory
                    anyhow::bail!(
                        "Extracted path for '{}' is not a directory: {} (exists: {}, is_file: {})",
                        name,
                        tool_installer_path.display(),
                        tool_installer_path.exists(),
                        tool_installer_path.is_file()
                    );
                }
            }
            // we don't need the download temp dir anymore,
            // we should keep the extraction temp dir alive instead.
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

    /// Configuration options for `cargo`.
    ///
    /// This will write a `config.toml` file to `CARGO_HOME`.
    pub fn config_cargo(&self) -> Result<()> {
        info!("{}", t!("install_cargo_config"));

        let mut config = CargoConfig::new();
        let registry = self.cargo_registry();
        config.add_source(&registry.name, &registry.index, true);

        let config_toml = config.to_toml()?;
        if !config_toml.trim().is_empty() {
            let config_path = self.cargo_home().join(CargoConfig::FILENAME);
            utils::write_file(config_path, &config_toml, false)?;
        }

        self.inc_progress(3)?;
        Ok(())
    }
}

// For updates
impl<T: ProgressHandler + Clone + 'static> InstallConfiguration<'_, T> {
    pub async fn update(mut self, components: Vec<Component>) -> Result<()> {
        let mut errors = InstallationErrors::new();
        
        self.progress_handler
            .start_master(t!("installing").into(), utils::ProgressKind::Len(100))?;

        // Create a copy of the manifest which is later used for component management.
        self.manifest.write_to_dir(&self.install_dir)?;

        let (toolchain, tools) = split_components(components);
        // setup env for current process
        for (key, val) in self.env_vars()? {
            std::env::set_var(key, val);
        }
        self.inc_progress(10)?;

        // don't update toolchain if no toolchain components are selected
        if !toolchain.is_empty() {
            if let Err(e) = self.update_toolchain(&toolchain).await {
                errors.add_step_error("更新工具链".to_string(), e);
            }
        }
        
        if let Err(e) = self.update_tools(&tools, &mut errors).await {
            errors.add_step_error("更新工具".to_string(), e);
        }

        // 报告所有错误
        errors.report();

        // 如果有错误，在日志中记录，但不阻止更新流程完成
        if errors.has_errors() {
            warn!("更新完成，但部分组件更新失败。请查看上面的错误信息。");
        }

        self.progress_handler
            .finish_master(t!("install_finished").into())?;
        Ok(())
    }

    async fn update_toolchain(&mut self, components: &[ToolchainComponent]) -> Result<()> {
        info!("{}", t!("update_toolchain"));

        ToolchainInstaller::init(&*self)
            .insecure(self.insecure)
            .update(self, components)
            .await?;

        let record = &mut self.install_record;
        // Add the rust info to the fingerprint.
        record.add_rust_record(&self.manifest.toolchain.channel, components);
        // record meta info
        record.clone_toolkit_meta_from_manifest(self.manifest);
        // write changes
        record.write()?;

        self.inc_progress(60)?;
        Ok(())
    }

    async fn update_tools(&mut self, tools: &ToolMap, errors: &mut InstallationErrors) -> Result<()> {
        info!("{}", t!("update_tools"));
        self.install_tools_(false, tools, 15, errors).await?;
        self.install_tools_(true, tools, 15, errors).await?;
        Ok(())
    }

    fn remove_obsoleted_tools(&mut self, tool: &ToolInfo) -> Result<()> {
        let obsoleted_tool_names = tool.obsoletes();
        for obsolete in obsoleted_tool_names {
            // check if this tool was installed, if yes, get the installation record of it
            let Some(rec) = self.install_record.tools.get(obsolete) else {
                continue;
            };
            let Some(tool) = Tool::from_installed(obsolete, rec) else {
                continue;
            };

            info!("{}", t!("removing_obsolete_tool", name = obsolete));
            tool.uninstall(&*self)?;
            self.install_record.remove_tool_record(obsolete);
        }

        Ok(())
    }
}

// TODO: Conflict resolve should take place during user interaction, not here,
// but it's kind hard to do with how we handle CLI interaction now, figure out a way.
fn reject_conflicting_tools(tools: &ToolMap) -> Result<()> {
    // use a HashSet to collect conflicting pairs to remove duplicates.
    let mut conflicts = HashSet::new();

    for (name, info) in tools {
        for conflicted_name in info.conflicts() {
            // ignore the tools that are not presented in the map
            if !tools.contains_key(conflicted_name) {
                continue;
            }

            // sort the conflicting pairs, so that (A, B) and (B, A) will both
            // resulting to (A, B), thus became unique in the set
            let pair = if name < conflicted_name.as_str() {
                (name, conflicted_name.as_str())
            } else {
                (conflicted_name.as_str(), name)
            };
            conflicts.insert(pair);
        }
    }

    if !conflicts.is_empty() {
        let conflict_list = conflicts
            .into_iter()
            .map(|(a, b)| format!("\t{a} ({})", t!("conflicts_with", name = b)))
            .collect::<Vec<_>>()
            .join("\n");
        bail!("{}:\n{conflict_list}", t!("conflict_detected"));
    }

    Ok(())
}

/// Get the default installation directory,
/// which is a directory under [`home_dir`](utils::home_dir).
pub fn default_install_dir() -> PathBuf {
    rim_common::dirs::home_dir().join(DEFAULT_FOLDER_NAME)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rim_common::utils::HiddenProgress;

    #[test]
    fn detect_package_conflicts() {
        let raw = r#"
a = { version = "0.1.0", conflicts = ["b"] }
b = { version = "0.1.0", conflicts = ["a"] }
c = { version = "0.1.0", conflicts = ["d", "a"] }
"#;
        let map: ToolMap = toml::from_str(raw).unwrap();
        let conflicts = reject_conflicting_tools(&map);

        assert!(conflicts.is_err());

        let error = conflicts.expect_err("has conflicts");
        println!("{error}");
    }

    #[test]
    fn no_proxy_env_var() {
        let raw = r#"
[rust]
version = "1.0.0"

[proxy]
no_proxy = "localhost,.example.com,.foo.com"
"#;

        let manifest = ToolkitManifest::from_str(raw).unwrap();

        let mut cache_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        cache_dir.push("tests");
        cache_dir.push("cache");
        std::fs::create_dir_all(&cache_dir).unwrap();
        let install_root = tempfile::Builder::new().tempdir_in(&cache_dir).unwrap();

        let install_cfg =
            InstallConfiguration::new(install_root.path(), &manifest, HiddenProgress).unwrap();

        // Temporarily modify no_proxy var to test inheritance.
        let no_proxy_backup = std::env::var("no_proxy");
        std::env::remove_var("no_proxy");

        let env_vars = install_cfg.env_vars().unwrap();
        let new_no_proxy_var = &env_vars.iter().find(|(k, _)| *k == "no_proxy").unwrap().1;

        #[cfg(windows)]
        assert_eq!(new_no_proxy_var, "localhost,.example.com,.foo.com");
        #[cfg(unix)]
        assert_eq!(
            new_no_proxy_var,
            "localhost,.example.com,.foo.com,$no_proxy"
        );

        std::env::set_var("no_proxy", ".bar.com,baz.com");
        let env_vars = install_cfg.env_vars().unwrap();
        let new_no_proxy_var = &env_vars.iter().find(|(k, _)| *k == "no_proxy").unwrap().1;

        #[cfg(windows)]
        assert_eq!(
            new_no_proxy_var,
            "localhost,.example.com,.foo.com,.bar.com,baz.com"
        );
        #[cfg(unix)]
        assert_eq!(
            new_no_proxy_var,
            "localhost,.example.com,.foo.com,$no_proxy"
        );

        if let Ok(bck) = no_proxy_backup {
            std::env::set_var("no_proxy", bck);
        }
    }
}
