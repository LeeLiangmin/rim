use rim_common::exe;
use std::path::PathBuf;

/// Check that the platform's webview runtime is available before launching Tauri.
///
/// On Windows the check is reliable (registry + well-known paths), so a
/// missing runtime triggers a user-friendly dialog and `exit(1)`.
///
/// On Linux the heuristic (pkg-config + so probing) can produce false
/// negatives (e.g. unusual lib prefix, missing pkg-config binary), so we
/// only emit a warning and let Tauri attempt to start — it will still crash
/// with its own error if the runtime is truly absent.
pub(crate) fn ensure_platform_webview_dependency() {
    match platform_webview_is_ready() {
        WebviewProbeResult::Ready => return,
        WebviewProbeResult::Missing => {
            let dependency = required_dependency_name();
            let cli = fallback_cli_display_name();
            let title = t!("missing_webview_runtime", dependency = dependency).to_string();
            let hint = t!("missing_webview_runtime_hint", cli = cli).to_string();
            let message = format!("{title}\n\n{hint}");

            show_user_message(&title, &message);
            std::process::exit(1);
        }
        WebviewProbeResult::Uncertain => {
            warn!(
                "Could not confirm webview runtime presence — \
                 proceeding anyway (detection may be incomplete on this platform)"
            );
        }
    }
}

#[allow(dead_code)]
enum WebviewProbeResult {
    Ready,
    Missing,
    /// Detection was inconclusive — the runtime *might* be present but we
    /// could not verify it with the available probes.
    Uncertain,
}

/// Build the display name for the CLI installer that users can run as a
/// fallback when the webview runtime is missing.
///
/// The name is derived from the current executable location at runtime:
/// look for a sibling binary named `installer-cli` (with platform suffix).
/// If it doesn't exist, just show the bare name so the hint is still useful.
fn fallback_cli_display_name() -> String {
    let exe = std::env::current_exe().unwrap_or_default();
    let dir = exe.parent().unwrap_or_else(|| std::path::Path::new("."));
    let cli_path = dir.join(exe!("installer-cli"));
    if cli_path.exists() {
        cli_path.display().to_string()
    } else {
        exe!("installer-cli")
    }
}

// ---------------------------------------------------------------------------
// Platform-specific: is the webview runtime ready?
// ---------------------------------------------------------------------------

#[cfg(target_os = "windows")]
fn platform_webview_is_ready() -> WebviewProbeResult {
    if webview2_from_registry() || webview2_from_well_known_paths() {
        WebviewProbeResult::Ready
    } else {
        WebviewProbeResult::Missing
    }
}

/// On Linux, Tauri 1.x ships with `wry` which links against `webkit2gtk-4.0`
/// (the `webkit2gtk` Rust crate 0.18.x). We probe both the 4.0 and 4.1 series
/// so that newer distros shipping only 4.1 are also accepted — Tauri 2.x
/// (`wry` 0.37+) will switch to 4.1 exclusively.
///
/// If the upstream dependency changes, update the lists below accordingly.
///
/// Because the probing is best-effort (pkg-config may be absent, so paths
/// may not cover all distros), a negative result is reported as `Uncertain`
/// rather than `Missing` to avoid blocking a potentially functional system.
#[cfg(target_os = "linux")]
fn platform_webview_is_ready() -> WebviewProbeResult {
    const PKG_CONFIG_NAMES: &[&str] = &["webkit2gtk-4.1", "webkit2gtk-4.0"];
    const SO_NAMES: &[&str] = &["libwebkit2gtk-4.1.so.0", "libwebkit2gtk-4.0.so.37"];

    if PKG_CONFIG_NAMES.iter().any(|name| has_pkg(name))
        || SO_NAMES.iter().any(|name| has_shared_object(name))
    {
        WebviewProbeResult::Ready
    } else {
        WebviewProbeResult::Uncertain
    }
}

#[cfg(target_os = "macos")]
fn platform_webview_is_ready() -> WebviewProbeResult {
    WebviewProbeResult::Ready
}

#[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
fn platform_webview_is_ready() -> WebviewProbeResult {
    WebviewProbeResult::Ready
}

// ---------------------------------------------------------------------------
// Platform-specific: human-readable dependency name
// ---------------------------------------------------------------------------

#[cfg(target_os = "windows")]
fn required_dependency_name() -> &'static str {
    "Microsoft Edge WebView2 Runtime"
}

#[cfg(target_os = "linux")]
fn required_dependency_name() -> &'static str {
    "WebKitGTK (webkit2gtk)"
}

#[cfg(target_os = "macos")]
fn required_dependency_name() -> &'static str {
    "WebKit (system)"
}

#[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
fn required_dependency_name() -> &'static str {
    "webview runtime"
}

// ---------------------------------------------------------------------------
// Show a user-friendly message (dialog on Windows, stderr elsewhere)
// ---------------------------------------------------------------------------

#[cfg(target_os = "windows")]
fn show_user_message(title: &str, body: &str) {
    eprintln!("{body}");

    use std::ffi::OsStr;
    use std::iter::once;
    use std::os::windows::ffi::OsStrExt;
    fn to_wide(s: &str) -> Vec<u16> {
        OsStr::new(s).encode_wide().chain(once(0)).collect()
    }
    let wide_body = to_wide(body);
    let wide_title = to_wide(title);
    unsafe {
        winapi::um::winuser::MessageBoxW(
            std::ptr::null_mut(),
            wide_body.as_ptr(),
            wide_title.as_ptr(),
            winapi::um::winuser::MB_OK | winapi::um::winuser::MB_ICONWARNING,
        );
    }
}

#[cfg(not(target_os = "windows"))]
fn show_user_message(_title: &str, body: &str) {
    eprintln!("{body}");
}

