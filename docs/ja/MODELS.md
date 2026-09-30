# モデルについて

エンジンには 2 つの ONNX モデルが必要で、どちらも `models/` に置きます（`--models <DIR>` または設定 `models_dir` で指定可能）。

| ファイル | 役割 | 入手元 |
| --- | --- | --- |
| `pc_nsf_hifigan.onnx` | ボコーダ：Mel + F0 → 波形 | [openvpi/vocoders](https://github.com/openvpi/vocoders)（ONNX へのエクスポートが必要）または既成のエクスポート済みファイル |
| `fcpe.onnx` | F0 抽出（16kHz 入力） | [CNChTu/FCPE](https://github.com/CNChTu/FCPE) |
| `hnsep.onnx`（任意） | HN-SEP による高調波/ノイズ分離。息声やノイズ区間を改善 | 任意。欠落時はスキップ |

## 自動ダウンロード

```bash
bash scripts/download_models.sh          # 下载到 ./models
bash scripts/download_models.sh /path/to/dir
powershell -ExecutionPolicy Bypass -File scripts/download_models.ps1   # Windows
```

スクリプトはダウンロードと検証のみを行い、変換は行いません。GitHub への直接接続が
到達不能な場合や速度が 100KB/s 未満の場合は、自動で gh-proxy ミラーに切り替わります
（`NR_MODEL_MIRROR=<ミラーの接頭辞>` で強制指定可能）。

## 手動での準備

### ボコーダ

[openvpi/vocoders](https://github.com/openvpi/vocoders/releases) から
`pc-nsf-hifigan-44.1k-hop512-128bin-*` のリリースパッケージをダウンロードし、
`.ckpt` を取得したら、`scripts/convert_vocoder_to_onnx.py`（`torch` + `onnx` が
必要）でエクスポートします：

```bash
python3 scripts/convert_vocoder_to_onnx.py \
    --ckpt pc_nsf_hifigan_44.1k_hop512_128bin_2025.02/model.ckpt \
    --out models/pc_nsf_hifigan.onnx
```

エクスポート要件（`config/resampler.yaml` の `mel.*` と一致させる必要があります）：

| パラメータ | 値 |
| --- | --- |
| sampling_rate | 44100 |
| num_mels | 128 |
| n_fft / win_size | 2048 |
| hop_size | 512 |
| fmin / fmax | 40 / 16000 |
| mini_nsf | true |

### F0 モデル

FCPE の公式リポジトリは PyTorch 重みを提供しており、コミュニティにはエクスポート
済みの `fcpe.onnx` もあります。自分でエクスポートする場合、入力は 16kHz の
**メルスペクトログラム**（`[1, T, 128]`）、出力は F0（Hz）です。

## ノード名

デフォルト設定の想定は次のとおりです：

```
声码器输入： mel       [1, T, 128]   float32
            f0        [1, T]        float32
声码器输出： waveform  [1, N]        float32
```

エクスポートしたモデルのノード名が異なる場合は、`vocoder.mel_input / f0_input / output`
を変更すればよく、コードの変更は不要です。

`resampler info` で、モデルが揃っているか・読み込めるかを確認できます。

## モデルが欠落している場合の動作

| 欠落 | 動作 |
| --- | --- |
| ボコーダ | `backend::stub` にフォールバック（Mel による簡易スペクトル再構成）。**音質は実用レベルではない**。パイプラインがつながることだけを保証 |
| FCPE | 内蔵 DSP F0（`f0.backend: world`）にフォールバックし、さらに失敗した場合はスコアのみの F0 へ |
| HN-SEP | 該当する処理ステップを単純にスキップ |

## 検証

```bash
resampler info                      # 列出模型是否存在 + 大小 + sha256 前 16 位
resampler selftest --out-dir /tmp/nr
```

CI ではモデルが存在しない場合、ONNX 関連のアサーションは自動的にスキップされます
（`.github/workflows/ci.yml` を参照）。
