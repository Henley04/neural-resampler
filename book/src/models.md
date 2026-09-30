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

发布产物里附带了下载脚本（bash 与 PowerShell 各一份，逻辑相同）：

```bash
./download_models.sh      # Linux / macOS / Git Bash
```

```powershell
powershell -ExecutionPolicy Bypass -File .\download_models.ps1    # Windows PowerShell
```

仓库中也有一份（`scripts/download_models.sh` 与 `scripts/download_models.ps1`）。
脚本支持自动重试，并在下载后做 **SHA-256 校验**（基线为与 v0.1.0 一起验证过的版本），
文件损坏或被上游更换时会拒绝安装并给出提示。

### 下载源与镜像加速

脚本默认直连 GitHub（`raw.githubusercontent.com`），并内置 gh-proxy 镜像加速：

1. **自动探测**：GitHub 直连不可达时，自动切换镜像并提示
2. **慢速切换**：下载平均速度低于 100KB/s 持续 10 秒时，若当前是直连会
   询问是否切换镜像（`NR_MODEL_AUTO_SWITCH=1` 可免询问自动切换）；
   直连重试耗尽后也会自动用镜像做最后一轮
3. **强制镜像**：设置 `NR_MODEL_MIRROR` 直接使用指定镜像前缀
   （[gh-proxy](https://github.com/hunshcn/gh-proxy) 格式：前缀 + 完整原始 URL）：

   ```bash
   NR_MODEL_MIRROR=https://ghfast.top/ ./download_models.sh
   ```

   也可换成任意自部署或其他 gh-proxy 实例。

> 上游模型更新导致校验不通过时，确认新版本可用后可用
> `NR_MODEL_SKIP_CHECKSUM=1` 临时跳过校验——**请确认来源可信再这么做**
> （镜像内容被篡改也会被校验拦下）。

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
