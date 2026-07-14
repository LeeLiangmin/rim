#!/bin/bash
# Extract originsign binary directly from OCI registry (no container runtime needed)
set -euo pipefail

REGISTRY="forgestash.verify.oepkgs.net"
IMAGE="dmgt/originsign"
TAG="0.0.2"
DEST_DIR="${1:-/tmp/originsign}"
WORK="/tmp/originsign-work"
mkdir -p "$WORK" "$DEST_DIR"

echo "=== Fetching OCI image manifest ==="
MANIFEST=$(curl -fsSL \
  -H "Accept: application/vnd.oci.image.manifest.v1+json" \
  -H "Accept: application/vnd.docker.distribution.manifest.v2+json" \
  "https://${REGISTRY}/v2/${IMAGE}/manifests/${TAG}" 2>&1)

echo "manifest: ${MANIFEST:0:200}..."

# Parse layers from manifest using python3
LAYERS=$(python3 -c "
import json, sys
m = json.loads(sys.stdin.read())
for i, layer in enumerate(m.get('layers', [])):
    d = layer['digest']
    s = layer.get('size', 0)
    print(f'{d}|{s}')
" <<< "$MANIFEST")

echo "=== Downloading layers ==="
mkdir -p "$WORK/extract"
while IFS='|' read -r digest size; do
  echo "  layer: $digest ($size bytes)"
  curl -fsSL --connect-timeout 10 --max-time 120 \
    -o "$WORK/layer.tar.gz" \
    "https://${REGISTRY}/v2/${IMAGE}/blobs/${digest}" 2>&1
  echo "  extracting..."
  tar xzf "$WORK/layer.tar.gz" -C "$WORK/extract" 2>/dev/null || true
  ORIGIN=$(find "$WORK/extract" -name originsign -type f 2>/dev/null | head -1)
  if [ -n "$ORIGIN" ]; then
    cp "$ORIGIN" "$DEST_DIR/originsign"
    chmod +x "$DEST_DIR/originsign"
    echo "  -> found originsign binary!"
  fi
  rm -f "$WORK/layer.tar.gz"
done <<< "$LAYERS"

if [ -x "$DEST_DIR/originsign" ]; then
  echo "=== originsign binary ready ==="
  file "$DEST_DIR/originsign" || true
  "$DEST_DIR/originsign" --version 2>&1 || true
else
  echo "ERROR: originsign binary not found in image layers"
  find "$WORK/extract" -type f 2>/dev/null | head -20
  exit 1
fi
