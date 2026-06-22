#!/bin/bash
set -euo pipefail

# ── OBS V2 签名辅助函数 ──────────────────────────────────────────────
# 为 curl 请求生成 OBS V2 (HMAC-SHA1) 签名头。
# 用法:
#   obs_curl_download <url> [curl-extra-args...]
#   obs_curl_to_file  <url> <output-file>
#   obs_curl_pipe     <url>                  (输出到 stdout，可管道到 tar)
#
# 需要环境变量 OBS_AK_XUANWU_RUST / OBS_SK_XUANWU_RUST。
# 如果凭证不存在，回退到匿名下载（兼容公共读对象）。
# ──────────────────────────────────────────────────────────────────────

# 从 OBS URL 中提取 bucket 和 object key
# 支持虚拟主机风格: https://{bucket}.obs.{region}.myhuaweicloud.com/{key}
_obs_extract_bucket_key() {
    local url="$1"
    # 去掉 scheme
    local hostpath="${url#https://}"
    hostpath="${hostpath#http://}"
    local host="${hostpath%%/*}"
    local path="${hostpath#*/}"
    # 虚拟主机风格: bucket.obs.cn-north-4.myhuaweicloud.com
    if [[ "$host" == *.obs.*.myhuaweicloud.com ]]; then
        OBS_BUCKET="${host%%.*}"
        OBS_OBJECT_KEY="$path"
        return 0
    fi
    return 1
}

# 生成 OBS V2 签名头并执行 curl
# $1 = URL, 其余参数传给 curl
_obs_signed_curl() {
    local url="$1"; shift
    local ak="${OBS_AK_XUANWU_RUST:-}"
    local sk="${OBS_SK_XUANWU_RUST:-}"

    # 无凭证时回退到匿名下载
    if [[ -z "$ak" ]] || [[ -z "$sk" ]]; then
        curl "$url" "$@"
        return $?
    fi

    # 提取 bucket 和 key
    local OBS_BUCKET="" OBS_OBJECT_KEY=""
    if ! _obs_extract_bucket_key "$url"; then
        # 非 OBS URL，直接匿名下载
        curl "$url" "$@"
        return $?
    fi

    local method="GET"
    local date
    date="$(LC_ALL=C date -u '+%a, %d %b %Y %H:%M:%S GMT')"
    local string_to_sign
    string_to_sign="$(printf '%s\n\n\n%s\n/%s/%s' "$method" "$date" "$OBS_BUCKET" "$OBS_OBJECT_KEY")"
    local signature
    signature="$(printf '%s' "$string_to_sign" | openssl dgst -sha1 -hmac "$sk" -binary | openssl base64 -A)"
    local host="${OBS_BUCKET}.obs.cn-north-4.myhuaweicloud.com"

    curl -H "Date: $date" \
         -H "Authorization: OBS ${ak}:${signature}" \
         -H "Host: $host" \
         "$url" "$@"
}

# 带签名下载到文件
obs_curl_to_file() {
    local url="$1" output="$2"; shift 2
    _obs_signed_curl "$url" -sSL --connect-timeout 10 -o "$output" -w "%{http_code}" "$@"
}

# 带签名下载到 stdout（管道用）
obs_curl_pipe() {
    local url="$1"; shift
    _obs_signed_curl "$url" -sSL --connect-timeout 10 --max-time 300 "$@"
}

usage() {
    echo "Usage: $0 --build-target <triple> --dist-targets <triple,...> [--skip-gui] [--binary-only]"
    echo "  --build-target   Target triple for compiling the installer binary"
    echo "  --dist-targets  Comma-separated target triples for distribution"
    echo "  --skip-gui       Skip Docker-based GUI build"
    echo "  --binary-only    Build CLI binary only, skip offline package"
    exit 1
}

BUILD_TARGET=""
DIST_TARGETS=""
SKIP_GUI=false
BINARY_ONLY=false

