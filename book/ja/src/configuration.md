# 設定ファイル

デフォルト設定はコンパイル時にバイナリへ組み込まれています。変更したい場合はまず書き出してください：

```bash
./resampler config my.yaml
./resampler --config my.yaml render voice/_a.wav out.wav A4
```

> ⚠️ `mel.*` と `f0.*` の数値は**モデルに紐づいています**。異なる仕様の ONNX モデルに差し替えるときは、
> これらのパラメータを必ず同時に変更してください。そうしないと出力がこもったり、歪んだり、まったく鳴らなくなったりします。デフォルトのモデルを使うなら触らないでください。

## models_dir

モデルのルートディレクトリです。設定内の各 `model` フィールドはここからの相対パスなので、**`models/` を二重に書かないでください**。

```yaml
models_dir: "models"     # ✅ 则 vocoder.model 写 "pc_nsf_hifigan.onnx"
```

## mel —— スペクトル分析

| 項目 | デフォルト | 説明 |
| --- | --- | --- |
| `sample_rate` | 44100 | 分析サンプルレート |
| `n_fft` | 2048 | FFT 点数 |
| `win_size` | 2048 | 窓長 |
| `hop_size` | 512 | レンダリングのフレームシフト |
| `origin_hop_size` | 128 | 分析のフレームシフト（より細かく、時間領域補間に使用） |
| `n_mels` | 128 | Mel bin 数 |
| `fmin` / `fmax` | 40 / 16000 | 周波数範囲（Hz） |
| `clip_val` | `1e-9` | 対数圧縮の下限 |
| `magnitude_eps` | 0.0 | 振幅フロア。`0` は単純な `abs()` |
| `mel_scale` | `slaney` | スケール：`slaney`（librosa のデフォルト）/ `htk` |

## f0 —— ピッチ抽出

| 項目 | デフォルト | 説明 |
| --- | --- | --- |
| `backend` | `auto` | `auto` / `fcpe` / `world` / `none` |
| `mode` | `hybrid` | `score` / `source` / `hybrid`。[F0 モード](f0-modes.md) を参照 |
| `model` | `fcpe.onnx` | FCPE モデルのファイル名 |
| `sample_rate` | 16000 | FCPE の入力サンプルレート |
| `hop_size` / `win_size` / `n_fft` | 160 / 1024 / 1024 | FCPE のスペクトルパラメータ |
| `mel_bins` | 128 | FCPE の Mel bin 数 |
| `fmin` / `fmax` | 0 / 8000 | FCPE の周波数範囲 |
| `f0_min` / `f0_max` | 80 / 880 | 有効な F0 範囲（Hz）。範囲外は無効とみなされます |
| `uv_threshold` | 0.006 | 信頼度がこの値未満なら無声と判定 |
| `uv_mask` | true | 無声フレームの F0 をゼロにする |
| `max_deviation_cents` | 100.0 | hybrid モードで許容される自然なずれの最大値 |
| `smoothing_ms` | 40.0 | 基準カーブの平滑化ウィンドウ |
| `decoder` | `local_argmax` | latent→cent のデコード：`local_argmax` / `argmax` |
| `local_argmax_width` | 4 | 片側のウィンドウ幅（合計ウィンドウは 2×width+1） |
| `cent_f0_min` / `cent_f0_max` | 32.70 / 1975.5 | cent テーブルの周波数範囲（C1～B6） |
| `clip_val` | `1e-5` | F0 分析の対数圧縮下限 |

## vocoder —— ボコーダ

| 項目 | デフォルト | 説明 |
| --- | --- | --- |
| `backend` | `ort` | 推論バックエンド |
| `model` | `pc_nsf_hifigan.onnx` | ボコーダモデル |
| `hnsep_model` | `hnsep.onnx` | オプション。調波/非調波分離 |
| `mel_input` / `f0_input` | `mel` / `f0` | 入力ノード名 |
| `output` | `audio` | 出力ノード名 |
| `mel_layout` | `auto` | `auto` / `channels_first` / `frames_first` |
| `providers` | `[cpu]` | 順に試行：`cpu` / `directml` / `cuda` / `coreml` |

`mel_layout: auto` はモデルのメタデータを読み、入力が `[1,n_mels,T]` か `[1,T,n_mels]` かを自動判定します。
モデルが誤判定されて出力がおかしい場合は、明示的に指定してください。

## processing —— 処理

| 項目 | デフォルト | 説明 |
| --- | --- | --- |
| `loop_mode` | true | 長い音符のとき、母音区間を反射ループでつなぐ |
| `fill` | 6 | 前後に余分に残すフレーム数 |
| `trim_silence` | false | 分析前に無音を切り取る |
| `silence_threshold_db` | -52.0 | 無音判定のしきい値 |
| `gender` | 0 | デフォルトのジェンダーオフセット（`g` フラグに対応） |

## output —— 出力

| 項目 | デフォルト | 説明 |
| --- | --- | --- |
| `sample_rate` | 44100 | 出力サンプルレート |
| `bit_depth` | 16 | `16`/`24`/`32` は整数 PCM、`0` は 32 ビット浮動小数点 |
| `peak_limit` | 1.0 | ピーク上限 |
| `wave_norm` | true | ラウドネス正規化を有効化 |
| `loudness_target` | -16.0 | 目標ラウドネス |
| `loudness_block_ms` | 400.0 | ラウドネス測定のブロックサイズ |
| `fade_in_ms` / `fade_out_ms` | 0.0 / 0.0 | フェードイン/アウトの長さ |

## cache —— 特徴量キャッシュ

| 項目 | デフォルト | 説明 |
| --- | --- | --- |
| `enabled` | true | キャッシュを有効化 |
| `extension` | `nrc` | キャッシュファイルの拡張子 |
| `zstd_level` | 3 | 圧縮レベル |
| `dir` | null | キャッシュディレクトリ。`null` は入力と同じディレクトリ |

キャッシュキーにはソースファイルのサイズ、更新時刻、設定フィンガープリントが含まれ、**設定を変更すると自動で無効化されます**。手動でのクリーンアップは不要です。

## runtime —— ランタイム

| 項目 | デフォルト | 説明 |
| --- | --- | --- |
| `log_level` | `info` | ログレベル |
| `log_format` | `text` | `text` / `json` |
| `intra_op_threads` | 0 | 演算子内スレッド数。`0` は自動 |
| `inter_op_threads` | 0 | 演算子間スレッド数。`0` は自動 |
