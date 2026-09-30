# 下载 neural-resampler 所需的 ONNX 模型（PowerShell 版，逻辑与 download_models.sh 一致）。
#
#   powershell -ExecutionPolicy Bypass -File scripts\download_models.ps1  # 仓库内：下载到 仓库根\models
#   .\download_models.ps1                                                  # 发布包内：下载到 包内 models\
#   .\download_models.ps1 C:\path\dir                                      # 指定目录
#
# 目标目录自动按脚本位置判断：脚本与 resampler（.exe）同级 -> 发布包布局，
# 否则按仓库布局（scripts\ 的上一级）。
#
# 下载源策略（与 bash 版 download_models.sh 逻辑一致）：
#   1. 设置 NR_MODEL_MIRROR 时直接使用该镜像前缀
#      （gh-proxy 格式：镜像前缀 + 完整原始 URL，如 https://ghfast.top/https://raw...）
#   2. 否则先探测 GitHub 直连；不可达时自动切换 gh-proxy 镜像并提示
#   3. 下载中平均速度低于 100KB/s 持续 10 秒：
#      当前为直连时询问是否切换镜像（NR_MODEL_AUTO_SWITCH=1 免询问自动切换，
#      非交互环境提示后按原源继续）；当前已是镜像则提示后继续重试
#   4. 切换下载源后从头下载（gh-proxy 不保证 Range 续传）
#   5. 无论来源如何，下载完成后都做 SHA-256 校验——镜像内容被篡改会被拦下
#
# 提示语言：NR_MODEL_LANG=zh|ja|en|all 可强制指定；否则按系统 UI 语言
# 选择简体中文 / 日本語 / English；检测不到或系统语言不属于三者时，三语同时显示。
#
# 模型来源于第三方仓库，许可遵循各自发布页要求。

param([string]$DestDir)

$ErrorActionPreference = 'Stop'
try { $ProgressPreference = 'SilentlyContinue' } catch {}
try { [Console]::OutputEncoding = New-Object System.Text.UTF8Encoding($false) } catch {}
try { [Net.ServicePointManager]::SecurityProtocol = [Net.ServicePointManager]::SecurityProtocol -bor [Net.SecurityProtocolType]::Tls12 } catch {}
Add-Type -AssemblyName System.Net.Http

$Script:ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path

# ---- 目标目录：兼容仓库布局与发布包布局 ----
if (-not $DestDir) {
    if ((Test-Path -LiteralPath (Join-Path $ScriptDir 'resampler')) -or
        (Test-Path -LiteralPath (Join-Path $ScriptDir 'resampler.exe'))) {
        $DestDir = Join-Path $ScriptDir 'models'    # 发布包：脚本与 resampler 同级
    } else {
        $DestDir = Join-Path (Join-Path $ScriptDir '..') 'models'    # 仓库：脚本在 scripts\ 下
    }
}
New-Item -ItemType Directory -Force -Path $DestDir | Out-Null

$Script:MirrorExplicit = $env:NR_MODEL_MIRROR
$Script:SkipChecksum = $env:NR_MODEL_SKIP_CHECKSUM
$Script:AutoSwitch = $env:NR_MODEL_AUTO_SWITCH
$Script:DefaultMirror = 'https://ghfast.top/'    # gh-proxy 生态实例，可用 NR_MODEL_MIRROR 覆盖
$Script:MinSpeed = 102400                        # 100 KB/s
$Script:SlowSeconds = 10                         # 平均速度持续低于阈值的判定时长

# 社区已导出的 ONNX（PC-NSF-HiFiGAN 44.1k/hop512/128bin 与 FCPE）
$Script:FcpeUrl = 'https://raw.githubusercontent.com/open-ai-tuning/HachiTune/master/models/fcpe.onnx'
$Script:VocoderUrl = 'https://raw.githubusercontent.com/open-ai-tuning/HachiTune/master/models/pc_nsf_hifigan.onnx'

# SHA-256 基线：与 v0.1.0 一同验证过的版本。
# 上游更换模型（哈希不匹配）时脚本会拒绝安装并说明处理办法；
# 确认新版本可用后可更新此表。
$Script:FcpeSha256 = '013b507acd406de122b61ee497300ac6be082511101bc2136b886e48d9737e5d'
$Script:VocoderSha256 = 'd1d0edcbd45a9fbbbb5387d8a2913f4d5a0fc21755692c5d511951fdb910c00b'

$Script:Base = ''                # 下载前缀：'' = GitHub 直连；非空 = 镜像前缀
$Script:UsingMirror = 0
$Script:MirrorAlreadyUsed = 0

