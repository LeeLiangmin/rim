# R.I.M (Rust Installation Manager) 项目结构文档

## 目录

1. [项目概述](#项目概述)
2. [项目结构](#项目结构)
3. [配置文件说明](#配置文件说明)
4. [安装流程](#安装流程)
5. [卸载流程](#卸载流程)
6. [核心模块说明](#核心模块说明)

---

## 项目概述

**R.I.M (Rust Installation Manager)** 是一个用于安装和管理 Rust 工具链及第三方工具的交互式程序，支持图形用户界面（GUI）和命令行界面（CLI）。

### 主要特性

- 支持安装 Rust 工具链（通过 rustup）
- 支持安装第三方工具（VS Code、MSVC Build Tools、MinGW 等）
- 支持工具包（Toolkit）的升级和降级
- 支持组件管理（安装/卸载单个组件）
- 支持离线安装包
- 双模式运行：Installer（安装器）和 Manager（管理器）

---

## 项目结构

```
custom-rust-dist/
├── .cargo/                    # Cargo 配置目录
│   └── config.toml           # Cargo 构建配置（链接器设置等）
├── ci/                        # CI/CD 相关
│   ├── docker/                # Docker 构建文件
│   └── scripts/               # 构建和部署脚本
├── dist/                      # 构建输出目录
│   └── {target}/              # 各平台的构建产物
├── locales/                   # 国际化文件
│   ├── en-US.json            # 英文翻译
│   └── zh-CN.json            # 中文翻译
├── resources/                 # 资源文件
│   ├── example/               # 示例项目
│   ├── images/               # 演示图片
│   ├── packages/              # 离线安装包
│   ├── templates/             # 环境变量模板
│   └── toolkit-manifest/      # 工具包清单模板
├── rim_common/                # 公共库
│   └── src/
│       ├── dirs.rs           # 目录路径管理
│       ├── types/             # 类型定义
│       │   ├── toolkit_manifest.rs  # 工具包清单类型
│       │   ├── tool_info.rs         # 工具信息类型
│       │   └── ...
│       └── utils/             # 工具函数
│           ├── download.rs   # 下载功能
│           ├── extraction.rs # 解压功能
│           ├── file_system.rs # 文件系统操作
│           └── ...
├── rim_dev/                   # 开发工具
│   └── src/
│       ├── dist.rs            # 构建分发包
│       ├── run.rs             # 运行开发环境
│       └── mocked/            # 模拟环境
├── rim_gui/                   # GUI 前端（Tauri）
│   ├── src/                   # Vue 前端代码
│   │   ├── components/        # Vue 组件
│   │   ├── views/             # 视图页面
│   │   └── utils/             # 前端工具函数
│   └── src-tauri/             # Tauri 后端
│       └── src/
│           ├── main.rs        # GUI 入口
│           ├── installer_mode.rs  # 安装器模式
│           └── manager_mode.rs    # 管理器模式
├── rim_test/                  # 测试支持库
│   ├── rim-test-macro/        # 测试宏
│   └── rim-test-support/      # 测试工具
├── src/                       # 主程序源码
│   ├── bin/
│   │   └── rim_cli.rs         # CLI 入口点
│   ├── cli/                   # CLI 命令实现
│   │   ├── install.rs         # 安装命令
│   │   ├── update.rs          # 更新命令
│   │   ├── uninstall.rs       # 卸载命令
│   │   ├── component.rs       # 组件管理命令
│   │   └── ...
│   └── core/                  # 核心功能
│       ├── install.rs         # 安装逻辑
│       ├── update.rs          # 更新逻辑
│       ├── uninstall.rs       # 卸载逻辑
│       ├── tools.rs           # 工具安装管理
│       ├── rustup.rs          # Rustup 集成
│       ├── components.rs      # 组件管理
│       ├── toolkit.rs         # 工具包管理
│       ├── parser/            # 配置文件解析
│       │   ├── fingerprint.rs # 安装记录解析
│       │   ├── cargo_config.rs # Cargo 配置解析
│       │   └── ...
│       └── os/                 # 操作系统特定功能
│           ├── windows.rs     # Windows 特定实现
│           └── unix.rs         # Unix/Linux 特定实现
├── tests/                     # 测试套件
│   ├── assets/                # 测试资源
│   └── testsuite/             # 测试用例
├── build.rs                   # 构建脚本
├── Cargo.toml                 # 主 Cargo 配置
├── configuration.toml          # 构建时配置
└── README.md                  # 项目说明文档
```

---

## 配置文件说明

### 1. `Cargo.toml` - Rust 项目配置

**位置**: 项目根目录

**作用**: 
- 定义工作空间（workspace）结构
- 管理依赖项
- 配置编译选项

**关键配置**:
```toml
[workspace]
members = ["rim_gui/src-tauri", "rim_dev", "rim_common", "rim_test/*"]

[workspace.package]
version = "0.10.0"
edition = "2021"
rust-version = "1.80.0"

[profile.dev]
opt-level = 0          # 开发模式：无优化，最快编译速度
codegen-units = 256    # 最大化并行编译

[profile.release]
opt-level = 3          # 发布模式：最大优化
codegen-units = 1      # 整体优化
lto = "thin"           # 链接时优化
```

### 2. `.cargo/config.toml` - Cargo 构建配置

**位置**: `.cargo/config.toml`

**作用**: 
- 配置 Cargo 别名（aliases）
- 设置平台特定的链接器

**关键配置**:
```toml
[alias]
dev = "run -p rim_dev --"
devr = "run -p rim_dev -- run"
cdev = "clif run -p rim_dev -- run"

[target.x86_64-pc-windows-msvc]
linker = "rust-lld.exe"  # 使用 rust-lld 作为链接器
```

### 3. `configuration.toml` - 构建时配置

**位置**: 项目根目录

**作用**: 
- 定义默认的下载服务器 URL
- 配置产品标识和本地化信息
- 设置默认的 Cargo registry

**关键配置**:
```toml
identifier = "xuanwu-rust"

# 默认下载源
rustup_dist_server = 'https://mirror.xuanwu.openatom.cn'
rustup_update_root = 'https://mirror.xuanwu.openatom.cn/rustup'
rim_dist_server = 'https://rust-mirror.obs.cn-north-4.myhuaweicloud.com'
registry = { name = 'xuanwu-sparse', index = 'sparse+https://mirror.xuanwu.openatom.cn/index/' }

[locale.zh-CN]
logo_text = "旋武社区"
vendor = "Rust 中国社区"
product = "Rust 中国社区发行版"
app_name = "Rust 安装管理器"
```

**注意**: 此文件在构建时被编译进二进制文件，通过 `rim_common::build_config()` 访问。

### 4. `toolset-manifest.toml` - 工具包清单

**位置**: `resources/toolkit-manifest/` 或安装目录

**作用**: 
- 定义要安装的 Rust 工具链版本和组件
- 定义要安装的第三方工具列表
- 配置工具依赖关系
- 指定下载源和代理设置

**结构示例**:
```toml
name = "XuanWu Rust Development Kit"
version = "1.86.0"
edition = "community"

[rust]
version = "stable"
profile = "default"
components = ["rustc", "cargo", "rustfmt", "clippy"]
optional-components = ["rust-docs", "rust-analyzer"]

[tools.target.x86_64-pc-windows-msvc]
vscode = { url = "https://...", version = "1.85.0" }
buildtools = { path = "./packages/buildtools.exe", required = true }

[tools.descriptions]
vscode = "Visual Studio Code 编辑器"
buildtools = "MSVC 构建工具"

[tools.group]
"开发工具" = ["vscode", "buildtools"]

[proxy]
http = "http://proxy.example.com:8080"
https = "https://proxy.example.com:8080"
no-proxy = "localhost,.example.com"
```

**关键字段**:
- `[rust]`: Rust 工具链配置
- `[tools.target.{target}]`: 平台特定的工具列表
- `[tools.descriptions]`: 工具描述
- `[tools.group]`: 工具分组
- `[proxy]`: 代理配置

### 5. `.fingerprint.toml` - 安装记录

**位置**: `{config_dir}/.fingerprint.toml`（新版本）或安装目录（旧版本）

**作用**: 
- 记录安装的目录路径
- 记录已安装的工具和版本
- 记录已安装的 Rust 工具链组件
- 用于判断是否进入 Manager 模式

**结构**:
```toml
install_dir = "C:\\Users\\{user}\\.rust"

[rust]
channel = "stable"
components = ["rustc", "cargo", "rustfmt"]

[tools]
vscode = { kind = "DirWithBin", paths = ["C:\\Users\\{user}\\.rust\\tools\\vscode"], version = "1.85.0" }
```

### 6. `config.toml` - Cargo 配置

**位置**: `{CARGO_HOME}/config.toml`

**作用**: 
- 配置 Cargo registry 源
- 配置依赖补丁（patches）
- 由 RIM 自动生成和更新

**生成示例**:
```toml
[source.mirror]
registry = "sparse+https://mirror.xuanwu.openatom.cn/index/"

[source.crates-io]
replace-with = "mirror"

[patch.crates-io]
some-crate = { path = "{CARGO_HOME}/crates/some-crate" }
```

### 7. `dist-manifest.toml` - 分发清单

**位置**: 服务器上的 `dist/dist-manifest.toml`

**作用**: 
- 列出所有可用的工具包版本
- 提供工具包的下载链接和元数据

**结构**:
```toml
[[packages]]
name = "XuanWu-Rust-Development-Kit"
version = "1.87.0"
edition = "community"
manifest_url = "https://.../toolset-manifest.toml"
desc = "Rust 开发工具包"
```

### 8. `release.toml` - 管理器发布信息

**位置**: 服务器上的 `manager/release.toml`

**作用**: 
- 记录管理器的最新版本
- 提供更新下载链接

**结构**:
```toml
version = "0.10.0"
```

---

## 安装流程

### 流程图

```
开始
  ↓
检测运行模式 (Mode::detect)
  ↓
┌─────────────────┐
│ Installer 模式  │
└─────────────────┘
  ↓
解析 CLI 参数
  ↓
设置日志和本地化
  ↓
加载工具包清单 (ToolkitManifest)
  ├─ 从 --manifest 参数
  ├─ 从服务器下载
  └─ 使用内置清单
  ↓
用户选择组件
  ↓
创建安装配置 (InstallConfiguration)
  ↓
执行安装 (install)
  │
  ├─ 1. setup() - 初始化安装目录
  │     ├─ 创建安装目录
  │     ├─ 复制工具包清单到安装目录
  │     ├─ 复制管理器二进制文件
  │     ├─ 创建符号链接到 cargo/bin
  │     └─ (Windows) 创建注册表项
  │
  ├─ 2. config_env_vars() - 配置环境变量
  │     ├─ 设置 CARGO_HOME
  │     ├─ 设置 RUSTUP_HOME
  │     ├─ 设置 RUSTUP_DIST_SERVER
  │     ├─ 设置 RUSTUP_UPDATE_ROOT
  │     └─ (Windows) 写入注册表
  │
  ├─ 3. config_cargo() - 配置 Cargo
  │     └─ 写入 config.toml（registry 配置）
  │
  ├─ 4. install_tools() - 安装第三方工具（不依赖 Rust）
  │     ├─ 按依赖关系排序
  │     ├─ 下载工具包（如需要）
  │     ├─ 解压或复制到 tools/ 目录
  │     ├─ 执行自定义安装指令（如 VS Code）
  │     └─ 记录安装信息
  │
  ├─ 5. install_rust() - 安装 Rust 工具链
  │     ├─ 确保 rustup 已安装
  │     │   ├─ 检查是否已存在
  │     │   ├─ 从清单中获取捆绑的 rustup-init
  │     │   └─ 或从服务器下载
  │     ├─ 运行 rustup-init
  │     ├─ 安装工具链和组件
  │     │   └─ rustup toolchain install {version} -c {components}
  │     ├─ 设置为默认工具链
  │     └─ 记录安装信息
  │
  ├─ 6. install_tools_late() - 安装依赖 Rust 的工具
  │     └─ 使用 cargo install 安装工具
  │
  └─ 7. 写入安装记录 (.fingerprint.toml)
  ↓
完成
```

### 详细步骤说明

#### 1. 模式检测 (`src/core/mod.rs`)

程序启动时通过 `Mode::detect()` 判断运行模式：

```rust
// 检测逻辑：
// 1. 检查环境变量 MODE
// 2. 检查程序名是否包含 "installer"
// 3. 检查是否存在安装记录 (.fingerprint.toml)
// 4. 默认进入 Installer 模式
```

#### 2. 加载工具包清单 (`src/core/toolkit_manifest_ext.rs`)

优先级顺序：
1. `--manifest` 参数指定的路径或 URL
2. 安装目录中的 `toolset-manifest.toml`（更新时）
3. 从服务器下载（根据 `dist-manifest.toml`）
4. 内置清单（编译时嵌入）

#### 3. 组件选择 (`src/cli/install.rs`)

用户可以选择：
- Rust 工具链组件（必需和可选）
- 第三方工具（按组分类）

#### 4. 安装目录初始化 (`src/core/install.rs::setup()`)

```rust
// 创建目录结构：
{install_dir}/
├── cargo/          # CARGO_HOME
│   ├── bin/       # 可执行文件目录（添加到 PATH）
│   └── config.toml
├── rustup/        # RUSTUP_HOME
├── tools/         # 第三方工具目录
├── crates/        # 本地 crate 补丁
├── temp/          # 临时文件
└── {app_name}.exe # 管理器二进制文件
```

#### 5. 环境变量配置 (`src/core/install.rs::config_env_vars()`)

**Windows**:
- 通过注册表设置用户环境变量
- 路径：`HKEY_CURRENT_USER\Environment`

**Unix/Linux**:
- 修改 shell 配置文件（`.bashrc`, `.zshrc`, `.fish/config.fish`）
- 添加环境变量设置脚本

#### 6. 工具安装 (`src/core/tools.rs`)

支持的工具类型：
- `CargoTool`: 通过 `cargo install` 安装
- `DirWithBin`: 包含 `bin/` 目录的工具包
- `Executables`: 单个或多个可执行文件
- `Plugin`: VS Code 扩展（.vsix）
- `Installer`: Windows 安装程序
- `Custom`: 自定义安装指令（VS Code、Build Tools 等）
- `Crate`: Rust crate（作为依赖补丁）
- `RuleSet`: Clippy 规则集

#### 7. Rust 工具链安装 (`src/core/rustup.rs`)

```rust
// 步骤：
// 1. 设置 RUSTUP_DIST_SERVER 环境变量
// 2. 确保 rustup 已安装
// 3. 运行: rustup toolchain install {channel} -c {components} --profile {profile}
// 4. 运行: rustup default {channel}
```

#### 8. 安装记录 (`src/core/parser/fingerprint.rs`)

安装完成后写入 `.fingerprint.toml`，包含：
- 安装目录路径
- Rust 工具链信息
- 已安装工具列表及版本

---

## 卸载流程

### 流程图

```
开始
  ↓
检测运行模式 → Manager 模式
  ↓
解析卸载命令
  ├─ uninstall (全部卸载)
  └─ uninstall --keep-self (仅卸载工具包)
  ↓
加载安装记录 (.fingerprint.toml)
  ↓
创建卸载配置 (UninstallConfiguration)
  ↓
执行卸载 (uninstall)
  │
  ├─ 1. remove_tools() - 卸载第三方工具
  │     ├─ 按依赖关系排序（反向）
  │     ├─ 执行工具特定的卸载逻辑
  │     │   ├─ CargoTool: cargo uninstall
  │     │   ├─ DirWithBin: 删除目录 + 从 PATH 移除
  │     │   ├─ Plugin: VS Code 扩展卸载
  │     │   └─ Custom: 自定义卸载指令
  │     └─ 更新安装记录
  │
  ├─ 2. 卸载 Rust 工具链
  │     ├─ 删除 rustup home 目录
  │     │   └─ 保留 ruleset 工具链（如果存在）
  │     ├─ 删除 cargo home 目录
  │     │   └─ 保留第三方工具（如果 keep-self）
  │     └─ 删除 rustup 二进制文件
  │
  ├─ 3. (如果 remove_self = true) 移除环境变量
  │     ├─ (Windows) 删除注册表项
  │     └─ (Unix) 从 shell 配置文件中移除
  │
  ├─ 4. (如果 remove_self = true) 删除管理器
  │     ├─ 删除管理器二进制文件
  │     ├─ 删除符号链接
  │     └─ 删除安装记录和配置
  │
  └─ 5. 更新或删除安装记录
  ↓
完成
```

### 详细步骤说明

#### 1. 工具卸载 (`src/core/uninstall.rs::remove_tools()`)

```rust
// 卸载顺序（反向依赖顺序）：
// 1. 拓扑排序工具（考虑依赖关系）
// 2. 反向遍历，先卸载依赖其他工具的工具
// 3. 执行卸载逻辑
// 4. 更新安装记录
```

**工具类型特定的卸载**:
- `CargoTool`: `cargo uninstall {name}`
- `DirWithBin`: 删除目录 + `remove_from_path()`
- `Plugin`: `code --uninstall-extension {path}`
- `Custom`: 调用自定义卸载函数
- `Crate`: 删除 crate 目录 + 更新 `cargo/config.toml`

#### 2. Rust 工具链卸载 (`src/core/rustup.rs::uninstall()`)

```rust
// 步骤：
// 1. 删除 {RUSTUP_HOME}/toolchains/*（保留 ruleset）
// 2. 删除 {CARGO_HOME}/bin/rustup*（rustup 及其代理）
// 3. 删除 {RUSTUP_HOME} 的其他内容
// 4. 如果 {RUSTUP_HOME} 为空，删除整个目录
```

**注意**: 
- 保留 `ruleset` 工具链（如果存在），因为它是独立的检查工具
- 保留 `cargo/bin` 中的第三方工具（如果 `keep_self = true`）

#### 3. 环境变量移除 (`src/core/os/`)

**Windows** (`src/core/os/windows.rs`):
```rust
// 删除注册表项：
// HKEY_CURRENT_USER\Environment\{VAR_NAME}
```

**Unix** (`src/core/os/unix.rs`):
```rust
// 从 shell 配置文件中删除环境变量设置行
// 支持的 shell: bash, zsh, fish
```

#### 4. 管理器卸载 (`src/core/uninstall.rs::remove_self()`)

```rust
// 步骤：
// 1. 删除管理器二进制文件
// 2. 删除符号链接（cargo/bin/rim, cargo/bin/{app_name}）
// 3. (Windows) 删除注册表项（已安装程序列表）
// 4. 删除安装记录文件
// 5. 删除配置目录
```

#### 5. 部分卸载 (`keep_self = true`)

当使用 `uninstall --keep-self` 时：
- 仅卸载工具包（Rust 工具链 + 第三方工具）
- 保留管理器二进制文件
- 保留环境变量配置
- 更新安装记录（移除工具包信息）

---

## 核心模块说明

### 1. `src/core/install.rs` - 安装核心

**主要结构**:
- `InstallConfiguration`: 安装配置，包含所有安装所需信息
- `EnvConfig`: 环境变量配置 trait

**关键方法**:
- `setup()`: 初始化安装目录
- `install()`: 执行完整安装流程
- `install_rust()`: 安装 Rust 工具链
- `install_tools()`: 安装第三方工具

### 2. `src/core/update.rs` - 更新核心

**主要功能**:
- `check_self_update()`: 检查管理器更新
- `check_toolkit_update()`: 检查工具包更新
- `self_update()`: 执行管理器自更新

**更新流程**:
1. 检查最新版本
2. 下载新版本
3. 替换当前二进制文件（包括符号链接）

### 3. `src/core/tools.rs` - 工具管理

**工具类型** (`ToolKind`):
- `CargoTool`: Cargo 安装的工具
- `DirWithBin`: 包含 bin 目录的工具包
- `Executables`: 可执行文件
- `Plugin`: VS Code 扩展
- `Installer`: 安装程序
- `Custom`: 自定义安装
- `Crate`: Rust crate
- `RuleSet`: Clippy 规则集

**关键方法**:
- `Tool::from_path()`: 从路径识别工具类型
- `Tool::install()`: 安装工具
- `Tool::uninstall()`: 卸载工具

### 4. `src/core/components.rs` - 组件管理

**组件类型** (`ComponentType`):
- `ToolchainComponent`: Rust 工具链组件
- `ToolchainProfile`: 工具链配置
- `Tool`: 第三方工具

**关键功能**:
- `split_components()`: 分离工具链组件和第三方工具
- `all_components_from_installation()`: 获取所有已安装组件

### 5. `src/core/rustup.rs` - Rustup 集成

**主要功能**:
- `ensure_rustup()`: 确保 rustup 已安装
- `install_toolchain_with_components()`: 安装工具链和组件
- `add_components()`: 添加组件
- `remove_components()`: 移除组件
- `uninstall()`: 卸载整个工具链

### 6. `rim_common/src/utils/` - 工具函数

**download.rs**: 下载功能
- 支持断点续传
- 支持代理
- 支持本地文件复制

**extraction.rs**: 解压功能
- 支持 `.zip`, `.7z`, `.tar.gz`, `.tar.xz`
- 自动跳过单独嵌套目录

**file_system.rs**: 文件系统操作
- 路径规范化
- 符号链接/硬链接处理
- 权限设置

---

## 环境变量

### 安装时设置的环境变量

- `CARGO_HOME`: Cargo 主目录
- `RUSTUP_HOME`: Rustup 主目录
- `RUSTUP_DIST_SERVER`: Rust 工具链下载服务器
- `RUSTUP_UPDATE_ROOT`: Rustup 更新服务器
- `http_proxy` / `https_proxy`: 代理设置（如果配置）
- `no_proxy`: 不使用代理的地址列表

### 运行时环境变量

- `MODE`: 强制指定运行模式（`installer` 或 `manager`）
- `RIM_DIST_SERVER`: 覆盖默认的分发服务器
- `RUSTUP_TOOLCHAIN`: 会被移除（避免干扰安装）

---

## 文件路径说明

### Windows

- **安装目录**: `%USERPROFILE%\.rust`（默认）
- **配置目录**: `%APPDATA%\rim\`
- **CARGO_HOME**: `{install_dir}\cargo`
- **RUSTUP_HOME**: `{install_dir}\rustup`

### Unix/Linux

- **安装目录**: `$HOME/.rust`（默认）
- **配置目录**: `$HOME/.config/rim/`
- **CARGO_HOME**: `{install_dir}/cargo`
- **RUSTUP_HOME**: `{install_dir}/rustup`

---

## 依赖关系处理

### 工具依赖

工具可以声明：
- `requires`: 依赖的其他工具
- `obsoletes`: 被此工具替代的旧工具
- `conflicts`: 冲突的工具

安装时会：
1. 按依赖关系拓扑排序
2. 先安装被依赖的工具
3. 检测并拒绝冲突的工具组合
4. 自动卸载被替代的旧工具

### 卸载顺序

卸载时按反向依赖顺序：
1. 先卸载依赖其他工具的工具
2. 最后卸载被依赖的工具

---

## 错误处理

### 错误类型

- `anyhow::Error`: 通用错误类型
- `InstallerError` (GUI): GUI 特定的错误类型

### 错误恢复

- 安装失败时：记录错误，不自动回滚（TODO: 实现回滚）
- 卸载失败时：记录警告，继续卸载其他组件
- 文件操作失败时：重试机制（最多 10 次）

---

## 国际化支持

### 支持的语言

- 中文（zh-CN）
- 英文（en-US）

### 本地化文件

- 位置: `locales/{lang}.json`
- 使用 `rust-i18n` crate 进行国际化
- 通过 `t!()` 宏访问翻译文本

---

## 测试

### 测试结构

- `tests/testsuite/`: 集成测试
- `rim_test/`: 测试支持库

### 测试覆盖

- CLI 命令测试
- 文件提取测试
- 环境变量测试
- 文件遍历测试

---

## 构建和发布

### 开发构建

```bash
cargo dev run --installer    # 运行安装器模式
cargo dev run --manager      # 运行管理器模式（需要模拟环境）
```

### 发布构建

```bash
cargo dev dist --cli          # 构建 CLI 版本（包含离线包）
cargo dev dist -b --cli       # 构建 CLI 版本（仅网络安装器）
cargo dev dist --gui          # 构建 GUI 版本
```

### 构建产物

- **CLI**: `rim-cli.exe` (Windows) / `rim-cli` (Unix)
- **GUI**: `rim-gui.exe` (Windows) / `rim-gui` (Unix)
- **管理器**: `{app_name}.exe` / `{app_name}`

---

## 扩展点

### 自定义工具安装

通过 `src/core/custom_instructions/` 实现：
- `vscode.rs`: VS Code 安装
- `vscodium.rs`: VSCodium 安装
- `codearts_rust.rs`: CodeArts Rust 安装
- `buildtools.rs`: MSVC Build Tools 安装

### 添加新工具类型

1. 在 `src/core/tools.rs` 中添加新的 `ToolKind`
2. 实现 `Tool::install()` 和 `Tool::uninstall()` 方法
3. 在 `Tool::from_path()` 中添加识别逻辑

---

## 注意事项

1. **权限要求**: 
   - Windows: 需要管理员权限修改注册表
   - Unix: 需要写入 `$HOME` 目录的权限

2. **并发安全**: 
   - 安装/卸载操作不是线程安全的
   - 不应同时运行多个安装/卸载进程

3. **路径限制**: 
   - 安装路径不能包含无效的 Unicode 字符
   - 路径长度限制（Windows: 260 字符，可通过策略扩展）

4. **网络要求**: 
   - 需要访问配置的下载服务器
   - 支持代理配置

5. **磁盘空间**: 
   - Rust 工具链约需要 1-2 GB
   - 第三方工具根据选择而定

---

## 参考文档

- [用户指南](https://xuanwu.beta.atomgit.com/guide/)
- [API 文档](https://j-zhengli.github.io/rim)
- [Rustup 文档](https://rust-lang.github.io/rustup/)

---

*最后更新: 2024*

