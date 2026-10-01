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

模型不存在时引擎会降级运行（声码器 → Stub、F0 → 内置 DSP），管线仍可跑通，
用于验证连通性；**音质需真实模型**。

模型目录解析顺序：`--models` 参数 > 环境变量 `NR_MODELS_DIR` > 可执行文件同级
`models/`（覆盖 OpenUtau 把 exe 复制进 `Resamplers/` 的场景）> 工作目录
`models/`。

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

Without models the engine degrades gracefully (vocoder → Stub, F0 → built-in
DSP) and the pipeline still runs, which is useful for connectivity checks;
**real audio quality requires the real models**.

Model directory resolution order: the `--models` argument > the
`NR_MODELS_DIR` environment variable > a `models/` folder next to the
executable (covers OpenUtau copying the exe into `Resamplers/`) > a `models/`
folder in the working directory.

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

モデルが存在しない場合、エンジンは低下動作で実行されます（ボコーダ → Stub、
F0 → 内蔵 DSP）。パイプライン自体は動作するため接続確認に使えますが、
**実用音質には実モデルが必要です**。

モデルディレクトリの解決順：`--models` 引数 > 環境変数 `NR_MODELS_DIR` >
実行ファイルと同じ階層の `models/`（OpenUtau が実行ファイルを `Resamplers/`
へコピーするケースをカバー）> 作業ディレクトリの `models/`。

`resampler info` で各モデルの準備状態を一覧表示できます。
