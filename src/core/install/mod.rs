mod environment;
mod errors;
mod toolchain;
mod tools;

pub use environment::EnvConfig;
pub(crate) use errors::InstallationErrors;

use super::components::{split_components, Component};
use super::directories::RimDir;
use super::parser::fingerprint::InstallationRecord;
use crate::core::baked_in_manifest_raw;
use anyhow::{bail, Context, Result};
use log::{info, warn};
use rim_common::types::{TomlParser, ToolMap, ToolkitManifest};
use rim_common::utils;
use rim_common::utils::ProgressHandler;
use rim_common::{build_config, types::CargoRegistry};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use url::Url;

const DEFAULT_FOLDER_NAME: &str = "rust";

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

impl<'a, T: ProgressHandler + Clone + 'static> InstallConfiguration<'a, T> {
    pub fn new(install_dir: &'a Path, manifest: &'a ToolkitManifest, handler: T) -> Result<Self> {
        let install_record = if InstallationRecord::exists() {
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
        info!("{}", tl!("install_init", dir = install_dir.display()));
        utils::ensure_dir(install_dir)?;

        if self.manifest.is_offline {
            ToolkitManifest::from_str(baked_in_manifest_raw(false))?.write_to_dir(install_dir)?;
        } else {
            self.manifest.write_to_dir(install_dir)?;
        }

        let self_exe = std::env::current_exe()?;
        let app_name = build_config().app_name();
        let manager_name = exe!(&app_name);
        let manager_exe = install_dir.join(&manager_name);
        utils::copy_as(self_exe, &manager_exe)?;

        let ico_content = include_bytes!("../../../rim_gui/public/favicon.ico");
        let ico_file_dest = install_dir.join(format!("{app_name}.ico"));
        utils::write_bytes(ico_file_dest, ico_content, false)?;

        let link_full = self.cargo_bin().join(manager_name);
        let link_short = self.cargo_bin().join(exe!("rim"));
        utils::create_link(&manager_exe, &link_full)
            .with_context(|| format!("unable to create a link as '{}'", link_full.display()))?;
        utils::create_link(&manager_exe, &link_short)
            .with_context(|| format!("unable to create a link as '{}'", link_short.display()))?;

        #[cfg(windows)]
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

        self.setup()?;

        if let Err(e) = self.config_env_vars() {
            errors.add_step_error("配置环境变量".to_string(), e);
        }

        if let Err(e) = self.config_cargo() {
            errors.add_step_error("配置Cargo".to_string(), e);
        }

        if let Err(e) = self.install_tools(&tools, &mut errors).await {
            errors.add_step_error("安装工具（早期）".to_string(), e);
        }

        if let Err(e) = self.install_rust(&tc_components, &mut errors).await {
            errors.add_step_error("安装Rust工具链".to_string(), e);
        }

        if let Err(e) = self.install_tools_late(&tools, &mut errors).await {
            errors.add_step_error("安装工具（后期）".to_string(), e);
        }

        errors.report();

        if errors.has_errors() {
            warn!("{}", tl!("install_finished_with_errors"));
        }

        self.progress_handler
            .finish_master(t!("install_finished").into())?;

        Ok(())
    }

    pub(crate) fn inc_progress(&self, val: u64) -> Result<()> {
        self.progress_handler.update_master(Some(val))
    }

    pub async fn update(mut self, components: Vec<Component>) -> Result<()> {
        let mut errors = InstallationErrors::new();

        self.progress_handler
            .start_master(t!("installing").into(), utils::ProgressKind::Len(100))?;

        self.manifest.write_to_dir(&self.install_dir)?;

        let (toolchain, tools) = split_components(components);
        for (key, val) in self.env_vars()? {
            std::env::set_var(key, val);
        }
        self.inc_progress(10)?;

        if !toolchain.is_empty() {
            if let Err(e) = self.update_toolchain(&toolchain).await {
                errors.add_step_error("更新工具链".to_string(), e);
            }
        }

        if let Err(e) = self.update_tools(&tools, &mut errors).await {
            errors.add_step_error("更新工具".to_string(), e);
        }

        errors.report();

        if errors.has_errors() {
            warn!("{}", tl!("update_finished_with_errors"));
        }

        self.progress_handler
            .finish_master(t!("install_finished").into())?;
        Ok(())
    }
}

fn reject_conflicting_tools(tools: &ToolMap) -> Result<()> {
    let mut conflicts = HashSet::new();

    for (name, info) in tools {
        for conflicted_name in info.conflicts() {
            if !tools.contains_key(conflicted_name) {
                continue;
            }

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
