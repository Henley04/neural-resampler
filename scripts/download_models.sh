#!/usr/bin/env bash
# 下载 neural-resampler 所需的 ONNX 模型。
#
#   bash scripts/download_models.sh            # → ./models
#   bash scripts/download_models.sh /path/dir  # → 指定目录
#
# 模型来源于第三方仓库，许可遵循各自发布页要求。
# 下载完成后做 SHA-256 校验（基线为 v0.1.0 一起验证过的版本）；
# 上游更新导致哈希变化时，可用 NR_MODEL_SKIP_CHECKSUM=1 临时跳过校验。
set -euo pipefail

DEST="${1:-$(cd "$(dirname "$0")/.." && pwd)/models}"
mkdir -p "$DEST"

# 若直连 GitHub 不稳定，可改用镜像前缀（如 https://ghfast.top/）
MIRROR="${NR_MODEL_MIRROR:-}"
SKIP_CHECKSUM="${NR_MODEL_SKIP_CHECKSUM:-0}"

# 社区已导出的 ONNX（PC-NSF-HiFiGAN 44.1k/hop512/128bin 与 FCPE）
FCPE_URL="https://raw.githubusercontent.com/open-ai-tuning/HachiTune/master/models/fcpe.onnx"
VOCODER_URL="https://raw.githubusercontent.com/open-ai-tuning/HachiTune/master/models/pc_nsf_hifigan.onnx"

# SHA-256 基线：与 v0.1.0 一同验证过的版本。
# 上游更换模型（哈希不匹配）时脚本会拒绝安装并说明处理办法；
# 确认新版本可用后可更新此表。
FCPE_SHA256="013b507acd406de122b61ee497300ac6be082511101bc2136b886e48d9737e5d"
VOCODER_SHA256="d1d0edcbd45a9fbbbb5387d8a2913f4d5a0fc21755692c5d511951fdb910c00b"

sha256_of() {
    # 兼容 GNU（sha256sum）与 macOS（shasum -a 256）
    if command -v sha256sum >/dev/null 2>&1; then
        sha256sum "$1" | awk '{print $1}'
    else
        shasum -a 256 "$1" | awk '{print $1}'
    fi
}

verify() {
    local file="$1" expected="$2" name="$3"
    if [[ "$SKIP_CHECKSUM" == "1" ]]; then
        echo "⚠ 跳过 $name 的 SHA-256 校验（NR_MODEL_SKIP_CHECKSUM=1）"
        return 0
    fi
    local actual
    actual="$(sha256_of "$file")"
    if [[ "$actual" != "$expected" ]]; then
        echo "✗ $name SHA-256 校验失败" >&2
        echo "  期望: $expected" >&2
        echo "  实际: $actual" >&2
        echo "  文件可能已损坏，或上游已更换模型。" >&2
        rm -f "$file"
        echo "  已删除该文件；可重跑本脚本重新下载。" >&2
        echo "  若确认上游模型已更新，可用 NR_MODEL_SKIP_CHECKSUM=1 跳过校验。" >&2
        return 1
    fi
    echo "✓ $name SHA-256 校验通过"
}

fetch() {
    local url="$1" out="$2" name="$3" expected="$4"
    if [[ -f "$out" ]]; then
        echo "已存在 $name：$out"
        verify "$out" "$expected" "$name"
        return 0
    fi
    echo "下载 $name → $out"
    # 失败自动重试并支持断点续传
    local attempt=0
    until curl -fL --retry 5 --retry-all-errors --retry-delay 3 -C - \
        -o "$out" "${MIRROR}${url}"; do
        attempt=$((attempt + 1))
        if ((attempt >= 3)); then
            echo "下载 $name 失败" >&2
            rm -f "$out"
            return 1
        fi
        echo "重试 $name（第 $attempt 次）..."
    done
    echo "完成 $name：$(du -h "$out" | cut -f1)"
    verify "$out" "$expected" "$name"
}

fetch "$FCPE_URL" "$DEST/fcpe.onnx" "FCPE" "$FCPE_SHA256"
fetch "$VOCODER_URL" "$DEST/pc_nsf_hifigan.onnx" "PC-NSF-HiFiGAN" "$VOCODER_SHA256"

echo
echo "模型目录：$DEST"
ls -la "$DEST"
echo
echo "用 'resampler info' 确认引擎能否加载模型。"
