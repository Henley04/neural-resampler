//! Mel 频谱特征提取
//!
//! 与 PC-NSF-HiFiGAN / FCPE 的训练前处理严格对齐
//! （`librosa.filters.mel` + `torch.stft(center=False)`）：
//! STFT（周期 Hann 窗 + 反射填充）→ 幅度谱 → Slaney Mel 滤波器组（面积归一化）
//! → 自然对数动态范围压缩。

use crate::config::{MelConfig, MelScale};

/// Hann 窗（周期形式，等价于 `torch.hann_window(n)`）。
pub fn hann_window(size: usize) -> Vec<f32> {
    (0..size)
        .map(|i| 0.5 * (1.0 - (2.0 * std::f32::consts::PI * i as f32 / size as f32).cos()))
        .collect()
}

/// 频率 → Mel（Slaney，librosa 默认）。
pub fn hz_to_mel_slaney(hz: f32) -> f32 {
    slaney::hz_to_mel(hz)
}

/// Mel → 频率（Slaney）。
pub fn mel_to_hz_slaney(mel: f32) -> f32 {
    slaney::mel_to_hz(mel)
}

/// 频率 → Mel（HTK）。
pub fn hz_to_mel_htk(hz: f32) -> f32 {
    2595.0 * (1.0 + hz / 700.0).log10()
}

/// Mel → 频率（HTK）。
pub fn mel_to_hz_htk(mel: f32) -> f32 {
    700.0 * (10f32.powf(mel / 2595.0) - 1.0)
}

/// Slaney mel 标度（librosa `_mel_to_hz` / `hz_to_mel` 的实现）。
mod slaney {
    const F_SP: f32 = 200.0 / 3.0;
    const MIN_LOG_HZ: f32 = 1000.0;
    const MIN_LOG_MEL: f32 = 15.0; // 1000.0 / (200.0/3)
    /// `ln(6.4) / 27.0`
    const LOGSTEP: f32 = 0.068_751_78;

    pub fn hz_to_mel(hz: f32) -> f32 {
        if hz >= MIN_LOG_HZ {
            MIN_LOG_MEL + (hz / MIN_LOG_HZ).ln() / LOGSTEP
        } else {
            hz / F_SP
        }
    }

    pub fn mel_to_hz(mel: f32) -> f32 {
        if mel >= MIN_LOG_MEL {
            MIN_LOG_HZ * (LOGSTEP * (mel - MIN_LOG_MEL)).exp()
        } else {
            mel * F_SP
        }
    }
}

/// 构建 librosa 风格的 Mel 滤波器组，返回 `[n_mels][n_freqs]`。
pub fn mel_filterbank(
    sample_rate: u32,
    n_fft: usize,
    n_mels: usize,
    fmin: f32,
    fmax: f32,
    scale: MelScale,
) -> Vec<Vec<f32>> {
    let n_freqs = n_fft / 2 + 1;
    let fmax = if fmax <= 0.0 {
        sample_rate as f32 / 2.0
    } else {
        fmax
    };
    type MelFns = (fn(f32) -> f32, fn(f32) -> f32);
    let (hz_to_mel, mel_to_hz): MelFns = match scale {
        MelScale::Slaney => (hz_to_mel_slaney, mel_to_hz_slaney),
        MelScale::Htk => (hz_to_mel_htk, mel_to_hz_htk),
    };

    let mel_min = hz_to_mel(fmin.max(0.0));
    let mel_max = hz_to_mel(fmax);

    // n_mels + 2 个等距 mel 点
    let points: Vec<f32> = (0..n_mels + 2)
        .map(|i| mel_to_hz(mel_min + (mel_max - mel_min) * i as f32 / (n_mels + 1) as f32))
        .collect();

    // 各 STFT bin 的中心频率
    let freqs: Vec<f32> = (0..n_freqs)
        .map(|i| i as f32 * sample_rate as f32 / n_fft as f32)
        .collect();

    let mut filters = vec![vec![0.0f32; n_freqs]; n_mels];
    for (m, filter) in filters.iter_mut().enumerate() {
        let left = points[m];
        let center = points[m + 1];
        let right = points[m + 2];
        // Slaney 归一化：按三角窗面积（2 / 带宽）
        let norm = 2.0 / (right - left).max(1e-10);
        for (i, &f) in freqs.iter().enumerate() {
            let lower = if center > left {
                (f - left) / (center - left)
            } else {
                f32::NEG_INFINITY
            };
            let upper = if right > center {
                (right - f) / (right - center)
            } else {
                f32::NEG_INFINITY
            };
            let v = lower.min(upper).max(0.0);
            if v > 0.0 {
                filter[i] = norm * v;
            }
        }
    }
    filters
}

