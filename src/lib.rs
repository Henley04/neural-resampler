//! # neural-resampler
//!
//! 通用神经重采样器引擎：纯 Rust + ONNX 重写 HiFiSampler，
//! 保留 PC-NSF-HiFiGAN 声码器，F0 提取替换为 FCPE（可回退到内置 DSP）。
//!
//! ```no_run
//! use neural_resampler::config::ResamplerConfig;
//! use neural_resampler::core::pipeline::Engine;
//! use neural_resampler::core::protocol::UtauParams;
//!
//! let cfg = ResamplerConfig::default_yaml()?;
//! let engine = Engine::new(cfg)?;
//! let params = UtauParams::default();
//! let stats = engine.run_pipeline(&params)?;
//! println!("{stats:?}");
//! # Ok::<(), anyhow::Error>(())
//! ```

pub mod adapters;
pub mod backend;
pub mod cache;
pub mod config;
pub mod core;

pub use crate::config::ResamplerConfig;
pub use crate::core::pipeline::{Engine, RenderRequest, RenderStats};
pub use crate::core::protocol::UtauParams;

/// crate 版本。
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// 构建信息（由 build.rs 注入）。
pub const BUILD_INFO: &str = env!("NEURAL_RESAMPLER_BUILD_INFO");

// ---------------------------------------------------------------------------
// C ABI（供其他语言通过 FFI 调用）
// ---------------------------------------------------------------------------

use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::panic::{catch_unwind, AssertUnwindSafe};

/// FFI 错误码。
pub const NR_OK: i32 = 0;
pub const NR_ERR_NULL_PTR: i32 = -1;
pub const NR_ERR_UTF8: i32 = -2;
pub const NR_ERR_RUNTIME: i32 = -3;
pub const NR_ERR_PANIC: i32 = -4;

thread_local! {
    static LAST_ERROR: std::cell::RefCell<CString> =
        std::cell::RefCell::new(CString::new("").unwrap_or_default());
}

fn set_last_error(msg: impl Into<String>) {
    LAST_ERROR.with(|cell| {
        *cell.borrow_mut() = CString::new(msg.into()).unwrap_or_default();
    });
}

/// 取回最近一次 FFI 错误的描述（线程局部，永不返回 NULL）。
///
/// 返回的字符串由本 crate 持有，调用方**不得**释放。
#[no_mangle]
pub extern "C" fn nr_last_error() -> *const c_char {
    LAST_ERROR.with(|cell| cell.borrow().as_ptr())
}

/// 以 JSON 描述的渲染请求执行渲染，返回 JSON 结果字符串（需用 [`nr_string_free`] 释放）。
///
/// JSON 字段与 [`UtauParams`] 对应：
/// `input`、`output`、`pitch`、`velocity`、`flags`、`offset`、`length`、`consonant`、
/// `cutoff`、`volume`、`modulation`、`tempo`、`pitch_bends`、`config`。
///
/// # Safety
/// `json` 必须是指向合法 C 字符串的指针。
#[no_mangle]
pub unsafe extern "C" fn nr_render_json(json: *const c_char) -> *mut c_char {
    if json.is_null() {
        set_last_error("json 指针为空");
        return std::ptr::null_mut();
    }
    let cstr = CStr::from_ptr(json);
    let text = match cstr.to_str() {
        Ok(s) => s.to_string(),
        Err(_) => {
            set_last_error("JSON 不是合法 UTF-8");
            return std::ptr::null_mut();
        }
    };

    let result = catch_unwind(AssertUnwindSafe(|| -> Result<String, String> {
        let value: serde_json::Value =
            serde_json::from_str(&text).map_err(|e| format!("JSON 解析失败: {e}"))?;
        let params = parse_params_from_json(&value)?;
        let cfg = match value.get("config").and_then(|c| c.as_str()) {
            Some(p) => ResamplerConfig::load(std::path::Path::new(p)).map_err(|e| e.to_string())?,
            None => ResamplerConfig::default_yaml().map_err(|e| e.to_string())?,
        };
        let engine = Engine::new(cfg).map_err(|e| e.to_string())?;
        let stats = engine.run_pipeline(&params).map_err(|e| e.to_string())?;
        serde_json::to_string(&serde_json::json!({
            "ok": true,
            "frames": stats.frames,
            "duration_ms": stats.duration_ms,
            "backend": stats.backend,
            "f0_backend": stats.f0_backend,
            "elapsed_ms": stats.elapsed_ms,
        }))
        .map_err(|e| format!("序列化结果失败: {e}"))
    }));

    match result {
        Ok(Ok(s)) => match CString::new(s) {
            Ok(c) => c.into_raw(),
            Err(_) => {
                set_last_error("结果包含 NUL 字节");
                std::ptr::null_mut()
            }
        },
        Ok(Err(e)) => {
            set_last_error(e);
            std::ptr::null_mut()
        }
        Err(_) => {
            set_last_error("渲染过程中发生 panic");
            std::ptr::null_mut()
        }
    }
}

