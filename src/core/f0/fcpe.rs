//! FCPE（ONNX）F0 提取
//!
//! FCPE 在 16kHz 下推理，输入 `[1, T, 128]` 的 log-Mel（hop=160，1024 点 FFT）。
//! 模型输出的是 **cent 分类 latent** `[1, T, out_dims]`（官方权重为 360 类），
//! 需要按 FCPE 的解码流程还原成 F0：
//!
//! 1. `cent_table = linspace(1200·log2(f0_min/10), 1200·log2(f0_max/10), out_dims)`
//! 2. 取 latent 的 argmax 及其邻域（`local_argmax`，默认 ±4 共 9 类），做加权平均得到 cent
//! 3. `f0 = 10 · 2^(cent / 1200)`
//! 4. `max(latent) <= threshold` 的帧判为清音（F0 置 0）
//!
//! 最后按目标帧移做线性时间对齐。

use crate::config::{F0Decoder, ResamplerConfig};
use crate::core::audio::resample_sinc;
use crate::core::f0::{BackendUnavailable, F0Extractor, F0Track};
use anyhow::{Context as _, Result};
use std::path::PathBuf;

/// FCPE 使用的 Mel 配置（16kHz 专用，与声码器配置解耦）。
#[derive(Debug, Clone)]
pub struct FcpeMelConfig {
    pub sample_rate: u32,
    pub n_fft: usize,
    pub win_size: usize,
    pub hop_size: usize,
    pub n_mels: usize,
    pub fmin: f32,
    pub fmax: f32,
    /// 动态范围压缩下限（FCPE 用 1e-5）。
    pub clip_val: f32,
}

impl FcpeMelConfig {
    pub fn from_config(cfg: &ResamplerConfig) -> Self {
        Self {
            sample_rate: cfg.f0.sample_rate,
            n_fft: cfg.f0.n_fft,
            win_size: cfg.f0.win_size,
            hop_size: cfg.f0.hop_size,
            n_mels: cfg.f0.mel_bins,
            fmin: cfg.f0.fmin,
            fmax: cfg.f0.fmax,
            clip_val: cfg.f0.clip_val,
        }
    }

    fn to_mel_config(&self) -> crate::config::MelConfig {
        crate::config::MelConfig {
            sample_rate: self.sample_rate,
            n_fft: self.n_fft,
            win_size: self.win_size,
            hop_size: self.hop_size,
            origin_hop_size: self.hop_size,
            n_mels: self.n_mels,
            fmin: self.fmin,
            fmax: self.fmax,
            clip_val: self.clip_val,
            // FCPE 用 sqrt(re² + im² + 1e-9)
            magnitude_eps: 1e-9,
            ..Default::default()
        }
    }
}

/// FCPE ONNX 提取器。
pub struct FcpeExtractor {
    inner: Box<dyn FcpeRuntime + Send + Sync>,
    mel: FcpeMelConfig,
    /// 输出帧移（毫秒）：16000/160 = 10ms
    frame_period_ms: f32,
    f0_min: f32,
    f0_max: f32,
    uv_threshold: f32,
    decoder: F0Decoder,
    local_argmax_width: usize,
    /// cent 表：`cent_table[i]` 为第 i 类对应的音分。
    cent_table: Vec<f32>,
}

impl std::fmt::Debug for FcpeExtractor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FcpeExtractor")
            .field("mel", &self.mel)
            .field("frame_period_ms", &self.frame_period_ms)
            .finish_non_exhaustive()
    }
}

/// 推理运行时抽象：ONNX 与离线回退两套实现。
trait FcpeRuntime {
    fn name(&self) -> &str;
    /// 输入 `[1, T, n_mels]` 的 log-Mel。
    ///
    /// 返回 `(out_dims, latent)`，latent 为展平的 `[T, out_dims]` cent 分类概率。
    fn infer(&self, mel: &[f32], n_frames: usize, n_mels: usize) -> Result<(usize, Vec<f32>)>;
}

