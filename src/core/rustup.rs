use std::path::Path;
use std::path::PathBuf;

use anyhow::{Context, Result};
use rim_common::types::Proxy;
use rim_common::types::ToolkitManifest;
use rim_common::utils;
use url::Url;

use super::components::ToolchainComponent;
use super::default_rustup_dist_server;
use super::directories::RimDir;
use super::install::InstallConfiguration;
use super::uninstall::UninstallConfiguration;
use super::GlobalOpts;
use super::ToolkitManifestExt;
use super::CARGO_HOME;
use super::RUSTUP_DIST_SERVER;
use super::RUSTUP_HOME;

#[cfg(windows)]
pub(crate) const RUSTUP_INIT: &str = "rustup-init.exe";
#[cfg(not(windows))]
pub(crate) const RUSTUP_INIT: &str = "rustup-init";

#[cfg(windows)]
const RUSTUP: &str = "rustup.exe";
#[cfg(not(windows))]
const RUSTUP: &str = "rustup";

pub struct ToolchainInstaller {
    insecure: bool,
}

impl ToolchainInstaller {
    pub(crate) fn init<T: RimDir>(config: T) -> Self {
        let cargo_home = config.cargo_home().to_path_buf();
        let rustup_home = config.rustup_home().to_path_buf();

        std::env::set_var(CARGO_HOME, cargo_home);
        std::env::set_var(RUSTUP_HOME, rustup_home);

        // this env var interfering our installation, may causing incorrect version being installed
        std::env::remove_var("RUSTUP_TOOLCHAIN");
        // skip path check, as it shows an `error: cannot install while Rust is installed`.
        // Although it's not a big deal since we use `-y` when executing `rustup-init`,
        // some user find this error message a bit concerning.
        std::env::set_var("RUSTUP_INIT_SKIP_PATH_CHECK", "yes");

        Self { insecure: false }
    }

    setter!(insecure(self.insecure, bool));

    /// Install toolchain including optional set of components.
    ///
    /// If `first_install` flag was set to `false`, meaning this is likely an
    /// update operation, thus will not try to use offline dist server and
    /// will not try to remove `rustup`'s uninstallation entry on Windows.
    fn install_toolchain_with_components(
        &self,
        config: &InstallConfiguration,
        components: &[ToolchainComponent],
        first_install: bool,
    ) -> Result<()> {
        ensure_rustup_dist_server_env(config.manifest, self.insecure, first_install)?;

        let rustup = &ensure_rustup(config, self.insecure)?;
        // if this is the first time installing the tool chain, we need to add the base components
        // from the manifest.
        let mut base = if first_install {
            config.manifest.rust.components.clone()
        } else {
            vec![]
        };
        base.extend(
            components
                .iter()
                .filter_map(|c| (!c.is_profile).then_some(c.name.clone())),
        );
        let components_arg = base.join(",");

        let version = &config.manifest.rust.channel;
        let mut cmd = cmd!(
            rustup,
            "toolchain",
            "install",
            version,
            "--no-self-update",
            "-c",
            &components_arg
        );
        if let Some(profile) = config.manifest.rust.profile() {
            cmd.args(["--profile", profile]);
        }

        // install the toolchain
        utils::execute(cmd)?;
        // set it as default
        run!(rustup, "-q", "default", version)?;

        // Remove the `rustup` uninstall entry on windows, because we don't want users to
        // accidentally uninstall `rustup` thus removing the tools installed by this program.
        #[cfg(windows)]
        if first_install {
            _ = super::os::windows::do_remove_from_programs(
                r"Software\Microsoft\Windows\CurrentVersion\Uninstall\Rustup",
            );
        }

        Ok(())
    }

    /// Install rust toolchain & components via rustup.
    pub(crate) fn install(
        &self,
        config: &InstallConfiguration,
        components: &[ToolchainComponent],
    ) -> Result<()> {
        self.install_toolchain_with_components(config, components, true)
    }

