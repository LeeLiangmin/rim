#!/usr/bin/env python3
"""GitCode Release API 工具

完整流程：创建 Release → 获取上传地址 → 上传附件 → 更新 Release

用法:
    # 通过 CLI 参数 + 环境变量使用
    python3 release.py --tag v0.1.0 --files dist/*.tar.gz

    # 指定配置文件
    python3 release.py --config config.toml

    # 列出所有 release
    python3 release.py --list

环境变量:
    GITCODE_TOKEN      - access_token（必需）
    GITCODE_BASE_URL   - API 基础地址（可选，默认 https://api.gitcode.com/api/v5）
    GITCODE_OWNER      - 仓库 owner（可选，默认 xuanwu）
    GITCODE_REPO       - 仓库名（可选，默认 custom-rust-dist）
"""

from __future__ import annotations

import argparse
import os
import sys
from pathlib import Path

import httpx

# Python 3.11+ 内置 tomllib，3.10 用 tomli
try:
    import tomllib
except ModuleNotFoundError:
    try:
        import tomli as tomllib  # type: ignore[no-redef]
    except ModuleNotFoundError:
        tomllib = None  # type: ignore[assignment]

# ---------------------------------------------------------------------------
# 配置
# ---------------------------------------------------------------------------

API_BASE = os.environ.get("GITCODE_BASE_URL", "https://api.gitcode.com/api/v5")
TIMEOUT = httpx.Timeout(30.0, connect=10.0)
UPLOAD_TIMEOUT = httpx.Timeout(600.0, connect=30.0)  # 大文件上传用更长超时


def load_config(path: Path | None) -> dict:
    """读取 TOML 配置文件，返回字典。文件不存在或未指定则返回空字典。"""
    if path is None:
        return {}
    if not path.exists():
        print(f"⚠️  配置文件不存在: {path}，使用纯 CLI 参数模式")
        return {}
    if tomllib is None:
        print(f"⚠️  tomllib/tomli 不可用，跳过配置文件: {path}")
        return {}
    with path.open("rb") as f:
        cfg = tomllib.load(f)
    print(f"📄 已加载配置: {path}")
    return cfg


# ---------------------------------------------------------------------------
# API 封装
# ---------------------------------------------------------------------------