impl FcpeExtractor {
    /// 载入 FCPE 模型。模型缺失或 `onnx` feature 未启用时返回错误（由上层回退）。
    pub fn new(cfg: &ResamplerConfig) -> Result<Self> {
        let model_path: PathBuf = cfg.model_path(&cfg.f0.model);
        let inner = create_runtime(&model_path)?;
        let frame_period_ms = cfg.f0.hop_size as f32 / cfg.f0.sample_rate as f32 * 1000.0;
        // cent 表先按配置给出的默认类数（360）构建；推理时若模型实际类数不同则重新生成。
        let cent_table = build_cent_table(cfg.f0.cent_f0_min, cfg.f0.cent_f0_max, 360);
        Ok(Self {
            inner,
            mel: FcpeMelConfig::from_config(cfg),
            frame_period_ms,
            f0_min: cfg.f0.f0_min,
            f0_max: cfg.f0.f0_max,
            uv_threshold: cfg.f0.uv_threshold,
            decoder: cfg.f0.decoder,
            local_argmax_width: cfg.f0.local_argmax_width,
            cent_table,
        })
    }

    /// 输出帧移（毫秒）。
    pub fn frame_period_ms(&self) -> f32 {
        self.frame_period_ms
    }

    /// 计算 16kHz 下的 log-Mel（不依赖 ONNX）。
    pub fn mel_16k(&self, audio_16k: &[f32]) -> Result<crate::core::feature::MelSpectrogram> {
        let mel_cfg = self.mel.to_mel_config();
        crate::core::feature::compute_mel(audio_16k, &mel_cfg, self.mel.hop_size, 0.0)
    }

    /// `latent` → `cent`：按配置选择 argmax 或局部加权平均解码。
    fn latent_to_cents(&self, latent: &[f32], out_dims: usize, cent_table: &[f32]) -> Vec<f32> {
        let n_frames = latent.len() / out_dims.max(1);
        let mut cents = Vec::with_capacity(n_frames);
        for t in 0..n_frames {
            let row = &latent[t * out_dims..(t + 1) * out_dims];
            let (conf, max_idx) =
                row.iter()
                    .enumerate()
                    .fold((f32::NEG_INFINITY, 0usize), |(best, bi), (i, &v)| {
                        if v > best {
                            (v, i)
                        } else {
                            (best, bi)
                        }
                    });
            // 置信度低于阈值 → 清音
            if conf <= self.uv_threshold {
                cents.push(f32::NEG_INFINITY);
                continue;
            }
            let (num, den) = match self.decoder {
                F0Decoder::Argmax => row
                    .iter()
                    .enumerate()
                    .fold((0.0f32, 0.0f32), |(n, d), (i, &v)| {
                        (n + cent_table[i] * v, d + v)
                    }),
                F0Decoder::LocalArgmax => {
                    let w = self.local_argmax_width;
                    let start = max_idx.saturating_sub(w);
                    let end = (max_idx + w + 1).min(out_dims);
                    row[start..end]
                        .iter()
                        .enumerate()
                        .fold((0.0f32, 0.0f32), |(n, d), (k, &v)| {
                            (n + cent_table[start + k] * v, d + v)
                        })
                }
            };
            cents.push(if den > 0.0 {
                num / den
            } else {
                f32::NEG_INFINITY
            });
        }
        cents
    }

    /// `cent` → `f0`（Hz），并套用有效范围与清音判定。
    fn post_process(&self, cents: &[f32]) -> Vec<f32> {
        cents
            .iter()
            .map(|&c| {
                if !c.is_finite() {
                    return 0.0;
                }
                let f = 10.0 * 2f32.powf(c / 1200.0);
                if f < self.f0_min || f > self.f0_max || !f.is_finite() {
                    0.0
                } else {
                    f
                }
            })
            .collect()
    }
}

