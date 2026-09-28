//! 后处理：长度对齐、响度归一化、限幅、淡入淡出、growl 效果。

use crate::config::OutputConfig;
use crate::core::audio::{resample_sinc, AudioBuffer};
use anyhow::Result;
use std::f32::consts::PI;

/// 把波形裁剪/补零到精确的目标样本数。
pub fn fit_length(samples: &[f32], target: usize) -> Vec<f32> {
    if samples.len() == target {
        return samples.to_vec();
    }
    let mut out = vec![0.0f32; target];
    let n = samples.len().min(target);
    out[..n].copy_from_slice(&samples[..n]);
    out
}

/// 淡入淡出（毫秒）。
pub fn apply_fade(samples: &mut [f32], sample_rate: u32, fade_in_ms: f32, fade_out_ms: f32) {
    let n_in = ((fade_in_ms / 1000.0) * sample_rate as f32).round() as usize;
    let n_out = ((fade_out_ms / 1000.0) * sample_rate as f32).round() as usize;
    let len = samples.len();
    let n_in = n_in.min(len);
    let n_out = n_out.min(len);
    for (i, s) in samples[..n_in].iter_mut().enumerate() {
        *s *= i as f32 / n_in.max(1) as f32;
    }
    for (i, s) in samples[len - n_out..].iter_mut().enumerate() {
        *s *= (n_out - i - 1) as f32 / n_out.max(1) as f32;
    }
}

/// RMS 响度归一化（block 分块平滑，避免瞬态被压掉）。
///
/// 这是 LUFS 的简化近似：以分块 RMS 的中高分位数代表整体响度，
/// 再整体缩放到 `target_db`。
pub fn loudness_norm(samples: &mut [f32], sample_rate: u32, target_db: f32, block_ms: f32) {
    if samples.is_empty() {
        return;
    }
    let block = ((block_ms / 1000.0) * sample_rate as f32).round().max(1.0) as usize;
    let mut block_rms: Vec<f32> = Vec::new();
    let mut pos = 0usize;
    while pos < samples.len() {
        let end = (pos + block).min(samples.len());
        let sum: f64 = samples[pos..end]
            .iter()
            .map(|&s| (s as f64) * (s as f64))
            .sum();
        block_rms.push(((sum / (end - pos) as f64).sqrt()) as f32);
        pos = end;
    }
    block_rms.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let idx = ((block_rms.len() as f32) * 0.8) as usize; // 80 分位
    let reference = block_rms[idx.min(block_rms.len() - 1)].max(1e-6);
    let current_db = 20.0 * reference.log10();
    let gain = 10f32.powf((target_db - current_db) / 20.0);
    let gain = gain.clamp(0.05, 20.0);
    for s in samples.iter_mut() {
        *s *= gain;
    }
}

/// 峰值限幅：超过 `limit` 时整体缩放。
pub fn limit_peak(samples: &mut [f32], limit: f32) {
    if samples.is_empty() || limit <= 0.0 {
        return;
    }
    let peak = samples.iter().fold(0.0f32, |m, &s| m.max(s.abs()));
    if peak > limit && peak > 1e-9 {
        let g = limit / peak;
        for s in samples.iter_mut() {
            *s *= g;
        }
    }
}

/// growl 效果：以固定频率做幅度调制（对应 HiFiSampler 的 `HG` 标记）。
pub fn growl(samples: &mut [f32], sample_rate: u32, frequency: f32, strength: f32) {
    if samples.is_empty() {
        return;
    }
    let strength = strength.clamp(0.0, 1.0);
    for (i, s) in samples.iter_mut().enumerate() {
        let t = i as f32 / sample_rate as f32;
        let lfo = 1.0 - strength * 0.5 * (1.0 - (2.0 * PI * frequency * t).cos());
        *s *= lfo;
    }
}

/// 振幅调制（对应 `A` 标记）：以每帧增益曲线插值到样本级。
pub fn amplitude_modulate(
    samples: &mut [f32],
    frame_times: &[f64],
    frame_gains: &[f32],
    sample_rate: u32,
) {
    if samples.is_empty() || frame_times.len() < 2 || frame_gains.is_empty() {
        return;
    }
    for (i, s) in samples.iter_mut().enumerate() {
        let t = i as f64 / sample_rate as f64;
        let g = crate::core::feature::sample_at(&[t], frame_times, frame_gains)[0];
        *s *= g;
    }
}

/// 统一后处理入口：重采样 → 长度对齐 → 响度归一化 → 限幅 → 淡入淡出。
pub fn finalize(
    mut buffer: AudioBuffer,
    cfg: &OutputConfig,
    target_samples: usize,
) -> Result<AudioBuffer> {
    if buffer.sample_rate != cfg.sample_rate && !buffer.is_empty() {
        let resampled = resample_sinc(&buffer.samples, buffer.sample_rate, cfg.sample_rate)?;
        buffer = AudioBuffer::new(resampled, cfg.sample_rate);
    }
    let mut samples = fit_length(&buffer.samples, target_samples);

    if cfg.wave_norm {
        loudness_norm(
            &mut samples,
            cfg.sample_rate,
            cfg.loudness_target,
            cfg.loudness_block_ms,
        );
    }
    limit_peak(&mut samples, cfg.peak_limit);
    if cfg.fade_in_ms > 0.0 || cfg.fade_out_ms > 0.0 {
        apply_fade(
            &mut samples,
            cfg.sample_rate,
            cfg.fade_in_ms,
            cfg.fade_out_ms,
        );
    }
    Ok(AudioBuffer::new(samples, cfg.sample_rate))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fit_length_pads_and_trims() {
        assert_eq!(fit_length(&[1.0, 2.0], 4), vec![1.0, 2.0, 0.0, 0.0]);
        assert_eq!(fit_length(&[1.0, 2.0, 3.0], 2), vec![1.0, 2.0]);
    }

    #[test]
    fn fade_edges_are_zero() {
        let mut s = vec![1.0f32; 100];
        apply_fade(&mut s, 1000, 10.0, 10.0);
        assert_eq!(s[0], 0.0);
        assert_eq!(s[99], 0.0);
        assert!(s[50] > 0.99);
    }

    #[test]
    fn loudness_normalizes_towards_target() {
        let mut s: Vec<f32> = (0..44100)
            .map(|i| (i as f32 / 100.0).sin() * 0.01)
            .collect();
        loudness_norm(&mut s, 44100, -16.0, 400.0);
        let rms: f64 = s.iter().map(|&v| (v as f64) * (v as f64)).sum::<f64>() / s.len() as f64;
        let db = 20.0 * rms.sqrt().log10();
        assert!(db > -22.0 && db < -10.0, "归一化后响度异常: {db} dB");
    }

    #[test]
    fn peak_limit_caps() {
        let mut s = vec![2.0f32, -3.0, 0.5];
        limit_peak(&mut s, 1.0);
        assert!(s.iter().all(|&v| v.abs() <= 1.0 + 1e-6));
    }

    #[test]
    fn finalize_resamples_and_fits() {
        let cfg = OutputConfig::default();
        let buf = AudioBuffer::new(vec![0.5f32; 2205], 22050);
        let out = finalize(buf, &cfg, 44100).unwrap();
        assert_eq!(out.sample_rate, 44100);
        assert_eq!(out.len(), 44100);
    }
}
