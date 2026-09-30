# Using with UTAU

The binary itself is the resampler; no extra wrapper script is needed. UTAU
passes the parameters in the agreed order, so call it directly:

```bash
resampler in.wav out.wav C4 100 "" 0 500 60 -50 100 0 !120 AA
```

## Parameter meanings

| # | Parameter | Meaning | Unit / default |
| --- | --- | --- | --- |
| 1 | `in.wav` | Input sample path | — |
| 2 | `out.wav` | Output path | — |
| 3 | `pitch` | Note name | `C4` |
| 4 | `velocity` | Velocity | default `100` |
| 5 | `flags` | Render flags | default empty |
| 6 | `offset` | Left blank | **milliseconds**, default `0` |
| 7 | `length` | Required length | **milliseconds**, default `1000` |
| 8 | `consonant` | Consonant part | **milliseconds**, default `0` |
| 9 | `cutoff` | Right blank | **milliseconds**, usually negative, default `0` |
| 10 | `volume` | Volume | percent, default `100` |
| 11 | `modulation` | Modulation | percent, default `0` |
| 12 | `tempo` | Tempo | `!120` or `120` |
| 13 | `pitchBend` | Pitch curve | Base64+RLE encoded, default `AA` |

> ⚠️ **The 7th parameter `length` is in milliseconds, not samples.**
> Passing `44100` produces 44.1 seconds of output, not 1 second. This is the
> easiest pitfall to hit.

A negative `cutoff` is normal (it denotes the right-side overlap region);
the program accepts negative parameters.

## Supported flags

Flags look like `g-3P60HG20` and are parsed longest-flag-first (the `/`
separator is ignored). **Only the flags below take effect in the current
version**; all other flags are parsed but do not affect the output:

| Flag | Meaning | Values |
| --- | --- | --- |
| `g` | Gender/timbre shift | `g-3` means −3, in units of roughly 1/100 semitone; defaults to `processing.gender` in the config |
| `t` | Fine pitch adjustment | integer, default `0` |
| `A` | Amplitude modulation following pitch changes | integer, default `0` (off) |
| `P` | Loudness normalization strength | `P60` = 60%, default `100` |
| `He` | Force loop splicing | switch-type; its presence enables it |
| `HG` | growl | `HG20` = 20% strength |

## Configuring in UTAU

1. Put `resampler.exe` in a fixed directory (the path must **not contain
   spaces or non-ASCII characters**; UTAU's compatibility here is poor)
2. Open UTAU → `Tools` → `Options` → `Voicebank plug-in / Resampler`
3. Select "Custom" or enter the full path to the resampler in the settings
4. Confirm and render a note to test it

If UTAU reports "resampler not responding" or the output is empty, first run
the same parameters manually from the command line and look at the program's
own error — the vast majority of problems are obvious from the command line.

## Logging and debugging

When called from UTAU, terminal output is not visible; raise the log level
and redirect it to a file:

```bash
resampler --log-level debug in.wav out.wav C4 100 "" 0 500 60 -50 100 0 !120 AA 2> debug.log
```

`--log-format json` outputs structured logs, convenient for script-based
analysis.

## Performance tips

The first render of a sample performs Mel analysis and F0 extraction, and the
results are cached as a `.nrc` file (zstd-compressed) in the same directory.
A second render of the same input is noticeably faster. The cache key
includes the source file size, modification time, and configuration
fingerprint, so changing the configuration invalidates and rebuilds the cache
automatically — **there is no need to delete the cache manually**.
