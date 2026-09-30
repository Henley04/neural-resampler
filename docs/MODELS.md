# 模型说明

引擎需要两个 ONNX 模型，都放在 `models/`（可用 `--models <DIR>` 或配置 `models_dir` 指定）。

| 文件 | 作用 | 来源 |
| --- | --- | --- |
| `pc_nsf_hifigan.onnx` | 声码器：Mel + F0 → 波形 | [openvpi/vocoders](https://github.com/openvpi/vocoders)（需导出为 ONNX）或现成导出件 |
| `fcpe.onnx` | F0 提取（16kHz 输入） | [CNChTu/FCPE](https://github.com/CNChTu/FCPE) |
| `hnsep.onnx`（可选） | HN-SEP 谐波/噪声分离，改善气声与噪声段 | 可选，缺失即跳过 |

## 自动获取

```bash
bash scripts/download_models.sh          # 下载到 ./models
bash scripts/download_models.sh /path/to/dir
powershell -ExecutionPolicy Bypass -File scripts/download_models.ps1   # Windows
```

脚本只做下载与校验，不做转换。GitHub 直连不可达或低于 100KB/s 时会自动
切换 gh-proxy 镜像（`NR_MODEL_MIRROR=镜像前缀` 可强制指定）。

## 手动准备

### 声码器

从 [openvpi/vocoders](https://github.com/openvpi/vocoders/releases) 下载
`pc-nsf-hifigan-44.1k-hop512-128bin-*` 发布包，得到 `.ckpt` 后用
`scripts/convert_vocoder_to_onnx.py`（需要 `torch` + `onnx`）导出：

```bash
python3 scripts/convert_vocoder_to_onnx.py \
    --ckpt pc_nsf_hifigan_44.1k_hop512_128bin_2025.02/model.ckpt \
    --out models/pc_nsf_hifigan.onnx
```

导出要求（必须与 `config/resampler.yaml` 的 `mel.*` 一致）：

| 参数 | 值 |
| --- | --- |
| sampling_rate | 44100 |
| num_mels | 128 |
| n_fft / win_size | 2048 |
| hop_size | 512 |
| fmin / fmax | 40 / 16000 |
| mini_nsf | true |

### F0 模型

FCPE 官方仓库提供 PyTorch 权重，社区已有导出好的 `fcpe.onnx`。
若自行导出，输入为 16kHz 的 **Mel 频谱**（`[1, T, 128]`），输出为 F0（Hz）。

## 节点名

默认配置假设：

```
声码器输入： mel       [1, T, 128]   float32
            f0        [1, T]        float32
声码器输出： waveform  [1, N]        float32
```

若你的导出件名字不同，改 `vocoder.mel_input / f0_input / output` 即可，无需改代码。

可以用 `resampler info` 检查模型是否已就位、能否加载。

## 缺失模型时的行为

| 缺失 | 行为 |
| --- | --- |
| 声码器 | 回退到 `backend::stub`（用 Mel 做简易频谱重建），**音质不可用**，仅保证管线连通 |
| FCPE | 回退到内置 DSP F0（`f0.backend: world`），再失败则纯乐谱 F0 |
| HN-SEP | 直接跳过该处理步骤 |

## 校验

```bash
resampler info                      # 列出模型是否存在 + 大小 + sha256 前 16 位
resampler selftest --out-dir /tmp/nr
```

CI 中模型不存在时会自动跳过 ONNX 相关断言（见 `.github/workflows/ci.yml`）。
