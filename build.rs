//! 构建脚本：注入构建信息（版本 / git 提交 / 目标平台 / ONNX Runtime 版本）。

use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=config/resampler.yaml");
    println!("cargo:rerun-if-changed=.git/HEAD");

    let git_hash = git_rev().unwrap_or_else(|| "unknown".to_string());
    let target = std::env::var("TARGET").unwrap_or_else(|_| "unknown".to_string());
    let profile = std::env::var("PROFILE").unwrap_or_else(|_| "unknown".to_string());
    let ort_version = std::env::var("DEP_ORT_VERSION").unwrap_or_else(|_| "n/a".to_string());

    let info = format!(
        "neural-resampler v{} (git:{git_hash}) target:{target} profile:{profile} onnxruntime:{ort_version}",
        env!("CARGO_PKG_VERSION")
    );
    println!("cargo:rustc-env=NEURAL_RESAMPLER_BUILD_INFO={info}");
}

fn git_rev() -> Option<String> {
    let output = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let s = String::from_utf8(output.stdout).ok()?.trim().to_string();
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}
