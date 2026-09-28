#!/usr/bin/env bash
# 下载 neural-resampler 所需的 ONNX 模型。
#
#   bash scripts/download_models.sh            # → ./models
#   bash scripts/download_models.sh /path/dir  # → 指定目录
#
# 模型来源于第三方仓库，许可遵循各自发布页要求。
set -euo pipefail

DEST="${1:-$(cd "$(dirname "$0")/.." && pwd)/models}"
mkdir -p "$DEST"

# 若直连 GitHub 不稳定，可改用镜像前缀（如 https://ghfast.top/）
MIRROR="${NR_MODEL_MIRROR:-}"

# 社区已导出的 ONNX（PC-NSF-HiFiGAN 44.1k/hop512/128bin 与 FCPE）
FCPE_URL="https://raw.githubusercontent.com/open-ai-tuning/HachiTune/master/models/fcpe.onnx"
VOCODER_URL="https://raw.githubusercontent.com/open-ai-tuning/HachiTune/master/models/pc_nsf_hifigan.onnx"

fetch() {
    local url="$1" out="$2" name="$3"
    if [[ -f "$out" ]]; then
        echo "跳过 $name（已存在）：$out"
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
}

fetch "$FCPE_URL" "$DEST/fcpe.onnx" "FCPE"
fetch "$VOCODER_URL" "$DEST/pc_nsf_hifigan.onnx" "PC-NSF-HiFiGAN"

echo
echo "模型目录：$DEST"
ls -la "$DEST"
echo
echo "用 'resampler info' 确认引擎能否加载模型。"
