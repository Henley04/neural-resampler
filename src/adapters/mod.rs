//! 宿主适配层
//!
//! 统一处理 OpenUtau 与原生 UTAU：两者使用同一套 resampler 命令行协议，
//! 因此共用 [`utau::UtauAdapter`]；其他编辑器只需实现 [`Adapter`] 即可接入。

pub mod utau;

use crate::core::pipeline::{Engine, RenderStats};
use crate::core::UtauParams;
use anyhow::Result;

/// 宿主适配器接口。
pub trait Adapter {
    /// 适配器名称。
    fn name(&self) -> &str;

    /// 解析宿主传入的原始参数。
    fn parse(&self, args: &[String]) -> Result<UtauParams>;

    /// 执行渲染。
    fn render(&self, engine: &Engine, params: &UtauParams) -> Result<RenderStats>;
}