# ---- 提示语言：NR_MODEL_LANG > 系统 UI 语言 > 三语同显 ----
function Detect-Lang {
    switch ($env:NR_MODEL_LANG) {
        { $_ -in @('zh', 'ja', 'en', 'all') } { return $_ }
    }
    $l = ''
    try { $l = [System.Globalization.CultureInfo]::CurrentUICulture.TwoLetterISOLanguageName } catch {}
    switch ($l) {
        'zh' { return 'zh' }
        'ja' { return 'ja' }
        'en' { return 'en' }
    }
    return 'all'    # 检测不到或非三语 → 三语同时显示
}
$Script:UILang = Detect-Lang

# Get-Msg <key> [args]：按 UILang 返回提示文本。
# all 模式下每条消息按 English -> 日本語 -> 简体中文 各占一行。
function Get-Msg {
    param([string]$Key, [object[]]$A = @())
    $en = ''; $ja = ''; $zh = ''
    switch ($Key) {
        'lang_mode' {
            if ($Script:UILang -eq 'all') {
                $en = "Trilingual mode: system language is not one of English / 日本語 / 简体中文 (set NR_MODEL_LANG=zh|ja|en|all to override)"
                $ja = "三言語表示モード: システム言語が English / 日本語 / 簡体字中国語 のいずれにも一致しません（NR_MODEL_LANG=zh|ja|en|all で変更可）"
                $zh = "三语同时显示：未检测到系统语言为三者之一（可设 NR_MODEL_LANG=zh|ja|en|all 覆盖）"
            } else {
                $en = "Language: English (set NR_MODEL_LANG=zh|ja|en|all to override)"
                $ja = "言語: 日本語（NR_MODEL_LANG=zh|ja|en|all で変更可）"
                $zh = "语言：简体中文（可设 NR_MODEL_LANG=zh|ja|en|all 覆盖）"
            } }
        'using_mirror' {
            $en = "Using mirror: $($A[0])"; $ja = "指定ミラーを使用: $($A[0])"; $zh = "使用指定镜像：$($A[0])" }
        'github_ok' {
            $en = "GitHub direct connection available"
            $ja = "GitHub への直接接続が利用可能"
            $zh = "GitHub 直连可用" }
        'gh_unreachable' {
            $en = "GitHub unreachable, switching to gh-proxy mirror: $($A[0])"
            $ja = "GitHub に接続できないため gh-proxy ミラーへ自動切替: $($A[0])"
            $zh = "GitHub 主站不可达，自动切换 gh-proxy 镜像：$($A[0])" }
        'downloading' {
            $en = "Downloading $($A[0]) ← $($A[1])"; $ja = "$($A[0]) をダウンロード中 ← $($A[1])"; $zh = "下载 $($A[0]) ← $($A[1])" }
        'dl_failed' {
            $en = "Download of $($A[0]) failed ($($A[1]))"
            $ja = "$($A[0]) のダウンロードに失敗（$($A[1])）"
            $zh = "下载 $($A[0]) 失败（$($A[1])）" }
        'slow_auto' {
            $en = "Download speed below 100KB/s; NR_MODEL_AUTO_SWITCH=1 -> switching to mirror automatically"
            $ja = "ダウンロード速度が 100KB/s 未満。NR_MODEL_AUTO_SWITCH=1 -> ミラーへ自動切替"
            $zh = "下载速度低于 100KB/s，NR_MODEL_AUTO_SWITCH=1 → 自动切换镜像" }
        'slow_nonint' {
            $en = "Download speed below 100KB/s. Non-interactive shell, skipping prompt (set NR_MODEL_AUTO_SWITCH=1 to switch automatically)"
            $ja = "ダウンロード速度が 100KB/s 未満。非対話環境のためスキップ（NR_MODEL_AUTO_SWITCH=1 で自動切替）"
            $zh = "下载速度低于 100KB/s。非交互环境跳过询问（可设 NR_MODEL_AUTO_SWITCH=1 自动切换镜像）" }
        'slow_retry' {
            $en = "⚠ $($A[0]) download too slow (below 100KB/s), retrying on current source"
            $ja = "⚠ $($A[0]) のダウンロードが遅すぎます（100KB/s 未満）。現在のソースで再試行"
            $zh = "⚠ $($A[0]) 下载速度过慢（低于 100KB/s），按当前源重试" }
        'switched' {
            $en = "Switched to mirror, restarting $($A[0]) from scratch"
            $ja = "ミラーへ切替、$($A[0]) を最初から再ダウンロードします"
            $zh = "已切换镜像，从头重新下载 $($A[0])" }
        'direct_exhausted' {
            $en = "Direct connection failed repeatedly, switching to gh-proxy mirror for $($A[0])"
            $ja = "直接接続が繰り返し失敗したため gh-proxy ミラーで $($A[0]) を再試行"
            $zh = "直连多次失败，切换 gh-proxy 镜像重试 $($A[0])" }
        'retry_n' {
            $en = "Retrying $($A[0]) (attempt $($A[1]))..."
            $ja = "$($A[0]) を再試行（$($A[1]) 回目）..."
            $zh = "重试 $($A[0])（第 $($A[1]) 次）..." }
        'exists' {
            $en = "Already exists: $($A[0]) ($($A[1]))"
            $ja = "既に存在: $($A[0])（$($A[1])）"
            $zh = "已存在 $($A[0])：$($A[1])" }
        'done_dl' {
            $en = "Done $($A[0]): $($A[1])"
            $ja = "$($A[0]) 完了: $($A[1])"
            $zh = "完成 $($A[0])：$($A[1])" }
        'dl_failed_final' {
            $en = "✗ $($A[0]) download failed. Check your network and rerun (set NR_MODEL_MIRROR=<mirror prefix> to force mirror)"
            $ja = "✗ $($A[0]) のダウンロードに失敗。ネットワークを確認して再実行してください（NR_MODEL_MIRROR=<ミラーURL> でミラー強制）"
            $zh = "✗ $($A[0]) 下载失败，请检查网络后重跑本脚本（可设 NR_MODEL_MIRROR=镜像前缀 强制走镜像）" }
        'verify_skip' {
            $en = "⚠ Skipping SHA-256 check for $($A[0]) (NR_MODEL_SKIP_CHECKSUM=1)"
            $ja = "⚠ $($A[0]) の SHA-256 検証をスキップ（NR_MODEL_SKIP_CHECKSUM=1）"
            $zh = "⚠ 跳过 $($A[0]) 的 SHA-256 校验（NR_MODEL_SKIP_CHECKSUM=1）" }
        'verify_ok' {
            $en = "✓ $($A[0]) SHA-256 check passed"
            $ja = "✓ $($A[0]) の SHA-256 検証に合格"
            $zh = "✓ $($A[0]) SHA-256 校验通过" }
        'verify_fail' {
            $en = "✗ $($A[0]) SHA-256 check FAILED"
            $ja = "✗ $($A[0]) の SHA-256 検証に失敗"
            $zh = "✗ $($A[0]) SHA-256 校验失败" }
        'verify_expect' {
            $en = "  expected: $($A[0])"; $ja = "  期待値: $($A[0])"; $zh = "  期望: $($A[0])" }
        'verify_actual' {
            $en = "  actual:   $($A[0])"; $ja = "  実際値: $($A[0])"; $zh = "  实际: $($A[0])" }
        'verify_reason' {
            $en = "The file may be corrupted, tampered with by the download source, or replaced upstream."
            $ja = "ファイルが破損・改ざんされたか、上流でモデルが更新された可能性があります。"
            $zh = "文件可能已损坏、被下载源篡改，或上游已更换模型。" }
        'verify_deleted' {
            $en = "  File deleted; rerun this script to download again."
            $ja = "  ファイルを削除しました。本スクリプトを再実行して再ダウンロードできます。"
            $zh = "  已删除该文件；可重跑本脚本重新下载。" }
        'verify_skip_hint' {
            $en = "  If the upstream model is confirmed updated, set NR_MODEL_SKIP_CHECKSUM=1 to skip the check."
            $ja = "  上流モデルの更新を確認した場合は NR_MODEL_SKIP_CHECKSUM=1 で検証をスキップできます。"
            $zh = "  若确认上游模型已更新，可用 NR_MODEL_SKIP_CHECKSUM=1 跳过校验。" }
        'models_dir' {
            $en = "Models directory: $($A[0])"; $ja = "モデルディレクトリ: $($A[0])"; $zh = "模型目录：$($A[0])" }
        'final_hint' {
            $en = "Run 'resampler info' to confirm the engine can load the models."
            $ja = "「resampler info」でモデルを読み込めるか確認できます。"
            $zh = "用 'resampler info' 确认引擎能否加载模型。" }
    }
    switch ($Script:UILang) {
        'en' { return $en }
        'ja' { return $ja }
        'zh' { return $zh }
        default { return "$en`n$ja`n$zh" }
    }
}

