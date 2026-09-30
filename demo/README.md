# 示例渲染 / Example Renders / レンダリング例

---

## 简体中文

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
bash scripts/download_models.sh            # 下载 ONNX 到 models/（Windows 用 scripts/download_models.ps1）
./target/release/resampler demo/input_a3.wav /tmp/c4.wav C4 100 "" 0 800 0 -30 100 0 '!120' AA
```

`input_a3.nrc` 是 F0/Mel 特征缓存，删掉会自动重建。

---

## English

Results produced from the synthesized test tone bundled with the repository
(`input_a3.wav`, 220 Hz saw wave with a slight vibrato), for quickly verifying
that the engine works as expected.

| File | Description | Expected |
| --- | --- | --- |
| `input_a3.wav` | Test input (220 Hz / A3) | 1.2 s |
| `render_c4.wav` | Rendered to C4 (261.63 Hz), requested length 800 ms | 0.800 s, F0 ≈ 261.6 Hz |
| `render_f4.wav` | Rendered to F4 (349.23 Hz), requested length 800 ms | 0.800 s, F0 ≈ 349.2 Hz |
| `render_glide.wav` | A3 + pitchBend −400→+400 cents, 1 s | 1.000 s, F0 175 Hz → 277 Hz |

To reproduce:

```bash
cargo build --release
bash scripts/download_models.sh            # Downloads ONNX models into models/ (Windows: scripts/download_models.ps1)
./target/release/resampler demo/input_a3.wav /tmp/c4.wav C4 100 "" 0 800 0 -30 100 0 '!120' AA
```

`input_a3.nrc` is an F0/Mel feature cache; it is rebuilt automatically if deleted.

---

## 日本語

リポジトリ同梱の合成テスト音（`input_a3.wav`、220Hz ノコギリ波 + 軽いビブラート）を
使ったレンダリング結果です。エンジンが期待どおりに動作するかの簡単な確認に使えます。

| ファイル | 説明 | 期待値 |
| --- | --- | --- |
| `input_a3.wav` | テスト入力（220Hz / A3） | 1.2s |
| `render_c4.wav` | C4（261.63Hz）へレンダリング、要求長 800ms | 0.800s、F0 ≈ 261.6Hz |
| `render_f4.wav` | F4（349.23Hz）へレンダリング、要求長 800ms | 0.800s、F0 ≈ 349.2Hz |
| `render_glide.wav` | A3 + pitchBend −400→+400 セント、1s | 1.000s、F0 175Hz → 277Hz |

再現方法：

```bash
cargo build --release
bash scripts/download_models.sh            # ONNX モデルを models/ にダウンロード（Windows は scripts/download_models.ps1）
./target/release/resampler demo/input_a3.wav /tmp/c4.wav C4 100 "" 0 800 0 -30 100 0 '!120' AA
```

`input_a3.nrc` は F0/Mel 特徴量のキャッシュです。削除すると自動的に再構築されます。
