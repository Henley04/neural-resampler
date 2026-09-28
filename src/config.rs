//! 重采样器配置
//!
//! 默认配置以 `config/resampler.yaml` 的形式随源码分发，并通过
//! [`DEFAULT_CONFIG_YAML`] 编译进二进制，保证单文件部署时无需外部配置文件。

use anyhow::{Context as _, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// 随源码分发的默认配置（编译期嵌入）。
pub const DEFAULT_CONFIG_YAML: &str = include_str!("../config/resampler.yaml");

/// F0 提取后端。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum F0Backend {
    /// 优先 FCPE（ONNX），不可用时回退到内置 DSP（WORLD 风格），再回退到乐谱。
    Auto,
    /// FCPE ONNX。
    Fcpe,
    /// 内置 DSP 回退实现（DIO / Harvest 风格）。
    World,
    /// 不做 F0 分析，F0 完全由乐谱（音名 + pitchBend）决定。
    None,
}

/// F0 生成模式。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum F0Mode {
    /// F0 完全来自乐谱：音名 + pitchBend（原始 HiFiSampler 行为）。
    Score,
    /// F0 来自声库样本，整体移调目标音高。
    Source,
    /// 乐谱给出绝对音高，样本 F0 提供自然的颤音/滑音偏移与清浊判定。
    Hybrid,
}

/// 声码器后端选择。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VocoderBackend {
    /// ONNX Runtime（ort）。
    Ort,
    /// 无模型时的内置回声（离线自测用，不用于生产）。
    Stub,
}

/// Mel 标度公式。
///
/// PC-NSF-HiFiGAN 与 FCPE 的训练前处理都用 `librosa.filters.mel`，
/// 其默认公式为 **Slaney**；HTK 公式仅在与少数第三方实现对齐时才需要。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MelScale {
    /// librosa 默认（Slaney）。
    #[default]
    Slaney,
    /// HTK 公式。
    Htk,
}

/// Mel 频谱分析配置（44.1kHz 侧）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MelConfig {
    pub sample_rate: u32,
    pub n_fft: usize,
    pub win_size: usize,
    /// 渲染用帧移（声码器 hop）。
    pub hop_size: usize,
    /// 分析用帧移（更细，便于时域拉伸插值）。
    pub origin_hop_size: usize,
    pub n_mels: usize,
    pub fmin: f32,
    pub fmax: f32,
    /// 动态范围压缩下限（log 压缩的 clamp 值）。
    pub clip_val: f32,
    /// 幅度谱的能量地板：`sqrt(re² + im² + eps)`。
    ///
    /// SingingVocoders / HiFiSampler 直接用 `abs()`（即 0.0）；
    /// FCPE 用 1e-9。差异只在极静音段可见。
    #[serde(default)]
    pub magnitude_eps: f32,
    /// Mel 标度公式，默认 Slaney（与 librosa 一致）。
    #[serde(default)]
    pub mel_scale: MelScale,
}

impl Default for MelConfig {
    fn default() -> Self {
        Self {
            sample_rate: 44100,
            n_fft: 2048,
            win_size: 2048,
            hop_size: 512,
            origin_hop_size: 128,
            n_mels: 128,
            fmin: 40.0,
            fmax: 16000.0,
            clip_val: 1e-9,
            magnitude_eps: 0.0,
            mel_scale: MelScale::Slaney,
        }
    }
}

/// FCPE 的 latent→cent 解码方式。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum F0Decoder {
    /// 在 argmax 附近的局部窗口上做加权平均（FCPE 默认，抗噪更好）。
    #[default]
    LocalArgmax,
    /// 在全部 cent 类上做加权平均。
    Argmax,
}

