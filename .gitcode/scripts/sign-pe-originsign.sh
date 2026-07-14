#!/bin/bash
# Sign a Windows PE (.exe) file using originsign local KMS + osslsigncode
# Usage: sign-pe-originsign.sh <unsigned.exe> <signed.exe>
set -euo pipefail

UNSIGNED="${1?Usage: $0 <unsigned.exe> <signed.exe>}"
SIGNED="${2?Usage: $0 <unsigned.exe> <signed.exe>}"
ORIGINSIGN_DIR="/tmp/originsign"
KEY_ID=""

echo "=== Signing: $UNSIGNED -> $SIGNED ==="

# 1. Ensure originsign server is running
if ! curl -s http://localhost:8081/v1/sign/digest -o /dev/null 2>/dev/null; then
  echo "  starting originsign server..."
  "$ORIGINSIGN_DIR/originsign" server &
  sleep 3
fi

# 2. Init KMS and get key_id (skip if already done)
if [ -z "$KEY_ID" ]; then
  "$ORIGINSIGN_DIR/originsign" import --kms local 2>/dev/null || true
  KEY_ID=$("$ORIGINSIGN_DIR/originsign" import --kms local --list 2>&1 | grep -oE '[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}' | head -1)
  if [ -z "$KEY_ID" ]; then
    echo "  generating rsa key..."
    "$ORIGINSIGN_DIR/originsign" keygen --algo rsa
    "$ORIGINSIGN_DIR/originsign" import --kms local --algo rsa --priv rsa-priv-key.pem --pub rsa-pub-key.pem
    KEY_ID=$("$ORIGINSIGN_DIR/originsign" import --kms local --list 2>&1 | grep -oE '[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}' | head -1)
  fi
fi
echo "  key_id: $KEY_ID"

# 3. Compute PE digest
DIGEST=$(sha256sum "$UNSIGNED" | cut -d' ' -f1)
echo "  digest: $DIGEST"

# 4. Call originsign to sign the digest
SIGN_RESP=$(curl -s -X POST http://localhost:8081/v1/sign/digest \
  -H "Content-Type: application/json" \
  -d "{\"base_config\":{\"algo\":\"rsa\",\"kms\":\"local\",\"flow\":\"sigstore\",\"sys\":\"pki\"},\"digest\":\"$DIGEST\",\"key_id\":\"$KEY_ID\"}")
echo "  sign response: ${SIGN_RESP:0:200}..."

# 5. Extract signature and CertPEM
SIGNATURE=$(echo "$SIGN_RESP" | python3 -c "import sys,json; print(json.load(sys.stdin)['signature'])" 2>/dev/null)
CERT_B64=$(echo "$SIGN_RESP" | python3 -c "import sys,json; c=json.load(sys.stdin).get('cert','{}'); print(json.loads(c).get('CertPEM',''))" 2>/dev/null)
if [ -z "$SIGNATURE" ] || [ -z "$CERT_B64" ]; then
  echo "ERROR: failed to extract signature or cert from response"
  exit 1
fi

# 6. Decode CertPEM → certificate.pem
echo "$CERT_B64" | base64 -d > /tmp/certificate.pem 2>/dev/null || {
  python3 -c "import base64; print(base64.b64decode('$CERT_B64').decode())" > /tmp/certificate.pem
}
echo "  cert decoded ($(wc -c < /tmp/certificate.pem) bytes)"

# 7. Sign PE with osslsigncode (use cert + key from originsign output)
#    originsign uses PKI flow, cert is the X.509 signing cert.
#    Sign with -certs (certificate) and -key (private key, already in KMS).
#    Since originsign holds the private key, we need to export it or use the .pem from keygen.
if [ -f rsa-priv-key.pem ] && [ -f rsa-pub-key.pem ]; then
  # Export to PKCS12 for osslsigncode
  openssl pkcs12 -export -in /tmp/certificate.pem -inkey rsa-priv-key.pem \
    -out /tmp/signing.pfx -passout pass: 2>/dev/null || true
  osslsigncode sign -pkcs12 /tmp/signing.pfx -in "$UNSIGNED" -out "$SIGNED" 2>&1
else
  # Fallback: attach signature to a detached .sig file
  echo "$SIGNATURE" > "${SIGNED}.sig"
  cp "$UNSIGNED" "$SIGNED"
  echo "  NOTE: osslsigncode PKCS12 export failed, saved detached .sig"
fi

echo "=== Signed: $SIGNED ==="
ls -la "$SIGNED" 2>/dev/null || true
