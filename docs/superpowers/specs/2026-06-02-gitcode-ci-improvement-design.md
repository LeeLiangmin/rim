# GitCode CI 流水线改进设计

Date: 2026-06-02
Status: approved

---

## 1. 背景与目标

GitCode 平台的 EulerOS runner 上配置 CI 流水线，交叉编译 `custom-rust-dist` 项目的全部平台 target，构建产物上传至 GitCode Release。

**现有问题:**
- `linux_build.yml` 和 `upload.yml` 功能重复，仅覆盖 Linux CLI 构建
- `release.yml` 是骨架，大量 TODO，不可用
- 硬编码 token 分散在多个文件
- 缺少 Windows 平台构建
- 没有 GUI 构建
- 缺少统一配置管理

**设计目标:**
- 覆盖全部目标平台（Linux x86_64, Linux aarch64, Windows）
- CLI + GUI 产物（Windows 暂仅 CLI）
- 脚本与 CI 解耦，可独立测试
- 统一配置源（`.env`）
- 两套流水线：PR 校验 / Release 构建+上传

---

## 2. 文件结构

```
.gitcode/
├── workflows/
│   ├── .env                         # [新增] 统一配置源
│   ├── ci.yml                      # [新增] PR/push 构建校验
│   ├── release.yml                 # [重写] tag/manual 构建+上传
│   ├── release.py                  # [保留-小改] 读取 .env 配置
│   ├── scripts/
│   │   ├── euleros-install-deps.sh # [新增] EulerOS 依赖安装
│   │   ├── build-target.sh         # [新增] 通用构建脚本
│   │   └── upload-gitcode-release.sh # [新增] 产物上传
│   └── example/                    # [不动]
├── linux_build.yml                 # [删除] → 合并到 ci.yml/release.yml
├── upload.yml                      # [删除] → 合并到 release.yml
├── 签名服务.md                      # [不动]
└── task.md                         # [不动]

ci/
├── scripts/                        # [不动] 已有 GitHub 脚本
│   ├── build-in-docker.sh
│   ├── install-tauri-deps.sh
│   └── upload-release.sh
└── docker/                         # [不动] Docker 构建镜像
    ├── dist-x86-64-linux/Dockerfile
    └── dist-aarch64-linux/Dockerfile
```

---

## 3. 构建目标矩阵

3 个并行 job，每个 job 负责一个平台：

| Job Name | Runner | Build Target | Dist Targets | CLI 方式 | GUI 方式 |
|----------|--------|-------------|--------------|----------|----------|
| `linux-x86_64` | `euleros-latest` | `x86_64-unknown-linux-musl` | `x86_64-unknown-linux-gnu` | cargo build (musl) | Docker `dist-x86-64-linux` |
| `linux-aarch64` | `euleros-aarch64-latest` | `aarch64-unknown-linux-musl` | `aarch64-unknown-linux-gnu` | cargo build (musl) | Docker `dist-aarch64-linux` |
| `windows-gnu` | `euleros-latest` | `x86_64-pc-windows-gnu` | `x86_64-pc-windows-gnu, x86_64-pc-windows-msvc` | cargo build (mingw-w64 交叉编译) | 暂跳过 |

**说明:**
- Linux CLI：musl 静态编译，产物无 glibc 依赖
- Linux GUI：Docker 容器内构建，对接已有 `ci/docker/Dockerfile`
- Windows：仅 CLI（Tauri GUI 交叉编译在 Linux 上不可行），离线包同时覆盖 gnu 和 msvc 工具链
- 后续改进项：Windows GUI 交叉编译

---

## 4. 触发策略

### ci.yml — 构建校验

```
on: push, pull_request
branches: ["main", "llm"]

jobs:
  linux-x86_64 (并行)
  linux-aarch64 (并行)
  windows-gnu  (并行)

每个 job 流程:
  checkout → install deps → build CLI → build GUI(Docker) → validate → 结束（不上传）
```

### release.yml — 构建+上传

```
on:
  push:
    tags: ["v*.*.*"]
  workflow_dispatch:              # 手动触发，走 dev tag

jobs:
  linux-x86_64 (并行)
    → build → upload artifact (管道制品)
  linux-aarch64 (并行)
    → build → upload artifact
  windows-gnu (并行)
    → build → upload artifact

  publish_release (依赖上面3个)
    → download 所有 artifact → release.py → 上传到 GitCode Release
```

---

## 5. 统一配置 .env

所有 workflow 和脚本的唯一配置源，变更只需改一个文件。

