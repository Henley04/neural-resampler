# 示例渲染

用仓库自带的合成测试音（`input_a3.wav`，220Hz 锯齿波 + 轻微颤音）跑出的结果，
用于快速确认引擎是否按预期工作。

| 文件 | 说明 | 期望 |
| --- | --- | --- |
| `input_a3.wav` | 测试输入（220Hz / A3） | 1.2s |
| `render_c4.wav` | 渲染到 C4（261.63Hz），要求长度 800ms | 0.800s，F0 ≈ 261.6Hz |
| `render_f4.wav` | 渲染到 F4（349.23Hz），要求长度 800ms | 0.800s，F0 ≈ 349.2Hz |
| `render_glide.wav` | A3 + pitchBend −400→+400 cent，1s | 1.000s，F0 175Hz → 277Hz |

复现：

```bash
cargo build --release
bash scripts/download_models.sh            # 下载 ONNX 到 models/
./target/release/resampler demo/input_a3.wav /tmp/c4.wav C4 100 "" 0 800 0 -30 100 0 '!120' AA
```

`input_a3.nrc` 是 F0/Mel 特征缓存，删掉会自动重建。
