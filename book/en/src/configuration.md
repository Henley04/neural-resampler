# Configuration File

The default configuration is embedded in the binary at compile time. To change it, export a copy first:

```bash
./resampler config my.yaml
./resampler --config my.yaml render voice/_a.wav out.wav A4
```

> ⚠️ The values of `mel.*` and `f0.*` are **tied to the model**. When switching to an ONNX model with different specifications,
> you must update these parameters accordingly, otherwise the output will sound muffled, distorted, or be completely silent. If you use the default models, do not touch them.

## models_dir

Root directory of the models. All `model` fields in the configuration are paths relative to it; **do not write `models/` a second time**.

```yaml
models_dir: "models"     # ✅ 则 vocoder.model 写 "pc_nsf_hifigan.onnx"
```

## mel — Spectral Analysis

| Field | Default | Description |
| --- | --- | --- |
| `sample_rate` | 44100 | Analysis sample rate |
| `n_fft` | 2048 | FFT size |
| `win_size` | 2048 | Window length |
| `hop_size` | 512 | Rendering hop size |
| `origin_hop_size` | 128 | Analysis hop size (finer; used for time-domain interpolation) |
| `n_mels` | 128 | Number of Mel bins |
| `fmin` / `fmax` | 40 / 16000 | Frequency range (Hz) |
| `clip_val` | `1e-9` | Lower bound for log compression |
| `magnitude_eps` | 0.0 | Magnitude floor; `0` = plain `abs()` |
| `mel_scale` | `slaney` | Scale: `slaney` (librosa default) / `htk` |

## f0 — Pitch Extraction

| Field | Default | Description |
| --- | --- | --- |
| `backend` | `auto` | `auto` / `fcpe` / `world` / `none` |
| `mode` | `hybrid` | `score` / `source` / `hybrid`, see [F0 modes](f0-modes.md) |
| `model` | `fcpe.onnx` | FCPE model filename |
| `sample_rate` | 16000 | FCPE input sample rate |
| `hop_size` / `win_size` / `n_fft` | 160 / 1024 / 1024 | FCPE spectrogram parameters |
| `mel_bins` | 128 | Number of Mel bins for FCPE |
| `fmin` / `fmax` | 0 / 8000 | FCPE frequency range |
| `f0_min` / `f0_max` | 80 / 880 | Valid F0 range (Hz); values outside are treated as invalid |
| `uv_threshold` | 0.006 | Confidence below this value is treated as unvoiced |
| `uv_mask` | true | Zero out the F0 of unvoiced frames |
| `max_deviation_cents` | 100.0 | Maximum natural deviation allowed in hybrid mode |
| `smoothing_ms` | 40.0 | Smoothing window of the baseline curve |
| `decoder` | `local_argmax` | latent→cent decoding: `local_argmax` / `argmax` |
| `local_argmax_width` | 4 | One-sided window width (total window 2×width+1) |
| `cent_f0_min` / `cent_f0_max` | 32.70 / 1975.5 | Frequency range of the cent table (C1~B6) |
| `clip_val` | `1e-5` | Lower bound for log compression in F0 analysis |

## vocoder — Vocoder

| Field | Default | Description |
| --- | --- | --- |
| `backend` | `ort` | Inference backend |
| `model` | `pc_nsf_hifigan.onnx` | Vocoder model |
| `hnsep_model` | `hnsep.onnx` | Optional; harmonic/noise separation |
| `mel_input` / `f0_input` | `mel` / `f0` | Input node names |
| `output` | `audio` | Output node name |
| `mel_layout` | `auto` | `auto` / `channels_first` / `frames_first` |
| `providers` | `[cpu]` | Tried in order: `cpu` / `directml` / `cuda` / `coreml` |

`mel_layout: auto` reads the model metadata to automatically determine whether the input is `[1,n_mels,T]` or `[1,T,n_mels]`.
If your model is misdetected and the output is wrong, specify it explicitly.

## processing — Processing

| Field | Default | Description |
| --- | --- | --- |
| `loop_mode` | true | Reflection-loop concatenation of the vowel segment for long notes |
| `fill` | 6 | Extra frames kept at the start and end |
| `trim_silence` | false | Trim silence before analysis |
| `silence_threshold_db` | -52.0 | Silence detection threshold |
| `gender` | 0 | Default gender offset (corresponds to the `g` flag) |

## output — Output

| Field | Default | Description |
| --- | --- | --- |
| `sample_rate` | 44100 | Output sample rate |
| `bit_depth` | 16 | `16`/`24`/`32` integer PCM; `0` for 32-bit float |
| `peak_limit` | 1.0 | Peak limit |
| `wave_norm` | true | Enable loudness normalization |
| `loudness_target` | -16.0 | Target loudness |
| `loudness_block_ms` | 400.0 | Loudness measurement block size |
| `fade_in_ms` / `fade_out_ms` | 0.0 / 0.0 | Fade in/out durations |

## cache — Feature Cache

| Field | Default | Description |
| --- | --- | --- |
| `enabled` | true | Enable caching |
| `extension` | `nrc` | Cache file extension |
| `zstd_level` | 3 | Compression level |
| `dir` | null | Cache directory; `null` means the same directory as the input |

The cache key includes the source file size, modification time, and configuration fingerprint. **Changing the configuration invalidates the cache automatically** — no manual cleanup needed.

## runtime — Runtime

| Field | Default | Description |
| --- | --- | --- |
| `log_level` | `info` | Log level |
| `log_format` | `text` | `text` / `json` |
| `intra_op_threads` | 0 | Intra-op threads; `0` = auto |
| `inter_op_threads` | 0 | Inter-op threads; `0` = auto |
