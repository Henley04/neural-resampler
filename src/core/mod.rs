//! 核心引擎：音频 I/O、oto.ini、UTAU 协议、F0、特征、管线、后处理。

pub mod audio;
pub mod f0;
pub mod feature;
pub mod oto;
pub mod pipeline;
pub mod post_process;
pub mod protocol;

pub use audio::{read_wav, write_wav, AudioBuffer};
pub use f0::{F0Extractor, F0Track};
pub use feature::MelSpectrogram;
pub use oto::{Oto, OtoEntry};
pub use pipeline::{Engine, RenderRequest, RenderStats};
pub use protocol::UtauParams;
