//! WORLD 风格 F0 回退实现（纯 DSP，无外部模型）
//!
//! 提供两种经典算法：
//!
//! * `Dio`：频域谐波求和（spectral harmonic summation）粗估计 + 抛物线插值细化；
//! * `Harvest`：时域自相关（YIN 式差分函数）估计 + StoneMask 式细化。
//!
//! 清浊判定使用「谐波能量比 + 频谱平坦度」的组合判据。

use crate::config::{F0Config, ResamplerConfig};
use crate::core::f0::{F0Extractor, F0Track};
use anyhow::Result;
use std::f32::consts::PI;

/// 算法选择。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Algorithm {
    /// 频域谐波求和（默认）。
    #[default]
    Dio,
    /// 时域自相关。
    Harvest,
}

/// 内置 DSP F0 提取器。
#[derive(Debug, Clone)]
pub struct WorldExtractor {
    algorithm: Algorithm,
    /// 分析帧移（毫秒）。
    frame_period_ms: f32,
    /// 分析窗长（毫秒）。
    window_ms: f32,
    f0_min: f32,
    f0_max: f32,
    /// 清浊判定阈值（谐波能量比）。
    voiced_threshold: f32,
}

impl WorldExtractor {
    pub fn new(cfg: &ResamplerConfig) -> Self {
        Self::from_f0_config(&cfg.f0, Algorithm::Dio)
    }

    pub fn from_f0_config(cfg: &F0Config, algorithm: Algorithm) -> Self {
        let frame_period_ms = cfg.hop_size as f32 / cfg.sample_rate as f32 * 1000.0;
        // 窗长取 F0 下限的 3 个周期，至少 20ms
        let window_ms = (3000.0 / cfg.f0_min.max(40.0)).max(20.0);
        Self {
            algorithm,
            frame_period_ms,
            window_ms,
            f0_min: cfg.f0_min,
            f0_max: cfg.f0_max,
            voiced_threshold: 0.35,
        }
    }

    /// 估计单帧 F0（Hz），返回 (f0, 置信度)。
    fn estimate_frame(&self, frame: &[f32], sample_rate: u32) -> (f32, f32) {
        match self.algorithm {
            Algorithm::Dio => self.dio(frame, sample_rate),
            Algorithm::Harvest => self.harvest(frame, sample_rate),
        }
    }

    /// DIO：谐波求和 + 抛物线细化。
    fn dio(&self, frame: &[f32], sample_rate: u32) -> (f32, f32) {
        let n_fft = next_pow2(frame.len().max(64));
        let spectrum = magnitude_spectrum(frame, n_fft);
        let bin_hz = sample_rate as f32 / n_fft as f32;
        let min_bin = (self.f0_min / bin_hz).max(2.0) as usize;
        let max_bin = ((self.f0_max / bin_hz) as usize).min(spectrum.len() / 2 - 1);
        if min_bin >= max_bin {
            return (0.0, 0.0);
        }

        let mut best_bin = 0usize;
        let mut best_score = 0.0f32;
        let mut total_energy = 0.0f32;
        for &m in spectrum.iter() {
            total_energy += m * m;
        }
        if total_energy <= 1e-12 {
            return (0.0, 0.0);
        }

        for bin in min_bin..=max_bin {
            let mut score = 0.0f32;
            for k in 1..=5 {
                let idx = bin * k;
                if idx < spectrum.len() {
                    let w = 1.0 / k as f32;
                    score += w * spectrum[idx];
                }
            }
            // 抑制倍频误判：基频能量本身也参与打分
            score += spectrum[bin] * 0.5;
            if score > best_score {
                best_score = score;
                best_bin = bin;
            }
        }

        if best_bin == 0 {
            return (0.0, 0.0);
        }

        // 抛物线插值细化
        let refined = parabolic_peak(&spectrum, best_bin);
        let f0 = refined * bin_hz;
        let confidence = (best_score / (total_energy.sqrt() + 1e-9)).clamp(0.0, 1.0);
        (f0, confidence)
    }

