[简体中文](README.md) | [English](README.en.md)

# neural-resampler

[![CI - Build & Test](https://github.com/Henley04/neural-resampler/actions/workflows/ci.yml/badge.svg)](https://github.com/Henley04/neural-resampler/actions/workflows/ci.yml)

汎用ニューラルリサンプラーエンジン —— **純 Rust + ONNX** で書き直した HiFiSampler。

> 📖 **使用ドキュメント**：<https://henley04.github.io/neural-resampler/>
> 📦 **ビルド済み成果物のダウンロード**：[Releases](https://github.com/Henley04/neural-resampler/releases)
> （Linux x86_64 / macOS Apple Silicon / Windows x86_64）

従来の音源（ボイスバンク）の WAV と `oto.ini` は**一切変更しません**。置き換えるのは、ピッチ変換・長さ調整・音色合成を担う resampler エンジンだけです：

* ボコーダは **PC-NSF-HiFiGAN** をそのまま採用（NSF アーキテクチャ、F0 条件付き。F0 を直接変更するだけで正確なピッチ変換が可能）；
* F0 抽出は WORLD（DIO/Harvest）から **FCPE（ONNX, 16kHz）** に変更し、歌声シーンでより安定；
* Python 依存なし、**シングルバイナリでデプロイ**；
* アダプタ層を介して **OpenUtau / ネイティブ UTAU** の両方に対応し、他のエディタへ拡張するためのインターフェースも保持しています。

```
┌──────────────────────────────────────────────────────────┐
│            ホストエディタ：OpenUtau / UTAU / その他      │
└──────────────────────┬───────────────────────────────────┘
                       │ コマンドラインプロトコル
┌──────────────────────▼───────────────────────────────────┐
│アダプタ層：UTAUパラメータ、pitchBend(Base64+RLE)、oto.ini│
└──────────────────────┬───────────────────────────────────┘
┌──────────────────────▼───────────────────────────────────┐
│  コアエンジン：Mel → 時間伸縮 → F0 → ボコーダ → 後処理   │
└──────────────────────┬───────────────────────────────────┘
┌──────────────────────▼───────────────────────────────────┐
│推論バックエンド：ort (CPU/DirectML/CUDA/CoreML, 交換可能)│
└──────────────────────────────────────────────────────────┘
```

## クイックスタート

```bash
# 1. 构建（首次构建会自动下载 ONNX Runtime 预编译库）
cargo build --release

# 2. 准备模型（见 models/README.md）
bash scripts/download_models.sh          # Windows: scripts/download_models.ps1
                                         # 或手动把 ONNX 放进 models/
# ダウンロード後、スクリプトは NR_MODELS_DIR をユーザー環境変数に自動設定します
# （NR_SKIP_ENV=1 でスキップ）。使用中は models ディレクトリを移動しないでください

# 3. 自检：生成测试音并跑通整条管线
./target/release/resampler selftest

# 4. 查看构建信息 / 配置 / 模型状态
./target/release/resampler info
```

## UTAU / OpenUtau の resampler として使う

バイナリ自体が resampler です。UTAU プロトコルに従って直接引数を渡します：

```bash
resampler in.wav out.wav C4 100 "" 0 500 60 -50 100 0 !120 AA
#        ↑输入  ↑输出   ↑音名 ↑力度 ↑flags ↑offset ↑length ↑consonant ↑cutoff ↑volume ↑modulation ↑tempo ↑pitchBend
```

OpenUtau の場合：バイナリを OpenUtau ディレクトリの `Resamplers` フォルダに置くか（または OpenUtau
ウィンドウに直接ドラッグして "Install as resampler" を選択）、レンダラを `CLASSIC` に切り替えてから横の
⚙ 歯車アイコンをクリックしてこの resampler を選択します。詳細は OpenUtau wiki の
[Resamplers and Wavtools](https://github.com/openutau/OpenUtau/wiki/Resamplers-and-Wavtools) を参照してください。

モデルがない場合、エンジンは自動でフォールバックし（ボコーダ → 内蔵 Stub、F0 → 内蔵 DSP）、パイプライン自体は通りますが、
**音質は実用にならず**、接続性の確認専用です。

## サブコマンド

| サブコマンド | 説明 |
| --- | --- |
| `render` | 名前付き引数で単一ノートをレンダリング（`--help` で全オプションを表示） |
| `batch` | バッチレンダリング。1 行に 1 組の UTAU パラメータ、`--jobs N` で並列実行 |
| `info` | バージョン、ビルド情報、設定フィンガープリント、モデル状態を出力 |
| `selftest` | テスト音を生成してパイプラインを通しで実行。スモーク検証用 |
| `config` | デフォルト設定を YAML でエクスポートし、二次修正を容易に |

グローバルオプション：`--config <FILE>`（設定ファイル）、`--models <DIR>`（モデルディレクトリ）、
`--log-level <LEVEL>`、`--log-format text|json`。

例：

```bash
resampler render --config my.yaml voice/_a.wav out.wav A4 100 "g-3" 30 500 60 -50 100 0 '!120' AA
resampler batch list.txt --jobs 4
resampler config resampler.yaml
```

## F0 生成モード

設定 `f0.mode` で F0 の生成方法を決定します（デフォルトは `hybrid`）：

| モード | 動作 | 適用場面 |
| --- | --- | --- |
| `score` | F0 は完全に楽譜から取得（音名 + pitchBend）。オリジナルの HiFiSampler と同一 | 楽譜に正確に合わせたい場合 |
| `source` | サンプル自身の F0 曲線を目標ピッチへ全体移調 | 元サンプルのビブラート/グライドを保持したい場合 |
| `hybrid` | 楽譜が絶対ピッチを示し、サンプル F0 が自然な偏差（`max_deviation_cents` でリミット）と有声/無声判定を提供 | デフォルト。正確さと自然さを両立 |

`f0.backend` が `auto` の場合の優先順位：**FCPE(ONNX) → 内蔵 DSP（DIO/Harvest 方式）→ 楽譜のみ**。

## 解析規約（リファレンス実装と値単位で照合済み）

Mel 抽出は学習前処理とビット単位で一致している必要があり、そうでないとボコーダ出力がこもったり歪んだりします。本プロジェクトが揃えた規約：

| 項目 | 値 |
| --- | --- |
| Mel スケール | **Slaney**（`librosa.filters.mel` のデフォルト、HTK ではない） |
| 正規化 | Slaney 面積正規化 `2 / (f_right - f_left)` |
| 窓関数 | 周期 Hann（`torch.hann_window`） |
| パディング | 反射パディング。左 `(win-hop)/2`、右 `(win-hop+1)/2` |
| STFT | `center=False`、フレーム数 `1 + (len - win) // hop` |
| 振幅 | ボコーダ側は `abs()`、FCPE 側は `sqrt(re²+im²+1e-9)` |
| 圧縮 | **自然対数** `log(clamp(x, min=clip))`、ボコーダは `1e-9`、FCPE は `1e-5` |

`tests/mel_reference.rs` が、Python リファレンス実装で生成した基準値によってこれらの規約を固定しています（有声 bin の誤差 < 0.06）。

FCPE の出力は **cent 分類 latent** `[1, T, 360]` で、公式の手順に従ってデコードする必要があります：
局所 argmax（±4 クラス）の加重平均 → `f0 = 10 · 2^(cent/1200)`。信頼度 ≤ 0.006 のフレームは無声と判定します。

## オリジナル HiFiSampler との違い

| 項目 | オリジナル HiFiSampler | 本プロジェクト |
| --- | --- | --- |
| F0 抽出 | WORLD (DIO/Harvest) | FCPE ONNX（内蔵 DSP へフォールバック可） |
| 言語 | Python + C# | 純 Rust |
| デプロイ | Python ランタイムが必要 | シングルバイナリ |
| ボコーダ | PC-NSF-HiFiGAN | PC-NSF-HiFiGAN（変更なし） |
| ピッチ制御 | F0 を変更 | F0 を変更（変更なし） |
| GPU | PyTorch CUDA | ort + DirectML / CUDA / CoreML（オプション feature） |
| キャッシュ | `.hifi.npz` | zstd 圧縮の `.nrc`（Organum の `.ogc` を参考） |

## プロジェクト構成

```
src/
├── main.rs                 CLI 入口（resampler）
├── lib.rs                  库入口 + C ABI（FFI）
├── adapters/utau.rs        UTAU / OpenUtau 适配层
├── core/
│   ├── audio.rs            WAV I/O、重采样、增益
│   ├── oto.rs              oto.ini 解析（UTF-8 / Shift-JIS）
│   ├── protocol.rs         UTAU 参数、pitchBend 解码、音名换算
│   ├── feature.rs          STFT / Mel 滤波器组 / 插值
│   ├── f0/{mod,fcpe,world}.rs  F0Extractor trait、FCPE ONNX、DSP 回退
│   ├── pipeline.rs         渲染管线
│   └── post_process.rs     响度归一化、限幅、growl
├── backend/{ort_backend,stub}.rs   推理后端
└── cache/f0_cache.rs       zstd 特征缓存
config/resampler.yaml       默认配置（编译期嵌入）
models/                     ONNX 模型（不随仓库分发）
tests/                      单元测试与端到端测试
```

## 設定

デフォルト設定はコンパイル時に埋め込まれています（`config/resampler.yaml`）。`--config` で上書きするか、
`resampler config out.yaml` でエクスポートしてから編集できます。主な項目：

* `mel.*`：44.1kHz / 2048 FFT / 分析フレームシフト 128 / レンダリング フレームシフト 512 / 128 mel bin / 40–16000Hz
* `f0.*`：16kHz / hop 160 / 1024 FFT / 128 mel bin / 0–8000Hz、F0 80–880Hz、cent テーブル 32.70–1975.5Hz
* `vocoder.*`：モデルパス、入出力ノード名、`mel_layout`（デフォルトはモデルメタデータから自動判定）、実行プロバイダの一覧
* `processing.*`：`loop_mode`（長音符のループ連結）、`fill`（フレーム余裕）、`gender`
* `output.*`：出力サンプルレート、ビット深度、ピーク上限、ラウドネス正規化
* `cache.*`：キャッシュの有効/無効、接尾辞、zstd レベル

## 配布物

```bash
bash scripts/build_release.sh                 # 默认 feature，输出 dist/<target>.tar.gz
bash scripts/build_release.sh --features cuda # 指定 feature
```

成果物は解凍するだけで使用でき、バイナリ、デフォルト設定、ドキュメント、`models/*.onnx`（ローカルにモデルが既にある場合）を含みます：

```
neural-resampler-x86_64-unknown-linux-gnu/
├── resampler                 # 单二进制
├── models/*.onnx             # 声码器 + FCPE
├── config.resampler.yaml     # 默认配置，可直接改
└── docs/ README.md LICENSE
```

モデルはリポジトリに同梱していません。CI や空の環境では `scripts/download_models.sh`
（Windows: `scripts/download_models.ps1`）で取得してください。スクリプトは GitHub への直接接続が到達不能、
または速度が 100KB/s 未満のときに自動で gh-proxy ミラーへフォールバックし（`NR_MODEL_MIRROR` で強制指定可）、
ダウンロード後に SHA-256 検証を行い、さらに `NR_MODELS_DIR` をユーザー環境変数に自動設定します——
以降、リサンプラーがエディタのフォルダ（OpenUtau の `Resamplers/` など）にコピーされてもモデルを
見つけられます（⚠ この方法使用中は models ディレクトリを移動しないでください。`NR_SKIP_ENV=1` で
自動設定をスキップ）。モデルが欠落している場合、レンダリングは明示的なエラーで終了します（三言語、
対処法つき）。`--allow-stub` / `NR_ALLOW_STUB=1` で許可できます（オフライン検証のみ）。

## テストと CI

```bash
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings      # 不要用 --all-features：GPU 提供者互斥
cargo test
```

GitHub Actions（`.github/workflows/ci.yml`）が Ubuntu / Windows / macOS 上でフォーマットチェック、Clippy、ビルド、
単体テスト、統合テスト、エンドツーエンドの selftest を実行し、（モデルが存在する場合は）ONNX モデルの検証も追加で行います。

## 検証済みの動作

`models/` に 2 つの ONNX を配置した状態での実測結果（入力は 220Hz の鋸歯波）：

| シナリオ | 結果 |
| --- | --- |
| `A3`（220Hz）、1s | 出力 1.000s、実測 F0 220.0 Hz |
| `B3`（246.94Hz） | 実測 F0 247.5 Hz（偏差 < 4 cent） |
| 要求長 500ms / 2000ms | 出力 0.500s / 2.000s、F0 は維持 |
| pitchBend −600→+600 cent | F0 軌跡は 165Hz → 294Hz と滑らかに追従 |
| モデルなし | Stub + 内蔵 DSP へ自動フォールバック、パイプラインは動作 |

## FFI

`lib.rs` は他言語からの統合を容易にするため、一連の C ABI を公開しています：

```c
const char* ver = nr_version();                 // 静态字符串，无需释放
char* out = nr_render_json("{\"input\":\"a.wav\", ...}");
if (!out) fprintf(stderr, "%s\n", nr_last_error());
nr_string_free(out);
```

`cargo build --release` は動的ライブラリも同時に生成します（`[lib] crate-type` に `cdylib` を含む）：

| プラットフォーム | 成果物 |
| --- | --- |
| Linux | `target/release/libneural_resampler.so` |
| macOS | `target/release/libneural_resampler.dylib` |
| Windows | `target/release/neural_resampler.dll` |

静的ライブラリが必要な場合は、`[lib]` の `crate-type` に `"staticlib"` を追加してください。
すべての FFI エントリポイントは `catch_unwind` でラップされており、panic が ABI 境界を越えることはありません。

## ライセンスと謝辞

MIT。参考・謝辞：[hifisampler](https://github.com/openhachimi/hifisampler)、
[straycat-rs](https://github.com/UtaUtaUtau/straycat-rs)、[Organum](https://github.com/KakouLabs/Organum)、
[pitch-core](https://github.com/gzivdo/pitch-core)、[ort](https://github.com/pykeio/ort)、
[FCPE](https://github.com/CNChTu/FCPE)、[DiffSinger](https://github.com/openvpi/DiffSinger)。

モデル自体のライセンスについては、各提供元リポジトリ/リリースページの要件に従ってください。