class GitCodeRelease:
    """对 GitCode Release API 的轻量封装。

    认证方式：按 API 文档要求，通过 access_token query 参数传递。
    """

    def __init__(self, owner: str, repo: str, access_token: str) -> None:
        self.owner = owner
        self.repo = repo
        self.access_token = access_token
        self.base = f"{API_BASE}/repos/{owner}/{repo}"
        self.client = httpx.Client(
            timeout=TIMEOUT,
            params={"access_token": access_token},
        )

    # -- 1. 创建 Release ------------------------------------------------

    def create_release(
        self,
        tag: str,
        name: str | None = None,
        body: str = "",
        target_commitish: str | None = None,
    ) -> dict:
        """POST /repos/:owner/:repo/releases"""
        payload: dict = {
            "tag_name": tag,
            "name": name or tag,
            "body": body,
        }
        if target_commitish:
            payload["target_commitish"] = target_commitish

        resp = self.client.post(f"{self.base}/releases", json=payload)
        if resp.status_code == 200:
            print(f"✅ Release 创建成功: {tag}")
            return resp.json()

        # release 可能已存在，尝试获取
        print(f"⚠️  创建返回 {resp.status_code}，尝试获取已有 release...")
        return self.get_release(tag)

    # -- 2. 更新 Release ------------------------------------------------

    def update_release(self, tag: str, name: str | None = None, body: str | None = None) -> dict:
        """PATCH /repos/:owner/:repo/releases/:tag

        API 要求 name 和 body 都必填，缺省时自动从当前 release 补全。
        """
        if name is None or body is None:
            current = self.get_release(tag)
            if name is None:
                name = current.get("name", tag)
            if body is None:
                body = current.get("body", "")

        payload = {"name": name, "body": body}

        resp = self.client.patch(
            f"{self.base}/releases/{tag}",
            json=payload,
            headers={"Content-Type": "application/json"},
        )
        resp.raise_for_status()
        print(f"✅ Release 已更新: {tag}")
        return resp.json()

    # -- 3. 获取上传地址 -------------------------------------------------

    def get_upload_url(self, tag: str, file_name: str) -> tuple[str, dict]:
        """GET /repos/:owner/:repo/releases/:tag/upload_url

        返回 (url, headers)，url 用 PUT 上传。
        """
        resp = self.client.get(
            f"{self.base}/releases/{tag}/upload_url",
            params={"file_name": file_name},
        )
        resp.raise_for_status()
        data = resp.json()
        url = data.get("url", "")
        headers = data.get("headers", {})
        if not url:
            print(f"❌ 未获取到上传地址: {data}", file=sys.stderr)
            sys.exit(1)
        return url, headers

    # -- 4. 上传附件 -----------------------------------------------------

    def upload_file(self, tag: str, filepath: Path) -> None:
        """获取上传地址后，使用 PUT 上传文件。

        注意：上传地址是预签名 URL（自带 AccessKeyId/Signature），
        不能附加 access_token，因此使用独立的 httpx 请求。
        """
        file_name = filepath.name
        file_size = filepath.stat().st_size
        print(f"📦 准备上传: {file_name} ({file_size / 1024 / 1024:.1f} MB)")

        upload_url, extra_headers = self.get_upload_url(tag, file_name)
        print(f"   上传地址: {upload_url}")
        if extra_headers:
            print(f"   附加 headers: {list(extra_headers.keys())}")

        with filepath.open("rb") as f:
            content = f.read()
            print(f"   读取完成, 准备 PUT ({len(content)} bytes)...")
            resp = httpx.put(
                upload_url,
                content=content,
                headers=extra_headers,
                timeout=UPLOAD_TIMEOUT,
            )
        print(f"   PUT 响应: HTTP {resp.status_code}, body={resp.text[:200]}")
        resp.raise_for_status()
        print(f"✅ 上传成功: {file_name}")

    # -- 5. 获取单个 Release ---------------------------------------------

    def get_release(self, tag: str) -> dict:
        """GET /repos/:owner/:repo/releases/:tag"""
        resp = self.client.get(f"{self.base}/releases/{tag}")
        resp.raise_for_status()
        return resp.json()

    # -- 6. 获取全部 Release ---------------------------------------------

    def list_releases(self, page: int = 1, per_page: int = 20) -> list[dict]:
        """GET /repos/:owner/:repo/releases"""
        resp = self.client.get(
            f"{self.base}/releases",
            params={"page": page, "per_page": per_page},
        )
        resp.raise_for_status()
        return resp.json()


# ---------------------------------------------------------------------------
# CLI
# ---------------------------------------------------------------------------


def parse_args() -> argparse.Namespace:
    p = argparse.ArgumentParser(
        description="GitCode Release 操作工具（创建 / 上传 / 更新）",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog=__doc__,
    )
    p.add_argument("--config", default=None, help="配置文件路径（可选）")
    p.add_argument("--tag", default=None, help="Release 对应的 tag 名称")
    p.add_argument("--name", default=None, help="Release 名称（默认同 tag）")
    p.add_argument("--body", default=None, help="Release 描述")
    p.add_argument("--target-commitish", default=None, help="分支或 commit SHA")
    p.add_argument("--files", nargs="*", default=None, help="要上传的文件路径")
    p.add_argument("--update-body", default=None, help="上传完成后更新 Release 描述")
    p.add_argument("--list", action="store_true", help="列出所有 release 后退出")
    return p.parse_args()