    /// Update rust toolchain by invoking `rustup toolchain add`, then `rustup default`.
    pub(crate) fn update(
        &self,
        config: &InstallConfiguration,
        components: &[ToolchainComponent],
    ) -> Result<()> {
        self.install_toolchain_with_components(config, components, false)
    }

    /// Install components via rustup.
    pub(crate) fn add_components(
        &self,
        config: &InstallConfiguration,
        components: &[ToolchainComponent],
    ) -> Result<()> {
        if components.is_empty() || components.iter().all(|c| c.is_profile) {
            return Ok(());
        }

        ensure_rustup_dist_server_env(config.manifest, self.insecure, false)?;
        let rustup = &ensure_rustup(config, self.insecure)?;

        // check if toolchain is installed
        let version = &config.manifest.rust.channel;
        let mut toolchain_list_cmd = cmd!(rustup, "toolchain", "list");
        let toolchain_list_output = String::from_utf8(toolchain_list_cmd.output()?.stdout)?;
        if toolchain_list_output
            .split('\n')
            .any(|line| line.starts_with(version))
        {
            // if toolchain is installed, add the component directly
            let mut cmd = cmd!(rustup, "component", "add");
            let comp_args = components
                .iter()
                .filter_map(|c| (!c.is_profile).then_some(&c.name));
            info!(
                "{}",
                t!(
                    "install_toolchain_components",
                    list = comp_args
                        .clone()
                        .map(|s| s.as_str())
                        .collect::<Vec<_>>()
                        .join(",")
                )
            );

            cmd.args(comp_args);
            utils::execute(cmd)
        } else {
            // otherwise install the toolchain with the components
            self.install_toolchain_with_components(config, components, false)
        }
    }

    pub(crate) fn remove_components(
        &self,
        config: &UninstallConfiguration,
        components: &[ToolchainComponent],
    ) -> Result<()> {
        if components.is_empty() || components.iter().all(|c| c.is_profile) {
            return Ok(());
        }

        let rustup_bin = config.cargo_bin().join(RUSTUP);
        if !rustup_bin.is_file() {
            // rustup not installed, perhaps user already remove it manually?
            // Therefore nothing needs to be done
            return Ok(());
        }

        let comp_args = components
            .iter()
            .filter_map(|c| (!c.is_profile).then_some(&c.name));

        info!(
            "{}",
            t!(
                "uninstall_toolchain_components",
                list = comp_args
                    .clone()
                    .map(|s| s.as_str())
                    .collect::<Vec<_>>()
                    .join(",")
            )
        );

        let mut cmd = cmd!(rustup_bin, "component", "remove");
        cmd.args(comp_args);
        utils::execute(cmd)?;
        Ok(())
    }

