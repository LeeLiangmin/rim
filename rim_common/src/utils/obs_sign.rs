//! OBS V2 签名实现
//!
//! 参考文档:
//! - 用户签名验证: <https://support.huaweicloud.com/api-obs/obs_04_0009.html>
//! - Header中携带签名: <https://support.huaweicloud.com/api-obs/obs_04_0010.html>
//!
//! 对应 Python 原型: `huawei-s3/python/obs_sign.py`

use anyhow::{bail, Result};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use hmac::{Hmac, Mac};
use sha1::Sha1;
use url::Url;

type HmacSha1 = Hmac<Sha1>;

/// Build the complete set of HTTP headers required for OBS V2 authentication.
///
/// Returns a list of (header_name, header_value) pairs to be added to the request.
pub fn obs_auth_headers(
    access_key: &str,
    secret_key: &str,
    method: &str,
    url: &Url,
) -> Result<Vec<(String, String)>> {
    let (bucket, object_key) = extract_bucket_and_key(url)?;
    let date = rfc1123_date();

    // StringToSign for a simple GET (no Content-MD5, no Content-Type, no special headers)
    let string_to_sign = format!(
        "{method}\n\
         \n\
         \n\
         {date}\n\
         /{bucket}/{object_key}"
    );

    let signature = obs_v2_signature(secret_key, &string_to_sign);
    let authorization = format!("OBS {access_key}:{signature}");

    let mut headers = vec![
        ("Date".to_string(), date),
        ("Authorization".to_string(), authorization),
    ];

    // Include Host header with the original host
    if let Some(host) = url.host_str() {
        headers.push(("Host".to_string(), host.to_string()));
    }

    Ok(headers)
}

/// Compute the OBS V2 signature: `Base64(HMAC-SHA1(SecretKey, StringToSign))`
fn obs_v2_signature(secret_key: &str, string_to_sign: &str) -> String {
    let mut mac =
        HmacSha1::new_from_slice(secret_key.as_bytes()).expect("HMAC can take key of any size");
    mac.update(string_to_sign.as_bytes());
    let result = mac.finalize();
    BASE64.encode(result.into_bytes())
}

/// Extract bucket name and object key from an OBS URL.
///
/// Supports two URL formats:
/// - Virtual-hosted style: `https://{bucket}.obs.{region}.myhuaweicloud.com/{key}`
/// - Path style: `https://obs.{region}.myhuaweicloud.com/{bucket}/{key}`
fn extract_bucket_and_key(url: &Url) -> Result<(String, String)> {
    let host = url
        .host_str()
        .ok_or_else(|| anyhow::anyhow!("URL has no host: {url}"))?;
    let path = url.path().trim_start_matches('/');

    // Virtual-hosted style: bucket.obs.cn-north-4.myhuaweicloud.com
    if host.contains(".obs.") && host.ends_with(".myhuaweicloud.com") {
        let bucket = host
            .split(".obs.")
            .next()
            .ok_or_else(|| anyhow::anyhow!("cannot extract bucket from host: {host}"))?;
        return Ok((bucket.to_string(), path.to_string()));
    }

    // Path style: obs.cn-north-4.myhuaweicloud.com/bucket/key
    if host.starts_with("obs.") && host.ends_with(".myhuaweicloud.com") {
        let mut parts = path.splitn(2, '/');
        let bucket = parts
            .next()
            .ok_or_else(|| anyhow::anyhow!("cannot extract bucket from path: {path}"))?;
        let key = parts.next().unwrap_or("");
        return Ok((bucket.to_string(), key.to_string()));
    }

    bail!("cannot extract bucket and key from URL: {url}")
}

/// Generate an RFC 1123 formatted date string (e.g., "Wed, 11 Mar 2026 06:00:00 GMT").
fn rfc1123_date() -> String {
    chrono::Utc::now()
        .format("%a, %d %b %Y %H:%M:%S GMT")
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_bucket_and_key_virtual_hosted() {
        let url =
            Url::parse("https://rust-mirror.obs.cn-north-4.myhuaweicloud.com/manager/release.toml")
                .unwrap();
        let (bucket, key) = extract_bucket_and_key(&url).unwrap();
        assert_eq!(bucket, "rust-mirror");
        assert_eq!(key, "manager/release.toml");
    }

    #[test]
    fn test_extract_bucket_and_key_path_style() {
        let url = Url::parse(
            "https://obs.cn-north-4.myhuaweicloud.com/rust-mirror/manager/release.toml",
        )
        .unwrap();
        let (bucket, key) = extract_bucket_and_key(&url).unwrap();
        assert_eq!(bucket, "rust-mirror");
        assert_eq!(key, "manager/release.toml");
    }

    #[test]
    fn test_extract_bucket_and_key_non_obs() {
        let url = Url::parse("https://code.visualstudio.com/download").unwrap();
        assert!(extract_bucket_and_key(&url).is_err());
    }

    #[test]
    fn test_obs_v2_signature_deterministic() {
        // Use a fixed StringToSign and verify the signature is consistent
        let string_to_sign = "GET\n\n\nWed, 11 Mar 2026 06:00:00 GMT\n/rust-mirror/manager/release.toml";
        let sig = obs_v2_signature("test-secret-key", string_to_sign);
        // Verify it's a valid base64 string
        assert!(BASE64.decode(&sig).is_ok());
        // Verify determinism
        let sig2 = obs_v2_signature("test-secret-key", string_to_sign);
        assert_eq!(sig, sig2);
    }

    #[test]
    fn test_obs_auth_headers_structure() {
        let url =
            Url::parse("https://rust-mirror.obs.cn-north-4.myhuaweicloud.com/manager/release.toml")
                .unwrap();
        let headers = obs_auth_headers("test-ak", "test-sk", "GET", &url).unwrap();

        // Should contain Date, Authorization, Host
        let header_names: Vec<&str> = headers.iter().map(|(k, _)| k.as_str()).collect();
        assert!(header_names.contains(&"Date"));
        assert!(header_names.contains(&"Authorization"));
        assert!(header_names.contains(&"Host"));

        // Authorization should have the correct format
        let auth = headers
            .iter()
            .find(|(k, _)| k == "Authorization")
            .map(|(_, v)| v)
            .unwrap();
        assert!(auth.starts_with("OBS test-ak:"));
    }
}
