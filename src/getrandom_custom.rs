//! Custom getrandom 0.3 RNG registration for windows-gnu cross-compilation.
//!
//! getrandom 0.3.x uses raw-dylib linking to bcryptprimitives on Windows,
//! which fails during cross-compilation from Linux because the DLL import
//! library cannot be generated. By enabling the `custom` feature on getrandom
//! 0.3, all OS backends are disabled, and we register our own RNG here using
//! the RDRAND instruction available on modern x86_64 CPUs.

/// Register a custom RNG implementation for getrandom 0.3 at startup.
/// Only active on windows-gnu where the `getrandom03` dependency exists.
pub fn init() {
    #[cfg(all(windows, target_env = "gnu"))]
    {
        extern crate getrandom03 as getrandom;

        fn rdrand_fill(buf: &mut [u8]) -> Result<(), getrandom::Error> {
            let mut remaining = buf;
            while remaining.len() >= 8 {
                let mut val: u64 = 0;
                #[cfg(target_arch = "x86_64")]
                unsafe {
                    while std::arch::x86_64::_rdrand64_step(&mut val) == 0 {}
                }
                #[cfg(not(target_arch = "x86_64"))]
                {
                    use std::time::{SystemTime, UNIX_EPOCH};
                    let seed = SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_nanos() as u64;
                    let mut state = seed.wrapping_add(0x9E3779B97F4A7C15);
                    state = (state ^ (state >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
                    state = (state ^ (state >> 27)).wrapping_mul(0x94D049BB133111EB);
                    val = state ^ (state >> 31);
                }
                remaining[..8].copy_from_slice(&val.to_ne_bytes());
                remaining = &mut remaining[8..];
            }
            if !remaining.is_empty() {
                let mut val: u64 = 0;
                #[cfg(target_arch = "x86_64")]
                unsafe {
                    while std::arch::x86_64::_rdrand64_step(&mut val) == 0 {}
                }
                #[cfg(not(target_arch = "x86_64"))]
                {
                    use std::time::{SystemTime, UNIX_EPOCH};
                    let seed = SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_nanos() as u64;
                    let mut state = seed.wrapping_add(0x9E3779B97F4A7C15);
                    state = (state ^ (state >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
                    state = (state ^ (state >> 27)).wrapping_mul(0x94D049BB133111EB);
                    val = state ^ (state >> 31);
                }
                let len = remaining.len();
                remaining.copy_from_slice(&val.to_ne_bytes()[..len]);
            }
            Ok(())
        }

        getrandom::register_custom_getrandom!(rdrand_fill);
    }
}
