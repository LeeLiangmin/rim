#!/bin/bash
# Download osslsigncode static binary
set -euo pipefail
DEST_DIR="${1:-/tmp/osslsigncode}"
TOOL="$DEST_DIR/osslsigncode"

if command -v osslsigncode >/dev/null 2>&1; then
  echo "osslsigncode already available"
  exit 0
fi
mkdir -p "$DEST_DIR"

echo "=== Downloading osslsigncode ==="
for url in \
  "https://xuanwu-rust.obs.cn-north-4.myhuaweicloud.com/dist/osslsigncode-linux-amd64" \
  "https://github.com/mtrojnar/osslsigncode/releases/download/2.9/osslsigncode-2.9-linux-x64.zip"; do
  echo "  trying: $url"
  if echo "$url" | grep -q obs; then
    curl -fsSL --connect-timeout 10 --max-time 30 -o "$TOOL" "$url" 2>&1 && chmod +x "$TOOL" && break
  else
    curl -fsSL --connect-timeout 10 --max-time 30 -o /tmp/osslsigncode.zip "$url" 2>&1 && \
      unzip -o /tmp/osslsigncode.zip -d "$DEST_DIR" 2>/dev/null && \
      find "$DEST_DIR" -name osslsigncode -type f -exec cp {} "$TOOL" \; && \
      chmod +x "$TOOL" && break
  fi
  echo "  failed"
done

if [ -x "$TOOL" ]; then
  echo "osslsigncode ready"
  export PATH="$DEST_DIR:$PATH"
else
  echo "ERROR: osslsigncode not available"
  exit 1
fi
