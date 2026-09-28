#!/usr/bin/env bash
# 构建可分发产物：把二进制、默认配置与模型打包成一个压缩包。
#
#   bash scripts/build_release.sh              # 输出到 ./dist
#   bash scripts/build_release.sh --features cuda
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

FEATURES=""
while [[ $# -gt 0 ]]; do
    case "$1" in
        --features) FEATURES="$2"; shift 2 ;;
        -h|--help) echo "用法: $0 [--features FEATURES]"; exit 0 ;;
        *) echo "未知参数: $1" >&2; exit 2 ;;
    esac
done

TARGET="$(rustc -vV | awk '/^host:/{print $2}')"
NAME="neural-resampler-${TARGET}"
DIST="dist/${NAME}"

echo "==> 构建 release（features: ${FEATURES:-default}）"
if [[ -n "$FEATURES" ]]; then
    cargo build --release --features "$FEATURES"
else
    cargo build --release
fi

rm -rf "$DIST"
mkdir -p "$DIST/models"

BIN="target/release/resampler"
[[ -f "$BIN.exe" ]] && BIN="$BIN.exe"
cp "$BIN" "$DIST/"
cp config/resampler.yaml "$DIST/config.resampler.yaml"
cp README.md LICENSE "$DIST/" 2>/dev/null || true
cp -r docs "$DIST/" 2>/dev/null || true

if compgen -G "models/*.onnx" > /dev/null; then
    cp models/*.onnx "$DIST/models/"
    echo "==> 已附带模型"
else
    echo "==> 未找到模型，产物不含 models/（可用 scripts/download_models.sh 获取）"
fi

(cd dist && tar czf "${NAME}.tar.gz" "$NAME")
echo "==> 完成: dist/${NAME}.tar.gz"
