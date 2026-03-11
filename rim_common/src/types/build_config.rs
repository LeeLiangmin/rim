use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use serde::Deserialize;
use std::{collections::HashMap, sync::OnceLock};
use url::Url;

use crate::types::CargoRegistry;

type LocaleMap = HashMap<String, String>;

static BUILD_CFG_SINGLETON: OnceLock<BuildConfig> = OnceLock::new();

/// Hardcoded XOR key — must match the key used in `rim_common/build.rs`.
const XOR_KEY: &[u8] = b"rim-obs-sign-key";

macro_rules! getter {
    ($name:ident: &$ret:ty) => {
        pub fn $name(&self) -> &$ret {
            &self.config.$name
        }
    };
}

#[derive(Debug, Clone, Deserialize)]
pub struct BuildConfig {
    pub identifier: String,
    pub home_page_url: Url,
    #[serde(flatten)]
    config: SourceConfig,
    pub registry: CargoRegistry,
    pub locale: HashMap<String, LocaleMap>,
    // OBS signing configuration
    #[serde(default)]
    obs_credentials: HashMap<String, ObsCredentialEncrypted>,
    #[serde(default)]
    obs_sign_rules: Vec<ObsSignRule>,
}

#[derive(Debug, Clone, Deserialize)]
struct SourceConfig {
    rustup_dist_server: Url,
    rustup_update_root: Url,
    rim_dist_server: Url,
}

/// Encrypted credential stored in configuration.toml (populated by build.rs).
#[derive(Debug, Clone, Deserialize)]
struct ObsCredentialEncrypted {
    encrypted_ak: String,
    encrypted_sk: String,
}

/// A signing rule: URL domain pattern → credential reference name.
#[derive(Debug, Clone, Deserialize)]
pub struct ObsSignRule {
    pub url_pattern: String,
    pub credential: String,
}

/// Decrypted credential for runtime use.
pub struct ObsCredential {
    pub access_key: String,
    pub secret_key: String,
}

impl BuildConfig {
    pub fn load() -> &'static Self {
        BUILD_CFG_SINGLETON.get_or_init(|| {
            // Read from OUT_DIR — the build.rs-processed configuration with encrypted credentials
            let raw = include_str!(concat!(env!("OUT_DIR"), "/configuration.toml"));
            toml::from_str(raw).expect("unable to load build configuration")
        })
    }

    /// The application name, which should be the name of this binary after installation.
    pub fn app_name(&self) -> String {
        format!("{}-manager", self.identifier)
    }

    getter!(rustup_dist_server: &Url);
    getter!(rustup_update_root: &Url);
    getter!(rim_dist_server: &Url);

    /// Find a matching OBS signing rule for the given URL and return the decrypted credential.
    ///
    /// Returns `None` if:
    /// - No rule matches the URL's host
    /// - The matched rule references a credential that doesn't exist
    /// - The credential has empty encrypted_ak/sk (env vars were not set at build time)
    pub fn find_obs_credential(&self, url: &Url) -> Option<ObsCredential> {
        let host = url.host_str()?;

        // Find the first matching rule
        let rule = self
            .obs_sign_rules
            .iter()
            .find(|r| glob_match_host(host, &r.url_pattern))?;

        // Look up the credential by reference name
        let enc = self.obs_credentials.get(&rule.credential)?;

        // Empty credential means env vars were not provided at build time
        if enc.encrypted_ak.is_empty() || enc.encrypted_sk.is_empty() {
            return None;
        }

        Some(ObsCredential {
            access_key: xor_decrypt(&enc.encrypted_ak),
            secret_key: xor_decrypt(&enc.encrypted_sk),
        })
    }
}

/// Simple glob-style host matching. Supports `*` as a wildcard that matches
/// any sequence of characters (but not the dot `.` separator — however for
/// simplicity we match any chars including dots, which is fine for our use case
/// of patterns like `rust-mirror.obs.*.myhuaweicloud.com`).
fn glob_match_host(host: &str, pattern: &str) -> bool {
    let parts: Vec<&str> = pattern.split('*').collect();

    if parts.len() == 1 {
        // No wildcard — exact match
        return host == pattern;
    }

    let mut remaining = host;

    for (i, part) in parts.iter().enumerate() {
        if part.is_empty() {
            continue;
        }
        if i == 0 {
            // First segment must be a prefix
            if !remaining.starts_with(part) {
                return false;
            }
            remaining = &remaining[part.len()..];
        } else if i == parts.len() - 1 {
            // Last segment must be a suffix
            if !remaining.ends_with(part) {
                return false;
            }
            return true;
        } else {
            // Middle segment — find it anywhere in the remainder
            match remaining.find(part) {
                Some(pos) => remaining = &remaining[pos + part.len()..],
                None => return false,
            }
        }
    }

    true
}

/// XOR + Base64 decryption (reverse of build.rs encryption).
fn xor_decrypt(encrypted_b64: &str) -> String {
    let encrypted = BASE64.decode(encrypted_b64).unwrap_or_default();
    let decrypted: Vec<u8> = encrypted
        .iter()
        .enumerate()
        .map(|(i, b)| b ^ XOR_KEY[i % XOR_KEY.len()])
        .collect();
    String::from_utf8(decrypted).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_glob_match_host() {
        // Exact match
        assert!(glob_match_host("example.com", "example.com"));
        assert!(!glob_match_host("other.com", "example.com"));

        // Wildcard patterns
        assert!(glob_match_host(
            "rust-mirror.obs.cn-north-4.myhuaweicloud.com",
            "rust-mirror.obs.*.myhuaweicloud.com"
        ));
        assert!(glob_match_host(
            "rust-dist.obs.cn-north-4.myhuaweicloud.com",
            "rust-dist.obs.*.myhuaweicloud.com"
        ));
        assert!(glob_match_host(
            "rust-mirror.obs.cn-south-1.myhuaweicloud.com",
            "rust-mirror.obs.*.myhuaweicloud.com"
        ));

        // Should not match different bucket names
        assert!(!glob_match_host(
            "other-bucket.obs.cn-north-4.myhuaweicloud.com",
            "rust-mirror.obs.*.myhuaweicloud.com"
        ));

        // Non-OBS domains should not match
        assert!(!glob_match_host(
            "code.visualstudio.com",
            "rust-mirror.obs.*.myhuaweicloud.com"
        ));
        assert!(!glob_match_host(
            "mirror.xuanwu.openatom.cn",
            "rust-mirror.obs.*.myhuaweicloud.com"
        ));
    }

    #[test]
    fn test_xor_roundtrip() {
        let original = "test-access-key-12345";
        // Simulate encryption (same logic as build.rs)
        let encrypted: Vec<u8> = original
            .bytes()
            .enumerate()
            .map(|(i, b)| b ^ XOR_KEY[i % XOR_KEY.len()])
            .collect();
        let encrypted_b64 = BASE64.encode(&encrypted);

        // Decrypt
        let decrypted = xor_decrypt(&encrypted_b64);
        assert_eq!(decrypted, original);
    }
}
