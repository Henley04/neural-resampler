[简体中文](README.md) | [日本語](README.ja.md)

# neural-resampler

[![CI - Build & Test](https://github.com/Henley04/neural-resampler/actions/workflows/ci.yml/badge.svg)](https://github.com/Henley04/neural-resampler/actions/workflows/ci.yml)

A universal neural resampler engine — HiFiSampler rewritten in **pure Rust + ONNX**.

> 📖 **Documentation**: <https://henley04.github.io/neural-resampler/>
> 📦 **Download prebuilt artifacts**: [Releases](https://github.com/Henley04/neural-resampler/releases)
> (Linux x86_64 / macOS Apple Silicon / Windows x86_64)

The WAV files and `oto.ini` of traditional voicebanks are **left completely untouched**; only the resampler engine responsible for pitch shifting, duration adjustment, and timbre synthesis is replaced:

* The vocoder remains **PC-NSF-HiFiGAN** (NSF architecture, F0-conditioned; changing F0 directly yields precise pitch shifting);
* F0 extraction switches from WORLD (DIO/Harvest) to **FCPE (ONNX, 16kHz)**, more stable in singing scenarios;
* No Python dependency, **single-binary deployment**;
* Supports **OpenUtau / native UTAU** simultaneously through the adapter layer, with interfaces reserved for extending to other editors.

```
┌──────────────────────────────────────────────────────────┐
│           Host editor: OpenUtau / UTAU / others          │
└──────────────────────┬───────────────────────────────────┘
                       │ Command-line protocol
┌──────────────────────▼───────────────────────────────────┐
│   Adapter: UTAU params, pitchBend(Base64+RLE), oto.ini   │
└──────────────────────┬───────────────────────────────────┘
┌──────────────────────▼───────────────────────────────────┐
│Core engine: Mel → time-stretch → F0 → vocoder → post-proc│
└──────────────────────┬───────────────────────────────────┘
┌──────────────────────▼───────────────────────────────────┐
│Inference: ort (CPU / DirectML / CUDA / CoreML, pluggable)│
└──────────────────────────────────────────────────────────┘
```

## Quick Start

```bash
# 1. 构建（首次构建会自动下载 ONNX Runtime 预编译库）
cargo build --release

# 2. 准备模型（见 models/README.md）
bash scripts/download_models.sh          # Windows: scripts/download_models.ps1
                                         # 或手动把 ONNX 放进 models/
# After download the script automatically writes NR_MODELS_DIR into your
# user environment (NR_SKIP_ENV=1 to skip); do not move the models dir while using it

# 3. 自检：生成测试音并跑通整条管线
./target/release/resampler selftest

# 4. 查看构建信息 / 配置 / 模型状态
./target/release/resampler info
```

## Using as a resampler for UTAU / OpenUtau

The binary itself is the resampler; pass arguments directly per the UTAU protocol:

```bash
resampler in.wav out.wav C4 100 "" 0 500 60 -50 100 0 !120 AA
#        ↑输入  ↑输出   ↑音名 ↑力度 ↑flags ↑offset ↑length ↑consonant ↑cutoff ↑volume ↑modulation ↑tempo ↑pitchBend
```

In OpenUtau: place the binary in the `Resamplers` folder of the OpenUtau directory (or drag it directly onto the
OpenUtau window and choose "Install as resampler"), switch the renderer to `CLASSIC`, then click the ⚙ gear
icon next to it and select this resampler. See the OpenUtau wiki's
[Resamplers and Wavtools](https://github.com/openutau/OpenUtau/wiki/Resamplers-and-Wavtools) for details.

Without models, the engine falls back automatically (vocoder → built-in Stub, F0 → built-in DSP); the pipeline
still runs, but **quality is unusable** — for connectivity verification only.

## Subcommands

| Subcommand | Description |
| --- | --- |
| `render` | Render a single note with named arguments (`--help` shows all options) |
| `batch` | Batch rendering, one set of UTAU parameters per line, `--jobs N` for parallelism |
| `info` | Print version, build info, config fingerprint, and model status |
| `selftest` | Generate a test tone and run the pipeline end to end once, for smoke verification |
| `config` | Export the default config as YAML for further editing |

Global options: `--config <FILE>` (config file), `--models <DIR>` (model directory),
`--log-level <LEVEL>`, `--log-format text|json`.

Examples:

```bash
resampler render --config my.yaml voice/_a.wav out.wav A4 100 "g-3" 30 500 60 -50 100 0 '!120' AA
resampler batch list.txt --jobs 4
resampler config resampler.yaml
```

## F0 Generation Modes

The `f0.mode` setting decides how F0 is generated (default `hybrid`):

| Mode | Behavior | Use case |
| --- | --- | --- |
| `score` | F0 comes entirely from the score (note name + pitchBend), identical to the original HiFiSampler | When precise score adherence is needed |
| `source` | Transpose the sample's own F0 curve as a whole to the target pitch | When the original sample's vibrato/glides should be kept |
| `hybrid` | The score gives absolute pitch; the sample F0 provides natural deviation (clamped by `max_deviation_cents`) and voiced/unvoiced decisions | Default, balancing accuracy and naturalness |

When `f0.backend` is `auto`, the priority is: **FCPE(ONNX) → built-in DSP (DIO/Harvest style) → score only**.

## Analysis Conventions (verified value-by-value against the reference implementation)

Mel extraction must match the training preprocessing bit-for-bit, otherwise the vocoder output will sound muffled or distorted. The conventions aligned in this project:

| Stage | Value |
| --- | --- |
| Mel scale | **Slaney** (default of `librosa.filters.mel`, not HTK) |
| Normalization | Slaney area normalization `2 / (f_right - f_left)` |
| Window | Periodic Hann (`torch.hann_window`) |
| Padding | Reflect padding, `(win-hop)/2` left and `(win-hop+1)/2` right |
| STFT | `center=False`, frame count `1 + (len - win) // hop` |
| Magnitude | `abs()` on the vocoder side; `sqrt(re²+im²+1e-9)` on the FCPE side |
| Compression | **Natural log** `log(clamp(x, min=clip))`, `1e-9` for the vocoder and `1e-5` for FCPE |

`tests/mel_reference.rs` locks these conventions with reference values generated by the Python reference implementation (voiced-bin error < 0.06).

The FCPE output is a **cent classification latent** `[1, T, 360]`, which must be decoded per the official procedure:
weighted average of the local argmax (±4 bins) → `f0 = 10 · 2^(cent/1200)`; frames with confidence ≤ 0.006 are judged unvoiced.

## Differences from the Original HiFiSampler

| Aspect | Original HiFiSampler | This project |
| --- | --- | --- |
| F0 extraction | WORLD (DIO/Harvest) | FCPE ONNX (falls back to the built-in DSP) |
| Language | Python + C# | Pure Rust |
| Deployment | Python runtime required | Single binary |
| Vocoder | PC-NSF-HiFiGAN | PC-NSF-HiFiGAN (unchanged) |
| Pitch control | Modifying F0 | Modifying F0 (unchanged) |
| GPU | PyTorch CUDA | ort + DirectML / CUDA / CoreML (optional features) |
| Cache | `.hifi.npz` | zstd-compressed `.nrc` (modeled after Organum `.ogc`) |

## Project Structure

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

## Configuration

The default config is embedded at compile time (`config/resampler.yaml`); override it with `--config`, or export it with
`resampler config out.yaml` and edit. Key entries:

* `mel.*`: 44.1kHz / 2048 FFT / analysis hop 128 / render hop 512 / 128 mel bins / 40–16000Hz
* `f0.*`: 16kHz / hop 160 / 1024 FFT / 128 mel bins / 0–8000Hz, F0 80–880Hz, cent table 32.70–1975.5Hz
* `vocoder.*`: model paths, input/output node names, `mel_layout` (auto-detected from model metadata by default), execution provider list
* `processing.*`: `loop_mode` (loop splicing for long notes), `fill` (frame margin), `gender`
* `output.*`: output sample rate, bit depth, peak ceiling, loudness normalization
* `cache.*`: cache switch, suffix, zstd level

## Distribution Artifacts

```bash
bash scripts/build_release.sh                 # 默认 feature，输出 dist/<target>.tar.gz
bash scripts/build_release.sh --features cuda # 指定 feature
```

The artifact is ready to use right after extraction, containing the binary, default config, docs, and `models/*.onnx` (if models exist locally):

```
neural-resampler-x86_64-unknown-linux-gnu/
├── resampler                 # 单二进制
├── models/*.onnx             # 声码器 + FCPE
├── config.resampler.yaml     # 默认配置，可直接改
└── docs/ README.md LICENSE
```

Models are not committed to the repository; in CI and blank environments, fetch them with `scripts/download_models.sh`
(Windows: `scripts/download_models.ps1`). The script automatically falls back to the gh-proxy mirror when GitHub direct connection
is unreachable or slower than 100KB/s (force one via `NR_MODEL_MIRROR`), verifies SHA-256 after download, and automatically writes
`NR_MODELS_DIR` into your user environment — the resampler then finds the models even when copied into the editor's own folder
(e.g. OpenUtau's `Resamplers/`) (⚠ do not move the models directory while using this method; `NR_SKIP_ENV=1` skips the setup).
When models are missing, rendering fails with an explicit trilingual error (including the fix); pass `--allow-stub` or set
`NR_ALLOW_STUB=1` to allow the degraded backend (offline testing only).

## Testing & CI

```bash
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings      # 不要用 --all-features：GPU 提供者互斥
cargo test
```

GitHub Actions (`.github/workflows/ci.yml`) runs format checks, Clippy, build, unit tests, integration tests, and the
end-to-end selftest on Ubuntu / Windows / macOS, plus ONNX model validation when models are present.

## Verified Behavior

Measured with both ONNX files in place under `models/` (input is a 220Hz sawtooth wave):

| Scenario | Result |
| --- | --- |
| `A3` (220Hz), 1s | Output 1.000s, measured F0 220.0 Hz |
| `B3` (246.94Hz) | Measured F0 247.5 Hz (deviation < 4 cents) |
| Requested length 500ms / 2000ms | Output 0.500s / 2.000s, F0 preserved |
| pitchBend −600→+600 cents | F0 trajectory follows smoothly from 165Hz → 294Hz |
| No models | Falls back automatically to Stub + built-in DSP, pipeline still runs |

## FFI

`lib.rs` exposes a set of C ABIs for easy integration from other languages:

```c
const char* ver = nr_version();                 // 静态字符串，无需释放
char* out = nr_render_json("{\"input\":\"a.wav\", ...}");
if (!out) fprintf(stderr, "%s\n", nr_last_error());
nr_string_free(out);
```

`cargo build --release` also produces a dynamic library (`[lib] crate-type` includes `cdylib`):

| Platform | Artifact |
| --- | --- |
| Linux | `target/release/libneural_resampler.so` |
| macOS | `target/release/libneural_resampler.dylib` |
| Windows | `target/release/neural_resampler.dll` |

For a static library, add `"staticlib"` to the `crate-type` of `[lib]`.
All FFI entry points are wrapped in `catch_unwind`; panics never cross the ABI boundary.

## License & Acknowledgments

MIT. References and acknowledgments: [hifisampler](https://github.com/openhachimi/hifisampler),
[straycat-rs](https://github.com/UtaUtaUtau/straycat-rs), [Organum](https://github.com/KakouLabs/Organum),
[pitch-core](https://github.com/gzivdo/pitch-core), [ort](https://github.com/pykeio/ort),
[FCPE](https://github.com/CNChTu/FCPE), [DiffSinger](https://github.com/openvpi/DiffSinger).

For the models themselves, follow the license requirements of their respective source repositories/release pages.