/// 构建 FCPE 的 cent 表：`linspace(1200·log2(f0_min/10), 1200·log2(f0_max/10), out_dims)`。
pub fn build_cent_table(f0_min: f32, f0_max: f32, out_dims: usize) -> Vec<f32> {
    let to_cent = |f: f32| 1200.0 * (f / 10.0).log2();
    let lo = to_cent(f0_min);
    let hi = to_cent(f0_max);
    if out_dims <= 1 {
        return vec![lo];
    }
    (0..out_dims)
        .map(|i| lo + (hi - lo) * i as f32 / (out_dims - 1) as f32)
        .collect()
}

impl F0Extractor for FcpeExtractor {
    fn name(&self) -> &str {
        self.inner.name()
    }

    fn frame_period_ms(&self) -> f32 {
        self.frame_period_ms
    }

    fn extract(&self, audio: &[f32], sample_rate: u32) -> Result<F0Track> {
        // 1. 降采样到 16kHz
        let audio_16k = if sample_rate == self.mel.sample_rate {
            audio.to_vec()
        } else {
            resample_sinc(audio, sample_rate, self.mel.sample_rate).with_context(|| {
                format!("重采样 {sample_rate}Hz → {}Hz 失败", self.mel.sample_rate)
            })?
        };

        // 2. 提取 128 bin log-Mel
        let mel = self.mel_16k(&audio_16k)?;
        let n_frames = mel.n_frames;
        if n_frames == 0 {
            return Ok(F0Track::empty(self.frame_period_ms));
        }

        // 3. 推理：得到 cent 分类 latent
        let (out_dims, latent) = self.inner.infer(&mel.data, n_frames, mel.n_mels)?;
        if out_dims == 0 || latent.len() != n_frames * out_dims {
            anyhow::bail!(
                "FCPE 输出形状异常: latent {} 帧数 {n_frames} 类数 {out_dims}",
                latent.len()
            );
        }

        // 4. latent → cent → f0
        let cent_table = if out_dims == self.cent_table.len() {
            self.cent_table.clone()
        } else {
            log::warn!("FCPE 模型类数 {out_dims} 与配置不符，按模型实际类数重建 cent 表");
            build_cent_table(
                crate::config::F0Config::default().cent_f0_min,
                crate::config::F0Config::default().cent_f0_max,
                out_dims,
            )
        };
        let cents = self.latent_to_cents(&latent, out_dims, &cent_table);
        let f0 = self.post_process(&cents);

        Ok(F0Track::new(f0, self.frame_period_ms))
    }
}

#[cfg(feature = "onnx")]
fn create_runtime(model_path: &std::path::Path) -> Result<Box<dyn FcpeRuntime + Send + Sync>> {
    use ort::session::Session;
    use ort::value::Tensor;
    use std::sync::Mutex;

    if !model_path.exists() {
        return Err(BackendUnavailable(format!("FCPE 模型不存在: {model_path:?}")).into());
    }

    struct OrtRuntime {
        session: Mutex<Session>,
        input_name: String,
        output_name: String,
    }

    impl FcpeRuntime for OrtRuntime {
        fn name(&self) -> &str {
            "fcpe-onnx"
        }

        fn infer(&self, mel: &[f32], n_frames: usize, n_mels: usize) -> Result<(usize, Vec<f32>)> {
            let tensor =
                Tensor::from_array((vec![1i64, n_frames as i64, n_mels as i64], mel.to_vec()))
                    .context("构造 FCPE 输入张量失败")?;
            let mut session = self
                .session
                .lock()
                .map_err(|e| anyhow::anyhow!("会话锁损坏: {e}"))?;
            let outputs = session
                .run(ort::inputs![self.input_name.as_str() => tensor])
                .context("FCPE 推理失败")?;
            let value = &outputs[self.output_name.as_str()];
            let (shape, data) = value
                .try_extract_tensor::<f32>()
                .context("提取 FCPE 输出失败")?;
            // 形状为 [1, T, out_dims]
            let out_dims = shape.last().copied().unwrap_or(0) as usize;
            Ok((out_dims, data.to_vec()))
        }
    }

    let mut builder =
        Session::builder().map_err(|e| anyhow::anyhow!("创建 ONNX 会话构造器失败: {e}"))?;
    builder = builder
        .with_execution_providers([ort::execution_providers::CPU::default().build()])
        .map_err(|e| anyhow::anyhow!("注册 CPU 执行提供者失败: {e}"))?;
    let session = builder
        .commit_from_file(model_path)
        .map_err(|e| anyhow::anyhow!("载入 FCPE 模型失败 {model_path:?}: {e}"))?;

    let input_name = session
        .inputs()
        .first()
        .map(|i| i.name().to_string())
        .unwrap_or_else(|| "mel".to_string());
    let output_name = session
        .outputs()
        .first()
        .map(|o| o.name().to_string())
        .unwrap_or_else(|| "f0".to_string());
    log::info!("FCPE 模型已载入: 输入 `{input_name}`，输出 `{output_name}`");

    Ok(Box::new(OrtRuntime {
        session: Mutex::new(session),
        input_name,
        output_name,
    }))
}

