# Quick Start

This assumes you have already [installed](installation.md) the program and
[obtained the models](models.md).

## Step 1: Self-test

```bash
./resampler selftest
```

This command generates a 440Hz test tone, runs the full pipeline end to end,
then prints:

```
已生成测试输入: "target/selftest/selftest_input.wav"
输出: "target/selftest/selftest_output.wav"（24696 样本 / 560.0ms / 峰值 0.254）
自检通过
```

If you see `自检通过` (self-test passed), the installation is complete. Output
goes to `target/selftest/` by default; use `--out-dir` to change it:

```bash
./resampler selftest --out-dir /tmp/nr-test
```

## Step 2: Check the backend status

```bash
./resampler info
```

Focus on the last two lines:

```
声码器后端: onnxruntime
F0 后端    : fcpe-onnx
```

If it shows `stub` / `world-dio`, the models were not found — go back to
[Obtaining the models](models.md).

## Step 3: Render a note

```bash
./resampler render voice/_a.wav out.wav A4
```

It also runs with only the two paths; every other parameter has a default
(A4, velocity 100, length 1000ms). The full parameter list:

```bash
./resampler render voice/_a.wav out.wav A4 100 "g-3" 30 500 60 -50 100 0 '!120' AA
```

The output looks like:

```
已写出 "out.wav"：87 帧 / 500.0ms / 后端 onnxruntime / F0 fcpe-onnx / 耗时 812.3ms
```

## Step 4: Use it in an editor

The binary itself is the resampler, so call it directly following the UTAU
protocol:

```bash
resampler in.wav out.wav C4 100 "" 0 500 60 -50 100 0 !120 AA
```

See [Using with UTAU](utau.md) or [Using with OpenUtau](openutau.md) for the
integration steps.

## Batch rendering

Put multiple sets of UTAU parameters into one file, one set per line:

```text
# list.txt
voice/_a.wav out/a.wav C4 100 "" 0 500 60 -50 100 0 !120 AA
voice/_i.wav out/i.wav D4 100 "" 0 500 60 -50 100 0 !120 AA
```

```bash
./resampler batch list.txt --jobs 4
```

Lines starting with `#` are ignored. `--jobs 0` lets the program decide the
parallelism automatically.

## Changing the configuration

Export the default configuration, edit it as needed, then pass it with
`--config`:

```bash
./resampler config my.yaml
# 编辑 my.yaml
./resampler --config my.yaml render voice/_a.wav out.wav A4
```

See [Configuration file](configuration.md) for the available options.