while [[ $# -gt 0 ]]; do
    case "$1" in
        --build-target) BUILD_TARGET="$2"; shift 2 ;;
        --dist-targets) DIST_TARGETS="$2"; shift 2 ;;
        --skip-gui) SKIP_GUI=true; shift ;;
        --binary-only) BINARY_ONLY=true; shift ;;
        -h|--help) usage ;;
        *) echo "Unknown option: $1"; usage ;;
    esac
done

if [[ -z "$BUILD_TARGET" ]] || [[ -z "$DIST_TARGETS" ]]; then
    echo "ERROR: --build-target and --dist-targets are required"
    usage
fi

# Load shared config
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ENV_FILE="$SCRIPT_DIR/../.env"
if [[ -f "$ENV_FILE" ]]; then
    set -a; source "$ENV_FILE"; set +a
fi

echo "=== Build Config ==="
echo "  BUILD_TARGET:  $BUILD_TARGET"
echo "  DIST_TARGETS:  $DIST_TARGETS"
echo "  SKIP_GUI:      $SKIP_GUI"
echo "  BINARY_ONLY:   $BINARY_ONLY"

echo "=== Excluding rim_gui from workspace ==="
sed -i 's|"rim_gui/src-tauri", ||' Cargo.toml
echo "workspace members after patch:"
grep -A5 '\[workspace\]' Cargo.toml | head -6

export GIT_HTTP_LOW_SPEED_LIMIT="${GIT_HTTP_LOW_SPEED_LIMIT:-1000}"
export GIT_HTTP_LOW_SPEED_TIME="${GIT_HTTP_LOW_SPEED_TIME:-30}"

# Restore cargo env early (needed for rustc --print sysroot in cross-compilation setup)
if [[ -f "$HOME/.cargo/env" ]]; then
    source "$HOME/.cargo/env"
fi

# Set CC for musl cross-compilation (openssl-sys needs this)
if [[ "$BUILD_TARGET" == *"musl"* ]]; then
    if ! command -v musl-gcc >/dev/null 2>&1; then
        echo "WARNING: musl-gcc not found, falling back to gnu target"
        BUILD_TARGET="${BUILD_TARGET//musl/gnu}"
        echo "  Using BUILD_TARGET=$BUILD_TARGET"
    else
        # openssl-sys hardcodes CC=x86_64-linux-musl-gcc for this target
        # musl-tools package only provides musl-gcc, so create symlink
        if ! command -v x86_64-linux-musl-gcc >/dev/null 2>&1; then
            mkdir -p /usr/local/bin
            ln -sf "$(which musl-gcc)" /usr/local/bin/x86_64-linux-musl-gcc
            echo "  Created symlink: /usr/local/bin/x86_64-linux-musl-gcc -> $(which musl-gcc)"
        fi
        export CC=musl-gcc
        export CC_x86_64_unknown_linux_musl=musl-gcc
        export CARGO_TARGET_X86_64_UNKNOWN_LINUX_MUSL_LINKER=musl-gcc
    fi
fi

