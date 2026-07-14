#!/bin/bash
# Download Docker static bundle (containerd, ctr, runc, docker) from OBS or mirrors.
# Requires: OBS_AK_XUANWU_RUST, OBS_SK_XUANWU_RUST env vars.
set -euo pipefail

DEST_DIR="${1:-/tmp/docker}"
DEST_TGZ="/tmp/docker.tgz"

if command -v containerd >/dev/null 2>&1 && command -v ctr >/dev/null 2>&1; then
  echo "containerd already available"
  exit 0
fi

mkdir -p "$DEST_DIR"

# OBS signed URL helper
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
url += '?' + q_algorithm + '&' + q_cred + '&' + q_date + '&' + q_expires + '&' + q_headers + '&X-Amz-Signature=' + sig
print(url)
"
}

echo "=== Downloading Docker bundle ==="
# Try OBS first, then mirrors.huaweicloud.com (proven fast)
DOCKER_URL=$(obs_sign "dist/docker-24.0.7.tgz" 2>/dev/null || echo "")
if [ -n "$DOCKER_URL" ] && curl -fsSL --connect-timeout 10 --max-time 120 -o "$DEST_TGZ" "$DOCKER_URL" 2>&1; then
  echo "  OBS download OK"
else
  echo "  Trying mirrors.huaweicloud.com..."
  curl -fsSL --connect-timeout 10 --max-time 120 -o "$DEST_TGZ" \
    "https://mirrors.huaweicloud.com/docker-ce/linux/static/stable/x86_64/docker-24.0.7.tgz" 2>&1 || exit 1
fi

echo "=== Extracting ==="
tar xzf "$DEST_TGZ" -C /tmp  # Docker.tgz contains ./docker/ dir
chmod +x "$DEST_DIR"/*
ls "$DEST_DIR"/containerd "$DEST_DIR"/ctr "$DEST_DIR"/runc 2>/dev/null || {
  echo "  checking for nested dir..."
  ls /tmp/docker/ 2>/dev/null
}
export PATH="$DEST_DIR:$PATH"
echo "containerd $(containerd --version)"
echo "ctr $(ctr --version)"
