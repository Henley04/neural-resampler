# 配置文件

默认配置编译期嵌入在二进制里。想改的话先导出一份：

```bash
./resampler config my.yaml
./resampler --config my.yaml render voice/_a.wav out.wav A4
```

> ⚠️ `mel.*` 与 `f0.*` 的数值是**跟着模型走的**。换成不同规格的 ONNX 模型时，
> 必须同步修改这些参数，否则输出会发闷、失真甚至完全无声。用默认模型就不要动它们。

## models_dir

模型根目录。配置里各 `model` 字段都是相对它的路径，**不要再写一遍 `models/`**。

```yaml
models_dir: "models"     # ✅ 则 vocoder.model 写 "pc_nsf_hifigan.onnx"
```

## mel —— 频谱分析

| 字段 | 默认 | 说明 |
| --- | --- | --- |
| `sample_rate` | 44100 | 分析采样率 |
| `n_fft` | 2048 | FFT 点数 |
| `win_size` | 2048 | 窗长 |
| `hop_size` | 512 | 渲染帧移 |
| `origin_hop_size` | 128 | 分析帧移（更细，用于时域插值） |
| `n_mels` | 128 | Mel bin 数 |
| `fmin` / `fmax` | 40 / 16000 | 频率范围（Hz） |
| `clip_val` | `1e-9` | 对数压缩下限 |
| `magnitude_eps` | 0.0 | 幅度地板，`0` = 直接 `abs()` |
| `mel_scale` | `slaney` | 标度：`slaney`（librosa 默认）/ `htk` |

## f0 —— 音高提取

| 字段 | 默认 | 说明 |
| --- | --- | --- |
| `backend` | `auto` | `auto` / `fcpe` / `world` / `none` |
| `mode` | `hybrid` | `score` / `source` / `hybrid`，见 [F0 模式](f0-modes.md) |
| `model` | `fcpe.onnx` | FCPE 模型文件名 |
| `sample_rate` | 16000 | FCPE 输入采样率 |
| `hop_size` / `win_size` / `n_fft` | 160 / 1024 / 1024 | FCPE 频谱参数 |
| `mel_bins` | 128 | FCPE 的 Mel bin 数 |
| `fmin` / `fmax` | 0 / 8000 | FCPE 频率范围 |
| `f0_min` / `f0_max` | 80 / 880 | 有效 F0 范围（Hz），超出视为无效 |
| `uv_threshold` | 0.006 | 置信度低于此值判为清音 |
| `uv_mask` | true | 把清音帧的 F0 置零 |
| `max_deviation_cents` | 100.0 | hybrid 模式允许的最大自然偏移 |
| `smoothing_ms` | 40.0 | 基准曲线平滑窗口 |
| `decoder` | `local_argmax` | latent→cent 解码：`local_argmax` / `argmax` |
| `local_argmax_width` | 4 | 单侧窗口宽度（总窗口 2×width+1） |
| `cent_f0_min` / `cent_f0_max` | 32.70 / 1975.5 | cent 表频率范围（C1~B6） |
| `clip_val` | `1e-5` | F0 分析的对数压缩下限 |

## vocoder —— 声码器

| 字段 | 默认 | 说明 |
| --- | --- | --- |
| `backend` | `ort` | 推理后端 |
| `model` | `pc_nsf_hifigan.onnx` | 声码器模型 |
| `hnsep_model` | `hnsep.onnx` | 可选，谐波/噪声分离 |
| `mel_input` / `f0_input` | `mel` / `f0` | 输入节点名 |
| `output` | `audio` | 输出节点名 |
| `mel_layout` | `auto` | `auto` / `channels_first` / `frames_first` |
| `providers` | `[cpu]` | 按顺序尝试：`cpu` / `directml` / `cuda` / `coreml` |

`mel_layout: auto` 会读模型元数据自动判断输入是 `[1,n_mels,T]` 还是 `[1,T,n_mels]`。
若你的模型被误判、输出异常，可显式指定。

## processing —— 处理

| 字段 | 默认 | 说明 |
| --- | --- | --- |
| `loop_mode` | true | 长音符时对元音段做反射循环拼接 |
| `fill` | 6 | 首尾额外保留的帧数 |
| `trim_silence` | false | 分析前裁剪静音 |
| `silence_threshold_db` | -52.0 | 静音判定阈值 |
| `gender` | 0 | 默认性别偏移（对应 `g` 标记） |

## output —— 输出

| 字段 | 默认 | 说明 |
| --- | --- | --- |
| `sample_rate` | 44100 | 输出采样率 |
| `bit_depth` | 16 | `16`/`24`/`32` 整数 PCM，`0` 为 32 位浮点 |
| `peak_limit` | 1.0 | 峰值上限 |
| `wave_norm` | true | 启用响度归一化 |
| `loudness_target` | -16.0 | 目标响度 |
| `loudness_block_ms` | 400.0 | 响度统计块大小 |
| `fade_in_ms` / `fade_out_ms` | 0.0 / 0.0 | 淡入淡出时长 |

## cache —— 特征缓存

| 字段 | 默认 | 说明 |
| --- | --- | --- |
| `enabled` | true | 启用缓存 |
| `extension` | `nrc` | 缓存文件后缀 |
| `zstd_level` | 3 | 压缩等级 |
| `dir` | null | 缓存目录，`null` 表示与输入同目录 |

缓存键包含源文件大小、修改时间与配置指纹，**改了配置会自动失效**，不用手动清理。

## runtime —— 运行时

| 字段 | 默认 | 说明 |
| --- | --- | --- |
| `log_level` | `info` | 日志级别 |
| `log_format` | `text` | `text` / `json` |
| `intra_op_threads` | 0 | 算子内线程数，`0` = 自动 |
| `inter_op_threads` | 0 | 算子间线程数，`0` = 自动 |
