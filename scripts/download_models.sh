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
# 提示语言：NR_MODEL_LANG=zh|ja|en|all 可强制指定；否则按系统 locale
# （LC_ALL/LC_MESSAGES/LANG）选择简体中文 / 日本語 / English；
# 检测不到或系统语言不属于三者时，三语同时显示。
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

# ---- 提示语言：NR_MODEL_LANG > 系统 locale > 三语同显 ----
detect_lang() {
    case "${NR_MODEL_LANG:-}" in
        zh|ja|en|all) printf '%s' "$NR_MODEL_LANG"; return ;;
    esac
    local l="${LC_ALL:-${LC_MESSAGES:-${LANG:-}}}"
    l="${l%%.*}"; l="${l%%[_-]*}"      # zh_CN.UTF-8 → zh
    case "$l" in
        zh) printf 'zh' ;;
        ja) printf 'ja' ;;
        en) printf 'en' ;;
        *)  printf 'all' ;;   # 检测不到或非三语 → 三语同时显示
    esac
}
UI_LANG="$(detect_lang)"

# msg <key> [args...]：按 UI_LANG 输出提示。
# all 模式下每条消息按 English → 日本語 → 简体中文 各占一行。
msg() {
    local key="$1"; shift
    local en="" ja="" zh=""
    case "$key" in
        lang_mode)
            if [[ "$UI_LANG" == "all" ]]; then
                en="Trilingual mode: system language is not one of English / 日本語 / 简体中文 (set NR_MODEL_LANG=zh|ja|en|all to override)"
                ja="三言語表示モード: システム言語が English / 日本語 / 簡体字中国語 のいずれにも一致しません（NR_MODEL_LANG=zh|ja|en|all で変更可）"
                zh="三语同时显示：未检测到系统语言为三者之一（可设 NR_MODEL_LANG=zh|ja|en|all 覆盖）"
            else
                en="Language: English (set NR_MODEL_LANG=zh|ja|en|all to override)"
                ja="言語: 日本語（NR_MODEL_LANG=zh|ja|en|all で変更可）"
                zh="语言：简体中文（可设 NR_MODEL_LANG=zh|ja|en|all 覆盖）"
            fi ;;
        using_mirror)
            en="Using mirror: $1"; ja="指定ミラーを使用: $1"; zh="使用指定镜像：$1" ;;
        github_ok)
            en="GitHub direct connection available"
            ja="GitHub への直接接続が利用可能"
            zh="GitHub 直连可用" ;;
        gh_unreachable)
            en="GitHub unreachable, switching to gh-proxy mirror: $1"
            ja="GitHub に接続できないため gh-proxy ミラーへ自動切替: $1"
            zh="GitHub 主站不可达，自动切换 gh-proxy 镜像：$1" ;;
        downloading)
            en="Downloading $1 ← $2"; ja="$1 をダウンロード中 ← $2"; zh="下载 $1 ← $2" ;;
        dl_failed)
            en="Download of $1 failed (curl exit code $2)"
            ja="$1 のダウンロードに失敗（curl 終了コード $2）"
            zh="下载 $1 失败（curl 退出码 $2）" ;;
        slow_auto)
            en="Download speed below 100KB/s; NR_MODEL_AUTO_SWITCH=1 -> switching to mirror automatically"
            ja="ダウンロード速度が 100KB/s 未満。NR_MODEL_AUTO_SWITCH=1 -> ミラーへ自動切替"
            zh="下载速度低于 100KB/s，NR_MODEL_AUTO_SWITCH=1 → 自动切换镜像" ;;
        slow_nonint)
            en="Download speed below 100KB/s. Non-interactive shell, skipping prompt (set NR_MODEL_AUTO_SWITCH=1 to switch automatically)"
            ja="ダウンロード速度が 100KB/s 未満。非対話環境のためスキップ（NR_MODEL_AUTO_SWITCH=1 で自動切替）"
            zh="下载速度低于 100KB/s。非交互环境跳过询问（可设 NR_MODEL_AUTO_SWITCH=1 自动切换镜像）" ;;
        slow_retry)
            en="⚠ $1 download too slow (below 100KB/s), retrying on current source"
            ja="⚠ $1 のダウンロードが遅すぎます（100KB/s 未満）。現在のソースで再試行"
            zh="⚠ $1 下载速度过慢（低于 100KB/s），按当前源重试" ;;
        switched)
            en="Switched to mirror, restarting $1 from scratch"
            ja="ミラーへ切替、$1 を最初から再ダウンロードします"
            zh="已切换镜像，从头重新下载 $1" ;;
        direct_exhausted)
            en="Direct connection failed repeatedly, switching to gh-proxy mirror for $1"
            ja="直接接続が繰り返し失敗したため gh-proxy ミラーで $1 を再試行"
            zh="直连多次失败，切换 gh-proxy 镜像重试 $1" ;;
        retry_n)
            en="Retrying $1 (attempt $2)..."
            ja="$1 を再試行（$2 回目）..."
            zh="重试 $1（第 $2 次）..." ;;
        exists)
            en="Already exists: $1 ($2)"
            ja="既に存在: $1（$2）"
            zh="已存在 $1：$2" ;;
        done_dl)
            en="Done $1: $2"
            ja="$1 完了: $2"
            zh="完成 $1：$2" ;;
        dl_failed_final)
            en="✗ $1 download failed. Check your network and rerun (set NR_MODEL_MIRROR=<mirror prefix> to force mirror)"
            ja="✗ $1 のダウンロードに失敗。ネットワークを確認して再実行してください（NR_MODEL_MIRROR=<ミラーURL> でミラー強制）"
            zh="✗ $1 下载失败，请检查网络后重跑本脚本（可设 NR_MODEL_MIRROR=镜像前缀 强制走镜像）" ;;
        verify_skip)
            en="⚠ Skipping SHA-256 check for $1 (NR_MODEL_SKIP_CHECKSUM=1)"
            ja="⚠ $1 の SHA-256 検証をスキップ（NR_MODEL_SKIP_CHECKSUM=1）"
            zh="⚠ 跳过 $1 的 SHA-256 校验（NR_MODEL_SKIP_CHECKSUM=1）" ;;
        verify_ok)
            en="✓ $1 SHA-256 check passed"
            ja="✓ $1 の SHA-256 検証に合格"
            zh="✓ $1 SHA-256 校验通过" ;;
        verify_fail)
            en="✗ $1 SHA-256 check FAILED"
            ja="✗ $1 の SHA-256 検証に失敗"
            zh="✗ $1 SHA-256 校验失败" ;;
        verify_expect)
            en="  expected: $1"; ja="  期待値: $1"; zh="  期望: $1" ;;
        verify_actual)
            en="  actual:   $1"; ja="  実際値: $1"; zh="  实际: $1" ;;
        verify_reason)
            en="The file may be corrupted, tampered with by the download source, or replaced upstream."
            ja="ファイルが破損・改ざんされたか、上流でモデルが更新された可能性があります。"
            zh="文件可能已损坏、被下载源篡改，或上游已更换模型。" ;;
        verify_deleted)
            en="  File deleted; rerun this script to download again."
            ja="  ファイルを削除しました。本スクリプトを再実行して再ダウンロードできます。"
            zh="  已删除该文件；可重跑本脚本重新下载。" ;;
        verify_skip_hint)
            en="  If the upstream model is confirmed updated, set NR_MODEL_SKIP_CHECKSUM=1 to skip the check."
            ja="  上流モデルの更新を確認した場合は NR_MODEL_SKIP_CHECKSUM=1 で検証をスキップできます。"
            zh="  若确认上游模型已更新，可用 NR_MODEL_SKIP_CHECKSUM=1 跳过校验。" ;;
        models_dir)
            en="Models directory: $1"; ja="モデルディレクトリ: $1"; zh="模型目录：$1" ;;
        final_hint)
            en="Run 'resampler info' to confirm the engine can load the models."
            ja="「resampler info」でモデルを読み込めるか確認できます。"
            zh="用 'resampler info' 确认引擎能否加载模型。" ;;
        *) ;;
    esac
    case "$UI_LANG" in
        en) printf '%s\n' "$en" ;;
        ja) printf '%s\n' "$ja" ;;
        zh) printf '%s\n' "$zh" ;;
        *)  printf '%s\n%s\n%s\n' "$en" "$ja" "$zh" ;;
    esac
}

