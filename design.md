# 旋武社区 Rust 发行版设计文档

| 文档属性 | 说明 |
| -------- | ---- |
| 版本     | v1.0 |
| 作者     |      |
| 创建日期 | 2026-03-27 |
| 最后更新 | 2026-03-27 |

---

## 目录

1. [概述](#1-概述)
2. [需求分析](#2-需求分析)

**第一部分：设计功能实现**

3. [系统架构设计](#3-系统架构设计)
4. [一站式安装管理工具 (R.I.M)](#4-一站式安装管理工具-rim)
5. [Rust 语言原生组件](#5-rust-语言原生组件)
6. [优选基础库设计](#6-优选基础库设计)
7. [代码检查工具集设计](#7-代码检查工具集设计)
8. [跨平台适配方案](#8-跨平台适配方案)
9. [网络与镜像策略](#9-网络与镜像策略)
10. [国际化方案](#10-国际化方案)
11. [构建与发布流程](#11-构建与发布流程)

**第二部分：DFX设计**

12. [可靠性/可用性设计](#12-可靠性可用性设计)
13. [功能安全设计](#13-功能安全设计)
14. [网络安全设计](#14-网络安全设计)
15. [可维测设计](#15-可维测设计)

---

16. [未来演进规划](#16-未来演进规划)
17. [附录](#17-附录)

---

## 1. 概述

### 1.1 文档目的

本文档描述旋武社区 Rust 发行版的整体技术设计方案，包括系统架构、核心模块设计、安装管理流程、跨平台适配等关键技术决策，为开发、测试、运维提供统一的技术参考。

### 1.2 项目背景

旋武社区 Rust 发行版旨在为国内 Rust 开发者提供便捷的一站式安装管理体验，解决以下核心痛点：

- Rust 工具链安装过程复杂，国内网络环境不稳定
- 缺少开箱即用的开发环境配置
- Rust 编程规范缺少自动化检查手段
- 开发者需自行筛选和安装辅助工具
- 优质 Rust 基础库分散，缺乏统一的集成与分发渠道

### 1.3 项目范围

| 范围     | 说明 |
| -------- | ---- |
| 包含     | Rust 工具链安装管理、优选基础库一键集成、第三方工具集成、代码检查工具集、GUI/CLI 双模式安装器、国内镜像加速 |
| 不包含   | Rust 编译器本身的开发与维护、IDE 的开发 |

### 1.4 术语与缩写

| 术语 | 说明 |
| ---- | ---- |
| R.I.M | Rust Installation Manager，旋武发行版的安装管理工具 |
| Toolkit | 由 Rust 工具链 + 第三方工具组成的开发套件 |
| Toolset Manifest | 工具包清单配置文件，描述可安装的工具及其来源 |
| Distribution Manifest | 分发清单，列出服务器上可用的 Toolkit 版本 |
| Fingerprint | 安装记录文件，用于追踪本地已安装的组件 |
| 优选基础库 | 旋武社区筛选并集成的高质量 Rust 基础库（ylong 系列），以本地 Crate 补丁方式提供 |
| Crate 补丁 | 通过 Cargo `[patch.crates-io]` 机制将本地基础库注入项目依赖的技术手段 |

### 1.5 参考文档

- [旋武社区官网](https://xuanwu.openatom.cn)
- [Rust 编程规范](https://xuanwu.openatom.cn/articles/rules/coding-guidelines/)
- [Rustup 官方文档](https://rust-lang.github.io/rustup/)
- [Tauri 框架文档](https://tauri.app/)

### 1.6 相比 Rust 原生工具链的竞争力

旋武发行版并非简单的 Rust 工具链镜像搬运，而是在原生 Rust 语言组件基础上进行系统性增强，形成面向国内开发者的差异化竞争力：

| 维度 | Rust 原生工具链 (rustup) | 旋武发行版 | 竞争力提升 |
| ---- | ----------------------- | ---------- | ---------- |
| 安装体验 | 命令行安装，需手动配置环境变量和镜像 | GUI/CLI 双模式一站式安装，自动配置环境变量与国内镜像 | 安装配置更便捷 |
| 代码规范 | 仅 clippy 通用 lint | clippy + 旋武编程规范规则集（自定义 lint） | 语言规范能力更完善 |
| 基础库 | 仅标准库 (std) | std + ylong 系列优选基础库（HTTP/JSON/异步运行时/XML/Actor） | 基础库功能更完善 |
| 开发工具 | 需自行下载安装 IDE 和插件 | 一键安装 IDE（CodeArts/VS Code）+ rust-analyzer 语言插件 | 工具链功能更便捷易用 |
| 编译后端 | 官方 LLVM | 毕昇 LLVM（华为优化版） | 主流芯片（ARM）性能更优 |

#### 1.6.1 一站式安装：安装配置更便捷

Rust 官方 rustup 安装流程要求用户：手动执行安装脚本 → 配置 `CARGO_HOME`/`RUSTUP_HOME` 环境变量 → 修改 `config.toml` 配置国内镜像 → 逐个安装 clippy/rustfmt/rust-src 等组件 → 自行下载 IDE。整个过程对新手不友好，且国内网络环境下常因下载超时而失败。

旋武发行版将上述步骤整合为一次操作：

| 对比项 | rustup 原生流程 | 旋武发行版 |
| ------ | -------------- | ---------- |
| 安装方式 | 仅命令行 | GUI 图形界面 + CLI 命令行 |
| 环境变量 | 手动配置或依赖 shell profile | 自动写入系统/用户环境变量 |
| 镜像配置 | 手动编辑 `~/.cargo/config.toml` | 安装时自动配置国内镜像 |
| 组件选择 | 逐个 `rustup component add` | 标准版/精简版/自定义三种预设方案 |
| IDE 集成 | 自行下载安装 | 安装流程中可选一键安装 |
| 离线支持 | 不支持 | 提供离线全量包 |
| Windows 依赖 | 需自行安装 Build Tools/MinGW | 自动检测并引导安装 |

#### 1.6.2 规范检查工具：语言规范能力更完善

Rust 官方 clippy 提供约 700+ 条通用 lint 规则，侧重代码风格和常见错误检测。旋武发行版在此基础上集成了《Rust 编程规范》规则集，提供面向工程实践的深度检查能力：

| 对比项 | clippy (官方) | 旋武编程规范规则集 |
| ------ | ------------- | ------------------ |
| 规则来源 | Rust 社区通用最佳实践 | 《Rust 编程规范》指导文档 |
| 规则侧重 | 代码风格、性能、常见错误 | 工程规范、安全编码、类型约束 |
| 典型规则 | `unused_variables`、`clippy::unwrap_used` | `unconstrained_numeric_literal`（数值类型必须显式标注） |
| 使用方式 | `cargo clippy` | `xuanwu-rust-manager check` |
| 自动修复 | 部分规则支持 `--fix` | 支持自动修复建议 |
| 规范文档 | 分散在各 lint 说明中 | 对应完整的编程规范文档章节 |

两者互补而非替代：clippy 覆盖通用场景，编程规范规则集覆盖工程化深度场景，共同构成更完善的代码质量保障体系。

#### 1.6.3 ylong 优选基础库：基础库功能更完善

Rust 标准库 (std) 设计哲学为"小而精"，仅提供最基础的数据结构、I/O、多线程原语，不包含 HTTP 客户端、JSON 解析、异步运行时等应用开发常用功能。开发者需自行从 crates.io 筛选第三方库，面临版本兼容、质量参差、供应链安全等问题。

旋武发行版集成 ylong 系列优选基础库，填补标准库与应用开发之间的能力空白：

| 能力域 | Rust 标准库 | ylong 优选基础库 | 补充说明 |
| ------ | ----------- | ---------------- | -------- |
| HTTP 通信 | 无 | `ylong_http` (客户端 + 协议组件) | 支持 HTTP/1.1 和 HTTP/2，面向 OpenHarmony 网络协议栈 |
| JSON 处理 | 无 | `ylong_json` (序列化/反序列化) | 通用 JSON 解析，支持 DOM 和流式解析 |
| 异步运行时 | 无（仅 `Future` trait） | `ylong_runtime` (完整异步运行时) | 异步任务调度、异步网络/文件 IO、定时器、并行迭代器 |
| XML 处理 | 无 | `ylong_xml` (序列化/反序列化) | XML 文本解析与生成 |
| 并发模型 | `std::thread` + `std::sync` | `ylong_light_actor` (Actor 模型) | 避免共享内存的数据竞争和死锁问题 |

集成方式采用 Cargo `[patch.crates-io]` 本地补丁机制，用户无需手动配置即可在项目中直接引用，且版本经过旋武社区验证测试。

#### 1.6.4 IDE 工具 + 语言插件：工具链功能更便捷易用

Rust 官方不提供 IDE，开发者需自行完成：选择 IDE → 下载安装 → 安装 rust-analyzer 插件 → 配置工具链路径 → 验证功能。对于新手用户，这一过程存在选择困难和配置门槛。

旋武发行版将 IDE 纳入安装流程，实现开发环境开箱即用：

| 对比项 | 原生 Rust 生态 | 旋武发行版 |
| ------ | -------------- | ---------- |
| IDE 获取 | 自行下载（多个来源） | 安装流程中一键选装 |
| 可选 IDE | 无官方推荐 | CodeArts IDE（华为云开发者桌面）、VS Code |
| 语言服务 | 自行安装 rust-analyzer | 可选组件一键安装 |
| 插件配置 | 手动配置工具链路径 | 安装后自动关联 |
| 调试支持 | 自行配置 GDB/LLDB | 集成 rust-gdb、rust-lldb |
| 文档访问 | `rustup doc`（需先知道命令） | 安装 rust-docs 后本地离线浏览 |

CodeArts IDE 作为华为云开发者桌面，提供面向 Rust 的智能补全、调试、项目管理等一站式开发体验，与旋武发行版深度集成。

#### 1.6.5 后端毕昇 LLVM：主流芯片（ARM）性能更优

Rust 官方编译器使用上游 LLVM 作为代码生成后端。旋武社区构建版本采用毕昇 LLVM（华为优化版 LLVM），针对 ARM 架构（鲲鹏、手机 SoC 等）进行了深度优化：

| 对比项 | 官方 LLVM | 毕昇 LLVM |
| ------ | --------- | ---------- |
| 优化目标 | 通用多架构均衡 | ARM 架构深度优化 |
| ARM 代码生成 | 标准优化 Pass | 增强的 ARM 后端优化（向量化、指令调度） |
| 鲲鹏适配 | 无专项优化 | 针对鲲鹏微架构的调度模型 |
| 性能表现 | 基准水平 | ARM 平台编译产物运行性能提升 |
| 兼容性 | 上游标准 | 完全兼容上游，增量优化 |

毕昇 LLVM 在保持与上游 LLVM 完全兼容的前提下，通过增强的 ARM 后端优化 Pass、改进的向量化策略和针对鲲鹏微架构的指令调度模型，使编译产出的二进制在 ARM 平台上获得更优的运行时性能。这对于部署在鲲鹏服务器、ARM 嵌入式设备上的 Rust 应用具有直接的性能收益。

---

## 2. 需求分析

### 2.1 用户画像

| 用户类型 | 特征 | 核心需求 |
| -------- | ---- | -------- |
| Rust 初学者 | 首次接触 Rust，不熟悉工具链 | 一键安装、开箱即用、中文界面 |
| 企业开发者 | 需要规范化开发环境 | 统一工具版本、代码规范检查、离线安装 |
| 开源贡献者 | 熟悉 Rust 生态 | 灵活组件选择、镜像加速、快速更新 |

### 2.2 功能需求

#### 2.2.1 核心功能

- **FR-001**: 一站式安装 Rust 工具链（rustc、cargo、std、clippy、rustdoc、rustup）
- **FR-002**: 支持 GUI 图形化安装界面与 CLI 命令行安装
- **FR-003**: 自动配置环境变量（CARGO_HOME、RUSTUP_HOME 等）
- **FR-004**: 使用国内镜像源，保证下载速度
- **FR-005**: 支持第三方优选工具安装（cargo-nextest、rust-analyzer 等）
- **FR-006**: 集成旋武社区 Rust 编程规范检查工具集
- **FR-007**: 支持工具包的升级与降级
- **FR-008**: 支持单个组件的安装与卸载
- **FR-009**: 支持离线安装包模式
- **FR-010**: 支持管理器自更新
- **FR-011**: 一键安装优选基础库（ylong_http、ylong_json、ylong_runtime、ylong_xml），以本地 Crate 补丁方式集成到用户项目依赖中

#### 2.2.2 扩展功能

- **FR-012**: 提供示例项目快速体验（try-it 命令）
- **FR-013**: 支持 IDE 选择安装：华为 CodeArts IDE（从华为云 OBS 下载）和 VS Code（从官方 Release 下载，避免二次分发）
- **FR-014**: 支持 Windows 环境依赖自动安装（MSVC Build Tools / MinGW）

### 2.3 非功能需求

| 编号 | 类别 | 需求描述 |
| ---- | ---- | -------- |
| NFR-001 | 兼容性 | 支持 Windows x86_64 (MSVC/MinGW)、Linux (x86_64/aarch64 + gnu/musl)，鸿蒙 PC 适配中 |
| NFR-002 | 性能 | 在正常网络环境下，完整安装时间不超过 10 分钟 |
| NFR-003 | 可用性 | 安装过程支持中英文双语 |
| NFR-004 | 可靠性 | 安装失败时提供明确错误信息，支持断点续传 |
| NFR-005 | 安全性 | 下载内容校验完整性，环境变量修改可控 |
| NFR-006 | 可维护性 | 模块化架构，支持新工具类型扩展 |

### 2.4 约束条件

- 安装器与管理器共用同一二进制文件，通过运行模式区分
- 工具链安装底层依赖 rustup，不重新实现工具链管理
- GUI 基于 Tauri 框架，前端使用 Vue
- 最低支持的 Rust 版本为 1.80.0

---

---

# 第一部分：设计功能实现

---

## 3. 系统架构设计

### 3.1 整体架构

一站式安装管理工具（R.I.M）采用 **基于 Tauri 的分层架构**，以 Rust 为核心语言实现跨平台桌面应用，同时提供独立的 CLI 二进制。Tauri 框架使得 GUI 版本在保持原生性能的同时，安装包体积远小于 Electron 方案（约 5-10 MB vs 100+ MB），对网络受限的国内环境更加友好。

```
┌─────────────────────────────────────────────────────────────────────┐
│                    旋武社区 Rust 发行版安装管理工具 (R.I.M)            │
│                                                                     │
│  ┌─────────────────────────────────────────────────────────────┐    │
│  │                      用户交互层                              │    │
│  │                                                             │    │
│  │  ┌─────────────────────────┐   ┌────────────────────────┐  │    │
│  │  │     GUI (Tauri + Vue 3) │   │     CLI (clap)         │  │    │
│  │  │  ┌───────────────────┐  │   │  ┌──────────────────┐  │  │    │
│  │  │  │  Vue 3 前端       │  │   │  │ rim_cli.rs 入口  │  │  │    │
│  │  │  │  ├ views/         │  │   │  │ ├ install.rs     │  │  │    │
│  │  │  │  ├ components/    │  │   │  │ ├ update.rs      │  │  │    │
│  │  │  │  ├ router/        │  │   │  │ ├ uninstall.rs   │  │  │    │
│  │  │  │  └ utils/         │  │   │  │ ├ component.rs   │  │  │    │
│  │  │  └───────┬───────────┘  │   │  │ └ check.rs       │  │  │    │
│  │  │    Tauri IPC (invoke)   │   │  └──────────────────┘  │  │    │
│  │  │  ┌───────┴───────────┐  │   │                        │  │    │
│  │  │  │  Tauri 后端 (Rust) │  │   │                        │  │    │
│  │  │  │  ├ installer_mode │  │   │                        │  │    │
│  │  │  │  ├ manager_mode   │  │   │                        │  │    │
│  │  │  │  ├ command.rs     │  │   │                        │  │    │
│  │  │  │  └ progress.rs    │  │   │                        │  │    │
│  │  │  └───────────────────┘  │   │                        │  │    │
│  │  └─────────────────────────┘   └────────────────────────┘  │    │
│  └──────────────────────────┬──────────────────────────────────┘    │
│                             │                                       │
│  ┌──────────────────────────┴──────────────────────────────────┐    │
│  │                      核心业务层 (rim crate)                  │    │
│  │                                                             │    │
│  │  src/core/                                                  │    │
│  │  ├── install.rs           安装流程编排                       │    │
│  │  ├── uninstall.rs         卸载流程编排                       │    │
│  │  ├── update.rs            版本检测与升级                     │    │
│  │  ├── toolkit.rs           Toolkit 生命周期管理               │    │
│  │  ├── components.rs        组件拆分与管理                     │    │
│  │  ├── rustup.rs            Rustup 工具链集成                  │    │
│  │  ├── tools.rs             第三方工具 + 优选基础库安装/卸载    │    │
│  │  ├── custom_instructions/  自定义安装指令                    │    │
│  │  │   ├── vscode.rs        VS Code 安装（VSCodeInstaller）    │    │
│  │  │   ├── codearts_rust.rs CodeArts IDE 安装（复用上述结构）   │    │
│  │  │   ├── vscodium.rs      VSCodium 安装                     │    │
│  │  │   ├── buildtools.rs    MSVC Build Tools 安装 (Windows)    │    │
│  │  │   └── mod.rs           宏路由 + 工具检测                  │    │
│  │  ├── parser/              配置文件解析                       │    │
│  │  └── os/                  平台适配抽象                       │    │
│  │      ├── windows.rs       注册表 / PATH / 快捷方式           │    │
│  │      └── unix.rs          Shell 配置文件 / PATH              │    │
│  └──────────────────────────┬──────────────────────────────────┘    │
│                             │                                       │
│  ┌──────────────────────────┴──────────────────────────────────┐    │
│  │                     公共基础层 (rim_common crate)             │    │
│  │                                                             │    │
│  │  ├── download      网络下载 (reqwest, 断点续传, 代理)        │    │
│  │  ├── extraction    归档解压 (zip/7z/tar.gz/tar.xz)          │    │
│  │  ├── file_system   文件操作 (符号链接/权限)                  │    │
│  │  ├── progress      进度抽象 (CliProgress / GuiProgress)      │    │
│  │  ├── types         Manifest/Fingerprint 类型定义             │    │
│  │  └── dirs          跨平台路径管理                            │    │
│  └─────────────────────────────────────────────────────────────┘    │
│                                                                     │
│  ┌─────────────────────────────────────────────────────────────┐    │
│  │                      分发服务器                              │    │
│  │  dist-manifest.toml  |  toolset-manifest.toml  |  release  │    │
│  └─────────────────────────────────────────────────────────────┘    │
└─────────────────────────────────────────────────────────────────────┘
```

### 3.2 Cargo 工作空间结构

R.I.M 采用 Cargo Workspace 组织为五个 crate，各 crate 职责边界清晰：

```
custom-rust-dist/                  # Cargo Workspace 根
├── Cargo.toml                     # workspace 定义 + rim 主 crate
├── src/                           # rim — 核心业务 + CLI 入口
│   ├── bin/rim_cli.rs             #   CLI 二进制入口点
│   ├── cli/                       #   CLI 命令解析 (clap)
│   └── core/                      #   核心安装/卸载/更新逻辑
├── rim_gui/                       # rim_gui — Tauri 桌面应用
│   ├── src/                       #   Vue 3 前端 (views/components/router)
│   ├── src-tauri/                 #   Tauri Rust 后端
│   └── package.json               #   前端依赖 (Vue 3 + vue-router + vue-i18n)
├── rim_common/                    # rim_common — 公共类型与工具函数
├── rim_dev/                       # rim_dev — 开发辅助 (打包/vendor/mock)
├── rim_test/                      # rim_test — 测试宏与测试支持
├── resources/                     # 资源文件
│   ├── toolkit-manifest/
│   │   ├── online/                #   在线安装清单 (URL 指向远端)
│   │   └── offline/               #   离线安装清单 (path 指向本地)
│   ├── packages/                  #   离线安装包资源
│   └── templates/                 #   环境变量模板
├── locales/                       # 国际化 (zh-CN.json / en-US.json)
├── ci/                            # CI/CD
│   ├── docker/                    #   Docker 构建环境 (x86_64 / aarch64)
│   └── scripts/                   #   构建脚本
└── .github/workflows/             # GitHub Actions 流水线
```

### 3.3 安装模式总览

R.I.M 提供三种安装模式，覆盖不同使用场景：

| 安装模式 | 入口 | 网络要求 | 适用场景 |
| -------- | ---- | -------- | -------- |
| **图形化在线安装** | `rim-gui` (Tauri 桌面应用) | 需联网 | 初学者、普通开发者，图形界面引导式安装 |
| **命令行在线安装** | `rim-cli [OPTIONS]` | 需联网 | 服务器环境、CI/CD 流水线、批量部署 |
| **命令行离线安装** | `rim-cli --manifest <本地路径>` | 无需联网 | 内网/隔离环境、企业统一部署 |

#### 3.3.1 图形化在线安装

基于 Tauri 框架，前端使用 Vue 3 渲染安装界面，后端通过 Tauri IPC 调用 Rust 核心逻辑。安装过程从分发服务器下载 Toolset Manifest 和工具包。

```
用户启动 rim-gui
  → Tauri 检测模式 (Installer/Manager)
  → Vue 前端渲染安装向导 (欢迎 → 配置 → 组件选择 → 安装 → 完成)
  → 前端通过 invoke 调用后端 Tauri Command
  → 后端从远程服务器下载 manifest + 工具包
  → 通过 event 推送实时进度到前端进度条
  → 安装完成，写入 .fingerprint.toml
```

#### 3.3.2 命令行在线安装

纯 CLI 交互，使用 clap 解析命令行参数，通过 indicatif 显示下载和安装进度条。默认从分发服务器获取最新 manifest。

```
rim-cli [--prefix <PATH>] [--lang cn|en] [--component rustc,cargo,...]
  → 从远程服务器下载 dist-manifest.toml → 选择最新 toolkit
  → 下载 toolset-manifest.toml
  → 交互式组件选择 (或通过 --component / --yes-to-all 跳过)
  → 下载并安装组件
  → 配置环境变量
```

#### 3.3.3 命令行离线安装

通过 `--manifest` 参数指定本地清单文件，所有工具包预置在本地 `packages/` 目录中。构建时通过 `no-web` feature 编译出离线专用二进制，内嵌离线 manifest。

```
rim-cli --manifest ./toolset-manifest.toml
  → 解析本地清单 (path 字段指向本地 ZIP/tar 文件)
  → 直接从本地 packages/ 解压安装
  → 全程无需网络连接
```

离线安装包由 `cargo dev dist --cli` (含离线包) 或 `cargo dev vendor --download-only` 预先构建。

### 3.4 运行模式检测

安装器 (Installer) 与管理器 (Manager) 共用同一二进制文件，运行时自动检测模式：

```rust
// 检测优先级 (Mode::detect)
1. 环境变量 MODE=manager       → Manager 模式
2. 环境变量 MODE=其他值        → Installer 模式
3. 程序名包含 "installer"      → Installer 模式
4. 存在 install-record.toml   → Manager 模式 (已安装)
5. 以上均不满足                → Installer 模式 (默认)
```

**GUI 模式下的双重入口**：

```
rim-gui 启动
  │
  ├── 检测命令行参数
  │   ├── --no-gui 或特定子命令 → 执行 CLI 逻辑后退出
  │   └── 无特殊参数 → 继续 GUI 启动
  │
  ├── 隐藏控制台窗口 (仅 Windows)
  │
  └── 根据 Mode 启动对应 Tauri 窗口
      ├── Mode::Installer → installer_mode::main()
      └── Mode::Manager   → manager_mode::main()
```

| 模式 | 触发条件 | 职责 |
| ---- | -------- | ---- |
| Installer | 首次运行 / 无安装记录 / `MODE=installer` | 初始化环境、安装工具链和工具 |
| Manager | 检测到已有安装 / `MODE=manager` | 更新、卸载、组件管理 |

### 3.5 技术栈

| 层次 | 技术选型 | 版本 | 说明 |
| ---- | -------- | ---- | ---- |
| 核心逻辑 | Rust | MSRV 1.80.0 | 安全、高性能、跨平台 |
| GUI 框架 | **Tauri** | v1 | Rust 原生桌面框架，安装包体积小、性能优 |
| GUI 前端 | Vue 3 + vue-router + vue-i18n | 3.5+ | 响应式 UI，支持路由与多语言 |
| 前端构建 | Vite + UnoCSS | 5.x | 快速热更新，原子化 CSS |
| 前端包管理 | pnpm | latest | 高效依赖管理 |
| CLI 解析 | clap | 4.0 | Rust 主流命令行解析库 |
| 网络请求 | reqwest | 0.12 | 异步 HTTP 客户端，支持 streaming + native-tls-vendored |
| 异步运行时 | tokio | 1.x | 多线程异步运行时 |
| 归档处理 | zip / tar / xz2 / sevenz-rust | - | 多格式解压 |
| 工具链管理 | rustup | 外部调用 | 复用官方工具链管理能力 |
| 国际化 | rust-i18n + vue-i18n | 3.x / 11.x | Rust 侧 + Vue 侧双语支持 |
| 错误处理 | anyhow | 1.x | 灵活的错误上下文链 |
| 进度展示 | indicatif (CLI) / Tauri Event (GUI) | - | 双端进度抽象 |
| 日志 | fern + log | - | 彩色分级日志 |
| 单实例 | tauri-plugin-single-instance | v1 | 防止重复启动 |
| 版本比较 | semver | 1.0.23 | 语义化版本比较 |
| 自更新 | self-replace | 1.0 | 原子替换运行中的二进制文件 |
| 序列化 | serde + toml | 1.0 | 配置文件解析与生成 |

---

## 4. 一站式安装管理工具 (R.I.M)

R.I.M (Rust Installation Manager) 是旋武发行版的核心一站式安装管理工具，集安装、管理、更新、卸载于一体，支持 GUI 和 CLI 双模式运行。本章涵盖工具的核心模块设计、安装模式、GUI 前端、工具链与组件管理及配置文件体系。

### 4.1 核心模块设计

#### 4.1.1 安装模块 (install.rs)

```
InstallConfiguration
├── install_dir: PathBuf           # 安装根目录
├── toolkit_manifest: ToolkitManifest  # 工具包清单
├── selected_components: Vec<Component> # 用户选择的组件
├── cargo_registry: Registry       # Cargo 镜像源配置
└── proxy: Option<ProxyConfig>     # 代理配置
```

**安装流程**：

```
安装入口 install(components)
  │
  ├── 1. split_components()    → 拆分为工具链组件和第三方工具
  ├── 2. 冲突检测              → 检查 conflicts 声明，拒绝冲突组件
  ├── 3. setup() [关键步骤]    → 初始化目录结构、复制管理器、创建符号链接
  │      ├── 创建安装目录
  │      ├── 复制 Toolset Manifest 到安装目录
  │      ├── 复制管理器二进制到安装目录
  │      ├── 创建符号链接 (rim + 完整名称) 到 cargo/bin/
  │      ├── 写入应用图标 (.ico)
  │      └── 注册到 Windows "添加/删除程序" (仅 Windows)
  ├── 4. config_env_vars()     → 配置环境变量 (失败不中断)
  ├── 5. config_cargo()        → 写入 cargo/config.toml 镜像源配置 (失败不中断)
  ├── 6. install_tools()       → 安装不依赖 Rust 的第三方工具 (early tools)
  ├── 7. install_rust()        → 通过 rustup 安装 Rust 工具链
  ├── 8. install_tools_late()  → 安装依赖 Rust 的工具 (cargo install 类)
  └── 9. 写入 install-record.toml 安装记录
        └── 汇总报告所有累积错误 (InstallationErrors)
```

**错误处理策略**：安装过程采用"尽力完成"策略，除 `setup()` 步骤外，其余步骤失败不会中断整体安装流程。所有错误按类别收集（工具安装错误、工具链错误、步骤错误），在安装结束后统一汇报，允许部分成功。

**安装目录结构**：

```
{install_dir}/
├── cargo/              # CARGO_HOME
│   ├── bin/            # 可执行文件 (加入 PATH)
│   │   ├── rim         # 管理器符号链接 (短名)
│   │   └── xuanwu-rust-manager  # 管理器符号链接 (完整名)
│   └── config.toml     # Cargo 配置 (含镜像源 + 优选基础库补丁)
├── rustup/             # RUSTUP_HOME
├── tools/              # 第三方工具目录
│   ├── vscode/         # VS Code (可选)
│   ├── codearts-rust/  # CodeArts IDE (可选)
│   ├── mingw64/        # MinGW-w64 (Windows GNU)
│   └── buildtools/     # Build Tools 安装程序备份 (Windows MSVC)
├── crates/             # 优选基础库本地 crate (Cargo patch 依赖)
│   ├── ylong_http/     # HTTP 协议栈 (含 ylong_http + ylong_http_client)
│   ├── ylong_json/     # JSON 解析库
│   ├── ylong_light_actor/  # Actor 并发编程模型
│   ├── ylong_runtime/  # 异步运行时 (含多个子 crate)
│   └── ylong_xml/      # XML 解析库
├── temp/               # 临时文件 (安装完成后清理)
├── toolset-manifest.toml  # 本地 Manifest 副本
└── {app_name}          # 管理器二进制文件
```

**安装记录文件**：安装状态持久化到 `~/.rim/install-record.toml`（通过 `rim_config_dir()` 获取路径），而非安装目录内，确保管理器在任意位置均可检测安装状态。

#### 4.1.2 卸载模块 (uninstall.rs)

```
卸载入口 uninstall()
  │
  ├── 1. remove_tools()       → 按反向依赖顺序卸载第三方工具
  ├── 2. 卸载 Rust 工具链      → 删除 rustup/cargo 目录
  ├── 3. 移除环境变量           → (仅完全卸载时)
  ├── 4. 删除管理器自身         → (仅完全卸载时)
  └── 5. 更新/删除安装记录
```

**卸载模式**：

| 模式 | 命令 | 行为 |
| ---- | ---- | ---- |
| 完全卸载 | `uninstall` | 删除所有内容，包括管理器和环境变量 |
| 保留管理器 | `uninstall --keep-self` | 仅卸载工具包，保留管理器以便重新安装 |

#### 4.1.3 更新模块 (update.rs)

| 更新类型 | 检测机制 | 更新流程 |
| -------- | -------- | -------- |
| 管理器自更新 | 读取服务器 `release.toml` 对比版本 | 下载新版 → 原子替换二进制 + 更新符号链接 |
| 工具包更新 | 读取服务器 `dist-manifest.toml` 对比版本 | 下载新清单 → 差量安装/卸载组件 |

**自更新机制**：

```
管理器自更新流程
  │
  ├── 1. check_self_update()    → 从服务器获取最新版本信息 (缓存在 OnceLock)
  │      返回: Newer{current, latest} / Uncertain / UnNeeded
  │
  ├── 2. 下载新版管理器二进制到临时目录
  │
  ├── 3. 处理硬链接和符号链接
  │      ├── 更新 rim 符号链接
  │      └── 更新完整名称符号链接
  │
  └── 4. 使用 self-replace crate 原子替换当前二进制
         (确保替换过程中不会出现损坏状态)
```

**更新检测结果类型** (`UpdateKind<T>`)：
- `Newer { current, latest }` — 有新版本可用
- `Uncertain` — 网络错误，无法确定
- `UnNeeded` — 已是最新版本

#### 4.1.4 组件管理模块 (components.rs)

```
ComponentType
├── ToolchainComponent   # Rust 工具链组件 (rustc, cargo, clippy 等)
├── ToolchainProfile     # 工具链配置
└── Tool                 # 第三方工具
```

**组件操作**：

- `split_components()`: 将用户选择拆分为工具链组件和第三方工具
- `all_components_from_installation()`: 从安装记录获取已安装组件列表

### 4.2 三种安装模式设计

#### 4.2.1 图形化在线安装 (rim-gui)

面向普通开发者的可视化安装方式，提供完整的引导式安装体验。

**安装流程**：

```
欢迎页 → 安装按钮 → 组件选择 → 确认安装 → 安装进度 → 完成
  │                      │
  │                      ├── 精简版: 仅 Rust 编译器所需最基础组件
  │                      ├── 标准版 (推荐): Rust 官方工具 + clippy + rustfmt
  │                      │     + rust-src + 编程规范规则集
  │                      └── 自定义: 自由选择所有组件
  │                           ├── Rust 基础工具集 (工具链 + 构建依赖)
  │                           ├── Rust 代码检查工具集 (编程规范规则集)
  │                           ├── Rust 优选工具集 (cargo-nextest)
  │                           ├── Rust 优选开发库 (ylong 系列)
  │                           └── IDE (CodeArts IDE / VS Code)
  │
  └── 安装完成后可勾选:
      ├── "完成后打开" → 启动 IDE
      └── "创建桌面快捷方式"
      └── 自动切换到 Manager 模式 (管理/更新/卸载)
```

**技术实现**: 详见 [4.6 GUI 前端设计](#46-gui-前端设计)。

#### 4.2.2 命令行在线安装 (rim-cli)

面向服务器环境、CI/CD 和高级用户的安装方式。

```
rim-cli [OPTIONS]

Options:
  -l, --lang <LANG>              显示语言 [cn, en]
      --prefix <PATH>            指定安装目录
      --manifest <PATH or URL>   指定工具包清单 (本地路径或远程 URL)
      --component <LIST>         直接指定安装组件 (逗号分隔)
      --yes-to-all               跳过所有确认提示 (适合自动化)
      --no-modify-path           不修改 PATH 环境变量
      --no-modify-env            不修改任何环境变量
      --registry-url <URL>       自定义 Cargo 注册表 URL
      --registry-name <NAME>     自定义 Cargo 注册表名称
      --rustup-dist-server <URL> 自定义 Rustup 分发服务器
      --rustup-update-root <URL> 自定义 Rustup 更新地址
      --insecure                 跳过 SSL 证书验证
      --list-components          列出可安装组件后退出
      --quiet                    静默模式
      --verbose                  详细输出
  -h, --help                     帮助信息
  -V, --version                  版本信息
```

**CLI 安装交互流程**：

```
运行安装程序
  → 选择安装版本 (标准版 / 精简版 / 自定义)
  → 指定安装路径
  → 确认安装组件列表
  → 等待安装完成
  → 提示是否创建示例项目 (try-it)
```

**自动化安装示例**:

```bash
# 全自动安装 (使用默认配置)
./rim-cli --yes-to-all --prefix /opt/rust

# 指定组件安装
./rim-cli --component rustc,cargo,clippy,ylong_json --yes-to-all

# 使用自定义 manifest URL
./rim-cli --manifest https://mirror.example.com/toolset-manifest.toml
```

#### 4.2.3 命令行离线安装 (rim-cli + 离线包)

面向内网隔离环境和企业统一部署的安装方式。

**离线安装包构建**:

```bash
# 开发机上预构建离线安装包
cargo dev vendor --download-only    # 下载所有工具包到 packages/
cargo dev dist --cli                # 构建含离线包的 CLI 安装器
```

**离线安装包结构**:

```
offline-package/
├── rim-cli                             # 安装器二进制 (内嵌离线 manifest)
├── toolset-manifest.toml               # 工具包清单 (path 指向本地文件)
└── tools/                              # 预下载的工具包
    ├── rustup-init
    ├── rust-{version}-{target}.tar.xz
    ├── commonlibrary_rust_ylong_*.zip  # 优选基础库
    └── ...                             # 其他第三方工具
```

**离线安装命令**:

```bash
# 使用本地 manifest 离线安装
./rim-cli --manifest ./toolset-manifest.toml --prefix /opt/rust
```

**在线/离线清单区分**: 构建时通过 Cargo feature `no-web` 控制，编译时内嵌不同的 manifest 模板：

| Feature | 内嵌 Manifest | 工具来源 |
| ------- | ------------- | -------- |
| 默认 (在线) | `resources/toolkit-manifest/online/{edition}.toml` | URL 指向远端服务器 |
| `no-web` (离线) | `resources/toolkit-manifest/offline/{edition}.toml` | path 指向本地文件 |

### 4.3 Manager 模式命令

安装完成后，管理器提供交互式菜单和命令行两种管理方式。

**交互式菜单** (直接运行 `xuanwu-rust-manager`)：

| 选项 | 功能 |
| ---- | ---- |
| 组件管理 | 添加或移除组件 |
| 更新 | 更新 Rust 工具链到最新版本 |
| 卸载 | 移除旋武发行版 |
| 显示组件/套件列表 | 查看已安装的组件 |

**命令行模式**：

```
xuanwu-rust-manager [OPTIONS] [COMMAND]

Commands:
  install     安装新组件
  update      更新工具包和/或管理器
  uninstall   卸载组件或全部
  list        列出已安装/可安装组件
  check       检查可用更新
  component   组件管理 (添加/移除)
  try-it      创建示例项目
  help        帮助信息

update Options:
  --toolkit-only   仅更新工具包
  --self-only      仅更新管理器

uninstall Options:
  --keep-self      保留管理器
```

### 4.4 工具类型支持

R.I.M 支持多种工具安装类型，通过 `ToolKind` 枚举统一管理：

| 工具类型 | Manifest 标识 | 安装方式 | 卸载方式 | 典型工具 |
| -------- | ------------- | -------- | -------- | -------- |
| CargoTool | `cargo-tool` | `cargo install` | `cargo uninstall` | - |
| DirWithBin | `dir-with-bin` | 解压到 tools/ + 合并 bin 到 PATH | 删除目录 + 移除 PATH | MinGW-w64 |
| Executables | `executables` | 复制可执行文件到 cargo/bin/ | 删除文件 | cargo-nextest |
| Plugin | `plugin` | VS Code 扩展安装 | VS Code 扩展卸载 | - |
| Installer | `installer` | 运行安装程序 | - | - |
| Custom | `custom` | 自定义安装指令 | 自定义卸载指令 | VS Code, CodeArts, Build Tools |
| Crate | `crate` | 解压到 crates/ + 配置 Cargo patch | 删除 + 移除 patch | ylong 系列 |
| RuleSet | `rule-set` | 规则集安装到工具链目录 | 规则集卸载 | coding-guidelines-ruleset |

**工具类型自动检测**：当 Manifest 未显式指定 `kind` 时，`Tool::from_path()` 按以下优先级自动推断：

```
1. 检查是否有自定义安装指令 (Custom)
2. 检查是否为可执行文件 (Executables)
3. 检查是否为插件 (Plugin)
4. 检查是否包含 Cargo.toml (Crate)
5. 检查是否包含 bin/ 目录 (DirWithBin)
6. 检查目录内是否有可执行文件 (Executables)
7. 以上均不满足 → Unknown
```

### 4.5 依赖关系处理

工具可声明以下依赖关系：

- `requires`: 依赖的其他工具（安装时先装依赖）
- `obsoletes`: 被此工具替代的旧工具（自动卸载旧工具）
- `conflicts`: 冲突的工具（拒绝同时安装）

安装时拓扑排序确保依赖顺序，卸载时反向处理。

### 4.6 GUI 前端设计

#### 4.6.1 Tauri 架构选型

R.I.M 图形化安装工具基于 **Tauri v1** 框架构建。相比 Electron，Tauri 的核心优势在于：

| 对比项 | Tauri | Electron |
| ------ | ----- | -------- |
| 后端语言 | **Rust** (与 RIM 核心共用) | Node.js |
| 安装包大小 | ~5-10 MB | ~100+ MB |
| 内存占用 | 低 (系统 WebView) | 高 (内嵌 Chromium) |
| 安全模型 | 白名单 API 暴露 | Node.js 完整权限 |
| 跨平台渲染 | 系统原生 WebView | 内嵌 Chromium |

Tauri 使得 GUI 层直接调用 Rust 核心逻辑（`rim` crate），无需序列化/进程通信开销，GUI 安装器与 CLI 安装器共享同一套核心安装代码。

#### 4.6.2 前端技术栈

| 组件 | 技术 | 版本 | 说明 |
| ---- | ---- | ---- | ---- |
| UI 框架 | Vue 3 | 3.5+ | Composition API，响应式状态管理 |
| 路由 | vue-router | 4.5+ | 双布局路由 (Installer / Manager) |
| 国际化 | vue-i18n | 11.x | 前端中英文切换 |
| 构建工具 | Vite | 5.x | 毫秒级热更新 |
| CSS 方案 | UnoCSS | 0.61+ | 原子化 CSS，按需生成 |
| 类型检查 | vue-tsc | 2.x | Vue 模板类型检查 |
| Tauri 通信 | @tauri-apps/api | v1 | IPC invoke / event |

#### 4.6.3 前端项目结构

```
rim_gui/
├── src/                           # Vue 3 前端源码
│   ├── App.vue                    # 根组件
│   ├── main.ts                    # 入口
│   ├── views/
│   │   ├── installer/             # Installer 模式页面流
│   │   │   ├── Home.vue           #   欢迎页 (语言选择)
│   │   │   ├── Configuration.vue  #   安装目录配置
│   │   │   ├── Profile.vue        #   Toolkit 版本选择
│   │   │   ├── Components.vue     #   组件勾选 (工具链/基础库/工具)
│   │   │   ├── PackageSources.vue #   镜像源配置
│   │   │   ├── Confirm.vue        #   安装确认
│   │   │   ├── Install.vue        #   安装进度页
│   │   │   └── Finish.vue         #   安装完成
│   │   └── manager/               # Manager 模式页面
│   │       ├── Overview.vue       #   已安装概览
│   │       ├── Components.vue     #   组件管理
│   │       ├── Update.vue         #   版本更新
│   │       └── Uninstall.vue      #   卸载
│   ├── components/                # 可复用 UI 组件 (13+)
│   │   ├── Button.vue
│   │   ├── Card.vue
│   │   ├── CheckBox.vue
│   │   ├── Input.vue
│   │   ├── Progress.vue
│   │   ├── Select.vue
│   │   └── ...
│   ├── router/                    # 路由配置 (双布局)
│   ├── utils/                     # 工具函数
│   │   ├── commands.ts            #   Tauri invoke 封装
│   │   ├── events.ts              #   Tauri event 监听
│   │   └── locale.ts              #   语言切换
│   └── theme/                     # UnoCSS 主题配置
├── src-tauri/                     # Tauri Rust 后端
│   ├── src/
│   │   ├── main.rs                #   入口: 模式检测 → 启动 Tauri
│   │   ├── installer_mode.rs      #   Installer Tauri Commands
│   │   ├── manager_mode.rs        #   Manager Tauri Commands
│   │   ├── command.rs             #   共享 Commands (配置/语言/版本)
│   │   ├── progress.rs            #   进度事件桥接 (GuiProgress)
│   │   ├── common.rs              #   窗口管理/Manifest 缓存
│   │   ├── consts.rs              #   窗口标签/事件常量
│   │   └── error.rs               #   GUI 错误类型
│   ├── Cargo.toml                 #   Tauri 依赖配置
│   └── tauri.conf.json            #   Tauri 应用配置
└── package.json                   # 前端依赖
```

#### 4.6.4 Tauri IPC 通信设计

GUI 前后端通过 Tauri IPC 进行双向通信，分为 **Command (同步调用)** 和 **Event (异步推送)** 两种模式：

##### Command 调用 (前端 → 后端)

| Command | 模式 | 说明 |
| ------- | ---- | ---- |
| `get_component_list` | Installer | 获取可安装的组件列表 |
| `load_toolkit_manifest` | Installer | 加载 Toolset Manifest (缓存) |
| `validate_install_path` | Installer | 校验安装路径合法性 |
| `start_installation` | Installer | 启动安装流程 |
| `get_installed_kit` | Manager | 获取已安装组件信息 |
| `start_uninstall` | Manager | 启动卸载流程 |
| `check_update` | Manager | 检查 Toolkit / 管理器更新 |
| `get_locale` | 共享 | 获取当前语言设置 |
| `get_app_info` | 共享 | 获取应用版本信息 |

##### Event 推送 (后端 → 前端)

进度通知采用双层事件结构，支持主进度条 + 子进度条：

| Event | 说明 |
| ----- | ---- |
| `progress:main-start` | 主任务开始（总组件数） |
| `progress:main-update` | 主任务进度更新（当前组件序号） |
| `progress:main-end` | 主任务完成 |
| `progress:sub-start` | 子任务开始（如单个文件下载） |
| `progress:sub-update` | 子任务进度更新（下载字节数） |
| `progress:sub-end` | 子任务完成 |

##### 进度抽象层

核心业务层通过 `ProgressHandler` trait 实现 CLI / GUI 双端适配：

```
ProgressHandler (rim_common)
├── CliProgress   → indicatif 进度条 (终端)
└── GuiProgress   → Tauri Event 推送 (WebView)
```

核心安装代码无需感知运行在 CLI 还是 GUI 模式，只通过 trait 接口报告进度。

#### 4.6.5 Manifest 缓存机制

GUI 中 Toolset Manifest 加载后缓存在内存中（`OnceLock<Mutex<ToolkitManifest>>`），避免用户在页面间切换时重复下载。Manifest 在以下情况失效并重新加载：

- 用户切换到不同 Toolkit 版本
- 用户修改 manifest 来源 URL

### 4.7 工具链与组件管理

#### 4.7.1 Rust 基础组件

| 组件 | 说明 | 必选 |
| ---- | ---- | ---- |
| rustc | Rust 编译器 | 是 |
| cargo | 包管理器 | 是 |
| std | 标准库 (仅适配当前环境) | 是 |
| clippy | 静态检查工具 | 是 |
| rustdoc | 文档生成工具 | 是 |
| rustup | 工具链管理器 | 是 |
| rustfmt | 代码格式化工具 | 是 |

#### 4.7.2 优选基础库

发行版集成了旋武社区筛选的高质量 Rust 基础库（ylong 系列），在 Toolkit Manifest 中归属 **"Rust 优选开发库"** 分组，用户可在安装时按需勾选。

| 基础库 | 版本 | 说明 | 默认安装 |
| ------ | ---- | ---- | -------- |
| ylong_http | 1.0.0 | HTTP 协议栈（HTTP/1.1、HTTP/2、HTTP/3、HTTPS） | 可选 |
| ylong_json | 1.0.0 | JSON 序列化/反序列化库 | 可选 |
| ylong_light_actor | 0.1.0 | Actor 并发编程模型与 EventHandler 线程间通信 | 可选 |
| ylong_runtime | 1.0.0 | 异步运行时（任务调度、异步 I/O、同步原语） | 可选 |
| ylong_xml | 0.1.0 | XML 序列化/反序列化库（DOM 结构、Serde 适配） | 可选 |

详细设计见 [第 6 章 优选基础库设计](#6-优选基础库设计)。

#### 4.7.3 优选工具集

| 工具 | 版本 | 说明 | 工具类型 | 默认安装 |
| ---- | ---- | ---- | -------- | -------- |
| cargo-nextest | 0.9.94 | 新一代 Rust 测试运行程序，更快速、界面更简洁 | executables | 可选 |
| coding-guidelines-ruleset | 0.1.0 | Rust 编程规范代码检查规则集 | rule-set | 标准版包含 |
| rust-analyzer | - | LSP 语言服务器 | 工具链组件 | 可选 |
| rust-docs | - | 离线文档 | 工具链组件 | 可选 |
| llvm-tools | - | LLVM 工具集 (覆盖率等) | 工具链组件 | 可选 |

#### 4.7.4 Windows 构建环境依赖安装设计

Rust 在 Windows 平台编译时依赖系统级 C/C++ 链接器与库。发行版针对 MSVC 和 GNU 两种 ABI 环境，分别提供 **Visual Studio Build Tools** 和 **MinGW-w64** 的一键安装，确保用户开箱即用。

##### 问题背景

| ABI 环境 | 编译目标 | 依赖项 | 缺失时的表现 |
| -------- | -------- | ------ | ------------ |
| MSVC | `x86_64-pc-windows-msvc` | MSVC 链接器 (`link.exe`)、C 运行时库、Windows SDK (`kernel32.lib` 等) | `rustc` 链接阶段报错：找不到 `link.exe` 或缺少 `.lib` 文件 |
| GNU | `x86_64-pc-windows-gnu` | GCC (`gcc`)、GNU 链接器 (`ld`)、MinGW 运行时 | `rustc` 链接阶段报错：找不到 `gcc` 或 `ld` |

##### Visual Studio Build Tools（MSVC 环境）

**工具类型**：`kind = "custom"`（自定义安装指令，由 `buildtools.rs` 实现）

**分发来源**：微软官方下载 `https://aka.ms/vs/17/release/vs_BuildTools.exe`

**安装流程**：

```
下载 vs_BuildTools.exe
        │
        ▼
检测 Windows SDK 是否已安装
(检查 LIB 环境变量中是否存在 kernel32.lib)
        │
        ├── 已安装 → 仅安装 MSVC 组件
        │
        └── 未安装 → 安装 MSVC + Windows SDK
                │
                ▼
    调用 vs_BuildTools.exe 执行静默安装
    参数: --wait --nocache --passive --norestart --focusedUi
          --add Microsoft.VisualStudio.Component.VC.Tools.x86.x64
          --add Microsoft.VisualStudio.Component.Windows11SDK.22000
                │
                ▼
        解析退出码，处理安装结果
        │
        ├── 0    → 安装成功
        ├── 3010 → 安装成功，需重启
        └── 其他 → 报告具体错误
                │
                ▼
    将安装程序备份至 {install_dir}/tools/buildtools/
    (用于后续卸载)
```

**安装组件明细**：

| 组件 ID | 说明 | 安装条件 |
| ------- | ---- | -------- |
| `Microsoft.VisualStudio.Component.VC.Tools.x86.x64` | MSVC v143 编译工具集（含 `cl.exe`、`link.exe`） | 始终安装 |
| `Microsoft.VisualStudio.Component.Windows11SDK.22000` | Windows 11 SDK（含系统 `.lib` 导入库） | 仅在未检测到 Windows SDK 时安装 |

**安装检测**：通过 `cc::windows_registry::find_tool()` 查找 `cl.exe`，判断 MSVC 环境是否可用。

**离线安装**：离线模式下附加 `--noWeb` 参数，指示安装程序使用本地已下载的布局文件而不访问网络。

**错误处理**：Build Tools 安装程序定义了丰富的退出码体系，R.I.M 将其映射为可读的错误消息：

| 退出码 | 含义 | 处理方式 |
| ------ | ---- | -------- |
| 0 | 安装成功 | 正常继续 |
| 3010 | 成功，需重启 | 提示用户重启后生效 |
| 1641 | 成功，已发起重启 | 提示用户等待重启 |
| 740 | 需要管理员权限 | 提示用户以管理员身份运行 |
| 1618 | 另一个安装程序正在运行 | 提示用户关闭其他安装进程 |
| 其他 | 参见 VS 安装程序文档 | 显示错误描述 |

##### MinGW-w64（GNU 环境）

**工具类型**：`kind = "dir-with-bin"`（目录型工具，解压后将 `bin/` 添加到 PATH）

**分发来源**：华为云 OBS 镜像托管

```
https://rust-mirror.obs.cn-north-4.myhuaweicloud.com/dist/toolset/mingw64/14.2.0-rt_v12-rev2/
    x86_64-14.2.0-release-posix-seh-ucrt-rt_v12-rev2.7z
```

**安装流程**：

```
下载 MinGW-w64 压缩包 (.7z)
        │
        ▼
解压到 {install_dir}/tools/mingw64/
        │
        ▼
将 mingw64/bin/ 目录添加到系统 PATH
(包含 gcc.exe, g++.exe, ld.exe 等)
        │
        ▼
安装完成，可直接使用 x86_64-pc-windows-gnu 目标
```

**MinGW 版本选型**：

| 属性 | 值 |
| ---- | -- |
| GCC 版本 | 14.2.0 |
| 运行时 | UCRT (Universal C Runtime) |
| 线程模型 | POSIX |
| 异常处理 | SEH (Structured Exception Handling) |

**安装检测**：检查 PATH 中是否同时存在 `gcc.exe` 和 `ld.exe`，两者均存在则判定 MinGW 已安装。

##### Manifest 配置

两种依赖均标记为 `required = true`，在各自目标平台上为必选安装项，归属 **"Rust 基础工具集"** 分组：

```toml
# MSVC 目标 — Build Tools (custom 类型)
[tools.target.x86_64-pc-windows-msvc.buildtools]
required = true
restricted = true
kind = "custom"
url = "https://aka.ms/vs/17/release/vs_BuildTools.exe"

# GNU 目标 — MinGW-w64 (dir-with-bin 类型)
[tools.target.x86_64-pc-windows-gnu.mingw64]
required = true
version = "14.2.0-rt_v12-rev2"
kind = "dir-with-bin"
url = "https://rust-mirror.obs.cn-north-4.myhuaweicloud.com/dist/toolset/mingw64/..."

[tools.group]
"Rust 基础工具集" = ["buildtools", "mingw64"]
```

##### 平台依赖总览

| 平台 | 依赖工具 | 工具类型 | 必选 | 安装方式 |
| ---- | -------- | -------- | :--: | -------- |
| Windows (MSVC) | Visual Studio Build Tools | `custom` | 是 | 调用微软官方安装程序静默安装 |
| Windows (GNU) | MinGW-w64 | `dir-with-bin` | 是 | 解压到 tools/ + 添加 PATH |
| Linux | 系统包 (gcc, pkg-config 等) | - | - | 由用户通过系统包管理器自行安装 |

#### 4.7.5 IDE 集成安装设计

发行版支持 **华为 CodeArts IDE** 和 **VS Code** 两种 IDE 的选择安装，均以 `kind = "custom"` 类型通过自定义安装指令实现。

##### 分发策略

两款 IDE 采用不同的分发来源，以规避开源许可证二次分发问题：

| IDE | 分发来源 | 说明 |
| --- | -------- | ---- |
| CodeArts IDE for Rust | 华为云 OBS（`rust-mirror.obs.cn-north-4.myhuaweicloud.com`） | 由华为官方托管二进制包，可自由分发 |
| VS Code | 微软官方 Release（`code.visualstudio.com`） | 直接从官方下载，**避免二次分发**以符合 VS Code 许可证要求 |

> **二次分发规避说明**：VS Code 的二进制发行版受微软专有许可证约束，不允许第三方重新分发。发行版在 Manifest 中配置官方下载 URL，安装时由用户设备直接从微软服务器获取，确保合规。

##### 平台支持矩阵

| IDE | Windows x86_64 (MSVC) | Windows x86_64 (GNU) | Linux x86_64 | Linux aarch64 |
| --- | :--------------------: | :-------------------: | :----------: | :-----------: |
| CodeArts IDE | 支持 | 支持 | - | - |
| VS Code | 支持 | 支持 | 支持 | 支持 |

CodeArts IDE 当前仅提供 Windows 版本；VS Code 覆盖全部 4 个平台目标。

##### 下载地址配置

**CodeArts IDE**（华为云 OBS）：
```
https://rust-mirror.obs.cn-north-4.myhuaweicloud.com/dist/toolset/codearts/3.2.0/codearts-rust.zip
```

**VS Code**（官方 Release，按平台区分）：

| 平台目标 | 下载 URL |
| -------- | -------- |
| x86_64-pc-windows-msvc | `https://code.visualstudio.com/sha/download?build=stable&os=win32-x64-archive` |
| x86_64-pc-windows-gnu | `https://code.visualstudio.com/sha/download?build=stable&os=win32-x64-archive` |
| x86_64-unknown-linux-gnu | `https://code.visualstudio.com/sha/download?build=stable&os=linux-x64` |
| aarch64-unknown-linux-gnu | `https://code.visualstudio.com/sha/download?build=stable&os=linux-arm64` |

##### 共享安装架构（VSCodeInstaller 复用）

CodeArts IDE 与 VS Code 共享同一安装逻辑。核心通过 `VSCodeInstaller` 结构体实现代码复用，仅参数不同：

```rust
pub(crate) struct VSCodeInstaller<'a> {
    pub(crate) cmd: &'a str,           // 命令行调用名 (code / codearts-rust)
    pub(crate) tool_name: &'a str,     // 工具标识 (vscode / codearts-rust)
    pub(crate) shortcut_name: &'a str, // 桌面快捷方式名称
    pub(crate) binary_name: &'a str,   // 主程序二进制文件名
}
```

各 IDE 实例化配置：

| 字段 | VS Code | CodeArts IDE |
| ---- | ------- | ------------ |
| cmd | `code` | `codearts-rust` |
| tool_name | `vscode` | `codearts-rust` |
| shortcut_name | `Visual Studio Code` | `CodeArts IDE for Rust` |
| binary_name | `Code` (Win) / `code` (Linux) | `codearts-rust` |

##### 安装流程

```
下载 IDE 压缩包 (zip/tar.gz)
        │
        ▼
   解压到临时目录
        │
        ▼
移动到 {install_dir}/tools/{tool_name}/
        │
        ▼
将 bin/ 目录添加到系统 PATH
        │
        ▼
设置可执行权限 (Linux: bin/code, Windows: Code.exe)
        │
        ▼
创建桌面快捷方式 (可选，失败不阻断安装)
```

- **快捷方式创建**：Windows 下创建桌面 `.lnk`，Linux 下生成 `.desktop` 文件到 `~/.local/share/applications/`，包含图标路径和 MIME 类型关联
- **安装检测**：通过 `cmd_exist()` 检查命令是否在 PATH 中，用于判断 IDE 是否已安装

##### 卸载流程

1. 从系统 PATH 中移除 `{install_dir}/tools/{tool_name}/bin`
2. 删除桌面快捷方式（Linux 下检查 `.desktop` 文件是否由 R.I.M 生成后再删除）
3. 删除 `{install_dir}/tools/{tool_name}/` 目录

> 注：用户配置目录（如 `~/.vscode`）不会被自动删除，需由用户自行决定是否清理。

##### Manifest 分组配置

在 Toolset Manifest 中，两款 IDE 归属同一分组，用户可在安装界面按组勾选：

```toml
[tools.group]
IDE = ["codearts-rust", "vscode"]
```

### 4.8 配置文件与清单系统

#### 4.8.1 配置文件全景

```
配置文件体系
├── 构建时配置
│   ├── configuration.toml       # 编译时嵌入的默认配置 (镜像地址、产品标识)
│   └── Cargo.toml               # 工作空间与依赖配置
│
├── 分发端配置 (服务器)
│   ├── dist-manifest.toml       # 分发清单 (可用 Toolkit 版本列表)
│   ├── toolset-manifest.toml    # 工具包清单 (工具链 + 工具定义)
│   └── release.toml             # 管理器最新版本信息
│
└── 客户端配置 (安装后)
    ├── .fingerprint.toml        # 安装记录 (已安装组件追踪)
    ├── cargo/config.toml        # Cargo 配置 (镜像源、补丁)
    └── toolset-manifest.toml    # 本地工具包清单副本
```

#### 4.8.2 Toolset Manifest 结构

```toml
name = "Rust 中国社区发行版"
version = "stable v1.87.0"
edition = "community"

rustup-dist-server = "https://mirror.xuanwu.openatom.cn/"
rustup-update-root = "https://mirror.xuanwu.openatom.cn/rustup"

[cargo-registry]
name = "xuanwu-sparse"
index = "sparse+https://mirror.xuanwu.openatom.cn/index/"

[toolchain]
channel = "1.87.0"
profile = "minimal"
display-name = "Rust 官方工具"
description = "Rust 官方工具链，包含 rustc (编译器), rust-std (标准库), cargo (包管理) 等工具"
components = ["clippy", "rustfmt", "rust-src"]
optional-components = ["llvm-tools", "rust-docs", "rust-analyzer"]
group = "Rust 基础工具集"

# === Windows 构建环境依赖 ===

# MSVC 环境 — VS Build Tools (custom 类型，必选)
[tools.target.x86_64-pc-windows-msvc.buildtools]
required = true
restricted = true
default = "https://aka.ms/vs/17/release/vs_BuildTools.exe"
kind = "custom"
display-name = "Visual Studio Build Tools"

# GNU 环境 — MinGW-w64 (dir-with-bin 类型，必选)
[tools.target.x86_64-pc-windows-gnu.mingw64]
required = true
version = "14.2.0-rt_v12-rev2"
url = "https://rust-mirror.obs.cn-north-4.myhuaweicloud.com/dist/toolset/mingw64/14.2.0-rt_v12-rev2/x86_64-14.2.0-release-posix-seh-ucrt-rt_v12-rev2.7z"
kind = "dir-with-bin"
display-name = "MinGW-w64"

# === IDE 工具 (Custom 类型) ===

# CodeArts IDE - 从华为云 OBS 下载
[tools.target.x86_64-pc-windows-msvc.codearts-rust]
version = "3.2.0"
url = "https://rust-mirror.obs.cn-north-4.myhuaweicloud.com/dist/toolset/codearts/3.2.0/codearts-rust.zip"
kind = "custom"
display-name = "CodeArts IDE"

# VS Code - 从官方 Release 下载 (避免二次分发)
[tools.target.x86_64-pc-windows-msvc.vscode]
optional = true
version = "1.105.1"
url = "https://code.visualstudio.com/sha/download?build=stable&os=win32-x64-archive"
kind = "custom"
display-name = "Visual Studio Code"

# === 优选工具 ===

[tools.target.x86_64-pc-windows-msvc.cargo-nextest]
optional = true
version = "0.9.94"
url = "https://rust-mirror.obs.cn-north-4.myhuaweicloud.com/dist/toolset/cargo-nextest/0.9.94/cargo-nextest-0.9.94-x86_64-pc-windows-msvc.zip"
kind = "executables"

# === 代码检查工具集 (RuleSet 类型) ===

[tools.target.x86_64-pc-windows-msvc.coding-guidelines-ruleset]
version = "0.1.0"
url = "https://rust-mirror.obs.cn-north-4.myhuaweicloud.com/dist/toolset/coding-guidelines-ruleset/0.1.0/rust-1.74.0-x86_64-pc-windows-msvc.tar.xz"
kind = "rule-set"
display-name = "编程规范规则集"
requires = ["rust"]

# === 优选基础库 (Crate 类型) ===

[tools.target.x86_64-pc-windows-msvc.ylong_json]
optional = true
version = "1.0.0"
url = "https://rust-mirror.obs.cn-north-4.myhuaweicloud.com/dist/toolset/ylong_json/1.0.0/commonlibrary_rust_ylong_json-master.zip"
kind = "crate"

[tools.target.x86_64-pc-windows-msvc.ylong_light_actor]
optional = true
version = "0.1.0"
url = "https://rust-mirror.obs.cn-north-4.myhuaweicloud.com/dist/toolset/ylong_light_actor/0.1.0/commonlibrary_rust_ylong_light_actor-master.zip"
kind = "crate"

[tools.target.x86_64-pc-windows-msvc.ylong_xml]
optional = true
version = "0.1.0"
url = "https://rust-mirror.obs.cn-north-4.myhuaweicloud.com/dist/toolset/ylong_xml/0.1.0/commonlibrary_rust_ylong_xml-master.zip"
kind = "crate"

[tools.target.x86_64-pc-windows-msvc.ylong_http]
optional = true
version = "1.0.0"
url = "https://rust-mirror.obs.cn-north-4.myhuaweicloud.com/dist/toolset/ylong_http/1.0.0/commonlibrary_rust_ylong_http-master.zip"
kind = "crate"

[tools.target.x86_64-pc-windows-msvc.ylong_runtime]
optional = true
version = "1.0.0"
url = "https://rust-mirror.obs.cn-north-4.myhuaweicloud.com/dist/toolset/ylong_runtime/1.0.0/commonlibrary_rust_ylong_runtime-master.zip"
kind = "crate"

[tools.descriptions]
buildtools = "Visual Studio 构建工具，对于 msvc 目标平台提供链接器、库以及 Windows API 导入库"
mingw64 = "编译器在 x86_64 Windows GNU 环境下的依赖组件"
codearts-rust = "CodeArts IDE for Rust (华为云 OBS 分发)"
vscode = "Visual Studio Code 轻量级源代码编辑器"
cargo-nextest = "新一代 Rust 项目测试运行程序，相比 cargo test 更快速、界面更简洁"
coding-guidelines-ruleset = "Rust 编程规范的代码检查规则集"
ylong_json = "JSON 序列化/反序列化库"
ylong_light_actor = "Actor 并发编程模型与 EventHandler 线程间通信机制"
ylong_xml = "XML 序列化/反序列化库"
ylong_http = "HTTP 协议栈 (HTTP/1.1, HTTP/2, HTTP/3)"
ylong_runtime = "异步运行时 (任务调度、异步 I/O、同步原语)"

[tools.group]
"Rust 基础工具集" = ["mingw64", "buildtools"]
"Rust 优选工具集" = ["cargo-nextest"]
IDE = ["codearts-rust", "vscode"]
"Rust 代码检查工具集" = ["coding-guidelines-ruleset"]
"Rust 优选开发库" = ["ylong_json", "ylong_light_actor", "ylong_xml", "ylong_http", "ylong_runtime"]
```

#### 4.8.3 安装记录结构 (InstallationRecord)

安装记录持久化到 `~/.rim/install-record.toml`，记录当前安装状态：

```toml
install_dir = "C:\\Users\\{user}\\.rust"
name = "Rust 中国社区发行版"
version = "stable v1.87.0"
edition = "community"

[rust]
version = "1.87.0"
components = ["clippy", "rustfmt", "rust-src"]

[tools]
buildtools = { kind = "Custom", paths = ["{install_dir}/tools/buildtools"], version = "17.0" }
mingw64 = { kind = "DirWithBin", paths = ["{install_dir}/tools/mingw64"], version = "14.2.0-rt_v12-rev2" }
codearts-rust = { kind = "Custom", paths = ["{install_dir}/tools/codearts-rust"], version = "3.2.0" }
vscode = { kind = "Custom", paths = ["{install_dir}/tools/vscode"], version = "1.105.1" }
cargo-nextest = { kind = "Executables", paths = ["{install_dir}/cargo/bin/cargo-nextest"], version = "0.9.94" }
coding-guidelines-ruleset = { kind = "RuleSet", paths = [...], version = "0.1.0", dependencies = ["rust"] }
ylong_json = { kind = "Crate", paths = ["{install_dir}/crates/ylong_json"], version = "1.0.0" }
ylong_light_actor = { kind = "Crate", paths = ["{install_dir}/crates/ylong_light_actor"], version = "0.1.0" }
ylong_http = { kind = "Crate", paths = ["{install_dir}/crates/ylong_http"], version = "1.0.0" }
ylong_runtime = { kind = "Crate", paths = ["{install_dir}/crates/ylong_runtime"], version = "1.0.0" }
ylong_xml = { kind = "Crate", paths = ["{install_dir}/crates/ylong_xml"], version = "0.1.0" }
```

**核心数据结构**：

```rust
pub struct InstallationRecord {
    pub install_dir: PathBuf,
    pub name: Option<String>,           // Toolkit 名称
    pub version: Option<String>,        // Toolkit 版本
    pub edition: Option<String>,        // Toolkit 版本标识
    pub rust: Option<RustRecord>,       // Rust 工具链记录
    pub tools: HashMap<String, ToolRecord>,  // 已安装工具记录
}

pub struct ToolRecord {
    kind: ToolKind,                     // 工具类型
    version: Option<String>,            // 版本号
    pub paths: Vec<PathBuf>,            // 安装路径列表
    pub dependencies: Vec<String>,      // 依赖的其他工具
}
```

#### 4.8.4 清单加载优先级

1. `--manifest` 参数指定的路径或 URL
2. 安装目录中的本地 `toolset-manifest.toml`（更新场景）
3. 从服务器 `dist/dist-manifest.toml` 获取最新清单 URL 并下载
4. 编译时内置的默认清单（通过 `EDITION` 环境变量选择 edition）

**Edition 选择**：构建时通过 `EDITION` 环境变量控制内嵌的 Manifest 模板：

| EDITION 值 | 对应 Manifest | 说明 |
| ---------- | ------------- | ---- |
| `community` | `resources/toolkit-manifest/{mode}/community.toml` | 社区发行版（生产） |
| `bisheng` | `resources/toolkit-manifest/{mode}/bisheng.toml` | 毕昇发行版 |
| `test` (默认) | `resources/toolkit-manifest/{mode}/test.toml` | 测试版 |

---

## 5. Rust 语言原生组件

发行版通过旋武社区 Rustup 镜像（`mirror.xuanwu.openatom.cn`）为用户提供 Rust 语言官方原生组件的二进制分发，用户无需访问国外服务器即可完成工具链安装与更新。

### 5.1 镜像分发机制

R.I.M 安装器在安装过程中自动配置以下环境变量，将 Rustup 的组件下载源指向旋武社区镜像：

| 环境变量 | 值 | 作用 |
| -------- | -- | ---- |
| `RUSTUP_DIST_SERVER` | `https://mirror.xuanwu.openatom.cn` | Rust 工具链二进制包的下载服务器 |
| `RUSTUP_UPDATE_ROOT` | `https://mirror.xuanwu.openatom.cn/rustup` | Rustup 自身更新的下载地址 |

安装流程：

```
R.I.M 安装器
    │
    ├── 1. 下载 rustup-init（从镜像或离线包）
    ├── 2. 配置 RUSTUP_DIST_SERVER / RUSTUP_UPDATE_ROOT 环境变量
    │      设置 RUSTUP_INIT_SKIP_PATH_CHECK=yes
    │      移除 RUSTUP_TOOLCHAIN (避免冲突)
    ├── 3. 执行 rustup toolchain install <version> --no-self-update
    │      --profile minimal -c <components>
    │      （从镜像下载工具链 + 组件二进制）
    ├── 4. 执行 rustup default <version> （设为默认工具链）
    └── 5. 移除 rustup 卸载注册表项 (仅 Windows 首次安装)
```

> **Profile 说明**：发行版使用 `minimal` profile 作为基础，在此之上通过 `--component` 参数显式指定需要安装的组件（如 clippy、rustfmt、rust-src），实现精确控制。

离线模式下，工具链二进制包预先打包在离线安装包中，通过 `offline-dist-server` 配置指向本地路径，无需网络访问。

### 5.2 原生组件列表

#### 5.2.1 rustc — Rust 编译器

Rust 语言核心编译器，将 `.rs` 源代码编译为目标平台的二进制文件。支持增量编译、交叉编译等能力，是所有 Rust 项目构建的基础。

安装后可在命令行使用 `rustc` 命令。更多使用方法参见 [《The rustc book》](https://doc.rust-lang.org/rustc/)。

#### 5.2.2 rust-std — Rust 标准库

Rust 标准库提供了语言基本类型（`String`、`Vec`、`HashMap` 等）、I/O 操作、多线程、网络等软件开发基础功能 API。标准库以预编译二进制形式分发，与 `rustc` 版本严格匹配。

发行版仅安装适配当前安装环境的标准库。若存在交叉编译需求，可通过 `rustup target add <target>` 安装其他目标平台的标准库。

更多标准库 API 参见 [Rust 标准库官方文档](https://doc.rust-lang.org/std/index.html)。

#### 5.2.3 cargo — 包管理器

Rust 官方包管理器与构建系统，负责项目依赖解析、下载、编译及整体构建编排。支持工作空间、Feature 条件编译、自定义构建脚本等高级功能。

安装后可在命令行使用 `cargo build`、`cargo run`、`cargo test` 等命令。更多使用方法参见 [《The Cargo Book》](https://doc.rust-lang.org/cargo/index.html)。

#### 5.2.4 clippy — 静态检查工具

Rust 官方代码质量检查工具，内置 700+ 条 lint 规则，覆盖正确性、性能、风格、复杂度等维度，在编译阶段以 warning 或 error 形式提示潜在问题。

安装后通过 `cargo clippy` 命令调用。更多使用方法参见 [《Clippy Documentation》](https://doc.rust-lang.org/clippy/)。

#### 5.2.5 rustfmt — 代码格式化工具

Rust 官方代码格式化工具，根据社区统一的编码风格规则自动格式化代码，保证项目内代码风格一致性。支持通过 `rustfmt.toml` 进行规则定制。

安装后通过 `cargo fmt` 命令调用。

#### 5.2.6 rustdoc — 文档生成工具

Rust 官方文档生成器，通过解析代码中的 `///` 和 `//!` 注释自动生成 HTML 格式的 API 文档，并支持文档内代码示例的自动编译测试（doc-test）。

安装后通过 `cargo doc` 命令调用。更多使用方法参见 [《The rustdoc book》](https://doc.rust-lang.org/rustdoc/what-is-rustdoc.html)。

#### 5.2.7 rustup — 工具链管理器

Rust 官方工具链安装与版本管理工具，支持在 stable / beta / nightly 频道间切换、安装多个工具链版本并行使用、添加交叉编译目标等。

安装后通过 `rustup` 命令调用。发行版已自动将其下载源指向旋武镜像，后续更新同样使用国内服务器。更多使用方法参见 [《The rustup book》](https://rust-lang.github.io/rustup/)。

### 5.3 可选原生组件

以下组件在安装时可由用户按需勾选：

| 组件 | 说明 | 默认安装 |
| ---- | ---- | :------: |
| rust-analyzer | LSP 语言服务器实现，为 IDE 提供代码补全、跳转定义、重构等功能 | 可选 |
| rust-docs | Rust 官方文档离线包，支持 `rustup doc` 命令本地浏览，无需联网 | 可选 |
| llvm-tools | LLVM 工具集合，含代码覆盖率统计（`llvm-profdata`、`llvm-cov`）等工具 | 可选 |
| rust-src | Rust 标准库及编译器源码，供 rust-analyzer 分析及 IDE 内源码跳转使用 | 可选 |

### 5.4 组件安装配置

在 Toolset Manifest 中，原生组件通过 `[toolchain]` 段进行配置：

```toml
[toolchain]
channel = "1.87.0"                                    # 工具链版本
profile = "minimal"                                   # Rustup profile (最小化基础)
display-name = "Rust 官方工具"
description = "Rust 官方工具链，包含 rustc (编译器), rust-std (标准库), cargo (包管理) 等工具"
components = ["clippy", "rustfmt", "rust-src"]        # 默认安装组件
optional-components = ["llvm-tools", "rust-docs",     # 可选组件
                       "rust-analyzer"]
group = "Rust 基础工具集"
```

安装器根据用户选择，调用 `rustup toolchain install` 命令并通过 `--component` 参数指定需要安装的组件列表。组件的添加和移除在 Manager 模式下同样通过 Rustup 命令完成，无需重新安装整个工具链。

### 5.5 编译后端抽屉式替换设计

#### 5.5.1 设计目标

旋武发行版采用编译后端抽屉式替换架构，支持在不改变上层工具链（rustc、cargo、clippy 等）的前提下，将底层 LLVM 代码生成后端替换为毕昇编译器后端（[Bisheng LLVM](https://gitcode.com/xuanwu/bisheng)），从而在主流 ARM 芯片（鲲鹏等）上获得更优的编译产物运行性能。

#### 5.5.2 架构原理

Rust 编译器 rustc 的代码生成流程为：Rust 源码 → HIR → MIR → LLVM IR → 目标机器码。其中 LLVM IR 到机器码的转换由 LLVM 后端完成，这一层与上层 Rust 语义分析完全解耦，具备独立替换的可行性：

```
┌─────────────────────────────────────────────────────────┐
│                    Rust 编译流程                          │
│                                                         │
│  .rs 源码 → HIR → MIR → LLVM IR → 机器码 (.o/.exe)     │
│                                    ▲                    │
│                                    │                    │
│                         ┌──────────┴──────────┐         │
│                         │   LLVM 后端 (可替换)  │         │
│                         ├─────────────────────┤         │
│                         │ ● 官方 LLVM (默认)    │         │
│                         │ ● 毕昇 LLVM (ARM 优化)│         │
│                         └─────────────────────┘         │
└─────────────────────────────────────────────────────────┘
```

抽屉式替换的核心在于：rustc 通过动态链接或静态链接方式调用 LLVM 库，只要替换的 LLVM 版本保持 API/ABI 兼容，即可无缝切换后端而无需重新编译 rustc 本身。

#### 5.5.3 毕昇编译器后端

毕昇 LLVM（[https://gitcode.com/xuanwu/bisheng](https://gitcode.com/xuanwu/bisheng)）是华为基于上游 LLVM 开发的优化版本，在保持完全兼容的前提下针对 ARM 架构进行深度优化：

| 优化维度 | 具体内容 |
| -------- | -------- |
| 向量化增强 | 改进的自动向量化策略，更好地利用 ARM NEON/SVE 指令集 |
| 指令调度 | 针对鲲鹏微架构的指令调度模型，优化流水线利用率 |
| 内存访问 | ARM 平台内存访问模式优化，减少 cache miss |
| 分支预测 | 基于 ARM 分支预测器特性的代码布局优化 |
| 循环优化 | 增强的循环展开和软件流水线策略 |

#### 5.5.4 替换机制实现

发行版通过 Toolset Manifest 的 edition 机制支持不同编译后端的分发：

```
工具链分发架构
├── community edition (社区版)
│   └── 官方 LLVM 后端 (默认)
│       └── rustc + 标准 LLVM → 通用性能
│
└── bisheng edition (毕昇版)
    └── 毕昇 LLVM 后端
        └── rustc + 毕昇 LLVM → ARM 性能优化
```

替换流程对用户透明：

| 步骤 | 操作 | 说明 |
| ---- | ---- | ---- |
| 1 | 选择毕昇版本 | 安装时选择 bisheng edition 或后续切换 |
| 2 | 下载毕昇工具链 | 从镜像下载基于毕昇 LLVM 构建的 rustc 二进制 |
| 3 | 替换工具链 | 通过 rustup toolchain 机制安装为自定义工具链 |
| 4 | 验证兼容性 | 所有 cargo build/test/clippy 命令行为不变 |

#### 5.5.5 兼容性保证

| 保证项 | 说明 |
| ------ | ---- |
| 源码兼容 | 任何能用官方 rustc 编译的代码，毕昇版本同样可编译 |
| 行为一致 | 编译产物的功能行为与官方版本完全一致 |
| 工具链兼容 | cargo、clippy、rustfmt 等上层工具无需任何修改 |
| 回退能力 | 可随时通过 `rustup default` 切换回官方工具链 |

毕昇版本的差异仅体现在编译产物的运行时性能上——相同的 Rust 源码，经毕昇 LLVM 编译后在 ARM 平台上可获得更优的执行效率，而在 x86 平台上性能与官方版本持平。

---

## 6. 优选基础库设计

### 6.1 概述

旋武社区优选基础库是一组经过社区筛选和验证的高质量 Rust 基础 crate，涵盖网络协议、数据格式、异步运行时等常用领域。

#### 6.1.1 核心问题：本地源码库如何融入 Cargo 生态

优选基础库以**源码形式**分发到用户本地，而非发布到 crates.io 公共仓库。这带来了一个核心难题：

> Cargo 默认从 crates.io（或配置的 registry）解析依赖。用户在 `Cargo.toml` 中声明 `ylong_json = "1.0.0"` 后，Cargo 会尝试从远程 registry 下载，但优选基础库并不存在于公共 registry 中。如何让用户像使用普通 crate 一样使用本地源码安装的基础库？

常见的替代方案及其缺陷：

| 方案 | 做法 | 缺陷 |
| ---- | ---- | ---- |
| **path 依赖** | `ylong_json = { path = "/home/user/.rust/crates/ylong_json" }` | 硬编码绝对路径，不同机器路径不同，`Cargo.toml` 无法跨团队共享，CI 环境需额外适配 |
| **git 依赖** | `ylong_json = { git = "https://gitee.com/..." }` | 依赖网络访问，离线环境不可用；构建速度慢（需 clone 仓库）；版本锁定不灵活 |
| **私有 registry** | 搭建私有 crates registry 服务 | 运维成本高，需独立服务器和认证体系，对中小团队和个人开发者不友好 |
| **vendored 依赖** | 将源码直接放入项目的 vendor 目录 | 侵入用户项目结构，每个项目都需复制，更新困难 |

#### 6.1.2 解决方案：Cargo Patch 全局补丁注入

发行版采用 Cargo 官方提供的 **`[patch.crates-io]`** 机制，在全局 Cargo 配置中注入补丁，将 crates.io 上的 crate 名称**透明重定向**到本地源码路径。这是 Cargo 原生支持的依赖覆盖能力，专为"本地替代远程 crate"场景设计。

**核心原理**：

```
用户项目 Cargo.toml                  全局 Cargo 配置
┌──────────────────────┐           ┌─────────────────────────────────────┐
│ [dependencies]       │           │ # {CARGO_HOME}/config.toml          │
│ ylong_json = "1.0.0" │  ──────>  │ [patch.crates-io]                   │
│                      │  Cargo    │ ylong_json = { path = "...crates/   │
│                      │  解析时   │   ylong_json" }                     │
└──────────────────────┘  自动拦截  └─────────────────────────────────────┘
                                              │
                                              ▼
                                   {install_dir}/crates/ylong_json/
                                   ├── Cargo.toml    (name = "ylong_json")
                                   └── src/
                                       └── lib.rs    (本地源码)
```

当 Cargo 解析到 `ylong_json = "1.0.0"` 时，发现 `[patch.crates-io]` 中存在同名条目，会**优先使用本地路径的源码**编译，而非从远程 registry 下载。

**关键优势**：

| 优势 | 说明 |
| ---- | ---- |
| **对用户项目零侵入** | 用户 `Cargo.toml` 中只需写标准的 `ylong_json = "1.0.0"`，无需 path/git 特殊语法，项目文件可正常跨团队共享 |
| **全局一次配置** | 补丁写入 `{CARGO_HOME}/config.toml`，对该用户所有 Rust 项目自动生效，无需逐项目配置 |
| **完全离线可用** | 源码已在本地 `crates/` 目录，构建时无需任何网络访问 |
| **版本语义兼容** | Cargo 要求 patch 的 crate 版本号与被替代版本兼容，保证类型系统一致性 |
| **Cargo 原生机制** | 不引入任何自定义构建系统或非标准工具链修改，完全遵循 Cargo 官方规范 |
| **安装/卸载可控** | 安装管理工具自动写入和清除补丁配置，用户无需手动编辑配置文件 |

### 6.2 基础库详细信息

#### 6.2.1 ylong_http — HTTP 协议栈

| 属性 | 说明 |
| ---- | ---- |
| 版本 | 1.0.0 |
| 许可证 | Apache License 2.0 |
| 源码仓库 | https://gitee.com/openharmony/commonlibrary_rust_ylong_http |
| 工作空间成员 | `ylong_http`（协议组件）、`ylong_http_client`（HTTP 客户端） |

**功能特性**：

- HTTP/1.1、HTTP/2、HTTP/3 协议支持
- HTTPS (OpenSSL) 加密传输
- 同步与异步两种客户端实现
- 可选后端：基于 `tokio` 或 `ylong_runtime`

**Feature Flags**：

| Feature | 说明 |
| ------- | ---- |
| `http1_1` | 启用 HTTP/1.1 协议支持 |
| `http2` | 启用 HTTP/2 协议支持 |
| `http3` | 启用 HTTP/3 协议支持 |
| `tokio_base` | 使用 tokio 作为异步运行时后端 |
| `ylong_base` | 使用 ylong_runtime 作为异步运行时后端 |

**依赖关系**：

```
ylong_http_client
├── ylong_http (协议核心)
├── ylong_runtime (可选，异步后端)
├── tokio (可选，异步后端)
└── openssl / quiche (HTTPS/HTTP3)
```

#### 6.2.2 ylong_json — JSON 解析库

| 属性 | 说明 |
| ---- | ---- |
| 版本 | 1.0.0 |
| 许可证 | Apache License 2.0 |
| 源码仓库 | https://gitee.com/openharmony-sig/commonlibrary_rust_ylong_json |

**功能特性**：

- JSON 序列化与反序列化
- 多种后端数据结构支持（Vec、LinkedList、BTreeMap）
- Serde trait 适配
- C FFI 适配器支持（可选）

**Feature Flags**：

| Feature | 说明 |
| ------- | ---- |
| `vec_array` | 使用 Vec 作为 JSON Array 后端 |
| `list_array` | 使用 LinkedList 作为 JSON Array 后端 |
| `btree_object` | 使用 BTreeMap 作为 JSON Object 后端 |
| `list_object` | 使用 LinkedList 作为 JSON Object 后端 |
| `c_adapter` | 启用 C FFI 适配器 |
| `ascii_only` | 仅处理 ASCII 字符 |

#### 6.2.3 ylong_light_actor — Actor 并发编程模型

| 属性 | 说明 |
| ---- | ---- |
| 版本 | 0.1.0 |
| 许可证 | Apache License 2.0 |
| 源码仓库 | https://gitee.com/openharmony-sig/commonlibrary_rust_ylong_light_actor |

**功能特性**：

- **Actor 编程模型**：一种并发编程模型，旨在解决传统内存共享模型带来的数据竞争、加锁导致的性能损失及死锁等问题。每个 Actor 拥有独立状态，通过消息传递进行通信
- **EventHandler 机制**：提供线程间通信机制，可创建新线程将耗时操作放到新线程执行，既不阻塞原线程，任务又可得到合理处理

**适用场景**：

- 高并发消息处理系统
- 需要避免共享状态的并发设计
- 异步事件驱动架构
- 线程间解耦通信

#### 6.2.4 ylong_runtime — 异步运行时

| 属性 | 说明 |
| ---- | ---- |
| 许可证 | Apache License 2.0 |
| 源码仓库 | https://gitee.com/openharmony/commonlibrary_rust_ylong_runtime |
| 工作空间成员 | `ylong_runtime`、`ylong_runtime_macros`、`ylong_io`、`ylong_signal`、`ylong_ffrt` |

**功能特性**：

- 异步任务调度与执行
- 异步 I/O（网络、文件）
- 同步原语（Mutex、RwLock、Semaphore 等）
- 定时器与信号处理
- 双执行器支持：Rust 原生执行器 与 FFRT (Function Flow Runtime) 执行器

**子 crate 说明**：

| 子 crate | 说明 |
| -------- | ---- |
| ylong_runtime | 运行时核心：任务调度、异步执行 |
| ylong_runtime_macros | 过程宏（`#[ylong_runtime::main]` 等） |
| ylong_io | 异步 I/O 抽象层 |
| ylong_signal | 异步信号处理 |
| ylong_ffrt | FFRT 执行器桥接层 |

#### 6.2.5 ylong_xml — XML 解析库

| 属性 | 说明 |
| ---- | ---- |
| 版本 | 0.1.0 |
| 许可证 | Apache License 2.0 |

**功能特性**：

- XML 序列化与反序列化
- DOM 树结构支持
- Serde trait 适配

### 6.3 Cargo Patch 机制详解

#### 6.3.1 Cargo Patch 工作原理

Cargo 的 `[patch]` 功能允许在依赖解析阶段，将某个 registry 来源的 crate **替换**为本地路径或 git 仓库的版本。发行版利用此机制，在全局配置 `{CARGO_HOME}/config.toml` 中声明 `[patch.crates-io]`，实现所有用户项目的透明依赖替换。

**Cargo 依赖解析流程（含 patch）**：

```
cargo build
  │
  ├── 1. 读取项目 Cargo.toml
  │      → 发现依赖: ylong_json = "1.0.0"
  │
  ├── 2. 读取全局配置 {CARGO_HOME}/config.toml
  │      → 发现 [patch.crates-io]:
  │         ylong_json = { path = "{install_dir}/crates/ylong_json" }
  │
  ├── 3. 版本兼容性检查
  │      → 本地 crate 的 Cargo.toml 中 version = "1.0.0"
  │      → 与项目要求的 "1.0.0" 语义兼容 ✓
  │
  ├── 4. 替换解析结果
  │      → 不再从 crates.io registry 下载 ylong_json
  │      → 改为使用本地路径 {install_dir}/crates/ylong_json/ 的源码
  │
  └── 5. 编译本地源码
         → 从本地 src/ 目录读取 .rs 文件编译
         → 完全离线，无需网络
```

**版本兼容性规则**：Cargo 要求 patch 中的 crate 版本号与被替代的版本语义兼容（semver）。例如，项目声明 `ylong_json = "1.0.0"` 时，patch 中的本地 crate 版本必须为 `1.x.y`。若版本不兼容，Cargo 会报错并忽略该 patch。

#### 6.3.2 全局配置文件生成

安装管理工具在安装优选基础库后，自动向 `{CARGO_HOME}/config.toml` 追加 patch 配置段。该文件是 Cargo 的**全局配置**，对当前用户的所有 Rust 项目自动生效。

**安装后生成的配置示例**：

```toml
# {CARGO_HOME}/config.toml
# ── 由旋武发行版安装管理工具自动生成 ──

# 镜像源配置
[source.mirror]
registry = "sparse+https://mirror.xuanwu.openatom.cn/index/"

[source.crates-io]
replace-with = "mirror"

# ── 优选基础库本地补丁 ──
[patch.crates-io]
# JSON 解析库
ylong_json = { path = "{install_dir}/crates/ylong_json" }

# XML 解析库
ylong_xml = { path = "{install_dir}/crates/ylong_xml" }

# HTTP 协议栈 (workspace 中的多个子 crate 需分别声明)
ylong_http = { path = "{install_dir}/crates/ylong_http/ylong_http" }
ylong_http_client = { path = "{install_dir}/crates/ylong_http/ylong_http_client" }

# 异步运行时 (workspace 中的多个子 crate 需分别声明)
ylong_runtime = { path = "{install_dir}/crates/ylong_runtime/ylong_runtime" }
ylong_runtime_macros = { path = "{install_dir}/crates/ylong_runtime/ylong_runtime_macros" }
ylong_io = { path = "{install_dir}/crates/ylong_runtime/ylong_io" }
ylong_signal = { path = "{install_dir}/crates/ylong_runtime/ylong_signal" }
ylong_ffrt = { path = "{install_dir}/crates/ylong_runtime/ylong_ffrt" }
```

> **Workspace 子 crate 处理**: `ylong_http` 和 `ylong_runtime` 是 Cargo Workspace，包含多个子 crate。Cargo patch 需要为每个可被外部依赖的子 crate **逐一声明** patch 条目，否则当用户项目 `Cargo.toml` 中引用某个子 crate（如 `ylong_http_client`）时，Cargo 仍会尝试从远程下载。

#### 6.3.3 安装流程

优选基础库在安装管理工具中属于 **Crate** 类型工具（`kind = "crate"`），安装流程与普通可执行工具不同：

```
用户勾选优选基础库
  │
  ├── 1. 获取基础库 ZIP 压缩包
  │      ├── 在线模式: 从分发服务器下载
  │      └── 离线模式: 从本地 packages/ 目录读取
  │
  ├── 2. 解压到 {install_dir}/crates/ 目录
  │      (处理嵌套 ZIP: 外层 ZIP 解压后内部可能还有一层 ZIP)
  │      解压后目录结构:
  │
  │      {install_dir}/crates/
  │      ├── ylong_json/                    # 单 crate
  │      │   ├── Cargo.toml                 #   name = "ylong_json", version = "1.0.0"
  │      │   └── src/
  │      ├── ylong_xml/                     # 单 crate
  │      │   ├── Cargo.toml                 #   name = "ylong_xml", version = "0.1.0"
  │      │   └── src/
  │      ├── ylong_http/                    # Workspace
  │      │   ├── Cargo.toml                 #   [workspace] members
  │      │   ├── ylong_http/                #   协议核心 crate
  │      │   │   ├── Cargo.toml
  │      │   │   └── src/
  │      │   └── ylong_http_client/         #   HTTP 客户端 crate
  │      │       ├── Cargo.toml
  │      │       └── src/
  │      └── ylong_runtime/                 # Workspace
  │          ├── Cargo.toml                 #   [workspace] members
  │          ├── ylong_runtime/             #   运行时核心
  │          ├── ylong_runtime_macros/      #   过程宏
  │          ├── ylong_io/                  #   异步 I/O
  │          ├── ylong_signal/              #   信号处理
  │          └── ylong_ffrt/                #   FFRT 桥接
  │
  ├── 3. 生成 Cargo Patch 配置
  │      → 读取每个 crate 的 Cargo.toml 获取 name 和 path
  │      → 对 workspace 类型递归扫描所有子 crate
  │      → 在 {CARGO_HOME}/config.toml 的 [patch.crates-io] 中追加条目
  │
  └── 4. 写入安装记录
         → 在 .fingerprint.toml 中记录 kind = "Crate" 及路径
```

#### 6.3.4 卸载流程

```
卸载优选基础库
  │
  ├── 1. 删除源码目录
  │      → rm -rf {install_dir}/crates/{lib_name}/
  │
  ├── 2. 移除 Cargo Patch 条目
  │      → 从 {CARGO_HOME}/config.toml 中精确删除该库及其子 crate 的
  │        [patch.crates-io] 条目
  │      → 保留其他优选库的 patch 配置不受影响
  │
  └── 3. 更新安装记录
         → 从 .fingerprint.toml 移除该库记录
```

#### 6.3.5 用户使用方式

安装完成后，用户在**任意** Rust 项目中只需按标准方式声明依赖，即可使用优选基础库：

```toml
# 用户项目 Cargo.toml — 无需任何特殊配置
[dependencies]
ylong_json = "1.0.0"                     # JSON 解析
ylong_http_client = { version = "1.0.0", features = ["http1_1", "ylong_base"] }  # HTTP 客户端
ylong_runtime = "0.1.0"                  # 异步运行时
ylong_xml = "0.1.0"                      # XML 解析
```

```rust
// 用户代码 — 与使用任何普通 crate 完全一致
use ylong_json::JsonValue;

fn main() {
    let json = JsonValue::from_str(r#"{"name": "旋武"}"#).unwrap();
    println!("{}", json["name"]);
}
```

**对用户的透明性**：

- `Cargo.toml` 写法与依赖 crates.io 上的普通 crate 完全一致
- 不需要 `path = "..."` 或 `git = "..."` 等特殊语法
- 项目文件可正常提交 Git、跨团队共享、在 CI 中构建
- 如果团队中其他成员也安装了发行版，同一份 `Cargo.toml` 直接可用
- 如果未安装发行版，且 crate 未来发布到 crates.io，则自动从远程下载（向前兼容）

#### 6.3.6 Patch 与 Registry 镜像的协同

发行版同时配置了 crates.io 镜像源（`[source.crates-io] replace-with = "mirror"`）和 patch。两者的关系是：

```
用户声明依赖 ylong_json = "1.0.0"
  │
  ├── [patch.crates-io] 中存在 ylong_json？
  │   ├── 是 → 使用本地 crates/ 源码 (优先级最高)
  │   └── 否 → 从 mirror 镜像源下载 (国内加速)
  │
  └── 结果: 优选基础库走本地，其他依赖走国内镜像，两者互不冲突
```

`[patch]` 的优先级**高于** `[source]` 的 registry 替换。即使配置了镜像源，已 patch 的 crate 仍从本地解析。这保证了：
- 优选基础库始终使用发行版安装的本地版本
- 项目的其他依赖（如 serde、tokio）仍通过国内镜像加速下载

#### 6.3.7 版本更新策略

当发行版升级优选基础库版本时，安装管理工具的处理流程：

```
Toolkit 更新包含新版基础库 (如 ylong_json 1.0.0 → 1.1.0)
  │
  ├── 1. 下载新版 ZIP 包
  ├── 2. 删除旧版 crates/ylong_json/ 目录
  ├── 3. 解压新版到 crates/ylong_json/
  ├── 4. Cargo Patch 配置中路径不变 (path 指向同一目录)
  │      → [patch.crates-io] 条目无需修改
  └── 5. 用户下次 cargo build 时自动使用新版源码
```

由于 patch 配置的 path 不变，只是目录内容更新，因此**全局配置文件无需修改**，已有用户项目下次构建时会自动使用新版源码。

### 6.4 多平台分发

优选基础库以 ZIP 压缩包形式在 Toolset Manifest 中按 target 声明，覆盖以下平台：

| Target Triple | 说明 |
| ------------- | ---- |
| `x86_64-unknown-linux-gnu` | Linux x86_64 |
| `aarch64-unknown-linux-gnu` | Linux aarch64 |
| `x86_64-pc-windows-msvc` | Windows x86_64 (MSVC) |
| `x86_64-pc-windows-gnu` | Windows x86_64 (GNU) |

Manifest 声明示例：

```toml
[tools.target.x86_64-unknown-linux-gnu.ylong_json]
optional = true
version = "1.0.0"
path = "tools/commonlibrary_rust_ylong_json-master.zip"
kind = "crate"

[tools.target.x86_64-unknown-linux-gnu.ylong_http]
optional = true
version = "1.0.0"
path = "tools/commonlibrary_rust_ylong_http-master.zip"
kind = "crate"

[tools.target.x86_64-unknown-linux-gnu.ylong_runtime]
optional = true
path = "tools/commonlibrary_rust_ylong_runtime-master.zip"
kind = "crate"

[tools.target.x86_64-unknown-linux-gnu.ylong_xml]
optional = true
version = "0.1.0"
path = "tools/commonlibrary_rust_ylong_xml-master.zip"
kind = "crate"
```

### 6.5 Toolkit 分组配置

在 Toolset Manifest 的 `[tools.group]` 中，优选基础库统一归入 **"Rust 优选开发库"** 分组：

```toml
[tools.group]
"Rust 优选开发库" = ["ylong_json", "ylong_light_actor", "ylong_xml", "ylong_http", "ylong_runtime"]
```

此分组在 GUI 安装界面和 CLI 组件选择中作为独立类别展示，与 "开发工具" 等分组并列。

### 6.6 依赖关系

优选基础库之间及其外部依赖关系如下：

```
ylong_json
├── serde
└── libc (可选, C FFI)

ylong_xml
└── serde

ylong_light_actor
└── (无外部依赖，纯 Rust 实现)

ylong_runtime
├── libc
├── ylong_io
├── ylong_ffrt (可选, FFRT 执行器)
├── ylong_signal
└── ylong_runtime_macros (ylong_macros)

ylong_http
├── ylong_http (协议核心)
├── ylong_runtime (可选)
├── tokio (可选)
├── openssl (HTTPS)
└── quiche (HTTP/3, 可选)
```

> **注意**: `ylong_http` 可基于 `ylong_runtime` 或 `tokio` 运行，两者通过 Feature Flag 互斥切换。若用户同时安装 `ylong_runtime` 和 `ylong_http`，推荐使用 `ylong_base` feature 以保持技术栈统一。

---

## 7. 代码检查工具集设计

### 7.1 概述

集成旋武社区《Rust 编程规范》中的各项规则，以 Clippy lint 的形式在编译时自动检查，帮助开发者写出高质量、符合规范的 Rust 代码。

### 7.2 实现机制

| 项目 | 说明 |
| ---- | ---- |
| 规则来源 | 旋武社区《Rust 编程规范》 |
| 实现方式 | 自定义 Clippy 规则集 (RuleSet 类型工具) |
| 工具类型 | `kind = "rule-set"` |
| 依赖关系 | `requires = ["rust"]`（需先安装 Rust 工具链） |
| 调用方式 | `xuanwu-rust-manager check` |
| 输出格式 | 与 Clippy 一致的 warning/error 格式 |
| 分发格式 | `.tar.xz` 压缩包，按平台分发 |

### 7.3 安装与分发

规则集按目标平台独立分发，在 Toolset Manifest 中配置：

| 平台 | 分发包 |
| ---- | ------ |
| x86_64-pc-windows-msvc | `rust-1.74.0-x86_64-pc-windows-msvc.tar.xz` |
| x86_64-pc-windows-gnu | `rust-1.74.0-x86_64-pc-windows-gnu.tar.xz` |
| x86_64-unknown-linux-gnu | `rust-1.74.0-x86_64-unknown-linux-gnu.tar.xz` |
| aarch64-unknown-linux-gnu | `rust-1.74.0-aarch64-unknown-linux-gnu.tar.xz` |

规则集归属 **"Rust 代码检查工具集"** 分组，在标准版安装方案中默认包含。

### 7.4 使用方式

在项目目录下运行代码检查：

```bash
xuanwu-rust-manager check
```

检查输出示例：

```
warning: type of this numeric variable is unconstrained
  --> src\main.rs:25:9
   |
25 |     let input = 10;
   |         ^^^^^
   |
   = help: for further information visit https://rust-lang.github.io/rust-clippy/master/index.html#unconstrained_numeric_literal
help: either add suffix to above numeric literal(s) or label the type explicitly
   |
25 |     let input: u64 = 10;
   |         ~~~~~~~~~~

warning: `example` (bin "example") generated 1 warning
    Finished dev [unoptimized + debuginfo] target(s) in 0.14s
```

### 7.5 规则分类

| 分类 | 示例规则 | 检查级别 |
| ---- | -------- | -------- |
| 类型安全 | 数值字面量必须约束类型 (`unconstrained_numeric_literal`) | warning |
| 命名规范 | 标识符命名风格检查 | warning |
| 内存安全 | 不安全内存操作检查 | warning |
| 并发安全 | 并发原语使用规范 | warning |

### 7.6 已知限制

- 暂不支持 Rust 2024 Edition
- 使用时需在 `Cargo.toml` 中设置 `edition = "2021"` 并删除 `Cargo.lock`
- 规则集基于 Rust 1.74.0 工具链构建，与更高版本工具链配合使用时需注意兼容性

---

## 8. 跨平台适配方案

### 8.1 支持矩阵

| 操作系统 | 架构 | Target Triple | 系统库 | GUI | CLI | 状态 |
| -------- | ---- | ------------- | ------ | --- | --- | ---- |
| Windows 10/11 | x86_64 | `x86_64-pc-windows-msvc` | MSVC | 已支持 | 已支持 | **已发布** |
| Windows 10/11 | x86_64 | `x86_64-pc-windows-gnu` | MinGW | - | 已支持 | **已发布** |
| Linux | x86_64 | `x86_64-unknown-linux-gnu` | glibc | 已支持 | 已支持 | **已发布** |
| Linux | x86_64 | `x86_64-unknown-linux-musl` | musl | - | 已支持 | **已发布** |
| Linux | aarch64 | `aarch64-unknown-linux-gnu` | glibc | 已支持 | 已支持 | **已发布** |
| Linux | aarch64 | `aarch64-unknown-linux-musl` | musl | - | 已支持 | **已发布** |
| 鸿蒙 PC (HarmonyOS) | x86_64/aarch64 | 待定 | 待定 | 规划中 | 规划中 | **适配中** |

> **鸿蒙 PC 适配说明**: 鸿蒙 PC 平台正在进行适配工作，主要涉及 GUI 渲染层（WebView 兼容性）、环境变量配置机制和 PATH 管理方式的适配。核心安装逻辑（Rust 层）具备天然的跨平台能力，预计适配工作量集中在 OS 适配层。

### 8.2 平台差异处理

#### 8.2.1 环境变量配置

| 平台 | 机制 | 实现文件 |
| ---- | ---- | -------- |
| Windows | 写入注册表 `HKCU\Environment`，通过 `WM_SETTINGCHANGE` 广播通知 | `src/core/os/windows.rs` |
| Linux/macOS | 修改 Shell 配置文件 (.bashrc / .zshrc / config.fish) | `src/core/os/unix.rs` |
| 鸿蒙 PC | 待适配 | 待定 |

#### 8.2.2 PATH 管理

| 平台 | 添加方式 | 移除方式 |
| ---- | -------- | -------- |
| Windows | 注册表 `HKCU\Environment\PATH` 追加 | 注册表移除对应条目 |
| Linux | Shell 配置文件 `export PATH=...` | 从配置文件删除对应行 |

#### 8.2.3 二进制产物

| 平台 | CLI 产物 | GUI 产物 | 管理器 |
| ---- | -------- | -------- | ------ |
| Windows (MSVC) | `rim-cli.exe` | `rim-gui.exe` | `{app_name}.exe` |
| Windows (GNU) | `rim-cli.exe` | - | `{app_name}.exe` |
| Linux x86_64 | `rim-cli` | `rim-gui` | `{app_name}` |
| Linux aarch64 | `rim-cli` | `rim-gui` | `{app_name}` |

#### 8.2.4 GUI 平台依赖

Tauri GUI 在 Linux 上依赖系统 WebView，需预装以下包：

```bash
# Debian/Ubuntu
sudo apt install -y \
    libwebkit2gtk-4.0-dev \
    libgtk-3-dev \
    libayatana-appindicator3-dev \
    librsvg2-dev

# 中文字体 (避免界面乱码)
sudo apt install -y fonts-noto-cjk fonts-wqy-microhei
```

Windows 版本使用系统内置 WebView2（Windows 10/11 默认已包含），无需额外安装。

### 8.3 构建环境

各平台构建通过 Docker 容器和 CI 矩阵实现：

| Target | 构建环境 | 基础镜像 | 特殊依赖 |
| ------ | -------- | -------- | -------- |
| `x86_64-unknown-linux-*` | Docker | Debian Buster | Node.js 22 + pnpm + Tauri deps |
| `aarch64-unknown-linux-*` | Docker (交叉编译) | Debian Buster | aarch64 交叉工具链 |
| `x86_64-pc-windows-msvc` | Windows Runner | - | MSVC Build Tools + Node.js |
| `x86_64-pc-windows-gnu` | Windows Runner | - | MinGW-w64 |

---

## 9. 网络与镜像策略

### 9.1 镜像配置

| 资源类型 | 镜像地址 |
| -------- | -------- |
| Rust 工具链 (RUSTUP_DIST_SERVER) | `https://mirror.xuanwu.openatom.cn/` |
| Rustup 更新 (RUSTUP_UPDATE_ROOT) | `https://mirror.xuanwu.openatom.cn/rustup` |
| R.I.M 分发包 (RIM_DIST_SERVER) | `https://rust-mirror.obs.cn-north-4.myhuaweicloud.com` |
| Cargo 注册表 (sparse 协议) | `sparse+https://mirror.xuanwu.openatom.cn/index/` |
| Cargo 注册表 (git 协议) | `https://mirror.xuanwu.openatom.cn/crates.io-index` |

### 9.2 Cargo 镜像源配置

安装管理工具自动在 `{CARGO_HOME}/config.toml` 中写入镜像源配置，用户无需手动配置：

```toml
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
```

配置文件路径：
- Windows：`%USERPROFILE%\.cargo\config.toml`
- Linux：`~/.cargo/config.toml`

### 9.3 下载策略

| 特性 | 说明 |
| ---- | ---- |
| 断点续传 | 支持 HTTP Range 请求 |
| 代理支持 | 通过 Toolset Manifest 配置 http/https/no-proxy |
| 重试机制 | 下载失败自动重试 |
| SSL 验证 | 默认启用，可通过 `--insecure` 跳过（仅限调试） |
| 完整性校验 | 下载内容校验完整性 |

### 9.4 归档解压策略

R.I.M 实现了 6 层弹性归档解压机制，处理各种嵌套和异常格式：

```
下载工具包
  │
  ├── 1. 扩展名检测 (最快)     → 根据文件扩展名选择解压方式
  ├── 2. 内容检测 (magic bytes) → 读取文件头判断实际格式
  ├── 3. 加载测试 (最可靠)     → 尝试以各种格式打开文件
  ├── 4. 路径比较              → 规范化路径检测嵌套
  ├── 5. 文件名模式匹配        → 根据文件名推断格式
  └── 6. 显式格式提示          → 使用 Manifest 中的格式声明
```

**支持的归档格式**：

| 格式 | 扩展名 | 使用场景 |
| ---- | ------ | -------- |
| ZIP | `.zip` | Windows 工具包、优选基础库 |
| tar.gz | `.tar.gz` | Linux 工具包 (cargo-nextest) |
| tar.xz | `.tar.xz` | 代码检查规则集、Rust 工具链 |
| 7z | `.7z` | MinGW-w64 |

**嵌套归档处理**：优选基础库分发包可能存在嵌套 ZIP（外层 ZIP 解压后内部还有一层 ZIP），解压引擎自动检测并递归解压，最终定位包含 `Cargo.toml` 或 `bin/` 的目标目录。

### 9.5 离线安装模式

```
离线安装包结构
├── rim-cli / rim-gui          # 安装器二进制
├── toolset-manifest.toml       # 工具包清单 (path 指向本地)
└── packages/                   # 预下载的工具包
    ├── rustup-init
    ├── rust-{version}-{target}.tar.xz
    └── tools/                  # 优选基础库 + 第三方工具压缩包
        ├── commonlibrary_rust_ylong_http-master.zip
        ├── commonlibrary_rust_ylong_json-master.zip
        ├── commonlibrary_rust_ylong_runtime-master.zip
        ├── commonlibrary_rust_ylong_xml-master.zip
        └── ...
```

---

## 10. 国际化方案

### 10.1 支持语言

| 语言 | 标识 | 状态 |
| ---- | ---- | ---- |
| 简体中文 | zh-CN | 已支持 |
| English | en-US | 已支持 |

### 10.2 实现方案

- **Rust 侧**: 使用 `rust-i18n` crate，通过 `t!()` 宏访问翻译
- **翻译文件**: `locales/zh-CN.json` 和 `locales/en-US.json`
- **GUI 侧**: 使用 `vue-i18n` (v11.x) 实现前端中英文切换，翻译文件位于 `rim_gui/src/` 目录
- **语言切换**: CLI 通过 `--lang` 参数指定；GUI 通过界面选择

### 10.3 本地化定制

通过 `configuration.toml` 中的 `[locale]` 段配置产品标识：

```toml
[locale.zh-CN]
logo_text = "旋武社区"
vendor = "Rust 中国社区"
product = "Rust 中国社区发行版"
app_name = "Rust 安装管理器"
```

---

## 11. 构建与发布流程

### 11.1 构建前置条件

| 依赖 | 版本要求 | 用途 |
| ---- | -------- | ---- |
| Rust | >= 1.80.0 | 编译核心代码 |
| NodeJS | 22.x (via nvm) | GUI 前端构建 |
| pnpm | latest | 前端包管理 |
| Tauri CLI | v1 | GUI 打包 |
| Docker | latest | Linux 跨平台构建 |

### 11.2 构建命令

| 场景 | 命令 | 产物 |
| ---- | ---- | ---- |
| CLI 网络安装器 | `cargo dev dist -b --cli` | rim-cli 二进制 (在线模式) |
| CLI 离线安装包 | `cargo dev dist --cli` | rim-cli + packages/ 目录 |
| GUI 网络安装器 | `cargo dev dist -b --gui` | rim-gui 二进制 (在线模式) |
| GUI 离线安装包 | `cargo dev dist --gui` | rim-gui + packages/ 目录 |
| 全部 | `cargo dev dist` | CLI + GUI + 离线包 |
| 预下载离线资源 | `cargo dev vendor --download-only` | 下载工具包到 packages/ |

Edition 选择通过环境变量控制：

```bash
export EDITION='community'    # 社区发行版 (生产)
export EDITION='bisheng'      # 毕昇发行版
export EDITION='test'         # 测试版 (默认)
```

### 11.3 发布版本管理

| 配置项 | 说明 |
| ------ | ---- |
| 版本号 | `workspace.package.version` (当前 0.10.0) |
| Edition | 通过 `EDITION` 环境变量指定 (默认 test，发布用 community) |
| 编译优化 | Release 模式: opt-level=3, codegen-units=1, lto=thin |

### 11.4 CI/CD 流程

```
CI/CD 流水线 (GitHub Actions)
├── 代码提交 → 触发
├── standard-tests.yml      # 代码检查 + 单元/集成测试
│   ├── clippy + fmt
│   └── cargo test
├── gui-tests.yml           # GUI 专项测试
├── release.yml             # 发布构建 (矩阵)
│   ├── Linux x86_64
│   │   ├── Docker (dist-x86-64-linux)
│   │   ├── 构建 CLI (musl 静态链接)
│   │   ├── 构建 GUI (musl + Tauri)
│   │   └── 打包离线安装包
│   ├── Linux aarch64
│   │   ├── Docker (dist-aarch64-linux)
│   │   ├── 构建 CLI (musl 交叉编译)
│   │   ├── 构建 GUI (musl 交叉编译)
│   │   └── 打包离线安装包
│   ├── Windows x86_64 (MSVC)
│   │   ├── 构建 CLI + GUI
│   │   └── 打包离线安装包
│   └── Windows x86_64 (GNU)
│       └── 构建 CLI
└── 发布产物上传到分发服务器
```

---

---

# 第二部分：DFX设计

---

## 12. 可靠性/可用性设计

### 12.1 可靠性设计目标

| 编号 | 设计目标 | 说明 |
| ---- | -------- | ---- |
| REL-001 | 安装失败可恢复 | 安装中断时提供明确错误信息，支持重新安装 |
| REL-002 | 断点续传 | 网络下载支持 HTTP Range 请求，中断后可续传 |
| REL-003 | 文件操作容错 | 文件操作失败自动重试（最多 10 次） |
| REL-004 | 单实例保护 | 防止多个安装/卸载进程同时运行导致状态冲突 |
| REL-005 | 安装记录持久化 | 通过 .fingerprint.toml 记录安装状态，确保管理器可正确识别已安装组件 |

### 12.2 网络可靠性

| 特性 | 实现方式 | 说明 |
| ---- | -------- | ---- |
| 断点续传 | HTTP Range 请求 | 下载中断后从断点继续，避免重复下载 |
| 自动重试 | reqwest 重试策略 | 网络请求失败后自动重试 |
| 代理支持 | Toolset Manifest 配置 | 支持 http/https/no-proxy 代理配置 |
| 镜像加速 | 国内镜像源 | 工具链和 Cargo 注册表均使用国内镜像 |
| 离线模式 | 本地安装包 | 完全离线环境下可通过预下载的安装包完成安装 |

### 12.3 可用性设计

| 编号 | 设计目标 | 说明 |
| ---- | -------- | ---- |
| AVL-001 | 多模式安装 | 提供 GUI 图形化、CLI 在线、CLI 离线三种安装模式，覆盖不同使用场景 |
| AVL-002 | 中英文双语 | 安装过程支持中英文界面切换 |
| AVL-003 | 一键安装 | 支持 `--yes-to-all` 全自动安装，适合 CI/CD 和批量部署 |
| AVL-004 | 安装时间 | 正常网络环境下完整安装时间不超过 10 分钟 |
| AVL-005 | 错误提示 | 安装失败时提供明确、可操作的错误信息 |
| AVL-006 | Linux 低版本兼容 | GUI 安装器支持较旧 Linux 发行版（glibc 2.31+），不强制要求最新系统 |

### 12.4 Linux glibc 兼容性设计

#### 12.4.1 问题背景

Tauri v2 依赖 `libwebkit2gtk-4.1`，该库仅在较新的 Linux 发行版中提供（Ubuntu 22.04+、Debian 12+），要求 glibc 2.35+。这导致在企业常用的 Ubuntu 20.04、CentOS 7/8 等系统上无法运行 GUI 安装器，严重影响发行版的可用性覆盖范围。

| Tauri 版本 | WebKit 依赖 | 最低 Ubuntu 版本 | glibc 要求 |
| ---------- | ----------- | --------------- | ---------- |
| Tauri v2 | `libwebkit2gtk-4.1` | Ubuntu 22.04+ | glibc 2.35+ |
| **Tauri v1** | `libwebkit2gtk-4.0` | **Ubuntu 20.04+** | **glibc 2.31+** |

#### 12.4.2 解决方案

发行版将 Tauri 框架从 v2 回退至 v1（PR #243），以确保 GUI 安装器在更广泛的 Linux 系统上可用：

| 组件 | 回退前 (Tauri v2) | 回退后 (Tauri v1) |
| ---- | ----------------- | ----------------- |
| tauri (Rust) | `2.x` | `1.x` |
| tauri-build | `2.x` | `1.x` |
| tauri-plugin-single-instance | `2.x` | v1 分支 (git 依赖) |
| @tauri-apps/api (前端) | `^2.5.0` | `1.x` |
| @tauri-apps/cli (构建) | `^2.5.0` | `1.x` |

#### 12.4.3 系统依赖对比

**Tauri v1 (当前采用)** — 兼容更多 Linux 发行版：

| 发行版 | 关键依赖包 |
| ------ | ---------- |
| Debian/Ubuntu | `libwebkit2gtk-4.0-dev`, `libgtk-3-dev`, `libayatana-appindicator3-dev`, `librsvg2-dev` |
| Arch Linux | `webkit2gtk`, `gtk3`, `libappindicator-gtk3`, `librsvg` |
| Fedora/RHEL | `webkit2gtk4.0-devel`, `openssl-devel`, `libappindicator-gtk3-devel` |
| openSUSE | `webkit2gtk3-soup2-devel`, `libopenssl-devel` |
| Gentoo | `net-libs/webkit-gtk:4`, `dev-libs/libappindicator` |

**Tauri v2 (已放弃)** — 要求较新系统：

| 发行版 | 关键依赖包 | 问题 |
| ------ | ---------- | ---- |
| Debian/Ubuntu | `libwebkit2gtk-4.1-dev`, `libxdo-dev` | Ubuntu 20.04 无此包 |
| Arch Linux | `webkit2gtk-4.1` | 仅最新版本提供 |
| Fedora/RHEL | `webkit2gtk4.1-devel` | CentOS 7/8 无此包 |

#### 12.4.4 兼容性验证

| Linux 发行版 | 版本 | glibc | GUI 可用性 |
| ------------ | ---- | ----- | :--------: |
| Ubuntu | 20.04 LTS | 2.31 | 支持 |
| Ubuntu | 22.04 LTS | 2.35 | 支持 |
| Debian | 11 (Bullseye) | 2.31 | 支持 |
| Debian | 12 (Bookworm) | 2.36 | 支持 |
| CentOS | 8 Stream | 2.28 | CLI 可用，GUI 需验证 |

#### 12.4.5 构建环境适配

为确保产出的二进制兼容低版本系统，CI 构建环境使用 Debian Buster (glibc 2.28) 基础镜像，并采用 musl 静态链接方式编译 CLI 版本：

| 构建目标 | 基础镜像 | 链接方式 | 兼容性 |
| -------- | -------- | -------- | ------ |
| CLI (x86_64) | Debian Buster | musl 静态链接 | 几乎所有 Linux 发行版 |
| CLI (aarch64) | Debian Buster | musl 交叉编译 | 几乎所有 Linux 发行版 |
| GUI (x86_64) | Debian Buster | 动态链接 (依赖 WebKit) | glibc 2.31+ |
| GUI (aarch64) | Debian Buster | 动态链接 (交叉编译) | glibc 2.31+ |

### 12.5 Windows WebView2 运行时兼容性设计

#### 12.5.1 问题背景

Tauri GUI 框架在 Windows 平台依赖 Microsoft Edge WebView2 Runtime 作为渲染引擎。虽然 Windows 11 及较新的 Windows 10 版本已预装 WebView2，但在企业内网环境、精简版系统或未更新的 Windows 10 系统中，WebView2 可能缺失。此时直接启动 Tauri 应用会导致不可读的底层错误（如 `webview2 not found`），用户无法理解问题原因。

| 平台 | WebView 依赖 | 预装情况 |
| ---- | ------------ | -------- |
| Windows 11 | WebView2 Runtime | 系统预装 |
| Windows 10 (较新) | WebView2 Runtime | 通过 Windows Update 推送 |
| Windows 10 (企业/精简) | WebView2 Runtime | 可能缺失 |
| Linux | libwebkit2gtk-4.0 | 需手动安装 |
| macOS | WebKit (系统内置) | 始终可用 |

#### 12.5.2 解决方案

发行版实现了 WebView 运行时预检机制（commit 72020b36），在 Tauri 框架初始化之前主动探测 WebView2 可用性，若缺失则显示本地化的用户友好提示，引导用户安装或使用 CLI 替代方案：

```
GUI 启动流程
├── 1. 判断是否需要启动 GUI (should_start_gui())
├── 2. 调用 ensure_platform_webview_dependency()
│   ├── Windows: 探测 WebView2 Runtime
│   │   ├── Ready → 继续启动
│   │   └── Missing → 弹出本地化对话框 → 退出
│   ├── Linux: 探测 libwebkit2gtk
│   │   ├── Ready → 继续启动
│   │   └── Uncertain → stderr 警告 → 继续尝试
│   └── macOS: 始终 Ready
└── 3. 启动 Tauri 应用
```

#### 12.5.3 Windows 探测机制

Windows 平台采用注册表探测 + 文件系统验证的双重检测策略：

**注册表探测路径**（按优先级）：

| 注册表位置 | 路径 | 说明 |
| ---------- | ---- | ---- |
| HKLM (原生) | `SOFTWARE\Microsoft\EdgeUpdate\Clients\{F3017226-...}` | 系统级安装 (64位) |
| HKLM (WOW64) | `SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\{F3017226-...}` | 系统级安装 (32位兼容) |
| HKCU | `SOFTWARE\Microsoft\EdgeUpdate\Clients\{F3017226-...}` | 用户级安装 |

**文件系统验证**：

注册表中 `pv` (product version) 值指示安装路径，但仅有注册表项不足以确认可用性。探测逻辑还会检查 Well-Known 路径下 `msedgewebview2.exe` 是否实际存在：

| 检查路径 | 说明 |
| -------- | ---- |
| `%ProgramFiles%\Microsoft\EdgeCore\<version>\` | 64位 Program Files |
| `%ProgramFiles(x86)%\Microsoft\EdgeCore\<version>\` | 32位 Program Files |
| `%LocalAppData%\Microsoft\EdgeCore\<version>\` | 用户本地安装 |

只有在注册表和文件系统双重确认后，才判定为 `Ready`。

#### 12.5.4 跨平台探测策略

| 平台 | 探测方式 | 缺失时行为 | 设计理由 |
| ---- | -------- | ---------- | -------- |
| Windows | 注册表 + 文件验证 | `Missing` → 弹窗提示 → 退出 | 可靠判定，避免 Tauri 崩溃 |
| Linux | `pkg-config` + `.so` 文件搜索 | `Uncertain` → stderr 警告 → 继续 | 检测不完全可靠，交由 Tauri 最终判定 |
| macOS | 无需检测 | 始终 `Ready` | 系统内置 WebKit |

探测结果枚举：

```rust
enum WebviewProbeResult {
    Ready,      // 确认可用，继续启动
    Missing,    // 确认缺失，阻止启动并提示
    Uncertain,  // 无法确定，警告后继续尝试
}
```

#### 12.5.5 用户提示设计

Windows 平台检测到 WebView2 缺失时，通过 Win32 MessageBox API 显示本地化对话框（无需额外 GUI 框架依赖）：

| 提示内容 | 说明 |
| -------- | ---- |
| 标题 | "缺少 WebView2 运行时" |
| 正文 | 说明 GUI 需要 WebView2，提供下载链接，建议使用 CLI 版本作为替代 |
| 按钮 | 确定（退出程序） |

Linux 平台仅输出 stderr 警告，不阻止启动（因为检测结果为 `Uncertain`，可能存在误判）。

#### 12.5.6 设计优势

| 设计点 | 说明 |
| ------ | ---- |
| 前置检测 | 在 Tauri 初始化前完成，避免框架级崩溃 |
| 零额外依赖 | 仅使用 winapi 注册表 API，不引入新的运行时依赖 |
| 优雅降级 | 提示用户可使用 CLI 版本，不阻断安装流程 |
| 双重验证 | 注册表 + 文件存在性，避免注册表残留导致误判 |
| 平台差异化 | 各平台采用适合自身特点的检测策略和响应方式 |

### 12.6 可靠性验证结果

发行版在 Linux 和 Windows 平台上完成了全量安装与最小安装的可靠性验证：

| 验证项 | Linux (x86_64) | Windows (x86_64) | 结果 |
| ------ | :------------: | :--------------: | :--: |
| 全量安装 | 安装完成 | 安装成功 | 通过 |
| 最小安装 | 安装成功 | 安装成功 | 通过 |
| 例程运行 | 运行成功 | 运行成功 | 通过 |
| 规范检查工具 | 运行成功，修复后报错消失 | 运行成功，修复后报错消失 | 通过 |
| Clippy | 运行成功 | 运行成功 | 通过 |
| Rustfmt | 运行成功 | 运行成功，格式自动对齐 | 通过 |
| Rustdoc | 运行成功 | 运行成功，输出文档 | 通过 |
| Rust-gdb | 启动成功 | 调试器运行成功 | 通过 |
| llvm-tools | 工具启动成功 | 工具启动成功 | 通过 |
| Rust docs | 文档安装成功 | 文档安装成功 | 通过 |
| rustup target | Target 添加成功 | target 安装成功 | 通过 |
| cargo nextest | 运行成功 | 例程运行成功 | 通过 |
| IDE 插件 | - | 安装成功 | 通过 |

### 12.7 分发服务可用性

| 组件 | 说明 |
| ---- | ---- |
| 存储 | 华为云 OBS 对象存储 |
| 域名 | `rust-mirror.obs.cn-north-4.myhuaweicloud.com` |
| 镜像同步 | 旋武社区镜像 `mirror.xuanwu.openatom.cn` |
| 冗余 | 多镜像源互为备份 |

### 12.8 版本发布可靠性

```
版本发布流程
├── 1. 更新 workspace.package.version
├── 2. 更新 toolset-manifest (Rust 版本、工具版本)
├── 3. 构建各平台安装包
├── 4. 上传至分发服务器
├── 5. 更新 dist-manifest.toml (添加新版本)
├── 6. 更新 release.toml (管理器版本)
└── 7. 用户端管理器自动检测更新
```

### 12.8 监控与告警

| 监控项 | 方式 | 告警阈值 |
| ------ | ---- | -------- |
| 镜像可用性 | 定期探测下载链路 | 连续 3 次探测失败 |
| 下载成功率 | 服务端日志统计 | 成功率低于 95% |
| 安装成功率 | 客户端匿名上报（可选） | 成功率低于 90% |

---

## 13. 功能安全设计

### 13.1 安装过程安全

| 安全措施 | 说明 |
| -------- | ---- |
| 路径校验 | 安装路径不允许包含无效 Unicode 字符，防止路径注入 |
| 权限最小化 | Unix 环境仅需用户目录写权限，不要求 root |
| 环境变量隔离 | Windows 环境变量修改仅影响当前用户 (HKCU)，不影响系统级配置 |
| 临时文件清理 | 安装完成后自动清理 temp/ 目录，避免残留 |
| 单实例互斥 | 通过 tauri-plugin-single-instance 防止重复启动导致状态冲突 |

### 13.2 卸载安全

| 安全措施 | 说明 |
| -------- | ---- |
| 分级卸载 | 支持完全卸载和保留管理器两种模式，避免误删 |
| 依赖顺序 | 按反向依赖顺序卸载，确保不破坏依赖关系 |
| 记录追踪 | 通过 .fingerprint.toml 精确追踪已安装组件，避免误删用户文件 |
| 用户配置保留 | 卸载 IDE 时不删除用户配置目录（如 ~/.vscode） |

### 13.3 组件冲突防护

工具可声明以下依赖关系，安装管理工具在安装/卸载时自动处理：

| 关系类型 | 说明 | 安全行为 |
| -------- | ---- | -------- |
| `requires` | 依赖的其他工具 | 安装时先装依赖，缺失依赖时拒绝安装 |
| `obsoletes` | 被此工具替代的旧工具 | 自动卸载旧工具，避免版本冲突 |
| `conflicts` | 冲突的工具 | 拒绝同时安装，提示用户选择 |

### 13.4 Windows 构建环境安全

| 场景 | 安全措施 |
| ---- | -------- |
| Build Tools 安装 | 使用微软官方安装程序，静默安装参数经过验证 |
| 退出码处理 | 完整映射 VS 安装程序退出码，区分成功/需重启/权限不足等状态 |
| 权限提示 | 需要管理员权限时明确提示用户，不静默失败 |
| MinGW 来源 | 从华为云 OBS 镜像获取，版本固定且经过验证 |

### 13.5 IDE 功能安全验证

CodeArts IDE for Rust 经过完整功能验证，确保集成安装后各项功能正常：

| 验证项 | 验证内容 | 结果 |
| ------ | -------- | :--: |
| 安装 | IDE 安装流程 | 安装成功 |
| 代码编辑 | 页面显示及关键字提示 | 正常 |
| 报错信息 | 语法/编译错误提示 | 正常 |
| 代码联想 | 自动补全与智能提示 | 正常 |
| 调试功能 | 断点调试与变量查看 | 正常 |
| 运行 | 项目编译与运行 | 正常 |
| 插件安装 | 扩展插件安装 | 安装成功 |

### 13.6 编译器功能安全验证

基于旋武社区构建版本的 Rust 编译器（rustc 1.72.0）在 x86_64-unknown-linux-gnu 平台完成功能安全验证：

| 测试套 | 用例数 | 测试内容 | 通过率 |
| ------ | :----: | -------- | :----: |
| FeatureTest | 52 | 编译器特性验证，含 ASan 和 gccrs 测试 | 100% |
| RustLangTest | 15149 | 移植自 Rust 开源仓 test 目录的标准测试集 | 100% |
| RustSmithTest | 1000 | 基于随机测试工具生成、编译、运行随机用例 | 100% |

验证版本：`rustc 1.72.0 (91a88a8c1 2025-05-08) (llvm-project da92590856)`

### 13.7 安装回滚机制（规划中）

| 状态 | 说明 |
| ---- | ---- |
| 当前 | 安装失败时记录错误日志，不自动回滚 |
| 规划 | 实现安装中断时自动清理已安装内容，恢复到安装前状态 |

---

## 14. 网络安全设计

### 14.1 下载安全

| 安全措施 | 说明 |
| -------- | ---- |
| HTTPS 传输 | 所有下载链路使用 HTTPS 加密传输 |
| 完整性校验 | 下载内容通过哈希校验确保完整性 |
| 来源可信 | 工具包仅从配置的可信镜像源下载 |
| 代理安全 | 支持 HTTPS 代理，代理配置通过 Manifest 管理 |

### 14.2 分发安全

| 安全措施 | 说明 |
| -------- | ---- |
| Manifest 签名 | 分发清单的完整性保障 |
| 版本锁定 | Toolset Manifest 中工具版本明确指定，防止供应链攻击 |
| 镜像一致性 | 镜像内容与上游保持同步校验 |

### 14.3 运行时安全

| 安全措施 | 说明 |
| -------- | ---- |
| 权限控制 | Windows 需管理员权限修改注册表；Unix 仅需用户目录写权限 |
| 环境变量隔离 | 环境变量修改仅影响当前用户 (HKCU)，不影响系统全局 |
| 路径安全 | 安装路径校验，不允许包含无效 Unicode 字符 |
| 单实例保护 | 不应同时运行多个安装/卸载进程 |
| 文件操作安全 | 文件操作失败最多重试 10 次，避免竞态条件 |

### 14.4 IDE 分发合规

| IDE | 安全策略 |
| --- | -------- |
| VS Code | 直接从微软官方 Release 下载，避免二次分发，符合许可证要求 |
| CodeArts IDE | 从华为云 OBS 官方托管下载，来源可信 |

### 14.5 Cargo Patch 安全性

| 安全考量 | 说明 |
| -------- | ---- |
| 版本兼容性 | Cargo 要求 patch 版本号与被替代版本语义兼容，防止类型不一致 |
| 路径可控 | patch 路径指向发行版安装目录内的 crates/，不引用外部不可控路径 |
| 配置可审计 | 所有 patch 配置写入 {CARGO_HOME}/config.toml，用户可随时审查 |
| 卸载可逆 | 卸载时精确移除对应 patch 条目，不影响其他配置 |

---

## 15. 可维测设计

### 15.1 可维护性设计

#### 15.1.1 模块化架构

R.I.M 采用 Cargo Workspace 组织为五个 crate，职责边界清晰：

| Crate | 职责 | 可维护性价值 |
| ----- | ---- | ------------ |
| `rim` | 核心业务逻辑 + CLI 入口 | 业务逻辑集中，修改影响范围可控 |
| `rim_gui` | Tauri 桌面应用 | GUI 变更不影响核心逻辑 |
| `rim_common` | 公共类型与工具函数 | 复用代码集中管理，避免重复 |
| `rim_dev` | 开发辅助工具 | 构建/打包逻辑独立，不污染生产代码 |
| `rim_test` | 测试支持库 | 测试基础设施独立演进 |

#### 15.1.2 扩展性设计

| 扩展点 | 机制 | 说明 |
| ------ | ---- | ---- |
| 新工具类型 | `ToolType` 枚举 + 安装/卸载 trait | 新增工具类型只需实现对应 trait |
| 新平台支持 | `os/` 模块 + target 条件编译 | 平台适配代码隔离在 os/ 目录 |
| 新 IDE 集成 | `VSCodeInstaller` 复用结构 | 新 IDE 只需实例化不同参数 |
| 新优选基础库 | Manifest 配置 + Crate 类型 | 新增基础库只需添加 Manifest 条目 |

#### 15.1.3 配置驱动

工具包定义完全由 Toolset Manifest 驱动，新增/移除工具无需修改代码，只需更新配置文件。

### 15.2 可测试性设计

#### 15.2.1 测试分层

| 层次 | 范围 | 工具 | 位置 |
| ---- | ---- | ---- | ---- |
| 单元测试 | 函数级 | `#[test]` | 各模块内 |
| 集成测试 | 模块间交互 | `rim_test` 支持库 | `tests/testsuite/` |
| E2E 测试 | 完整安装/卸载流程 | 自定义测试框架 | `tests/` |

#### 15.2.2 测试覆盖范围

| 测试项 | 说明 |
| ------ | ---- |
| CLI 命令解析 | 各命令参数解析与执行 |
| 文件解压 | 各种压缩格式 (.zip, .7z, .tar.gz, .tar.xz) |
| 环境变量 | 各平台环境变量设置/移除 |
| 文件遍历 | 安装目录结构生成 |
| 清单解析 | 各配置文件 TOML 解析 |
| 优选基础库安装 | Crate 类型工具的解压、嵌套 ZIP 处理、Cargo 补丁配置写入与移除 |

#### 15.2.3 平台级验证测试

发行版在各目标平台上执行端到端验证测试，覆盖安装、工具链功能、组件集成等维度：

**Linux 平台验证 (x86_64-unknown-linux-gnu)**：

| 测试项 | 验证内容 | 结果 |
| ------ | -------- | :--: |
| 全量安装 | 所有组件一次性安装 | 通过 |
| 最小安装 | 仅安装必选组件 | 通过 |
| 例程运行 | 安装后编译运行示例项目 | 通过 |
| 规范检查工具 | 代码检查 + 自动修复 | 通过 |
| Clippy | 静态分析工具运行 | 通过 |
| Rustfmt | 代码格式化 | 通过 |
| Rustdoc | 文档生成 | 通过 |
| Rust-gdb | GDB 调试器集成 | 通过 |
| Rust-lldb | LLDB 调试器集成 | 通过 |
| llvm-tools | LLVM 工具集 | 通过 |
| Rust docs | 离线文档安装 | 通过 |
| rustup target | 交叉编译目标添加 | 通过 |
| cargo nextest | 高性能测试框架 | 通过 |

**Windows 平台验证 (x86_64-pc-windows-msvc)**：

| 测试项 | 验证内容 | 结果 |
| ------ | -------- | :--: |
| 全量安装 | 所有组件一次性安装 | 通过 |
| 最小安装 | 仅安装必选组件 | 通过 |
| 例程运行 | 安装后编译运行示例项目 | 通过 |
| 规范检查工具 | 代码检查 + 自动修复 | 通过 |
| Clippy | 静态分析工具运行 | 通过 |
| Rustfmt | 代码格式化，格式自动对齐 | 通过 |
| Rustdoc | 文档生成输出 | 通过 |
| rustup target | 交叉编译目标安装 | 通过 |
| Rust-gdb | 调试器运行 | 通过 |
| llvm-tools | LLVM 工具集启动 | 通过 |
| Rust docs | 离线文档安装 | 通过 |
| cargo nextest | 高性能测试框架 | 通过 |
| IDE 插件 | CodeArts IDE 安装与集成 | 通过 |

#### 15.2.4 优选基础库测试验证

测试原则：基础库自身已集成功能测试代码，在基础库源码项目目录中使用 `cargo build` 及 `cargo test` 命令进行构建测试及单元测试。

| 基础库 | 构建测试 | 单元测试 | 结果 |
| ------ | :------: | :------: | :--: |
| ylong_xml | 通过 | 全量单元测试通过 | 通过 |
| ylong_json | 通过 | 通过 | 通过 |
| ylong_light_actor | 通过 | 通过 | 通过 |
| ylong_runtime | 通过 | 通过 | 通过 |
| ylong_http | 通过 | 通过 | 通过 |

结论：全部优选基础库通过构建测试及单元测试。

#### 15.2.5 编译器版本验证测试

针对旋武社区构建版本的 Rust 编译器，执行三层测试套验证编译器正确性：

| 测试套 | 用例数 | 测试内容 | 通过率 |
| ------ | :----: | -------- | :----: |
| FeatureTest | 52 | 编译器特性验证（含 ASan、gccrs） | 100% |
| RustLangTest | 15149 | Rust 官方测试集（移植自 rust-lang/rust test 目录） | 100% |
| RustSmithTest | 1000 | 随机生成用例的模糊测试（生成→编译→运行） | 100% |

测试环境：x86_64-unknown-linux-gnu
测试版本：rustc 1.72.0 (91a88a8c1 2025-05-08) (llvm-project da92590856)

#### 15.2.6 IDE 集成测试

| 测试项 | 验证内容 | 结果 |
| ------ | -------- | :--: |
| 安装 | IDE 安装流程完整性 | 通过 |
| 代码编辑 | 页面显示及关键字提示 | 正常 |
| 报错信息 | 语法/编译错误实时提示 | 正常 |
| 代码联想 | 自动补全与智能提示 | 正常 |
| 调试功能 | 断点设置、变量查看、单步执行 | 正常 |
| 运行 | 项目编译与运行 | 正常 |
| 插件安装 | 扩展插件安装与加载 | 通过 |

#### 15.2.7 测试环境

- 使用 `rim_dev` 创建模拟环境运行 Manager 模式测试
- `rim_test/rim-test-macro/` 提供测试宏简化测试编写
- `tests/assets/` 存放测试用资源文件

#### 15.2.8 CI 自动化测试

```
CI 测试流水线
├── standard-tests.yml      # 代码检查 + 单元/集成测试
│   ├── clippy + fmt        # 静态分析
│   └── cargo test          # 单元 + 集成测试
├── gui-tests.yml           # GUI 专项测试
└── 多平台矩阵              # Linux x86_64/aarch64 + Windows MSVC/GNU
```

### 15.3 可观测性设计

#### 15.3.1 日志系统

| 组件 | 技术 | 说明 |
| ---- | ---- | ---- |
| 日志框架 | fern + log | 彩色分级日志输出 |
| 日志级别 | error/warn/info/debug | 通过 `--verbose` 切换详细输出 |
| 进度展示 | indicatif (CLI) / Tauri Event (GUI) | 双端进度抽象 |

#### 15.3.2 安装记录

通过 `.fingerprint.toml` 持久化记录安装状态，支持：

- 已安装组件追踪（名称、版本、路径、类型）
- 安装目录记录
- 工具链频道与组件列表
- 管理器模式自动检测依据

#### 15.3.3 错误诊断

| 场景 | 诊断信息 |
| ---- | -------- |
| 安装失败 | 记录错误日志，输出具体失败组件和原因 |
| 卸载失败 | 记录警告，继续卸载其他组件，汇总报告 |
| 网络失败 | 输出 URL、HTTP 状态码、重试次数 |
| 文件操作失败 | 输出文件路径、操作类型、系统错误码 |
| Build Tools 安装失败 | 映射 VS 安装程序退出码为可读错误消息 |

---

## 16. 未来演进规划

### 16.1 短期规划

<!-- 近期计划的功能和优化 -->

| 优先级 | 事项 | 说明 |
| ------ | ---- | ---- |
| P0 | 支持 Rust 2024 Edition | 代码检查工具集兼容新 Edition |
| P0 | 鸿蒙 PC 平台适配 | GUI WebView 兼容、环境变量/PATH 适配、CI 构建支持 |
| P1 | 安装失败回滚机制 | 安装中断时自动清理已安装内容 |
| P1 | GUI 英文支持完善 | 当前 GUI 仅支持中文 |

### 16.2 中期规划

| 事项 | 说明 |
| ---- | ---- |
| 鸿蒙 PC 全面支持 | 完成适配后纳入 CI 矩阵，提供 GUI + CLI 安装器 |
| 插件化工具集 | 支持社区贡献第三方工具集 |
| 增量更新 | 工具包差量更新，减少下载量 |
| 优选基础库扩展 | 引入更多 ylong 系列库（如 ylong_light_actor）及其他社区优质 crate |

### 16.3 长期愿景

<!-- 描述项目的长期目标 -->

- 成为国内 Rust 开发者的首选安装管理方案
- 建立完善的 Rust 开发工具生态
- 推动 Rust 编程规范在行业中的落地

---

## 17. 附录

### 附录 A: 环境变量一览

| 变量名 | 说明 | 设置时机 |
| ------ | ---- | -------- |
| `CARGO_HOME` | Cargo 主目录 | 安装时 |
| `RUSTUP_HOME` | Rustup 主目录 | 安装时 |
| `RUSTUP_DIST_SERVER` | Rust 工具链镜像 | 安装时 |
| `RUSTUP_UPDATE_ROOT` | Rustup 更新地址 | 安装时 |
| `MODE` | 强制运行模式 (installer/manager) | 运行时 |
| `RIM_DIST_SERVER` | 覆盖默认分发服务器 | 运行时 |
| `EDITION` | 指定构建版本 | 构建时 |
| `http_proxy` / `https_proxy` | 代理设置 | 安装时 (如配置) |
| `no_proxy` | 不走代理的地址 | 安装时 (如配置) |

### 附录 B: 错误处理策略

| 场景 | 策略 |
| ---- | ---- |
| 安装失败 | 记录错误日志，不自动回滚 (TODO: 实现回滚) |
| 卸载失败 | 记录警告，继续卸载其他组件 |
| 文件操作失败 | 重试机制（最多 10 次） |
| 网络失败 | 断点续传 + 自动重试 |

### 附录 C: 联系方式

| 渠道 | 地址 |
| ---- | ---- |
| Issue 反馈 | https://gitcode.com/xuanwu/custom-rust-dist/issues |
| 邮箱 | liyuan179@huawei.com |
| 社区官网 | https://xuanwu.openatom.cn |

---

> 本文档基于旋武社区 Rust 发行版项目 (R.I.M) 编写，文档中 `<!-- -->` 注释部分为待补充内容。