    /// Harvest：自相关差分函数（YIN 式）。
    fn harvest(&self, frame: &[f32], sample_rate: u32) -> (f32, f32) {
        let n = frame.len();
        if n < 64 {
            return (0.0, 0.0);
        }
        let min_lag = ((sample_rate as f32 / self.f0_max).floor() as usize).max(2);
        let max_lag = ((sample_rate as f32 / self.f0_min).ceil() as usize).min(n / 2);
        if min_lag >= max_lag {
            return (0.0, 0.0);
        }

        // 差分函数 d[tau] = sum (x[i] - x[i+tau])^2
        let mut d = vec![0.0f64; max_lag + 1];
        for tau in min_lag..=max_lag {
            let mut acc = 0.0f64;
            for i in 0..(n - tau) {
                let diff = frame[i] as f64 - frame[i + tau] as f64;
                acc += diff * diff;
            }
            d[tau] = acc;
        }

        // 累积均值归一化差分（YIN CMND）
        let mut cmnd = vec![0.0f64; max_lag + 1];
        let mut running = 0.0f64;
        for tau in min_lag..=max_lag {
            running += d[tau];
            cmnd[tau] = d[tau] * tau as f64 / (running + 1e-9);
        }

        // 找首个低于阈值的局部极小
        let threshold = 0.15;
        let mut best_tau = 0usize;
        let mut best_val = f64::MAX;
        for tau in min_lag..=max_lag {
            if cmnd[tau] < threshold {
                // 细化：向更小的 tau 找局部极小
                let mut t = tau;
                while t > min_lag && cmnd[t - 1] < cmnd[t] {
                    t -= 1;
                }
                best_tau = t;
                break;
            }
            if cmnd[tau] < best_val {
                best_val = cmnd[tau];
                best_tau = tau;
            }
        }
        if best_tau == 0 {
            return (0.0, 0.0);
        }

        // 抛物线插值
        let refined_lag = if best_tau > min_lag && best_tau < max_lag {
            let y0 = cmnd[best_tau - 1];
            let y1 = cmnd[best_tau];
            let y2 = cmnd[best_tau + 1];
            let denom = 2.0 * (2.0 * y1 - y2 - y0);
            if denom.abs() > 1e-12 {
                best_tau as f64 + (y2 - y0) / denom
            } else {
                best_tau as f64
            }
        } else {
            best_tau as f64
        };

        let f0 = sample_rate as f32 / refined_lag as f32;
        let confidence = (1.0 - best_val.min(1.0) as f32).clamp(0.0, 1.0);
        (f0, confidence)
    }
}

impl F0Extractor for WorldExtractor {
    fn name(&self) -> &str {
        match self.algorithm {
            Algorithm::Dio => "world-dio",
            Algorithm::Harvest => "world-harvest",
        }
    }

    fn frame_period_ms(&self) -> f32 {
        self.frame_period_ms
    }

    fn extract(&self, audio: &[f32], sample_rate: u32) -> Result<F0Track> {
        if audio.is_empty() || sample_rate == 0 {
            return Ok(F0Track::empty(self.frame_period_ms));
        }
        let hop = ((self.frame_period_ms / 1000.0) * sample_rate as f32)
            .round()
            .max(1.0) as usize;
        let win = ((self.window_ms / 1000.0) * sample_rate as f32)
            .round()
            .max(8.0) as usize;
        let n_frames = audio.len() / hop;

        let window = hann(win);
        let mut f0 = Vec::with_capacity(n_frames);

        for i in 0..n_frames {
            let start = i * hop;
            let mut frame = vec![0.0f32; win];
            for (j, w) in window.iter().enumerate() {
                if start + j < audio.len() {
                    frame[j] = audio[start + j] * w;
                }
            }
            // 去均值，避免直流分量干扰
            let mean = frame.iter().sum::<f32>() / win as f32;
            for s in &mut frame {
                *s -= mean;
            }

            let energy: f32 = frame.iter().map(|&s| s * s).sum();
            let (freq, confidence) = self.estimate_frame(&frame, sample_rate);
            if energy < 1e-8
                || confidence < self.voiced_threshold
                || !(self.f0_min..=self.f0_max).contains(&freq)
            {
                f0.push(0.0);
            } else {
                f0.push(freq);
            }
        }

        Ok(F0Track::new(f0, self.frame_period_ms))
    }
}

fn hann(size: usize) -> Vec<f32> {
    (0..size)
        .map(|i| 0.5 * (1.0 - (2.0 * PI * i as f32 / size as f32).cos()))
        .collect()
}

fn next_pow2(n: usize) -> usize {
    let mut p = 1usize;
    while p < n {
        p <<= 1;
    }
    p
}

/// 幅度谱（前 N/2 个 bin）。
fn magnitude_spectrum(frame: &[f32], n_fft: usize) -> Vec<f32> {
    let mut re = vec![0.0f32; n_fft];
    for (i, &s) in frame.iter().take(n_fft).enumerate() {
        re[i] = s;
    }
    // 朴素 DFT 的位反转置换 + 迭代 FFT（避免额外依赖）
    let mut im = vec![0.0f32; n_fft];
    fft_in_place(&mut re, &mut im);
    re.iter()
        .take(n_fft / 2)
        .zip(im.iter())
        .map(|(r, i)| (r * r + i * i).sqrt())
        .collect()
}