    // Rustup self uninstall all the components and toolchains.
    pub(crate) fn remove_self(&self, config: &UninstallConfiguration) -> Result<()> {
        info!("{}", t!("uninstalling_rust_toolchain"));

        // On Windows, `rustup self uninstall` is extremely slow because it removes
        // `RUSTUP_HOME` which contains `toolchains/<ver>/share/doc/rust/html/` —
        // tens of thousands of tiny HTML files.  NTFS metadata updates + Windows
        // Defender real-time scanning make this take minutes with high CPU/fan.
        //
        // Workaround: **rename** the heavy sub-directories to a sibling trash
        // location (instant on the same NTFS volume), then after `rustup` finishes,
        // spawn a **detached background process** (`rd /s /q`) to delete them.
        // The user sees ~1s instead of minutes.
        #[cfg(windows)]
        let _background_cleanup = {
            let rustup_home = config.rustup_home();
            let mut trash_dir: Option<std::path::PathBuf> = None;

            if rustup_home.exists() {
                // Create a trash directory OUTSIDE install_dir — MUST be on the
                // same NTFS volume for rename to be instant.  We go to the parent
                // of install_dir so that the later walk_dir in `remove_self` won't
                // stumble over the background-delete remnants.
                let trash = config
                    .install_dir()
                    .parent()
                    .unwrap_or(config.install_dir())
                    .join(format!(".rustup-cleanup-{}", std::process::id()));
                if std::fs::create_dir_all(&trash).is_ok() {
                    let mut moved_something = false;
                    for subdir in &["toolchains", "downloads", "tmp", "update-hashes"] {
                        let src = rustup_home.join(subdir);
                        if src.is_dir() {
                            let dest = trash.join(subdir);
                            debug!("moving '{}' -> '{}'", src.display(), dest.display());
                            if std::fs::rename(&src, &dest).is_ok() {
                                moved_something = true;
                            }
                        }
                    }
                    if moved_something {
                        trash_dir = Some(trash);
                    } else {
                        _ = std::fs::remove_dir(&trash);
                    }
                }
            }
            trash_dir
        };

        let rustup = config.cargo_bin().join(RUSTUP);
        let uninstall_result = run!(rustup, "self", "uninstall", "-y");

        #[cfg(windows)]
        {
            if let Err(err) = uninstall_result {
                // `rustup self uninstall` may still fail due to transient file locks
                // (antivirus, indexer).  Fall back to removing RUSTUP_HOME ourselves.
                warn!("{}: {err}", t!("uninstall_rust_toolchain_failed"));
                if let Err(e) = windows_remove_dir_with_retry(config.rustup_home()) {
                    warn!("failed to remove RUSTUP_HOME after retries: {e}");
                }
            }

            // Spawn a detached background process to delete the trash directory.
            // This runs after rustup is done so the user is not blocked.
            // Always attempt cleanup regardless of earlier errors.
            if let Some(trash) = _background_cleanup {
                windows_background_delete(&trash);
            }
        }

        #[cfg(not(windows))]
        uninstall_result?;

        info!("{}", t!("rust_toolchain_uninstalled"));
        Ok(())
    }
}