#[cfg(not(feature = "onnx"))]
fn create_runtime(model_path: &std::path::Path) -> Result<Box<dyn FcpeRuntime + Send + Sync>> {
    let _ = model_path;
    Err(BackendUnavailable("编译未启用 onnx feature".into()).into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mel_config_from_resampler_config() {
        let cfg = ResamplerConfig::default_yaml().unwrap();
        let fcpe = FcpeMelConfig::from_config(&cfg);
        assert_eq!(fcpe.sample_rate, 16000);
        assert_eq!(fcpe.hop_size, 160);
        assert_eq!(fcpe.n_mels, 128);
    }

    #[test]
    fn mel_16k_frame_count() {
        let cfg = ResamplerConfig::default_yaml().unwrap();
        // 无法保证模型存在，仅测试 Mel 计算部分
        let fcpe_cfg = FcpeMelConfig::from_config(&cfg);
        let mel_cfg = fcpe_cfg.to_mel_config();
        let audio = vec![0.0f32; 16000];
        let mel = crate::core::feature::compute_mel(&audio, &mel_cfg, 160, 0.0).unwrap();
        assert!(
            mel.n_frames >= 98 && mel.n_frames <= 102,
            "帧数异常: {}",
            mel.n_frames
        );
        assert_eq!(mel.n_mels, 128);
    }

    /// cent 表与官方权重导出的 `cent_table.bin` 一致（360 类，C1~B6）。
    #[test]
    fn cent_table_matches_official_weights() {
        let t = build_cent_table(32.70, 1975.5, 360);
        assert_eq!(t.len(), 360);
        // 参考值取自 HachiTune 仓库 models/cent_table.bin（float32）
        assert!((t[0] - 2051.1487).abs() < 0.5, "cent[0] = {}", t[0]);
        assert!((t[359] - 9151.289).abs() < 0.5, "cent[359] = {}", t[359]);
        let step = t[1] - t[0];
        assert!((step - 19.777588).abs() < 0.01, "step = {step}");
        // 端点换算回 Hz 应落在 C1 / B6
        let f0_lo = 10.0 * 2f32.powf(t[0] / 1200.0);
        let f0_hi = 10.0 * 2f32.powf(t[359] / 1200.0);
        assert!((f0_lo - 32.70).abs() < 0.1, "f0_min = {f0_lo}");
        assert!((f0_hi - 1975.5).abs() < 0.5, "f0_max = {f0_hi}");
    }

    #[test]
    fn missing_model_is_reported() {
        let mut cfg = ResamplerConfig::default_yaml().unwrap();
        cfg.f0.model = PathBuf::from("models/__definitely_missing__.onnx");
        let err = FcpeExtractor::new(&cfg);
        assert!(err.is_err());
    }
}
