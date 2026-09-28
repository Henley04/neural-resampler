# 架构说明

## 数据流

一次渲染的完整路径（对应 `src/core/pipeline.rs`）：

```
UTAU 参数 / CLI 参数
        │
        ▼
  [1] 读取 WAV  ─────────────► AudioBuffer（单声道 f32）
        │
        ▼
  [2] 时间轴映射
        offset / consonant / cutoff / length
        → 源采样区间 [start, end)
        │
        ▼
  [3] 时序拉伸（rubato SincFixedIn 或线性插值）
        把源区间重采样到目标时长（BPM + 音符长度）
        │
        ▼
  [4] Mel 分析（origin_hop_size=128）
        → mel: [frames, 128]
        │
        ▼
  [5] 帧率转换 128 → 512（线性插值）
        │
        ▼
  [6] F0 生成
        FCPE(ONNX) / 内置 DSP / 纯乐谱
        + pitchBend + 性别偏移 + 颤音
        │
        ▼
  [7] 声码器推理（ort）
        (mel, f0) → waveform
        │
        ▼
  [8] 后处理：循环拼接 → 淡入淡出 → 响度归一化 → 峰值限幅
        │
        ▼
  写出 WAV
```

## 模块职责

| 模块 | 职责 | 可替换性 |
| --- | --- | --- |
| `adapters/utau.rs` | UTAU/OpenUtau 参数适配、批量调度 | 新增编辑器只需实现新的 adapter |
| `core/protocol.rs` | 音名 ↔ MIDI ↔ Hz、flags 解析、`pitchBend` 解码 | 纯函数，无状态 |
| `core/oto.rs` | `oto.ini` 解析（UTF-8 / Shift-JIS / GBK 兜底） | 只读 |
| `core/audio.rs` | WAV 读写、Sinc 重采样、增益、静音裁剪 | 依赖 hound |
| `core/feature.rs` | STFT、Mel 滤波器组、帧间插值 | 纯 Rust + realfft |
| `core/f0/*` | `F0Extractor` trait 的三个实现 | 通过 `f0.backend` 切换 |
| `core/pipeline.rs` | 编排上述步骤 | 单一入口 `render()` |
| `core/post_process.rs` | 响度归一化、限幅、淡入淡出 | 可关闭 |
| `backend/*` | 推理后端（ort / stub） | 通过 `vocoder.backend` 切换 |
| `cache/f0_cache.rs` | Mel/F0 特征的 zstd 持久化 | 通过 `cache.enabled` 关闭 |

## 关键设计决策

### 1. F0 通过修改条件输入实现变调

PC-NSF-HiFiGAN 把 F0 作为显式条件，因此**改变 F0 即可精确变调**，不需要传统 resampler 的重采样+变调（会同时改时长）。
时长控制独立由 Mel 的时序拉伸完成，二者解耦 —— 这是 HiFiSampler 的核心思路，本项目完整保留。

### 2. 双帧移（128 分析 / 512 渲染）

分析阶段用 128 的细帧移做时序拉伸，插值精度更高；声码器只接受 512 帧移的 Mel，
所以在送入声码器前做一次线性插值降帧（`origin_hop_size → hop_size`）。

### 3. F0 三级回退

```
FCPE ONNX  ──失败──►  内置 DSP（DIO 风格谐波打分）  ──失败──►  纯乐谱 F0
```

任何一级失败都只降级、不中断渲染，保证在没有模型或模型损坏的机器上仍能出声（音质下降）。

### 4. 缓存键

缓存键包含：源文件路径、文件大小、mtime（秒+纳秒）、Mel 参数哈希、F0 参数哈希、采样率。
任一项变化即失效重建，避免样本替换后读到旧特征。

### 5. 线程模型

`Session` 不是 `Sync`，批量渲染时每个 worker 线程各自持有一个后端实例（`--jobs N`）。
单线程渲染复用同一实例，避免重复加载模型。

## 扩展点

* **新编辑器**：在 `src/adapters/` 下新增模块，产出 `RenderRequest` 即可。
* **新 F0 后端**：实现 `F0Extractor` trait，在 `core/f0/mod.rs` 的工厂函数里注册字符串 key。
* **新声码器**：实现 `backend::Vocoder` trait；若仍是 ONNX，只需改配置里的输入输出节点名。
* **新执行提供者**：在 `backend/ort_backend.rs` 的 `build_providers()` 里加分支，并用 cargo feature 控制编译。
