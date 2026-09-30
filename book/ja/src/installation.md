# インストール

## 方法 1：ビルド済みバイナリのダウンロード（推奨）

[Releases ページ](https://github.com/Henley04/neural-resampler/releases) から、
お使いのプラットフォーム向けのアーカイブをダウンロードしてください。

| プラットフォーム | ファイル |
| --- | --- |
| Linux x86_64 | `neural-resampler-<ver>-linux-x86_64.tar.gz` |
| macOS Apple Silicon | `neural-resampler-<ver>-macos-aarch64.tar.gz` |
| Windows x86_64 | `neural-resampler-<ver>-windows-x86_64.zip` |

ビルド済みバイナリがまだ提供されていないプラットフォーム（macOS Intel、
Linux ARM64 など）では、ご自身で `cargo build --release` を実行してください。
成果物は `target/release/resampler` に生成されます。

各アーカイブには同名の `.sha256` チェックサムファイルが付属しており、
ダウンロード後に確認できます：

```bash
shasum -a 256 -c neural-resampler-0.1.0-linux-x86_64.tar.gz.sha256
# Windows (PowerShell):
# (Get-FileHash -Algorithm SHA256 .\xxx.zip).Hash
```

展開後のディレクトリ構成：

```
neural-resampler-0.1.0-linux-x86_64/
├── resampler              # 主程序（Windows 为 resampler.exe）
├── config.resampler.yaml  # 默认配置，可直接改后用 --config 指定
├── download_models.sh     # 模型下载脚本（Linux/macOS）
├── download_models.ps1    # 模型下载脚本（Windows PowerShell）
├── models/                # 模型放置目录（初始为空）
├── docs/                  # 架构与模型说明
└── README.md LICENSE
```

> **アーカイブに ONNX モデルは含まれません。** モデルは約 100 MB あり、それぞれの
> 配布元ライセンスの制約を受けるため、リポジトリには同梱されていません。展開後に
> `./download_models.sh`（Windows は `.\download_models.ps1`）を実行して取得して
> ください。詳細は[モデルの取得](models.md)を参照してください。

## 方法 2：ソースからビルド

Rust の安定版ツールチェーン（1.70+）が必要です。初回ビルド時に ONNX Runtime の
ビルド済みライブラリが自動でダウンロードされます。

```bash
git clone https://github.com/Henley04/neural-resampler.git
cd neural-resampler
cargo build --release
# 产物：target/release/resampler
```

オプションの feature（GPU 加速）：

```bash
cargo build --release --features cuda      # NVIDIA CUDA
cargo build --release --features directml  # Windows DirectML
cargo build --release --features coreml    # macOS CoreML
```

> ⚠️ GPU 用 feature は**相互排他**です。複数を同時に有効化しないでください。また
> `--all-features` も使用しないでください。組み合わせた feature セットに一致する
> ビルド済みライブラリを ort が見つけられず、リンクに失敗します。

## 方法 3：独自の配布物をパッケージする

```bash
bash scripts/build_release.sh                 # 输出到 dist/
bash scripts/build_release.sh --features cuda
```

スクリプトはバイナリ・デフォルト設定・ドキュメントをパッケージ化し、ローカルに
モデルが存在する場合はそれも同梱します。

## インストールの確認

```bash
./resampler info        # 查看构建信息、配置指纹与模型状态
./resampler selftest    # 生成测试音并端到端跑一遍
```

`info` の出力で、ボコーダと FCPE の両方に `[已就绪]`（準備完了）と表示されて
初めて実際に使用可能です。`[缺失]`（欠落）と表示されている場合はエンジンが
フォールバックします。詳細は[トラブルシューティング](troubleshooting.md)を
参照してください。
