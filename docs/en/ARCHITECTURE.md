# Architecture

## Data Flow

The complete path of a single render (corresponding to `src/core/pipeline.rs`):

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
  [8] 后处理（finalize 单一入口）：
        重采样 → 定长 → 响度归一化(P 插值) → 峰值限幅 → 音量(V) → 淡入淡出
        归一化/限幅/音量只发生一次，避免互相覆盖
        │
        ▼
  写出 WAV
```

## Module Responsibilities

| Module | Responsibility | Replaceability |
| --- | --- | --- |
| `adapters/utau.rs` | UTAU/OpenUtau argument adaptation, batch scheduling | Adding a new editor only requires implementing a new adapter |
| `core/protocol.rs` | Note name ↔ MIDI ↔ Hz, flag parsing, `pitchBend` decoding | Pure functions, stateless |
| `core/oto.rs` | `oto.ini` parsing (UTF-8 / Shift-JIS / GBK fallback) | Read-only |
| `core/audio.rs` | WAV read/write, Sinc resampling, gain, silence trimming | Depends on hound |
| `core/feature.rs` | STFT, Mel filter bank, inter-frame interpolation | Pure Rust + realfft |
| `core/f0/*` | Three implementations of the `F0Extractor` trait | Switched via `f0.backend` |
| `core/pipeline.rs` | Orchestrates the steps above | Single entry point `render()` |
| `core/post_process.rs` | Loudness normalization, peak limiting, fade in/out | Can be disabled |
| `backend/*` | Inference backends (ort / stub) | Switched via `vocoder.backend` |
| `cache/f0_cache.rs` | zstd persistence of Mel/F0 features | Disabled via `cache.enabled` |

## Key Design Decisions

### 1. Pitch is changed by modifying the condition input

PC-NSF-HiFiGAN takes F0 as an explicit condition, so **changing F0 alone shifts
pitch precisely**, without the traditional resampler's resample-then-shift
approach (which also changes duration). Duration control is handled
independently by time-stretching the Mel, decoupling the two — this is the core
idea of HiFiSampler, fully preserved in this project.

### 2. Dual hop sizes (128 analysis / 512 rendering)

The analysis stage uses the fine 128 hop size for time-stretching, giving
higher interpolation accuracy; the vocoder only accepts Mel with a 512 hop
size, so a linear-interpolation frame-rate reduction
(`origin_hop_size → hop_size`) is applied before feeding the vocoder.

### 3. Three-level F0 fallback

```
FCPE ONNX  ──失败──►  内置 DSP（DIO 风格谐波打分）  ──失败──►  纯乐谱 F0
```

A failure at any level only degrades quality, never aborts the render, so
machines without models or with corrupted models still produce sound (with
reduced quality).

### 4. Cache keys

The cache key includes: source file path, file size, mtime (seconds +
nanoseconds), Mel parameter hash, F0 parameter hash, and sample rate. Any
change invalidates the entry and triggers a rebuild, so replacing a sample
never yields stale features.

### 5. Threading model

`Session` is not `Sync`; in batch rendering each worker thread holds its own
backend instance (`--jobs N`). Single-threaded rendering reuses the same
instance to avoid loading the models repeatedly.

## Extension Points

* **New editor**: add a module under `src/adapters/` that produces a `RenderRequest`.
* **New F0 backend**: implement the `F0Extractor` trait and register a string key in the factory function in `core/f0/mod.rs`.
* **New vocoder**: implement the `backend::Vocoder` trait; if it is still ONNX, only the input/output node names in the config need to change.
* **New execution provider**: add a branch in `build_providers()` in `backend/ort_backend.rs` and gate compilation with a cargo feature.