# Set CC/linker for aarch64 cross-compilation
if [[ "$BUILD_TARGET" == "aarch64-unknown-linux-gnu" ]]; then
    # Add toolchain path if previously installed
    if [[ -d "$HOME/aarch64-toolchain/bin" ]]; then
        export PATH="$HOME/aarch64-toolchain/bin:$PATH"
    fi

    if ! command -v aarch64-linux-gnu-gcc >/dev/null 2>&1; then
        # 检查当前机器架构
        MACHINE_ARCH=$(uname -m)
        if [[ "$MACHINE_ARCH" == "aarch64" || "$MACHINE_ARCH" == "arm64" ]]; then
            echo "  Native aarch64 machine, no cross-compiler needed"
        else
            # Download cross-compiler toolchain from OBS (internal mirror, fast)
            echo "  aarch64-linux-gnu-gcc not found, downloading cross-compiler from OBS..."
            TOOLCHAIN_DIR="$HOME/aarch64-toolchain"
            TOOLCHAIN_URL="${AARCH64_TOOLCHAIN_URL:-https://xuanwu-rust.obs.cn-north-4.myhuaweicloud.com/dist/arm-gnu-toolchain-13.3.rel1-x86_64-aarch64-none-linux-gnu.tar.xz}"
            mkdir -p "$TOOLCHAIN_DIR"

            if obs_curl_pipe "$TOOLCHAIN_URL" | tar -xJ --strip-components=1 -C "$TOOLCHAIN_DIR"; then
                export PATH="$TOOLCHAIN_DIR/bin:$PATH"
                # ARM toolchain uses aarch64-none-linux-gnu- prefix, create symlinks
                ln -sf "$TOOLCHAIN_DIR/bin/aarch64-none-linux-gnu-gcc" "$TOOLCHAIN_DIR/bin/aarch64-linux-gnu-gcc"
                ln -sf "$TOOLCHAIN_DIR/bin/aarch64-none-linux-gnu-ar" "$TOOLCHAIN_DIR/bin/aarch64-linux-gnu-ar"
                ln -sf "$TOOLCHAIN_DIR/bin/aarch64-none-linux-gnu-ld" "$TOOLCHAIN_DIR/bin/aarch64-linux-gnu-ld"
                echo "  Installed ARM toolchain to $TOOLCHAIN_DIR"
            else
                echo "ERROR: failed to download aarch64 cross-compiler from OBS"
                echo "  URL: $TOOLCHAIN_URL"
                exit 1
            fi
        fi
    fi

    if command -v aarch64-linux-gnu-gcc >/dev/null 2>&1; then
        echo "  Using aarch64-linux-gnu-gcc for cross-compilation"
        export CC_aarch64_unknown_linux_gnu=aarch64-linux-gnu-gcc
        export CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER=aarch64-linux-gnu-gcc
    fi
fi

echo "=== Vendoring offline packages ==="
cargo run -p rim_dev -- vendor --for "$DIST_TARGETS"

