#!/usr/bin/env python3
"""
Authenticode sign a Windows PE (.exe) file using Azure KeyVault.

Strategy:
  1. Use osslsigncode with a temp self-signed cert to produce a correctly-
     structured Authenticode PKCS#7. osslsigncode handles all PE complexity
     (digest computation, SpcIndirectDataContent, optional attributes,
      PE alignment, checksum skipping, etc.).
  2. Extract the raw authenticatedAttributes DER (the [0] IMPLICIT SET).
  3. Send those bytes to Azure KeyVault sign API (RS256).
  4. Get the signing cert + chain from Azure KeyVault.
  5. Build a new PKCS#7 SignedData using pure-Python DER encoding,
     reusing the extracted SpcIndirectDataContent and auth attrs.
  6. Embed the new PKCS#7 into the PE.

Usage:
  python sign-pe-azure.py \\
      --exe hello.exe \\
      --vault-url https://openatomkey.vault.azure.net/ \\
      --cert-name OPENATOM \\
      --key-name OPENATOM \\
      --tenant-id <tenant> \\
      --client-id <client> \\
      --client-secret <secret> \\
      [--osslsigncode /path/to/osslsigncode] \\
      [--dry-run] [--verbose]

Dependencies: pip install pefile cryptography requests
Optional: osslsigncode (auto-detected)
"""

import argparse
import base64
import hashlib
import os
import shutil
import struct
import subprocess
import sys
import tempfile
import time
from datetime import datetime, timezone
from pathlib import Path
from typing import List, Optional, Tuple

# ── Check required packages ───────────────────────────────────────────
try:
    import pefile
except ImportError:
    sys.exit("ERROR: pefile not installed. Run: pip install pefile")

try:
    from cryptography import x509
    from cryptography.x509.oid import NameOID
    from cryptography.hazmat.primitives import hashes, serialization
    from cryptography.hazmat.primitives.asymmetric import rsa
    from cryptography.hazmat.backends import default_backend
except ImportError:
    sys.exit("ERROR: cryptography not installed. Run: pip install cryptography")

try:
    import requests
except ImportError:
    sys.exit("ERROR: requests not installed. Run: pip install requests")


# ── Constants ─────────────────────────────────────────────────────────
WIN_CERT_TYPE_PKCS_SIGNED_DATA = 0x0002
WIN_CERT_REVISION = 0x0200
PE_CERT_TABLE_OFFSET_PE32 = 152
PE_CERT_TABLE_OFFSET_PE32PLUS = 168

# OIDs
OID_SIGNED_DATA = "1.2.840.113549.1.7.2"
OID_SPC_INDIRECT_DATA_OBJID = "1.3.6.1.4.1.311.2.1.4"   # encapContentInfo contentType
OID_SPC_PE_IMAGE_DATA = "1.3.6.1.4.1.311.2.1.15"        # SpcAttributeTypeAndOptionalValue.type
OID_SHA256 = "2.16.840.1.101.3.4.2.1"
OID_RSA = "1.2.840.113549.1.1.1"
OID_CONTENT_TYPE = "1.2.840.113549.1.9.3"
OID_MESSAGE_DIGEST = "1.2.840.113549.1.9.4"
OID_SIGNING_TIME = "1.2.840.113549.1.9.5"
# Note: OID_SPC_INDIRECT_DATA kept as alias for backward compat in extraction
OID_SPC_INDIRECT_DATA = OID_SPC_INDIRECT_DATA_OBJID


# ── DER encoding helpers ──────────────────────────────────────────────