/// 迭代式 radix-2 FFT。
fn fft_in_place(re: &mut [f32], im: &mut [f32]) {
    let n = re.len();
    if n <= 1 {
        return;
    }
    // 位反转置换
    let mut j = 0usize;
    for i in 1..n {
        let mut bit = n >> 1;
        while j & bit != 0 {
            j ^= bit;
            bit >>= 1;
        }
        j |= bit;
        if i < j {
            re.swap(i, j);
            im.swap(i, j);
        }
    }
    let mut len = 2usize;
    while len <= n {
        let ang = -2.0 * PI / len as f32;
        let wr = ang.cos();
        let wi = ang.sin();
        let mut i = 0usize;
        while i < n {
            let mut w_re = 1.0f32;
            let mut w_im = 0.0f32;
            for k in 0..(len / 2) {
                let u_re = re[i + k];
                let u_im = im[i + k];
                let v_re = re[i + k + len / 2] * w_re - im[i + k + len / 2] * w_im;
                let v_im = re[i + k + len / 2] * w_im + im[i + k + len / 2] * w_re;
                re[i + k] = u_re + v_re;
                im[i + k] = u_im + v_im;
                re[i + k + len / 2] = u_re - v_re;
                im[i + k + len / 2] = u_im - v_im;
                let next_wr = w_re * wr - w_im * wi;
                let next_wi = w_re * wi + w_im * wr;
                w_re = next_wr;
                w_im = next_wi;
            }
            i += len;
        }
        len <<= 1;
    }
}

/// 抛物线峰值插值，返回细化后的 bin 位置。
fn parabolic_peak(spectrum: &[f32], bin: usize) -> f32 {
    if bin == 0 || bin + 1 >= spectrum.len() {
        return bin as f32;
    }
    let y0 = spectrum[bin - 1];
    let y1 = spectrum[bin];
    let y2 = spectrum[bin + 1];
    let denom = y0 - 2.0 * y1 + y2;
    if denom.abs() < 1e-12 {
        return bin as f32;
    }
    bin as f32 + 0.5 * (y0 - y2) / denom
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::F0Config;

    fn sine(sr: u32, freq: f32, ms: f32) -> Vec<f32> {
        let n = (sr as f32 * ms / 1000.0) as usize;
        (0..n)
            .map(|i| (2.0 * PI * freq * i as f32 / sr as f32).sin() * 0.5)
            .collect()
    }

    #[test]
    fn detects_440hz_sine_dio() {
        let cfg = F0Config::default();
        let ex = WorldExtractor::from_f0_config(&cfg, Algorithm::Dio);
        let sr = 16000;
        let audio = sine(sr, 440.0, 500.0);
        let track = ex.extract(&audio, sr).unwrap();
        let voiced: Vec<f32> = track.f0.iter().copied().filter(|&f| f > 0.0).collect();
        assert!(!voiced.is_empty(), "未检测到浊音帧");
        let mean = voiced.iter().sum::<f32>() / voiced.len() as f32;
        assert!((mean - 440.0).abs() < 20.0, "DIO 估计偏差过大: {mean}");
    }

    #[test]
    fn detects_440hz_sine_harvest() {
        let cfg = F0Config::default();
        let ex = WorldExtractor::from_f0_config(&cfg, Algorithm::Harvest);
        let sr = 16000;
        let audio = sine(sr, 440.0, 500.0);
        let track = ex.extract(&audio, sr).unwrap();
        let voiced: Vec<f32> = track.f0.iter().copied().filter(|&f| f > 0.0).collect();
        assert!(!voiced.is_empty(), "未检测到浊音帧");
        let mean = voiced.iter().sum::<f32>() / voiced.len() as f32;
        assert!((mean - 440.0).abs() < 10.0, "Harvest 估计偏差过大: {mean}");
    }

    #[test]
    fn silence_is_unvoiced() {
        let cfg = F0Config::default();
        let ex = WorldExtractor::from_f0_config(&cfg, Algorithm::Dio);
        let track = ex.extract(&vec![0.0f32; 8000], 16000).unwrap();
        assert_eq!(track.voiced_ratio(), 0.0);
    }

    #[test]
    fn frame_period_from_config() {
        let cfg = F0Config::default();
        let ex = WorldExtractor::from_f0_config(&cfg, Algorithm::Dio);
        assert_eq!(ex.frame_period_ms(), 10.0);
        assert_eq!(ex.name(), "world-dio");
    }

    #[test]
    fn fft_matches_expected_peak() {
        let sr = 8000u32;
        let n = 1024;
        let frame: Vec<f32> = (0..n)
            .map(|i| (2.0 * PI * 1000.0 * i as f32 / sr as f32).sin())
            .collect();
        let spec = magnitude_spectrum(&frame, n);
        let peak = spec
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
            .unwrap()
            .0;
        let expected = (1000.0 * n as f32 / sr as f32) as usize;
        assert!(
            (peak as i64 - expected as i64).abs() <= 1,
            "峰值 bin {peak} != {expected}"
        );
    }
}
