//! Windows-specific env backup: reads from registry, restores via registry.

use super::{should_backup_value, FILENAME};
use crate::core::{CARGO_HOME, RUSTUP_HOME, ALL_VARS};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use rim_common::utils;

/// Read the persisted (registry) value of an environment variable as a path.
pub fn read_persisted_env(key: &str) -> Option<PathBuf> {
    read_persisted_env_str(key).map(PathBuf::from)
}

/// Read the persisted (registry) value of an environment variable as a string.
pub fn read_persisted_env_str(key: &str) -> Option<String> {
    use winreg::enums::{HKEY_CURRENT_USER, KEY_READ};
    use winreg::RegKey;

    let env_key = RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey_with_flags("Environment", KEY_READ)
        .ok()?;
    let val: String = env_key.get_value(key).ok()?;
    if val.is_empty() {
        None
    } else {
        Some(val)
    }
}

/// Backup the user's current rustup-related env vars to rim_config_dir
/// before we overwrite them. Reads from Windows registry.
/// Empty values are skipped (user had no Rust config).
pub fn backup_before_overwrite(install_dir: &Path) {
    let mut entries = Vec::new();
    for key in ALL_VARS {
        let Some(value) = read_persisted_env_str(key) else {
            continue;
        };
        if value.is_empty()
            || !should_backup_value(key, &value, install_dir)
            || value.contains('"')
            || value.contains('\n')
        {
            continue;
        }
        entries.push(format!("{key} = \"{value}\""));
    }

    if entries.is_empty() {
        return;
    }

    let backup_path = rim_common::dirs::rim_config_dir().join(FILENAME);
    let content = entries.join("\n");
    if let Err(e) = utils::write_file(&backup_path, &content, false) {
        warn!("Failed to backup env vars for uninstall restoration: {e}");
    } else {
        debug!("Backed up {} env var(s) to {}",
            entries.len(),
            backup_path.display()
        );
    }
}

/// Find the user's original RUSTUP_HOME and CARGO_HOME from backup, or fallback
/// to registry / default locations.
pub fn find_pre_existing_rust_paths(
    rim_install_dir: &Path,
    backup: &HashMap<String, String>,
) -> (Option<PathBuf>, Option<PathBuf>) {
    let from_backup = (
        backup.get(RUSTUP_HOME).map(|s| PathBuf::from(s)),
        backup.get(CARGO_HOME).map(|s| PathBuf::from(s)),
    );
    if from_backup.0.is_some() || from_backup.1.is_some() {
        return from_backup;
    }

    let persisted_rustup =
        read_persisted_env(RUSTUP_HOME).filter(|p| !p.starts_with(rim_install_dir));
    let persisted_cargo =
        read_persisted_env(CARGO_HOME).filter(|p| !p.starts_with(rim_install_dir));

    if persisted_rustup.is_some() || persisted_cargo.is_some() {
        return (persisted_rustup, persisted_cargo);
    }

    let home = rim_common::dirs::home_dir();
    let default_rustup = home.join(".rustup");
    let default_cargo = home.join(".cargo");

    (
        default_rustup.is_dir().then_some(default_rustup),
        default_cargo.is_dir().then_some(default_cargo),
    )
}
