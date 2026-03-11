use std::{env, fs, path::Path};

use base64::{engine::general_purpose::STANDARD as BASE64, Engine};

/// Hardcoded XOR key for simple obfuscation (not cryptographically secure,
/// just raises the bar for casual reverse engineering).
const XOR_KEY: &[u8] = b"rim-obs-sign-key";

fn main() {
    println!("cargo:rerun-if-changed=../configuration.toml");

    process_obs_credentials();
}

fn process_obs_credentials() {
    let config_path = Path::new("../configuration.toml");
    let config_content = fs::read_to_string(config_path).expect("cannot read configuration.toml");

    let mut doc: toml_edit::DocumentMut = config_content
        .parse()
        .expect("cannot parse configuration.toml");

    // Process obs_credentials: read AK/SK from env vars and encrypt
    if let Some(creds) = doc.get_mut("obs_credentials").and_then(|v| v.as_table_mut()) {
        for (name, cred) in creds.iter_mut() {
            let env_name_upper = name.to_uppercase();
            let ak_env = format!("OBS_AK_{env_name_upper}");
            let sk_env = format!("OBS_SK_{env_name_upper}");

            println!("cargo:rerun-if-env-changed={ak_env}");
            println!("cargo:rerun-if-env-changed={sk_env}");

            if let Some(table) = cred.as_table_mut() {
                if let (Ok(ak), Ok(sk)) = (env::var(&ak_env), env::var(&sk_env)) {
                    table.insert("encrypted_ak", toml_edit::value(xor_encrypt(&ak)));
                    table.insert("encrypted_sk", toml_edit::value(xor_encrypt(&sk)));
                }
                // If env vars are not set, leave encrypted_ak/sk as empty strings
                // (the rule will be skipped at runtime)
            }
        }
    }

    // Write processed configuration to OUT_DIR
    let out_dir = env::var("OUT_DIR").unwrap();
    let dest = Path::new(&out_dir).join("configuration.toml");
    fs::write(&dest, doc.to_string()).expect("cannot write processed configuration.toml");
}

/// Simple XOR + Base64 obfuscation.
fn xor_encrypt(plaintext: &str) -> String {
    let encrypted: Vec<u8> = plaintext
        .bytes()
        .enumerate()
        .map(|(i, b)| b ^ XOR_KEY[i % XOR_KEY.len()])
        .collect();
    BASE64.encode(&encrypted)
}
