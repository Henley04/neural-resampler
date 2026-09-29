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
///
/// 使用单调推进的帧索引（双指针）逐样本插值，复杂度 O(样本数 + 帧数)。
/// 之前对每个样本调用 `sample_at(&[t], ..)`：每次调用都会重置扫描索引并
/// 分配一个临时 Vec，整体退化为 O(样本数 × 帧数)，长音符 + `A` 标记时
/// 开销显著放大。
pub fn amplitude_modulate(
    samples: &mut [f32],
    frame_times: &[f64],
    frame_gains: &[f32],
    sample_rate: u32,
) {
    if samples.is_empty() || frame_times.is_empty() || frame_gains.is_empty() {
        return;
    }
    if frame_times.len() == 1 {
        let g = frame_gains[0];
        for s in samples.iter_mut() {
            *s *= g;
        }
        return;
    }
    let last_time = frame_times[frame_times.len() - 1];
    let mut idx = 0usize;
    for (i, s) in samples.iter_mut().enumerate() {
        let t = i as f64 / sample_rate as f64;
        if t <= frame_times[0] {
            *s *= frame_gains[0];
            continue;
        }
        if t >= last_time {
            *s *= frame_gains[frame_gains.len() - 1];
            continue;
        }
        while idx + 1 < frame_times.len() && frame_times[idx + 1] < t {
            idx += 1;
        }
        let t0 = frame_times[idx];
        let t1 = frame_times[idx + 1];
        let frac = if (t1 - t0).abs() < 1e-12 {
            0.0
        } else {
            ((t - t0) / (t1 - t0)) as f32
        };
        let v0 = frame_gains[idx.min(frame_gains.len() - 1)];
        let v1 = frame_gains[(idx + 1).min(frame_gains.len() - 1)];
        *s *= v0 * (1.0 - frac) + v1 * frac;
    }
}

