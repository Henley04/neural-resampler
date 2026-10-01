# UTAU / OpenUtau 連携

## プロトコル

バイナリそのものがリサンプラーです。UTAU の 13 引数プロトコルに従ってそのまま呼び出します：

```
resampler <in.wav> <out.wav> <pitch> <velocity> <flags> <offset> <length_req>
          <consonant> <cutoff> <volume> <modulation> <tempo> <pitchBend>
```

| 位置 | 引数 | 単位 | 説明 |
| --- | --- | --- | --- |
| 1 | in.wav | — | 音源（ボイスバンク）のサンプル |
| 2 | out.wav | — | レンダリング結果 |
| 3 | pitch | 音名 | 例：`C4`、`A#3` |
| 4 | velocity | 0–200 | 強さ |
| 5 | flags | — | 下記参照 |
| 6 | offset | ms | 左側の空白 |
| 7 | length_req | ms | 要求長 |
| 8 | consonant | ms | 子音部（ストレッチしない） |
| 9 | cutoff | ms | 右側の空白。通常は負の値 |
| 10 | volume | % | 音量 |
| 11 | modulation | % | モジュレーション |
| 12 | tempo | `!BPM` または BPM | テンポ |
| 13 | pitchBend | Base64+RLE | ピッチ曲線。単位は cent |

`cutoff` は通常負の値（例：`-50`）です。CLI では `allow_negative_numbers` が
有効になっているため、そのまま渡せます。

## OpenUtau での設定

`resampler`（Windows では `resampler.exe`）を OpenUtau の `Resamplers`
ディレクトリ（Windows はプログラムディレクトリ内、Linux は
`~/.local/share/OpenUtau/Resamplers`。または OpenUtau ウィンドウにドラッグして
「Install as resampler」を選択）に置き、レンダラーを `CLASSIC` に切り替えてから、
横の ⚙ 歯車アイコンでそれを選択します。音源の `oto.ini` と WAV
は一切変更する必要がありません。
[OpenUtau wiki: Resamplers and Wavtools](https://github.com/openutau/OpenUtau/wiki/Resamplers-and-Wavtools) を参照してください。

## モデルディレクトリの解決

OpenUtau がリサンプラーをインストールすると、実行ファイルを `Resamplers/`
へ**コピー**します。レンダリング時の作業ディレクトリに `models/` はないため、
エンジンは静かに劣化します（音は出ますが音質は実用になりません）。モデル
ディレクトリは次の順で解決されます：コマンドラインの `--models` > 環境変数
`NR_MODELS_DIR` > 実行ファイルと同じ階層の `models/`（つまり
`Resamplers/models/`）> 作業ディレクトリの `models/`。OpenUtau では
`NR_MODELS_DIR` を設定するか、モデルを `Resamplers/models/` に置き、
`resampler info` でボコーダのバックエンドが `onnxruntime` であることを
確認してください。

## サポートされるフラグ

| フラグ | 意味 | 実装 |
| --- | --- | --- |
| `g±N` | ジェンダー/フォルマント偏移（0.01 半音） | 分析段階で窓長をスケーリング（key shift） |
| `P±N` | ラウドネス正規化の強さ（%） | 元の結果と正規化結果の間で補間 |
| `t/N` | ビブラート速度 | `pitchBend` に重畳 |
| `A/B/G/S/p/R/D/C/Z` | HiFiSampler 互換フラグ | パースするが強制適用はしない |

未知のフラグは map にパースされますが無視され、レンダリングが失敗することは
ありません。

## pitchBend のデコード

1. `#` で分割：`<b64>#<rle>#<b64>#<rle>#...`
2. Base64 2 文字ごとに 12 ビット符号付き整数（`-2048..2047`）を 1 つ復元
3. RLE セグメントは直前の値を繰り返す
4. 末尾に 0 を 1 つ補う（元の実装と同じ）

単位は **cent** で、値の範囲は ±2048（約 ±20 半音）です。サンプリンググリッドは
`60 / (tempo × 96)` 秒/ポイントで、曲線範囲外の時点は端点の値でクランプされます。

## oto.ini

エンジンは同じディレクトリの `oto.ini` を読み、フラグ/オフセットなどのデフォルト値を
取得します（コマンドライン引数が優先）。エンコーディングは UTF-8 → Shift-JIS → GBK
の順で試行し、日本語と簡体字中国語の音源に対応しています。

## バッチレンダリング

各パラメータセットを 1 行に 1 つずつ書き、`batch` に渡して並列処理します：

```bash
resampler batch list.txt --jobs 4
```

各 worker は独立した ONNX セッションを保持し（`Session` は `Sync` ではない）、
モデルの再読み込みは行われません。
