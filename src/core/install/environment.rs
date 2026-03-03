use super::InstallConfiguration;
use crate::core::directories::RimDir;
use crate::core::parser::cargo_config::CargoConfig;
use crate::core::{GlobalOpts, CARGO_HOME, RUSTUP_DIST_SERVER, RUSTUP_HOME, RUSTUP_UPDATE_ROOT};
use crate::{default_cargo_registry, default_rustup_dist_server, default_rustup_update_root};
use anyhow::{Context, Result};
use log::info;
use rim_common::types::{CargoRegistry, TomlParser};
use rim_common::utils;
use rim_common::utils::ProgressHandler;
use std::path::{Path, PathBuf};
use tempfile::TempDir;
use url::Url;

/// Contains definition of installation steps, including pre-install configs.
pub trait EnvConfig {
    /// Configure environment variables.
    ///
    /// This will set persistent environment variables including
    /// `RUSTUP_DIST_SERVER`, `RUSTUP_UPDATE_ROOT`, `CARGO_HOME`, `RUSTUP_HOME`, etc.
    fn config_env_vars(&self) -> Result<()>;
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
        // Both cargo_home and rustup_home are created from the same install_dir,
        // so if cargo_home conversion succeeded, rustup_home should also succeed.
        // However, we handle this explicitly for safety and resilience.
        let rustup_home = self
            .rustup_home()
            .to_str()
            .map(ToOwned::to_owned)
            .context("`rustup-home` cannot contains invalid unicode")?;

        let mut env_vars = vec![
            (RUSTUP_DIST_SERVER, self.rustup_dist_server().to_string()),
            (RUSTUP_UPDATE_ROOT, self.rustup_update_root().to_string()),
            (CARGO_HOME, cargo_home),
            (RUSTUP_HOME, rustup_home),
        ];

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
            let mut extractable = extractable.quiet(GlobalOpts::get().quiet);
            let extracted_path = extractable.extract_then_skip_solo_dir(dest, Some("bin"))?;
            
            if !extracted_path.is_dir() {
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

impl<T: ProgressHandler + Clone + 'static> InstallConfiguration<'_, T> {
    /// Configuration options for `cargo`.
    ///
    /// This will write a `config.toml` file to `CARGO_HOME`.
    pub fn config_cargo(&self) -> Result<()> {
        info!("{}", tl!("install_cargo_config"));

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
