//! Environment variable backup and restore for install/uninstall.
//!
//! Before installation overwrites RUSTUP_HOME, CARGO_HOME, etc., we backup
//! the user's current values. On uninstall, we restore from backup to reduce
//! impact on any pre-existing Rust/rustup installation.

use crate::core::{CARGO_HOME, RUSTUP_HOME, ALL_VARS};
use rim_common::utils;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub const FILENAME: &str = "env-backup.toml";

/// Path vars need install_dir filtering (don't backup if value is under rim's install dir).
pub const PATH_VARS: &[&str] = &[RUSTUP_HOME, CARGO_HOME];

pub fn is_path_var(key: &str) -> bool {
    PATH_VARS.contains(&key)
}

pub fn should_backup_value(key: &str, value: &str, install_dir: &Path) -> bool {
    if is_path_var(key) {
        !PathBuf::from(value).starts_with(install_dir)
    } else {
        true
    }
}

/// Load the backup of user's original env vars saved before install.
/// Returns a map of key -> value. Format: `key = "value"` per line.
pub fn load() -> HashMap<String, String> {
    let path = rim_common::dirs::rim_config_dir().join(FILENAME);
    let content = match utils::read_to_string("env backup", &path) {
        Ok(c) => c,
        Err(_) => return HashMap::new(),
    };

    let mut map = HashMap::new();
    for line in content.lines() {
        let line = line.trim();
        let Some((key, rest)) = line.split_once(" = ") else {
            continue;
        };
        let key = key.trim();
        if !ALL_VARS.contains(&key) {
            continue;
        }
        if let Some(unquoted) = unquote(rest.trim()) {
            map.insert(key.to_string(), unquoted.to_string());
        }
    }
    map
}

fn unquote(s: &str) -> Option<&str> {
    s.trim().strip_prefix('"').and_then(|s| s.strip_suffix('"'))
}

#[cfg(windows)]
pub(crate) mod windows;

#[cfg(unix)]
pub(crate) mod unix;
