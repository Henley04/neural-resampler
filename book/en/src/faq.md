# FAQ

### Do I need to change my voicebank?

No. The WAV files and `oto.ini` stay as they are; just point the resampler in your editor to this program.

### Why are there no models in the release package?

Two reasons: the models total about 100 MB, and their licenses may not allow redistribution.
So you obtain the models yourself; the release package includes `download_models.sh`
(`download_models.ps1` on Windows).

### Can it run without models?

Depends on the use case. `resampler selftest` and `resampler info` are unaffected
and can verify the pipeline; but `render` / `batch` / UTAU-protocol rendering
**fails with an explicit error** when models are missing (trilingual message with
the fix), instead of producing unusable audio. If you intentionally need the
degraded backend for offline testing, pass `--allow-stub` or set
`NR_ALLOW_STUB=1`.

### Rendering in OpenUtau fails with "vocoder model missing"?

When OpenUtau installs a resampler it **copies** the exe into its own
`Resamplers/` directory — the models do not come along. Since v0.1.3 this is no
longer a silent degradation (previously: muffled audio, no error); rendering now
fails with a trilingual error dialog, and a `MODEL-MISSING-READ-ME.txt` file
appears next to the resampler. Two ways to fix:

1. **Recommended**: run `download_models.sh` / `download_models.ps1` once in the
   release package directory — the script automatically writes `NR_MODELS_DIR`
   into your user environment (the exe can then live anywhere; just do not move
   the models directory while using this method)
2. Or copy `models/` to `Resamplers/models/` (next to the exe)

Then run `resampler info` and confirm the vocoder backend shows `onnxruntime`
instead of `stub`.

### Why is the first render slow?

The first run has to do Mel analysis and F0 extraction, and load the ONNX models. A second render of the same input
hits the cache and is much faster. When rendering a whole melody, the models load only once.

### Is GPU supported?

Yes, but you have to compile with the corresponding feature yourself:

```bash
cargo build --release --features cuda      # NVIDIA
cargo build --release --features directml  # Windows
cargo build --release --features coreml    # macOS
```

**Do not enable more than one at a time**, and do not use `--all-features` — the features are mutually exclusive and cause link failures.
Default builds use the CPU.

### How is the output length controlled?

By the 7th argument, `length`, **in milliseconds**. Pass `1000` for one second of output.
Pass `44100` and you get 44.1 seconds.

### Does it support both UTAU and OpenUtau?

Yes. Both use the same 13-argument protocol, so a single binary is enough.
See [UTAU](utau.md) and [OpenUtau](openutau.md) for how to configure them.

### Do all flags take effect?

No. Currently only six flags actually take effect: `g`, `t`, `A`, `P`, `He`, `HG`;
the other flags are parsed but do not yet affect the output. See [UTAU parameters](utau.md#支持的-flags) for details.

### Is the output deterministic?

Yes. The same input + the same arguments + the same configuration produce sample-identical output, with no randomness.

### What about muffled/distorted sound?

99% of the time it is a mismatch between the Mel parameters and the model specifications. Restore the default `mel.*` configuration,
and confirm `mel_scale: slaney`, `sample_rate: 44100`, `n_mels: 128`.
If you switched to a non-default model, you must update these parameters accordingly.

### Can I delete the cache files?

Yes. Deleting them only makes the next render a bit slower. The cache key includes the source file size, modification time, and configuration fingerprint,
so manual cleanup is normally unnecessary.

### Is commercial use allowed?

The code is MIT. But **the model licenses are governed by their respective sources**; confirm them yourself before commercial use.
This is also one of the reasons the models are not distributed with the repository.