/// 反射填充。
pub fn reflect_pad(samples: &[f32], left: usize, right: usize) -> Vec<f32> {
    if samples.is_empty() {
        return vec![0.0; left + right];
    }
    let mut out = Vec::with_capacity(samples.len() + left + right);
    for i in (0..left).rev() {
        let idx = if i + 1 < samples.len() {
            i + 1
        } else {
            samples.len() - 1
        };
        out.push(samples[idx]);
    }
    out.extend_from_slice(samples);
    for i in 0..right {
        let idx = if samples.len() > i + 1 {
            samples.len() - i - 2
        } else {
            0
        };
        out.push(samples[idx]);
    }
    out
}

/// STFT 幅度谱，返回 `[n_frames][n_fft/2+1]`。
///
/// 与 `torch.stft(..., center=False)` 一致：先做 `(win-hop)//2` / `(win-hop+1)//2`
/// 的反射填充，再按 `1 + (len - win) // hop` 取帧。
/// `eps > 0` 时计算 `sqrt(re² + im² + eps)`（FCPE 用 1e-9），否则等价 `.abs()`。
pub fn stft_magnitude(
    samples: &[f32],
    n_fft: usize,
    win_size: usize,
    hop_size: usize,
    eps: f32,
) -> anyhow::Result<Vec<Vec<f32>>> {
    use realfft::{num_complex::Complex, RealFftPlanner};

    if samples.is_empty() || hop_size == 0 || win_size == 0 {
        return Ok(Vec::new());
    }
    let pad_left = (win_size - hop_size) / 2;
    let pad_right = (win_size - hop_size).div_ceil(2);
    let padded = reflect_pad(samples, pad_left, pad_right);

    let n_frames = if padded.len() >= win_size {
        (padded.len() - win_size) / hop_size + 1
    } else {
        0
    };

    let mut planner = RealFftPlanner::<f32>::new();
    let fft = planner.plan_fft_forward(n_fft.max(win_size));
    let window = hann_window(win_size);

    let mut input = vec![0.0f32; n_fft.max(win_size)];
    let mut output = vec![Complex::<f32>::default(); n_fft.max(win_size) / 2 + 1];
    let mut frames = Vec::with_capacity(n_frames);

    for f in 0..n_frames {
        let start = f * hop_size;
        input.fill(0.0);
        for (i, &w) in window.iter().enumerate() {
            input[i] = padded[start + i] * w;
        }
        fft.process(&mut input, &mut output)?;
        frames.push(
            output
                .iter()
                .map(|c| {
                    let p = c.re * c.re + c.im * c.im;
                    if eps > 0.0 {
                        (p + eps).sqrt()
                    } else {
                        p.sqrt()
                    }
                })
                .collect::<Vec<f32>>(),
        );
    }
    Ok(frames)
}

/// Mel 频谱：[帧][bin] 布局（`mel[frame][bin]`）。
#[derive(Debug, Clone)]
pub struct MelSpectrogram {
    pub data: Vec<f32>,
    pub n_frames: usize,
    pub n_mels: usize,
    pub hop_size: usize,
    pub sample_rate: u32,
}

impl MelSpectrogram {
    pub fn frame(&self, i: usize) -> &[f32] {
        let start = i * self.n_mels;
        &self.data[start..start + self.n_mels]
    }

    /// 帧移（秒）。
    pub fn frame_period(&self) -> f64 {
        self.hop_size as f64 / self.sample_rate as f64
    }

    /// 按帧线性重采样（时间轴拉伸/压缩）。
    pub fn resample_frames(&self, target_frames: usize) -> MelSpectrogram {
        if self.n_frames == 0 || target_frames == 0 {
            return MelSpectrogram {
                data: vec![0.0; target_frames * self.n_mels],
                n_frames: target_frames,
                n_mels: self.n_mels,
                hop_size: self.hop_size,
                sample_rate: self.sample_rate,
            };
        }
        if target_frames == self.n_frames {
            return self.clone();
        }
        let mut data = vec![0.0f32; target_frames * self.n_mels];
        let scale = (self.n_frames - 1) as f32 / (target_frames.max(2) - 1) as f32;
        for t in 0..target_frames {
            let src = t as f32 * scale;
            let i0 = src.floor() as usize;
            let i1 = (i0 + 1).min(self.n_frames - 1);
            let frac = src - i0 as f32;
            let a = self.frame(i0);
            let b = self.frame(i1);
            let dst = &mut data[t * self.n_mels..(t + 1) * self.n_mels];
            for m in 0..self.n_mels {
                dst[m] = a[m] * (1.0 - frac) + b[m] * frac;
            }
        }
        MelSpectrogram {
            data,
            n_frames: target_frames,
            n_mels: self.n_mels,
            hop_size: self.hop_size,
            sample_rate: self.sample_rate,
        }
    }

