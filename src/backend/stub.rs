//! 离线自测用的内置后端
//!
//! 当声码器 ONNX 模型缺失时使用：由 F0 做加性合成、由 Mel 能量做包络，
//! 生成一个确定性的、可听的近似波形。
//!
//! **该后端不具备 PC-NSF-HiFiGAN 的音质，仅用于管线连通性验证，切勿用于实际作品。**

use crate::backend::InferenceBackend;
use anyhow::Result;
use std::f32::consts::PI;

/// 内置回声后端。
#[derive(Debug, Clone, Default)]
pub struct StubBackend {
    /// 输出采样率。
    pub sample_rate: u32,
    /// 声码器帧移（样本），0 表示使用默认 512。
    pub hop_size: usize,
}

impl StubBackend {
    fn hop(&self) -> usize {
        if self.hop_size == 0 {
            512
        } else {
            self.hop_size
        }
    }
}

impl InferenceBackend for StubBackend {
    fn name(&self) -> &str {
        "stub"
    }

    fn input_names(&self) -> (String, String) {
        ("mel".to_string(), "f0".to_string())
    }

    fn output_name(&self) -> String {
        "waveform".to_string()
    }

    fn infer(&self, mel: &[f32], n_frames: usize, n_mels: usize, f0: &[f32]) -> Result<Vec<f32>> {
        let hop = self.hop();
        let n_samples = n_frames * hop;
        let mut out = vec![0.0f32; n_samples];
        let mut phase = 0.0f32;

        for t in 0..n_frames {
            let freq = f0.get(t).copied().unwrap_or(0.0);
            // Mel 能量作为该帧的幅度包络（mel 为 log 域，先还原到线性近似）
            let frame = &mel[t * n_mels..(t + 1) * n_mels];
            let energy = frame.iter().map(|&v| v.exp()).sum::<f32>() / n_mels.max(1) as f32;
            let amp = (energy.sqrt() * 0.25).clamp(0.0, 1.0);

            for i in 0..hop {
                let idx = t * hop + i;
                if idx >= out.len() {
                    break;
                }
                if freq > 0.0 {
                    phase += 2.0 * PI * freq / 44100.0;
                    if phase > 2.0 * PI {
                        phase -= 2.0 * PI;
                    }
                    // 基频 + 二次谐波，避免过于刺耳
                    out[idx] = amp * (phase.sin() * 0.8 + (2.0 * phase).sin() * 0.2);
                } else {
                    // 清音：极弱的伪噪声（确定性）
                    out[idx] =
                        amp * 0.05 * (((idx * 1103515245 + 12345) % 1000) as f32 / 1000.0 - 0.5);
                }
            }
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn produces_audio_of_expected_length() {
        let b = StubBackend::default();
        let n_frames = 10;
        let n_mels = 128;
        let mel = vec![0.5f32; n_frames * n_mels];
        let f0 = vec![440.0f32; n_frames];
        let out = b.infer(&mel, n_frames, n_mels, &f0).unwrap();
        assert_eq!(out.len(), n_frames * 512);
        assert!(out.iter().any(|&s| s.abs() > 1e-6));
    }

    #[test]
    fn silent_when_f0_unvoiced() {
        let b = StubBackend::default();
        let n_frames = 4;
        let n_mels = 8;
        let mel = vec![-10.0f32; n_frames * n_mels];
        let f0 = vec![0.0f32; n_frames];
        let out = b.infer(&mel, n_frames, n_mels, &f0).unwrap();
        assert!(out.iter().all(|&s| s.abs() < 0.05));
    }
}
