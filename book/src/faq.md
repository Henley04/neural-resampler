# 常见问题

### 需要改我的声库吗？

不需要。WAV 和 `oto.ini` 原样保留，只把编辑器里的 resampler 指向本程序。

### 为什么发布包里没有模型？

两个原因：模型合计约 100 MB；而且它们的许可不一定允许再分发。
所以模型由使用者自行获取，发布包里附带了 `download_models.sh`
（Windows 为 `download_models.ps1`）。

### 没有模型能跑吗？

分场景。`resampler selftest` 与 `resampler info` 不受影响，可以验证管线连通性；
但 `render` / `batch` / UTAU 协议渲染在模型缺失时会**直接报错退出**（三语提示，
含解决方法），不会输出音质不可用的音频。离线自测确需降级运行时，加
`--allow-stub` 或设 `NR_ALLOW_STUB=1`。

### 在 OpenUtau 里渲染报错「声码器模型缺失」？

OpenUtau 安装 resampler 时把 exe **复制**进自己的 `Resamplers/` 目录，模型不会
跟着走。v0.1.3 起，这种情况不再是静默降级（以前能出声但音质糊），而是渲染失败
并弹出三语错误说明；resampler 旁边还会生成 `MODEL-MISSING-READ-ME.txt`。
解决（二选一）：

1. **推荐**：在发布包目录运行一次 `download_models.sh` / `download_models.ps1`——
   脚本会自动把 `NR_MODELS_DIR` 写入用户级环境变量（此后 exe 放哪都行；
   注意使用期间不要移动 models 目录）
2. 或把 `models/` 复制为 `Resamplers/models/`（与 exe 同级）

然后用 `resampler info` 确认声码器后端显示 `onnxruntime` 而不是 `stub`。

### 为什么第一次渲染很慢？

首次要做 Mel 分析、F0 提取，还要加载 ONNX 模型。同一输入的第二次渲染会走缓存，
快很多。整段旋律渲染时，模型只加载一次。

### 支持 GPU 吗？

支持，但需要用对应 feature 自行编译：

```bash
cargo build --release --features cuda      # NVIDIA
cargo build --release --features directml  # Windows
cargo build --release --features coreml    # macOS
```

**不要同时启用多个**，也不要用 `--all-features`——feature 互斥，会导致链接失败。
默认产物走 CPU。

### 输出长度怎么控制？

第 7 个参数 `length`，**单位是毫秒**。想要 1 秒的输出传 `1000`。
传 `44100` 会得到 44.1 秒。

### 能同时支持 UTAU 和 OpenUtau 吗？

能，两者用的是同一套 13 参数协议，同一个二进制即可。
配置方式见 [UTAU](utau.md) 和 [OpenUtau](openutau.md)。

### flags 全都生效吗？

不是。当前实际生效的只有 `g`、`t`、`A`、`P`、`He`、`HG` 六个，
其余标记会被解析但暂不影响输出。详见 [UTAU 参数](utau.md#支持的-flags)。

### 输出是确定性的吗？

是。同样的输入 + 同样的参数 + 同样的配置，输出逐样本一致，没有随机性。

### 声音发闷/失真怎么办？

99% 是 Mel 参数与模型规格不匹配。恢复默认 `mel.*` 配置，
确认 `mel_scale: slaney`、`sample_rate: 44100`、`n_mels: 128`。
如果换了非默认模型，必须同步改这些参数。

### 缓存文件可以删吗？

可以，删了只是下次渲染慢一点。缓存键包含源文件大小、修改时间和配置指纹，
正常情况下不需要手动清理。

### 商业用途可以吗？

代码是 MIT。但**模型的许可由各自来源决定**，商用前请自行确认。
这也是模型不随仓库分发的原因之一。