def der_len(length: int) -> bytes:
    if length < 0x80:
        return bytes([length])
    encoded = length.to_bytes((length.bit_length() + 7) // 8, "big")
    return bytes([0x80 | len(encoded)]) + encoded


def der_tlv(tag: int, value: bytes) -> bytes:
    return bytes([tag]) + der_len(len(value)) + value


def der_sequence(value: bytes) -> bytes:
    return der_tlv(0x30, value)


def der_set(value: bytes) -> bytes:
    return der_tlv(0x31, value)


def der_oid(dotted: str) -> bytes:
    parts = [int(x) for x in dotted.split(".")]
    data = bytes([40 * parts[0] + parts[1]])
    for p in parts[2:]:
        enc = []
        while p > 0:
            enc.insert(0, p & 0x7F)
            p >>= 7
        if not enc:
            enc = [0]
        for i in range(len(enc) - 1):
            enc[i] |= 0x80
        data += bytes(enc)
    return der_tlv(0x06, data)


def der_octet_string(data: bytes) -> bytes:
    return der_tlv(0x04, data)


def der_integer(val: int) -> bytes:
    if val == 0:
        return b"\x02\x01\x00"
    if val > 0:
        v = val.to_bytes((val.bit_length() + 7) // 8, "big")
        if v[0] & 0x80:
            v = b"\x00" + v
    else:
        v = val.to_bytes(((-val).bit_length() + 8) // 8, "big", signed=True)
    return der_tlv(0x02, v)


def der_null() -> bytes:
    return b"\x05\x00"


def der_utc_time(dt: datetime) -> bytes:
    return der_tlv(0x17, dt.strftime("%y%m%d%H%M%SZ").encode("ascii"))


def der_context(tag: int, value: bytes, constructed: bool = True) -> bytes:
    tag_byte = 0xA0 | (0x20 if constructed else 0) | (tag & 0x1F)
    return bytes([tag_byte]) + der_len(len(value)) + value


# ── Azure KeyVault client ─────────────────────────────────────────────

class AzureKeyVaultClient:
    def __init__(self, vault_url: str, tenant_id: str,
                 client_id: str, client_secret: str):
        self.vault_url = vault_url.rstrip("/")
        self.tenant_id = tenant_id
        self.client_id = client_id
        self.client_secret = client_secret
        self._token: Optional[str] = None
        self._token_expiry: float = 0.0

    def _get_token(self) -> str:
        if self._token and time.time() < self._token_expiry - 60:
            return self._token
        resp = requests.post(
            f"https://login.microsoftonline.com/{self.tenant_id}/oauth2/v2.0/token",
            data={
                "grant_type": "client_credentials",
                "client_id": self.client_id,
                "client_secret": self.client_secret,
                "scope": "https://vault.azure.net/.default",
            },
            timeout=30,
        )
        resp.raise_for_status()
        data = resp.json()
        self._token = data["access_token"]
        self._token_expiry = time.time() + data.get("expires_in", 3600)
        return self._token

    def _api(self, method: str, path: str, **kwargs) -> dict:
        token = self._get_token()
        url = f"{self.vault_url}{path}?api-version=7.4"
        headers = kwargs.pop("headers", {})
        headers["Authorization"] = f"Bearer {token}"
        headers.setdefault("Content-Type", "application/json")
        resp = requests.request(method, url, headers=headers, timeout=60, **kwargs)
        resp.raise_for_status()
        return resp.json()

    def sign(self, key_name: str, data: bytes, algo: str = "RS256") -> bytes:
        """Sign raw data via Azure KeyVault. RS256: Azure SHA256-hex then RSA-signs."""
        value_b64 = base64.urlsafe_b64encode(data).decode("ascii")
        result = self._api("POST", f"/keys/{key_name}/sign",
                           json={"alg": algo, "value": value_b64})
        return base64.urlsafe_b64decode(result["value"] + "==")

    def get_certificates(self, cert_name: str) -> Tuple[x509.Certificate, List[bytes]]:
        """Returns (leaf_x509_cert, [cert1_der, cert2_der, ...])."""
        info = self._api("GET", f"/certificates/{cert_name}")
        sid = info.get("sid", "")
        if not sid:
            raise ValueError(f"No SID for certificate '{cert_name}'")
        parts = sid.rstrip("/").split("/")
        secret = self._api("GET", f"/secrets/{parts[-2]}/{parts[-1]}")
        cert_data = base64.b64decode(secret["value"])

        # Parse cert bundle
        certs = []
        # Try PKCS7 PEM
        try:
            certs = list(serialization.pkcs7.load_pem_pkcs7_certificates(cert_data))
        except Exception:
            pass
        if not certs:
            try:
                certs = list(x509.load_pem_x509_certificates(cert_data))
            except Exception:
                pass
        if not certs:
            try:
                certs.append(x509.load_der_x509_certificate(cert_data))
            except Exception:
                pass
        if not certs:
            try:
                from cryptography.hazmat.primitives.serialization import pkcs12
                _, c, chain = pkcs12.load_key_and_certificates(cert_data, None)
                if c: certs.append(c)
                if chain: certs.extend(chain)
            except Exception:
                pass
        if not certs:
            raise ValueError("Failed to parse certificate from KeyVault")

        der_list = [c.public_bytes(serialization.Encoding.DER) for c in certs]
        return certs[0], der_list


# ── osslsigncode helpers ──────────────────────────────────────────────

def find_osslsigncode() -> Optional[str]:
    for name in ["osslsigncode", "osslsigncode.exe"]:
        path = shutil.which(name)
        if path:
            return path
    import glob
    for base in [
        os.path.expandvars(r"%LOCALAPPDATA%\Microsoft\WinGet\Packages"),
        os.path.expandvars(r"%ProgramFiles%\osslsigncode"),
    ]:
        pattern = os.path.join(base, "MichalTrojnara.osslsigncode_*", "bin", "osslsigncode.exe")
        matches = sorted(glob.glob(pattern))
        if matches:
            return matches[-1]
    return None


def _run_osslsigncode_scaffold(
    osslsigncode_bin: str, exe_path: str
) -> Tuple[bytes, bytes, bytes]:
    """Run osslsigncode with temp cert to create PKCS#7 scaffold.

    Returns: (pkcs7_der, cert_table_va, cert_table_size)
    """
    tmpdir = tempfile.mkdtemp()
    try:
        # Generate temp key + cert
        key = rsa.generate_private_key(65537, 2048)
        key_path = os.path.join(tmpdir, "key.pem")
        with open(key_path, "wb") as f:
            f.write(key.private_bytes(
                encoding=serialization.Encoding.PEM,
                format=serialization.PrivateFormat.TraditionalOpenSSL,
                encryption_algorithm=serialization.NoEncryption(),
            ))

        now = datetime.now(timezone.utc)
        subject = issuer = x509.Name([
            x509.NameAttribute(NameOID.COMMON_NAME, "Temp Scaffold"),
        ])
        cert = (
            x509.CertificateBuilder()
            .subject_name(subject).issuer_name(issuer)
            .public_key(key.public_key())
            .serial_number(x509.random_serial_number())
            .not_valid_before(now)
            .not_valid_after(now.replace(year=now.year + 1))
            .add_extension(
                x509.ExtendedKeyUsage([x509.oid.ExtendedKeyUsageOID.CODE_SIGNING]),
                critical=False,
            )
            .sign(key, hashes.SHA256())
        )
        cert_path = os.path.join(tmpdir, "cert.pem")
        with open(cert_path, "wb") as f:
            f.write(cert.public_bytes(serialization.Encoding.PEM))

        # Create PKCS#12
        pfx_path = os.path.join(tmpdir, "temp.pfx")
        subprocess.run([
            "openssl", "pkcs12", "-export",
            "-in", cert_path, "-inkey", key_path,
            "-out", pfx_path, "-passout", "pass:temp",
        ], check=True, capture_output=True)

        # Sign with osslsigncode
        signed_exe = os.path.join(tmpdir, "signed.exe")
        subprocess.run([
            osslsigncode_bin, "sign",
            "-pkcs12", pfx_path, "-pass", "temp",
            "-h", "sha256",
            "-in", exe_path, "-out", signed_exe,
        ], check=True, capture_output=True)

        # Extract PKCS#7
        pe = pefile.PE(signed_exe)
        sec = pe.OPTIONAL_HEADER.DATA_DIRECTORY[
            pefile.DIRECTORY_ENTRY["IMAGE_DIRECTORY_ENTRY_SECURITY"]
        ]
        with open(signed_exe, "rb") as f:
            f.seek(sec.VirtualAddress)
            dwLength = struct.unpack("<I", f.read(4))[0]
            _ = f.read(4)  # revision + type
            pkcs7 = f.read(dwLength - 8)

        return pkcs7, sec.VirtualAddress, sec.Size
    finally:
        shutil.rmtree(tmpdir, ignore_errors=True)


# ── DER parser (simple, just enough for PKCS#7) ───────────────────────

def _rd(data: bytes, pos: int) -> Tuple[int, bytes, int]:
    """Read TLV: (tag, value, next_pos)."""
    tag = data[pos]
    pos += 1
    length = data[pos]
    pos += 1
    if length & 0x80:
        n = length & 0x7F
        length = int.from_bytes(data[pos:pos + n], "big")
        pos += n
    return tag, data[pos:pos + length], pos + length


def _extract_nested(data: bytes, path: list) -> bytes:
    """Walk DER structure to extract a nested TLV by tag path.
    Path is a list of expected tags. Returns the value bytes of the last tag.
    """
    rest = data
    for expected_tag in path:
        tag, rest, _ = _rd(rest, 0)
        if tag != expected_tag:
            raise ValueError(f"Expected tag {expected_tag:#04x}, got {tag:#04x}")
        # For constructed tags, we need to go one level deeper
        if tag in (0x30, 0x31, 0xA0):
            # Unwrap and continue inside
            inner = rest
            # Actually rest IS the content of the SEQUENCE/SET/[0]
            rest = inner
        else:
            rest = rest
    return rest


def _extract_auth_attrs_and_spc(pkcs7_der: bytes) -> Tuple[bytes, bytes]:
    """Extract authenticatedAttributes DER and SpcIndirectDataContent DER
    from a PKCS#7 SignedData by walking the DER structure precisely.

    Uses absolute positions in the original pkcs7_der to extract exact
    byte ranges for each component. No TLV reconstruction needed.

    Returns: (auth_attrs_der, spc_content_der)
      - auth_attrs_der: the full [0] IMPLICIT authenticatedAttributes TLV
      - spc_content_der: the full [0] EXPLICIT SpcIndirectDataContent TLV
    """
    # Walk: ContentInfo SEQUENCE
    tag, ci_inner, pos0 = _rd(pkcs7_der, 0)
    assert tag == 0x30, f"Bad ContentInfo tag: {tag:#04x}"
    ci_start = 0

    # Skip OID inside ContentInfo
    _, oid_val, pos1 = _rd(ci_inner, 0)

    # [0] EXPLICIT SignedData
    tag, sd_outer, pos2 = _rd(ci_inner, pos1)
    assert tag == 0xA0, f"Bad SignedData [0] tag: {tag:#04x}"

    # SignedData SEQUENCE
    tag, sd_inner, pos3 = _rd(sd_outer, 0)
    assert tag == 0x30, f"Bad SignedData SEQUENCE tag: {tag:#04x}"

    # ── Walk SignedData elements, tracking absolute byte ranges ─────
    # We need to know where each element starts in the original PKCS#7
    # sd_inner starts at absolute offset: (the position after SEQUENCE header in sd_outer)
    # But we can find positions relative to pkcs7_der later by walking forward

    # Let's walk sd_inner element by element
    sd_pos = 0  # relative to sd_inner

    # element 1: version
    _, _, sd_pos = _rd(sd_inner, sd_pos)
    # element 2: digestAlgorithms
    _, _, sd_pos = _rd(sd_inner, sd_pos)
    # element 3: encapContentInfo
    _, eci_val, sd_pos = _rd(sd_inner, sd_pos)

    # Inside encapContentInfo: OID + [0] EXPLICIT SpcIndirectDataContent
    eci_p = 0
    _, _, eci_p = _rd(eci_val, eci_p)  # skip OID
    # [0] EXPLICIT SpcContent
    _, spc_val, eci_p = _rd(eci_val, eci_p)

    # Extract the exact [0] EXPLICIT + SpcContent bytes from eci_val
    # The [0] tag is at position 0 of the value we read, relative to eci_val
    # Let's just walk eci_val again to get exact positions
    eci_p2 = 0
    _, oid_v, eci_p2 = _rd(eci_val, eci_p2)
    oid_tlv_len = _tlv_len(oid_v, eci_val, eci_p2, eci_p2)  # get TLV byte span
    # Actually simpler: the A0 starts at eci_p2 (position after OID in eci_val)
    spc_start_in_eci = eci_p2  # This is where A0 tag is
    spc_tlv = eci_val[spc_start_in_eci:]  # From A0 to end of eci_val

    # element 4: [0] certificates
    _, _, sd_pos = _rd(sd_inner, sd_pos)
    # element 5: signerInfos SET
    _, si_set_val, sd_pos = _rd(sd_inner, sd_pos)

    # Inside signerInfos: SET { SignerInfo SEQUENCE { ... } }
    si_pos = 0
    _, si_val, si_pos = _rd(si_set_val, si_pos)

    # Walk SignerInfo elements
    si_p = 0
    # version
    _, _, si_p = _rd(si_val, si_p)
    # issuerAndSerialNumber
    _, _, si_p = _rd(si_val, si_p)
    # digestAlgorithm
    _, _, si_p = _rd(si_val, si_p)
    # [0] authenticatedAttributes — this is what we want
    auth_start = si_p  # Position of A0 tag in si_val
    _, auth_val, si_p = _rd(si_val, si_p)
    auth_end = si_p  # Position after auth attrs value in si_val

    # Extract auth attrs TLV from si_val
    auth_attrs_der = si_val[auth_start:auth_end]

    return auth_attrs_der, spc_tlv


def _tlv_len(value: bytes, parent: bytes, pos_after_value: int, start_pos: int) -> int:
    """Helper: not used, kept for reference."""
    return 0


# ── PKCS#7 builder ────────────────────────────────────────────────────

def build_pkcs7_signed_data(
    spc_content_der: bytes,
    auth_attrs_der: bytes,
    certs_der_list: List[bytes],
    signature: bytes,
) -> bytes:
    """Build a complete Authenticode PKCS#7 SignedData.

    spc_content_der: Raw DER of SpcIndirectDataContent (inner SEQUENCE,
                     without the [0] EXPLICIT wrapper).
                     Or full [0] EXPLICIT + SEQUENCE - we handle both.
    auth_attrs_der: Raw DER of [0] IMPLICIT authenticatedAttributes.
    certs_der_list: List of DER-encoded X.509 certificates (leaf first).
    signature: Raw RSA signature bytes (256 for RSA 2048).
    """

    # Parse leaf cert to get issuer + serial
    leaf_der = certs_der_list[0]
    leaf_cert = x509.load_der_x509_certificate(leaf_der)
    issuer_der = leaf_cert.issuer.public_bytes()
    serial = leaf_cert.serial_number

    # ---- digestAlgorithms ----
    digest_algo_seq = der_sequence(der_oid(OID_SHA256) + der_null())
    digest_algos = der_set(digest_algo_seq)

    # ---- encapContentInfo ----
    # spc_content_der might include [0] EXPLICIT wrapper. We need the inner SEQUENCE.
    if spc_content_der[0] == 0xA0:
        # Already wrapped - use as-is for the content field
        # But we need to verify: content field expects the [0] EXPLICIT wrapper
        spc_content_val = spc_content_der
    else:
        # Wrap in [0] EXPLICIT
        spc_content_val = der_context(0, spc_content_der)

    encap_content_info = der_sequence(
        der_oid(OID_SPC_INDIRECT_DATA) +
        spc_content_val
    )

    # ---- certificates ----
    # Concatenate all cert DERs
    all_certs = b"".join(certs_der_list)
    certs_tagged = der_context(0, all_certs, constructed=True)

    # ---- SignerInfo ----
    # issuerAndSerialNumber
    ias = der_sequence(issuer_der + der_integer(serial))

    # digestAlgorithm (same as above)
    da = der_sequence(der_oid(OID_SHA256) + der_null())

    # authenticatedAttributes: already has [0] IMPLICIT tag → use as-is
    auth_attrs_tagged = auth_attrs_der  # bytes starting with 0xA0

    # digestEncryptionAlgorithm
    dea = der_sequence(der_oid(OID_RSA) + der_null())

    # encryptedDigest
    enc_digest = der_octet_string(signature)

    signer_info = der_sequence(
        der_integer(1) +          # version
        ias +                     # issuerAndSerialNumber
        da +                      # digestAlgorithm
        auth_attrs_tagged +       # [0] authenticatedAttributes
        dea +                     # digestEncryptionAlgorithm
        enc_digest                # encryptedDigest
    )

    # ---- signerInfos ----
    signer_infos = der_set(signer_info)

    # ---- SignedData ----
    signed_data = der_sequence(
        der_integer(1) +          # version
        digest_algos +            # digestAlgorithms
        encap_content_info +      # encapContentInfo
        certs_tagged +            # [0] certificates
        signer_infos              # signerInfos
    )

    # ---- ContentInfo ----
    pkcs7 = der_sequence(
        der_oid(OID_SIGNED_DATA) +
        der_context(0, signed_data)
    )

    return pkcs7


# ── PE embedding ──────────────────────────────────────────────────────

def _cert_table_offset(pe: pefile.PE) -> int:
    """Calculate the file offset of the Certificate Table DataDirectory entry.

    The DataDirectory array is inside the Optional Header.
    For PE32:  DataDirectory starts at OptionalHeader + 96
    For PE32+: DataDirectory starts at OptionalHeader + 112
    Security Directory = DataDirectory[4] → + 4 * 8 = + 32.
    """
    opt_base = pe.OPTIONAL_HEADER.get_file_offset()
    if pe.PE_TYPE == pefile.OPTIONAL_HEADER_MAGIC_PE:
        return opt_base + 96 + 4 * 8   # PE32: DataDir at +96, entry 4 at +32
    return opt_base + 112 + 4 * 8       # PE32+: DataDir at +112, entry 4 at +32


def embed_pkcs7_in_pe(exe_path: str, pkcs7_der: bytes, output_path: str):
    pe = pefile.PE(exe_path)
    with open(exe_path, "rb") as f:
        data = bytearray(f.read())

    cert_off = _cert_table_offset(pe)
    security = pe.OPTIONAL_HEADER.DATA_DIRECTORY[
        pefile.DIRECTORY_ENTRY["IMAGE_DIRECTORY_ENTRY_SECURITY"]
    ]

    # Remove old certificate
    if security.VirtualAddress != 0 and security.Size > 0:
        # Round up old size to alignment
        old_end = security.VirtualAddress + security.Size
        if old_end % 8:
            old_end += 8 - old_end % 8
        data = data[:security.VirtualAddress]

    # Align PE body to 8 bytes
    while len(data) % 8:
        data.append(0)

    # Build WIN_CERTIFICATE
    cert_header = struct.pack("<IHH", len(pkcs7_der) + 8,
                              WIN_CERT_REVISION,
                              WIN_CERT_TYPE_PKCS_SIGNED_DATA)
    cert_blob = cert_header + pkcs7_der
    while len(cert_blob) % 8:
        cert_blob += b"\x00"

    cert_va = len(data)
    data += cert_blob

    # Update DataDirectory
    struct.pack_into("<I", data, cert_off, cert_va)
    struct.pack_into("<I", data, cert_off + 4, len(cert_blob))

    with open(output_path, "wb") as f:
        f.write(data)


# ── Verification ──────────────────────────────────────────────────────

def verify_pe_signature(exe_path: str) -> bool:
    try:
        pe = pefile.PE(exe_path)
        sec = pe.OPTIONAL_HEADER.DATA_DIRECTORY[
            pefile.DIRECTORY_ENTRY["IMAGE_DIRECTORY_ENTRY_SECURITY"]
        ]
        if sec.VirtualAddress == 0:
            print("  [VERIFY] No signature present")
            return False

        with open(exe_path, "rb") as f:
            f.seek(sec.VirtualAddress)
            dwLen = struct.unpack("<I", f.read(4))[0]
            _ = f.read(4)
            p7 = f.read(dwLen - 8)

        print(f"  [VERIFY] PKCS#7 size: {len(p7)} bytes")
        certs = serialization.pkcs7.load_der_pkcs7_certificates(p7)
        print(f"  [VERIFY] Certificates: {len(certs)}")
        for i, c in enumerate(certs):
            cn = c.subject.get_attributes_for_oid(NameOID.COMMON_NAME)
            cn_str = cn[0].value if cn else "(no CN)"
            print(f"    [{i}] {cn_str}")
        return len(certs) > 0
    except Exception as e:
        print(f"  [VERIFY] Error: {e}")
        return False


# ── Pure Python fallback ──────────────────────────────────────────────

def compute_pure_python_scaffold(exe_path: str) -> Tuple[bytes, bytes, bytes]:
    """Compute Authenticode digest and build auth attrs in pure Python.
    Returns (file_digest, spc_content_der, auth_attrs_der).
    Only used when osslsigncode is not available.
    """
    pe = pefile.PE(exe_path)
    with open(exe_path, "rb") as f:
        data = bytearray(f.read())

    cert_off = _cert_table_offset(pe)

    # Zero checksum + cert table + existing cert data
    for i in range(0x58, 0x58 + 4):
        if i < len(data): data[i] = 0
    for i in range(cert_off, cert_off + 8):
        if i < len(data): data[i] = 0
    sec = pe.OPTIONAL_HEADER.DATA_DIRECTORY[
        pefile.DIRECTORY_ENTRY["IMAGE_DIRECTORY_ENTRY_SECURITY"]
    ]
    if sec.VirtualAddress and sec.Size:
        for i in range(sec.VirtualAddress,
                       min(sec.VirtualAddress + sec.Size, len(data))):
            data[i] = 0

    file_digest = hashlib.sha256(data).digest()

    # SpcIndirectDataContent
    spc_attr_type = der_sequence(der_oid(OID_SPC_INDIRECT_DATA))
    digest_info = der_sequence(
        der_sequence(der_oid(OID_SHA256) + der_null()) +
        der_octet_string(file_digest)
    )
    spc_content = der_sequence(spc_attr_type + digest_info)

    # Authenticated attributes
    now = datetime.now(timezone.utc)

    attr_ct = der_sequence(
        der_oid(OID_CONTENT_TYPE) +
        der_set(der_oid(OID_SPC_INDIRECT_DATA))
    )
    attr_md = der_sequence(
        der_oid(OID_MESSAGE_DIGEST) +
        der_set(der_octet_string(file_digest))
    )
    attr_st = der_sequence(
        der_oid(OID_SIGNING_TIME) +
        der_set(der_utc_time(now))
    )
    # SpcStatementType (required by Microsoft)
    attr_sst = der_sequence(
        _der_oid_1_3_6_1_4_1_311_2_1_11() +
        der_set(der_sequence(der_oid(OID_SPC_INDIRECT_DATA)))
    )

    attrs_der = der_set(attr_ct + attr_md + attr_st + attr_sst)
    attrs_tagged = der_context(0, attrs_der)

    return file_digest, spc_content, attrs_tagged


def _der_oid_1_3_6_1_4_1_311_2_1_11() -> bytes:
    """der_oid for 1.3.6.1.4.1.311.2.1.11 (SpcStatementType)"""
    # 1.3.6.1.4.1.311.2.1.11
    # 40*1+3=43=0x2B, 6, 1, 4, 1, 311, 2, 1, 11
    # 311 = 0x137 = 2*128+55 = 0x82 0x37
    return bytes([0x06, 0x0A, 0x2B, 0x06, 0x01, 0x04, 0x01, 0x82, 0x37, 0x02, 0x01, 0x0B])


# ── Main ──────────────────────────────────────────────────────────────

def main():
    parser = argparse.ArgumentParser(
        description="Authenticode sign a Windows PE file using Azure KeyVault"
    )
    parser.add_argument("--exe", required=True, help="Path to .exe file to sign")
    parser.add_argument("--output", help="Output path (default: overwrite input)")
    parser.add_argument("--vault-url", required=True, help="Azure KeyVault URL")
    parser.add_argument("--cert-name", required=True, help="Certificate name in KeyVault")
    parser.add_argument("--key-name", required=True, help="Key name in KeyVault")
    parser.add_argument("--tenant-id", required=True, help="Azure AD tenant ID")
    parser.add_argument("--client-id", required=True, help="Azure AD client ID")
    parser.add_argument("--client-secret", required=True, help="Azure AD client secret")
    parser.add_argument("--osslsigncode", help="Path to osslsigncode binary")
    parser.add_argument("--dry-run", action="store_true", help="Skip Azure call")
    parser.add_argument("--verbose", "-v", action="store_true")
    args = parser.parse_args()

    exe_path = os.path.abspath(args.exe)
    if not os.path.exists(exe_path):
        sys.exit(f"ERROR: File not found: {exe_path}")

    output_path = args.output or exe_path
    osslsigncode_bin = args.osslsigncode or find_osslsigncode()

    print(f"=== Signing: {exe_path} ===")

    # ── Step 1: Build Authenticode scaffold ─────────────────────────
    if osslsigncode_bin:
        print(f"[1/5] Building Authenticode scaffold via osslsigncode...")
        try:
            pkcs7_scaffold, _, _ = _run_osslsigncode_scaffold(osslsigncode_bin, exe_path)
            auth_attrs_der, spc_content_der = _extract_auth_attrs_and_spc(pkcs7_scaffold)
            if args.verbose:
                print(f"  PKCS#7 scaffold: {len(pkcs7_scaffold)} bytes")
                print(f"  Auth attrs DER: {len(auth_attrs_der)} bytes")
                print(f"  SpcContent DER: {len(spc_content_der)} bytes")
            scaffold_mode = "osslsigncode"
        except Exception as e:
            print(f"  WARNING: osslsigncode failed: {e}")
            print("  Falling back to pure Python...")
            osslsigncode_bin = None  # trigger fallback

    if not osslsigncode_bin:
        print("[1/5] Computing Authenticode digest (pure Python)...")
        file_digest, spc_content_der, auth_attrs_der = compute_pure_python_scaffold(exe_path)
        if args.verbose:
            print(f"  File digest: {file_digest.hex()}")
            print(f"  SpcContent DER: {len(spc_content_der)} bytes")
            print(f"  Auth attrs DER: {len(auth_attrs_der)} bytes")
        scaffold_mode = "pure"

    # ── Step 2: Compute auth attrs digest ────────────────────────────
    print("[2/5] Computing authenticated attributes digest...")
    auth_digest = hashlib.sha256(auth_attrs_der).digest()
    if args.verbose:
        print(f"  SHA256(auth_attrs) = {auth_digest.hex()}")

    if args.dry_run:
        print("\n=== DRY RUN COMPLETE ===")
        print(f"  Auth attrs DER:  {len(auth_attrs_der)} bytes")
        print(f"  Auth digest:     {auth_digest.hex()}")
        print("  Skipping Azure KeyVault call.")
        return

    # ── Step 3: Azure KeyVault sign ──────────────────────────────────
    print("[3/5] Signing with Azure KeyVault...")
    client = AzureKeyVaultClient(
        args.vault_url, args.tenant_id, args.client_id, args.client_secret
    )
    signature = client.sign(args.key_name, auth_attrs_der, algo="RS256")
    print(f"  Signature obtained ({len(signature)} bytes)")

    # ── Step 4: Fetch certificate chain ──────────────────────────────
    print("[4/5] Fetching certificate from Azure KeyVault...")
    leaf_cert, certs_der_list = client.get_certificates(args.cert_name)
    cn = leaf_cert.subject.get_attributes_for_oid(NameOID.COMMON_NAME)
    cn_str = cn[0].value if cn else "N/A"
    print(f"  Leaf cert: {cn_str}")
    print(f"  Chain length: {len(certs_der_list)}")
    if args.verbose:
        for i, cd in enumerate(certs_der_list):
            c = x509.load_der_x509_certificate(cd)
            c_cn = c.subject.get_attributes_for_oid(NameOID.COMMON_NAME)
            print(f"    [{i}] {(c_cn[0].value if c_cn else b'?').decode()}")

    # ── Step 5: Build final PKCS#7 and embed ─────────────────────────
    print("[5/5] Building Authenticode signature and embedding...")
    # If using osslsigncode, spc_content_der may contain [0] EXPLICIT wrapper
    # The build function handles both cases
    final_pkcs7 = build_pkcs7_signed_data(
        spc_content_der, auth_attrs_der, certs_der_list, signature
    )
    print(f"  Final PKCS#7: {len(final_pkcs7)} bytes")

    embed_pkcs7_in_pe(exe_path, final_pkcs7, output_path)
    print(f"  Signed PE → {output_path}")

    # ── Verify ────────────────────────────────────────────────────────
    print("\n=== Verification ===")
    verify_pe_signature(output_path)
    print("\n=== Done ===")


if __name__ == "__main__":
    main()
