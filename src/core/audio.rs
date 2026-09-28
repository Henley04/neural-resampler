//! 音频读写与基础 DSP（WAV I/O、重采样、增益、裁剪）。

use anyhow::{bail, Context as _, Result};
use hound::{SampleFormat, WavReader, WavSpec, WavWriter};
use std::path::Path;

/// 单声道 f32 音频缓冲。
#[derive(Debug, Clone, PartialEq)]
pub struct AudioBuffer {
    pub samples: Vec<f32>,
    pub sample_rate: u32,
}

impl AudioBuffer {
    pub fn new(samples: Vec<f32>, sample_rate: u32) -> Self {
        Self {
            samples,
            sample_rate,
        }
    }

    /// 空缓冲。
    pub fn empty(sample_rate: u32) -> Self {
        Self {
            samples: Vec::new(),
            sample_rate,
        }
    }

    pub fn len(&self) -> usize {
        self.samples.len()
    }

    pub fn is_empty(&self) -> bool {
        self.samples.is_empty()
    }

    /// 时长（秒）。
    pub fn duration(&self) -> f64 {
        self.samples.len() as f64 / self.sample_rate as f64
    }

    /// 时长（毫秒）。
    pub fn duration_ms(&self) -> f64 {
        self.duration() * 1000.0
    }

    /// 峰值绝对值。
    pub fn peak(&self) -> f32 {
        self.samples.iter().fold(0.0f32, |m, &s| m.max(s.abs()))
    }

    /// RMS（dBFS）。
    pub fn rms_db(&self) -> f32 {
        if self.samples.is_empty() {
            return -120.0;
        }
        let sum: f64 = self.samples.iter().map(|&s| (s as f64) * (s as f64)).sum();
        let rms = (sum / self.samples.len() as f64).sqrt();
        if rms <= 1e-12 {
            -120.0
        } else {
            (20.0 * rms.log10()) as f32
        }
    }

    /// 按毫秒区间切片（越界部分自动裁剪）。
    pub fn slice_ms(&self, start_ms: f64, end_ms: f64) -> AudioBuffer {
        let sr = self.sample_rate as f64;
        let start = ((start_ms / 1000.0 * sr).round() as isize).max(0) as usize;
        let end = ((end_ms / 1000.0 * sr).round() as isize).max(0) as usize;
        let end = end.min(self.samples.len());
        let samples = if start >= end {
            Vec::new()
        } else {
            self.samples[start..end].to_vec()
        };
        AudioBuffer {
            samples,
            sample_rate: self.sample_rate,
        }
    }

    /// 整体缩放。
    pub fn scale(&mut self, gain: f32) {
        for s in &mut self.samples {
            *s *= gain;
        }
    }
}

/// 读取 WAV，混降为单声道 f32（范围约 [-1, 1]）。
pub fn read_wav(path: &Path) -> Result<AudioBuffer> {
    let mut reader = WavReader::open(path).with_context(|| format!("打开 WAV 失败: {path:?}"))?;
    let spec = reader.spec();
    let channels = spec.channels.max(1) as usize;

    let mono: Vec<f32> = match (spec.sample_format, spec.bits_per_sample) {
        (SampleFormat::Float, 32) => {
            let data: Vec<f32> = reader
                .samples::<f32>()
                .collect::<Result<Vec<_>, _>>()
                .context("读取浮点采样失败")?;
            fold_channels(&data, channels)
        }
        (SampleFormat::Int, 16) => {
            let data: Vec<i16> = reader
                .samples::<i16>()
                .collect::<Result<Vec<_>, _>>()
                .context("读取 16bit 采样失败")?;
            fold_channels(
                &data.iter().map(|&v| v as f32 / 32768.0).collect::<Vec<_>>(),
                channels,
            )
        }
        (SampleFormat::Int, 24) => {
            let data: Vec<i32> = reader
                .samples::<i32>()
                .collect::<Result<Vec<_>, _>>()
                .context("读取 24bit 采样失败")?;
            fold_channels(
                &data
                    .iter()
                    .map(|&v| v as f32 / 8_388_608.0)
                    .collect::<Vec<_>>(),
                channels,
            )
        }
        (SampleFormat::Int, 32) => {
            let data: Vec<i32> = reader
                .samples::<i32>()
                .collect::<Result<Vec<_>, _>>()
                .context("读取 32bit 采样失败")?;
            fold_channels(
                &data
                    .iter()
                    .map(|&v| v as f32 / 2_147_483_648.0)
                    .collect::<Vec<_>>(),
                channels,
            )
        }
        (SampleFormat::Int, 8) => {
            let data: Vec<i8> = reader
                .samples::<i8>()
                .collect::<Result<Vec<_>, _>>()
                .context("读取 8bit 采样失败")?;
            fold_channels(
                &data.iter().map(|&v| v as f32 / 128.0).collect::<Vec<_>>(),
                channels,
            )
        }
        (fmt, bits) => bail!("不支持的 WAV 格式: {fmt:?} / {bits} bit"),
    };

    Ok(AudioBuffer {
        samples: mono,
        sample_rate: spec.sample_rate,
    })
}

