# Models

The engine requires two ONNX models, both placed in `models/` (set via `--models <DIR>` or the `models_dir` config key).

| File | Purpose | Source |
| --- | --- | --- |
| `pc_nsf_hifigan.onnx` | Vocoder: Mel + F0 → waveform | [openvpi/vocoders](https://github.com/openvpi/vocoders) (must be exported to ONNX) or a ready-made export |
| `fcpe.onnx` | F0 extraction (16kHz input) | [CNChTu/FCPE](https://github.com/CNChTu/FCPE) |
| `hnsep.onnx` (optional) | HN-SEP harmonic/noise separation; improves breathy and noisy segments | Optional; skipped when missing |

## Automatic Download

```bash
bash scripts/download_models.sh          # 下载到 ./models
bash scripts/download_models.sh /path/to/dir
powershell -ExecutionPolicy Bypass -File scripts/download_models.ps1   # Windows
```

The scripts only download and verify; no conversion is performed. If GitHub is
unreachable directly or the speed falls below 100KB/s, they automatically
switch to the gh-proxy mirror (force one with `NR_MODEL_MIRROR=<mirror prefix>`).

## Manual Preparation

### Vocoder

Download the `pc-nsf-hifigan-44.1k-hop512-128bin-*` release package from
[openvpi/vocoders](https://github.com/openvpi/vocoders/releases), then export
the resulting `.ckpt` with `scripts/convert_vocoder_to_onnx.py` (requires
`torch` + `onnx`):

```bash
python3 scripts/convert_vocoder_to_onnx.py \
    --ckpt pc_nsf_hifigan_44.1k_hop512_128bin_2025.02/model.ckpt \
    --out models/pc_nsf_hifigan.onnx
```

Export requirements (must match `mel.*` in `config/resampler.yaml`):

| Parameter | Value |
| --- | --- |
| sampling_rate | 44100 |
| num_mels | 128 |
| n_fft / win_size | 2048 |
| hop_size | 512 |
| fmin / fmax | 40 / 16000 |
| mini_nsf | true |

### F0 Model

The official FCPE repository provides PyTorch weights, and community-exported
`fcpe.onnx` files already exist. If you export it yourself, the input is a
16kHz **Mel spectrogram** (`[1, T, 128]`) and the output is F0 (Hz).

## Node Names

The default configuration assumes:

```
声码器输入： mel       [1, T, 128]   float32
            f0        [1, T]        float32
声码器输出： waveform  [1, N]        float32
```

If the node names in your export differ, just change
`vocoder.mel_input / f0_input / output` — no code changes needed.

Use `resampler info` to check whether the models are in place and loadable.

## Behavior When Models Are Missing

| Missing | Behavior |
| --- | --- |
| Vocoder | Falls back to `backend::stub` (simple spectral reconstruction from Mel); **audio quality is unusable**; only keeps the pipeline connected |
| FCPE | Falls back to the built-in DSP F0 (`f0.backend: world`), then to score-only F0 if that fails too |
| HN-SEP | The processing step is simply skipped |

## Verification

```bash
resampler info                      # 列出模型是否存在 + 大小 + sha256 前 16 位
resampler selftest --out-dir /tmp/nr
```

In CI, ONNX-related assertions are skipped automatically when the models are
absent (see `.github/workflows/ci.yml`).