function Write-Msg {
    param([string]$Key, [object[]]$A = @())
    Write-Host (Get-Msg $Key $A)
}

Write-Msg 'lang_mode'

function Test-GithubReachable {
    # HEAD 探测直连可达性（总超时 8 秒）
    $client = $null
    try {
        $client = New-Object System.Net.Http.HttpClient
        $client.Timeout = [TimeSpan]::FromSeconds(8)
        $request = New-Object System.Net.Http.HttpRequestMessage([System.Net.Http.HttpMethod]::Head, $Script:FcpeUrl)
        $resp = $client.SendAsync($request).GetAwaiter().GetResult()
        $ok = $resp.IsSuccessStatusCode
        $resp.Dispose()
        return $ok
    } catch {
        return $false
    } finally {
        if ($client) { $client.Dispose() }
    }
}

function Switch-ToMirror {
    $Script:Base = $Script:DefaultMirror
    $Script:UsingMirror = 1
    $Script:MirrorAlreadyUsed = 1
}

function Ask-SwitchMirror {
    if ($Script:AutoSwitch -eq '1') {
        Write-Msg 'slow_auto'
        return $true
    }
    $interactive = $true
    try { $interactive = -not [Console]::IsInputRedirected } catch { $interactive = $false }
    if (-not $interactive) {
        Write-Msg 'slow_nonint'
        return $false
    }
    try {
        $prompt = switch ($Script:UILang) {
            'en' { "Download speed below 100KB/s. Switch to gh-proxy mirror and redownload? [Y/n]" }
            'ja' { "ダウンロード速度が 100KB/s 未満です。gh-proxy ミラーへ切り替えて再ダウンロードしますか？[Y/n]" }
            'zh' { "下载速度低于 100KB/s，是否切换 gh-proxy 镜像重新下载？[Y/n]" }
            default { "Speed < 100KB/s. Switch to gh-proxy mirror? [Y/n] / 速度 < 100KB/s。ミラーへ切替？[Y/n] / 速度 < 100KB/s，切换镜像？[Y/n]" }
        }
        $ans = Read-Host $prompt
        return ($ans -notmatch '^[Nn]')
    } catch {
        Write-Msg 'slow_nonint'
        return $false
    }
}