    /// 转置为声码器输入布局 `[1, T, n_mels]`（展平）。
    pub fn to_batch(&self) -> Vec<f32> {
        self.data.clone()
    }
}

/// 计算 Mel 频谱（含 log 动态范围压缩）。
///
/// `key_shift` 为半音单位（对应 `g` 标记的 gender/100），通过缩放分析窗长实现。
pub fn compute_mel(
    samples: &[f32],
    cfg: &MelConfig,
    hop_size: usize,
    key_shift: f32,
) -> anyhow::Result<MelSpectrogram> {
    let factor = 2f32.powf(key_shift / 12.0);
    let n_fft = ((cfg.n_fft as f32 * factor).round() as usize).max(16);
    let win_size = ((cfg.win_size as f32 * factor).round() as usize).max(16);
    let hop = hop_size.max(1);

    let filters = mel_filterbank(
        cfg.sample_rate,
        cfg.n_fft,
        cfg.n_mels,
        cfg.fmin,
        cfg.fmax,
        cfg.mel_scale,
    );
    let magnitudes = stft_magnitude(samples, n_fft, win_size, hop, cfg.magnitude_eps)?;
    let n_frames = magnitudes.len();
    let n_bins = filters[0].len();

    let mut data = vec![0.0f32; n_frames * cfg.n_mels];
    for (f, mag) in magnitudes.iter().enumerate() {
        for m in 0..cfg.n_mels {
            let filter = &filters[m];
            let mut acc = 0.0f32;
            for (b, &w) in filter.iter().enumerate() {
                if w != 0.0 && b < mag.len().min(n_bins) {
                    acc += w * mag[b];
                }
            }
            if key_shift != 0.0 {
                acc *= cfg.win_size as f32 / win_size as f32;
            }
            // 动态范围压缩：log(clamp(x, min=clip_val))
            data[f * cfg.n_mels + m] = acc.max(cfg.clip_val).ln();
        }
    }

    Ok(MelSpectrogram {
        data,
        n_frames,
        n_mels: cfg.n_mels,
        hop_size: hop,
        sample_rate: cfg.sample_rate,
    })
}

/// 一维线性插值（用于 F0 / 参数曲线的时间对齐）。
pub fn interp_linear(values: &[f32], target_len: usize) -> Vec<f32> {
    if values.is_empty() {
        return vec![0.0; target_len];
    }
    if values.len() == target_len {
        return values.to_vec();
    }
    if values.len() == 1 {
        return vec![values[0]; target_len];
    }
    let scale = (values.len() - 1) as f32 / (target_len.max(2) - 1) as f32;
    (0..target_len)
        .map(|i| {
            let src = i as f32 * scale;
            let i0 = src.floor() as usize;
            let i1 = (i0 + 1).min(values.len() - 1);
            let frac = src - i0 as f32;
            values[i0] * (1.0 - frac) + values[i1] * frac
        })
        .collect()
}

/// 在给定时间网格上采样曲线（时间与数值一一对应，端点外做钳制）。
pub fn sample_at(times_out: &[f64], times_in: &[f64], values: &[f32]) -> Vec<f32> {
    if times_in.is_empty() || values.is_empty() {
        return vec![0.0; times_out.len()];
    }
    if times_in.len() == 1 {
        return vec![values[0]; times_out.len()];
    }
    let mut out = Vec::with_capacity(times_out.len());
    let mut idx = 0usize;
    for &t in times_out {
        if t <= times_in[0] {
            out.push(values[0]);
            continue;
        }
        if t >= times_in[times_in.len() - 1] {
            out.push(values[values.len() - 1]);
            continue;
        }
        while idx + 1 < times_in.len() && times_in[idx + 1] < t {
            idx += 1;
        }
        let t0 = times_in[idx];
        let t1 = times_in[idx + 1];
        let frac = if (t1 - t0).abs() < 1e-12 {
            0.0
        } else {
            ((t - t0) / (t1 - t0)) as f32
        };
        let v0 = values[idx.min(values.len() - 1)];
        let v1 = values[(idx + 1).min(values.len() - 1)];
        out.push(v0 * (1.0 - frac) + v1 * frac);
    }
    out
}

