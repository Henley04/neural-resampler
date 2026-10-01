# Obtaining Models

The engine needs ONNX models to produce usable audio quality. **Models are not distributed with the repository or the release artifacts** — they total about 100 MB,
and their respective licenses may not allow redistribution, so obtain them yourself.

## Where to Put Them

Place the models in the `models/` directory **next to the binary or at a specified location**:

| File | Purpose | Required |
| --- | --- | --- |
| `pc_nsf_hifigan.onnx` | Vocoder: Mel + F0 → waveform | **Yes** |
| `fcpe.onnx` | F0 extraction (16kHz) | No, falls back to the built-in DSP when missing |
| `hnsep.onnx` | HN-SEP harmonic/noise separation | No |

Use `--models <DIR>` to point to a different directory temporarily:

```bash
./resampler --models /path/to/models info
```

## One-Command Download

The release artifacts include download scripts (one for bash and one for PowerShell, same logic):

```bash
./download_models.sh      # Linux / macOS / Git Bash
```

```powershell
powershell -ExecutionPolicy Bypass -File .\download_models.ps1    # Windows PowerShell
```

A copy also lives in the repository (`scripts/download_models.sh` and `scripts/download_models.ps1`).
The scripts retry automatically and perform a **SHA-256 check** after downloading (baseline: the versions verified alongside v0.1.0).
If a file is corrupted or was swapped upstream, the script refuses to install and reports it.

### Download Sources and Mirror Acceleration

By default the scripts connect directly to GitHub (`raw.githubusercontent.com`) and have gh-proxy mirror acceleration built in:

1. **Auto detection**: when GitHub is unreachable, the script switches to the mirror automatically and says so
2. **Slow-speed switch**: when the average download speed stays below 100KB/s for 10 seconds and the current mode is direct connection, the script
   asks whether to switch to the mirror (`NR_MODEL_AUTO_SWITCH=1` switches automatically without asking);
   after direct-connection retries are exhausted, one final round runs via the mirror automatically
3. **Force mirror**: set `NR_MODEL_MIRROR` to use the given mirror prefix directly
   ([gh-proxy](https://github.com/hunshcn/gh-proxy) format: prefix + full original URL):

   ```bash
   NR_MODEL_MIRROR=https://gh-proxy.com/ ./download_models.sh
   ```

   You can also swap in any self-hosted or other gh-proxy instance.

> When an upstream model update breaks the checksum and you have confirmed the new version works, you can set
> `NR_MODEL_SKIP_CHECKSUM=1` to skip the check temporarily — **make sure the source is trustworthy before doing so**
> (tampered mirror content is also caught by the check).

## Manual Download

If the scripts stop working, download manually, rename to the filenames in the table above, and put them in `models/`:

1. **FCPE** — the releases page of [CNChTu/FCPE](https://github.com/CNChTu/FCPE); take `fcpe.onnx`
2. **PC-NSF-HiFiGAN** — ONNX format required. If you only have `.ckpt` weights,
   use the conversion script in the repository:
   ```bash
   python scripts/convert_vocoder_to_onnx.py --ckpt model.ckpt --out models/pc_nsf_hifigan.onnx
   ```

## Verify the Models

```bash
./resampler info
```

Watch these three lines:

```
声码器  : "models/pc_nsf_hifigan.onnx"  [已就绪]
FCPE    : "models/fcpe.onnx"  [已就绪]
HN-SEP  : "models/hnsep.onnx"  [缺失]
```

* Vocoder `[缺失]` → falls back to the Stub backend; **it makes sound but the quality is unusable**
* FCPE `[缺失]` → falls back to the built-in DSP for F0 extraction; still works fine, just slightly less stable for singing
* HN-SEP `[缺失]` → normal; it is an optional enhancement

## Licenses

Follow the license terms of each model's source repository/releases page. Before bundling models into your own distribution,
always confirm the license allows it.