function Invoke-Download {
    # 等价 curl -fL --speed-limit 102400 --speed-time 10 --connect-timeout 15：
    #   - 头部阶段 15 秒超时（连接 + 服务器响应）
    #   - 单次读流最多等 15 秒（服务器建连后不发数据的场景）
    #   - 最近 10 秒窗口平均速度低于 100KB/s -> 判定慢速
    # 返回 'ok' | 'slow' | 'error: 原因'
    param([string]$Url, [string]$OutFile)
    $client = $null; $resp = $null; $stream = $null; $fs = $null
    try {
        $client = New-Object System.Net.Http.HttpClient
        $client.Timeout = [TimeSpan]::FromSeconds(15)
        $request = New-Object System.Net.Http.HttpRequestMessage([System.Net.Http.HttpMethod]::Get, $Url)
        $resp = $client.SendAsync($request, [System.Net.Http.HttpCompletionOption]::ResponseHeadersRead).GetAwaiter().GetResult()
        if (-not $resp.IsSuccessStatusCode) {
            return "error: HTTP $([int]$resp.StatusCode)"
        }
        $stream = $resp.Content.ReadAsStreamAsync().GetAwaiter().GetResult()
        $fs = [System.IO.File]::Create($OutFile)
        $sw = [System.Diagnostics.Stopwatch]::StartNew()
        $buf = New-Object byte[] 65536
        $total = [long]0
        # 窗口样本：@(时刻ms, 累计字节)，保留最近 10 秒
        $samples = New-Object 'System.Collections.Generic.List[object[]]'
        while ($true) {
            $now = $sw.ElapsedMilliseconds
            if ($samples.Count -gt 0) {
                while ($samples.Count -ge 2 -and (($now - $samples[1][0]) -ge ($Script:SlowSeconds * 1000))) {
                    $samples.RemoveAt(0)
                }
                $dt = ($now - $samples[0][0]) / 1000.0
                if ($dt -ge $Script:SlowSeconds) {
                    $speed = ($total - $samples[0][1]) / $dt
                    if ($speed -lt $Script:MinSpeed) { return 'slow' }
                }
            }
            $readTask = $stream.ReadAsync($buf, 0, $buf.Length)
            if (-not $readTask.Wait(15000)) { return 'slow' }    # 15 秒无任何数据
            $n = $readTask.Result
            if ($n -le 0) { break }
            $fs.Write($buf, 0, $n)
            $total += $n
            $samples.Add([object[]]@($sw.ElapsedMilliseconds, $total))
        }
        return 'ok'
    } catch {
        return "error: $($_.Exception.Message)"
    } finally {
        if ($fs) { $fs.Dispose() }
        if ($stream) { $stream.Dispose() }
        if ($resp) { $resp.Dispose() }
        if ($client) { $client.Dispose() }
    }
}

