# モデルの入手

エンジンが実用レベルの音質を出すには ONNX モデルが必要です。**モデルはリポジトリやリリース成果物には同梱されていません**——合計で約 100 MB になり、
それぞれの出所のライセンスが再配布を許可しているとは限らないため、各自で入手してください。

## 配置場所

モデルは実行ファイルと**同じ階層または指定した** `models/` ディレクトリに置きます：

| ファイル | 役割 | 必須 |
| --- | --- | --- |
| `pc_nsf_hifigan.onnx` | ボコーダ：Mel + F0 → 波形 | **はい** |
| `fcpe.onnx` | F0 抽出（16kHz） | いいえ、欠落時は内蔵 DSP にフォールバック |
| `hnsep.onnx` | HN-SEP の調波/非調波分離 | いいえ |

`--models <DIR>` で一時的に別のディレクトリを指定できます：

```bash
./resampler --models /path/to/models info
```

## ワンコマンドでのダウンロード

リリース成果物にはダウンロードスクリプトが同梱されています（bash と PowerShell の 2 種類、処理内容は同じ）：

```bash
./download_models.sh      # Linux / macOS / Git Bash
```

```powershell
powershell -ExecutionPolicy Bypass -File .\download_models.ps1    # Windows PowerShell
```

リポジトリ内にもあります（`scripts/download_models.sh` と `scripts/download_models.ps1`）。
スクリプトは自動リトライに対応し、ダウンロード後に **SHA-256 検証**を行います
（基準は v0.1.0 とともに検証済みのバージョン）。ファイルが破損している、または上流で差し替えられていた場合はインストールを拒否して通知します。

### ダウンロード元とミラー高速化

スクリプトはデフォルトで GitHub に直接接続し（`raw.githubusercontent.com`）、gh-proxy ミラーによる高速化を内蔵しています：

1. **自動検出**：GitHub への直接接続が不通な場合、自動的にミラーへ切り替えて通知します
2. **低速時の切り替え**：平均ダウンロード速度が 10 秒間 100KB/s 未満で、現在 direct 接続のときは
   ミラーへ切り替えるか尋ねます（`NR_MODEL_AUTO_SWITCH=1` なら尋ねずに自動切り替えします）；
   direct 接続のリトライを使い切った後も、最後の 1 回は自動的にミラーを使います
3. **ミラーの強制**：`NR_MODEL_MIRROR` を設定すると、指定したミラープレフィックスを直接使います
   （[gh-proxy](https://github.com/hunshcn/gh-proxy) 形式：プレフィックス + 完全な元 URL）：

   ```bash
   NR_MODEL_MIRROR=https://gh-proxy.com/ ./download_models.sh
   ```

   自前デプロイや他の gh-proxy インスタンスに差し替えることもできます。

> 上流のモデル更新で検証が通らなくなった場合は、新しいバージョンが使えることを確認したうえで
> `NR_MODEL_SKIP_CHECKSUM=1` を設定して一時的に検証をスキップできます——**出所が信頼できることを確認してから行ってください**
> （ミラーの内容が改ざんされていても検証で捕捉されます）。

## 手動での入手

スクリプトが使えない場合は手動でダウンロードし、上の表のファイル名にリネームして `models/` に置きます：

1. **FCPE** —— [CNChTu/FCPE](https://github.com/CNChTu/FCPE) のリリースページから `fcpe.onnx` を取得
2. **PC-NSF-HiFiGAN** —— ONNX 形式が必要です。手元に `.ckpt` 重みしかない場合は、
   リポジトリ内の変換スクリプトを使えます：
   ```bash
   python scripts/convert_vocoder_to_onnx.py --ckpt model.ckpt --out models/pc_nsf_hifigan.onnx
   ```

## モデルの動作確認

```bash
./resampler info
```

次の 3 行に注目してください：

```
声码器  : "models/pc_nsf_hifigan.onnx"  [已就绪]
FCPE    : "models/fcpe.onnx"  [已就绪]
HN-SEP  : "models/hnsep.onnx"  [缺失]
```

* ボコーダ `[缺失]` → Stub バックエンドにフォールバックし、**音は出ますが音質は実用になりません**
* FCPE `[缺失]` → 内蔵 DSP による F0 抽出にフォールバックします。通常どおり動作しますが、歌声用途ではやや不安定になります
* HN-SEP `[缺失]` → 正常です。オプションの拡張機能です

## ライセンス

各モデルの出所であるリポジトリ/リリースページのライセンス条件に従ってください。モデルを独自の配布物に同梱する前に、
必ずライセンスで許可されているか確認してください。
