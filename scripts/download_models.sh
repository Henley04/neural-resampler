#!/usr/bin/env bash
# 下载 neural-resampler 所需的 ONNX 模型。
#
#   bash scripts/download_models.sh            # 仓库内：下载到 仓库根/models
#   ./download_models.sh                       # 发布包内：下载到 包内 models/
#   ./download_models.sh /path/dir             # 指定目录
#
# 目标目录自动按脚本位置判断：脚本与 resampler（.exe）同级 → 发布包布局，
# 否则按仓库布局（scripts/ 的上一级）。
#
# 下载源策略（PowerShell 版 download_models.ps1 逻辑一致）：
#   1. 设置 NR_MODEL_MIRROR 时直接使用该镜像前缀
#      （gh-proxy 格式：镜像前缀 + 完整原始 URL，如 https://ghfast.top/https://raw...）
#   2. 否则先探测 GitHub 直连；不可达时自动切换 gh-proxy 镜像并提示
#   3. 下载中平均速度低于 100KB/s 持续 10 秒：
#      当前为直连时询问是否切换镜像（NR_MODEL_AUTO_SWITCH=1 免询问自动切换，
#      非交互环境提示后按原源继续）；当前已是镜像则提示后继续重试
#   4. 切换下载源后从头下载（gh-proxy 不保证 Range 续传）
#   5. 无论来源如何，下载完成后都做 SHA-256 校验——镜像内容被篡改会被拦下
#
# 模型来源于第三方仓库，许可遵循各自发布页要求。
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"

# ---- 目标目录：兼容仓库布局与发布包布局 ----
if [[ -f "$SCRIPT_DIR/resampler" || -f "$SCRIPT_DIR/resampler.exe" ]]; then
    DEST="${1:-$SCRIPT_DIR/models}"       # 发布包：脚本与 resampler 同级
else
    DEST="${1:-$SCRIPT_DIR/../models}"    # 仓库：脚本在 scripts/ 下
fi
mkdir -p "$DEST"

MIRROR_EXPLICIT="${NR_MODEL_MIRROR:-}"
SKIP_CHECKSUM="${NR_MODEL_SKIP_CHECKSUM:-0}"
AUTO_SWITCH="${NR_MODEL_AUTO_SWITCH:-0}"
DEFAULT_MIRROR="https://ghfast.top/"    # gh-proxy 生态实例，可用 NR_MODEL_MIRROR 覆盖
MIN_SPEED=102400                        # 100 KB/s
SLOW_SECONDS=10                         # 平均速度持续低于阈值的判定时长

# 社区已导出的 ONNX（PC-NSF-HiFiGAN 44.1k/hop512/128bin 与 FCPE）
FCPE_URL="https://raw.githubusercontent.com/open-ai-tuning/HachiTune/master/models/fcpe.onnx"
VOCODER_URL="https://raw.githubusercontent.com/open-ai-tuning/HachiTune/master/models/pc_nsf_hifigan.onnx"

# SHA-256 基线：与 v0.1.0 一同验证过的版本。
# 上游更换模型（哈希不匹配）时脚本会拒绝安装并说明处理办法；
# 确认新版本可用后可更新此表。
FCPE_SHA256="013b507acd406de122b61ee497300ac6be082511101bc2136b886e48d9737e5d"
VOCODER_SHA256="d1d0edcbd45a9fbbbb5387d8a2913f4d5a0fc21755692c5d511951fdb910c00b"

BASE=""            # 下载前缀："" = GitHub 直连；非空 = 镜像前缀
USING_MIRROR=0
MIRROR_ALREADY_USED=0

is_interactive() { [[ -t 0 ]]; }

probe_github() {
    # HEAD 探测直连可达性（连接超时 4s，总超时 8s）
    curl -fsI --connect-timeout 4 --max-time 8 -o /dev/null "$FCPE_URL" 2>/dev/null
}

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
        echo "  文件可能已损坏、被下载源篡改，或上游已更换模型。" >&2
        rm -f "$file"
        echo "  已删除该文件；可重跑本脚本重新下载。" >&2
        echo "  若确认上游模型已更新，可用 NR_MODEL_SKIP_CHECKSUM=1 跳过校验。" >&2
        return 1
    fi
    echo "✓ $name SHA-256 校验通过"
}