// ---------------------------------------------------------------------------
// Windows: validate WebView2 via registry using winreg crate (no subprocess)
// ---------------------------------------------------------------------------

/// Stable GUID for the WebView2 Runtime registered under EdgeUpdate\Clients.
/// Reference: <https://learn.microsoft.com/en-us/microsoft-edge/webview2/concepts/distribution#detect-if-a-suitable-webview2-runtime-is-already-installed>
#[cfg(target_os = "windows")]
const WEBVIEW2_CLIENT_GUID: &str = "{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}";

#[cfg(target_os = "windows")]
fn webview2_from_registry() -> bool {
    use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ};
    use winreg::RegKey;

    let native = format!(r"SOFTWARE\Microsoft\EdgeUpdate\Clients\{WEBVIEW2_CLIENT_GUID}");
    let wow64 = format!(r"SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\{WEBVIEW2_CLIENT_GUID}");
    let candidates: &[(winreg::HKEY, &str)] = &[
        (HKEY_LOCAL_MACHINE, &native),
        (HKEY_LOCAL_MACHINE, &wow64),
        (HKEY_CURRENT_USER, &native),
    ];

    for &(root, subkey) in candidates {
        let Ok(key) = RegKey::predef(root).open_subkey_with_flags(subkey, KEY_READ) else {
            continue;
        };

        // We intentionally require the executable to exist on disk rather
        // than trusting registry metadata alone (`pv`, etc.), because
        // registry entries can survive incomplete uninstalls while the
        // actual runtime files are gone — leading to 0x80070002 at launch.
        if let Ok(location) = key.get_value::<String, _>("location") {
            let dir = PathBuf::from(&location);
            if dir.is_dir() && dir_contains_exe(&dir, "msedgewebview2") {
                return true;
            }
        }
    }
    false
}

/// Check if a directory (or its immediate version-numbered subdirectories)
/// contains the given executable.
#[cfg(target_os = "windows")]
fn dir_contains_exe(dir: &std::path::Path, exe_stem: &str) -> bool {
    let target = format!("{exe_stem}.exe");
    if dir.join(&target).exists() {
        return true;
    }
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let child = entry.path();
            if child.is_dir() && child.join(&target).exists() {
                return true;
            }
        }
    }
    false
}

/// Fallback: check well-known installation paths obtained from the system
/// via `SHGetKnownFolderPath` instead of trusting environment variables.
#[cfg(target_os = "windows")]
fn webview2_from_well_known_paths() -> bool {
    let roots: Vec<PathBuf> = [
        known_folder_path(&winapi::um::knownfolders::FOLDERID_ProgramFiles),
        known_folder_path(&winapi::um::knownfolders::FOLDERID_ProgramFilesX86),
    ]
    .into_iter()
    .flatten()
    .collect();

    for root in &roots {
        let app_dir = root.join("Microsoft").join("EdgeWebView").join("Application");
        if dir_contains_exe(&app_dir, "msedgewebview2") {
            return true;
        }
    }
    false
}

/// Retrieve a Known Folder path via the Windows Shell API.
/// Returns `None` on any failure, avoiding reliance on environment variables.
#[cfg(target_os = "windows")]
fn known_folder_path(folder_id: &winapi::shared::guiddef::GUID) -> Option<PathBuf> {
    use std::ffi::OsString;
    use std::os::windows::ffi::OsStringExt;

    unsafe {
        let mut path_ptr: winapi::shared::ntdef::PWSTR = std::ptr::null_mut();
        let hr = winapi::um::shlobj::SHGetKnownFolderPath(
            folder_id,
            0,
            std::ptr::null_mut(),
            &mut path_ptr,
        );
        if hr != winapi::shared::winerror::S_OK || path_ptr.is_null() {
            if !path_ptr.is_null() {
                winapi::um::combaseapi::CoTaskMemFree(path_ptr as *mut _);
            }
            return None;
        }
        let len = (0..).take_while(|&i| *path_ptr.add(i) != 0).count();
        let slice = std::slice::from_raw_parts(path_ptr, len);
        let os = OsString::from_wide(slice);
        winapi::um::combaseapi::CoTaskMemFree(path_ptr as *mut _);
        Some(PathBuf::from(os))
    }
}

// ---------------------------------------------------------------------------
// Linux helpers
// ---------------------------------------------------------------------------

/// Well-known absolute paths for `pkg-config`.
/// Checked first to avoid PATH-based executable hijacking.
#[cfg(target_os = "linux")]
const PKG_CONFIG_PATHS: &[&str] = &["/usr/bin/pkg-config", "/usr/local/bin/pkg-config"];

#[cfg(target_os = "linux")]
fn resolve_pkg_config() -> Option<std::ffi::OsString> {
    for path in PKG_CONFIG_PATHS {
        if std::path::Path::new(path).is_file() {
            return Some(std::ffi::OsString::from(path));
        }
    }
    None
}

#[cfg(target_os = "linux")]
fn has_pkg(name: &str) -> bool {
    resolve_pkg_config()
        .and_then(|bin| {
            std::process::Command::new(bin)
                .args(["--exists", name])
                .status()
                .ok()
        })
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Well-known library directories to probe for shared objects.
#[cfg(target_os = "linux")]
const LIB_SEARCH_DIRS: &[&str] = &[
    "/usr/lib",
    "/usr/lib64",
    "/usr/local/lib",
    "/lib",
    "/lib64",
    "/usr/lib/x86_64-linux-gnu",
    "/usr/lib/aarch64-linux-gnu",
];

#[cfg(target_os = "linux")]
fn has_shared_object(soname: &str) -> bool {
    LIB_SEARCH_DIRS
        .iter()
        .map(std::path::Path::new)
        .any(|root| root.join(soname).exists())
}
