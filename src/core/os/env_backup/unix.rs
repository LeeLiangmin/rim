//! Unix-specific env backup: reads from process env and rc files, restores to shell rc files.
//!
//! Install: backup current values (process env + rc file exports) → remove conflicting exports → add source.
//! Uninstall: remove source → restore from backup.

use super::{should_backup_value, FILENAME};
use crate::core::ALL_VARS;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use rim_common::utils;

/// Parse rc file for existing export/set lines of our vars.
/// Handles: `export KEY="value"` (posix) and `set -Ux KEY "value"` (fish).
fn parse_exports_from_rc(content: &str) -> HashMap<&'static str, String> {
    let mut found = HashMap::new();
    for line in content.lines() {
        let line = line.trim();
        // Posix: export KEY="value"
        if line.starts_with("export ") {
            let rest = line.strip_prefix("export ").unwrap_or(line);
            for key in ALL_VARS {
                let prefix = format!("{key}=");
                if rest.starts_with(&prefix) {
                    if let Some(v) = unquote_value(rest.strip_prefix(&prefix).unwrap_or(rest)) {
                        found.insert(*key, v);
                    }
                    break;
                }
            }
            continue;
        }
        // Fish: set -Ux KEY "value"
        if line.starts_with("set -Ux ") {
            let rest = line.strip_prefix("set -Ux ").unwrap_or(line);
            for key in ALL_VARS {
                let prefix = format!("{key} ");
                if rest.starts_with(&prefix) {
                    if let Some(v) = unquote_value(rest.strip_prefix(&prefix).unwrap_or(rest).trim()) {
                        found.insert(*key, v);
                    }
                    break;
                }
            }
        }
    }
    found
}

fn unquote_value(s: &str) -> Option<String> {
    let s = s.trim();
    let value = s
        .strip_prefix('"')
        .and_then(|s| s.strip_suffix('"'))
        .or_else(|| s.strip_prefix('\'').and_then(|s| s.strip_suffix('\'')))
        .unwrap_or(s);
    if !value.contains('"') && !value.contains('\n') {
        Some(value.to_string())
    } else {
        None
    }
}

/// Backup the user's current rustup-related env vars before we overwrite them.
/// Reads from: 1) current process environment, 2) rc files (to capture config not in process).
/// Process env takes precedence; rc exports fill in keys not in process.
pub fn backup_before_overwrite(install_dir: &Path, rc_files: &[PathBuf]) {
    let mut backup: HashMap<&'static str, String> = HashMap::new();

    // 1. From process env
    for key in ALL_VARS {
        if let Ok(value) = std::env::var(key) {
            if !value.is_empty() && should_backup_value(key, &value, install_dir) {
                if !value.contains('"') && !value.contains('\n') {
                    backup.insert(key, value);
                }
            }
        }
    }

    // 2. From rc files (fill in keys we don't have)
    for rc in rc_files {
        if !rc.is_file() {
            continue;
        }
        let Ok(content) = utils::read_to_string("rc for backup", rc) else {
            continue;
        };
        for (key, value) in parse_exports_from_rc(&content) {
            if !value.is_empty()
                && !backup.contains_key(key)
                && should_backup_value(key, &value, install_dir)
            {
                backup.insert(key, value);
            }
        }
    }

    if backup.is_empty() {
        return;
    }

    let entries: Vec<_> = ALL_VARS
        .iter()
        .filter_map(|k| backup.get(*k).map(|v| format!("{k} = \"{v}\"")))
        .collect();
    let backup_path = rim_common::dirs::rim_config_dir().join(FILENAME);
    let content = entries.join("\n");
    if let Err(e) = utils::write_file(&backup_path, &content, false) {
        warn!("Failed to backup env vars for uninstall restoration: {e}");
    } else {
        debug!(
            "Backed up {} env var(s) to {}",
            entries.len(),
            backup_path.display()
        );
    }
}