is_interactive() { [[ -t 0 ]]; }

msg lang_mode

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
        msg verify_skip "$name" >&2
        return 0
    fi
    local actual
    actual="$(sha256_of "$file")"
    if [[ "$actual" != "$expected" ]]; then
        msg verify_fail "$name" >&2
        msg verify_expect "$expected" >&2
        msg verify_actual "$actual" >&2
        msg verify_reason >&2
        rm -f "$file"
        msg verify_deleted >&2
        msg verify_skip_hint >&2
        return 1
    fi
    msg verify_ok "$name" >&2
}

switch_to_mirror() {
    BASE="$DEFAULT_MIRROR"
    USING_MIRROR=1
    MIRROR_ALREADY_USED=1
}

ask_switch_mirror() {
    if [[ "$AUTO_SWITCH" == "1" ]]; then
        msg slow_auto >&2
        return 0
    fi
    if ! is_interactive; then
        msg slow_nonint >&2
        return 1
    fi
    local prompt
    case "$UI_LANG" in
        en) prompt="Download speed below 100KB/s. Switch to gh-proxy mirror and redownload? [Y/n] " ;;
        ja) prompt="ダウンロード速度が 100KB/s 未満です。gh-proxy ミラーへ切り替えて再ダウンロードしますか？[Y/n] " ;;
        zh) prompt="下载速度低于 100KB/s，是否切换 gh-proxy 镜像重新下载？[Y/n] " ;;
        *)  prompt="Speed < 100KB/s. Switch to gh-proxy mirror? [Y/n] / 速度 < 100KB/s。ミラーへ切替？[Y/n] / 速度 < 100KB/s，切换镜像？[Y/n] " ;;
    esac
    local ans=""
    read -r -p "$prompt" ans </dev/tty 2>/dev/null || ans=""
    [[ ! "$ans" =~ ^[Nn] ]]
}

