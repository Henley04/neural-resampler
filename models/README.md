# 模型目录 / Model Directory / モデルディレクトリ

---

## 简体中文

本目录放置推理所需的 ONNX 模型，**不随仓库分发**（体积大且受各自许可约束）。

| 文件 | 作用 | 必需 |
| --- | --- | --- |
| `pc_nsf_hifigan.onnx` | 声码器：Mel + F0 → 波形 | 是 |
| `fcpe.onnx` | F0 提取（16kHz） | 否（缺失时回退到内置 DSP） |
| `hnsep.onnx` | HN-SEP 谐波/噪声分离 | 否 |

获取方式（自动在 GitHub 直连不可达/过慢时切换 gh-proxy 镜像，下载后做 SHA-256 校验）：

```bash
bash scripts/download_models.sh          # Linux / macOS / Git Bash
powershell -ExecutionPolicy Bypass -File scripts/download_models.ps1   # Windows
```

模型不存在时：`selftest` / `info` 仍会降级运行（声码器 → Stub、F0 → 内置 DSP）
用于验证连通性；但 `render` / `batch` / UTAU 协议渲染会**直接报错退出**（三语提示，
含解决方法），不会输出音质不可用的音频。离线自测确需降级运行时，加 `--allow-stub`
或设 `NR_ALLOW_STUB=1`。**实际使用必须安装真实模型。**

模型目录解析顺序：`--models` 参数 > 环境变量 `NR_MODELS_DIR` > 可执行文件同级
`models/`（覆盖 OpenUtau 把 exe 复制进 `Resamplers/` 的场景）> 工作目录
`models/`。

> **推荐**：运行下载脚本（见上），模型就绪后脚本会自动把 `NR_MODELS_DIR` 写入
> 用户级环境变量（`NR_SKIP_ENV=1` 可跳过），此后 resampler 被复制到任何位置
> （如 OpenUtau 的 `Resamplers/`）都能找到模型。
> ⚠ 使用此方案期间**不要移动或重命名**本 `models/` 目录；若必须移动，请重跑脚本
> 或手动更新 `NR_MODELS_DIR`。

`resampler info` 会列出每个模型的就绪状态。

---

## English

This directory holds the ONNX models required for inference. They are **not
distributed with the repository** (large size, and their licenses may not allow
redistribution).

| File | Purpose | Required |
| --- | --- | --- |
| `pc_nsf_hifigan.onnx` | Vocoder: Mel + F0 → waveform | Yes |
| `fcpe.onnx` | F0 extraction (16 kHz) | No (falls back to built-in DSP if missing) |
| `hnsep.onnx` | HN-SEP harmonic/noise separation | No |

To obtain them (the script automatically switches to a gh-proxy mirror when
GitHub is unreachable or too slow, and verifies SHA-256 after download):

```bash
bash scripts/download_models.sh          # Linux / macOS / Git Bash
powershell -ExecutionPolicy Bypass -File scripts/download_models.ps1   # Windows
```

Without models: `selftest` / `info` still run in degraded mode (vocoder → Stub,
F0 → built-in DSP) for connectivity checks; but `render` / `batch` / UTAU-protocol
rendering **fails with an explicit error** (trilingual message with the fix)
instead of producing unusable audio. If you intentionally need the degraded
backend for offline testing, pass `--allow-stub` or set `NR_ALLOW_STUB=1`.
**Real use requires the real models.**

Model directory resolution order: the `--models` argument > the
`NR_MODELS_DIR` environment variable > a `models/` folder next to the
executable (covers OpenUtau copying the exe into `Resamplers/`) > a `models/`
folder in the working directory.

> **Recommended**: run the download script (above). Once the models are ready,
> it automatically writes `NR_MODELS_DIR` into your user environment
> (`NR_SKIP_ENV=1` to skip), so the resampler finds the models wherever it is
> copied (e.g. OpenUtau's `Resamplers/`).
> ⚠ While using this method, do **not** move or rename this `models/` directory;
> if you must move it, rerun the script or update `NR_MODELS_DIR` manually.

`resampler info` lists the ready state of each model.

---

## 日本語

このディレクトリには推論に必要な ONNX モデルを置きます。**リポジトリには
同梱されていません**（容量が大きく、各ライセンスの制約を受けるため）。

| ファイル | 役割 | 必須 |
| --- | --- | --- |
| `pc_nsf_hifigan.onnx` | ボコーダ: Mel + F0 → 波形 | はい |
| `fcpe.onnx` | F0 抽出（16kHz） | いいえ（欠落時は内蔵 DSP にフォールバック） |
| `hnsep.onnx` | HN-SEP 調波/雑音分離 | いいえ |

入手方法（GitHub への直接接続が不可能・低速な場合、gh-proxy ミラーへ自動切替。
ダウンロード後に SHA-256 検証を行います）：

```bash
bash scripts/download_models.sh          # Linux / macOS / Git Bash
powershell -ExecutionPolicy Bypass -File scripts/download_models.ps1   # Windows
```

モデルが存在しない場合：`selftest` / `info` は低下動作で実行されます
（ボコーダ → Stub、F0 → 内蔵 DSP）ので接続確認に使えます。一方
`render` / `batch` / UTAU プロトコルのレンダリングは**明示的なエラーで終了**
します（対処法つきの三言語メッセージ）。実用にならない音声は出力されません。
オフライン検証のために意図的に劣化バックエンドを使いたい場合は
`--allow-stub` を付けるか `NR_ALLOW_STUB=1` を設定してください。
**実用には実モデルのインストールが必要です。**

モデルディレクトリの解決順：`--models` 引数 > 環境変数 `NR_MODELS_DIR` >
実行ファイルと同じ階層の `models/`（OpenUtau が実行ファイルを `Resamplers/`
へコピーするケースをカバー）> 作業ディレクトリの `models/`。

> **推奨**：ダウンロードスクリプト（上記）を実行すると、モデルの準備ができた後
> 自動的に `NR_MODELS_DIR` がユーザー環境変数に書き込まれます
> （`NR_SKIP_ENV=1` でスキップ）。以降、リサンプラーがどこにコピーされても
> （OpenUtau の `Resamplers/` など）モデルを見つけられます。
> ⚠ この方法を使っている間、この `models/` ディレクトリを**移動・リネーム
> しないでください**。移動する場合はスクリプトを再実行するか
> `NR_MODELS_DIR` を手動更新してください。

`resampler info` で各モデルの準備状態を一覧表示できます。
