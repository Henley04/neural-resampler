# Installation

## Option 1: Download prebuilt binaries (recommended)

Download the archive for your platform from the
[Releases page](https://github.com/Henley04/neural-resampler/releases).

| Platform | File |
| --- | --- |
| Linux x86_64 | `neural-resampler-<ver>-linux-x86_64.tar.gz` |
| macOS Apple Silicon | `neural-resampler-<ver>-macos-aarch64.tar.gz` |
| Windows x86_64 | `neural-resampler-<ver>-windows-x86_64.zip` |

For platforms without prebuilt binaries yet (macOS Intel, Linux ARM64, etc.),
build with `cargo build --release`; the binary lands in
`target/release/resampler`.

Each archive ships with a matching `.sha256` checksum file, which you can
verify after downloading:

```bash
shasum -a 256 -c neural-resampler-0.1.0-linux-x86_64.tar.gz.sha256
# Windows (PowerShell):
# (Get-FileHash -Algorithm SHA256 .\xxx.zip).Hash
```

Directory layout after extraction:

```
neural-resampler-0.1.0-linux-x86_64/
├── resampler              # 主程序（Windows 为 resampler.exe）
├── config.resampler.yaml  # 默认配置，可直接改后用 --config 指定
├── download_models.sh     # 模型下载脚本（Linux/macOS）
├── download_models.ps1    # 模型下载脚本（Windows PowerShell）
├── models/                # 模型放置目录（初始为空）
├── docs/                  # 架构与模型说明
└── README.md LICENSE
```

> **The binaries do not include the ONNX models.** The models are ~100 MB and
> subject to their respective source licenses, so they are not distributed
> with the repo. After extraction, run `./download_models.sh`
> (`.\download_models.ps1` on Windows) to fetch them; see
> [Obtaining the models](models.md).

## Option 2: Build from source

A stable Rust toolchain (1.70+) is required. The first build automatically
downloads the prebuilt ONNX Runtime libraries.

```bash
git clone https://github.com/Henley04/neural-resampler.git
cd neural-resampler
cargo build --release
# 产物：target/release/resampler
```

Optional features (GPU acceleration):

```bash
cargo build --release --features cuda      # NVIDIA CUDA
cargo build --release --features directml  # Windows DirectML
cargo build --release --features coreml    # macOS CoreML
```

> ⚠️ GPU features are **mutually exclusive**. Do not enable more than one at
> a time, and do not use `--all-features` — ort cannot find matching prebuilt
> libraries for the combined feature set, which causes link failures.

## Option 3: Package your own distribution

```bash
bash scripts/build_release.sh                 # 输出到 dist/
bash scripts/build_release.sh --features cuda
```

The script packages the binary, the default configuration, and the docs, and
includes the models as well when they exist locally.

## Verifying the installation

```bash
./resampler info        # 查看构建信息、配置指纹与模型状态
./resampler selftest    # 生成测试音并端到端跑一遍
```

In the `info` output, the vocoder and FCPE must both show `[已就绪]` (ready)
before the setup is truly usable. When `[缺失]` (missing) is shown, the
engine falls back; see [Troubleshooting](troubleshooting.md).
