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

while [[ $# -gt 0 ]]; do
    case "$1" in
        --linux-gui) INSTALL_GUI=true ;;
        --windows-cross) INSTALL_WIN_CROSS=true ;;
        -h|--help) usage ;;
        *) echo "Unknown option: $1"; usage ;;
    esac
    shift
done

# Detect package manager and OS family
install_pkg() {
    if command -v apt-get >/dev/null 2>&1; then
        apt-get install -y "$@" 2>/dev/null || true
    elif command -v yum >/dev/null 2>&1; then
        yum install -y "$@" 2>/dev/null || true
    elif command -v dnf >/dev/null 2>&1; then
        dnf install -y "$@" 2>/dev/null || true
    else
        echo "WARNING: no supported package manager found, skipping: $*"
    fi
}

echo "=== Detecting OS ==="
if [ -f /etc/os-release ]; then
    . /etc/os-release
    echo "  ID=$ID, VERSION=$VERSION_ID"
fi

echo "=== Installing base dependencies ==="
if command -v apt-get >/dev/null 2>&1; then
    apt-get update -y 2>/dev/null || true
    install_pkg curl wget file gcc g++ make perl pkg-config git libssl-dev
elif command -v yum >/dev/null 2>&1 || command -v dnf >/dev/null 2>&1; then
    install_pkg curl wget file gcc gcc-c++ make perl openssl-devel pkg-config git
fi

if $INSTALL_GUI; then
    echo "=== Installing GUI (Tauri) dependencies ==="
    if command -v apt-get >/dev/null 2>&1; then
        install_pkg libwebkit2gtk-4.0-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev
    else
        install_pkg gtk3-devel webkit2gtk4.0-devel libappindicator-gtk3-devel librsvg2-devel
    fi
fi

if $INSTALL_WIN_CROSS; then
    echo "=== Installing Windows cross-compilation toolchain ==="
    if command -v apt-get >/dev/null 2>&1; then
        install_pkg mingw-w64 gcc-mingw-w64-x86-64
    else
        install_pkg mingw64-gcc mingw64-headers mingw64-winpthreads mingw64-crt
    fi
fi

echo "=== Installing musl tools ==="
if command -v apt-get >/dev/null 2>&1; then
    apt-get install -y musl-tools 2>/dev/null || echo "  musl-tools not available (will fall back to gnu target)"
    if command -v musl-gcc >/dev/null 2>&1 && ! command -v x86_64-linux-musl-gcc >/dev/null 2>&1; then
        ln -sf "$(which musl-gcc)" /usr/local/bin/x86_64-linux-musl-gcc
    fi
else
    dnf install -y musl-gcc 2>/dev/null || yum install -y musl-gcc 2>/dev/null || echo "  musl-gcc not available (will fall back to gnu target)"
fi
echo "  musl-gcc: $(command -v musl-gcc || echo 'not found')"

echo "=== Installing Node.js ==="
if ! command -v node >/dev/null 2>&1; then
    if command -v apt-get >/dev/null 2>&1; then
        curl -fsSL https://deb.nodesource.com/setup_20.x | bash -
        apt-get install -y nodejs
    else
        curl -fsSL https://rpm.nodesource.com/setup_20.x | bash -
        install_pkg nodejs
    fi
fi
node --version

echo "=== Installing pnpm ==="
if ! command -v pnpm >/dev/null 2>&1; then
    # pnpm 9.x supports Node.js 18+, pnpm 10+ requires Node.js 22+
    npm install -g pnpm@9
    export PATH="$(npm prefix -g)/bin:$PATH"
fi
pnpm --version

echo "=== Installing Rust (via xuanwu mirror) ==="
export RUSTUP_DIST_SERVER="${RUSTUP_DIST_SERVER:-https://mirror.xuanwu.openatom.cn}"
export RUSTUP_UPDATE_ROOT="${RUSTUP_UPDATE_ROOT:-https://mirror.xuanwu.openatom.cn/rustup}"
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
[net]
git-fetch-with-cli = true

[source.crates-io]
replace-with = 'xuanwu-sparse'

[source.xuanwu]
registry = "https://mirror.xuanwu.openatom.cn/crates.io-index"

[source.xuanwu-sparse]
registry = "sparse+https://mirror.xuanwu.openatom.cn/index/"

[registries.xuanwu]
index = "https://mirror.xuanwu.openatom.cn/crates.io-index"
CARGOEOF

echo "=== Installing Python httpx for release script ==="
python3 -c "import httpx" 2>/dev/null || pip3 install --quiet httpx || true

echo "=== Dependencies installed successfully ==="