def main() -> None:
    args = parse_args()

    # 加载配置文件（仅在显式指定 --config 时）
    config_path = Path(args.config) if args.config else None
    cfg = load_config(config_path)

    cfg_gitcode = cfg.get("gitcode", {})
    cfg_release = cfg.get("release", {})
    cfg_upload = cfg.get("upload", {})
    cfg_update = cfg.get("update", {})

    # 合并参数：CLI > 环境变量 > config.toml
    access_token = os.environ.get("GITCODE_TOKEN") or cfg_gitcode.get("access_token")
    if not access_token:
        print("❌ 缺少 access_token：请设置 GITCODE_TOKEN 环境变量或通过 --config 指定配置文件", file=sys.stderr)
        sys.exit(1)

    owner = os.environ.get("GITCODE_OWNER") or cfg_gitcode.get("owner", "xuanwu")
    repo = os.environ.get("GITCODE_REPO") or cfg_gitcode.get("repo", "custom-rust-dist")

    api = GitCodeRelease(owner, repo, access_token)

    # 列出所有 release
    if args.list:
        releases = api.list_releases()
        for r in releases:
            print(f"  {r.get('tag_name', '?'):20s}  {r.get('name', '')}")
        return

    # tag: CLI > config.toml
    tag = args.tag or cfg_release.get("tag")
    if not tag:
        print("❌ 缺少 --tag 参数", file=sys.stderr)
        sys.exit(1)

    release_name = args.name or cfg_release.get("name")
    release_body = args.body if args.body is not None else cfg_release.get("body", "")
    target_commitish = args.target_commitish or cfg_release.get("target_commitish")

    # 文件列表: CLI > config.toml
    if args.files is not None:
        file_paths = [Path(f) for f in args.files]
    else:
        file_paths = [Path(f) for f in cfg_upload.get("files", [])]

    # 更新描述: CLI > config.toml
    update_body = args.update_body if args.update_body is not None else cfg_update.get("body")

    # ---------------------------------------------------------------
    print(f"\n📋 参数汇总:")
    print(f"   owner:  {owner}")
    print(f"   repo:   {repo}")
    print(f"   tag:    {tag}")
    print(f"   name:   {release_name or tag}")
    print(f"   files:  {len(file_paths)} 个")

    # Step 1: 创建 Release
    print(f"\n{'='*50}")
    print(f"🚀 Step 1: 创建 Release (tag={tag})")
    print(f"{'='*50}")
    release = api.create_release(
        tag=tag,
        name=release_name,
        body=release_body,
        target_commitish=target_commitish,
    )
    print(f"   tag_name: {release.get('tag_name')}")
    print(f"   name:     {release.get('name')}")

    # Step 2: 上传附件
    if file_paths:
        print(f"\n{'='*50}")
        print(f"📤 Step 2: 上传附件 ({len(file_paths)} 个文件)")
        print(f"{'='*50}")
        for fp in file_paths:
            if not fp.exists():
                print(f"⚠️  文件不存在，跳过: {fp}", file=sys.stderr)
                continue
            api.upload_file(tag, fp)

        # 验证上传结果
        print(f"\n{'='*50}")
        print(f"🔍 Step 2.1: 验证上传结果")
        print(f"{'='*50}")
        try:
            release_data = api.get_release(tag)
            assets = release_data.get("assets", [])
            asset_names = {a.get("name", "") for a in assets}
            uploaded_names = {fp.name for fp in file_paths if fp.exists()}
            missing = uploaded_names - asset_names
            if missing:
                print(f"⚠️  以下文件未在 release assets 中找到: {missing}")
                print(f"   release 当前 assets: {asset_names}")
            else:
                print(f"✅ 所有 {len(uploaded_names)} 个文件已确认出现在 release assets 中")
        except Exception as e:
            print(f"⚠️  验证失败（不影响上传）: {e}")
    else:
        print("\n⏭️  Step 2: 无文件需要上传")

    # Step 3: 更新 Release
    if update_body is not None:
        print(f"\n{'='*50}")
        print(f"📝 Step 3: 更新 Release 描述")
        print(f"{'='*50}")
        api.update_release(tag, body=update_body)
    else:
        print("\n⏭️  Step 3: 无需更新描述")

    print(f"\n🎉 完成！Release {tag} 处理完毕。")


if __name__ == "__main__":
    main()