download_one() {
    local url="$1" out="$2" name="$3"
    local attempt=0 rc=0
    while true; do
        msg downloading "$name" "${BASE}${url}"
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
                    msg switched "$name" >&2
                    continue
                fi
                msg slow_retry "$name" >&2
                ;;
            *)
                msg dl_failed "$name" "$rc" >&2
                ;;
        esac
        attempt=$((attempt + 1))
        if ((attempt >= 3)); then
            # 直连重试耗尽且尚未用过镜像 → 自动切换镜像做最后一轮
            if [[ "$USING_MIRROR" == "0" && "$MIRROR_ALREADY_USED" == "0" ]]; then
                msg direct_exhausted "$name" >&2
                switch_to_mirror
                rm -f "$out"
                attempt=0
                continue
            fi
            rm -f "$out"
            return 1
        fi
        rm -f "$out"
        msg retry_n "$name" "$attempt" >&2
        sleep 3
    done
}

fetch() {
    local url="$1" out="$2" name="$3" expected="$4"
    if [[ -f "$out" ]]; then
        msg exists "$name" "$out"
        verify "$out" "$expected" "$name"
        return 0
    fi
    if download_one "$url" "$out" "$name"; then
        msg done_dl "$name" "$(du -h "$out" | cut -f1)"
        verify "$out" "$expected" "$name"
        return 0
    fi
    msg dl_failed_final "$name" >&2
    return 1
}

# ---- 选择下载源 ----
if [[ -n "$MIRROR_EXPLICIT" ]]; then
    switch_to_mirror
    BASE="$MIRROR_EXPLICIT"
    msg using_mirror "$BASE"
elif probe_github; then
    msg github_ok
else
    switch_to_mirror
    msg gh_unreachable "$BASE" >&2
fi

fetch "$FCPE_URL" "$DEST/fcpe.onnx" "FCPE" "$FCPE_SHA256"
fetch "$VOCODER_URL" "$DEST/pc_nsf_hifigan.onnx" "PC-NSF-HiFiGAN" "$VOCODER_SHA256"

echo
msg models_dir "$DEST"
ls -la "$DEST"
echo
msg final_hint
