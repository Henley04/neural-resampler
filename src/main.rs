//! neural-resampler CLI（`resampler` 可执行文件）
//!
//! * 直接以 UTAU 协议调用：`resampler in.wav out.wav C4 100 "" 0 500 0 0 100 0 !120 AA`
//! * 显式子命令：`render` / `batch` / `info` / `selftest` / `config`

use anyhow::{bail, Context as _, Result};
use clap::{Parser, Subcommand};
use neural_resampler::adapters::utau::{looks_like_utau_invocation, UtauAdapter};
use neural_resampler::adapters::Adapter;
use neural_resampler::config::ResamplerConfig;
use neural_resampler::core::pipeline::{engine_from_config, Engine};
use neural_resampler::core::protocol::UtauParams;
use std::path::{Path, PathBuf};

#[derive(Debug, Parser)]
#[command(
    name = "resampler",
    version,
    // UTAU 的 cutoff / offset 等参数可能为负数，必须允许 `-50` 这类取值
    allow_negative_numbers = true,
    about = "通用神经重采样器引擎（Rust + ONNX / FCPE + PC-NSF-HiFiGAN）",
    long_about = "UTAU / OpenUtau resampler 的纯 Rust 实现：保留 oto.ini 与声库 WAV，\
                  替换重采样引擎为神经声码器方案。直接传入 UTAU 的 13 个参数即可作为 resampler 使用。"
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// 配置文件路径
    #[arg(long, global = true, value_name = "FILE")]
    config: Option<PathBuf>,

    /// 模型根目录（覆盖配置中的 models_dir）
    #[arg(long, global = true, value_name = "DIR")]
    models: Option<PathBuf>,

    /// 日志级别（error/warn/info/debug/trace）
    #[arg(long, global = true, default_value = "info")]
    log_level: String,

    /// 日志格式：text 或 json
    #[arg(long, global = true, default_value = "text")]
    log_format: String,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// 按 UTAU 协议渲染单个音符
    #[command(allow_negative_numbers = true)]
    Render(RenderArgs),
    /// 批量渲染：每行一组 UTAU 参数
    Batch {
        /// 参数清单文件
        #[arg(value_name = "LIST_FILE")]
        list: PathBuf,
        /// 并行线程数（0 = 自动）
        #[arg(long, default_value_t = 0)]
        jobs: usize,
    },
    /// 打印构建信息、配置与模型状态
    Info,
    /// 自检：生成测试音并跑通整条管线
    Selftest {
        /// 输出目录
        #[arg(long, default_value = "target/selftest")]
        out_dir: PathBuf,
    },
    /// 导出默认配置为 YAML
    Config {
        /// 输出路径
        #[arg(value_name = "FILE")]
        output: PathBuf,
    },
}

#[derive(Debug, clap::Args)]
struct RenderArgs {
    /// 输入 WAV
    #[arg(value_name = "IN")]
    input: PathBuf,
    /// 输出 WAV
    #[arg(value_name = "OUT")]
    output: PathBuf,
    /// 音名（如 C4）
    #[arg(default_value = "C4")]
    pitch: String,
    /// 力度
    #[arg(default_value_t = 100.0)]
    velocity: f32,
    /// flags
    #[arg(default_value = "")]
    flags: String,
    /// 左空白（毫秒）
    #[arg(default_value_t = 0.0)]
    offset: f64,
    /// 要求长度（毫秒）
    #[arg(default_value_t = 1000.0)]
    length: f64,
    /// 子音部（毫秒）
    #[arg(default_value_t = 0.0)]
    consonant: f64,
    /// 右侧空白（毫秒）
    #[arg(default_value_t = 0.0)]
    cutoff: f64,
    /// 音量（%）
    #[arg(default_value_t = 100.0)]
    volume: f64,
    /// 调制（%）
    #[arg(default_value_t = 0.0)]
    modulation: f64,
    /// 速度（!BPM 或 BPM）
    #[arg(default_value = "!120")]
    tempo: String,
    /// pitchBend 字符串
    #[arg(default_value = "AA")]
    pitch_string: String,
}

/// 全局选项名（可出现在裸 UTAU 调用之前）。
const GLOBAL_FLAGS: [&str; 4] = ["--config", "--models", "--log-level", "--log-format"];

/// 把开头的全局选项与后续参数分开。
///
/// 这样 `resampler --models /path in.wav out.wav C4 ...` 也能走 UTAU 协议路径，
/// 便于在不改动宿主调用方式的前提下指定模型目录。
fn split_global_flags(raw: &[String]) -> (Vec<String>, Vec<String>) {
    let mut globals = Vec::new();
    let mut i = 0;
    while i < raw.len() {
        let a = raw[i].as_str();
        let name = a.split('=').next().unwrap_or(a);
        if GLOBAL_FLAGS.contains(&name) {
            globals.push(a.to_string());
            if !a.contains('=') {
                if let Some(v) = raw.get(i + 1) {
                    globals.push(v.clone());
                    i += 1;
                }
            }
            i += 1;
        } else {
            break;
        }
    }
    (globals, raw[i..].to_vec())
}

