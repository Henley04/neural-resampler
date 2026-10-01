# Using with OpenUtau

OpenUtau manages third-party resamplers through the `Resamplers` directory.
It passes 13 parameters following the standard resampler protocol — exactly the
same as [native UTAU](utau.md) — so no extra parameter template setup is needed.

## Setup steps

Following the [official OpenUtau wiki](https://github.com/openutau/OpenUtau/wiki/Resamplers-and-Wavtools):

1. Put `resampler` (`resampler.exe` on Windows) into OpenUtau's `Resamplers` folder:
   * Windows: `Resamplers` under the OpenUtau program directory
   * Linux: `~/.local/share/OpenUtau/Resamplers`
   * You can also drag the executable onto the OpenUtau main window and choose **"Install as resampler"** (0.1.119+)
2. Switch the renderer to **`CLASSIC`**
3. Click the **⚙ gear icon** next to the renderer and select this resampler in the Resampler dropdown
4. Re-render the project

> Optional: place a `.yaml` file named after the executable (e.g. `resampler.yaml`) in the
> `Resamplers` directory as a Resampler Manifest, declaring to OpenUtau the flags
> (expressions) this engine supports; the expression panel then shows suggested values and ranges.

## Placing the models (important)

When you install a resampler, OpenUtau **copies** the executable into its own
`Resamplers/` directory. At render time the working directory is the OpenUtau
installation root — the `models/` folder there does not exist. When models are
missing, the engine **no longer degrades silently**: rendering is refused,
OpenUtau shows an error dialog with a trilingual message including the fix, and
a `MODEL-MISSING-READ-ME.txt` warning file is created next to the resampler
executable.

The model directory is resolved in this order:

1. Command-line `--models` (OpenUtau never passes it; for manual CLI use only)
2. The `NR_MODELS_DIR` environment variable (**recommended**: set automatically by the download script, see below)
3. A `models/` folder **next to the executable** (i.e. `Resamplers/models/`)
4. A `models/` folder in the current working directory (release package layout)

For OpenUtau, option 2 or 3 is recommended:

* **Recommended**: run `download_models.sh` / `download_models.ps1` once in the
  release package directory. After downloading and verifying the models, the
  script automatically writes `NR_MODELS_DIR` into your user environment
  (`NR_SKIP_ENV=1` to skip). Afterwards the resampler finds the models no matter
  where it was copied — just drag the exe into OpenUtau, no other setup needed.
  ⚠ While using this method, do **not** move or rename that `models/` directory;
  if you must move it, rerun the script or update `NR_MODELS_DIR` manually.
* Or place a copy of `models/` at `Resamplers/models/` (next to the exe).

Afterwards run `resampler info` and make sure the vocoder backend shows
`onnxruntime` (not `stub`).

> If you intentionally need the degraded backend for offline testing: pass
> `--allow-stub` or set `NR_ALLOW_STUB=1`; the `selftest` and `info`
> subcommands are unaffected.

## Path notes

* Once placed in the `Resamplers` directory (or installed via drag-and-drop), the path is managed by OpenUtau — no need to specify it manually
* On macOS / Linux, make sure the binary has execute permission: `chmod +x resampler`
* If Gatekeeper blocks the binary on first run on macOS (it is unsigned), allow it in
  "System Settings → Privacy & Security", or run: `xattr -d com.apple.quarantine ./resampler`
* Running the Windows resampler on macOS / Linux requires Wine (`Tools > Preferences >
  Advanced > Wine Path`); this repo provides native macOS/Linux builds, so Wine is not needed

## Verifying the configuration

Render a note in OpenUtau and check the output WAV. You can also verify the same
parameters independently from the command line:

```bash
./resampler render <样本路径> /tmp/check.wav C4 100 "" 0 500 60 -50 100 0 '!120' AA
```

If the command line produces sound but OpenUtau does not, the problem is most likely
paths or permissions, not the engine itself.

## Differences from classic resamplers

OpenUtau's bundled world-based resamplers (e.g. `worldline`) perform phase alignment
at splice points; this engine takes the neural vocoder route and outputs waveforms
synthesized by the model. Therefore:

* The sound is closer to natural vocal production, but **every render is exactly identical** (no randomness)
* Long notes rely on loop splicing; the `He` flag forces it on
* Inference runs on the CPU, so a single render takes longer than with classic resamplers — use the cache alongside

## Performance

The first render carries model-loading overhead (tens of milliseconds); after that, the
per-note time is dominated by Mel analysis and vocoder inference. With the cache
enabled, re-rendering the same input is much faster.

For GPU acceleration, build yourself with `--features cuda` (NVIDIA) or
`--features directml` (Windows); see [Installation](installation.md).
