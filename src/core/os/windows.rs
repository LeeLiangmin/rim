use std::env::current_exe;

use crate::core::directories::RimDir;
use crate::core::env_backup::EnvBackup;
use crate::core::install::{EnvConfig, InstallConfiguration};
use crate::core::uninstall::{UninstallConfiguration, Uninstallation};
use crate::core::GlobalOpts;
use anyhow::Result;
use rim_common::utils;

pub(crate) use rustup::*;

impl EnvConfig for InstallConfiguration<'_> {
    fn config_env_vars(&self) -> Result<()> {
        info!("{}", t!("install_env_config"));

        // Backup existing environment variables from registry before setting new ones
        let pre_existing = {
            let mut vars = std::collections::BTreeMap::new();
            if let Ok(env) = environment() {
                for var_name in crate::core::ALL_VARS {
                    if let Ok(value) = env.get_value::<String, _>(var_name) {
                        vars.insert(var_name.to_string(), value);
                    }
                }
            }
            vars
        };
        if let Err(e) = EnvBackup::backup_env_vars(pre_existing) {
            warn!("{}", t!("backup_env_vars_fail", error = e.to_string()));
        } else {
            info!("{}", t!("backup_env_vars_success"));
        }

        for (key, val) in self.env_vars()? {
            set_env_var(key, val.encode_utf16().collect())?;
        }
        update_env();

        self.inc_progress(2.0)
    }
}

impl Uninstallation for UninstallConfiguration<'_> {
    fn remove_rustup_env_vars(&self) -> Result<()> {
        // Remove the `<InstallDir>/.cargo/bin` which is added by rustup
        let cargo_bin_dir = self.cargo_home().join("bin");
        if let Err(e) = remove_from_path(&cargo_bin_dir) {
            warn!("failed to remove cargo bin from PATH: {e}");
        }

        // Try to restore environment variables from backup first
        match EnvBackup::load() {
            Ok(Some(backup)) => {
                info!("{}", t!("restore_env_vars_from_backup"));
                let mut all_restored = true;
                for (var_name, value) in &backup.variables {
                    // Restore the original value
                    if let Err(e) = set_persist_env_var(var_name, value.encode_utf16().collect()) {
                        warn!("{}", t!("restore_env_var_fail", var = var_name, error = e.to_string()));
                        all_restored = false;
                    } else {
                        info!("{}", t!("restore_env_var_success", var = var_name));
                    }
                }
                // Unset variables that were added by the installer (not in the backup)
                for var_to_remove in crate::core::ALL_VARS {
                    if !backup.variables.contains_key(*var_to_remove) {
                        if let Err(e) = unset_env_var(var_to_remove) {
                            warn!("{}", t!("unset_env_var_fail", var = var_to_remove, error = e.to_string()));
                        }
                    }
                }
                // Only delete the backup file if all restorations succeeded,
                // so the user can retry if some variables failed to restore.
                if all_restored {
                    if let Err(e) = EnvBackup::delete_backup_file() {
                        warn!("{}", t!("delete_backup_file_fail", error = e.to_string()));
                    }
                } else {
                    warn!("{}", t!("backup_not_deleted_partial_restore"));
                }
            }
            Ok(None) => {
                // No backup file exists, remove the environment variables as before
                info!("{}", t!("no_env_backup_found_removing_vars"));
                for var_to_remove in crate::core::ALL_VARS {
                    unset_env_var(var_to_remove)?;
                }
            }
            Err(e) => {
                warn!("{}", t!("load_backup_fail", error = e.to_string()));
                // Fallback to removing variables
                for var_to_remove in crate::core::ALL_VARS {
                    unset_env_var(var_to_remove)?;
                }
            }
        }

        update_env();

        Ok(())
    }

    fn remove_self(&self) -> Result<()> {
        do_remove_from_programs(uninstall_entry())?;
        remove_from_path(&self.install_dir)?;

        let current_exe = current_exe()?;
        // On windows, we cannot delete an executable that is currently running.
        // So, let's remove what we can, and **hopefully** that will only left us
        // this binary, and its parent directory (aka.`install_dir`)
        for entry in utils::walk_dir(&self.install_dir, true)?.iter().rev() {
            if utils::remove(entry).is_err() {
                if entry == &current_exe || entry == &self.install_dir {
                    // we'll deal with these two later
                    continue;
                }
                warn!("{}", t!("unable_to_remove", path = entry.display()));
            }
        }

        // remove current exe
        self_replace::self_delete()?;
        // remove parent dir, which should be empty by now, and should be very quick to remove.
        // but if for some reason it fails, well it's too late then, the `self` binary is gone now.
        _ = utils::remove(&self.install_dir);
        Ok(())
    }
}

