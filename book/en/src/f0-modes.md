# F0 Generation Modes

The F0 (fundamental frequency) curve determines how high the output sings and how it glides between pitches. The `f0.mode` setting determines where F0 comes from.

| Mode | Behavior | Use case |
| --- | --- | --- |
| `score` | F0 comes entirely from the score (note names + pitchBend), identical to the original HiFiSampler | When exact score tracking is needed |
| `source` | Transposes the sample's own F0 curve as a whole to the target pitch | When you want to keep the original sample's vibrato/glides |
| `hybrid` | The score provides absolute pitch; the sample's F0 contributes natural deviation and voicing decisions | **Default**, balancing accuracy and naturalness |

## score

Ignores the sample's own pitch entirely and generates F0 from the score. The result is the most controllable and closest to the MIDI,
but it discards the natural vibrato and timbre variation in the sample and may sound "electronic".

Good for: projects that need strict pitch alignment, stacking harmonies.

## source

Shifts the sample's own F0 curve wholesale to the target pitch. The sample's vibrato, glides, and breath are all preserved,
and it sounds the most "like the original voicebank", but the absolute pitch will not line up with the score (if the sample itself sings sharp, it stays sharp).

Good for: when naturalness matters most, and when the score itself was transcribed from the sample.

## hybrid (default)

Uses the score as the absolute baseline, then overlays the **relative deviation** of the sample's F0, and uses the sample's voicing
decisions to distinguish voiced/unvoiced frames. This both matches the pitch and preserves natural variation.

The deviation is limited by `f0.max_deviation_cents` (default 100 cents, i.e. one semitone),
so extreme deviations in the sample cannot drag the pitch off course. The smoothing window of the baseline curve is controlled by `f0.smoothing_ms` (default 40ms).

Good for: almost every scenario. Lower `max_deviation_cents` to track the score more tightly,
raise it for more naturalness (or just use `source`).

## How to Choose

Start with the default `hybrid`. If the pitch is not accurate enough, move toward `score`;
if it feels too rigid, move toward `source`. The difference between the three may not be obvious on a single note,
but it becomes audible once they form a melody line.

## About F0 Backends

Priority when `f0.backend: auto`:

1. **FCPE (ONNX)** — the most stable for singing; requires `fcpe.onnx`
2. **Built-in DSP** — DIO/Harvest style, no model needed
3. **Score only** — the last-resort fallback

The `F0 后端` line of `resampler info` tells you which one is actually in use.

> FCPE's output is a **360-class cent classification latent**, not direct F0 values.
> The program decodes it following the official procedure: weighted averaging over the local argmax (±4 classes) → `f0 = 10 · 2^(cent/1200)`;
> frames with confidence ≤ `f0.uv_threshold` (default 0.006) are treated as unvoiced.
