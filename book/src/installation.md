# 安装

## 方式一：下载预编译产物（推荐）

从 [Releases 页面](https://github.com/Henley04/neural-resampler/releases) 下载对应平台的压缩包。

| 平台 | 文件 |
| --- | --- |
| Linux x86_64 | `neural-resampler-<ver>-linux-x86_64.tar.gz` |
| macOS Apple Silicon | `neural-resampler-<ver>-macos-aarch64.tar.gz` |
| Windows x86_64 | `neural-resampler-<ver>-windows-x86_64.zip` |

暂不提供预编译产物的平台（macOS Intel、Linux ARM64 等）请自行
`cargo build --release`，产物在 `target/release/resampler`。

每个压缩包附带同名 `.sha256` 校验文件，下载后可核对：

```bash
shasum -a 256 -c neural-resampler-0.1.0-linux-x86_64.tar.gz.sha256
# Windows (PowerShell):
# (Get-FileHash -Algorithm SHA256 .\xxx.zip).Hash
```

解压后的目录结构：

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

> **产物不含 ONNX 模型。** 模型体积约 100 MB，且受各自来源许可约束，不随仓库分发。
> 解压后请运行 `./download_models.sh`（Windows 为 `.\download_models.ps1`）获取，
> 详见[获取模型](models.md)。

## 方式二：从源码构建

需要 Rust 稳定版工具链（1.70+）。首次构建会自动下载 ONNX Runtime 预编译库。

```bash
git clone https://github.com/Henley04/neural-resampler.git
cd neural-resampler
cargo build --release
# 产物：target/release/resampler
```

可选 feature（GPU 加速）：

```bash
cargo build --release --features cuda      # NVIDIA CUDA
cargo build --release --features directml  # Windows DirectML
cargo build --release --features coreml    # macOS CoreML
```

> ⚠️ GPU feature **互斥**，不要同时启用多个，也不要用 `--all-features`——
> ort 无法为组合后的 feature set 找到匹配的预编译库，会导致链接失败。

## 方式三：打包自己的分发产物

```bash
bash scripts/build_release.sh                 # 输出到 dist/
bash scripts/build_release.sh --features cuda
```

脚本会把二进制、默认配置、文档打包，并在本地存在模型时一并附上。

## 验证安装

```bash
./resampler info        # 查看构建信息、配置指纹与模型状态
./resampler selftest    # 生成测试音并端到端跑一遍
```

`info` 的输出里，声码器和 FCPE 都显示 `[已就绪]` 才算真正可用。
显示 `[缺失]` 时引擎会降级，详见[故障排查](troubleshooting.md)。
