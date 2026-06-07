#!/bin/bash
set -euo pipefail

usage() {
    echo "Usage: $0 --tag <release-tag> --dist-dir <path>"
    echo "  --tag       GitCode Release tag name"
    echo "  --dist-dir  Path to dist/<target> directory containing artifacts"
    exit 1
}

TAG=""
DIST_DIR=""

while [[ $# -gt 0 ]]; do
    case "$1" in
        --tag) TAG="$2"; shift 2 ;;
        --dist-dir) DIST_DIR="$2"; shift 2 ;;
        -h|--help) usage ;;
        *) echo "Unknown option: $1"; usage ;;
    esac
done

if [[ -z "$TAG" ]] || [[ -z "$DIST_DIR" ]]; then
    echo "ERROR: --tag and --dist-dir are required"
    usage
fi

if [[ ! -d "$DIST_DIR" ]]; then
    echo "ERROR: dist directory not found: $DIST_DIR"
    exit 1
fi

# Load shared config
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ENV_FILE="$SCRIPT_DIR/../.env"
if [[ -f "$ENV_FILE" ]]; then
    set -a; source "$ENV_FILE"; set +a
fi

export GITCODE_TOKEN="${GITCODE_TOKEN:-}"
export GITCODE_OWNER="${GITCODE_OWNER:-xuanwu}"
export GITCODE_REPO="${GITCODE_REPO:-custom-rust-dist}"

if [[ -z "$GITCODE_TOKEN" ]]; then
    echo "ERROR: GITCODE_TOKEN is not set"
    exit 1
fi

echo "=== Collecting release artifacts from $DIST_DIR ==="
FILE_ARGS=""
while IFS= read -r file; do
    [[ -n "$file" ]] || continue
    FILE_ARGS="$FILE_ARGS \"$file\""
done < <(find "$DIST_DIR" -maxdepth 1 -type f | sort)

if [[ -z "$FILE_ARGS" ]]; then
    echo "ERROR: no artifacts found in $DIST_DIR"
    exit 1
fi

echo "=== Uploading to GitCode Release (tag=$TAG) ==="
RELEASE_SCRIPT="$SCRIPT_DIR/release.py"

if [[ ! -f "$RELEASE_SCRIPT" ]]; then
    echo "ERROR: release.py not found at $RELEASE_SCRIPT"
    exit 1
fi

eval python3 "$RELEASE_SCRIPT" --tag "$TAG" --name "$TAG" --body \"Auto release from GitCode CI\" --files $FILE_ARGS

echo "=== Upload completed ==="
