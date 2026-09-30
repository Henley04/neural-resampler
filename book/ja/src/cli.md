# コマンドラインリファレンス

## 2 つの呼び出し形式

**1）UTAU プロトコル（サブコマンドなし）** —— ホストエディタから使用します。引数の順序は固定：

```bash
resampler in.wav out.wav C4 100 "" 0 500 60 -50 100 0 !120 AA
```

**2）明示的サブコマンド** —— 手動で使用する場合：

```bash
resampler render  in.wav out.wav A4
resampler batch   list.txt --jobs 4
resampler info
resampler selftest
resampler config  my.yaml
```

プログラムはまず、引数が UTAU 呼び出しらしく見えるか（位置引数が 4 つ以上で、第 1 引数がサブコマンド名でない）を判定します。
そうであればプロトコルパスを通し、そうでなければサブコマンドとして解析します。

## グローバルオプション

サブコマンドの**前**に置けます（後ろに置いても可。いずれも `global` です）：

| オプション | 説明 |
| --- | --- |
| `--config <FILE>` | 設定ファイルを指定 |
| `--models <DIR>` | 設定内のモデルディレクトリを上書き |
| `--log-level <LEVEL>` | `error`/`warn`/`info`/`debug`/`trace`、デフォルト `info` |
| `--log-format <FORMAT>` | `text` または `json`、デフォルト `text` |

グローバルオプションは素の UTAU 呼び出しの前にも置けます。ホスト側の呼び出し方を変えずにモデルを指定できます：

```bash
resampler --models /opt/nr/models in.wav out.wav C4 100 "" 0 500 60 -50 100 0 !120 AA
```

## render

UTAU プロトコルに従って 1 音をレンダリングします。

```bash
resampler render <IN> <OUT> [pitch] [velocity] [flags] [offset] [length]
                 [consonant] [cutoff] [volume] [modulation] [tempo] [pitchBend]
```

必須は最初の 2 引数のみで、残りの既定値は `C4 100 "" 0 1000 0 0 100 0 !120 AA` です。
引数の意味は [UTAU パラメータ表](utau.md) を参照してください。

```bash
./resampler render voice/_a.wav out.wav A4 100 "g-3" 30 500 60 -50 100 0 '!120' AA
```

出力例：

```
已写出 "out.wav"：87 帧 / 500.0ms / 后端 onnxruntime / F0 fcpe-onnx / 耗时 812.3ms
```

## batch

一括レンダリング。リストファイルの各行が 1 組の UTAU パラメータで、空行と `#` で始まる行はスキップされます。

```text
# list.txt
voice/_a.wav out/a.wav C4 100 "" 0 500 60 -50 100 0 !120 AA
voice/_i.wav out/i.wav D4 100 "" 0 500 60 -50 100 0 !120 AA
```

```bash
./resampler batch list.txt --jobs 4
```

`--jobs 0` は直列を意味し、デフォルトは `0` です。並列時の出力順序は保証されません。

バッチ全体で 1 つのエンジンインスタンスを共有します：ONNX モデルの読み込みは 1 回だけで、行ごとに 97MB のモデルを
再読み込みすることはありません。推論セッションは内部のミューテックスで保護されているため、`--jobs` による高速化の多くは
前処理の並列化と再読み込みの省略から得られるもので、推論部分は直列のままです——自動バッチ処理のような場面では
`--jobs` を物理コア数の半分程度にしておけばよく、無闇に増やしても追加の効果はありません。

## info

バージョン、ビルド情報、設定フィンガープリント、モデルの状態を出力します。音声は生成しません。

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

**設定フィンガープリント**は有用です：設定を変更すると値が変わり、キャッシュもそれに伴って無効化されます。

## selftest

440Hz のテスト音を生成し、パイプラインをエンドツーエンドで実行して出力が空でないことを検証します。インストールが正常かの確認に使います。

```bash
./resampler selftest                    # 默认输出到 target/selftest/
./resampler selftest --out-dir /tmp/nr  # 指定目录
```

成功すると `自检通过` と表示して終了コード 0 で終了します。出力が空の場合はエラーを出して終了します。

## config

現在有効な設定を YAML としてエクスポートします。それをベースに編集するのに便利です。

```bash
./resampler config my.yaml
```

## 終了コード

`0` は成功、`0` 以外は失敗です。失敗メッセージは stderr に出力されます。`--log-level debug` を上げると、
モデル読み込みやバックエンド選択などの詳細が見られます。