/// 统一后处理入口：重采样 → 长度对齐 → 响度归一化 → 限幅 → 音量 → 淡入淡出。
///
/// 响度归一化、峰值限幅与音量缩放**只在这里发生一次**。
/// `norm_strength` 来自 `P` 标记（0~1，0 表示不做归一化，含 `wave_norm=false`）；
/// `volume_percent` 来自 UTAU 的 `V` 参数（100 = 不变）。
///
/// 此前这两步分散在管线与 `finalize` 中各执行一次：第二次全量归一化会把
/// `P` 的强度插值覆盖掉，也会把 `V` 的音量缩放抵消回目标响度。
pub fn finalize(
    mut buffer: AudioBuffer,
    cfg: &OutputConfig,
    target_samples: usize,
    norm_strength: f32,
    volume_percent: f64,
) -> Result<AudioBuffer> {
    if buffer.sample_rate != cfg.sample_rate && !buffer.is_empty() {
        let resampled = resample_sinc(&buffer.samples, buffer.sample_rate, cfg.sample_rate)?;
        buffer = AudioBuffer::new(resampled, cfg.sample_rate);
    }
    let mut samples = fit_length(&buffer.samples, target_samples);

    // 响度归一化：P 标记控制强度，strength < 1 时在原始与归一化结果之间插值
    if norm_strength > 0.0 {
        if norm_strength < 1.0 {
            let original = samples.clone();
            loudness_norm(
                &mut samples,
                cfg.sample_rate,
                cfg.loudness_target,
                cfg.loudness_block_ms,
            );
            for (s, o) in samples.iter_mut().zip(original.iter()) {
                *s = o * (1.0 - norm_strength) + *s * norm_strength;
            }
        } else {
            loudness_norm(
                &mut samples,
                cfg.sample_rate,
                cfg.loudness_target,
                cfg.loudness_block_ms,
            );
        }
    }
    // 峰值限幅放在归一化之后：归一化可能把峰值拉到目标响度之上
    limit_peak(&mut samples, cfg.peak_limit);
    if volume_percent != 100.0 {
        let gain = (volume_percent / 100.0) as f32;
        for s in samples.iter_mut() {
            *s *= gain;
        }
    }
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
        let out = finalize(buf, &cfg, 44100, 1.0, 100.0).unwrap();
        assert_eq!(out.sample_rate, 44100);
        assert_eq!(out.len(), 44100);
    }

    #[test]
    fn finalize_applies_volume_exactly_once() {
        // 音量缩放不能被（第二次）归一化抵消：volume=50 的峰值应约为 100 的一半。
        // 目标响度调低 + 输入幅度小，保证两次都不会触发峰值限幅。
        let cfg = OutputConfig {
            wave_norm: true,
            loudness_target: -30.0,
            peak_limit: 4.0,
            ..OutputConfig::default()
        };
        let input = vec![0.05f32; 44100];
        let full = finalize(
            AudioBuffer::new(input.clone(), 44100),
            &cfg,
            44100,
            1.0,
            100.0,
        )
        .unwrap()
        .peak();
        let half = finalize(AudioBuffer::new(input, 44100), &cfg, 44100, 1.0, 50.0)
            .unwrap()
            .peak();
        let ratio = half / full;
        assert!(
            (ratio - 0.5).abs() < 0.02,
            "音量缩放被抵消：期望 ≈0.5，实际 {ratio}"
        );
    }

    #[test]
    fn finalize_respects_norm_strength() {
        let cfg = OutputConfig {
            loudness_target: -20.0,
            ..OutputConfig::default()
        };
        // strength = 0：完全不归一化，输出幅度与输入一致
        let raw = finalize(
            AudioBuffer::new(vec![0.1f32; 44100], 44100),
            &cfg,
            44100,
            0.0,
            100.0,
        )
        .unwrap();
        assert!((raw.peak() - 0.1).abs() < 1e-4, "strength=0 不应归一化");
        // strength = 1：完整归一化，RMS 应贴近目标。
        // 输入 -26dBFS（0.05），到 -20dBFS 只需约 2 倍增益，不会触发
        // loudness_norm 内部 0.05~20 的增益钳制。
        let normed = finalize(
            AudioBuffer::new(vec![0.05f32; 44100], 44100),
            &cfg,
            44100,
            1.0,
            100.0,
        )
        .unwrap();
        let rms: f64 = normed
            .samples
            .iter()
            .map(|&v| (v as f64) * (v as f64))
            .sum::<f64>()
            / normed.len() as f64;
        let db = 20.0 * rms.sqrt().log10();
        assert!(
            (db - cfg.loudness_target as f64).abs() < 1.5,
            "strength=1 应归一化到目标附近: {db} dB"
        );
    }

    #[test]
    fn amplitude_modulate_matches_sample_at() {
        // 优化后的双指针实现必须与逐点调用 sample_at 的结果逐点一致。
        let frame_times: Vec<f64> = (0..=40).map(|i| i as f64 * 0.025).collect();
        let frame_gains: Vec<f32> = (0..40).map(|i| 0.5 + i as f32 * 0.05).collect();
        let sr = 8000u32;
        let n = 1000usize;
        let mut a: Vec<f32> = (0..n).map(|i| (i as f32 * 0.01).sin() + 1.5).collect();
        let b = a.clone();
        amplitude_modulate(&mut a, &frame_times, &frame_gains, sr);
        for (i, s) in b.iter().enumerate() {
            let t = i as f64 / sr as f64;
            let expect = s * crate::core::feature::sample_at(&[t], &frame_times, &frame_gains)[0];
            assert!((a[i] - expect).abs() < 1e-5, "t={t}: {} vs {expect}", a[i]);
        }
    }

    #[test]
    fn amplitude_modulate_handles_degenerate_input() {
        let mut s = vec![1.0f32; 10];
        // 单帧增益
        amplitude_modulate(&mut s, &[0.0], &[0.5], 1000);
        assert!(s.iter().all(|&v| (v - 0.5).abs() < 1e-6));
        // 空增益
        let mut s2 = vec![1.0f32; 10];
        amplitude_modulate(&mut s2, &[0.0, 1.0], &[], 1000);
        assert!(s2.iter().all(|&v| (v - 1.0).abs() < 1e-6));
    }
}