# Set CC/linker for Windows mingw cross-compilation
if [[ "$BUILD_TARGET" == *"windows-gnu"* ]]; then
    # ── LLVM mingw toolchain (provides linker and other tools) ──
    if [[ -d "$HOME/mingw-toolchain/bin" ]]; then
        export PATH="$HOME/mingw-toolchain/bin:$PATH"
    fi

    if ! command -v x86_64-w64-mingw32-gcc >/dev/null 2>&1; then
        echo "  x86_64-w64-mingw32-gcc not found, downloading mingw toolchain..."
        TOOLCHAIN_DIR="$HOME/mingw-toolchain"
        TOOLCHAIN_URL="${MINGW_TOOLCHAIN_URL:-https://xuanwu-rust.obs.cn-north-4.myhuaweicloud.com/dist/mingw-w64-cross.tar.xz}"
        TOOLCHAIN_URL2="${MINGW_TOOLCHAIN_URL2:-https://mirrors.huaweicloud.com/mingw-w64/mingw-w64-cross.tar.xz}"
        mkdir -p "$TOOLCHAIN_DIR"

        if obs_curl_pipe "$TOOLCHAIN_URL" | tar -xJ --strip-components=1 -C "$TOOLCHAIN_DIR"; then
            echo "  Installed from primary URL"
        elif obs_curl_pipe "$TOOLCHAIN_URL2" | tar -xJ --strip-components=1 -C "$TOOLCHAIN_DIR"; then
            echo "  Installed from fallback URL"
        else
            echo "ERROR: failed to download mingw cross-compiler from all sources"
            echo "  Primary: $TOOLCHAIN_URL"
            echo "  Fallback: $TOOLCHAIN_URL2"
            exit 1
        fi
        export PATH="$TOOLCHAIN_DIR/bin:$PATH"
        echo "  Installed mingw toolchain to $TOOLCHAIN_DIR"
    fi

    # ── Helper: verify a dlltool binary can actually execute ──
    # Checks both file existence and runtime compatibility (e.g. GLIBC version).
    # Returns 0 if the binary runs successfully, 1 otherwise.
    _can_run_dlltool() {
        local bin="$1"
        [ -n "$bin" ] && [ -x "$bin" ] && "$bin" --version >/dev/null 2>&1
    }

    _can_run_as() {
        local bin="$1"
        [ -n "$bin" ] && [ -x "$bin" ] || return 1
        echo '.text' | "$bin" --64 -o /dev/null 2>/dev/null
    }

    _is_llvm_as() {
        case "$("$1" --version 2>&1)" in
            *LLVM*|*clang*) return 0 ;;
            *) return 1 ;;
        esac
    }

    # ── GNU binutils (provides real GNU dlltool for raw-dylib import libs) ──
    # llvm-dlltool (bundled with the LLVM mingw toolchain above) cannot generate
    # import libraries for kernel32 and other system DLLs. The real GNU
    # dlltool can. Try in order:
    #   1. System mingw64-binutils package (if installed by euleros-install-deps.sh)
    #   2. Download pre-built tarball from OBS
    GNUBIN_DIR="$HOME/gnu-binutils"
    REAL_DLLTOOL=""
    # ── Strategy 1: system package ──
    for cand in /usr/bin/x86_64-w64-mingw32-dlltool /usr/x86_64-w64-mingw32/bin/dlltool; do
        if _can_run_dlltool "$cand"; then
            _gnu_ver="$("$cand" --version 2>&1 | head -1)"
            if echo "$_gnu_ver" | grep -qi 'GNU'; then
                REAL_DLLTOOL="$cand"
                echo "  Found system GNU dlltool: $cand ($_gnu_ver)"
                break
            fi
        fi
    done
    # ── Strategy 2: download from OBS ──
    if [ -z "$REAL_DLLTOOL" ]; then
        GNUBIN_URL="${GNUBIN_URL:-https://xuanwu-rust.obs.cn-north-4.myhuaweicloud.com/dist/mingw-binutils-x86_64-2.42.tar.gz}"
        if [[ ! -x "$GNUBIN_DIR/bin/x86_64-w64-mingw32-dlltool" ]]; then
            echo "  Downloading GNU binutils for dlltool..."
            mkdir -p "$GNUBIN_DIR"
            GNUBIN_TGZ="$HOME/gnu-binutils.tar.gz"
            _http_code=$(obs_curl_to_file "$GNUBIN_URL" "$GNUBIN_TGZ" --max-time 120 || echo "000")
            _fsize=$(stat -c%s "$GNUBIN_TGZ" 2>/dev/null || wc -c < "$GNUBIN_TGZ")
            echo "  Downloaded $_fsize bytes (HTTP $_http_code)"
            if [ "$_http_code" != "200" ]; then
                echo "  WARNING: GNU binutils download failed (HTTP $_http_code); will try llvm-dlltool as fallback"
                rm -rf "$GNUBIN_DIR"
            elif [ "$_fsize" -lt 10240 ]; then
                echo "  WARNING: downloaded file too small ($_fsize bytes), likely an error page"
                rm -rf "$GNUBIN_DIR"
            elif tar -xf "$GNUBIN_TGZ" --strip-components=1 -C "$GNUBIN_DIR" 2>&1; then
                echo "  GNU binutils installed to $GNUBIN_DIR"
            else
                echo "  WARNING: tar extraction failed; will try llvm-dlltool as fallback"
                rm -rf "$GNUBIN_DIR"
            fi
            rm -f "$GNUBIN_TGZ"
        fi
        if _can_run_dlltool "$GNUBIN_DIR/bin/x86_64-w64-mingw32-dlltool"; then
            REAL_DLLTOOL="$GNUBIN_DIR/bin/x86_64-w64-mingw32-dlltool"
            export PATH="$GNUBIN_DIR/bin:$PATH"
            echo "  GNU binutils in PATH: $GNUBIN_DIR/bin"
        elif [ -x "$GNUBIN_DIR/bin/x86_64-w64-mingw32-dlltool" ]; then
            echo "  WARNING: downloaded GNU dlltool exists but cannot run (likely GLIBC mismatch):"
            echo "    $("$GNUBIN_DIR/bin/x86_64-w64-mingw32-dlltool" --version 2>&1 | head -3 | tr '\n' ' ')"
            echo "  Will try other dlltool candidates as fallback"
        fi
    fi

    if command -v x86_64-w64-mingw32-gcc >/dev/null 2>&1; then
        echo "  Using x86_64-w64-mingw32-gcc for Windows cross-compilation"
        export CC_x86_64_pc_windows_gnu=x86_64-w64-mingw32-gcc
        export CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER=x86_64-w64-mingw32-gcc

        # rustc on windows-gnu generates raw-dylib import libs by invoking a
        # dlltool binary at link time.  GNU dlltool generates GNU-as-syntax
        # assembly and needs a mingw-capable assembler (x86_64-w64-mingw32-as)
        # via the -S flag — the system `as` only handles ELF and will fail with
        # "cannot represent relocation type BFD_RELOC_RVA".
        #
        # The OBS mingw-binutils package contains both dlltool AND as (statically
        # linked), so GNU dlltool + GNU as is the primary path.  LLVM as is
        # incompatible with GNU dlltool's assembly syntax (not just --64).
        MINGW_BIN="$(dirname "$(command -v x86_64-w64-mingw32-gcc)")"
        DLLTOOL_PATH=""

        # ── Helper: check if a dlltool is LLVM (not GNU) ──
        _is_llvm_dlltool() {
            case "$("$1" --version 2>&1)" in
                *LLVM*) return 0 ;;
                *) return 1 ;;
            esac
        }

        # ── Helper: find a working mingw assembler ──
        # Phase 1: prefer GNU as that natively supports --64 (from gnu-binutils).
        # Phase 2: fall back to ANY mingw as (e.g. LLVM as) even if it
        #          rejects --64 — the as-wrapper below will strip it.
        MINGW_AS=""
        for cand in \
            "$GNUBIN_DIR/bin/x86_64-w64-mingw32-as" \
            "$(command -v x86_64-w64-mingw32-as 2>/dev/null)"; do
            if [ -n "$cand" ] && [ -x "$cand" ]; then
                if _can_run_as "$cand"; then
                    MINGW_AS="$cand"
                    echo "  Found GNU as: $cand"
                    break
                else
                    echo "  WARNING: $cand exists but cannot assemble with --64"
                fi
            fi
        done
        if [ -z "$MINGW_AS" ]; then
            for cand in \
                "$MINGW_BIN/x86_64-w64-mingw32-as" \
                "$(command -v x86_64-w64-mingw32-as 2>/dev/null)"; do
                if [ -n "$cand" ] && [ -x "$cand" ]; then
                    MINGW_AS="$cand"
                    echo "  Falling back to $cand (will apply --64 filter if needed)"
                    break
                fi
            done
        fi

        # ── Helper: create a dlltool wrapper that passes -S <assembler> ──
        _make_dlltool_wrapper() {
            local real_dlltool="$1" mingw_as="$2"
            local wrapper="/tmp/dlltool-wrapper"
            cat > "$wrapper" <<WEOF
#!/bin/sh
exec "$real_dlltool" -S "$mingw_as" "\$@"
WEOF
            chmod +x "$wrapper"
            echo "$wrapper"
        }

        # ── Helper: create an as wrapper that filters out --64 (incompatible with LLVM as) ──
        _make_as_wrapper() {
            local real_as="$1"
            local wrapper="/tmp/x86_64-w64-mingw32-as-wrapper"
            cat > "$wrapper" <<WEOF
#!/bin/sh
args=()
for a in "\$@"; do
    [ "\$a" = "--64" ] && continue
    args+=("\$a")
done
exec "$real_as" "\${args[@]}"
WEOF
            chmod +x "$wrapper"
            echo "$wrapper"
        }

        # If the selected assembler is LLVM's clang-based as, it rejects
        # the --64 flag that GNU dlltool passes. Wrap it to strip --64,
        # which is redundant on x86_64 anyway.
        if [ -n "$MINGW_AS" ] && _is_llvm_as "$MINGW_AS"; then
            MINGW_AS="$(_make_as_wrapper "$MINGW_AS")"
            echo "  WARNING: LLVM as detected; using --64 filter wrapper: $MINGW_AS"
        fi

        if [ -z "$MINGW_AS" ]; then
            echo "  WARNING: no working mingw assembler found"
        fi

        # ── Strategy 1 (primary): GNU dlltool + GNU as (via -S wrapper) ──
        if _can_run_dlltool "$REAL_DLLTOOL" && [ -n "$MINGW_AS" ]; then
            DLLTOOL_PATH="$(_make_dlltool_wrapper "$REAL_DLLTOOL" "$MINGW_AS")"
            echo "  Using GNU dlltool wrapper: $DLLTOOL_PATH"
            echo "    dlltool=$REAL_DLLTOOL, as=$MINGW_AS"
        fi
        # ── Strategy 1b: GNU dlltool from GNUBIN_DIR + mingw assembler ──
        if [ -z "$DLLTOOL_PATH" ] && [ -n "$MINGW_AS" ]; then
            for cand in \
                "$GNUBIN_DIR/bin/x86_64-w64-mingw32-dlltool" \
                "$GNUBIN_DIR/bin/dlltool"; do
                if _can_run_dlltool "$cand"; then
                    DLLTOOL_PATH="$(_make_dlltool_wrapper "$cand" "$MINGW_AS")"
                    echo "  Using GNU dlltool wrapper: $DLLTOOL_PATH"
                    echo "    dlltool=$cand, as=$MINGW_AS"
                    break
                fi
            done
        fi
        # ── Strategy 2 (fallback): LLVM dlltool (has internal assembler) ──
        if [ -z "$DLLTOOL_PATH" ]; then
            for cand in \
                "$MINGW_BIN/x86_64-w64-mingw32-dlltool" \
                "$MINGW_BIN/dlltool" \
                "$(command -v x86_64-w64-mingw32-dlltool 2>/dev/null)" \
                "$(command -v llvm-dlltool 2>/dev/null)"; do
                if _can_run_dlltool "$cand" && _is_llvm_dlltool "$cand"; then
                    DLLTOOL_PATH="$cand"
                    echo "  Using LLVM dlltool: $DLLTOOL_PATH"
                    break
                fi
            done
        fi

        # ── Sanity test: verify dlltool can generate import libraries ──
        if [ -n "$DLLTOOL_PATH" ]; then
            _sanity_tmp=$(mktemp -d)
            printf 'LIBRARY kernel32.dll\nEXPORTS\nGetTickCount@0\nSleep@4\n' > "$_sanity_tmp/test.def"
            if "$DLLTOOL_PATH" -m i386:x86-64 -d "$_sanity_tmp/test.def" \
                -l "$_sanity_tmp/test.dll.a" -D kernel32.dll 2>/dev/null \
                && [ -f "$_sanity_tmp/test.dll.a" ]; then
                echo "  dlltool sanity test PASSED"
            else
                echo "  WARNING: dlltool sanity test FAILED — raw-dylib linking will likely fail"
            fi
            rm -rf "$_sanity_tmp"
        fi
        if [ -n "$DLLTOOL_PATH" ]; then
            echo "  dlltool (explicit): $DLLTOOL_PATH"
            echo "    $( "$DLLTOOL_PATH" --version 2>&1 | tr '\n' ' ' )"
            CFG=".cargo/config.toml"
            python3 - "$CFG" "$DLLTOOL_PATH" <<'PYEOF' || echo "  WARNING: failed to patch .cargo/config.toml"