/// F0 提取配置。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct F0Config {
    pub backend: F0Backend,
    pub mode: F0Mode,
    /// FCPE ONNX 模型路径（相对 models 目录或绝对路径）。
    pub model: PathBuf,
    pub sample_rate: u32,
    pub hop_size: usize,
    pub win_size: usize,
    pub n_fft: usize,
    pub mel_bins: usize,
    pub fmin: f32,
    pub fmax: f32,
    /// 有效 F0 下限 / 上限（Hz）。
    pub f0_min: f32,
    pub f0_max: f32,
    /// 低于该阈值的帧判定为清音。
    pub uv_threshold: f32,
    /// 是否用分析结果把清音帧的 F0 置零。
    pub uv_mask: bool,
    /// hybrid 模式下允许的最大自然偏移（cent）。
    pub max_deviation_cents: f32,
    /// hybrid 模式下基准曲线的平滑窗口（毫秒）。
    pub smoothing_ms: f32,
    /// FCPE latent 解码方式。
    #[serde(default)]
    pub decoder: F0Decoder,
    /// `local_argmax` 解码的单侧窗口宽度（总窗口 2×width+1，FCPE 固定为 4）。
    #[serde(default = "default_local_argmax_width")]
    pub local_argmax_width: usize,
    /// FCPE cent 表对应的最低频率（Hz）。模型内为 32.70（C1）。
    #[serde(default = "default_cent_f0_min")]
    pub cent_f0_min: f32,
    /// FCPE cent 表对应的最高频率（Hz）。模型内为 1975.5（B6）。
    #[serde(default = "default_cent_f0_max")]
    pub cent_f0_max: f32,
    /// F0 分析时 Mel 的动态范围压缩下限（FCPE 用 1e-5）。
    #[serde(default = "default_f0_clip_val")]
    pub clip_val: f32,
}

fn default_local_argmax_width() -> usize {
    4
}
fn default_cent_f0_min() -> f32 {
    32.70
}
fn default_cent_f0_max() -> f32 {
    1975.5
}
fn default_f0_clip_val() -> f32 {
    1e-5
}

impl Default for F0Config {
    fn default() -> Self {
        Self {
            backend: F0Backend::Auto,
            mode: F0Mode::Hybrid,
            model: PathBuf::from("fcpe.onnx"),
            sample_rate: 16000,
            hop_size: 160,
            win_size: 1024,
            n_fft: 1024,
            mel_bins: 128,
            fmin: 0.0,
            fmax: 8000.0,
            f0_min: 80.0,
            f0_max: 880.0,
            uv_threshold: 0.006,
            uv_mask: true,
            max_deviation_cents: 100.0,
            smoothing_ms: 40.0,
            decoder: F0Decoder::LocalArgmax,
            local_argmax_width: 4,
            cent_f0_min: 32.70,
            cent_f0_max: 1975.5,
            clip_val: 1e-5,
        }
    }
}

/// 声码器 `mel` 输入的内存布局。
///
/// 不同导出脚本会给出不同的张量形状：`[1, n_mels, T]` 或 `[1, T, n_mels]`。
/// `Auto` 会读取模型元数据自动判断（形状里等于 mel bin 数的那一维即为通道维）。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MelLayout {
    /// 按模型元数据自动判断。
    #[default]
    Auto,
    /// `[batch, n_mels, frames]`
    ChannelsFirst,
    /// `[batch, frames, n_mels]`
    FramesFirst,
}

/// 声码器配置。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VocoderConfig {
    pub backend: VocoderBackend,
    /// PC-NSF-HiFiGAN ONNX 模型路径。
    pub model: PathBuf,
    /// 可选：HN-SEP 谐波/噪声分离模型。
    pub hnsep_model: Option<PathBuf>,
    pub mel_input: String,
    pub f0_input: String,
    pub output: String,
    /// `mel` 输入的内存布局。
    #[serde(default)]
    pub mel_layout: MelLayout,
    /// 执行提供者，按顺序尝试：cpu / directml / cuda / coreml。
    pub providers: Vec<String>,
}

