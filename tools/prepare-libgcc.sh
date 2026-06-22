#!/usr/bin/env bash
# prepare-libgcc.sh — Extract libgcc_eh.a / libgcc.a from the local Rust
# rust-mingw component and package them for upload to OBS.
#
# Background:
#   The OBS-hosted mingw-w64-cross.tar.xz is a stripped llvm-mingw that omits
#   x86_64-w64-mingw32/lib/{libgcc.a, libgcc_eh.a}.  Rust's windows-gnu target
#   spec hardcodes -lgcc_eh -lgcc; the _Unwind_* symbols are referenced by
#   libpanic_unwind, libstd's personality routine, and user crates.  Empty
#   stubs cause undefined-symbol linker errors.
#
#   The cleanest source of these archives is the Rust toolchain's own
#   rust-mingw component (only available on Windows hosts).  It ships
#   libgcc_eh.a / libgcc.a built by the Rust project for the exact toolchain
#   version, guaranteeing ABI compatibility with libpanic_unwind.rlib and
#   libstd.rlib from the same version.
#
#   These are target-side COFF static archives (Windows object files) and do
#   NOT depend on host glibc, so they can be dropped into any Linux-hosted
#   mingw toolchain's lib directory.
#
# Prerequisites:
#   - Rust toolchain <version>-x86_64-pc-windows-gnu with rust-mingw component
#     installed (via: rustup toolchain install <ver>-x86_64-pc-windows-gnu
#     --component rust-mingw)
#
# Output:
#   mingw-libgcc-x86_64.tar.gz
#     └── x86_64-w64-mingw32/lib/
#           ├── libgcc.a
#           └── libgcc_eh.a
#
# Usage:
#   bash tools/prepare-libgcc.sh [rust-toolchain-version]
#   (default version: 1.85.0, matching the CI toolchain)
#
# Then upload mingw-libgcc-x86_64.tar.gz to OBS at:
#   https://xuanwu-rust.obs.cn-north-4.myhuaweicloud.com/dist/mingw-libgcc-x86_64.tar.gz

set -euo pipefail

# ── Config ───────────────────────────────────────────────────────────
RUST_VERSION="${1:-1.85.0}"
TARGET_TRIPLE="x86_64-pc-windows-gnu"
MINGW_TRIPLE="x86_64-w64-mingw32"
LIB_SUBDIR="${MINGW_TRIPLE}/lib"
OUT_TGZ="mingw-libgcc-x86_64.tar.gz"

# ── Helpers ──────────────────────────────────────────────────────────
log()  { printf '  %s\n' "$*"; }
err()  { printf 'ERROR: %s\n' "$*" >&2; }
die()  { err "$*"; exit 1; }

# ── Locate the rust-mingw self-contained directory ───────────────────
RUSTUP_HOME="${RUSTUP_HOME:-$HOME/.rustup}"
TOOLCHAIN_DIR="$RUSTUP_HOME/toolchains/${RUST_VERSION}-${TARGET_TRIPLE}"
SELF_CONTAINED="$TOOLCHAIN_DIR/lib/rustlib/${TARGET_TRIPLE}/lib/self-contained"

if [ ! -d "$SELF_CONTAINED" ]; then
    err "rust-mingw self-contained directory not found:"
    err "  $SELF_CONTAINED"
    err ""
    err "Install it with:"
    err "  rustup toolchain install ${RUST_VERSION}-${TARGET_TRIPLE} --component rust-mingw"
    die "cannot proceed without rust-mingw component"
fi

log "Rust toolchain: ${RUST_VERSION}-${TARGET_TRIPLE}"
log "self-contained:  $SELF_CONTAINED"
echo

# ── Verify required archives exist ───────────────────────────────────
GCC_EH="$SELF_CONTAINED/libgcc_eh.a"
GCC="$SELF_CONTAINED/libgcc.a"

[ -f "$GCC_EH" ] || die "libgcc_eh.a not found in self-contained dir"
[ -f "$GCC" ]    || die "libgcc.a not found in self-contained dir"

log "libgcc_eh.a: $(stat -c%s "$GCC_EH") bytes"
log "libgcc.a:    $(stat -c%s "$GCC") bytes"
echo

# ── Verify _Unwind_* symbols in libgcc_eh.a ──────────────────────────
log "Verifying _Unwind_* symbols in libgcc_eh.a..."

# Try nm variants (llvm-nm preferred for COFF archives)
# GNU nm on Linux cannot read COFF archives (Windows object files).
# Use grep -a (treat binary as text) to verify symbol names are present
# in the archive's symbol table. This is reliable for COFF archives
# where symbol names are stored as null-terminated ASCII strings.
REQUIRED_SYMS=(
    "_Unwind_Resume"
    "_Unwind_RaiseException"
    "_Unwind_DeleteException"
    "_Unwind_GetLanguageSpecificData"
    "_Unwind_GetIPInfo"
    "_Unwind_GetRegionStart"
    "_Unwind_GetTextRelBase"
    "_Unwind_GetDataRelBase"
    "_Unwind_SetGR"
    "_Unwind_SetIP"
    "_GCC_specific_handler"
)

MISSING=()
log "  Verifying via grep -a (binary text scan)"
for sym in "${REQUIRED_SYMS[@]}"; do
    if grep -qaF "$sym" "$GCC_EH" 2>/dev/null; then
        printf '    OK   %s\n' "$sym"
    else
        printf '    MISS %s\n' "$sym"
        MISSING+=("$sym")
    fi
done

if [ "${#MISSING[@]}" -gt 0 ]; then
    die "libgcc_eh.a missing required symbols: ${MISSING[*]}"
fi
echo

# ── Stage files into final layout ────────────────────────────────────
WORKDIR="$(mktemp -d)"
trap 'rm -rf "$WORKDIR"' EXIT

STAGE_LIBDIR="$WORKDIR/${LIB_SUBDIR}"
mkdir -p "$STAGE_LIBDIR"

cp "$GCC_EH" "$STAGE_LIBDIR/libgcc_eh.a"
cp "$GCC"    "$STAGE_LIBDIR/libgcc.a"
log "Staged: ${LIB_SUBDIR}/libgcc_eh.a ($(stat -c%s "$GCC_EH") bytes)"
log "Staged: ${LIB_SUBDIR}/libgcc.a ($(stat -c%s "$GCC") bytes)"
echo

# ── Package ──────────────────────────────────────────────────────────
log "Packaging $OUT_TGZ..."
OUT_ABS="$(pwd)/$OUT_TGZ"
tar -czf "$OUT_ABS" -C "$WORKDIR" "${LIB_SUBDIR}"

log "Created: $OUT_ABS ($(stat -c%s "$OUT_ABS") bytes)"
echo

# ── Print upload instructions ────────────────────────────────────────
cat <<EOF

──────────────────────────────────────────────────────────────────────
  SUCCESS — $OUT_TGZ is ready.

  Upload it to OBS:

      https://xuanwu-rust.obs.cn-north-4.myhuaweicloud.com/dist/mingw-libgcc-x86_64.tar.gz

  Source: Rust ${RUST_VERSION} rust-mingw component
  Layout:
      ${LIB_SUBDIR}/
        libgcc.a        ($(stat -c%s "$GCC") bytes)
        libgcc_eh.a     ($(stat -c%s "$GCC_EH") bytes)

  These archives are target-side COFF statics (Windows object files)
  with no host glibc dependency.  The CI build-target.sh script will
  download and extract them into the OBS mingw toolchain's lib dir.
──────────────────────────────────────────────────────────────────────
EOF
