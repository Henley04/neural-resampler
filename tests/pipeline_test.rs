//! 端到端管线测试
//!
//! 不依赖 ONNX 模型：声码器缺失时引擎自动降级为内置 Stub 后端，
//! F0 缺失时降级为内置 DSP（WORLD 风格），因此可以完整跑通
//! `WAV → 特征提取 → F0 → 声码器 → 后处理 → WAV`。

use neural_resampler::config::ResamplerConfig;
use neural_resampler::core::audio::{read_wav, write_wav};
use neural_resampler::core::pipeline::{Engine, RenderRequest};
use neural_resampler::core::protocol::UtauParams;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

fn workdir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("nr-pipeline-{name}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// 生成一段带包络的测试音（模拟声库样本）。
fn make_sample(dir: &Path, name: &str, freq: f32, ms: f64) -> PathBuf {
    let path = dir.join(name);
    let sr = 44100u32;
    let n = (sr as f64 * ms / 1000.0) as usize;
    let samples: Vec<f32> = (0..n)
        .map(|i| {
            let t = i as f32 / sr as f32;
            let env = (t * 8.0).min(1.0) * (((ms as f32 / 1000.0) - t) * 8.0).min(1.0);
            let vib = 1.0 + 0.01 * (2.0 * std::f32::consts::PI * 5.0 * t).sin();
            (2.0 * std::f32::consts::PI * freq * vib * t).sin() * 0.4 * env
        })
        .collect();
    write_wav(&path, &samples, sr, 16).unwrap();
    path
}

fn request(input: PathBuf, output: PathBuf, pitch_midi: f32, length_ms: f64) -> RenderRequest {
    RenderRequest {
        input,
        output,
        pitch_midi,
        pitch_bends: vec![0.0; 48],
        velocity: 100.0,
        offset_ms: 30.0,
        length_ms,
        consonant_ms: 50.0,
        cutoff_ms: -30.0,
        volume: 100.0,
        modulation: 0.0,
        tempo: 120.0,
        flags: HashMap::new(),
        oto: None,
    }
}

#[test]
fn pipeline_end_to_end() {
    let dir = workdir("e2e");
    let input = make_sample(&dir, "test_input.wav", 220.0, 900.0);
    let output = dir.join("test_output.wav");

    let cfg = ResamplerConfig::default_yaml().unwrap();
    let engine = Engine::new(cfg).unwrap();
    let stats = engine
        .render(&request(input, output.clone(), 60.0, 500.0))
        .unwrap();

    assert!(stats.frames > 1, "渲染帧数异常: {}", stats.frames);
    assert!(output.exists(), "未生成输出文件");

    let written = read_wav(&output).unwrap();
    // 单声道输出
    assert_eq!(written.sample_rate, 44100);
    assert_eq!(written.sample_rate, 44100);
    assert!(!written.is_empty());
    assert!(written.peak() > 0.0, "输出为静音");
}

#[test]
fn longer_note_yields_longer_audio() {
    let dir = workdir("length");
    let input = make_sample(&dir, "sample.wav", 220.0, 1200.0);
    let short_out = dir.join("short.wav");
    let long_out = dir.join("long.wav");

    let cfg = ResamplerConfig::default_yaml().unwrap();
    let engine = Engine::new(cfg).unwrap();
    let s1 = engine
        .render(&request(input.clone(), short_out.clone(), 60.0, 300.0))
        .unwrap();
    let s2 = engine
        .render(&request(input, long_out.clone(), 60.0, 900.0))
        .unwrap();

    assert!(s2.duration_ms > s1.duration_ms, "长音符应产生更长音频");
    let expected_long = 900.0 + 50.0;
    assert!(
        (s2.duration_ms - expected_long).abs() < 80.0,
        "长音符时长 {}ms 与预期 {expected_long}ms 偏差过大",
        s2.duration_ms
    );
}

#[test]
fn higher_pitch_renders_without_error() {
    let dir = workdir("pitch");
    let input = make_sample(&dir, "sample.wav", 220.0, 800.0);
    let output = dir.join("high.wav");
    let cfg = ResamplerConfig::default_yaml().unwrap();
    let engine = Engine::new(cfg).unwrap();
    let stats = engine
        .render(&request(input, output.clone(), 79.0, 400.0))
        .unwrap();
    assert!(stats.frames > 1);
    assert!(output.exists());
}

#[test]
fn utau_params_pipeline() {
    let dir = workdir("utau");
    let input = make_sample(&dir, "_a.wav", 260.0, 800.0);
    let output = dir.join("utau_out.wav");

    let params = UtauParams {
        input_file: input,
        output_file: output.clone(),
        pitch: "C4".into(),
        pitch_midi: Some(60.0),
        offset: 20.0,
        length_req: 500.0,
        consonant: 40.0,
        cutoff: -20.0,
        pitch_bends: vec![0.0, 25.0, 50.0, 25.0, 0.0],
        ..UtauParams::default()
    };

    let cfg = ResamplerConfig::default_yaml().unwrap();
    let engine = Engine::new(cfg).unwrap();
    let stats = engine.run_pipeline(&params).unwrap();
    assert!(output.exists());
    assert!(stats.duration_ms > 0.0);
}

#[test]
fn missing_input_reports_error() {
    let dir = workdir("missing");
    let cfg = ResamplerConfig::default_yaml().unwrap();
    let engine = Engine::new(cfg).unwrap();
    let req = request(dir.join("nope.wav"), dir.join("out.wav"), 60.0, 500.0);
    assert!(engine.render(&req).is_err());
}