impl Default for VocoderConfig {
    fn default() -> Self {
        Self {
            backend: VocoderBackend::Ort,
            model: PathBuf::from("pc_nsf_hifigan.onnx"),
            hnsep_model: Some(PathBuf::from("hnsep.onnx")),
            mel_input: "mel".into(),
            f0_input: "f0".into(),
            output: "audio".into(),
            mel_layout: MelLayout::Auto,
            providers: vec!["cpu".into()],
        }
    }
}

/// 时序与循环相关配置。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessingConfig {
    /// 长音符时循环拼接（反射填充）元音段。
    pub loop_mode: bool,
    /// 首尾额外保留的帧数（避免插值越界造成的爆音）。
    pub fill: usize,
    /// 分析前是否裁剪静音。
    pub trim_silence: bool,
    pub silence_threshold_db: f32,
    /// 默认性别/共振峰偏移（对应 `g` 标记，单位 0.01 半音）。
    pub gender: f32,
}

impl Default for ProcessingConfig {
    fn default() -> Self {
        Self {
            loop_mode: true,
            fill: 6,
            trim_silence: false,
            silence_threshold_db: -52.0,
            gender: 0.0,
        }
    }
}

/// 输出与后处理配置。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputConfig {
    pub sample_rate: u32,
    /// 16 / 24 / 32 位 PCM，或 0 表示 32 位浮点。
    pub bit_depth: u16,
    /// 峰值上限，超过则整体归一化。
    pub peak_limit: f32,
    /// 是否做响度归一化。
    pub wave_norm: bool,
    /// 目标响度（dBFS，RMS 近似）。
    pub loudness_target: f32,
    pub loudness_block_ms: f32,
    /// 强制淡入淡出（毫秒），0 表示关闭。
    pub fade_in_ms: f32,
    pub fade_out_ms: f32,
}

impl Default for OutputConfig {
    fn default() -> Self {
        Self {
            sample_rate: 44100,
            bit_depth: 16,
            peak_limit: 1.0,
            wave_norm: true,
            loudness_target: -16.0,
            loudness_block_ms: 400.0,
            fade_in_ms: 0.0,
            fade_out_ms: 0.0,
        }
    }
}

/// 特征缓存配置（参考 Organum 的 zstd 缓存思路）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheConfig {
    pub enabled: bool,
    /// 缓存文件后缀。
    pub extension: String,
    /// zstd 压缩等级（1~22）。
    pub zstd_level: i32,
    /// 缓存目录；为空则与源音频同目录。
    pub dir: Option<PathBuf>,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            extension: "nrc".into(),
            zstd_level: 3,
            dir: None,
        }
    }
}

/// 运行时配置。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeConfig {
    pub log_level: String,
    pub log_format: String,
    /// ONNX Runtime 线程数，0 表示由 ORT 自行决定。
    pub intra_op_threads: usize,
    pub inter_op_threads: usize,
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            log_level: "info".into(),
            log_format: "text".into(),
            intra_op_threads: 0,
            inter_op_threads: 0,
        }
    }
}

/// 顶层配置。
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ResamplerConfig {
    #[serde(default)]
    pub mel: MelConfig,
    #[serde(default)]
    pub f0: F0Config,
    #[serde(default)]
    pub vocoder: VocoderConfig,
    #[serde(default)]
    pub processing: ProcessingConfig,
    #[serde(default)]
    pub output: OutputConfig,
    #[serde(default)]
    pub cache: CacheConfig,
    #[serde(default)]
    pub runtime: RuntimeConfig,
    /// 模型根目录，用于解析配置里的相对模型路径。
    #[serde(default = "default_models_dir")]
    pub models_dir: PathBuf,
}

fn default_models_dir() -> PathBuf {
    PathBuf::from("models")
}

impl ResamplerConfig {
    /// 载入内嵌的默认配置。
    pub fn default_yaml() -> Result<Self> {
        let cfg: Self =
            serde_yaml::from_str(DEFAULT_CONFIG_YAML).context("解析内嵌默认配置失败")?;
        Ok(cfg)
    }