import re, sys
path, dlltool = sys.argv[1], sys.argv[2]
with open(path, encoding='utf-8') as f:
    txt = f.read()
pat = re.compile(
    r'(\[target\.x86_64-pc-windows-gnu\][^\[]*?rustflags\s*=\s*\[)([^\]]*\])',
    re.S)
m = pat.search(txt)
if not m:
    print('  WARNING: no [target.x86_64-pc-windows-gnu] rustflags block found')
    sys.exit(1)
head, body = m.group(1), m.group(2)
if '"dlltool=' in body:
    print('  dlltool flag already in rustflags array')
else:
    new = head + '\n  "-C", "dlltool=' + dlltool + '",' + body
    txt = txt[:m.start()] + new + txt[m.end():]
    with open(path, 'w', encoding='utf-8') as f:
        f.write(txt)
    print('  Inserted -C dlltool= into [target.x86_64-pc-windows-gnu].rustflags')
PYEOF
            echo "  --- [target.x86_64-pc-windows-gnu] block now: ---"
            awk '/\[target\.x86_64-pc-windows-gnu\]/{p=1} p{print} /^]/{if(p)p=0}' "$CFG" \
                | sed 's/^/      /'
        else
            echo "  WARNING: no dlltool found; raw-dylib crates will fail to link"
        fi
    fi
