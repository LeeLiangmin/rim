//! Unix-specific env backup: reads from process env, restores to shell rc files.

use super::{should_backup_value, FILENAME};
use crate::core::ALL_VARS;
use std::path::Path;

use rim_common::utils;

/// Backup the user's current rustup-related env vars before we overwrite them.
/// Reads from current process environment (env::var).
pub fn backup_before_overwrite(install_dir: &Path) {
    let mut entries = Vec::new();
    for key in ALL_VARS {
        let Ok(value) = std::env::var(key) else {
            continue;
        };
        if value.is_empty() || !should_backup_value(key, &value, install_dir) {
            continue;
        }
        if value.contains('"') || value.contains('\n') {
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
        debug!(
            "Backed up {} env var(s) to {}",
            entries.len(),
            backup_path.display()
        );
    }
}
