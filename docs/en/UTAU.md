# UTAU / OpenUtau Integration

## Protocol

The binary itself is the resampler; call it directly with UTAU's 13-argument protocol:

```
resampler <in.wav> <out.wav> <pitch> <velocity> <flags> <offset> <length_req>
          <consonant> <cutoff> <volume> <modulation> <tempo> <pitchBend>
```

| Position | Argument | Unit | Description |
| --- | --- | --- | --- |
| 1 | in.wav | — | Voicebank sample |
| 2 | out.wav | — | Rendered result |
| 3 | pitch | Note name | e.g. `C4`, `A#3` |
| 4 | velocity | 0–200 | Intensity |
| 5 | flags | — | See below |
| 6 | offset | ms | Left blank |
| 7 | length_req | ms | Requested length |
| 8 | consonant | ms | Consonant part (not stretched) |
| 9 | cutoff | ms | Right blank, usually negative |
| 10 | volume | % | Volume |
| 11 | modulation | % | Modulation |
| 12 | tempo | `!BPM` or BPM | Tempo |
| 13 | pitchBend | Base64+RLE | Pitch curve, in cents |

`cutoff` is usually negative (e.g. `-50`); the CLI enables
`allow_negative_numbers`, so it can be passed directly.

## Setup in OpenUtau

Put `resampler` (`resampler.exe` on Windows) into OpenUtau's `Resamplers`
directory (under the program directory on Windows; `~/.local/share/OpenUtau/Resamplers`
on Linux; or drag it into the OpenUtau window and choose "Install as resampler"),
switch the renderer to `CLASSIC`, then click the ⚙ gear next to it and select
it. The voicebank's `oto.ini` and WAV files need no changes at all.
See the [OpenUtau wiki: Resamplers and Wavtools](https://github.com/openutau/OpenUtau/wiki/Resamplers-and-Wavtools).

## Model directory resolution

When OpenUtau installs a resampler it **copies** the executable into
`Resamplers/`; there is no `models/` folder in the working directory at render
time. When models are missing, the engine **no longer degrades silently**:
rendering fails with an explicit error (trilingual message, plus a
`MODEL-MISSING-READ-ME.txt` file next to the resampler). The model directory is
resolved in this order: command-line `--models` > the `NR_MODELS_DIR`
environment variable > a `models/` folder next to the executable (i.e.
`Resamplers/models/`) > a `models/` folder in the working directory. For
OpenUtau, run the bundled `download_models.sh` / `download_models.ps1` once —
the script automatically writes `NR_MODELS_DIR` into your user environment
(`NR_SKIP_ENV=1` to skip; ⚠ do not move the models directory while using this
method) — or put the models into `Resamplers/models/`. Then confirm with
`resampler info` that the vocoder backend shows `onnxruntime`. For offline
testing with the degraded backend, pass `--allow-stub` or set
`NR_ALLOW_STUB=1`.

## Supported Flags

| Flag | Meaning | Implementation |
| --- | --- | --- |
| `g±N` | Gender/formant shift (0.01 semitone) | Scales the window length at the analysis stage (key shift) |
| `P±N` | Loudness normalization strength (%) | Interpolates between the raw and normalized results |
| `t/N` | Vibrato speed | Superimposed on `pitchBend` |
| `A/B/G/S/p/R/D/C/Z` | HiFiSampler compatibility flags | Parsed but not enforced |

Unknown flags are parsed into the map but ignored; they never cause rendering
to fail.

## pitchBend Decoding

1. Split by `#`: `<b64>#<rle>#<b64>#<rle>#...`
2. Every two Base64 characters decode to one 12-bit signed integer (`-2048..2047`)
3. RLE segments repeat the previous value
4. Append a trailing 0 (same as the original implementation)

The unit is **cent**, with values in ±2048 (about ±20 semitones). The sampling
grid is `60 / (tempo × 96)` seconds per point; time points beyond the curve
range are clamped to the endpoint values.

## oto.ini

The engine reads the `oto.ini` in the same directory for flag / offset defaults
(command-line arguments take precedence). Encodings are tried in the order
UTF-8 → Shift-JIS → GBK, so both Japanese and Simplified Chinese voicebanks
work.

## Batch Rendering

Write each set of arguments on its own line and hand the list to `batch` for
parallel processing:

```bash
resampler batch list.txt --jobs 4
```

Each worker holds its own ONNX session (`Session` is not `Sync`); the models
are not loaded repeatedly.