function Download-One {
    param([string]$Url, [string]$OutFile, [string]$Name)
    $attempt = 0
    while ($true) {
        Write-Msg 'downloading' @($Name, "$($Script:Base)$Url")
        $rc = Invoke-Download -Url "$($Script:Base)$Url" -OutFile $OutFile
        if ($rc -eq 'ok') { return $true }
        if ($rc -eq 'slow') {
            if (($Script:UsingMirror -eq 0) -and ($Script:MirrorAlreadyUsed -eq 0) -and (Ask-SwitchMirror)) {
                Switch-ToMirror
                Remove-Item -LiteralPath $OutFile -Force -ErrorAction SilentlyContinue
                Write-Msg 'switched' @($Name)
                continue
            }
            Write-Msg 'slow_retry' @($Name)
        } else {
            Write-Msg 'dl_failed' @($Name, $rc)
        }
        $attempt++
        if ($attempt -ge 3) {
            # 直连重试耗尽且尚未用过镜像 -> 自动切换镜像做最后一轮
            if (($Script:UsingMirror -eq 0) -and ($Script:MirrorAlreadyUsed -eq 0)) {
                Write-Msg 'direct_exhausted' @($Name)
                Switch-ToMirror
                Remove-Item -LiteralPath $OutFile -Force -ErrorAction SilentlyContinue
                $attempt = 0
                continue
            }
            Remove-Item -LiteralPath $OutFile -Force -ErrorAction SilentlyContinue
            return $false
        }
        Remove-Item -LiteralPath $OutFile -Force -ErrorAction SilentlyContinue
        Write-Msg 'retry_n' @($Name, $attempt)
        Start-Sleep -Seconds 3
    }
}

function Test-Checksum {
    param([string]$File, [string]$Expected, [string]$Name)
    if ($Script:SkipChecksum -eq '1') {
        Write-Msg 'verify_skip' @($Name)
        return $true
    }
    $actual = (Get-FileHash -LiteralPath $File -Algorithm SHA256).Hash.ToLower()
    if ($actual -ne $Expected) {
        Write-Msg 'verify_fail' @($Name)
        Write-Msg 'verify_expect' @($Expected)
        Write-Msg 'verify_actual' @($actual)
        Write-Msg 'verify_reason'
        Remove-Item -LiteralPath $File -Force
        Write-Msg 'verify_deleted'
        Write-Msg 'verify_skip_hint'
        return $false
    }
    Write-Msg 'verify_ok' @($Name)
    return $true
}

function Fetch-Model {
    param([string]$Url, [string]$OutFile, [string]$Name, [string]$Expected)
    if (Test-Path -LiteralPath $OutFile) {
        Write-Msg 'exists' @($Name, $OutFile)
        return (Test-Checksum -File $OutFile -Expected $Expected -Name $Name)
    }
    if (Download-One -Url $Url -OutFile $OutFile -Name $Name) {
        $sizeMb = '{0:N1} MB' -f ((Get-Item -LiteralPath $OutFile).Length / 1MB)
        Write-Msg 'done_dl' @($Name, $sizeMb)
        return (Test-Checksum -File $OutFile -Expected $Expected -Name $Name)
    }
    Write-Msg 'dl_failed_final' @($Name)
    return $false
}

# ---- 选择下载源 ----
if ($Script:MirrorExplicit) {
    Switch-ToMirror
    $Script:Base = $Script:MirrorExplicit
    Write-Msg 'using_mirror' @($Script:Base)
} elseif (Test-GithubReachable) {
    Write-Msg 'github_ok'
} else {
    Switch-ToMirror
    Write-Msg 'gh_unreachable' @($Script:Base)
}

if (-not (Fetch-Model -Url $Script:FcpeUrl -OutFile (Join-Path $DestDir 'fcpe.onnx') -Name 'FCPE' -Expected $Script:FcpeSha256)) { exit 1 }
if (-not (Fetch-Model -Url $Script:VocoderUrl -OutFile (Join-Path $DestDir 'pc_nsf_hifigan.onnx') -Name 'PC-NSF-HiFiGAN' -Expected $Script:VocoderSha256)) { exit 1 }

Write-Host ''
Write-Msg 'models_dir' @((Resolve-Path -LiteralPath $DestDir).Path)
Get-ChildItem -LiteralPath $DestDir | ForEach-Object { Write-Host ("  {0,12}  {1}" -f $_.Length, $_.Name) }
Write-Host ''
Write-Msg 'final_hint'
