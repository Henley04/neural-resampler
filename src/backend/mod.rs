//! 推理后端（可插拔）
//!
//! 目前提供：
//!
//! * [`ort_backend::OrtBackend`]：ONNX Runtime，支持 DirectML / CUDA / CoreML 等执行提供者；
//! * [`stub::StubBackend`]：无模型时的内置回声（离线自测用，**不用于生产**）。

pub mod ort_backend;
pub mod stub;

use crate::config::ResamplerConfig;
use anyhow::Result;

/// 声码器推理后端统一接口。
pub trait InferenceBackend: Send + Sync {
    /// 后端名称。
    fn name(&self) -> &str;

    /// (mel 输入名, f0 输入名)。
    fn input_names(&self) -> (String, String);

    /// 输出节点名。
    fn output_name(&self) -> String;

    /// 推理：`mel` 为 `[n_frames][n_mels]` 展平数据，`f0` 为长度 `n_frames` 的 Hz 曲线。
    fn infer(&self, mel: &[f32], n_frames: usize, n_mels: usize, f0: &[f32]) -> Result<Vec<f32>>;
}

/// 按配置创建后端。模型缺失时回退到 [`stub::StubBackend`] 并告警。
pub fn create_backend(cfg: &ResamplerConfig) -> Result<Box<dyn InferenceBackend>> {
    let model_path = cfg.model_path(&cfg.vocoder.model);
    if !model_path.exists() {
        log::warn!(
            "声码器模型不存在: {model_path:?}，使用内置 Stub 后端（仅用于离线自测，音质不可用）"
        );
        return Ok(Box::new(stub::StubBackend::default()));
    }
    match cfg.vocoder.backend {
        crate::config::VocoderBackend::Ort => Ok(Box::new(ort_backend::OrtBackend::load(cfg)?)),
        crate::config::VocoderBackend::Stub => Ok(Box::new(stub::StubBackend::default())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn falls_back_to_stub_when_model_missing() {
        // 指向一个必然为空的目录，避免受开发机上已放置的模型影响
        let mut cfg = ResamplerConfig::default_yaml().unwrap();
        cfg.models_dir = std::env::temp_dir().join("nr-backend-no-models");
        let backend = create_backend(&cfg).unwrap();
        assert_eq!(backend.name(), "stub");
    }
}
