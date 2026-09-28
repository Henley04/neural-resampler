//! F0 提取精度测试
//!
//! * WORLD（内置 DSP）后端：始终运行，验证正弦音的 F0 估计精度；
//! * FCPE（ONNX）后端：存在 `models/fcpe.onnx` 时运行，否则跳过并打印提示。

use neural_resampler::config::{F0Backend, F0Config, ResamplerConfig};
use neural_resampler::core::audio::write_wav;
use neural_resampler::core::f0::world::{Algorithm, WorldExtractor};
use neural_resampler::core::f0::{create_extractor, F0Extractor, F0Track};
use std::path::Path;

fn sine(sr: u32, freq: f32, ms: f64) -> Vec<f32> {
    let n = (sr as f64 * ms / 1000.0) as usize;
    (0..n)
        .map(|i| (2.0 * std::f32::consts::PI * freq * i as f32 / sr as f32).sin() * 0.5)
        .collect()
}

fn mean_voiced(track: &F0Track) -> Option<f32> {
    let voiced: Vec<f32> = track.f0.iter().copied().filter(|&v| v > 0.0).collect();
    if voiced.is_empty() {
        None
    } else {
        Some(voiced.iter().sum::<f32>() / voiced.len() as f32)
    }
}

#[test]
fn world_dio_detects_440hz() {
    let cfg = F0Config::default();
    let extractor = WorldExtractor::from_f0_config(&cfg, Algorithm::Dio);
    let sr = 16000;
    let audio = sine(sr, 440.0, 1000.0);
    let track = extractor.extract(&audio, sr).unwrap();
    let mean = mean_voiced(&track).expect("未检测到浊音帧");
    assert!(
        (mean - 440.0).abs() < 10.0,
        "DIO 均值 {mean} 偏离 440Hz 过多"
    );
    assert!(
        track.voiced_ratio() > 0.8,
        "浊音比例过低: {}",
        track.voiced_ratio()
    );
}

#[test]
fn world_harvest_detects_220hz() {
    let cfg = F0Config::default();
    let extractor = WorldExtractor::from_f0_config(&cfg, Algorithm::Harvest);
    let sr = 16000;
    let audio = sine(sr, 220.0, 1000.0);
    let track = extractor.extract(&audio, sr).unwrap();
    let mean = mean_voiced(&track).expect("未检测到浊音帧");
    assert!(
        (mean - 220.0).abs() < 8.0,
        "Harvest 均值 {mean} 偏离 220Hz 过多"
    );
}

#[test]
fn world_marks_silence_unvoiced() {
    let cfg = F0Config::default();
    let extractor = WorldExtractor::from_f0_config(&cfg, Algorithm::Dio);
    let track = extractor.extract(&vec![0.0f32; 16000], 16000).unwrap();
    assert_eq!(track.voiced_ratio(), 0.0);
}

#[test]
fn default_config_falls_back_to_dsp() {
    let mut cfg = ResamplerConfig::default_yaml().unwrap();
    cfg.f0.model = std::path::PathBuf::from("models/__missing__/fcpe.onnx");
    let extractor = create_extractor(&cfg).unwrap();
    // Auto 模式下 FCPE 不可用时回退到内置 DSP
    assert!(extractor.is_some());
    assert!(extractor.unwrap().name().starts_with("world"));
}

#[test]
fn fcpe_backend_skips_without_model() {
    let mut cfg = ResamplerConfig::default_yaml().unwrap();
    cfg.f0.backend = F0Backend::Fcpe;
    if !cfg.model_path(&cfg.f0.model).exists() {
        eprintln!("跳过：未找到 FCPE 模型 {:?}", cfg.model_path(&cfg.f0.model));
        return;
    }
    let sr = 16000;
    let audio = sine(sr, 440.0, 1000.0);
    let extractor = create_extractor(&cfg).unwrap().expect("FCPE 提取器应可用");
    let track = extractor.extract(&audio, sr).unwrap();
    let mean = mean_voiced(&track).expect("FCPE 未检测到浊音帧");
    assert!(
        (mean - 440.0).abs() < 15.0,
        "FCPE 均值 {mean} 偏离 440Hz 过多"
    );
}

#[test]
fn fixture_wav_is_readable() {
    let path = Path::new("tests/fixtures/test_input.wav");
    if !path.exists() {
        eprintln!("跳过：缺少 fixture {path:?}");
        return;
    }
    let audio = neural_resampler::core::audio::read_wav(path).unwrap();
    assert_eq!(audio.sample_rate, 44100);
    assert!(audio.len() > 4410, "fixture 过短: {}", audio.len());
}

#[test]
fn writes_and_reads_temp_wav() {
    let dir = std::env::temp_dir().join("nr-f0-test");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("sine.wav");
    let sr = 44100;
    write_wav(&path, &sine(sr, 330.0, 300.0), sr, 16).unwrap();
    let read = neural_resampler::core::audio::read_wav(&path).unwrap();
    assert_eq!(read.sample_rate, sr);
    assert!(read.peak() > 0.3);
}
