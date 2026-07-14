#!/bin/bash
# Download osslsigncode from OBS (signed URL)
set -euo pipefail
DEST_DIR="${1:-/tmp/osslsigncode}"
TOOL="$DEST_DIR/osslsigncode"

if command -v osslsigncode >/dev/null 2>&1; then
  echo "osslsigncode already available"
  exit 0
fi
mkdir -p "$DEST_DIR"

echo "=== Downloading osslsigncode ==="

# Generate OBS signed URL
if [ -n "${OBS_AK_XUANWU_RUST:-}" ] && [ -n "${OBS_SK_XUANWU_RUST:-}" ]; then
  SIGNED_URL=$(python3 -c "
import hashlib, hmac, datetime, urllib.parse, os
ak = os.environ['OBS_AK_XUANWU_RUST']
sk = os.environ['OBS_SK_XUANWU_RUST']
bucket = 'xuanwu-rust'
region = 'cn-north-4'
obj = 'dist/osslsigncode'
t = datetime.datetime.utcnow()
ds = t.strftime('%Y%m%d')
amz_date = t.strftime('%Y%m%dT%H%M%SZ')
cred = ak + '/' + ds + '/' + region + '/s3/aws4_request'
qh = 'host'
qa = 'X-Amz-Algorithm=AWS4-HMAC-SHA256'
qc = 'X-Amz-Credential=' + urllib.parse.quote(cred, safe='')
qd = 'X-Amz-Date=' + amz_date
qe = 'X-Amz-Expires=3600'
qs = 'X-Amz-SignedHeaders=' + qh
canon_qs = qa + '&' + qc + '&' + qd + '&' + qe + '&' + qs
canon = 'GET\n/' + obj + '\n' + canon_qs + '\nhost:' + bucket + '.obs.' + region + '.myhuaweicloud.com\n\n' + qh + '\nUNSIGNED-PAYLOAD'
scope = ds + '/' + region + '/s3/aws4_request'
sts = 'AWS4-HMAC-SHA256\n' + amz_date + '\n' + scope + '\n' + hashlib.sha256(canon.encode()).hexdigest()
def sign(k, m):
    return hmac.new(k, m.encode(), hashlib.sha256).digest()
skb = ('AWS4' + sk).encode()
k = sign(sign(sign(sign(skb, ds), region), 's3'), 'aws4_request')
sig = hmac.new(k, sts.encode(), hashlib.sha256).hexdigest()
print('https://' + bucket + '.obs.' + region + '.myhuaweicloud.com/' + obj + '?' + qa + '&' + qc + '&' + qd + '&' + qe + '&' + qs + '&X-Amz-Signature=' + sig)
")
  if curl -fsSL --connect-timeout 10 --max-time 30 -o "$TOOL" "$SIGNED_URL" 2>&1; then
    chmod +x "$TOOL"
    echo "  OBS download OK"
  fi
fi

if [ -x "$TOOL" ]; then
  echo "osslsigncode ready"
  export PATH="$DEST_DIR:$PATH"
else
  echo "ERROR: osslsigncode not available"
  exit 1
fi