/// A module that contains functions that are modified from `rustup`:
/// https://github.com/rust-lang/rustup/blob/master/src/cli/self_update/windows.rs
pub(crate) mod rustup {
    use super::{utils, GlobalOpts};
    use anyhow::{anyhow, Context, Result};
    use std::env;
    use std::ffi::OsString;
    use std::os::windows::ffi::{OsStrExt, OsStringExt};
    use std::path::Path;
    use std::sync::OnceLock;
    use winapi::shared::minwindef;
    use winapi::um::winuser;
    use winreg::enums::{RegType, HKEY_CURRENT_USER, KEY_READ, KEY_WRITE};
    use winreg::{RegKey, RegValue};

    static UNINSTALL_ENTRY: OnceLock<String> = OnceLock::new();

    pub(super) fn uninstall_entry() -> &'static str {
        UNINSTALL_ENTRY.get_or_init(|| {
            format!(
                "Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\{}",
                env!("CARGO_PKG_NAME")
            )
        })
    }

    pub(crate) fn do_add_to_programs(program_bin: &Path) -> Result<()> {
        use std::path::PathBuf;

        let key = RegKey::predef(HKEY_CURRENT_USER)
            .create_subkey(uninstall_entry())
            .context("Failed creating uninstall key")?
            .0;

        // Don't overwrite registry if Rustup is already installed
        let prev = key
            .get_raw_value("UninstallString")
            .map(|val| from_winreg_value(&val));
        if let Ok(Some(s)) = prev {
            let mut path = PathBuf::from(OsString::from_wide(&s));
            path.pop();
            if path.exists() {
                return Ok(());
            }
        }

        let mut uninstall_cmd = OsString::from("\"");
        uninstall_cmd.push(program_bin);
        uninstall_cmd.push("\"");
        uninstall_cmd.push(" uninstall");

        let reg_value = RegValue {
            bytes: to_winreg_bytes(uninstall_cmd.encode_wide().collect()),
            vtype: RegType::REG_SZ,
        };

        key.set_raw_value("UninstallString", &reg_value)
            .context("Failed to set `UninstallString`")?;
        let product = utils::build_cfg_locale("product");
        key.set_value("DisplayName", &product)
            .context("Failed to set `DisplayName`")?;

        Ok(())
    }

    /// This is used to decode the value of HKCU\Environment\PATH. If that key is
    /// not REG_SZ | REG_EXPAND_SZ then this returns None. The winreg library itself
    /// does a lossy unicode conversion.
    fn from_winreg_value(val: &winreg::RegValue) -> Option<Vec<u16>> {
        use std::slice;

        match val.vtype {
            RegType::REG_SZ | RegType::REG_EXPAND_SZ => {
                // Copied from winreg
                let mut words = unsafe {
                    slice::from_raw_parts(val.bytes.as_ptr().cast::<u16>(), val.bytes.len() / 2)
                        .to_owned()
                };
                while words.last() == Some(&0) {
                    words.pop();
                }
                Some(words)
            }
            _ => None,
        }
    }

    /// Convert a vector UCS-2 chars to a null-terminated UCS-2 string in bytes
    fn to_winreg_bytes(mut v: Vec<u16>) -> Vec<u8> {
        v.push(0);
        unsafe { std::slice::from_raw_parts(v.as_ptr().cast::<u8>(), v.len() * 2).to_vec() }
    }

    pub(crate) fn do_remove_from_programs(entry: &str) -> Result<()> {
        match RegKey::predef(HKEY_CURRENT_USER).delete_subkey_all(entry) {
            Ok(()) => Ok(()),
            Err(ref e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(anyhow!(e)),
        }
    }

    pub(super) fn environment() -> Result<RegKey> {
        RegKey::predef(HKEY_CURRENT_USER)
            .open_subkey_with_flags("Environment", KEY_READ | KEY_WRITE)
            .context("Failed opening Environment key")
    }

    /// Get the current user's PATH variable out of the registry as a String.
    ///
    /// If this returns None then the PATH variable is not a string and we
    /// should not mess with it.
    pub(super) fn get_user_path_var() -> Result<Option<Vec<u16>>> {
        let environment = environment()?;

        let reg_value = environment.get_raw_value("PATH");
        match reg_value {
            Ok(val) => {
                if let Some(s) = from_winreg_value(&val) {
                    Ok(Some(s))
                } else {
                    warn!("{}", t!("windows_not_modify_path_warn"));
                    Ok(None)
                }
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Some(Vec::new())),
            Err(e) => Err(anyhow!(e)),
        }
    }

    /// Set the environment variable `key` with a given `value`.
    ///
    /// This will modify the environment permanently for current user,
    /// as well as for current running process.
    pub(super) fn set_env_var(key: &str, val: Vec<u16>) -> Result<()> {
        // Set for current process
        env::set_var(key, OsString::from_wide(&val));
        set_persist_env_var(key, val)?;

        Ok(())
    }

    /// Remove a environment variable with given `key`
    ///
    /// This will modify the environment permanently for current user,
    /// as well as for current running process.
    pub(super) fn unset_env_var(key: &str) -> Result<()> {
        // Delete for current process
        env::remove_var(key);
        set_persist_env_var(key, vec![])?;

        Ok(())
    }

    /// Set or remove env var using windows api.
    pub(super) fn set_persist_env_var(key: &str, val: Vec<u16>) -> Result<()> {
        if GlobalOpts::get().no_modify_env() {
            return Ok(());
        }

        let env = environment()?;
        if val.is_empty() {
            // remove
            match env.get_raw_value(key) {
                // Delete for user environment
                Ok(_) => env.delete_value(key)?,
                // Don't do anything if the variable doesn't exist
                Err(ref e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
                Err(e) => {
                    return Err(anyhow!(
                        "failed to read existing environment variable `{key}` before removing: {e}"
                    ));
                }
            }
        } else {
            // set var
            let reg_value = RegValue {
                bytes: to_winreg_bytes(val),
                vtype: RegType::REG_EXPAND_SZ,
            };
            // Set for user environment
            env.set_raw_value(key, &reg_value)?;
        }

        Ok(())
    }

    /// Broadcast environment changes to other processes,
    /// required after making env changes.
    pub(super) fn update_env() {
        unsafe {
            winuser::SendMessageTimeoutA(
                winuser::HWND_BROADCAST,
                winuser::WM_SETTINGCHANGE,
                0 as minwindef::WPARAM,
                c"Environment".as_ptr() as minwindef::LPARAM,
                winuser::SMTO_ABORTIFHUNG,
                5000,
                std::ptr::null_mut(),
            );
        }
    }

    fn lower_ascii_u16(ch: u16) -> u16 {
        if (b'A' as u16..=b'Z' as u16).contains(&ch) {
            ch + 32
        } else {
            ch
        }
    }

    fn eq_u16_ignore_ascii_case(lhs: &[u16], rhs: &[u16]) -> bool {
        lhs.len() == rhs.len()
            && lhs
                .iter()
                .zip(rhs.iter())
                .all(|(l, r)| lower_ascii_u16(*l) == lower_ascii_u16(*r))
    }

    /// Attempt to find the exact entry position of given path in the `PATH` environment variable.
    ///
    /// Return `(start, end)` (both byte index in UTF-16 code units), where `end` is exclusive.
    /// Matching is case-insensitive for ASCII characters and boundary-aware (split by ';').
    fn find_path_in_env(paths: &[u16], path_bytes: &[u16]) -> Option<(usize, usize)> {
        if path_bytes.is_empty() {
            return None;
        }

        let mut start = 0;
        for (idx, ch) in paths.iter().enumerate() {
            if *ch == b';' as u16 {
                let entry = &paths[start..idx];
                if eq_u16_ignore_ascii_case(entry, path_bytes) {
                    return Some((start, idx));
                }
                start = idx + 1;
            }
        }

        let entry = &paths[start..];
        if eq_u16_ignore_ascii_case(entry, path_bytes) {
            return Some((start, paths.len()));
        }

        None
    }

    fn normalize_path_for_cmp(path: &Path) -> String {
        let mut s = path.as_os_str().to_string_lossy().replace('/', "\\");
        while s.ends_with('\\') && !s.ends_with(":\\") && s.len() > 1 {
            s.pop();
        }
        s.to_ascii_lowercase()
    }

    fn path_eq_ignore_case(lhs: &Path, rhs: &Path) -> bool {
        normalize_path_for_cmp(lhs) == normalize_path_for_cmp(rhs)
    }

    /// Add or remove a path for current running process.
    pub(super) fn set_path_for_current_process(path: &Path, is_remove: bool) -> Result<()> {
        let orig_path = env::var_os("PATH");
        match (orig_path, is_remove) {
            (Some(path_oss), false) => {
                let mut path_list = env::split_paths(&path_oss).collect::<Vec<_>>();
                if path_list.iter().any(|p| path_eq_ignore_case(p.as_path(), path)) {
                    return Ok(());
                }
                path_list.insert(0, path.to_path_buf());
                env::set_var("PATH", env::join_paths(path_list)?);
            }
            (None, false) => env::set_var("PATH", path.as_os_str()),
            (Some(path_oss), true) => {
                let path_list = env::split_paths(&path_oss).collect::<Vec<_>>();
                let new_paths = path_list
                    .iter()
                    .filter(|p| !path_eq_ignore_case(p.as_path(), path));
                env::set_var("PATH", env::join_paths(new_paths)?);
            }
            // Nothing to remove
            (None, true) => (),
        }
        Ok(())
    }

    /// Add a path permanently to user's `PATH` environment variable,
    /// also work for current running process.
    pub(crate) fn add_to_path(path: &Path) -> Result<()> {
        // Note: Windows's PATH variables are splitted into User's and System's,
        // we cannot use `env::set_var` with this `user_path_new` because it would
        // write it with user PATH value and erase all the system PATH values in it.
        set_path_for_current_process(path, false)?;

        if GlobalOpts::get().no_modify_path() {
            return Ok(());
        }

        let Some(user_path_orig) = get_user_path_var()? else {
            return Ok(());
        };
        let path_bytes = path.as_os_str().encode_wide().collect::<Vec<_>>();

        if find_path_in_env(&user_path_orig, &path_bytes).is_some() {
            // The path was already added, return without doing anything.
            return Ok(());
        }

        let mut user_path_new = path_bytes;
        user_path_new.push(b';' as u16);
        user_path_new.extend_from_slice(&user_path_orig);

        // Apply the new path
        set_persist_env_var("PATH", user_path_new)?;
        // Sync changes
        update_env();

        Ok(())
    }

    /// Remove a path permanently from user's `PATH` environment variable,
    /// also work for current running process.
    pub(crate) fn remove_from_path(path: &Path) -> Result<()> {
        // Note: Windows's PATH variables are splitted into User's and System's,
        // we cannot use `env::set_var` with this `user_path_new` because it would
        // write it with user PATH value and erase all the system PATH values in it.
        set_path_for_current_process(path, true)?;

        if GlobalOpts::get().no_modify_path() {
            return Ok(());
        }

        let Some(user_path_orig) = get_user_path_var()? else {
            return Ok(());
        };
        let path_bytes = path.as_os_str().encode_wide().collect::<Vec<_>>();

        let Some((start, end)) = find_path_in_env(&user_path_orig, &path_bytes) else {
            // The path is not added, return without doing anything.
            return Ok(());
        };

        // Remove complete PATH entry with delimiter handling:
        // - first entry: remove following ';' if present
        // - middle/last entry: remove preceding ';'
        let (remove_start, remove_end) = if start == 0 {
            let end = if user_path_orig.get(end) == Some(&(b';' as u16)) {
                end + 1
            } else {
                end
            };
            (0, end)
        } else {
            (start - 1, end)
        };

        let mut user_path_new = user_path_orig[..remove_start].to_owned();
        user_path_new.extend_from_slice(&user_path_orig[remove_end..]);

        // Apply the new path
        set_persist_env_var("PATH", user_path_new)?;
        // Sync changes
        update_env();

        Ok(())
    }

    #[cfg(test)]
    mod rustup_tests {
        use super::*;

        #[test]
        fn find_path_in_env_is_boundary_aware() {
            let paths = "C:\\bin2;D:\\tools"
                .encode_utf16()
                .collect::<Vec<_>>();
            let target = "C:\\bin".encode_utf16().collect::<Vec<_>>();

            assert!(find_path_in_env(&paths, &target).is_none());
        }

        #[test]
        fn find_path_in_env_is_case_insensitive_for_ascii() {
            let paths = "C:\\RUST\\bin;D:\\tools"
                .encode_utf16()
                .collect::<Vec<_>>();
            let target = "c:\\rust\\BIN".encode_utf16().collect::<Vec<_>>();

            assert_eq!(find_path_in_env(&paths, &target), Some((0, 11)));
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::rustup;

    #[test]
    fn update_path() {
        let dummy_path = PathBuf::from("/path/to/non_exist/bin");
        let cur_paths = std::env::var_os("PATH").unwrap_or_default();

        // ADD
        rustup::set_path_for_current_process(&dummy_path, false).unwrap();
        let new_paths = std::env::var_os("PATH").unwrap();
        let mut expected = dummy_path.as_os_str().to_os_string();
        expected.push(";");
        expected.push(cur_paths.clone());
        assert_eq!(new_paths, expected);

        // REMOVE
        rustup::set_path_for_current_process(&dummy_path, true).unwrap();
        let new_paths = std::env::var_os("PATH").unwrap();
        assert_eq!(new_paths, cur_paths);
    }
}
