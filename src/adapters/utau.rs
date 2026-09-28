//! UTAU / OpenUtau 适配器
//!
//! 协议（不含程序名）：
//!
//! ```text
//! in_file out_file pitch velocity flags offset length consonant cutoff volume modulation tempo pitch_string
//! ```

use crate::adapters::Adapter;
use crate::core::pipeline::{Engine, RenderStats};
use crate::core::protocol::{parse_utau_args, UtauParams};
use anyhow::{bail, Result};

/// UTAU / OpenUtau 统一适配器。
#[derive(Debug, Default)]
pub struct UtauAdapter;

impl UtauAdapter {
    pub fn new() -> Self {
        Self
    }
}

impl Adapter for UtauAdapter {
    fn name(&self) -> &str {
        "utau"
    }

    fn parse(&self, args: &[String]) -> Result<UtauParams> {
        if args.len() < 4 {
            bail!(
                "UTAU 参数不足：需要至少 4 个（in out pitch velocity），实际 {}",
                args.len()
            );
        }
        parse_utau_args(args)
    }

    fn render(&self, engine: &Engine, params: &UtauParams) -> Result<RenderStats> {
        engine.run_pipeline(params)
    }
}

/// 判断给定的命令行参数是否像 UTAU 调用（而非本程序的子命令/选项）。
pub fn looks_like_utau_invocation(args: &[String]) -> bool {
    if args.is_empty() {
        return false;
    }
    let first = args[0].as_str();
    if first.starts_with('-') {
        return false;
    }
    !matches!(
        first,
        "render" | "batch" | "info" | "selftest" | "config" | "help"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_reports_name() {
        let a = UtauAdapter::new();
        assert_eq!(a.name(), "utau");
        let args: Vec<String> = vec![
            "in.wav", "out.wav", "A4", "100", "", "0", "500", "0", "0", "100", "0", "!120", "AA",
        ]
        .into_iter()
        .map(|s| s.to_string())
        .collect();
        let p = a.parse(&args).unwrap();
        assert_eq!(p.pitch_midi, Some(69.0));
    }

    #[test]
    fn detects_invocation_style() {
        let utau: Vec<String> = vec!["in.wav".into(), "out.wav".into()];
        assert!(looks_like_utau_invocation(&utau));
        let cli: Vec<String> = vec!["render".into()];
        assert!(!looks_like_utau_invocation(&cli));
        let flag: Vec<String> = vec!["--help".into()];
        assert!(!looks_like_utau_invocation(&flag));
    }
}
