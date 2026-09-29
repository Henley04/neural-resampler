# 快速开始

假定你已经[安装](installation.md)好程序并[获取了模型](models.md)。

## 第一步：自检

```bash
./resampler selftest
```

这条命令会生成一段 440Hz 测试音，然后端到端跑一遍完整管线，最后打印：

```
已生成测试输入: "target/selftest/selftest_input.wav"
输出: "target/selftest/selftest_output.wav"（24696 样本 / 560.0ms / 峰值 0.254）
自检通过
```

看到 `自检通过` 就说明装好了。默认输出到 `target/selftest/`，可用 `--out-dir` 指定：

```bash
./resampler selftest --out-dir /tmp/nr-test
```

## 第二步：确认后端状态

```bash
./resampler info
```

重点看最后两行：

```
声码器后端: onnxruntime
F0 后端    : fcpe-onnx
```

如果显示的是 `stub` / `world-dio`，说明没找到模型，回到[获取模型](models.md)。

## 第三步：渲染一个音

```bash
./resampler render voice/_a.wav out.wav A4
```

只给两个路径也能跑，其余参数都有默认值（A4、力度 100、长度 1000ms）。
完整参数：

```bash
./resampler render voice/_a.wav out.wav A4 100 "g-3" 30 500 60 -50 100 0 '!120' AA
```

输出类似：

```
已写出 "out.wav"：87 帧 / 500.0ms / 后端 onnxruntime / F0 fcpe-onnx / 耗时 812.3ms
```

## 第四步：在编辑器里用起来

二进制本身就是 resampler，直接按 UTAU 协议调用：

```bash
resampler in.wav out.wav C4 100 "" 0 500 60 -50 100 0 !120 AA
```

接入编辑器的步骤见[在 UTAU 中使用](utau.md)或[在 OpenUtau 中使用](openutau.md)。

## 批量渲染

把多组 UTAU 参数写进一个文件，每行一组：

```text
# list.txt
voice/_a.wav out/a.wav C4 100 "" 0 500 60 -50 100 0 !120 AA
voice/_i.wav out/i.wav D4 100 "" 0 500 60 -50 100 0 !120 AA
```

```bash
./resampler batch list.txt --jobs 4
```

以 `#` 开头的行会被忽略。`--jobs 0` 表示自动决定并行度。

## 改配置

导出默认配置后按需修改，再用 `--config` 指定：

```bash
./resampler config my.yaml
# 编辑 my.yaml
./resampler --config my.yaml render voice/_a.wav out.wav A4
```

详细配置项见[配置文件](configuration.md)。
