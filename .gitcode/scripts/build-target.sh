#!/bin/bash
set -euo pipefail

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

            if curl -sSL --connect-timeout 10 --max-time 300 "$TOOLCHAIN_URL" | tar -xJ --strip-components=1 -C "$TOOLCHAIN_DIR"; then
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

# Set CC/linker for Windows mingw cross-compilation
if [[ "$BUILD_TARGET" == *"windows-gnu"* ]]; then
    if [[ -d "$HOME/mingw-toolchain/bin" ]]; then
        export PATH="$HOME/mingw-toolchain/bin:$PATH"
    fi

    if ! command -v x86_64-w64-mingw32-gcc >/dev/null 2>&1; then
        echo "  x86_64-w64-mingw32-gcc not found, downloading mingw toolchain from OBS..."
        TOOLCHAIN_DIR="$HOME/mingw-toolchain"
        TOOLCHAIN_URL="${MINGW_TOOLCHAIN_URL:-https://xuanwu-rust.obs.cn-north-4.myhuaweicloud.com/dist/mingw-w64-cross.tar.xz}"
        mkdir -p "$TOOLCHAIN_DIR"

        if curl -sSL --connect-timeout 10 --max-time 300 "$TOOLCHAIN_URL" | tar -xJ --strip-components=1 -C "$TOOLCHAIN_DIR"; then
            export PATH="$TOOLCHAIN_DIR/bin:$PATH"
            echo "  Installed mingw toolchain to $TOOLCHAIN_DIR"
        else
            echo "ERROR: failed to download mingw cross-compiler from OBS"
            echo "  URL: $TOOLCHAIN_URL"
            exit 1
        fi
    fi

    if command -v x86_64-w64-mingw32-gcc >/dev/null 2>&1; then
        echo "  Using x86_64-w64-mingw32-gcc for Windows cross-compilation"
        export CC_x86_64_pc_windows_gnu=x86_64-w64-mingw32-gcc
        export CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER=x86_64-w64-mingw32-gcc

        # Ensure dlltool is available in PATH
        MINGW_BIN=$(dirname "$(which x86_64-w64-mingw32-gcc)")
        if ! command -v dlltool >/dev/null 2>&1; then
            if [ -f "$MINGW_BIN/x86_64-w64-mingw32-dlltool" ]; then
                ln -sf "$MINGW_BIN/x86_64-w64-mingw32-dlltool" "$MINGW_BIN/dlltool"
            elif [ -f "$MINGW_BIN/llvm-dlltool" ]; then
                ln -sf "$MINGW_BIN/llvm-dlltool" "$MINGW_BIN/dlltool"
            fi
        fi

        # Debug: show Rust's windows-gnu target files and dlltool situation
        RUST_SYSROOT=$(rustc --print sysroot 2>/dev/null || true)
        echo "  Rust sysroot: $RUST_SYSROOT"
        echo "  Rust version: $(rustc --version)"
        echo "  Contents of windows-gnu target dir:"
        find "$RUST_SYSROOT/lib/rustlib/x86_64-pc-windows-gnu" -type f 2>/dev/null | head -20 || echo "  (no x86_64-pc-windows-gnu dir)"
        echo "  dlltool in PATH: $(which dlltool 2>/dev/null || echo 'not found')"
        echo "  dlltool test run:"
        dlltool --version 2>&1 || echo "  dlltool execution failed"
    fi
fi

# Restore cargo env if available
if [[ -f "$HOME/.cargo/env" ]]; then
    source "$HOME/.cargo/env"
fi

echo "=== Vendoring offline packages ==="
cargo run -p rim_dev -- vendor --for "$DIST_TARGETS"

echo "=== Building CLI installer ==="
CMD="cargo run -p rim_dev -- dist --cli"
if $BINARY_ONLY; then
    CMD="$CMD -b"
fi
CMD="$CMD --target $BUILD_TARGET --for $DIST_TARGETS"
echo "  Running: $CMD"
$CMD

# Build GUI inside Docker
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