/// 滑动平均平滑。
pub fn smooth(values: &[f32], window: usize) -> Vec<f32> {
    if window <= 1 || values.is_empty() {
        return values.to_vec();
    }
    let half = window / 2;
    values
        .iter()
        .enumerate()
        .map(|(i, _)| {
            let start = i.saturating_sub(half);
            let end = (i + half + 1).min(values.len());
            let sum: f32 = values[start..end].iter().sum();
            sum / (end - start) as f32
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sine(sr: u32, freq: f32, n: usize) -> Vec<f32> {
        (0..n)
            .map(|i| (2.0 * std::f32::consts::PI * freq * i as f32 / sr as f32).sin() * 0.5)
            .collect()
    }

    #[test]
    fn hann_window_shape() {
        let w = hann_window(8);
        assert_eq!(w.len(), 8);
        assert!(w[0].abs() < 1e-6);
        assert!(w[4] > 0.9);
    }

    #[test]
    fn mel_scale_roundtrip() {
        for f in [40.0f32, 100.0, 440.0, 8000.0] {
            let m = hz_to_mel_slaney(f);
            assert!((mel_to_hz_slaney(m) - f).abs() < 1e-2, "{f}");
            let m = hz_to_mel_htk(f);
            assert!((mel_to_hz_htk(m) - f).abs() < 1e-3, "htk {f}");
        }
    }

    /// 与 librosa 的 `mel(sr=44100, n_fft=2048, n_mels=128, fmin=40, fmax=16000)` 参考值比对。
    ///
    /// 参考值取自实际推理链路（SingingVocoders / FCPE 均使用 librosa 默认 Slaney 标度）。
    #[test]
    fn slaney_scale_matches_librosa() {
        // librosa: hz_to_mel(40, slaney) ≈ 0.6；mel_to_hz 反推应还原
        assert!((hz_to_mel_slaney(1000.0) - 15.0).abs() < 1e-4);
        assert!((mel_to_hz_slaney(15.0) - 1000.0).abs() < 1e-2);
        // 折叠点以下为线性：200/3 mel per Hz
        assert!((hz_to_mel_slaney(300.0) - 4.5).abs() < 1e-5);
        // 折叠点以上为对数
        assert!(hz_to_mel_slaney(8000.0) > hz_to_mel_slaney(4000.0));
        assert!((hz_to_mel_slaney(2000.0) - 15.0 - (2.0f32.ln() / 0.068_751_78)).abs() < 1e-3);
    }

    #[test]
    fn filterbank_shape_and_coverage() {
        let fb = mel_filterbank(44100, 2048, 128, 40.0, 16000.0, MelScale::Slaney);
        assert_eq!(fb.len(), 128);
        assert_eq!(fb[0].len(), 1025);
        assert!(fb.iter().any(|f| f.iter().any(|&v| v > 0.0)));
    }

    /// FCPE 侧的滤波器组为 `[128, 513]`（16kHz / 1024 FFT / 0–8000Hz）。
    #[test]
    fn fcpe_filterbank_shape() {
        let fb = mel_filterbank(16000, 1024, 128, 0.0, 8000.0, MelScale::Slaney);
        assert_eq!(fb.len(), 128);
        assert_eq!(fb[0].len(), 513);
        // 最高 bin 覆盖到奈奎斯特附近
        assert!(fb[127].iter().skip(480).any(|&v| v > 0.0));
    }

    #[test]
    fn mel_of_sine_has_energy_at_expected_bin() {
        let cfg = MelConfig {
            hop_size: 512,
            ..Default::default()
        };
        let sr = cfg.sample_rate;
        let samples = sine(sr, 1000.0, sr as usize);
        let mel = compute_mel(&samples, &cfg, 512, 0.0).unwrap();
        assert!(mel.n_frames > 80, "帧数过少: {}", mel.n_frames);
        let frame = mel.frame(mel.n_frames / 2);
        let max_idx = frame
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
            .unwrap()
            .0;
        // 1000Hz 在 40~16000Hz 的 128 bin mel 标度上应落在中低段
        assert!(max_idx > 10 && max_idx < 90, "峰值 bin 异常: {max_idx}");
    }

    #[test]
    fn interp_and_sample() {
        let v = vec![0.0f32, 10.0, 20.0];
        let out = interp_linear(&v, 5);
        assert_eq!(out.len(), 5);
        assert!((out[4] - 20.0).abs() < 1e-5);
        let times_in = vec![0.0f64, 1.0, 2.0];
        let times_out = vec![-1.0f64, 0.5, 1.5, 5.0];
        let s = sample_at(&times_out, &times_in, &v);
        assert_eq!(s, vec![0.0, 5.0, 15.0, 20.0]);
    }

    #[test]
    fn frame_resample_length() {
        let cfg = MelConfig {
            hop_size: 128,
            ..Default::default()
        };
        let sr = cfg.sample_rate;
        let mel = compute_mel(&sine(sr, 440.0, sr as usize / 2), &cfg, 128, 0.0).unwrap();
        let stretched = mel.resample_frames(mel.n_frames * 2);
        assert_eq!(stretched.n_frames, mel.n_frames * 2);
        assert_eq!(stretched.n_mels, mel.n_mels);
    }
}
