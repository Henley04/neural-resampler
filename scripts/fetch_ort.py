#!/usr/bin/env python3
"""预取并校验 ort-sys `download-binaries` 所需的 ONNX Runtime 静态库。

为什么需要这个脚本：
  ort-sys 2.0.0-rc.13 的 build script 在下载/解压失败时只用 `cargo::error=`
  打印一条消息然后正常退出（exit 0），并可能在缓存目录留下不完整状态；
  而缓存目录一旦存在，后续构建会跳过下载直接输出
    -L <cache>/dfbin/<target>/<hash>  -l static=onnxruntime
  最终 rustc 以
    error: could not find native static library `onnxruntime`
  在链接阶段才报错（表现为 `cargo build` exit 101）。

本脚本把这一步提前、可校验（SHA-256）、可重试地完成，并在缓存目录
存在但缺少静态库时自动清除重建。

注意：URL 与 SHA-256 来自 ort-sys 2.0.0-rc.13 内置的 dist.tsv。
ort-sys 未精确匹配 feature set 时按 dist.tsv 顺序取第一个候选，
因此 macOS 实际取 coreml 版、Windows 实际取 directml 版（与
ort-sys build script 的 resolve_dist 行为一致）。
若升级 ort/ort-sys 版本，必须同步更新 DISTS 表。
"""

from __future__ import annotations

import hashlib
import lzma
import os
import re
import shutil
import sys
import tarfile
import tempfile
import time
import urllib.request
import urllib.error
from pathlib import Path

ORT_SYS_VERSION = "2.0.0-rc.13"
BASE_URL = os.environ.get(
    "ORT_PREFETCH_BASE_URL",  # 仅用于本地测试覆盖下载源
    "https://cdn.pyke.io/0/pyke:ort-rs/ms@1.28.0",
)
USER_AGENT = "neural-resampler-ci/1.0 (ort-sys prebuilt fetcher; +https://github.com/Henley04/neural-resampler)"

# (target, dist 文件名后缀, sha256, 静态库文件名)
DISTS = {
    "Linux": (
        "x86_64-unknown-linux-gnu",
        "",
        "e454f710f8a49f53aa5b4ff51e3454ae1835777e431c6c35c5255ce6f205fd68",
        "libonnxruntime.a",
    ),
    "macOS": (
        "aarch64-apple-darwin",
        "+coreml",
        "6934874e2e953576d9c1db47ff1af39c62c4f4220dbe6f988e131f72879674c7",
        "libonnxruntime.a",
    ),
    "Windows": (
        "x86_64-pc-windows-msvc",
        "+directml",
        "f7c654b3729cb9e5ad2a36a0c38e5b48e63bf4eed22968931aed33a0ad0b527d",
        "onnxruntime.lib",
    ),
}


def info(msg: str) -> None:
    print(f"[ort-prefetch] {msg}", flush=True)


def fail(msg: str) -> "NoReturn":  # type: ignore[valid-type]
    print(f"::error::{msg}", flush=True)
    sys.exit(1)


