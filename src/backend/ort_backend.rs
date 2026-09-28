//! ONNX Runtime 推理后端（PC-NSF-HiFiGAN）
//!
//! 输入：`mel` 与 `f0`，输出单声道波形。
//! `mel` 的布局由模型元数据自动判断（`[1, n_mels, T]` 或 `[1, T, n_mels]`），
//! 也可用 `vocoder.mel_layout` 强制指定。
//! 执行提供者按配置顺序注册（Windows 上可启用 DirectML）。

use crate::backend::InferenceBackend;
use crate::config::{MelLayout, ResamplerConfig};
use anyhow::{Context as _, Result};
use std::sync::Mutex;

/// ONNX Runtime 后端。
pub struct OrtBackend {
    session: Mutex<ort::session::Session>,
    mel_input: String,
    f0_input: String,
    output: String,
    /// `mel` 输入是否通道在前（`[1, n_mels, T]`）。
    channels_first: bool,
}

impl std::fmt::Debug for OrtBackend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OrtBackend")
            .field("mel_input", &self.mel_input)
            .field("f0_input", &self.f0_input)
            .field("output", &self.output)
            .field("channels_first", &self.channels_first)
            .finish_non_exhaustive()
    }
}

impl OrtBackend {
    /// 载入声码器模型。
    pub fn load(cfg: &ResamplerConfig) -> Result<Self> {
        let model_path = cfg.model_path(&cfg.vocoder.model);
        Self::load_from(&model_path, cfg)
    }

    /// 从指定路径载入。
    pub fn load_from(model_path: &std::path::Path, cfg: &ResamplerConfig) -> Result<Self> {
        use ort::session::Session;

        let mut builder =
            Session::builder().map_err(|e| anyhow::anyhow!("创建 ONNX 会话构造器失败: {e}"))?;

        if cfg.runtime.intra_op_threads > 0 {
            builder = builder
                .with_intra_threads(cfg.runtime.intra_op_threads)
                .map_err(|e| anyhow::anyhow!("设置 intra-op 线程数失败: {e}"))?;
        }
        if cfg.runtime.inter_op_threads > 0 {
            builder = builder
                .with_inter_threads(cfg.runtime.inter_op_threads)
                .map_err(|e| anyhow::anyhow!("设置 inter-op 线程数失败: {e}"))?;
        }

        builder = builder
            .with_execution_providers(build_providers(cfg))
            .map_err(|e| anyhow::anyhow!("注册执行提供者失败: {e}"))?;

        let session = builder
            .commit_from_file(model_path)
            .map_err(|e| anyhow::anyhow!("载入声码器模型失败 {model_path:?}: {e}"))?;

        // 以模型实际节点名为准：配置名命中则使用配置名，否则按顺序取前两个输入
        let inputs: Vec<String> = session
            .inputs()
            .iter()
            .map(|i| i.name().to_string())
            .collect();
        let outputs: Vec<String> = session
            .outputs()
            .iter()
            .map(|o| o.name().to_string())
            .collect();
        log::info!("声码器输入节点: {inputs:?}，输出节点: {outputs:?}");

        let mel_input = pick_name(&inputs, 0, &cfg.vocoder.mel_input);
        let f0_input = pick_name(&inputs, 1, &cfg.vocoder.f0_input);
        let output = pick_name(&outputs, 0, &cfg.vocoder.output);

        let mut channels_first = detect_channels_first(&session, &mel_input, cfg.mel.n_mels);
        match cfg.vocoder.mel_layout {
            MelLayout::Auto => {}
            MelLayout::ChannelsFirst => channels_first = true,
            MelLayout::FramesFirst => channels_first = false,
        }
        log::info!(
            "声码器 mel 布局: {}",
            if channels_first {
                "[1, n_mels, T]"
            } else {
                "[1, T, n_mels]"
            }
        );

        Ok(Self {
            session: Mutex::new(session),
            mel_input,
            f0_input,
            output,
            channels_first,
        })
    }
}

/// 依据模型元数据判断 `mel` 输入是通道在前还是帧在前。
///
/// 取声明形状里数值已知且等于 mel bin 数的那一维：位于第 1 维即通道在前。
fn detect_channels_first(session: &ort::session::Session, mel_input: &str, n_mels: usize) -> bool {
    for input in session.inputs() {
        if input.name() != mel_input {
            continue;
        }
        if let ort::value::ValueType::Tensor { shape, .. } = input.dtype() {
            for (axis, d) in shape.iter().enumerate() {
                if *d as usize == n_mels {
                    return axis == 1;
                }
            }
        }
    }
    // 无法确定时按最常见的导出形式（通道在前）处理
    true
}