fn fold_channels(data: &[f32], channels: usize) -> Vec<f32> {
    if channels == 1 {
        return data.to_vec();
    }
    data.chunks(channels)
        .map(|frame| frame.iter().sum::<f32>() / channels as f32)
        .collect()
}

/// 写出单声道 WAV。`bit_depth` 为 16/24/32 表示整数 PCM，0 表示 32 位浮点。
pub fn write_wav(path: &Path, samples: &[f32], sample_rate: u32, bit_depth: u16) -> Result<()> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }

    let (bits, format) = match bit_depth {
        0 => (32, SampleFormat::Float),
        16 => (16, SampleFormat::Int),
        24 => (24, SampleFormat::Int),
        32 => (32, SampleFormat::Int),
        other => bail!("不支持的输出位深: {other}"),
    };

    let spec = WavSpec {
        channels: 1,
        sample_rate,
        bits_per_sample: bits,
        sample_format: format,
    };
    let mut writer =
        WavWriter::create(path, spec).with_context(|| format!("创建 WAV 失败: {path:?}"))?;

    match format {
        SampleFormat::Float => {
            for &s in samples {
                writer.write_sample(s)?;
            }
        }
        SampleFormat::Int => match bits {
            16 => {
                for &s in samples {
                    writer.write_sample(float_to_int(s, 32767.0) as i16)?;
                }
            }
            24 => {
                for &s in samples {
                    writer.write_sample(float_to_int(s, 8_388_607.0) as i32)?;
                }
            }
            _ => {
                for &s in samples {
                    writer.write_sample(float_to_int(s, 2_147_483_647.0) as i32)?;
                }
            }
        },
    }

    writer.finalize()?;
    Ok(())
}

#[inline]
fn float_to_int(s: f32, max: f32) -> i64 {
    let v = (s.clamp(-1.0, 1.0) * max).round();
    v as i64
}

/// 线性插值重采样（轻量，适合短信号与参数曲线）。
pub fn resample_linear(samples: &[f32], src_sr: u32, dst_sr: u32) -> Vec<f32> {
    if samples.is_empty() || src_sr == dst_sr {
        return samples.to_vec();
    }
    let ratio = dst_sr as f64 / src_sr as f64;
    let out_len = ((samples.len() as f64) * ratio).round().max(1.0) as usize;
    (0..out_len)
        .map(|i| {
            let src_pos = i as f64 / ratio;
            let i0 = src_pos.floor() as usize;
            let i1 = (i0 + 1).min(samples.len() - 1);
            let frac = (src_pos - i0 as f64) as f32;
            samples[i0] * (1.0 - frac) + samples[i1] * frac
        })
        .collect()
}

