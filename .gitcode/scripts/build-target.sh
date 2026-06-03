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
    # openssl-sys hardcodes CC=x86_64-linux-musl-gcc for this target
    # musl-tools package only provides musl-gcc, so create symlink in /usr/bin
    if ! command -v x86_64-linux-musl-gcc >/dev/null 2>&1 && command -v musl-gcc >/dev/null 2>&1; then
        ln -sf "$(which musl-gcc)" /usr/bin/x86_64-linux-musl-gcc
        echo "  Created symlink: /usr/bin/x86_64-linux-musl-gcc -> $(which musl-gcc)"
    fi
    # Also set cargo linker and CC env vars to ensure all subprocesses use musl-gcc
    export CC=musl-gcc
    export CC_x86_64_unknown_linux_musl=musl-gcc
    export CARGO_TARGET_X86_64_UNKNOWN_LINUX_MUSL_LINKER=musl-gcc
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
