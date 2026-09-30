# Introduction

**neural-resampler** is a general-purpose neural resampler engine that rewrites
HiFiSampler in **pure Rust + ONNX**.

A traditional UTAU voicebank consists of two parts: the recorded samples (WAV)
and the splice parameters for each sample (`oto.ini`). The engine responsible
for shifting pitch, changing durations, and stitching the fragments into song
is the resampler engine. This project has exactly one goal:
**replace only that engine, and touch nothing else**.

## Design trade-offs

| Dimension | Description |
| --- | --- |
| Voicebank compatibility | WAV and `oto.ini` are left completely untouched and reused as-is |
| Vocoder | PC-NSF-HiFiGAN (NSF architecture, F0-conditioned) |
| F0 extraction | FCPE (ONNX, 16kHz), with fallback to the built-in DSP |
| Dependencies | No Python runtime; single-binary deployment |
| Host compatibility | OpenUtau / native UTAU; the protocol layer is extensible |

## How pitch and duration changes work

The clever part of the neural approach is that it **decouples** the two:

* **Pitch** — change the F0 curve directly. The vocoder is conditioned on F0,
  so a different F0 produces a different pitch; no waveform resampling is
  involved, and therefore none of the metallic sound of traditional resampling.
* **Duration** — time-stretch or loop-splice the Mel spectrogram. What changes
  is the frame count, independent of pitch.

This means a single sample can be stretched to any length and sung at any
pitch, with the two never interfering.

## Processing pipeline

```
输入 WAV
   │
   ├─ Mel 分析（44.1kHz / 2048 FFT / 帧移 128）
   │
   ├─ F0 提取（FCPE，16kHz）  ──┐
   │                             ├─→ 按乐谱调整 F0
   ├─ 乐谱：音名 + pitchBend ────┘
   │
   ├─ Mel 时序拉伸到目标帧数
   │
   ├─ PC-NSF-HiFiGAN 声码器（Mel + F0 → 波形）
   │
   └─ 后处理（响度归一化、限幅、淡入淡出）→ 输出 WAV
```

## When to use it

**A good fit**: you want traditional voicebanks to sound more natural; you
need to deploy on machines without a Python environment; or you want a Rust
implementation that is readable, modifiable, and embeddable in other tools.

**Not a good fit**: expecting it to conjure phonemes that do not exist (it is
still concatenative synthesis — only with more natural splicing); needing GPU
acceleration while refusing to install CUDA/DirectML (CPU is the default —
usable, but slow).

## License

MIT. The models used follow the licenses of their respective sources; confirm
them yourself before distributing.
