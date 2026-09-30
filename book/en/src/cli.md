# Command Line Reference

## Two Invocation Forms

**1) UTAU protocol (no subcommand)** — used by host editors; the argument order is fixed:

```bash
resampler in.wav out.wav C4 100 "" 0 500 60 -50 100 0 !120 AA
```

**2) Explicit subcommands** — for manual use:

```bash
resampler render  in.wav out.wav A4
resampler batch   list.txt --jobs 4
resampler info
resampler selftest
resampler config  my.yaml
```

The program first checks whether the arguments look like a UTAU invocation (at least 4 positional arguments and the first one is not a subcommand name);
if so it takes the protocol path, otherwise it parses them as subcommands.

## Global Options

Can be placed **before** the subcommand (or after it; both count as `global`):

| Option | Description |
| --- | --- |
| `--config <FILE>` | Specify a configuration file |
| `--models <DIR>` | Override the model directory from the configuration |
| `--log-level <LEVEL>` | `error`/`warn`/`info`/`debug`/`trace`, default `info` |
| `--log-format <FORMAT>` | `text` or `json`, default `text` |

Global options also work before a bare UTAU invocation, so you can specify models without changing how the host invokes the program:

```bash
resampler --models /opt/nr/models in.wav out.wav C4 100 "" 0 500 60 -50 100 0 !120 AA
```

## render

Renders a single note according to the UTAU protocol.

```bash
resampler render <IN> <OUT> [pitch] [velocity] [flags] [offset] [length]
                 [consonant] [cutoff] [volume] [modulation] [tempo] [pitchBend]
```

Only the first two arguments are required; the defaults for the rest are `C4 100 "" 0 1000 0 0 100 0 !120 AA`.
See the [UTAU parameter table](utau.md) for what each argument means.

```bash
./resampler render voice/_a.wav out.wav A4 100 "g-3" 30 500 60 -50 100 0 '!120' AA
```

Example output:

```
已写出 "out.wav"：87 帧 / 500.0ms / 后端 onnxruntime / F0 fcpe-onnx / 耗时 812.3ms
```

## batch

Batch rendering. Each line of the manifest file is one set of UTAU parameters; blank lines and lines starting with `#` are skipped.

```text
# list.txt
voice/_a.wav out/a.wav C4 100 "" 0 500 60 -50 100 0 !120 AA
voice/_i.wav out/i.wav D4 100 "" 0 500 60 -50 100 0 !120 AA
```

```bash
./resampler batch list.txt --jobs 4
```

`--jobs 0` means serial; the default is `0`. Output order is not guaranteed when running in parallel.

The whole batch shares a single engine instance: the ONNX models load only once, so lines no longer
re-load the 97MB model each. Because inference sessions are guarded by an internal mutex, the speedup from `--jobs`
mainly comes from parallel preprocessing and skipping repeated loading; inference itself stays serial — for automated
batch pipelines, keep `--jobs` at about half the number of physical cores; pushing it higher brings no extra benefit.

## info

Prints the version, build information, configuration fingerprint, and model status. Produces no audio.

```bash
./resampler info
```

```
neural-resampler 0.1.0
配置指纹: eb865efe4eba82b7
--- 模型 ---
声码器  : "models/pc_nsf_hifigan.onnx"  [已就绪]
FCPE    : "models/fcpe.onnx"  [已就绪]
--- 运行时 ---
声码器后端: onnxruntime
F0 后端    : fcpe-onnx
```

The **configuration fingerprint** is handy: it changes whenever the configuration changes, and the cache is invalidated with it.

## selftest

Generates a 440Hz test tone, runs the whole pipeline end to end, and checks that the output is non-empty. Use it to verify the installation.

```bash
./resampler selftest                    # 默认输出到 target/selftest/
./resampler selftest --out-dir /tmp/nr  # 指定目录
```

On success it prints `自检通过` and exits with code 0; if the output is empty it reports an error and exits.

## config

Exports the currently effective configuration as YAML so you can edit on top of it.

```bash
./resampler config my.yaml
```

## Exit Codes

`0` means success, non-`0` means failure. Failure messages go to stderr; raise `--log-level debug` to see
details such as model loading and backend selection.
