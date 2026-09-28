//! F0 提取层
//!
//! 参考 pitch-core 的设计：把「纯 DSP 后端」与「ONNX 后端」拆成独立实现，
//! 通过 [`F0Extractor`] trait 统一，运行时可回退。

pub mod fcpe;
pub mod world;

use crate::config::{F0Backend, ResamplerConfig};
use anyhow::Result;
use std::path::{Path, PathBuf};

/// 一条 F0 曲线（逐帧，单位 Hz；0 表示清音）。
#[derive(Debug, Clone, PartialEq)]
pub struct F0Track {
    /// 每帧 F0（Hz），0 = 清音。
    pub f0: Vec<f32>,
    /// 帧移（毫秒）。
    pub frame_period_ms: f32,
}

impl F0Track {
    pub fn new(f0: Vec<f32>, frame_period_ms: f32) -> Self {
        Self {
            f0,
            frame_period_ms,
        }
    }

    /// 空曲线。
    pub fn empty(frame_period_ms: f32) -> Self {
        Self {
            f0: Vec::new(),
            frame_period_ms,
        }
    }

    pub fn n_frames(&self) -> usize {
        self.f0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.f0.is_empty()
    }

    pub fn is_voiced(&self, i: usize) -> bool {
        matches!(self.f0.get(i), Some(v) if *v > 0.0)
    }

    /// 浊音帧的清浊掩码（true = 浊音）。
    pub fn voice_mask(&self) -> Vec<bool> {
        self.f0.iter().map(|&v| v > 0.0).collect()
    }

    /// 浊音帧比例。
    pub fn voiced_ratio(&self) -> f32 {
        if self.f0.is_empty() {
            return 0.0;
        }
        let voiced = self.f0.iter().filter(|&&v| v > 0.0).count();
        voiced as f32 / self.f0.len() as f32
    }

    /// 浊音帧均值（无浊音帧时返回 `fallback`）。
    pub fn mean_voiced(&self, fallback: f32) -> f32 {
        let voiced: Vec<f32> = self.f0.iter().copied().filter(|&v| v > 0.0).collect();
        if voiced.is_empty() {
            return fallback;
        }
        voiced.iter().sum::<f32>() / voiced.len() as f32
    }

    /// 浊音帧中位数（无浊音帧时返回 `fallback`）。
    pub fn median_voiced(&self, fallback: f32) -> f32 {
        let mut voiced: Vec<f32> = self.f0.iter().copied().filter(|&v| v > 0.0).collect();
        if voiced.is_empty() {
            return fallback;
        }
        voiced.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        voiced[voiced.len() / 2]
    }

    /// 时间轴重采样到目标帧移（毫秒）。
    pub fn resample_to(&self, target_frame_period_ms: f32) -> F0Track {
        if self.is_empty() || target_frame_period_ms <= 0.0 || self.frame_period_ms <= 0.0 {
            return F0Track {
                f0: self.f0.clone(),
                frame_period_ms: target_frame_period_ms,
            };
        }
        if (self.frame_period_ms - target_frame_period_ms).abs() < 1e-6 {
            return self.clone();
        }
        let ratio = self.frame_period_ms / target_frame_period_ms;
        let out_len = ((self.f0.len() as f32) * ratio).round().max(1.0) as usize;
        let f0 = crate::core::feature::interp_linear(&self.f0, out_len);
        F0Track {
            f0,
            frame_period_ms: target_frame_period_ms,
        }
    }

    /// Hz → MIDI 曲线（清音帧保持 0）。
    pub fn to_midi(&self) -> Vec<f32> {
        self.f0
            .iter()
            .map(|&f| {
                if f > 0.0 {
                    crate::core::protocol::hz_to_midi(f)
                } else {
                    0.0
                }
            })
            .collect()
    }
}

/// F0 提取器统一接口。
pub trait F0Extractor: Send + Sync {
    /// 后端名称（日志/诊断用）。
    fn name(&self) -> &str;

    /// 分析用帧移（毫秒）。
    fn frame_period_ms(&self) -> f32;

    /// 从单声道音频中提取 F0（Hz）。
    fn extract(&self, audio: &[f32], sample_rate: u32) -> Result<F0Track>;
}

/// 后端不可用（模型缺失或 feature 未启用）。
#[derive(Debug, thiserror::Error)]
#[error("F0 后端不可用: {0}")]
pub struct BackendUnavailable(pub String);

/// 按配置创建 F0 提取器。
///
/// `Auto` 顺序：FCPE（需要 ONNX 模型与 `onnx` feature）→ 内置 DSP（WORLD 风格）→ None。
pub fn create_extractor(cfg: &ResamplerConfig) -> Result<Option<Box<dyn F0Extractor>>> {
    let wanted = cfg.f0.backend;
    match wanted {
        F0Backend::None => {
            log::info!("F0 后端配置为 none，F0 完全由乐谱决定");
            Ok(None)
        }
        F0Backend::Fcpe => Ok(Some(Box::new(fcpe::FcpeExtractor::new(cfg)?))),
        F0Backend::World => Ok(Some(Box::new(world::WorldExtractor::new(cfg)))),
        F0Backend::Auto => match fcpe::FcpeExtractor::new(cfg) {
            Ok(e) => {
                log::info!("F0 后端：FCPE (ONNX)");
                Ok(Some(Box::new(e)))
            }
            Err(err) => {
                log::warn!("FCPE 不可用（{err}），回退到内置 DSP 后端");
                Ok(Some(Box::new(world::WorldExtractor::new(cfg))))
            }
        },
    }
}

/// FCPE 模型路径（不存在时返回 None）。
pub fn fcpe_model_path(cfg: &ResamplerConfig) -> Option<PathBuf> {
    let p = cfg.model_path(&cfg.f0.model);
    if p.exists() {
        Some(p)
    } else {
        None
    }
}

/// 检查某路径是否为可用的模型文件。
pub fn model_exists(p: &Path) -> bool {
    p.exists() && p.is_file()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn track_stats() {
        let t = F0Track::new(vec![0.0, 220.0, 440.0, 0.0, 660.0], 10.0);
        assert_eq!(t.n_frames(), 5);
        assert_eq!(t.voiced_ratio(), 0.6);
        assert!((t.mean_voiced(0.0) - 440.0).abs() < 1e-5);
        assert!((t.median_voiced(0.0) - 440.0).abs() < 1e-5);
        assert!(t.is_voiced(2));
        assert!(!t.is_voiced(0));
    }

    #[test]
    fn resample_doubles_frames() {
        let t = F0Track::new(vec![100.0, 200.0, 300.0], 10.0);
        let r = t.resample_to(5.0);
        assert_eq!(r.n_frames(), 6);
        assert_eq!(r.frame_period_ms, 5.0);
        assert!((r.f0[0] - 100.0).abs() < 1e-5);
    }

    #[test]
    fn to_midi_conversion() {
        let t = F0Track::new(vec![440.0, 0.0], 10.0);
        let m = t.to_midi();
        assert!((m[0] - 69.0).abs() < 1e-4);
        assert_eq!(m[1], 0.0);
    }
}