```
# -- 项目信息 --
EDITION=community
OWNER=xuanwu
REPO=custom-rust-dist

# -- GitCode API --
GITCODE_TOKEN=<token>
GITCODE_BASE_URL=https://api.gitcode.com/api/v5
DEFAULT_RELEASE_TAG=dev

# -- Rust 镜像 --
RUSTUP_DIST_SERVER=https://mirror.xuanwu.openatom.cn
RUSTUP_UPDATE_ROOT=https://mirror.xuanwu.openatom.cn/rustup
CARGO_REGISTRY=sparse+https://mirror.xuanwu.openatom.cn/index/

# -- Linux x86_64 --
BUILD_TARGET_X86_64=x86_64-unknown-linux-musl
DIST_TARGETS_X86_64=x86_64-unknown-linux-gnu
DOCKER_IMAGE_X86_64=dist-x86-64-linux

# -- Linux aarch64 --
BUILD_TARGET_AARCH64=aarch64-unknown-linux-musl
DIST_TARGETS_AARCH64=aarch64-unknown-linux-gnu
DOCKER_IMAGE_AARCH64=dist-aarch64-linux

# -- Windows (mingw 交叉编译) --
BUILD_TARGET_WINDOWS=x86_64-pc-windows-gnu
DIST_TARGETS_WINDOWS=x86_64-pc-windows-gnu,x86_64-pc-windows-msvc
SKIP_GUI_WINDOWS=true

# -- 网络 --
GIT_HTTP_LOW_SPEED_LIMIT=1000
GIT_HTTP_LOW_SPEED_TIME=30
```

脚本头部统一加载：`set -a; source .gitcode/workflows/.env; set +a`

---

## 6. 脚本设计

### euleros-install-deps.sh

接收参数指定需要安装的依赖组：

```
Usage: euleros-install-deps.sh [--linux-gui] [--windows-cross]

流程:
1. 检测 yum/dnf → EulerOS 走 yum 安装
2. 基础依赖：curl wget gcc gcc-c++ make perl openssl-devel pkg-config git
3. --linux-gui: 加装 gtk3-devel webkit2gtk4.0-devel librsvg2-devel libappindicator-gtk3-devel
4. --windows-cross: 加装 mingw64-gcc mingw64-headers mingw64-winpthreads
5. 安装 Node.js 20.x + pnpm
6. 安装 Rust via rustup，添加对应 target
7. 配置 cargo mirror → .cargo/config.toml
8. 安装 python3 httpx（release 模式用）
```

### build-target.sh

封装单个 target 的构建逻辑：

```
Usage: build-target.sh --build-target <triple> --dist-targets <triple,...>
                       [--skip-gui] [--binary-only]

流程:
1. source .gitcode/.env
2. 排除 rim_gui workspace member（避免网络慢）
3. cargo dev vendor --for <dist-targets>
4. cargo dev dist --cli -b --target <build-target> --for <dist-targets>
5. 若 --skip-gui 未设置:
   docker build + run → cargo dev dist --gui
6. 验证产物:
   - dist/<dist-target>/*-installer-cli*
   - dist/<dist-target>/*.tar.xz (Linux) / *.zip (Windows)
   - [可选] dist/<dist-target>/*-installer* (GUI)
```

### upload-gitcode-release.sh

```
usage: upload-gitcode-release.sh --tag <tag> --dist-dir <path>

流程:
1. source .gitcode/.env
2. find <dist-dir> -type f → 收集产物文件列表
3. python3 .gitcode/workflows/release.py --tag <tag> --files <files>
4. release.py 通过 env 读取 GITCODE_TOKEN / GITCODE_OWNER / GITCODE_REPO
```

---

## 7. 错误处理

| 场景 | 策略 |
|------|------|
| 网络超时 | `GIT_HTTP_LOW_SPEED_LIMIT` / `GIT_HTTP_LOW_SPEED_TIME` + release.py 内 30s httpx timeout |
| 依赖安装失败 | `set -e` 终止 job |
| 单个 target 构建失败 | 独立 job 隔离，不影响其他 job |
| Docker 不可用 | 降级跳过 GUI，只构建 CLI；Warning 输出 |
| 产物缺失 | job 末尾 `test -d dist/<target>` + `test -n <find>` 检查 |
| Release 已存在 | release.py `create_release` 自动 fallback 到 `get_release` |
| 上传单文件失败 | 逐文件上传，失败不阻塞剩余文件，最终汇总 |
| Release 上传阶段任一 job 失败 | `publish_release` needs 条件不满足，整体不发布 |

---

## 8. 安全说明

- **Token 硬编码**: GitCode 不支持 CI secrets 变量，`GITCODE_TOKEN` 存储在 `.gitcode/.env` 中
- **防护措施**: `.gitcode/.gitignore` 中添加 `.env` 防止意外提交到仓库外；Token 建议仅授予 release 读写权限，定期更换
- **后续改进**: 若 GitCode 后续支持 secrets，将 Token 迁移至 CI 变量注入

---

## 9. 已知限制与后续改进

| 项目 | 说明 | 优先级 |
|------|------|--------|
| Windows GUI 构建 | EulerOS 上无法交叉编译 Tauri GUI，需原生 Windows runner 或寻找替代方案 | 中 |
| Windows MSVC 构建 | 当前仅构建 GNU target 产物，MSVC install 工具链通过离线包覆盖 | 低 |
| 签名集成 | `.gitcode/签名服务.md` 已记录签名方案，暂未集成到 CI | 低 |
| aarch64 Docker | `ci/docker/dist-aarch64-linux/Dockerfile` 依赖 qemu 模拟，需确认 EulerOS aarch64 runner 原生支持 | 中 |