fi

echo "=== Building CLI installer ==="
CMD="cargo run -p rim_dev -- dist --cli"
if $BINARY_ONLY; then
    CMD="$CMD -b"
fi
CMD="$CMD --target $BUILD_TARGET --for $DIST_TARGETS"
echo "  Running: $CMD"
$CMD

# Build GUI inside Docker (linux targets only)
if ! $SKIP_GUI; then
    if ! command -v docker >/dev/null 2>&1; then
        echo "WARNING: docker not available, skipping GUI build"
    else
        echo "=== Building GUI via Docker ==="
        DIST_NAME=""
        case "$DIST_TARGETS" in
            *windows*)      echo "WARNING: GUI build skipped for Windows target" ;;
            *aarch64*)      DIST_NAME="${DOCKER_IMAGE_AARCH64:-dist-aarch64-linux}" ;;
            *x86_64*)       DIST_NAME="${DOCKER_IMAGE_X86_64:-dist-x86-64-linux}" ;;
        esac
        if [[ -n "$DIST_NAME" ]]; then
            bash ci/scripts/build-in-docker.sh "$DIST_NAME"
        fi
    fi
fi

echo "=== Validating build artifacts ==="
IFS=',' read -ra DIST_TARGET_ARRAY <<< "$DIST_TARGETS"
HAS_ERROR=false