/// 从分离出的全局选项里取出常用项（配置路径、模型目录、日志）。
fn globals_to_cli(globals: &[String]) -> Cli {
    let mut args = vec!["resampler".to_string()];
    args.extend_from_slice(globals);
    args.push("info".to_string()); // 占位子命令，保证能解析
    Cli::parse_from(args)
}

fn main() -> Result<()> {
    let raw: Vec<String> = std::env::args().skip(1).collect();
    let (globals, rest) = split_global_flags(&raw);

    // UTAU / OpenUtau 调用：13 个位置参数（最少 4 个）
    if looks_like_utau_invocation(&rest) {
        let cli = globals_to_cli(&globals);
        init_logger(&cli.log_level, &cli.log_format);
        let adapter = UtauAdapter::new();
        let params = adapter.parse(&rest)?;
        let engine = engine_from_config(cli.config.as_deref(), cli.models.as_deref())?;
        let stats = adapter.render(&engine, &params)?;
        log::info!(
            "渲染完成: {:?} → {:?}（{} 帧 / {:.1}ms / {:.1}ms）",
            params.input_file,
            params.output_file,
            stats.frames,
            stats.duration_ms,
            stats.elapsed_ms
        );
        return Ok(());
    }

    let cli = Cli::parse_from(std::env::args());
    init_logger(&cli.log_level, &cli.log_format);

    match cli.command.as_ref() {
        Some(Commands::Render(a)) => cmd_render(&cli, a),
        Some(Commands::Batch { list, jobs }) => cmd_batch(&cli, list, *jobs),
        Some(Commands::Info) => cmd_info(&cli),
        Some(Commands::Selftest { out_dir }) => cmd_selftest(&cli, out_dir),
        Some(Commands::Config { output }) => cmd_config(&cli, output),
        None => {
            // 没有子命令且不是 UTAU 调用 → 打印帮助
            Cli::parse_from(["resampler", "--help"]);
            Ok(())
        }
    }
}

fn init_logger(level: &str, format: &str) {
    use std::str::FromStr;
    let level = log::LevelFilter::from_str(level).unwrap_or(log::LevelFilter::Info);
    let mut builder = env_logger::Builder::new();
    builder.filter_level(level);
    if format.eq_ignore_ascii_case("json") {
        builder.format(|buf, record| {
            use std::io::Write;
            writeln!(
                buf,
                "{{\"level\":\"{}\",\"target\":\"{}\",\"message\":\"{}\"}}",
                record.level(),
                record.target(),
                record.args()
            )
        });
    } else {
        builder.format_timestamp_millis();
    }
    let _ = builder.try_init();
}

fn build_engine(cli: &Cli) -> Result<Engine> {
    engine_from_config(cli.config.as_deref(), cli.models.as_deref())
}

fn cmd_render(cli: &Cli, a: &RenderArgs) -> Result<()> {
    let params = UtauParams {
        input_file: a.input.clone(),
        output_file: a.output.clone(),
        pitch: a.pitch.clone(),
        pitch_midi: neural_resampler::core::protocol::note_to_midi(&a.pitch).ok(),
        velocity: a.velocity,
        flags: a.flags.clone(),
        flag_map: neural_resampler::core::protocol::parse_flags(&a.flags),
        offset: a.offset,
        length_req: a.length,
        consonant: a.consonant,
        cutoff: a.cutoff,
        volume: a.volume,
        modulation: a.modulation,
        tempo: neural_resampler::core::protocol::parse_tempo(&a.tempo),
        pitch_bends: neural_resampler::core::protocol::decode_pitch_bends(&a.pitch_string)?,
    };
    if params.pitch_midi.is_none() {
        bail!("无法解析音名: {}", a.pitch);
    }
    let engine = build_engine(cli)?;
    let stats = engine.run_pipeline(&params)?;
    println!(
        "已写出 {:?}：{} 帧 / {:.1}ms / 后端 {} / F0 {} / 耗时 {:.1}ms",
        a.output,
        stats.frames,
        stats.duration_ms,
        stats.backend,
        stats.f0_backend,
        stats.elapsed_ms
    );
    Ok(())
}

fn cmd_batch(cli: &Cli, list: &Path, jobs: usize) -> Result<()> {
    let text = std::fs::read_to_string(list).with_context(|| format!("读取清单失败: {list:?}"))?;
    let lines: Vec<Vec<String>> = text
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(|l| l.split_whitespace().map(|s| s.to_string()).collect())
        .collect();
    if lines.is_empty() {
        bail!("清单为空: {list:?}");
    }

    // 整个批量过程共享一个引擎：Engine 的推理会话由 Mutex 保护且整体
    // Send+Sync，可安全地跨线程复用。之前每行都重建引擎，97MB 的 ONNX
    // 模型被反复加载（每行多 ~80ms），多会话内部线程互相争抢还会让
    // --jobs 的并行收益变成负数。
    let engine = std::sync::Arc::new(build_engine(cli)?);

    if jobs > 1 {
        let pool = rayon::ThreadPoolBuilder::new().num_threads(jobs).build()?;
        pool.install(|| {
            lines
                .par_iter()
                .map(|args| render_one(&engine, args))
                .collect::<Result<Vec<_>>>()
        })?;
    } else {
        for args in &lines {
            render_one(&engine, args)?;
        }
    }
    println!("批量渲染完成：{} 条", lines.len());
    Ok(())
}

