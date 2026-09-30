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