/// Spawn a fully detached `cmd /c rd /s /q` process to delete a directory tree
/// **in the background**.  The caller does not wait for it to finish.
///
/// Uses `DETACHED_PROCESS | CREATE_NO_WINDOW` creation flags so that the
/// spawned process is **not** attached to the parent's console.  This way,
/// closing the console window (or the parent process exiting) will not
/// terminate the background deletion.
#[cfg(windows)]
fn windows_background_delete(path: &Path) {
    use std::os::windows::process::CommandExt;
    use std::process::{Command, Stdio};

    // Detach from parent console so closing the window won't kill this child.
    const CREATE_NO_WINDOW: u32 = 0x08000000;
    const DETACHED_PROCESS: u32 = 0x00000008;

    if !path.exists() {
        return;
    }

    debug!("spawning background deletion of '{}'", path.display());

    let _ = Command::new("cmd")
        .args(["/c", "rd", "/s", "/q"])
        .arg(path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .creation_flags(CREATE_NO_WINDOW | DETACHED_PROCESS)
        .spawn();
}

/// Remove a directory on Windows with retries, handling transient permission errors
/// caused by antivirus software, search indexers, etc.
#[cfg(windows)]
fn windows_remove_dir_with_retry(path: &Path) -> Result<()> {
    use std::io::ErrorKind;

    if !path.exists() {
        return Ok(());
    }

    const RETRY_TIMES: u8 = 5;
    for attempt in 1..=RETRY_TIMES {
        match std::fs::remove_dir_all(path) {
            Ok(()) => return Ok(()),
            Err(err) if err.kind() == ErrorKind::NotFound => return Ok(()),
            Err(err) if err.kind() == ErrorKind::PermissionDenied => {
                warn!(
                    "unable to remove '{}', retrying ({attempt}/{RETRY_TIMES}): {err}",
                    path.display()
                );
                std::thread::sleep(std::time::Duration::from_secs(2));
            }
            Err(err) => return Err(err.into()),
        }
    }

    anyhow::bail!(
        "failed to remove '{}' after {RETRY_TIMES} retries",
        path.display()
    )
}

fn ensure_rustup_dist_server_env(
    manifest: &ToolkitManifest,
    insecure: bool,
    use_offline_server: bool,
) -> Result<()> {
    if use_offline_server && manifest.rust.offline_dist_server.is_some() {
        let local_server = manifest
            .offline_dist_server()?
            .unwrap_or_else(|| unreachable!("already checked in if condition"));
        info!(
            "{}",
            t!("use_offline_dist_server", url = local_server.as_str())
        );
        std::env::set_var(RUSTUP_DIST_SERVER, local_server.as_str());
    } else {
        let mut server: Url = default_rustup_dist_server().clone();
        if server.scheme() == "https" && insecure {
            warn!("{}", t!("insecure_http_override"));
            // the old scheme is `https` and new scheme is `http`, meaning that this
            // is guaranteed to be `Ok`.
            server.set_scheme("http").unwrap();
        }
        std::env::set_var(RUSTUP_DIST_SERVER, server.as_str());
    }

    Ok(())
}

fn ensure_rustup(config: &InstallConfiguration, insecure: bool) -> Result<PathBuf> {
    let rustup_bin = config.cargo_bin().join(RUSTUP);
    if rustup_bin.exists() {
        return Ok(rustup_bin);
    }

    // Run the bundled rustup-init or download it from server.
    // NOTE: When running updates, the manifest we cached might states that it has a bundled
    // rustup-init, but in reality it might not exists, therefore we need to check if that file
    // exists and download it otherwise.
    let (rustup_init, maybe_temp_dir) =
        if let Some(bundled_rustup) = &config.manifest.rustup_bin()?.filter(|p| p.is_file()) {
            (bundled_rustup.to_path_buf(), None)
        } else {
            // We are putting the binary here so that it will be deleted automatically after done.
            let temp_dir = config.create_temp_dir("rustup-init")?;
            let rustup_init = temp_dir.path().join(RUSTUP_INIT);
            // Download rustup-init.
            download_rustup_init(
                &rustup_init,
                &config.rustup_update_root,
                config.manifest.proxy.as_ref(),
                insecure,
            )?;
            (rustup_init, Some(temp_dir))
        };

    install_rustup(&rustup_init)?;
    // We don't need the rustup-init anymore, drop the whole temp dir containing it.
    drop(maybe_temp_dir);

    Ok(rustup_bin)
}

fn download_rustup_init(
    dest: &Path,
    server: &Url,
    proxy: Option<&Proxy>,
    insecure: bool,
) -> Result<()> {
    info!("{}", t!("downloading_rustup_init"));

    let download_url = utils::url_join(server, format!("dist/{}/{RUSTUP_INIT}", env!("TARGET")))
        .context("Failed to init rustup download url.")?;
    utils::DownloadOpt::new(RUSTUP_INIT, GlobalOpts::get().quiet)
        .insecure(insecure)
        .with_proxy(proxy.cloned())
        .blocking_download(&download_url, dest)
        .context("Failed to download rustup.")
}

fn install_rustup(rustup_init: &PathBuf) -> Result<()> {
    // make sure it can be executed
    utils::set_exec_permission(rustup_init)?;

    let mut args = vec![
        // tell rustup not to add `. $HOME/.cargo/env` because we already wrote one for them.
        "--no-modify-path",
        "--default-toolchain",
        "none",
        "--default-host",
        env!("TARGET"),
        "-y",
    ];
    if GlobalOpts::get().verbose {
        args.push("-v");
    } else if GlobalOpts::get().quiet {
        args.push("-q");
    }
    let mut cmd = cmd!(rustup_init);
    cmd.args(args);
    utils::execute(cmd)?;
    Ok(())
}
