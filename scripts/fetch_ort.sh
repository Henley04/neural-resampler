#!/usr/bin/env bash
# 预取并校验 ort-sys `download-binaries` 所需的 ONNX Runtime 静态库。
#
# 为什么需要这个脚本：
#   ort-sys 2.0.0-rc.13 的 build script 在下载/解压失败时只用 `cargo::error=`
#   打印一条消息然后正常退出（exit 0），并可能在缓存目录留下不完整状态；
#   而缓存目录一旦存在，后续构建会跳过下载直接输出
#     -L <cache>/dfbin/<target>/<hash>  -l static=onnxruntime
#   最终 rustc 以
#     error: could not find native static library `onnxruntime`
#   在链接阶段才报错（表现为 `cargo build` exit 101）。
#
# 本脚本把这一步提前、可校验（SHA-256）、可重试（curl --retry）地完成，
# 并在缓存目录存在但缺少静态库时自动清除重建。
#
# 注意：URL 与 SHA-256 来自 ort-sys 2.0.0-rc.13 内置的 dist.tsv。
# ort-sys 未精确匹配 feature set 时按 dist.tsv 顺序取第一个候选，
# 因此 macOS 实际取 coreml 版、Windows 实际取 directml 版（与
# ort-sys build script 的 resolve_dist 行为一致）。
# 若升级 ort/ort-sys 版本，必须同步更新下表。
set -euo pipefail

CACHE_DIR="${ORT_CACHE_DIR:-$HOME/.cache/ort.pyke.io}"
# Windows 上 GitHub 传来的路径可能含反斜杠（D:\a\...），统一成正斜杠，
# git-bash 与 Rust 侧都能正确处理。
CACHE_DIR="${CACHE_DIR//\\//}"

case "${RUNNER_OS:-}" in
  Linux)
    TARGET="x86_64-unknown-linux-gnu"; SUFFIX=""
    HASH="e454f710f8a49f53aa5b4ff51e3454ae1835777e431c6c35c5255ce6f205fd68"
    LIB="libonnxruntime.a"
    ;;
  macOS)
    TARGET="aarch64-apple-darwin"; SUFFIX="+coreml"
    HASH="6934874e2e953576d9c1db47ff1af39c62c4f4220dbe6f988e131f72879674c7"
    LIB="libonnxruntime.a"
    ;;
  Windows)
    TARGET="x86_64-pc-windows-msvc"; SUFFIX="+directml"
    HASH="f7c654b3729cb9e5ad2a36a0c38e5b48e63bf4eed22968931aed33a0ad0b527d"
    LIB="onnxruntime.lib"
    ;;
  *)
    echo "::error::无法识别的 RUNNER_OS: '${RUNNER_OS:-<空>}'（本脚本用于 GitHub Actions runner）"
    exit 1
    ;;
esac

# ORT_PREFETCH_BASE_URL 仅用于本地测试覆盖下载源，CI 上保持默认。
BASE_URL="${ORT_PREFETCH_BASE_URL:-https://cdn.pyke.io/0/pyke:ort-rs/ms@1.28.0}"
URL="$BASE_URL/${TARGET}${SUFFIX}.tar.lzma2"
DEST="$CACHE_DIR/dfbin/$TARGET/$HASH"

sha256_of() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$1" | cut -d' ' -f1
  else
    shasum -a 256 "$1" | cut -d' ' -f1
  fi
}

# 提示：脚本内的 URL/HASH 与 ort-sys 版本绑定
ORT_SYS_VER="$(grep -A1 'name = "ort-sys"' Cargo.lock 2>/dev/null | grep version | cut -d'"' -f2 || true)"
if [ -n "$ORT_SYS_VER" ] && [ "$ORT_SYS_VER" != "2.0.0-rc.13" ]; then
  echo "::warning::Cargo.lock 中 ort-sys 版本为 $ORT_SYS_VER，而本脚本的 URL/HASH 对应 2.0.0-rc.13，请同步更新 scripts/fetch_ort.sh"
fi

# 1. 缓存完整 → 直接使用
if [ -f "$DEST/$LIB" ]; then
  echo "[ort-prefetch] 命中缓存: $DEST/$LIB"
  exit 0
fi

# 2. 目录存在但缺静态库（曾导致 CI 链接失败的坏状态）→ 清除重建
if [ -d "$DEST" ]; then
  echo "::warning::缓存目录 $DEST 存在但缺少 $LIB，删除后重新获取"
  rm -rf "$DEST"
fi

echo "[ort-prefetch] 下载 $URL"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

curl -fL --retry 5 --retry-delay 5 --retry-all-errors --connect-timeout 30 \
  -o "$TMP/ort.tar.lzma2" "$URL"

ACTUAL="$(sha256_of "$TMP/ort.tar.lzma2")"
if [ "$ACTUAL" != "$HASH" ]; then
  echo "::error::ONNX Runtime 包校验失败：期望 $HASH，实际 $ACTUAL"
  exit 1
fi
echo "[ort-prefetch] SHA-256 校验通过"

# 3. 解压：.tar.lzma2 是裸 LZMA2 流（非 .xz 容器），用 Python 解
PY=python3
command -v python3 >/dev/null 2>&1 || PY=python
"$PY" - "$TMP/ort.tar.lzma2" "$TMP/ort.tar" <<'PYEOF'
import lzma, sys

with open(sys.argv[1], "rb") as fin:
    d = lzma.LZMADecompressor(
        format=lzma.FORMAT_RAW, filters=[{"id": lzma.FILTER_LZMA2}]
    )
    with open(sys.argv[2], "wb") as fout:
        while True:
            chunk = fin.read(1 << 20)
            if not chunk:
                break
            data = d.decompress(chunk)
            if data:
                fout.write(data)
        try:  # flush 尾部；流带 end marker 时已到 eof，再调用会抛 EOFError
            data = d.decompress(b"")
            if data:
                fout.write(data)
        except (EOFError, lzma.LZMAError):
            pass
PYEOF

STAGE="$DEST.tmp"
rm -rf "$STAGE"
mkdir -p "$STAGE"
tar -xf "$TMP/ort.tar" -C "$STAGE"

# 4. 解压产物必须包含静态库，否则拒绝落盘
if [ ! -f "$STAGE/$LIB" ]; then
  echo "::error::解压后未找到 $LIB，实际内容："
  ls -la "$STAGE" || true
  exit 1
fi

mv "$STAGE" "$DEST"
echo "[ort-prefetch] 就绪: $DEST/$LIB ($(du -h "$DEST/$LIB" | cut -f1))"