fn render_one(engine: &Engine, args: &[String]) -> Result<()> {
    let adapter = UtauAdapter::new();
    let params = adapter.parse(args)?;
    let stats = adapter.render(engine, &params)?;
    println!(
        "{:?} → {:?}（{} 帧 / {:.1}ms）",
        params.input_file, params.output_file, stats.frames, stats.duration_ms
    );
    Ok(())
}

use rayon::prelude::*;

fn cmd_info(cli: &Cli) -> Result<()> {
    let cfg = match cli.config.as_deref() {
        Some(p) => ResamplerConfig::load(p)?,
        None => ResamplerConfig::default_yaml()?,
    };
    let mut cfg = cfg;
    if let Some(dir) = neural_resampler::core::pipeline::resolve_models_dir(cli.models.as_deref()) {
        cfg.models_dir = dir;
    }
    let vocoder = cfg.model_path(&cfg.vocoder.model);
    let fcpe = cfg.model_path(&cfg.f0.model);

    println!("neural-resampler {}", neural_resampler::VERSION);
    println!("构建信息: {}", neural_resampler::BUILD_INFO);
    println!("配置指纹: {}", cfg.digest());
    println!("--- 模型 ---");
    println!("声码器  : {vocoder:?}  [{}]", exists_mark(&vocoder));
    println!("FCPE    : {fcpe:?}  [{}]", exists_mark(&fcpe));
    if let Some(hn) = &cfg.vocoder.hnsep_model {
        let p = cfg.model_path(hn);
        println!("HN-SEP  : {p:?}  [{}]", exists_mark(&p));
    }
    println!("--- 频谱 / F0 ---");
    println!(
        "采样率  : {} Hz，Mel {} bin，帧移 {}/{}",
        cfg.mel.sample_rate, cfg.mel.n_mels, cfg.mel.origin_hop_size, cfg.mel.hop_size
    );
    println!("F0 后端 : {:?} / 模式 {:?}", cfg.f0.backend, cfg.f0.mode);
    let engine = build_engine(cli)?;
    println!("--- 运行时 ---");
    println!("声码器后端: {}", engine.backend_name());
    println!("F0 后端    : {}", engine.f0_backend_name());
    Ok(())
}

fn exists_mark(p: &Path) -> &'static str {
    if p.exists() {
        "已就绪"
    } else {
        "缺失"
    }
}

fn cmd_selftest(cli: &Cli, out_dir: &Path) -> Result<()> {
    std::fs::create_dir_all(out_dir)?;
    let input = out_dir.join("selftest_input.wav");
    let output = out_dir.join("selftest_output.wav");

    // 生成一段 440Hz 测试音（1 秒）
    let sr = 44100u32;
    let samples: Vec<f32> = (0..sr as usize)
        .map(|i| {
            let t = i as f32 / sr as f32;
            (2.0 * std::f32::consts::PI * 440.0 * t).sin() * 0.4
                + 0.05 * (2.0 * std::f32::consts::PI * 880.0 * t).sin()
        })
        .collect();
    neural_resampler::core::audio::write_wav(&input, &samples, sr, 16)?;
    println!("已生成测试输入: {input:?}");

    let params = UtauParams {
        input_file: input.clone(),
        output_file: output.clone(),
        pitch: "A4".into(),
        pitch_midi: Some(69.0),
        velocity: 100.0,
        flags: String::new(),
        flag_map: Default::default(),
        offset: 50.0,
        length_req: 500.0,
        consonant: 60.0,
        cutoff: -50.0,
        volume: 100.0,
        modulation: 0.0,
        tempo: 120.0,
        pitch_bends: vec![0.0; 64],
    };
    let engine = build_engine(cli)?;
    let stats = engine.run_pipeline(&params)?;
    println!("渲染完成: {stats:?}");
    let written = neural_resampler::core::audio::read_wav(&output)?;
    println!(
        "输出: {output:?}（{} 样本 / {:.1}ms / 峰值 {:.3}）",
        written.len(),
        written.duration_ms(),
        written.peak()
    );
    if written.is_empty() {
        bail!("自检失败：输出为空");
    }
    println!("自检通过");
    Ok(())
}

fn cmd_config(cli: &Cli, output: &Path) -> Result<()> {
    let cfg = match cli.config.as_deref() {
        Some(p) => ResamplerConfig::load(p)?,
        None => ResamplerConfig::default_yaml()?,
    };
    cfg.save(output)?;
    println!("已导出配置: {output:?}");
    Ok(())
}
