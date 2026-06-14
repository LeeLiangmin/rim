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

echo "=== Installing aarch64 cross-compilation toolchain ==="
if command -v aarch64-linux-gnu-gcc >/dev/null 2>&1; then
    echo "  aarch64-linux-gnu-gcc already available"
elif command -v apt-get >/dev/null 2>&1; then
    apt-get install -y gcc-aarch64-linux-gnu || echo "  WARN: gcc-aarch64-linux-gnu install failed via apt"
else
    # EulerOS/CentOS: try package manager
    dnf install -y gcc-aarch64-linux-gnu 2>/dev/null || \
    yum install -y gcc-aarch64-linux-gnu 2>/dev/null || \
    echo "  WARN: gcc-aarch64-linux-gnu not in repos (will download in build step if needed)"
fi
echo "  aarch64-linux-gnu-gcc: $(command -v aarch64-linux-gnu-gcc || echo 'not found')"

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
rustup toolchain install 1.85.0 --profile minimal
rustup default 1.85.0

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

[target.x86_64-pc-windows-gnu]
linker = "x86_64-w64-mingw32-gcc"
ar = "x86_64-w64-mingw32-ar"
CARGOEOF

echo "=== Installing Python httpx for release script ==="
python3 -c "import httpx" 2>/dev/null || pip3 install --quiet httpx || true

echo "=== Installing LLVM (for llvm-dlltool, used by windows-gnu cross-compile) ==="
install_pkg llvm clang llvm-devel 2>/dev/null || true
if command -v llvm-dlltool >/dev/null 2>&1; then
    echo "  llvm-dlltool: $(command -v llvm-dlltool)"
else
    echo "  llvm-dlltool not in system packages (will download from mirror if needed)"
fi

# Docker binary download (experimental: current CI K8s Pod has no Docker and
# cannot start dockerd. Kept for future environments that support it.)
if $INSTALL_WIN_CROSS; then
    echo "=== Installing Docker static binary (for windows-gnu cross-rs build) ==="
    if ! command -v docker >/dev/null 2>&1; then
        DOCKER_VERSION="${DOCKER_VERSION:-24.0.9}"
        DOCKER_URLS="
            https://mirrors.huaweicloud.com/docker-ce/linux/static/stable/x86_64/docker-${DOCKER_VERSION}.tgz
            https://download.docker.com/linux/static/stable/x86_64/docker-${DOCKER_VERSION}.tgz
        "
        DOCKER_OK=false
        for url in $DOCKER_URLS; do
            echo "  Trying: $url"
            rm -f /tmp/docker.tgz
            curl -sSL --connect-timeout 10 --max-time 180 "$url" -o /tmp/docker.tgz 2>/dev/null || true
            if [ -f /tmp/docker.tgz ] && file /tmp/docker.tgz 2>/dev/null | grep -q 'gzip'; then
                if tar xzf /tmp/docker.tgz -C /usr/local/bin --strip-components=1 2>/dev/null; then
                    rm -f /tmp/docker.tgz
                    DOCKER_OK=true
                    break
                fi
            fi
            echo "  (failed, trying next)"
        done
        rm -f /tmp/docker.tgz
        if $DOCKER_OK; then
            echo "  Docker $(docker --version 2>&1 || true) installed"
        else
            echo "  WARNING: failed to download Docker static binary (will fall back to native build)"
        fi
    else
        echo "  Docker already installed: $(docker --version 2>&1 || true)"
    fi
fi

echo "=== Dependencies installed successfully ==="