fn parse_params_from_json(value: &serde_json::Value) -> Result<UtauParams, String> {
    let mut params = UtauParams::default();
    let get_str = |k: &str| value.get(k).and_then(|v| v.as_str()).map(|s| s.to_string());
    let get_f64 = |k: &str| value.get(k).and_then(|v| v.as_f64());

    if let Some(s) = get_str("input") {
        params.input_file = std::path::PathBuf::from(s);
    }
    if let Some(s) = get_str("output") {
        params.output_file = std::path::PathBuf::from(s);
    }
    if let Some(s) = get_str("pitch") {
        params.pitch = s.clone();
        params.pitch_midi = crate::core::protocol::note_to_midi(&s).ok();
    }
    if let Some(v) = get_f64("velocity") {
        params.velocity = v as f32;
    }
    if let Some(s) = get_str("flags") {
        params.flags = s.clone();
        params.flag_map = crate::core::protocol::parse_flags(&s);
    }
    if let Some(v) = get_f64("offset") {
        params.offset = v;
    }
    if let Some(v) = get_f64("length") {
        params.length_req = v;
    }
    if let Some(v) = get_f64("consonant") {
        params.consonant = v;
    }
    if let Some(v) = get_f64("cutoff") {
        params.cutoff = v;
    }
    if let Some(v) = get_f64("volume") {
        params.volume = v;
    }
    if let Some(v) = get_f64("modulation") {
        params.modulation = v;
    }
    if let Some(v) = get_f64("tempo") {
        params.tempo = v;
    }
    if let Some(arr) = value.get("pitch_bends").and_then(|v| v.as_array()) {
        params.pitch_bends = arr
            .iter()
            .filter_map(|v| v.as_f64())
            .map(|v| v as f32)
            .collect();
    }
    Ok(params)
}

/// 释放由本 crate 返回的 C 字符串。
///
/// # Safety
/// 指针必须来自本 crate 的返回值，且只能释放一次。
#[no_mangle]
pub unsafe extern "C" fn nr_string_free(s: *mut c_char) {
    if !s.is_null() {
        drop(CString::from_raw(s));
    }
}

/// 返回版本号字符串（静态，无需释放）。
#[no_mangle]
pub extern "C" fn nr_version() -> *const c_char {
    concat!(env!("CARGO_PKG_VERSION"), "\0").as_ptr() as *const c_char
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_is_exposed() {
        assert!(!VERSION.is_empty());
        assert!(BUILD_INFO.contains("neural-resampler"));
    }

    #[test]
    fn json_params_parsing() {
        let v: serde_json::Value = serde_json::json!({
            "input": "a.wav", "output": "b.wav", "pitch": "D4",
            "length": 500.0, "tempo": 100.0, "pitch_bends": [0.0, 10.0]
        });
        let p = parse_params_from_json(&v).unwrap();
        assert_eq!(p.pitch_midi, Some(62.0));
        assert_eq!(p.length_req, 500.0);
        assert_eq!(p.pitch_bends, vec![0.0, 10.0]);
    }
}
