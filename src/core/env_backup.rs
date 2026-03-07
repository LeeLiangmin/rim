//! Environment variable backup and restore functionality for Windows
//!
//! This module provides functionality to backup and restore environment variables
//! before installation and after uninstallation on Windows platform.

use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use rim_common::utils;

const BACKUP_FILENAME: &str = "env_backup.toml";

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct EnvBackup {
    version: String,
    pub variables: BTreeMap<String, String>,
}

impl EnvBackup {
    pub fn new() -> Self {
        Self {
            version: "1.0".to_string(),
            variables: BTreeMap::new(),
        }
    }

    fn backup_dir() -> Result<PathBuf> {
        let app_data = dirs::data_dir().context("Failed to get AppData directory")?;
        let backup_dir = app_data.join(env!("CARGO_PKG_NAME"));
        utils::ensure_dir(&backup_dir)?;
        Ok(backup_dir)
    }

    fn backup_path() -> Result<PathBuf> {
        Ok(Self::backup_dir()?.join(BACKUP_FILENAME))
    }

    pub fn backup_env_vars(variables: BTreeMap<String, String>) -> Result<Self> {
        let mut backup = Self::new();
        backup.variables = variables;
        backup.save()?;
        Ok(backup)
    }

    fn save(&self) -> Result<()> {
        let backup_path = Self::backup_path()?;
        let toml_content = toml::to_string_pretty(self).context("Failed to serialize backup to TOML")?;
        fs::write(&backup_path, toml_content).with_context(|| format!("Failed to write backup file: {}", backup_path.display()))?;
        Ok(())
    }

    pub fn load() -> Result<Option<Self>> {
        let backup_path = Self::backup_path()?;
        if !backup_path.exists() {
            return Ok(None);
        }
        let content = fs::read_to_string(&backup_path).with_context(|| format!("Failed to read backup file: {}", backup_path.display()))?;
        let backup: EnvBackup = toml::from_str(&content).context("Failed to parse backup TOML")?;
        Ok(Some(backup))
    }

    pub fn delete_backup_file() -> Result<()> {
        let backup_path = Self::backup_path()?;
        if backup_path.exists() {
            fs::remove_file(&backup_path).with_context(|| format!("Failed to delete backup file: {}", backup_path.display()))?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_env_backup_serialization() {
        let mut backup = EnvBackup::new();
        backup.variables.insert("RUSTUP_HOME".to_string(), "/path/to/rustup".to_string());
        backup.variables.insert("CARGO_HOME".to_string(), "/path/to/cargo".to_string());
        let toml_str = toml::to_string_pretty(&backup).unwrap();
        let parsed: EnvBackup = toml::from_str(&toml_str).unwrap();
        assert_eq!(parsed.variables.len(), 2);
        assert_eq!(parsed.variables.get("RUSTUP_HOME").unwrap(), "/path/to/rustup");
        assert_eq!(parsed.variables.get("CARGO_HOME").unwrap(), "/path/to/cargo");
    }

    #[test]
    fn test_env_backup_empty() {
        let backup = EnvBackup::new();
        assert!(backup.variables.is_empty());
    }
}
