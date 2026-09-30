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
        Write-Host "⚠ 下载速度低于 100KB/s，NR_MODEL_AUTO_SWITCH=1 → 自动切换镜像"
        return $true
    }
    $interactive = $true
    try { $interactive = -not [Console]::IsInputRedirected } catch { $interactive = $false }
    if (-not $interactive) {
        Write-Host "⚠ 下载速度低于 100KB/s。非交互环境跳过询问（可设 NR_MODEL_AUTO_SWITCH=1 自动切换镜像）"
        return $false
    }
    try {
        $ans = Read-Host "下载速度低于 100KB/s，是否切换 gh-proxy 镜像重新下载？[Y/n]"
        return ($ans -notmatch '^[Nn]')
    } catch {
        Write-Host "⚠ 下载速度低于 100KB/s。非交互环境跳过询问（可设 NR_MODEL_AUTO_SWITCH=1 自动切换镜像）"
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
        Write-Host "下载 $Name ← $($Script:Base)$Url"
        $rc = Invoke-Download -Url "$($Script:Base)$Url" -OutFile $OutFile
        if ($rc -eq 'ok') { return $true }
        if ($rc -eq 'slow') {
            if (($Script:UsingMirror -eq 0) -and ($Script:MirrorAlreadyUsed -eq 0) -and (Ask-SwitchMirror)) {
                Switch-ToMirror
                Remove-Item -LiteralPath $OutFile -Force -ErrorAction SilentlyContinue
                Write-Host "已切换镜像，从头重新下载 $Name"
                continue
            }
            Write-Host "⚠ $Name 下载速度过慢（低于 100KB/s），按当前源重试"
        } else {
            Write-Host "下载 $Name 失败（$rc）"
        }
        $attempt++
        if ($attempt -ge 3) {
            # 直连重试耗尽且尚未用过镜像 -> 自动切换镜像做最后一轮
            if (($Script:UsingMirror -eq 0) -and ($Script:MirrorAlreadyUsed -eq 0)) {
                Write-Host "直连多次失败，切换 gh-proxy 镜像重试 $Name"
                Switch-ToMirror
                Remove-Item -LiteralPath $OutFile -Force -ErrorAction SilentlyContinue
                $attempt = 0
                continue
            }
            Remove-Item -LiteralPath $OutFile -Force -ErrorAction SilentlyContinue
            return $false
        }
        Remove-Item -LiteralPath $OutFile -Force -ErrorAction SilentlyContinue
        Write-Host "重试 $Name（第 $attempt 次）..."
        Start-Sleep -Seconds 3
    }
}

function Test-Checksum {
    param([string]$File, [string]$Expected, [string]$Name)
    if ($Script:SkipChecksum -eq '1') {
        Write-Host "⚠ 跳过 $Name 的 SHA-256 校验（NR_MODEL_SKIP_CHECKSUM=1）"
        return $true
    }
    $actual = (Get-FileHash -LiteralPath $File -Algorithm SHA256).Hash.ToLower()
    if ($actual -ne $Expected) {
        Write-Host "✗ $Name SHA-256 校验失败"
        Write-Host "  期望: $Expected"
        Write-Host "  实际: $actual"
        Write-Host "  文件可能已损坏、被下载源篡改，或上游已更换模型。"
        Remove-Item -LiteralPath $File -Force
        Write-Host "  已删除该文件；可重跑本脚本重新下载。"
        Write-Host "  若确认上游模型已更新，可用 NR_MODEL_SKIP_CHECKSUM=1 跳过校验。"
        return $false
    }
    Write-Host "✓ $Name SHA-256 校验通过"
    return $true
}

function Fetch-Model {
    param([string]$Url, [string]$OutFile, [string]$Name, [string]$Expected)
    if (Test-Path -LiteralPath $OutFile) {
        Write-Host "已存在 $Name：$OutFile"
        return (Test-Checksum -File $OutFile -Expected $Expected -Name $Name)
    }
    if (Download-One -Url $Url -OutFile $OutFile -Name $Name) {
        $sizeMb = '{0:N1} MB' -f ((Get-Item -LiteralPath $OutFile).Length / 1MB)
        Write-Host "完成 $Name：$sizeMb"
        return (Test-Checksum -File $OutFile -Expected $Expected -Name $Name)
    }
    Write-Host "✗ $Name 下载失败，请检查网络后重跑本脚本（可设 NR_MODEL_MIRROR=镜像前缀 强制走镜像）"
    return $false
}

# ---- 选择下载源 ----
if ($Script:MirrorExplicit) {
    Switch-ToMirror
    $Script:Base = $Script:MirrorExplicit
    Write-Host "使用指定镜像：$($Script:Base)"
} elseif (Test-GithubReachable) {
    Write-Host "GitHub 直连可用"
} else {
    Switch-ToMirror
    Write-Host "⚠ GitHub 主站不可达，自动切换 gh-proxy 镜像：$($Script:Base)"
}

if (-not (Fetch-Model -Url $Script:FcpeUrl -OutFile (Join-Path $DestDir 'fcpe.onnx') -Name 'FCPE' -Expected $Script:FcpeSha256)) { exit 1 }
if (-not (Fetch-Model -Url $Script:VocoderUrl -OutFile (Join-Path $DestDir 'pc_nsf_hifigan.onnx') -Name 'PC-NSF-HiFiGAN' -Expected $Script:VocoderSha256)) { exit 1 }

Write-Host ''
Write-Host "模型目录：$((Resolve-Path -LiteralPath $DestDir).Path)"
Get-ChildItem -LiteralPath $DestDir | ForEach-Object { Write-Host ("  {0,12}  {1}" -f $_.Length, $_.Name) }
Write-Host ''
Write-Host "用 'resampler info' 确认引擎能否加载模型。"
