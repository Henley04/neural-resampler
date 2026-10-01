# 在 OpenUtau 中使用

OpenUtau 通过 `Resamplers` 目录管理第三方 resampler。它按标准 resampler 协议
传入 13 个参数，与[原生 UTAU](utau.md) 完全一致，无需额外配置参数模板。

## 配置步骤

依据 [OpenUtau 官方 wiki](https://github.com/openutau/OpenUtau/wiki/Resamplers-and-Wavtools)：

1. 把 `resampler`（Windows 为 `resampler.exe`）放进 OpenUtau 的 `Resamplers` 文件夹：
   * Windows：OpenUtau 程序目录下的 `Resamplers`
   * Linux：`~/.local/share/OpenUtau/Resamplers`
   * 也可把可执行文件直接拖到 OpenUtau 主窗口，选 **"Install as resampler"**（0.1.119+）
2. 把渲染器切到 **`CLASSIC`**
3. 点击渲染器旁的 **⚙ 齿轮图标**，在 Resampler 下拉中选择本 resampler
4. 重新渲染工程

> 可选：在 `Resamplers` 目录放一个与可执行文件同名的 `.yaml`
> （如 `resampler.yaml`）作为 Resampler Manifest，向 OpenUtau 声明本引擎支持的
> flags（expressions），表达式面板会据此显示建议值与范围。

## 模型放置（重要）

OpenUtau 安装 resampler 时会把可执行文件**复制**到自己的 `Resamplers/` 目录，
渲染时的工作目录是 OpenUtau 安装根目录——那里的 `models/` 不存在，引擎会
**静默降级**（Stub 声码器 + 内置 DSP F0：能出声、不报错，但音质不可用）。

模型目录按以下顺序解析：

1. 命令行 `--models`（OpenUtau 不会传，仅供 CLI 手动调用）
2. 环境变量 `NR_MODELS_DIR`（推荐：设一次，全局生效）
3. 可执行文件**同级**的 `models/`（即 `Resamplers/models/`）
4. 当前工作目录的 `models/`（发布包布局）

OpenUtau 场景推荐 2 或 3。放好后用 `resampler info` 确认声码器后端不再是
`stub`（应显示 `onnxruntime`）。

## 路径注意事项

* 放进 `Resamplers` 目录（或拖放安装）后，路径由 OpenUtau 管理，无需手动指定
* macOS / Linux 上确保二进制有执行权限：`chmod +x resampler`
* macOS 首次运行若被 Gatekeeper 拦截（二进制非签名），需在「系统设置 → 隐私与安全性」中允许，
  或执行：`xattr -d com.apple.quarantine ./resampler`
* 在 macOS / Linux 上运行 Windows 版 resampler 需配置 Wine
  （`Tools > Preferences > Advanced > Wine Path`）；本仓库提供原生 macOS/Linux 产物，无需 Wine

## 验证配置是否生效

在 OpenUtau 里渲染一个音符后，查看输出 WAV。也可以用命令行独立验证同样的参数：

```bash
./resampler render <样本路径> /tmp/check.wav C4 100 "" 0 500 60 -50 100 0 '!120' AA
```

如果命令行能出声而 OpenUtau 不能，问题多半在路径或权限，而不是引擎本身。

## 与经典 resampler 的差异

OpenUtau 默认自带的世界系 resampler（如 `worldline`）在拼接处做相位对齐；
本引擎走神经声码器路线，输出的是模型合成的波形，因此：

* 音质更接近自然发声，但**每次渲染结果完全一致**（无随机性）
* 长音符依赖循环拼接，可用 `He` 标记强制开启
* CPU 推理，单次渲染耗时比经典 resampler 高，建议配合缓存使用

## 性能

首次渲染会有模型加载开销（约几十毫秒），之后每个音符的耗时主要取决于
Mel 分析与声码器推理。启用缓存后重复渲染同一输入会快很多。

如需 GPU 加速，需用 `--features cuda`（NVIDIA）或 `--features directml`（Windows）
自行编译，详见[安装](installation.md)。
