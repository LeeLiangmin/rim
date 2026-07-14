#!/bin/bash
# Download Podman static binary, preferring OBS (signed URL) over GitHub.
# Requires: OBS_AK_XUANWU_RUST, OBS_SK_XUANWU_RUST env vars (from .env).
set -euo pipefail

DEST_DIR="${1:-/tmp/podman}"
DEST_TGZ="/tmp/podman.tar.gz"

if command -v podman >/dev/null 2>&1; then
  echo "Podman already installed: $(podman --version)"
  exit 0
fi

# --- Try OBS with signed URL ---
if [ -n "${OBS_AK_XUANWU_RUST:-}" ] && [ -n "${OBS_SK_XUANWU_RUST:-}" ]; then
  echo "=== Generating signed OBS URL ==="
  SIGNED_URL=$(python3 -c "
import hashlib, hmac, datetime, urllib.parse, os

ak = os.environ['OBS_AK_XUANWU_RUST']
sk = os.environ['OBS_SK_XUANWU_RUST']
bucket = 'xuanwu-rust'
region = 'cn-north-4'
obj = 'dist/podman-linux-amd64.tar.gz'
expires = '3600'

t = datetime.datetime.utcnow()
ds = t.strftime('%Y%m%d')
amz_date = t.strftime('%Y%m%dT%H%M%SZ')
cred = ak + '/' + ds + '/' + region + '/s3/aws4_request'
signed_headers = 'host'

# Build query string (must be in canonical request for presigned URL)
q_algorithm = 'X-Amz-Algorithm=AWS4-HMAC-SHA256'
q_cred = 'X-Amz-Credential=' + urllib.parse.quote(cred, safe='')
q_date = 'X-Amz-Date=' + amz_date
q_expires = 'X-Amz-Expires=' + expires
q_headers = 'X-Amz-SignedHeaders=' + signed_headers
canon_qs = q_algorithm + '&' + q_cred + '&' + q_date + '&' + q_expires + '&' + q_headers

canon_req = 'GET\n/' + obj + '\n' + canon_qs + '\nhost:' + bucket + '.obs.' + region + '.myhuaweicloud.com\n\n' + signed_headers + '\nUNSIGNED-PAYLOAD'

scope = ds + '/' + region + '/s3/aws4_request'
sts = 'AWS4-HMAC-SHA256\n' + amz_date + '\n' + scope + '\n' + hashlib.sha256(canon_req.encode()).hexdigest()

def sign(k, m):
    return hmac.new(k, m.encode(), hashlib.sha256).digest()

skb = ('AWS4' + sk).encode()
k = sign(sign(sign(sign(skb, ds), region), 's3'), 'aws4_request')
sig = hmac.new(k, sts.encode(), hashlib.sha256).hexdigest()

url = 'https://' + bucket + '.obs.' + region + '.myhuaweicloud.com/' + obj
url += '?' + q_algorithm + '&' + q_cred + '&' + q_date + '&' + q_expires + '&' + q_headers
url += '&X-Amz-Signature=' + sig
print(url)
")

  echo "  downloading from OBS..."
  if curl -fsSL --connect-timeout 10 --max-time 120 -o "$DEST_TGZ" "$SIGNED_URL" 2>&1; then
    echo "  OBS download OK"
    mkdir -p "$DEST_DIR"
    tar xzf "$DEST_TGZ" -C "$DEST_DIR" --strip-components=1 2>/dev/null || tar xzf "$DEST_TGZ" -C "$DEST_DIR"
    find "$DEST_DIR" -name podman -type f -exec chmod +x {} \;
    export PATH="$DEST_DIR:$DEST_DIR/usr/local/bin:$PATH"
    if command -v podman >/dev/null 2>&1; then
      echo "Podman $(podman --version) installed via OBS"
      exit 0
    fi
  else
    echo "  OBS download failed"
  fi
fi

echo "ERROR: Could not download Podman from OBS"
exit 1
