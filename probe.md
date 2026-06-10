# CI Runner 环境探测结果

> 来源: env-check workflow, 2026-06-10

## 1. OS

| 项目 | 值 |
|------|-----|
| 发行版 | EulerOS 2.0 (SP10 x86_64) |
| 内核 | 4.18.0 (eulerosv2r9) |
| glibc | 2.28-63 (≈ RHEL 8, NOT CentOS 7) |
| CPU | 16 核 Intel Xeon Gold 6161 @ 2.20GHz |
| 内存 | 30 GB (无 swap) |
| 磁盘 | 196 GB overlay (178G 可用) |
| 用户 | octopus:octopusgroup, HOME=/home/octopus |

## 2. 预装工具链

| 工具 | 版本 | 备注 |
|------|------|------|
| gcc | 7.3.0 | **无 g++** |
| binutils | 2.34 | 含 devel/extra |
| make | 4.3 | |
| cmake | 3.16.5 | |
| pkg-config | 1.7.3 | |
| git | 2.27.0 | |
| curl | 7.71.1 | OpenSSL/1.1.1k-fips |
| wget | 1.20.3 | |
| python3 | 3.8.18 (pip 23.0.1) | /opt/tools/python/3.8.18/ |
| node | v18.18.2 (npm 9.8.1) | **无 pnpm** |
| docker | **未安装且无法安装** | yum 已修复但零仓库，见 §2.1 和 §4 |
| openssl | 1.1.1k-fips | |
| OpenGL | mesa 20.1.4 | 有 libEGL/libGL |

**未安装**: g++、pnpm、docker、llvm/clang、mingw-w64、aarch64-linux-gnu-gcc、musl-gcc、llvm-dlltool

## 2.1 包管理器 (yum/dnf) 修复过程

**根因**: `/bin/yum` 和 `/bin/dnf` 的 shebang 指向 `/usr/bin/python3`（不存在），且默认 `python3` 是 3.8.18，而 dnf/libdnf/rpm 的 C 扩展为 Python 3.7 编译，ABI 不兼容。

**修复**: 系统存在 `/usr/bin/python3.7` (Python 3.7.9)，可直接调用：
```
/usr/bin/python3.7 /bin/yum --version  # ✅ 正常输出 dnf 4.2.23
```

**新问题**: yum 可用但 **零个启用的仓库**：
```
Error: There are no enabled repositories in "/etc/yum.repos.d"
```
`yum repolist` 返回空，无法安装任何软件包。

## 3. 网络连通性

| 目标 | 状态 |
|------|------|
| mirror.xuanwu.openatom.cn | ✅ HTTP 200 |
| xuanwu-rust.obs.cn-north-4.myhuaweicloud.com | ❌ **list 返回 403** (但单文件可达) |
| mirrors.tuna.tsinghua.edu.cn | ✅ HTTP 200 |
| mirrors.ustc.edu.cn | ✅ HTTP 200 |
| github.com | ✅ HTTP 200 |
| api.gitcode.com | ✅ 301 redirect |

**OBS 单文件可达性**:

| 文件 | 大小 | 状态 |
|------|------|------|
| mingw-w64-cross.tar.xz | 72 MB | ✅ 200 |
| arm-gnu-toolchain-13.3...tar.xz | 138 MB | ✅ 200 |
| **llvm-dlltool-x86_64-linux** | — | ❌ **403 Forbidden** |

> **关键发现**: OBS bucket 的 list 操作返回 403，但单文件通过签名 URL 可达。`llvm-dlltool-x86_64-linux` 文件不存在或未公开 —— 这就是之前 OBS 下载一直失败的根因。

## 4. Docker 可用性

通过多轮 env-check workflow 测试：

| 步骤 | 结果 |
|------|------|
| yum/dnf shebang 修复 | ✅ 用 `/usr/bin/python3.7 /bin/yum` 可正常执行 |
| yum install docker | ❌ 失败 — **零个启用的 yum 仓库** |
| docker build test | ❌ 跳过 — `docker: command not found` |
| docker socket | ❌ 无 `/var/run/docker.sock`、`/run/docker.sock` |

**结论**: 此 CI runner 无法使用 Docker。yum 虽然可运行但无仓库，无法安装任何包。网络连通性正常，可通过 curl 下载静态 Docker 二进制文件绕过包管理器，但 Docker daemon 在 Kubernetes Pod 内本身就受限。

## 5. YUM 仓库问题

`yum repolist` 返回空，`/etc/yum.repos.d/` 下无启用的 `.repo` 文件。需要进一步排查：
- 是否有 `.repo` 文件在备份位置？
- 是否可手动创建指向可用镜像的 repo 文件？
- 已知可达的镜像: mirror.xuanwu.openatom.cn, mirrors.tuna.tsinghua.edu.cn, mirrors.ustc.edu.cn

## 5. 可复现镜像的 Dockerfile 起点

基于 EulerOS 2.0 SP10 (≈ openEuler 20.03 LTS / RHEL 8):

```dockerfile
FROM openeuler/openeuler:20.03-lts
# 或 FROM hub.oepkgs.net/openeuler/openeuler:20.03-lts

RUN yum install -y gcc gcc-c++ make cmake \
    binutils binutils-devel \
    openssl-devel pkg-config \
    git curl wget file \
    python38 python38-pip \
    nodejs npm \
    perl

# 缺少的需要额外安装:
# - mingw-w64 (交叉编译 windows)
# - musl-tools (静态链接)
# - pnpm (npm install -g pnpm)
# - docker (GUI 构建)
# - aarch64 交叉编译器 (需从 OBS 下载)
```

## 6. CI 平台信息

| 项目 | 值 |
|------|-----|
| 平台 | 华为云 CodeArts Pipeline (cn-north-4) |
| 执行模式 | Kubernetes Pod (octopus_container) |
| 触发方式 | Manual / branch push |
| 代码源 | GitCode (gitcode.com) |
| REPO_TYPE | gitcode |
| Go 代理 | https://mirrors.huaweicloud.com/repository/goproxy/ |
| Maven | /opt/tools/maven/3.8.8/ |

关键环境变量:
- `GITCODE_REF` / `GITCODE_REPOSITORY` / `GITCODE_SERVER_URL` → 代码源信息
- `GOPROXY` / `GO111MODULE=on` / `GONOSUMDB=*` → Go 环境已配好
- `SHARE_PATH=/data/workspace/{job_id}/share` → 代码工作区
- `MANIFEST_VERSION=2.0.0` → workflow 语法版本
```