fn pick_name(actual: &[String], index: usize, configured: &str) -> String {
    if actual.iter().any(|n| n == configured) {
        configured.to_string()
    } else {
        actual
            .get(index)
            .cloned()
            .unwrap_or_else(|| configured.to_string())
    }
}

/// 按配置构造执行提供者列表。
///
/// GPU 提供者（cuda / coreml / directml）需要编译期开启对应 feature，
/// 未开启时自动忽略并打印告警。
fn build_providers(cfg: &ResamplerConfig) -> Vec<ort::ep::ExecutionProviderDispatch> {
    use ort::ep::{ExecutionProviderDispatch, CPU};

    let mut providers: Vec<ExecutionProviderDispatch> = Vec::new();
    for p in &cfg.vocoder.providers {
        match p.to_lowercase().as_str() {
            "cpu" => providers.push(CPU::default().with_arena_allocator(true).build()),
            "cuda" => {
                #[cfg(feature = "cuda")]
                providers.push(ort::ep::CUDA::default().build());
                #[cfg(not(feature = "cuda"))]
                log::warn!("当前构建未启用 cuda feature，忽略 CUDA 提供者");
            }
            "coreml" => {
                #[cfg(feature = "coreml")]
                providers.push(ort::ep::CoreML::default().build());
                #[cfg(not(feature = "coreml"))]
                log::warn!("当前构建未启用 coreml feature，忽略 CoreML 提供者");
            }
            "directml" => {
                #[cfg(feature = "directml")]
                providers.push(ort::ep::DirectML::default().build());
                #[cfg(not(feature = "directml"))]
                log::warn!("当前构建未启用 directml feature，忽略 DirectML 提供者");
            }
            other => log::warn!("未知执行提供者: {other}"),
        }
    }
    if providers.is_empty() {
        providers.push(CPU::default().build());
    }
    providers
}

impl InferenceBackend for OrtBackend {
    fn name(&self) -> &str {
        "onnxruntime"
    }

    fn input_names(&self) -> (String, String) {
        (self.mel_input.clone(), self.f0_input.clone())
    }

    fn output_name(&self) -> String {
        self.output.clone()
    }

    fn infer(&self, mel: &[f32], n_frames: usize, n_mels: usize, f0: &[f32]) -> Result<Vec<f32>> {
        use ort::value::Tensor;

        if f0.len() != n_frames {
            anyhow::bail!("f0 帧数 {} 与 mel 帧数 {n_frames} 不一致", f0.len());
        }
        if mel.len() != n_frames * n_mels {
            anyhow::bail!("mel 数据长度 {} != {} × {n_mels}", mel.len(), n_frames);
        }

        // `mel` 以 [帧][bin] 行主序存储，按布局转成模型期望的张量
        let mel_data = if self.channels_first {
            let mut out = vec![0.0f32; n_frames * n_mels];
            for t in 0..n_frames {
                for m in 0..n_mels {
                    out[m * n_frames + t] = mel[t * n_mels + m];
                }
            }
            out
        } else {
            mel.to_vec()
        };
        let mel_shape = if self.channels_first {
            vec![1i64, n_mels as i64, n_frames as i64]
        } else {
            vec![1i64, n_frames as i64, n_mels as i64]
        };
        let mel_tensor = Tensor::from_array((mel_shape, mel_data)).context("构造 mel 张量失败")?;
        let f0_tensor = Tensor::from_array((vec![1i64, n_frames as i64], f0.to_vec()))
            .context("构造 f0 张量失败")?;

        let mut session = self
            .session
            .lock()
            .map_err(|e| anyhow::anyhow!("会话锁损坏: {e}"))?;
        let outputs = session
            .run(ort::inputs![
                self.mel_input.as_str() => mel_tensor,
                self.f0_input.as_str() => f0_tensor,
            ])
            .context("声码器推理失败")?;

        let (_shape, data) = outputs[self.output.as_str()]
            .try_extract_tensor::<f32>()
            .context("提取声码器输出失败")?;
        Ok(data.to_vec())
    }
}
