#!/bin/bash
set -euo pipefail

usage() {
    echo "Usage: $0 [--linux-gui] [--windows-cross]"
    echo "  --linux-gui       Install Tauri GUI build dependencies"
    echo "  --windows-cross   Install mingw-w64 cross-compilation toolchain"
    exit 1
}

INSTALL_GUI=false
INSTALL_WIN_CROSS=false

# Use sudo only if not already root
SUDO=""
if [[ "$(id -u)" -ne 0 ]]; then
    SUDO="sudo"
fi

while [[ $# -gt 0 ]]; do
    case "$1" in
        --linux-gui) INSTALL_GUI=true ;;
        --windows-cross) INSTALL_WIN_CROSS=true ;;
        -h|--help) usage ;;
        *) echo "Unknown option: $1"; usage ;;
    esac
    shift
done

install_yum() {
    local pkgs=("$@")
    $SUDO yum install -y "${pkgs[@]}" || {
        echo "yum install failed, trying dnf..."
        $SUDO dnf install -y "${pkgs[@]}"
    }
}

echo "=== Installing base dependencies ==="
if command -v yum >/dev/null 2>&1; then
    install_yum curl wget file gcc gcc-c++ make perl openssl-devel pkg-config git musl-gcc
elif command -v dnf >/dev/null 2>&1; then
    $SUDO dnf install -y curl wget file gcc gcc-c++ make perl openssl-devel pkg-config git musl-gcc
else
    echo "ERROR: no supported package manager found"
    exit 1
fi

if $INSTALL_GUI; then
    echo "=== Installing GUI (Tauri) dependencies ==="
    install_yum gtk3-devel webkit2gtk4.0-devel libappindicator-gtk3-devel librsvg2-devel
fi

if $INSTALL_WIN_CROSS; then
    echo "=== Installing Windows cross-compilation toolchain ==="
    install_yum mingw64-gcc mingw64-headers mingw64-winpthreads mingw64-crt
fi

echo "=== Installing Node.js 20.x ==="
if ! command -v node >/dev/null 2>&1; then
    curl -fsSL https://rpm.nodesource.com/setup_20.x | $SUDO bash -
    install_yum nodejs
fi
node --version

echo "=== Installing pnpm ==="
if ! command -v pnpm >/dev/null 2>&1; then
    npm install -g pnpm
fi
pnpm --version

echo "=== Installing Rust ==="
if ! command -v cargo >/dev/null 2>&1; then
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal
fi
source "$HOME/.cargo/env"
rustup toolchain install stable --profile minimal
rustup default stable

echo "=== Installing Rust cross-compilation targets ==="
rustup target add x86_64-unknown-linux-musl aarch64-unknown-linux-gnu x86_64-pc-windows-gnu || true
rustc --version
cargo --version

echo "=== Configuring cargo mirror ==="
mkdir -p .cargo
cat > .cargo/config.toml <<CARGOEOF
[source.crates-io]
replace-with = 'xuanwu-sparse'

[source.xuanwu]
registry = "https://mirror.xuanwu.openatom.cn/crates.io-index"

[source.xuanwu-sparse]
registry = "sparse+https://mirror.xuanwu.openatom.cn/index/"

[registries.xuanwu]
index = "https://mirror.xuanwu.openatom.cn/crates.io-index"

[net]
git-fetch-with-cli = true
CARGOEOF

echo "=== Installing Python httpx for release script ==="
python3 -c "import httpx" 2>/dev/null || pip3 install --quiet httpx

echo "=== Dependencies installed successfully ==="
