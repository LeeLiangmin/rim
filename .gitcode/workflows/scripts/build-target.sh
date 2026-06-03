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
ENV_FILE="$SCRIPT_DIR/../../.env"
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
            *aarch64*)      DIST_NAME="dist-aarch64-linux" ;;
            *x86_64*)       DIST_NAME="dist-x86-64-linux" ;;
        esac
        if [[ -n "$DIST_NAME" ]]; then
            bash ci/scripts/build-in-docker.sh "$DIST_NAME"
        fi
    fi
fi

echo "=== Validating build artifacts ==="
PRIMARY_DIST_TARGET="${DIST_TARGETS%%,*}"
test -d "dist/$PRIMARY_DIST_TARGET" || {
    echo "ERROR: dist/$PRIMARY_DIST_TARGET not found"
    exit 1
}

CLI_COUNT=$(find "dist/$PRIMARY_DIST_TARGET" -maxdepth 1 -type f -name '*-installer-cli*' | wc -l)
if [[ "$CLI_COUNT" -eq 0 ]]; then
    echo "ERROR: no CLI installer found in dist/$PRIMARY_DIST_TARGET"
    exit 1
fi
echo "  CLI installers: $CLI_COUNT found"

if ! $SKIP_GUI; then
    GUI_COUNT=$(find "dist/$PRIMARY_DIST_TARGET" -maxdepth 1 -type f -name '*-installer*' ! -name '*-installer-cli*' | wc -l)
    echo "  GUI installers: $GUI_COUNT found"
fi

case "$PRIMARY_DIST_TARGET" in
    *windows*)
        ZIP_COUNT=$(find "dist/$PRIMARY_DIST_TARGET" -maxdepth 1 -type f -name '*.zip' | wc -l)
        echo "  zip packages:  $ZIP_COUNT found"
        ;;
    *linux*)
        XZ_COUNT=$(find "dist/$PRIMARY_DIST_TARGET" -maxdepth 1 -type f -name '*.tar.xz' | wc -l)
        echo "  tar.xz packages: $XZ_COUNT found"
        ;;
esac

echo "=== Build artifacts listing ==="
find "dist/$PRIMARY_DIST_TARGET" -maxdepth 2 -type f -print

echo "=== Build completed successfully ==="
