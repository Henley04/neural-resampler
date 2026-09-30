# Troubleshooting

When something goes wrong, **reproduce it from the command line first**. Errors inside the editor usually carry little
information, while the command line gives you the specific cause:

```bash
./resampler --log-level debug render <样本> /tmp/t.wav C4 100 "" 0 500 0 0 100 0 '!120' AA
```

## Symptom Table

| Symptom | Possible cause | Fix |
| --- | --- | --- |
| Models show `[缺失]` in `info` | Models not placed in `models/` | Run `./download_models.sh` (Windows: `.\download_models.ps1`), or point `--models` at the actual directory |
| Downloading models is slow or fails | Poor direct connectivity to GitHub | The script switches to a gh-proxy mirror automatically; you can also force the mirror with `NR_MODEL_MIRROR=https://ghfast.top/` |
| Vocoder backend is `stub` | Same as above | Same as above. **The Stub is for connectivity checks only; the quality is unusable** |
| F0 backend is `world-dio` | `fcpe.onnx` missing | Works fine, just slightly less stable for singing; add the model if you want FCPE |
| Output length is clearly wrong | `length` is in **milliseconds** | Pass `1000` for one second, not `44100` |
| Output is almost silent | Input sample too quiet / loudness normalization suppressing the sound | Check the input volume; try `output.wave_norm: false` for comparison |
| Muffled sound, as if covered | `mel.*` does not match the model specifications | Restore the default `mel.*`; confirm `mel_scale: slaney` |
| Broken sound with metallic artifacts | `mel_layout` misdetection | Explicitly set `vocoder.mel_layout: channels_first` or `frames_first` and test |
| Pitch is wrong overall | Note name format / F0 mode | Confirm note names like `C4`; try a different `f0.mode` (see [F0 modes](f0-modes.md)) |
| Build fails with an ort link error | Multiple GPU features enabled at once | **Do not use `--all-features`**; GPU features are mutually exclusive |
| SIGSEGV when the process exits (exit 139) | dylib version mismatch when linking with `load-dynamic` | See [load-dynamic version constraints](#load-dynamic-版本约束) below |
| Every render is slow | Cache misses | Confirm `cache.enabled: true`; changing the configuration changes the fingerprint and forces a rebuild |
| macOS says it "cannot be opened" | Unsigned binary | `xattr -d com.apple.quarantine ./resampler` |
| No reaction inside UTAU | Path contains spaces or Chinese characters | Put the program in a pure-English, space-free path, e.g. `C:\nr\` |
| pitchBend warnings in the log | Abnormal pitchBend string length | Already handled gracefully: only trailing characters are dropped; the overall rendering is unaffected |

## Which Layer Is the Problem In?

Troubleshoot in this order to narrow it down quickly:

**1. Are the models there?**

```bash
./resampler info
```

Two `[已就绪]` entries means everything is in place.

**2. Can the engine produce sound?**

```bash
./resampler selftest
```

Runs the built-in 440Hz test tone through the pipeline. If this fails, the problem is in the engine/models,
unrelated to your voicebank.

**3. Can your sample produce sound?**

```bash
./resampler render <你的样本> /tmp/t.wav C4
```

If selftest passes but this fails, the problem is in the sample itself (format, sample rate, duration).

**4. Are the editor's parameters correct?**

If the command line produces sound but the editor does not, the problem is in the editor's invocation arguments or path configuration.

## Logs

Raise the log level to see details such as model loading, backend selection, Mel shapes, and F0 statistics:

```bash
./resampler --log-level debug --models ./models info
```

`--log-format json` emits structured logs, handy for scripting:

```bash
./resampler --log-format json render a.wav b.wav C4 2>&1 | grep '"level":"error"'
```

## load-dynamic Version Constraints

The default release binaries statically link ONNX Runtime into the executable, so this problem does not occur.
But if you build yourself with `load-dynamic` (loading `libonnxruntime.so` /
`onnxruntime.dll` at runtime), there is a subtle pitfall:

**The dylib version must exactly match the version ort-sys expected at build time** (e.g. 1.28.0).
With adjacent versions (e.g. 1.19.2, 1.22.0) — rendering works perfectly, but **the process SIGSEGVs
during exit** (exit 139): the crash point is `exit() → ld-linux 析构 → libonnxruntime`;
the gdb backtrace looks like a library-unloading problem, but it is actually a cleanup path going out of bounds due to ABI incompatibility.

How to diagnose:

* Confirm the loaded library version: `ldd resampler | grep onnxruntime`, then check that .so's version
* Switch to the exact version matching the build configuration, or use static linking (the default release mode)
* On CI hosts that check exit codes, this problem shows up as "rendering succeeded but the job failed"

## About the Cache

The cache files are `.nrc` (zstd-compressed) in the same directory as the input. The cache key = source file path + size + mtime +
configuration fingerprint + sample rate, so:

* Configuration changed → automatically invalidated and rebuilt
* Source file changed → automatically invalidated and rebuilt
* Manually deleting `.nrc` only makes the next render slower once; nothing breaks

To disable the cache: `cache.enabled: false`.

## Still Stuck

Ask at [Issues](https://github.com/Henley04/neural-resampler/issues) with:

* The full output of `./resampler info`
* The command line and arguments used to reproduce
* The logs from `--log-level debug`
* Your operating system and platform
