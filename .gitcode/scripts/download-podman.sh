#!/bin/bash
# Download Podman + dependencies (conmon) for rootless container operations.
# Requires: OBS_AK_XUANWU_RUST, OBS_SK_XUANWU_RUST env vars (from .env).
set -euo pipefail

DEST_DIR="${1:-/tmp/podman}"
DEST_TGZ="/tmp/podman.tar.gz"
OBS_BASE="https://xuanwu-rust.obs.cn-north-4.myhuaweicloud.com/dist"

if command -v podman >/dev/null 2>&1 && command -v conmon >/dev/null 2>&1; then
  echo "Podman already installed: $(podman --version)"
  exit 0
fi

# --- Helper: generate OBS signed URL ---
obs_sign() {
  python3 -c "
import hashlib, hmac, datetime, urllib.parse, os
ak = os.environ['OBS_AK_XUANWU_RUST']
sk = os.environ['OBS_SK_XUANWU_RUST']
bucket = 'xuanwu-rust'
region = 'cn-north-4'
obj = '$1'

t = datetime.datetime.utcnow()
ds = t.strftime('%Y%m%d')
amz_date = t.strftime('%Y%m%dT%H%M%SZ')
cred = ak + '/' + ds + '/' + region + '/s3/aws4_request'
signed_headers = 'host'

q_algorithm = 'X-Amz-Algorithm=AWS4-HMAC-SHA256'
q_cred = 'X-Amz-Credential=' + urllib.parse.quote(cred, safe='')
q_date = 'X-Amz-Date=' + amz_date
q_expires = 'X-Amz-Expires=3600'
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
"
}

# --- Download a file from OBS ---
obs_dl() {
  local obj="$1" dest="$2" label="$3"
  echo "=== Downloading $label from OBS ==="
  local url
  url=$(obs_sign "$obj")
  if curl -fsSL --connect-timeout 10 --max-time 120 -o "$dest" "$url" 2>&1; then
    echo "  $label OK"
    return 0
  fi
  echo "  $label FAILED"
  return 1
}

mkdir -p "$DEST_DIR"

# --- Download podman ---
if ! command -v podman >/dev/null 2>&1; then
  obs_dl "dist/podman-linux-amd64.tar.gz" "$DEST_TGZ" "podman" || exit 1
  tar xzf "$DEST_TGZ" -C "$DEST_DIR" --strip-components=1 2>/dev/null || tar xzf "$DEST_TGZ" -C "$DEST_DIR"
  find "$DEST_DIR" -name podman -type f -exec chmod +x {} \;
  export PATH="$DEST_DIR:$DEST_DIR/usr/local/bin:$PATH"
  echo "Podman $(podman --version)"
fi

# --- Download conmon (required by podman) ---
if ! command -v conmon >/dev/null 2>&1; then
  CONMON_TGZ="/tmp/conmon.tgz"
  if obs_dl "dist/conmon-amd64-v2.1.12.tgz" "$CONMON_TGZ" "conmon"; then
    tar xzf "$CONMON_TGZ" -C "$DEST_DIR"
  else
    echo "=== Trying conmon from GitHub ==="
    CONMON_URL="https://github.com/containers/conmon/releases/download/v2.1.12/conmon.amd64"
    if curl -fsSL --connect-timeout 10 --max-time 60 -o "$DEST_DIR/conmon" "$CONMON_URL" 2>&1; then
      chmod +x "$DEST_DIR/conmon"
      echo "  conmon OK (GitHub)"
    else
      echo "ERROR: conmon not available"
      exit 1
    fi
  fi
  chmod +x "$DEST_DIR/conmon" 2>/dev/null || true
  echo "conmon $(command -v conmon)"
fi

export PATH="$DEST_DIR:$DEST_DIR/usr/local/bin:$PATH"
echo "All dependencies ready"
