# クイックスタート

[インストール](installation.md)と[モデルの取得](models.md)が済んでいるものとします。

## ステップ 1：セルフテスト

```bash
./resampler selftest
```

このコマンドは 440Hz のテスト音を生成し、パイプライン全体をエンドツーエンドで
実行して、最後に次の内容を表示します：

```
已生成测试输入: "target/selftest/selftest_input.wav"
输出: "target/selftest/selftest_output.wav"（24696 样本 / 560.0ms / 峰值 0.254）
自检通过
```

`自检通过`（セルフテスト合格）と表示されればインストールは完了です。デフォルトの
出力先は `target/selftest/` で、`--out-dir` で変更できます：

```bash
./resampler selftest --out-dir /tmp/nr-test
```

## ステップ 2：バックエンドの状態を確認

```bash
./resampler info
```

最後の 2 行に注目してください：

```
声码器后端: onnxruntime
F0 后端    : fcpe-onnx
```

`stub` / `world-dio` と表示されている場合はモデルが見つかっていないので、
[モデルの取得](models.md)に戻ってください。

## ステップ 3：1 音レンダリングする

```bash
./resampler render voice/_a.wav out.wav A4
```

パスを 2 つ渡すだけでも実行でき、ほかの引数にはすべてデフォルト値
（A4、ベロシティ 100、長さ 1000ms）が入ります。全引数を指定すると：

```bash
./resampler render voice/_a.wav out.wav A4 100 "g-3" 30 500 60 -50 100 0 '!120' AA
```

出力は次のようになります：

```
已写出 "out.wav"：87 帧 / 500.0ms / 后端 onnxruntime / F0 fcpe-onnx / 耗时 812.3ms
```

## ステップ 4：エディタで使う

バイナリ自体がリサンプラーなので、UTAU プロトコルに従って直接呼び出せます：

```bash
resampler in.wav out.wav C4 100 "" 0 500 60 -50 100 0 !120 AA
```

エディタへの組み込み手順は[UTAU で使う](utau.md)または
[OpenUtau で使う](openutau.md)を参照してください。

## 一括レンダリング

複数組の UTAU パラメータを 1 つのファイルに 1 行ずつ書き込みます：

```text
# list.txt
voice/_a.wav out/a.wav C4 100 "" 0 500 60 -50 100 0 !120 AA
voice/_i.wav out/i.wav D4 100 "" 0 500 60 -50 100 0 !120 AA
```

```bash
./resampler batch list.txt --jobs 4
```

`#` で始まる行は無視されます。`--jobs 0` で並列度を自動決定します。

## 設定を変更する

デフォルト設定を書き出し、必要に応じて編集してから `--config` で指定します：

```bash
./resampler config my.yaml
# 编辑 my.yaml
./resampler --config my.yaml render voice/_a.wav out.wav A4
```

詳細な設定項目は[設定ファイル](configuration.md)を参照してください。
