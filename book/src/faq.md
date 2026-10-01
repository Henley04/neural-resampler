# 常见问题

### 需要改我的声库吗？

不需要。WAV 和 `oto.ini` 原样保留，只把编辑器里的 resampler 指向本程序。

### 为什么发布包里没有模型？

两个原因：模型合计约 100 MB；而且它们的许可不一定允许再分发。
所以模型由使用者自行获取，发布包里附带了 `download_models.sh`
（Windows 为 `download_models.ps1`）。

### 没有模型能跑吗？

能跑，但没有实用价值。缺模型时声码器会退化成 Stub、F0 退化为内置 DSP，
管线仍然走通、也有声音，但音质不可用。这个降级是为了方便验证连通性，
不是为了实际使用。`./resampler info` 会明确告诉你当前用的什么后端。

### 在 OpenUtau 里渲染，音质不对（像降级了）？

OpenUtau 安装 resampler 时把 exe **复制**进自己的 `Resamplers/` 目录，渲染时的
工作目录下没有 `models/`，引擎会静默降级（能出声但不报错）。解决：把模型放到
`Resamplers/models/`，或设置环境变量 `NR_MODELS_DIR` 指向模型目录。然后用
`resampler info` 确认声码器后端显示 `onnxruntime` 而不是 `stub`。

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
