# FAQ

### Do I need to change my voicebank?

No. The WAV files and `oto.ini` stay as they are; just point the resampler in your editor to this program.

### Why are there no models in the release package?

Two reasons: the models total about 100 MB, and their licenses may not allow redistribution.
So you obtain the models yourself; the release package includes `download_models.sh`
(`download_models.ps1` on Windows).

### Can it run without models?

It runs, but is of no practical value. Without models the vocoder degrades to a Stub and F0 falls back to the built-in DSP;
the pipeline still completes and makes sound, but the quality is unusable. This fallback exists to make
connectivity checks easy, not for real use. `./resampler info` tells you exactly which backend is in use.

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
