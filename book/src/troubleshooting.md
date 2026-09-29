# 故障排查

遇到问题时，**先用命令行复现**。编辑器里的报错通常信息很少，而命令行会给出具体原因：

```bash
./resampler --log-level debug render <样本> /tmp/t.wav C4 100 "" 0 500 0 0 100 0 '!120' AA
```

## 症状对照表

| 症状 | 可能原因 | 处理 |
| --- | --- | --- |
| `info` 里模型显示 `[缺失]` | 模型没放到 `models/` | 跑 `./download_models.sh`，或用 `--models` 指向实际目录 |
| 声码器后端是 `stub` | 同上 | 同上。**Stub 只用于连通性验证，音质不可用** |
| F0 后端是 `world-dio` | `fcpe.onnx` 缺失 | 功能正常，只是歌声场景略不稳；想用 FCPE 就补模型 |
| 输出长度明显不对 | `length` 单位是**毫秒** | 想要 1 秒就传 `1000`，不是 `44100` |
| 输出几乎无声 | 输入样本过小 / 响度归一化把声音压掉了 | 检查输入音量；试 `output.wave_norm: false` 对比 |
| 声音发闷、像被捂住 | `mel.*` 与模型规格不匹配 | 恢复默认 `mel.*`；确认 `mel_scale: slaney` |
| 声音破碎、有金属声 | `mel_layout` 判断错误 | 显式设 `vocoder.mel_layout: channels_first` 或 `frames_first` 试 |
| 音高整体不对 | 音名格式 / F0 模式 | 确认音名如 `C4`；换 `f0.mode` 试（见 [F0 模式](f0-modes.md)） |
| 编译报 ort 链接失败 | 同时启用了多个 GPU feature | **不要用 `--all-features`**，GPU feature 互斥 |
| 进程退出时报 SIGSEGV（exit 139） | `load-dynamic` 链接时 dylib 版本不匹配 | 见下方 [load-dynamic 版本约束](#load-dynamic-版本约束) |
| 每次渲染都很慢 | 缓存未命中 | 确认 `cache.enabled: true`；改配置会换指纹导致重建 |
| macOS 提示「无法打开」 | 二进制未签名 | `xattr -d com.apple.quarantine ./resampler` |
| UTAU 里没反应 | 路径含空格或中文 | 把程序放到纯英文无空格路径，如 `C:\nr\` |
| 日志里出现 pitchBend 告警 | pitchBend 字符串长度异常 | 已容错处理，只会丢掉尾部字符，不影响整体渲染 |

## 判断问题出在哪一层

按这个顺序排查，能快速定位：

**1. 模型在不在？**

```bash
./resampler info
```

两个 `[已就绪]` 才算完整。

**2. 引擎能不能出声？**

```bash
./resampler selftest
```

用内置生成的 440Hz 测试音跑一遍。如果这里都失败，问题在引擎/模型，
与你的声库无关。

**3. 你的样本能不能出声？**

```bash
./resampler render <你的样本> /tmp/t.wav C4
```

selftest 通过但这里失败，问题在样本本身（格式、采样率、时长）。

**4. 编辑器参数对不对？**

命令行能出声而编辑器不能，问题在编辑器的调用参数或路径配置。

## 日志

调高日志级别可以看到模型加载、后端选择、Mel 形状、F0 统计等细节：

```bash
./resampler --log-level debug --models ./models info
```

`--log-format json` 输出结构化日志，便于脚本处理：

```bash
./resampler --log-format json render a.wav b.wav C4 2>&1 | grep '"level":"error"'
```

## load-dynamic 版本约束

默认分发产物把 ONNX Runtime **静态链接**进二进制，无此问题。
但如果你以 `load-dynamic` 方式自行编译（运行时加载 `libonnxruntime.so` /
`onnxruntime.dll`），有一个隐蔽的坑：

**dylib 的版本必须与 ort-sys 构建时期望的版本精确一致**（如 1.28.0）。
用相邻版本（如 1.19.2、1.22.0）时——渲染完全正常，但**进程退出阶段会
SIGSEGV**（exit 139）：崩溃点在 `exit() → ld-linux 析构 → libonnxruntime`，
gdb 栈回溯看起来像库卸载问题，实际是 ABI 不兼容导致的清理路径越界。

排查方法：

* 确认加载的库版本：`ldd resampler | grep onnxruntime` 后查该 so 的版本
* 换成构建配置所对应的精确版本，或改用静态链接（默认分发模式）
* CI 宿主若检查退出码，此问题会表现为「渲染成功但任务失败」

## 缓存相关

缓存文件是输入同目录下的 `.nrc`（zstd 压缩）。缓存键 = 源文件路径 + 大小 + mtime +
配置指纹 + 采样率，所以：

* 改了配置 → 自动失效重建
* 换了源文件 → 自动失效重建
* 手动删 `.nrc` 只会让下次渲染变慢一次，不会出错

想关掉缓存：`cache.enabled: false`。

## 还是不行

带上这些信息到 [Issues](https://github.com/Henley04/neural-resampler/issues) 提问：

* `./resampler info` 的完整输出
* 复现用的命令行与参数
* `--log-level debug` 的日志
* 操作系统与平台