def sha256_of(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def fetch(url: str, dst: Path, retries: int = 5) -> None:
    """带重试的下载；urllib 的 HTTPError 会给出明确的 HTTP 状态码。"""
    last: Exception | None = None
    for attempt in range(1, retries + 1):
        try:
            req = urllib.request.Request(url, headers={"User-Agent": USER_AGENT})
            with urllib.request.urlopen(req, timeout=120) as resp:
                info(f"HTTP {resp.status}，开始下载 {url}")
                with dst.open("wb") as f:
                    shutil.copyfileobj(resp, f, 1 << 20)
            return
        except Exception as exc:  # noqa: BLE001 - 需要把任何网络错误纳入重试
            last = exc
            info(f"下载失败（第 {attempt}/{retries} 次）: {exc!r}")
            if attempt < retries:
                time.sleep(5 * attempt)
    fail(f"下载最终失败: {url} — {last!r}")


def decompress_lzma2(src: Path, dst: Path) -> None:
    """解压裸 LZMA2 流（.tar.lzma2 不是 .xz 容器格式）。"""
    dec = lzma.LZMADecompressor(format=lzma.FORMAT_RAW, filters=[{"id": lzma.FILTER_LZMA2}])
    with src.open("rb") as fin, dst.open("wb") as fout:
        while True:
            chunk = fin.read(1 << 20)
            if not chunk:
                break
            data = dec.decompress(chunk)
            if data:
                fout.write(data)
        try:  # 流带 end marker 时已到 eof，再 flush 会抛错
            data = dec.decompress(b"")
            if data:
                fout.write(data)
        except (EOFError, lzma.LZMAError):
            pass


def extract_tar(tar_path: Path, stage: Path) -> None:
    with tarfile.open(tar_path) as tf:
        try:  # filter 参数需要 Python 3.11.4+；旧版本直接解包
            tf.extractall(stage, filter="data")
        except TypeError:
            tf.extractall(stage)


def main() -> int:
    runner_os = os.environ.get("RUNNER_OS", "")
    if runner_os not in DISTS:
        shown = runner_os or "<空>"
        fail(f"无法识别的 RUNNER_OS: '{shown}'（本脚本用于 GitHub Actions runner）")
    target, suffix, expected_sha, lib_name = DISTS[runner_os]
    url = f"{BASE_URL}/{target}{suffix}.tar.lzma2"

    cache_dir = Path(
        os.environ.get("ORT_CACHE_DIR")
        or Path.home() / ".cache" / "ort.pyke.io"
    )
    dest = cache_dir / "dfbin" / target / expected_sha

    # 与 ort-sys 版本绑定的一致性提示
    lock = Path("Cargo.lock")
    if lock.is_file():
        text = lock.read_text(encoding="utf-8", errors="replace")
        m = re.search(r'name = "ort-sys"\nversion = "([^"]+)"', text)
        if m and m.group(1) != ORT_SYS_VERSION:
            print(
                f"::warning::Cargo.lock 中 ort-sys 版本为 {m.group(1)}，"
                f"而本脚本的 URL/HASH 对应 {ORT_SYS_VERSION}，请同步更新 scripts/fetch_ort.py",
                flush=True,
            )

    # 1. 缓存完整 → 直接使用
    if (dest / lib_name).is_file():
        info(f"命中缓存: {dest / lib_name}")
        return 0

    # 2. 目录存在但缺静态库（曾导致 CI 链接失败的坏状态）→ 清除重建
    if dest.is_dir():
        print(
            f"::warning::缓存目录 {dest} 存在但缺少 {lib_name}，删除后重新获取",
            flush=True,
        )
        shutil.rmtree(dest)

    info(f"下载 {url}")
    with tempfile.TemporaryDirectory(prefix="ort-prefetch-") as tmp:
        tmp_path = Path(tmp)
        tgz = tmp_path / "ort.tar.lzma2"
        fetch(url, tgz)

        actual = sha256_of(tgz)
        if actual != expected_sha:
            fail(f"ONNX Runtime 包校验失败：期望 {expected_sha}，实际 {actual}")
        info("SHA-256 校验通过")

        tar_path = tmp_path / "ort.tar"
        decompress_lzma2(tgz, tar_path)

        stage = dest.with_name(dest.name + ".tmp")
        if stage.exists():
            shutil.rmtree(stage)
        stage.mkdir(parents=True)
        extract_tar(tar_path, stage)

        # 3. 解压产物必须包含静态库，否则拒绝落盘
        if not (stage / lib_name).is_file():
            entries = ", ".join(sorted(p.name for p in stage.iterdir())) or "<空>"
            fail(f"解压后未找到 {lib_name}，实际内容: {entries}")

    dest.parent.mkdir(parents=True, exist_ok=True)
    stage.rename(dest)
    size_mb = (dest / lib_name).stat().st_size / (1 << 20)
    info(f"就绪: {dest / lib_name} ({size_mb:.1f} MB)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
