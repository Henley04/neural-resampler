# neural-resampler

[![CI - Build & Test](https://github.com/Henley04/neural-resampler/actions/workflows/ci.yml/badge.svg)](https://github.com/Henley04/neural-resampler/actions/workflows/ci.yml)

通用神经重采样器引擎 —— 用 **纯 Rust + ONNX** 重写的 HiFiSampler。

> 📖 **使用文档**：<https://henley04.github.io/neural-resampler/>
> 📦 **下载预编译产物**：[Releases](https://github.com/Henley04/neural-resampler/releases)
> （Linux x86_64 / macOS Apple Silicon / Windows x86_64）

传统声库的 WAV 与 `oto.ini` **完全不动**，只替换负责音高变换、时长调整和音色合成的 resampler 引擎：

* 声码器沿用 **PC-NSF-HiFiGAN**（NSF 架构，F0 条件化，直接改 F0 即可精确变调）；
* F0 提取从 WORLD（DIO/Harvest）换成 **FCPE（ONNX, 16kHz）**，歌声场景更稳；
* 无 Python 依赖，**单二进制部署**；
* 通过适配层同时支持 **OpenUtau / 原生 UTAU**，并保留扩展到其他编辑器的接口。

```
┌──────────────────────────────────────────────────────────┐
│            宿主编辑器：OpenUtau / UTAU / 其他              │
└──────────────────────┬───────────────────────────────────┘
                       │ 命令行协议
┌──────────────────────▼───────────────────────────────────┐
│ 适配层：UTAU 参数解析、pitchBend(Base64+RLE) 解码、oto.ini │
└──────────────────────┬───────────────────────────────────┘
┌──────────────────────▼───────────────────────────────────┐
│ 核心引擎：Mel 分析 → 时序拉伸 → F0 生成 → 声码器 → 后处理  │
└──────────────────────┬───────────────────────────────────┘
┌──────────────────────▼───────────────────────────────────┐
│ 推理后端：ort（CPU / DirectML / CUDA / CoreML，可插拔）    │
└──────────────────────────────────────────────────────────┘
```

## 快速开始

```bash
# 1. 构建（首次构建会自动下载 ONNX Runtime 预编译库）
cargo build --release

# 2. 准备模型（见 models/README.md）
bash scripts/download_models.sh          # Windows: scripts/download_models.ps1
                                         # 或手动把 ONNX 放进 models/

# 3. 自检：生成测试音并跑通整条管线
./target/release/resampler selftest

# 4. 查看构建信息 / 配置 / 模型状态
./target/release/resampler info
```

## 作为 UTAU / OpenUtau 的 resampler 使用

二进制本身即 resampler，直接按 UTAU 协议传参：

```bash
resampler in.wav out.wav C4 100 "" 0 500 60 -50 100 0 !120 AA
#        ↑输入  ↑输出   ↑音名 ↑力度 ↑flags ↑offset ↑length ↑consonant ↑cutoff ↑volume ↑modulation ↑tempo ↑pitchBend
```

在 OpenUtau 中：把二进制放进 OpenUtau 目录的 `Resamplers` 文件夹（或直接拖到
OpenUtau 窗口选 "Install as resampler"），渲染器切到 `CLASSIC` 后点击旁边的
⚙ 齿轮图标选择本 resampler。详见 OpenUtau wiki 的
[Resamplers and Wavtools](https://github.com/openutau/OpenUtau/wiki/Resamplers-and-Wavtools)。

没有模型时引擎会自动降级（声码器 → 内置 Stub，F0 → 内置 DSP），管线仍然可跑通，
但**音质不可用**，仅用于连通性验证。

## 子命令

| 子命令 | 说明 |
| --- | --- |
| `render` | 按具名参数渲染单个音符（`--help` 查看全部选项） |
| `batch` | 批量渲染，每行一组 UTAU 参数，`--jobs N` 并行 |
| `info` | 打印版本、构建信息、配置指纹与模型状态 |
| `selftest` | 生成测试音并端到端跑一遍，用于冒烟验证 |
| `config` | 导出默认配置为 YAML，便于二次修改 |

全局选项：`--config <FILE>`（配置文件）、`--models <DIR>`（模型目录）、
`--log-level <LEVEL>`、`--log-format text|json`。

示例：

```bash
resampler render --config my.yaml voice/_a.wav out.wav A4 100 "g-3" 30 500 60 -50 100 0 '!120' AA
resampler batch list.txt --jobs 4
resampler config resampler.yaml
```

## F0 生成模式

配置 `f0.mode` 决定 F0 如何生成（默认 `hybrid`）：

| 模式 | 行为 | 适用 |
| --- | --- | --- |
| `score` | F0 完全来自乐谱（音名 + pitchBend），与原始 HiFiSampler 一致 | 需要精确贴合乐谱 |
| `source` | 用样本自身的 F0 曲线整体移调到目标音高 | 希望保留原样本的颤音/滑音 |
| `hybrid` | 乐谱给出绝对音高，样本 F0 提供自然偏移（限幅 `max_deviation_cents`）与清浊判定 | 默认，兼顾准确与自然 |

`f0.backend` 为 `auto` 时优先级：**FCPE(ONNX) → 内置 DSP（DIO/Harvest 风格）→ 纯乐谱**。

## 分析约定（已与参考实现逐值校验）

Mel 提取必须和训练前处理逐位一致，否则声码器输出会发闷或失真。本项目对齐的约定：

| 环节 | 取值 |
| --- | --- |
| Mel 标度 | **Slaney**（`librosa.filters.mel` 默认，非 HTK） |
| 归一化 | Slaney 面积归一化 `2 / (f_right - f_left)` |
| 窗函数 | 周期 Hann（`torch.hann_window`） |
| 填充 | 反射填充，`(win-hop)/2` 左、`(win-hop+1)/2` 右 |
| STFT | `center=False`，帧数 `1 + (len - win) // hop` |
| 幅度 | 声码器侧 `abs()`；FCPE 侧 `sqrt(re²+im²+1e-9)` |
| 压缩 | **自然对数** `log(clamp(x, min=clip))`，声码器 `1e-9`、FCPE `1e-5` |

`tests/mel_reference.rs` 用 Python 参考实现生成的基准值锁死了这些约定（有声 bin 误差 < 0.06）。

FCPE 的输出是 **cent 分类 latent** `[1, T, 360]`，需按官方流程解码：
局部 argmax（±4 类）加权平均 → `f0 = 10 · 2^(cent/1200)`，置信度 ≤ 0.006 的帧判为清音。

## 与原始 HiFiSampler 的差异

| 维度 | 原始 HiFiSampler | 本项目 |
| --- | --- | --- |
| F0 提取 | WORLD (DIO/Harvest) | FCPE ONNX（可回退到内置 DSP） |
| 语言 | Python + C# | 纯 Rust |
| 部署 | 需 Python 运行时 | 单二进制 |
| 声码器 | PC-NSF-HiFiGAN | PC-NSF-HiFiGAN（不变） |
| 音高控制 | 修改 F0 | 修改 F0（不变） |
| GPU | PyTorch CUDA | ort + DirectML / CUDA / CoreML（可选 feature） |
| 缓存 | `.hifi.npz` | zstd 压缩的 `.nrc`（参考 Organum `.ogc`） |

## 项目结构

```
src/
├── main.rs                 CLI 入口（resampler）
├── lib.rs                  库入口 + C ABI（FFI）
├── adapters/utau.rs        UTAU / OpenUtau 适配层
├── core/
│   ├── audio.rs            WAV I/O、重采样、增益
│   ├── oto.rs              oto.ini 解析（UTF-8 / Shift-JIS）
│   ├── protocol.rs         UTAU 参数、pitchBend 解码、音名换算
│   ├── feature.rs          STFT / Mel 滤波器组 / 插值
│   ├── f0/{mod,fcpe,world}.rs  F0Extractor trait、FCPE ONNX、DSP 回退
│   ├── pipeline.rs         渲染管线
│   └── post_process.rs     响度归一化、限幅、growl
├── backend/{ort_backend,stub}.rs   推理后端
└── cache/f0_cache.rs       zstd 特征缓存
config/resampler.yaml       默认配置（编译期嵌入）
models/                     ONNX 模型（不随仓库分发）
tests/                      单元测试与端到端测试
```

## 配置

默认配置编译期嵌入（`config/resampler.yaml`），可用 `--config` 覆盖，或 `resampler config out.yaml` 导出后修改。
关键项：

* `mel.*`：44.1kHz / 2048 FFT / 分析帧移 128 / 渲染帧移 512 / 128 mel bin / 40–16000Hz
* `f0.*`：16kHz / hop 160 / 1024 FFT / 128 mel bin / 0–8000Hz，F0 80–880Hz、cent 表 32.70–1975.5Hz
* `vocoder.*`：模型路径、输入输出节点名、`mel_layout`（默认按模型元数据自动判断）、执行提供者列表
* `processing.*`：`loop_mode`（长音符循环拼接）、`fill`（帧余量）、`gender`
* `output.*`：输出采样率、位深、峰值上限、响度归一化
* `cache.*`：缓存开关、后缀、zstd 等级

## 分发产物

```bash
bash scripts/build_release.sh                 # 默认 feature，输出 dist/<target>.tar.gz
bash scripts/build_release.sh --features cuda # 指定 feature
```

产物解压即用，内含二进制、默认配置、文档与 `models/*.onnx`（若本地已有模型）：

```
neural-resampler-x86_64-unknown-linux-gnu/
├── resampler                 # 单二进制
├── models/*.onnx             # 声码器 + FCPE
├── config.resampler.yaml     # 默认配置，可直接改
└── docs/ README.md LICENSE
```

模型未放入仓库，CI 与空白环境请用 `scripts/download_models.sh`
（Windows: `scripts/download_models.ps1`）获取。脚本在 GitHub 直连不可达
或低于 100KB/s 时自动走 gh-proxy 镜像（可用 `NR_MODEL_MIRROR` 强制指定），下载后做 SHA-256 校验。

## 测试与 CI

```bash
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings      # 不要用 --all-features：GPU 提供者互斥
cargo test
```

GitHub Actions（`.github/workflows/ci.yml`）在 Ubuntu / Windows / macOS 上执行格式检查、Clippy、
构建、单元测试、集成测试与端到端 selftest，并额外做 ONNX 模型校验（模型存在时）。

## 已验证的行为

在 `models/` 放好两个 ONNX 后实测（输入为 220Hz 锯齿波）：

| 场景 | 结果 |
| --- | --- |
| `A3`（220Hz）、1s | 输出 1.000s，实测 F0 220.0 Hz |
| `B3`（246.94Hz） | 实测 F0 247.5 Hz（偏差 < 4 cent） |
| 要求长度 500ms / 2000ms | 输出 0.500s / 2.000s，F0 保持 |
| pitchBend −600→+600 cent | F0 轨迹 165Hz → 294Hz 平滑跟随 |
| 无模型 | 自动降级到 Stub + 内置 DSP，管线仍跑通 |

## FFI

`lib.rs` 暴露了一组 C ABI，便于其他语言集成：

```c
const char* ver = nr_version();                 // 静态字符串，无需释放
char* out = nr_render_json("{\"input\":\"a.wav\", ...}");
if (!out) fprintf(stderr, "%s\n", nr_last_error());
nr_string_free(out);
```

`cargo build --release` 会同时产出动态库（`[lib] crate-type` 含 `cdylib`）：

| 平台 | 产物 |
| --- | --- |
| Linux | `target/release/libneural_resampler.so` |
| macOS | `target/release/libneural_resampler.dylib` |
| Windows | `target/release/neural_resampler.dll` |

需要静态库时，在 `[lib]` 的 `crate-type` 中追加 `"staticlib"` 即可。
所有 FFI 入口都用 `catch_unwind` 包裹，panic 不会跨越 ABI 边界。

## 许可与致谢

MIT。参考与致谢：[hifisampler](https://github.com/openhachimi/hifisampler)、
[straycat-rs](https://github.com/UtaUtaUtau/straycat-rs)、[Organum](https://github.com/KakouLabs/Organum)、
[pitch-core](https://github.com/gzivdo/pitch-core)、[ort](https://github.com/pykeio/ort)、
[FCPE](https://github.com/CNChTu/FCPE)、[DiffSinger](https://github.com/openvpi/DiffSinger)。

模型本身的许可请遵循各自来源仓库/发布页的要求。