for DT in "${DIST_TARGET_ARRAY[@]}"; do
    test -d "dist/$DT" || {
        echo "ERROR: dist/$DT not found"
        HAS_ERROR=true
        continue
    }

    CLI_COUNT=$(find "dist/$DT" -maxdepth 1 -type f -name '*-installer-cli*' | wc -l)
    if [[ "$CLI_COUNT" -eq 0 ]]; then
        echo "ERROR: no CLI installer found in dist/$DT"
        HAS_ERROR=true
    else
        echo "  [$DT] CLI installers: $CLI_COUNT found"
    fi

    if ! $SKIP_GUI; then
        GUI_COUNT=$(find "dist/$DT" -maxdepth 1 -type f -name '*-installer*' ! -name '*-installer-cli*' | wc -l)
        echo "  [$DT] GUI installers: $GUI_COUNT found"
    fi

    case "$DT" in
        *windows*)
            ZIP_COUNT=$(find "dist/$DT" -maxdepth 1 -type f -name '*.zip' | wc -l)
            echo "  [$DT] zip packages:  $ZIP_COUNT found"
            ;;
        *linux*)
            XZ_COUNT=$(find "dist/$DT" -maxdepth 1 -type f -name '*.tar.xz' | wc -l)
            echo "  [$DT] tar.xz packages: $XZ_COUNT found"
            ;;
    esac

    echo "=== [$DT] artifacts listing ==="
    find "dist/$DT" -maxdepth 2 -type f -print
done

if $HAS_ERROR; then
    echo "ERROR: one or more dist targets failed validation"
    exit 1
fi

echo "=== Build completed successfully ==="
