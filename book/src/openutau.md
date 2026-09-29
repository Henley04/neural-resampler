# 在 OpenUtau 中使用

OpenUtau 的 resampler 机制比原生 UTAU 更灵活，直接指向二进制即可。

## 配置步骤

1. 打开 OpenUtau → `Tools` → `Options`（工具 → 选项）
2. 找到 `Resampler` 一项
3. 选择 **`Custom`**（自定义）
4. 在可执行文件路径处填入 `resampler`（Windows 为 `resampler.exe`）的完整路径
5. 保存，重新渲染工程

OpenUtau 会按标准 resampler 协议传入 13 个参数，与[原生 UTAU](utau.md) 完全一致，
无需额外配置参数模板。

## 路径注意事项

* Windows 路径含空格时建议用短路径或把程序放在无空格目录，例如 `C:\nr\resampler.exe`
* macOS / Linux 上确保二进制有执行权限：`chmod +x resampler`
* macOS 首次运行若被 Gatekeeper 拦截（二进制非签名），需在「系统设置 → 隐私与安全性」中允许，
  或执行：`xattr -d com.apple.quarantine ./resampler`

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