    /// 从 YAML 文件载入；文件不存在时回退到默认配置。
    pub fn load(path: &Path) -> Result<Self> {
        if !path.exists() {
            log::warn!("配置文件 {path:?} 不存在，使用默认配置");
            return Self::default_yaml();
        }
        let text =
            std::fs::read_to_string(path).with_context(|| format!("读取配置文件失败: {path:?}"))?;
        let cfg: Self =
            serde_yaml::from_str(&text).with_context(|| format!("解析配置文件失败: {path:?}"))?;
        Ok(cfg)
    }

    /// 写回 YAML 文件（便于用户生成一份可编辑的模板）。
    pub fn save(&self, path: &Path) -> Result<()> {
        let text = serde_yaml::to_string(self).context("序列化配置失败")?;
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
        }
        std::fs::write(path, text)?;
        Ok(())
    }

    /// 解析模型路径：相对路径相对 `models_dir`，绝对路径原样返回。
    pub fn model_path(&self, p: &Path) -> PathBuf {
        if p.is_absolute() {
            p.to_path_buf()
        } else {
            self.models_dir.join(p)
        }
    }

    /// 配置指纹，用于缓存键（配置变更时自动失效）。
    pub fn digest(&self) -> String {
        let json = serde_json::to_string(self).unwrap_or_default();
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(json.as_bytes());
        let out = hasher.finalize();
        out.iter().take(8).map(|b| format!("{b:02x}")).collect()
    }

    /// 分析阶段 Mel 的帧移（秒）。
    pub fn origin_frame_period(&self) -> f64 {
        self.mel.origin_hop_size as f64 / self.mel.sample_rate as f64
    }

    /// 渲染阶段（声码器）帧移（秒）。
    pub fn frame_period(&self) -> f64 {
        self.mel.hop_size as f64 / self.mel.sample_rate as f64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_yaml_parses() {
        let cfg = ResamplerConfig::default_yaml().unwrap();
        assert_eq!(cfg.mel.sample_rate, 44100);
        assert_eq!(cfg.mel.n_mels, 128);
        assert_eq!(cfg.f0.sample_rate, 16000);
        assert_eq!(cfg.f0.hop_size, 160);
        assert_eq!(cfg.vocoder.mel_input, "mel");
        assert_eq!(cfg.vocoder.f0_input, "f0");
        assert_eq!(cfg.vocoder.output, "audio");
        // 与训练前处理一致的默认值
        assert_eq!(cfg.mel.mel_scale, MelScale::Slaney);
        assert_eq!(cfg.mel.magnitude_eps, 0.0);
        assert_eq!(cfg.mel.clip_val, 1e-9);
        assert_eq!(cfg.f0.decoder, F0Decoder::LocalArgmax);
        assert_eq!(cfg.vocoder.mel_layout, MelLayout::Auto);
    }

    #[test]
    fn digest_is_stable() {
        let a = ResamplerConfig::default_yaml().unwrap();
        let b = ResamplerConfig::default_yaml().unwrap();
        assert_eq!(a.digest(), b.digest());
        assert_eq!(a.digest().len(), 16);
    }

    #[test]
    fn model_path_resolution() {
        let mut cfg = ResamplerConfig::default_yaml().unwrap();
        cfg.models_dir = PathBuf::from("/tmp/models");
        assert_eq!(
            cfg.model_path(Path::new("fcpe.onnx")),
            PathBuf::from("/tmp/models/fcpe.onnx")
        );
        // 默认配置中的模型路径应相对 models_dir，避免出现 models/models/...
        let default_cfg = ResamplerConfig::default_yaml().unwrap();
        let p = default_cfg.model_path(&default_cfg.vocoder.model);
        assert!(
            !p.to_string_lossy().contains("models/models"),
            "路径重复: {p:?}"
        );
    }
}
