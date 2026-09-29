# 获取模型

引擎需要 ONNX 模型才能产出可用音质。**模型不随仓库与发布产物分发**——体积约 100 MB，
且各自来源的许可不一定允许再分发，所以请自行获取。

## 放置位置

模型放在程序**同级或指定**的 `models/` 目录下：

| 文件 | 作用 | 必需 |
| --- | --- | --- |
| `pc_nsf_hifigan.onnx` | 声码器：Mel + F0 → 波形 | **是** |
| `fcpe.onnx` | F0 提取（16kHz） | 否，缺失时回退到内置 DSP |
| `hnsep.onnx` | HN-SEP 谐波/噪声分离 | 否 |

用 `--models <DIR>` 可以临时指向别的目录：

```bash
./resampler --models /path/to/models info
```

## 一键下载

发布产物里附带了下载脚本：

```bash
./download_models.sh
```

仓库中也有一份（`scripts/download_models.sh`）。脚本支持重试与断点续传，
如果直连 GitHub 不畅，可用镜像前缀：

```bash
NR_MODEL_MIRROR=https://ghfast.top/ ./download_models.sh
```

## 手动获取

脚本失效时可手动下载，重命名为上表中的文件名后放进 `models/`：

1. **FCPE** —— [CNChTu/FCPE](https://github.com/CNChTu/FCPE) 的发布页，取 `fcpe.onnx`
2. **PC-NSF-HiFiGAN** —— 需 ONNX 格式。若手头只有 `.ckpt` 权重，
   可用仓库内的转换脚本：
   ```bash
   python scripts/convert_vocoder_to_onnx.py --ckpt model.ckpt --out models/pc_nsf_hifigan.onnx
   ```

## 确认模型可用

```bash
./resampler info
```

关注这三行：

```
声码器  : "models/pc_nsf_hifigan.onnx"  [已就绪]
FCPE    : "models/fcpe.onnx"  [已就绪]
HN-SEP  : "models/hnsep.onnx"  [缺失]
```

* 声码器 `[缺失]` → 会退化为 Stub 后端，**出声但音质不可用**
* FCPE `[缺失]` → 退化为内置 DSP 提取 F0，仍可正常工作，只是歌声场景略不稳
* HN-SEP `[缺失]` → 正常，属于可选增强

## 许可

请遵循各模型来源仓库/发布页的许可要求。将模型打包进你自己的分发物之前，
务必先确认许可是否允许。
