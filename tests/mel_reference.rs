//! Mel 提取的基准值比对测试
//!
//! 参考值由 Python 侧（`librosa.filters.mel` + `torch.stft(center=False)`）生成，
//! 即 PC-NSF-HiFiGAN / FCPE 训练前处理所用的同一套实现。
//! 这组测试锁死分析约定，防止重构时悄悄改变数值语义。

use neural_resampler::config::MelConfig;
use neural_resampler::core::feature::compute_mel;
use std::collections::HashMap;

#[derive(serde::Deserialize)]
struct Reference {
    n_mels: usize,
    frames: HashMap<String, Vec<f32>>,
}

fn load(path: &str) -> Reference {
    let text =
        std::fs::read_to_string(path).unwrap_or_else(|e| panic!("读取参考数据 {path} 失败: {e}"));
    serde_json::from_str(&text).expect("解析参考数据失败")
}

fn sine(sample_rate: u32, freq: f32, n: usize) -> Vec<f32> {
    (0..n)
        .map(|i| (2.0 * std::f32::consts::PI * freq * i as f32 / sample_rate as f32).sin() * 0.5)
        .collect()
}

/// 低于该值的 bin 处于能量地板（FFT 数值噪声），不参与比对。
const FLOOR: f32 = -10.0;

fn compare(cfg: &MelConfig, samples: &[f32], hop: usize, path: &str, tol: f32) {
    let mel = compute_mel(samples, cfg, hop, 0.0).expect("计算 Mel 失败");
    let reference = load(path);
    assert_eq!(mel.n_mels, reference.n_mels);

    let mut checked = 0;
    for (frame_str, expected) in &reference.frames {
        let frame: usize = frame_str.parse().expect("帧号解析失败");
        assert!(
            frame < mel.n_frames,
            "参考帧 {frame} 超出实际帧数 {}",
            mel.n_frames
        );
        let actual = mel.frame(frame);
        assert_eq!(actual.len(), expected.len());
        for (i, (&a, &e)) in actual.iter().zip(expected.iter()).enumerate() {
            if e < FLOOR {
                // 静音 bin：只要求同样落在地板附近
                assert!(
                    a < FLOOR + 2.0,
                    "{path} 帧 {frame} bin {i}: 静音 bin 实际能量过高 {a}（参考 {e}）"
                );
                continue;
            }
            let diff = (a - e).abs();
            assert!(
                diff < tol,
                "{path} 帧 {frame} bin {i}: 实际 {a} vs 参考 {e}（差 {diff}）"
            );
            checked += 1;
        }
    }
    assert!(checked > 30, "有效比对 bin 过少: {checked}");
}

/// 44.1kHz / 2048 FFT / hop 512 / 128 bin / 40–16000Hz。
#[test]
fn mel_matches_reference_at_44k() {
    let cfg = MelConfig::default();
    let samples = sine(44100, 440.0, 22050);
    compare(
        &cfg,
        &samples,
        cfg.hop_size,
        "tests/fixtures/mel_ref_44100.json",
        0.06,
    );
}

/// 16kHz / 1024 FFT / hop 160 / 128 bin / 0–8000Hz（FCPE 侧配置）。
#[test]
fn mel_matches_reference_at_16k() {
    let cfg = MelConfig {
        sample_rate: 16000,
        n_fft: 1024,
        win_size: 1024,
        hop_size: 160,
        origin_hop_size: 160,
        n_mels: 128,
        fmin: 0.0,
        fmax: 8000.0,
        clip_val: 1e-5,
        magnitude_eps: 1e-9,
        ..Default::default()
    };
    let samples = sine(16000, 440.0, 16000);
    compare(
        &cfg,
        &samples,
        160,
        "tests/fixtures/mel_ref_16000.json",
        0.06,
    );
}
