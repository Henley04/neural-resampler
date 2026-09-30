# 模型目录

本目录放置推理所需的 ONNX 模型，**不随仓库分发**（体积大且受各自许可约束）。

| 文件 | 作用 | 必需 |
| --- | --- | --- |
| `pc_nsf_hifigan.onnx` | 声码器：Mel + F0 → 波形 | 是 |
| `fcpe.onnx` | F0 提取（16kHz） | 否（缺失时回退到内置 DSP） |
| `hnsep.onnx` | HN-SEP 谐波/噪声分离 | 否 |

获取方式（自动在 GitHub 直连不可达/过慢时切换 gh-proxy 镜像，下载后做 SHA-256 校验）：

```bash
bash scripts/download_models.sh          # Linux / macOS / Git Bash
powershell -ExecutionPolicy Bypass -File scripts/download_models.ps1   # Windows
```

模型不存在时引擎会降级运行（声码器 → Stub、F0 → 内置 DSP），管线仍可跑通，
用于验证连通性；**音质需真实模型**。

`resampler info` 会列出每个模型的就绪状态。