/// 高质量 sinc 重采样（rubato），用于 16kHz ↔ 44.1kHz 等真实采样率转换。
pub fn resample_sinc(samples: &[f32], src_sr: u32, dst_sr: u32) -> Result<Vec<f32>> {
    if samples.is_empty() {
        return Ok(Vec::new());
    }
    if src_sr == dst_sr {
        return Ok(samples.to_vec());
    }

    use rubato::{
        Resampler, SincFixedIn, SincInterpolationParameters, SincInterpolationType, WindowFunction,
    };

    let params = SincInterpolationParameters {
        sinc_len: 256,
        f_cutoff: 0.95,
        oversampling_factor: 128,
        interpolation: SincInterpolationType::Cubic,
        window: WindowFunction::BlackmanHarris2,
    };

    let ratio = dst_sr as f64 / src_sr as f64;
    let input: Vec<f64> = samples.iter().map(|&s| s as f64).collect();
    let mut resampler = SincFixedIn::<f64>::new(ratio, 1.1, params, input.len(), 1)
        .map_err(|e| anyhow::anyhow!("构造重采样器失败: {e}"))?;
    let out = resampler
        .process(&[&input], None)
        .map_err(|e| anyhow::anyhow!("重采样失败: {e}"))?;
    Ok(out
        .into_iter()
        .next()
        .unwrap_or_default()
        .into_iter()
        .map(|s| s as f32)
        .collect())
}

/// 去除首尾静音（阈值单位 dBFS）。
pub fn trim_silence(buf: &AudioBuffer, threshold_db: f32) -> AudioBuffer {
    if buf.is_empty() {
        return buf.clone();
    }
    let threshold = 10f32.powf(threshold_db / 20.0);
    let block = (buf.sample_rate as usize / 100).max(1); // 10ms 分块
    let n_blocks = buf.len() / block;
    let energy = |b: usize| -> f32 {
        let start = b * block;
        let end = (start + block).min(buf.len());
        let sum: f32 = buf.samples[start..end].iter().map(|&s| s * s).sum();
        (sum / (end - start) as f32).sqrt()
    };
    let mut first = 0usize;
    while first < n_blocks && energy(first) < threshold {
        first += 1;
    }
    let mut last = n_blocks;
    while last > first && energy(last - 1) < threshold {
        last -= 1;
    }
    let start = (first * block).min(buf.len());
    let end = (last * block).min(buf.len());
    AudioBuffer {
        samples: buf.samples[start..end].to_vec(),
        sample_rate: buf.sample_rate,
    }
}

/// dB → 线性增益。
pub fn db_to_gain(db: f32) -> f32 {
    10f32.powf(db / 20.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn tmp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join("nr-audio-test");
        std::fs::create_dir_all(&dir).unwrap();
        dir.join(name)
    }

    #[test]
    fn wav_roundtrip_16bit() {
        let path = tmp("roundtrip.wav");
        let sr = 44100u32;
        let original: Vec<f32> = (0..1000)
            .map(|i| (2.0 * std::f32::consts::PI * 440.0 * i as f32 / sr as f32).sin() * 0.5)
            .collect();
        write_wav(&path, &original, sr, 16).unwrap();
        let read = read_wav(&path).unwrap();
        assert_eq!(read.sample_rate, sr);
        assert_eq!(read.len(), original.len());
        let mut max_err = 0.0f32;
        for (a, b) in original.iter().zip(read.samples.iter()) {
            max_err = max_err.max((a - b).abs());
        }
        assert!(max_err < 1e-3, "16bit 量化误差过大: {max_err}");
    }

    #[test]
    fn resample_keeps_length_ratio() {
        let src: Vec<f32> = (0..4410).map(|i| (i as f32 / 10.0).sin() * 0.3).collect();
        let out = resample_linear(&src, 44100, 16000);
        let expected = (4410.0f64 * 16000.0 / 44100.0).round() as usize;
        assert!((out.len() as i64 - expected as i64).abs() <= 1);
    }

    #[test]
    fn sinc_resample_changes_rate() {
        let src: Vec<f32> = (0..16000).map(|i| (i as f32 / 20.0).sin() * 0.4).collect();
        let out = resample_sinc(&src, 16000, 44100).unwrap();
        // SincFixedIn 的输出长度与理论值存在滤波器延迟带来的小幅偏差
        assert!(
            (out.len() as i64 - 44100).abs() < 1000,
            "输出长度异常: {}",
            out.len()
        );
        assert!(!out.is_empty());
    }

    #[test]
    fn peak_and_rms() {
        let mut buf = AudioBuffer::new(vec![0.5, -1.0, 0.25], 44100);
        assert_eq!(buf.peak(), 1.0);
        assert!(buf.rms_db() < 0.0);
        buf.scale(0.5);
        assert_eq!(buf.peak(), 0.5);
    }
}
