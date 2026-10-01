# UTAU / OpenUtau 接入

## 协议

二进制本身即 resampler，直接按 UTAU 的 13 参数协议调用：

```
resampler <in.wav> <out.wav> <pitch> <velocity> <flags> <offset> <length_req>
          <consonant> <cutoff> <volume> <modulation> <tempo> <pitchBend>
```

| 位置 | 参数 | 单位 | 说明 |
| --- | --- | --- | --- |
| 1 | in.wav | — | 声库样本 |
| 2 | out.wav | — | 渲染结果 |
| 3 | pitch | 音名 | 如 `C4`、`A#3` |
| 4 | velocity | 0–200 | 力度 |
| 5 | flags | — | 见下 |
| 6 | offset | ms | 左空白 |
| 7 | length_req | ms | 要求长度 |
| 8 | consonant | ms | 子音部（不拉伸） |
| 9 | cutoff | ms | 右侧空白，常为负数 |
| 10 | volume | % | 音量 |
| 11 | modulation | % | 调制 |
| 12 | tempo | `!BPM` 或 BPM | 速度 |
| 13 | pitchBend | Base64+RLE | 音高曲线，单位 cent |

`cutoff` 通常为负数（如 `-50`），CLI 已开启 `allow_negative_numbers`，可直接传。

## 在 OpenUtau 中配置

把 `resampler`（Windows 为 `resampler.exe`）放进 OpenUtau 的 `Resamplers` 目录
（Windows 为程序目录下；Linux 为 `~/.local/share/OpenUtau/Resamplers`；或拖入
OpenUtau 窗口选 "Install as resampler"），渲染器切到 `CLASSIC` 后点旁边的
⚙ 齿轮选择它。声库的 `oto.ini` 与 WAV 完全不用改动。
参见 [OpenUtau wiki: Resamplers and Wavtools](https://github.com/openutau/OpenUtau/wiki/Resamplers-and-Wavtools)。

## 模型目录解析

OpenUtau 安装 resampler 时会把可执行文件**复制**进 `Resamplers/`，渲染时的
工作目录下没有 `models/`。模型缺失时引擎**不再静默降级**：渲染会报错退出
（三语提示 + 在 resampler 旁生成 `MODEL-MISSING-READ-ME.txt`）。模型目录按
以下顺序解析：命令行 `--models` > 环境变量 `NR_MODELS_DIR` > 可执行文件同级
`models/`（即 `Resamplers/models/`）> 工作目录 `models/`。OpenUtau 场景推荐
运行一次包内 `download_models.sh` / `download_models.ps1`——脚本会自动把
`NR_MODELS_DIR` 写入用户级环境变量（`NR_SKIP_ENV=1` 跳过；⚠ 使用期间不要
移动该 models 目录）；或把模型放进 `Resamplers/models/`。用 `resampler info`
确认声码器后端为 `onnxruntime`。离线自测需降级运行时加 `--allow-stub` 或设
`NR_ALLOW_STUB=1`。

## 支持的 flags

| 标记 | 含义 | 实现 |
| --- | --- | --- |
| `g±N` | 性别/共振峰偏移（0.01 半音） | 分析阶段缩放窗长（key shift） |
| `P±N` | 响度归一化强度（%） | 在原始与归一化结果间插值 |
| `t/N` | 颤音速度 | 与 `pitchBend` 叠加 |
| `A/B/G/S/p/R/D/C/Z` | HiFiSampler 兼容标记 | 解析但不强制生效 |

未知标记会解析进 map 但被忽略，不会导致渲染失败。

## pitchBend 解码

1. 按 `#` 分段：`<b64>#<rle>#<b64>#<rle>#...`
2. 每两个 Base64 字符解出一个 12 位有符号整数（`-2048..2047`）
3. RLE 段重复上一个值
4. 末尾补一个 0（与原始实现一致）

单位为 **cent**，取值 ±2048（约 ±20 半音）。采样网格为 `60 / (tempo × 96)` 秒/点，
超出曲线范围的时间点按端点值钳制。

## oto.ini

引擎读取同目录下的 `oto.ini` 以获取 falg / 偏移等默认值（命令行参数优先）。
编码按 UTF-8 → Shift-JIS → GBK 顺序尝试，兼容日文与简中声库。

## 批量渲染

把每组参数写成一行，交给 `batch` 并行处理：

```bash
resampler batch list.txt --jobs 4
```

每个 worker 持有独立的 ONNX 会话（`Session` 非 `Sync`），模型不重复加载。