switch_to_mirror() {
    BASE="$DEFAULT_MIRROR"
    USING_MIRROR=1
    MIRROR_ALREADY_USED=1
}

ask_switch_mirror() {
    if [[ "$AUTO_SWITCH" == "1" ]]; then
        echo "⚠ 下载速度低于 100KB/s，NR_MODEL_AUTO_SWITCH=1 → 自动切换镜像" >&2
        return 0
    fi
    if ! is_interactive; then
        echo "⚠ 下载速度低于 100KB/s。非交互环境跳过询问（可设 NR_MODEL_AUTO_SWITCH=1 自动切换镜像）" >&2
        return 1
    fi
    local ans=""
    read -r -p "下载速度低于 100KB/s，是否切换 gh-proxy 镜像重新下载？[Y/n] " ans </dev/tty 2>/dev/null || ans=""
    [[ ! "$ans" =~ ^[Nn] ]]
}

download_one() {
    local url="$1" out="$2" name="$3"
    local attempt=0 rc=0
    while true; do
        echo "下载 $name ← ${BASE}${url}"
        set +e
        # --speed-limit/--speed-time：平均速度低于 100KB/s 持续 10s 时以 28 退出
        curl -fL --speed-limit "$MIN_SPEED" --speed-time "$SLOW_SECONDS" \
            --connect-timeout 15 -o "$out" "${BASE}${url}"
        rc=$?
        set -e
        case "$rc" in
            0)
                return 0
                ;;
            28)  # 慢速
                if [[ "$USING_MIRROR" == "0" && "$MIRROR_ALREADY_USED" == "0" ]] && ask_switch_mirror; then
                    switch_to_mirror
                    rm -f "$out"
                    echo "已切换镜像，从头重新下载 $name" >&2
                    continue
                fi
                echo "⚠ $name 下载速度过慢（低于 100KB/s），按当前源重试" >&2
                ;;
            *)
                echo "下载 $name 失败（curl 退出码 $rc）" >&2
                ;;
        esac
        attempt=$((attempt + 1))
        if ((attempt >= 3)); then
            # 直连重试耗尽且尚未用过镜像 → 自动切换镜像做最后一轮
            if [[ "$USING_MIRROR" == "0" && "$MIRROR_ALREADY_USED" == "0" ]]; then
                echo "直连多次失败，切换 gh-proxy 镜像重试 $name" >&2
                switch_to_mirror
                rm -f "$out"
                attempt=0
                continue
            fi
            rm -f "$out"
            return 1
        fi
        rm -f "$out"
        echo "重试 $name（第 $attempt 次）..."
        sleep 3
    done
}

fetch() {
    local url="$1" out="$2" name="$3" expected="$4"
    if [[ -f "$out" ]]; then
        echo "已存在 $name：$out"
        verify "$out" "$expected" "$name"
        return 0
    fi
    if download_one "$url" "$out" "$name"; then
        echo "完成 $name：$(du -h "$out" | cut -f1)"
        verify "$out" "$expected" "$name"
        return 0
    fi
    echo "✗ $name 下载失败，请检查网络后重跑本脚本（可设 NR_MODEL_MIRROR=镜像前缀 强制走镜像）" >&2
    return 1
}

# ---- 选择下载源 ----
if [[ -n "$MIRROR_EXPLICIT" ]]; then
    switch_to_mirror
    BASE="$MIRROR_EXPLICIT"
    echo "使用指定镜像：$BASE"
elif probe_github; then
    echo "GitHub 直连可用"
else
    switch_to_mirror
    echo "⚠ GitHub 主站不可达，自动切换 gh-proxy 镜像：$BASE" >&2
fi

fetch "$FCPE_URL" "$DEST/fcpe.onnx" "FCPE" "$FCPE_SHA256"
fetch "$VOCODER_URL" "$DEST/pc_nsf_hifigan.onnx" "PC-NSF-HiFiGAN" "$VOCODER_SHA256"

echo
echo "模型目录：$DEST"
ls -la "$DEST"
echo
echo "用 'resampler info' 确认引擎能否加载模型。"
