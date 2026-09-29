# 命令行参考

## 两种调用形式

**1）UTAU 协议（无子命令）** —— 宿主编辑器使用，参数顺序固定：

```bash
resampler in.wav out.wav C4 100 "" 0 500 60 -50 100 0 !120 AA
```

**2）显式子命令** —— 手工使用：

```bash
resampler render  in.wav out.wav A4
resampler batch   list.txt --jobs 4
resampler info
resampler selftest
resampler config  my.yaml
```

程序会先判断参数是否像 UTAU 调用（至少 4 个位置参数且第 1 个不是子命令名），
是则走协议路径，否则按子命令解析。

## 全局选项

可放在子命令**之前**（也可跟在后面，均为 `global`）：

| 选项 | 说明 |
| --- | --- |
| `--config <FILE>` | 指定配置文件 |
| `--models <DIR>` | 覆盖配置中的模型目录 |
| `--log-level <LEVEL>` | `error`/`warn`/`info`/`debug`/`trace`，默认 `info` |
| `--log-format <FORMAT>` | `text` 或 `json`，默认 `text` |

全局选项也能出现在裸 UTAU 调用之前，便于在不改宿主调用方式的前提下指定模型：

```bash
resampler --models /opt/nr/models in.wav out.wav C4 100 "" 0 500 60 -50 100 0 !120 AA
```

## render

按 UTAU 协议渲染单个音符。

```bash
resampler render <IN> <OUT> [pitch] [velocity] [flags] [offset] [length]
                 [consonant] [cutoff] [volume] [modulation] [tempo] [pitchBend]
```

只有前两个参数是必需的，其余缺省值：`C4 100 "" 0 1000 0 0 100 0 !120 AA`。
参数含义见 [UTAU 参数表](utau.md)。

```bash
./resampler render voice/_a.wav out.wav A4 100 "g-3" 30 500 60 -50 100 0 '!120' AA
```

输出示例：

```
已写出 "out.wav"：87 帧 / 500.0ms / 后端 onnxruntime / F0 fcpe-onnx / 耗时 812.3ms
```

## batch

批量渲染。清单文件每行一组 UTAU 参数，空行与 `#` 开头的行会被跳过。

```text
# list.txt
voice/_a.wav out/a.wav C4 100 "" 0 500 60 -50 100 0 !120 AA
voice/_i.wav out/i.wav D4 100 "" 0 500 60 -50 100 0 !120 AA
```

```bash
./resampler batch list.txt --jobs 4
```

`--jobs 0` 表示串行，默认 `0`。并行时输出顺序不保证。

整个批量过程共享单个引擎实例：ONNX 模型只加载一次，多行渲染不再各自
重复载入 97MB 模型。由于推理会话内部以互斥锁保护，`--jobs` 的加速主要
来自预处理并行与省去重复加载，推理部分仍为串行——排版机场景建议保持
`--jobs` 等于物理核数的一半左右即可，盲目调大无额外收益。

## info

打印版本、构建信息、配置指纹与模型状态，不产生音频。

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

**配置指纹**很有用：改了配置它会变，缓存也会随之失效。

## selftest

生成一段 440Hz 测试音，端到端跑一遍管线并校验输出非空。用于验证安装是否正常。

```bash
./resampler selftest                    # 默认输出到 target/selftest/
./resampler selftest --out-dir /tmp/nr  # 指定目录
```

成功会打印 `自检通过` 并以退出码 0 结束；输出为空则报错退出。

## config

导出当前生效的配置为 YAML，便于在其基础上修改。

```bash
./resampler config my.yaml
```

## 退出码

`0` 成功，非 `0` 失败。失败信息会打到 stderr；调高 `--log-level debug` 可以看到
模型加载、后端选择等细节。
