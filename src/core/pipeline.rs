//! 核心渲染管线
//!
//! 流程（与原始 HiFiSampler 保持一致，F0 提取替换为 FCPE）：
//!
//! 1. 读取声库 WAV（必要时重采样到 44.1kHz）
//! 2. 提取分析用 Mel（origin_hop_size，默认 128）
//! 3. 计算时序：offset / consonant / cutoff / length → 拉伸映射
//! 4. 按拉伸映射重采样出渲染用 Mel（hop_size，默认 512）
//! 5. 生成 F0：乐谱（音高 + pitchBend）为主，FCPE 分析结果提供自然偏移与清浊
//! 6. PC-NSF-HiFiGAN 推理 → 波形
//! 7. 裁剪 + 后处理 → 写出 WAV

use crate::backend::{create_backend, InferenceBackend};
use crate::cache::f0_cache::F0Cache;
use crate::config::{F0Mode, ResamplerConfig};
use crate::core::audio::{read_wav, trim_silence, write_wav, AudioBuffer};
use crate::core::f0::{create_extractor, F0Extractor, F0Track};
use crate::core::feature::{compute_mel, sample_at, MelSpectrogram};
use crate::core::oto::OtoEntry;
use crate::core::post_process::{amplitude_modulate, finalize, growl};
use crate::core::protocol::{hz_to_midi, midi_to_hz, UtauParams};
use anyhow::{bail, Context as _, Result};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Instant;

/// 一次渲染请求（宿主无关的内部表示）。
#[derive(Debug, Clone)]
pub struct RenderRequest {
    pub input: PathBuf,
    pub output: PathBuf,
    /// 目标音高（MIDI 音符号）。
    pub pitch_midi: f32,
    /// pitchBend 曲线（cent）。
    pub pitch_bends: Vec<f32>,
    /// 力度（0~200）。
    pub velocity: f32,
    /// 左空白（毫秒）。
    pub offset_ms: f64,
    /// 要求长度（毫秒）。
    pub length_ms: f64,
    /// 子音部（毫秒）。
    pub consonant_ms: f64,
    /// 右侧空白（毫秒，通常 ≤ 0）。
    pub cutoff_ms: f64,
    /// 音量（%）。
    pub volume: f64,
    /// 调制（%）。
    pub modulation: f64,
    /// 速度（BPM）。
    pub tempo: f64,
    /// 解析后的 flags。
    pub flags: HashMap<String, Option<i32>>,
    /// 可选：oto.ini 条目（用于补齐宿主未传递的参数）。
    pub oto: Option<OtoEntry>,
}

impl RenderRequest {
    /// 由 UTAU 参数构造渲染请求。
    pub fn from_utau(params: &UtauParams, oto: Option<OtoEntry>) -> Result<Self> {
        let pitch_midi = params
            .pitch_midi
            .with_context(|| format!("无法解析音高: {}", params.pitch))?;
        let mut offset_ms = params.offset;
        let mut consonant_ms = params.consonant;
        let mut cutoff_ms = params.cutoff;

        // 宿主未传递（0）时回退到 oto.ini
        if let Some(oto) = &oto {
            if offset_ms == 0.0 {
                offset_ms = oto.offset;
            }
            if consonant_ms == 0.0 {
                consonant_ms = oto.consonant;
            }
            if cutoff_ms == 0.0 {
                cutoff_ms = oto.cutoff;
            }
        }

        Ok(Self {
            input: params.input_file.clone(),
            output: params.output_file.clone(),
            pitch_midi,
            pitch_bends: params.pitch_bends.clone(),
            velocity: params.velocity,
            offset_ms,
            length_ms: params.length_req,
            consonant_ms,
            cutoff_ms,
            volume: params.volume,
            modulation: params.modulation,
            tempo: params.tempo,
            flags: params.flag_map.clone(),
            oto,
        })
    }
}

/// 渲染统计信息。
#[derive(Debug, Clone, Default)]
pub struct RenderStats {
    pub backend: String,
    pub f0_backend: String,
    pub frames: usize,
    pub duration_ms: f64,
    pub cache_hit: bool,
    pub elapsed_ms: f64,
}

/// 渲染引擎（持有模型会话，建议复用）。
pub struct Engine {
    cfg: ResamplerConfig,
    backend: Box<dyn InferenceBackend>,
    f0: Option<Box<dyn F0Extractor>>,
    cache: Option<F0Cache>,
    digest: String,
}

impl Engine {
    /// 初始化引擎：载入声码器与 F0 模型（模型缺失时自动降级）。
    pub fn new(cfg: ResamplerConfig) -> Result<Self> {
        let backend = create_backend(&cfg)?;
        let f0 = create_extractor(&cfg).unwrap_or_else(|err| {
            log::warn!("F0 提取器初始化失败（{err}），将完全使用乐谱 F0");
            None
        });
        let digest = cfg.digest();
        let cache = if cfg.cache.enabled {
            Some(F0Cache::new(
                cfg.cache.dir.clone(),
                &cfg.cache.extension,
                cfg.cache.zstd_level,
            ))
        } else {
            None
        };
        log::info!(
            "引擎就绪：声码器后端 = {}，F0 后端 = {}",
            backend.name(),
            f0.as_ref().map(|e| e.name()).unwrap_or("score-only")
        );
        Ok(Self {
            cfg,
            backend,
            f0,
            cache,
            digest,
        })
    }

    pub fn config(&self) -> &ResamplerConfig {
        &self.cfg
    }

    pub fn backend_name(&self) -> &str {
        self.backend.name()
    }

    pub fn f0_backend_name(&self) -> &str {
        self.f0.as_ref().map(|e| e.name()).unwrap_or("score-only")
    }

    /// 执行 UTAU 协议的一次渲染。
    pub fn run_pipeline(&self, params: &UtauParams) -> Result<RenderStats> {
        let oto = crate::core::oto::find_oto_ini(&params.input_file)
            .and_then(|p| crate::core::oto::Oto::load(&p).ok())
            .and_then(|o| {
                o.get_for_wav(&params.input_file, Some(&params.pitch))
                    .cloned()
                    .or_else(|| o.get_by_alias(&params.pitch).cloned())
            });
        if let Some(entry) = &oto {
            log::debug!(
                "使用 oto.ini 条目: {} (offset={})",
                entry.alias,
                entry.offset
            );
        }
        let req = RenderRequest::from_utau(params, oto)?;
        self.render(&req)
    }

    /// 渲染并写出 WAV。
    pub fn render(&self, req: &RenderRequest) -> Result<RenderStats> {
        let started = Instant::now();
        let cfg = &self.cfg;
        let sr = cfg.mel.sample_rate;

        // ---- 1. 读取音频 ----
        let mut audio =
            read_wav(&req.input).with_context(|| format!("读取输入失败: {:?}", req.input))?;
        if audio.sample_rate != sr && !audio.is_empty() {
            let resampled =
                crate::core::audio::resample_sinc(&audio.samples, audio.sample_rate, sr)?;
            audio = AudioBuffer::new(resampled, sr);
        }
        if cfg.processing.trim_silence {
            let before = audio.len();
            audio = trim_silence(&audio, cfg.processing.silence_threshold_db);
            log::debug!("裁剪静音: {before} → {} 样本", audio.len());
        }
        if audio.is_empty() {
            bail!("输入音频为空: {:?}", req.input);
        }

        // ---- 2. flags ----
        let gender = flag_value_key(req, "g", cfg.processing.gender as i32) as f32;
        let key_shift = gender / 100.0;
        let t_flag = flag_value_key(req, "t", 0) as f32;
        let loop_mode = cfg.processing.loop_mode || req.flags.contains_key("He");

        // ---- 3. 电平归一（与参考实现一致：先压到 0.5 峰值）----
        let peak = audio.peak();
        let scale = if peak >= 0.5 { 0.5 / peak } else { 1.0 };
        let mut wave = audio.clone();
        wave.scale(scale);

        // ---- 4. 分析用 Mel ----
        let mel_origin = compute_mel(&wave.samples, &cfg.mel, cfg.mel.origin_hop_size, key_shift)
            .context("提取 Mel 频谱失败")?;
        if mel_origin.n_frames < 2 {
            bail!("音频过短，无法提取足够的 Mel 帧: {:?}", req.input);
        }
        let thop_origin = cfg.origin_frame_period();
        let t_area_origin: Vec<f64> = (0..mel_origin.n_frames)
            .map(|i| i as f64 * thop_origin + thop_origin / 2.0)
            .collect();
        let mut total_time = t_area_origin[t_area_origin.len() - 1] + thop_origin / 2.0;

        // ---- 5. 时序 ----
        let thop = cfg.frame_period();
        let start = req.offset_ms / 1000.0;
        let cutoff_s = req.cutoff_ms / 1000.0;
        let end = if req.cutoff_ms < 0.0 {
            start - cutoff_s
        } else {
            total_time - cutoff_s
        };
        let con = start + req.consonant_ms / 1000.0;
        let length_req = req.length_ms / 1000.0;
        let vel = 2f64.powf(1.0 - req.velocity as f64 / 100.0);
        let mut stretch_length = end - con;

        log::debug!(
            "时序: start={start:.3}s end={end:.3}s con={con:.3}s length_req={length_req:.3}s vel={vel:.3}"
        );

        // ---- 5b. 循环拼接（长音符）----
        let mel_origin = if loop_mode {
            let con_frame = ((con + thop_origin / 2.0) / thop_origin).floor() as usize;
            let end_frame = ((end + thop_origin / 2.0) / thop_origin).floor() as usize;
            let end_frame = end_frame.clamp(con_frame + 1, mel_origin.n_frames);
            let pad_loop_size = (length_req / thop_origin).floor() as usize + 1;
            let looped = loop_frames(&mel_origin, con_frame, end_frame, pad_loop_size);
            stretch_length = pad_loop_size as f64 * thop_origin;
            total_time = looped.n_frames as f64 * thop_origin;
            log::debug!("循环拼接: {} → {} 帧", mel_origin.n_frames, looped.n_frames);
            looped
        } else {
            mel_origin
        };
        let t_area_origin: Vec<f64> = (0..mel_origin.n_frames)
            .map(|i| i as f64 * thop_origin + thop_origin / 2.0)
            .collect();

        // 时序映射：输出（拉伸后）时间 → 源时间
        let scaling_ratio = if stretch_length < length_req && stretch_length > 1e-6 {
            length_req / stretch_length
        } else {
            1.0
        };
        let stretch = |t: f64| -> f64 {
            if t < vel * con {
                t / vel
            } else {
                con + (t - vel * con) / scaling_ratio
            }
        };

        let stretched_n_frames =
            ((con * vel + (total_time - con) * scaling_ratio) / thop).floor() as usize + 1;
        let stretched_n_frames = stretched_n_frames.max(2);

        // 首尾裁剪的帧数（保留 fill 帧余量）
        let start_left_mel_frames = ((start * vel + thop / 2.0) / thop).floor() as usize;
        let cut_left = start_left_mel_frames.saturating_sub(cfg.processing.fill);
        let end_right_mel_frames =
            stretched_n_frames - ((length_req + con * vel + thop / 2.0) / thop).floor() as usize;
        let cut_right = end_right_mel_frames.saturating_sub(cfg.processing.fill);
        let out_frames = stretched_n_frames
            .saturating_sub(cut_left + cut_right)
            .max(2);

        // 输出帧的绝对（拉伸后）时间
        let t_out_abs: Vec<f64> = (0..out_frames)
            .map(|j| (cut_left + j) as f64 * thop + thop / 2.0)
            .collect();
        // 对应的源时间
        let t_src: Vec<f64> = t_out_abs
            .iter()
            .map(|&t| stretch(t).clamp(0.0, t_area_origin[t_area_origin.len() - 1]))
            .collect();

        let new_start = start * vel - cut_left as f64 * thop;
        let new_end = (length_req + con * vel) - cut_left as f64 * thop;
        log::debug!("输出帧数 {out_frames}，裁剪区间 [{new_start:.3}, {new_end:.3}]s");

        // ---- 6. 渲染用 Mel ----
        let mel_render = interp_mel(&mel_origin, &t_area_origin, &t_src);

        // ---- 7. F0 ----
        let mut cache_hit = false;
        let (f0, f0_backend_name) =
            self.build_f0(req, &t_src, &t_out_abs, new_start, t_flag, &mut cache_hit)?;

        // ---- 8. 声码器推理 ----
        let wave_out = self
            .backend
            .infer(
                &mel_render.data,
                mel_render.n_frames,
                mel_render.n_mels,
                &f0,
            )
            .context("声码器推理失败")?;

        // ---- 9. 裁剪 ----
        let cut_start = (new_start * sr as f64).round().max(0.0) as usize;
        let cut_end = (new_end * sr as f64).round().max(0.0) as usize;
        let mut rendered: Vec<f32> = if cut_end > cut_start && cut_start < wave_out.len() {
            wave_out[cut_start..cut_end.min(wave_out.len())].to_vec()
        } else {
            log::warn!("裁剪区间异常，使用完整输出");
            wave_out.clone()
        };

        // ---- 10. 后处理 ----
        // A 标记：按音高变化做振幅调制
        let a_flag = flag_value_key(req, "A", 0);
        if a_flag != 0 && out_frames > 1 {
            let pitch_midi_curve: Vec<f32> = f0
                .iter()
                .map(|&f| if f > 0.0 { hz_to_midi(f) } else { 0.0 })
                .collect();
            let t_rel: Vec<f64> = (0..out_frames).map(|j| j as f64 * thop).collect();
            let mut gains = Vec::with_capacity(out_frames);
            for j in 0..out_frames {
                let prev = if j == 0 { 0 } else { j - 1 };
                let next = if j + 1 < out_frames { j + 1 } else { j };
                let dt = t_rel[next] - t_rel[prev];
                let dp = pitch_midi_curve[next] - pitch_midi_curve[prev];
                let deriv = if dt.abs() > 1e-9 { dp as f64 / dt } else { 0.0 };
                let a = (a_flag as f64).clamp(-100.0, 100.0);
                gains.push(5f32.powf((1e-4 * a * deriv) as f32));
            }
            amplitude_modulate(&mut rendered, &t_rel, &gains, sr);
        }

        // HG 标记：growl
        if let Some(hg) = req.flags.get("HG").copied().flatten() {
            growl(&mut rendered, sr, 80.0, hg as f32 / 100.0);
        }

        // 还原分析阶段的缩放
        let mut buffer = AudioBuffer::new(rendered, sr);
        if scale > 0.0 && scale != 1.0 {
            buffer.scale(1.0 / scale);
        }
        let new_max = buffer.peak();

        // 响度归一化（P 标记控制强度）
        if cfg.output.wave_norm {
            let target = cfg.output.loudness_target;
            let strength = req
                .flags
                .get("P")
                .copied()
                .flatten()
                .map(|v| (v as f32 / 100.0).clamp(0.0, 1.0))
                .unwrap_or(1.0);
            if strength > 0.0 {
                if strength < 1.0 {
                    let original = buffer.samples.clone();
                    crate::core::post_process::loudness_norm(
                        &mut buffer.samples,
                        sr,
                        target,
                        cfg.output.loudness_block_ms,
                    );
                    // 按强度在原始与归一化结果之间插值
                    for (s, o) in buffer.samples.iter_mut().zip(original.iter()) {
                        *s = o * (1.0 - strength) + *s * strength;
                    }
                } else {
                    crate::core::post_process::loudness_norm(
                        &mut buffer.samples,
                        sr,
                        target,
                        cfg.output.loudness_block_ms,
                    );
                }
            }
        }

        if new_max > cfg.output.peak_limit && new_max > 1e-9 {
            buffer.scale(cfg.output.peak_limit / new_max);
        }
        buffer.scale((req.volume / 100.0) as f32);

        // ---- 11. 定长 & 写出 ----
        let target_samples = ((new_end - new_start) * sr as f64).round().max(1.0) as usize;
        let final_buffer = finalize(buffer, &cfg.output, target_samples)?;
        write_wav(
            &req.output,
            &final_buffer.samples,
            final_buffer.sample_rate,
            cfg.output.bit_depth,
        )
        .with_context(|| format!("写出输出失败: {:?}", req.output))?;

        Ok(RenderStats {
            backend: self.backend.name().to_string(),
            f0_backend: f0_backend_name,
            frames: mel_render.n_frames,
            duration_ms: final_buffer.duration_ms(),
            cache_hit,
            elapsed_ms: started.elapsed().as_secs_f64() * 1000.0,
        })
    }

    /// 生成 F0 曲线（Hz，逐渲染帧）。
    fn build_f0(
        &self,
        req: &RenderRequest,
        t_src: &[f64],
        t_out_abs: &[f64],
        new_start: f64,
        t_flag: f32,
        cache_hit: &mut bool,
    ) -> Result<(Vec<f32>, String)> {
        let out_frames = t_out_abs.len();
        let thop = self.cfg.frame_period();

        // --- 乐谱 F0 ---
        // pitchBend 采样网格：60/(tempo*96) 秒/点（96 ticks/拍）
        let step = if req.tempo > 0.0 {
            60.0 / (req.tempo * 96.0)
        } else {
            0.005
        };
        let bends = if req.pitch_bends.is_empty() {
            vec![0.0f32]
        } else {
            req.pitch_bends.clone()
        };
        let t_pitch: Vec<f64> = (0..bends.len())
            .map(|i| new_start + i as f64 * step)
            .collect();
        // 输出帧的相对时间（与参考实现一致：以输出帧序号 × 帧移）
        let t_rel: Vec<f64> = (0..out_frames).map(|j| j as f64 * thop).collect();
        let pitch_curve: Vec<f32> = bends
            .iter()
            .map(|&c| c / 100.0 + req.pitch_midi + t_flag / 100.0)
            .collect();
        let pitch_at_frames = sample_at(&t_rel, &t_pitch, &pitch_curve);
        let f0_score: Vec<f32> = pitch_at_frames.iter().map(|&m| midi_to_hz(m)).collect();

        // --- 样本 F0（FCPE / WORLD）---
        let analyzer = match &self.f0 {
            Some(a) => a,
            None => return Ok((f0_score, "score-only".to_string())),
        };
        if matches!(self.cfg.f0.mode, F0Mode::Score) {
            // score 模式仍可用分析结果做清浊屏蔽
            if !self.cfg.f0.uv_mask {
                return Ok((f0_score, analyzer.name().to_string()));
            }
        }

        let (track, hit) = self.analyze_f0(req)?;
        *cache_hit = hit;
        if track.is_empty() {
            return Ok((f0_score, analyzer.name().to_string()));
        }

        // 把源 F0 对齐到渲染帧：按每个输出帧对应的源时间采样
        let src_times: Vec<f64> = t_src.to_vec();
        let track_times: Vec<f64> = (0..track.n_frames())
            .map(|i| i as f64 * track.frame_period_ms as f64 / 1000.0)
            .collect();
        let f0_src = sample_at(&src_times, &track_times, &track.f0);

        let mode = self.cfg.f0.mode;
        let mut f0 = Vec::with_capacity(out_frames);
        let modulation_scale = ((100.0 + req.modulation as f32) / 100.0).clamp(0.0, 4.0);

        // 源 F0 的平滑基准（用于提取相对偏移）
        let smoothing_frames = ((self.cfg.f0.smoothing_ms / track.frame_period_ms).round()
            as usize)
            .max(1)
            .min(track.n_frames().max(1));
        let smoothed = crate::core::feature::smooth(&track.f0, smoothing_frames);
        let dev_times: Vec<f64> = (0..smoothed.len())
            .map(|i| i as f64 * track.frame_period_ms as f64 / 1000.0)
            .collect();
        let smooth_at_frames = sample_at(&src_times, &dev_times, &smoothed);

        let src_mean_midi = {
            let m = track.median_voiced(0.0);
            if m > 0.0 {
                hz_to_midi(m)
            } else {
                0.0
            }
        };

        for j in 0..out_frames {
            let src = f0_src[j];
            let base = smooth_at_frames[j];
            let score = f0_score[j];
            let value = match mode {
                F0Mode::Score => score,
                F0Mode::Source => {
                    if src > 0.0 && src_mean_midi > 0.0 {
                        let score_midi = hz_to_midi(score);
                        let shift = score_midi - src_mean_midi;
                        src * 2f32.powf(shift / 12.0)
                    } else {
                        0.0
                    }
                }
                F0Mode::Hybrid => {
                    if src > 0.0 && base > 0.0 {
                        let mut dev_cents = 1200.0 * (src / base).log2();
                        dev_cents = dev_cents.clamp(
                            -self.cfg.f0.max_deviation_cents,
                            self.cfg.f0.max_deviation_cents,
                        ) * modulation_scale;
                        let score_midi = hz_to_midi(score);
                        midi_to_hz(score_midi + dev_cents / 100.0)
                    } else {
                        score
                    }
                }
            };
            // 清浊屏蔽：源为清音时输出清音
            if self.cfg.f0.uv_mask && src <= 0.0 && !matches!(mode, F0Mode::Score) {
                f0.push(0.0);
            } else {
                f0.push(value);
            }
        }

        Ok((f0, analyzer.name().to_string()))
    }

    /// 提取（或命中缓存）源音频的 F0。
    fn analyze_f0(&self, req: &RenderRequest) -> Result<(F0Track, bool)> {
        let analyzer = self.f0.as_ref().expect("调用前已检查存在");
        if let Some(cache) = &self.cache {
            if let Some(entry) = cache.load(&req.input, &self.digest) {
                log::debug!("F0 缓存命中: {:?}", req.input);
                return Ok((F0Track::new(entry.f0, entry.frame_period_ms), true));
            }
        }
        let audio = read_wav(&req.input)?;
        let track = analyzer.extract(&audio.samples, audio.sample_rate)?;
        if let Some(cache) = &self.cache {
            if let Err(err) = cache.store(&req.input, &self.digest, &track) {
                log::warn!("写入 F0 缓存失败: {err}");
            }
        }
        Ok((track, false))
    }
}

fn flag_value_key(req: &RenderRequest, key: &str, default: i32) -> i32 {
    req.flags.get(key).copied().flatten().unwrap_or(default)
}

/// 帧级循环拼接：把 [con_frame, end_frame) 区间反射延拓。
fn loop_frames(
    mel: &MelSpectrogram,
    con_frame: usize,
    end_frame: usize,
    pad: usize,
) -> MelSpectrogram {
    let head = con_frame.min(mel.n_frames);
    let loop_start = con_frame.min(mel.n_frames);
    let loop_end = end_frame.min(mel.n_frames).max(loop_start);
    let loop_len = loop_end - loop_start;

    let total = head + if loop_len == 0 { pad } else { loop_len + pad };
    let mut data = vec![0.0f32; total * mel.n_mels];

    // 头部
    for i in 0..head {
        data[i * mel.n_mels..(i + 1) * mel.n_mels].copy_from_slice(mel.frame(i));
    }
    // 循环段（反射延拓）
    for k in 0..(total - head) {
        let idx = if loop_len == 0 {
            loop_start
        } else {
            let m = k % (loop_len * 2);
            if m < loop_len {
                loop_start + m
            } else {
                loop_start + (loop_len * 2 - 1 - m)
            }
        };
        let idx = idx.min(mel.n_frames - 1);
        data[(head + k) * mel.n_mels..(head + k + 1) * mel.n_mels].copy_from_slice(mel.frame(idx));
    }

    MelSpectrogram {
        data,
        n_frames: total,
        n_mels: mel.n_mels,
        hop_size: mel.hop_size,
        sample_rate: mel.sample_rate,
    }
}

/// 按源时间插值 Mel（帧轴线性插值）。
fn interp_mel(mel: &MelSpectrogram, src_times: &[f64], query: &[f64]) -> MelSpectrogram {
    let n_mels = mel.n_mels;
    let mut data = vec![0.0f32; query.len() * n_mels];
    if mel.n_frames == 0 || query.is_empty() {
        return MelSpectrogram {
            data,
            n_frames: query.len(),
            n_mels,
            hop_size: mel.hop_size,
            sample_rate: mel.sample_rate,
        };
    }
    let last_time = src_times[src_times.len() - 1];
    let mut idx = 0usize;
    for (j, &t) in query.iter().enumerate() {
        let t = t.clamp(src_times[0], last_time);
        while idx + 1 < src_times.len() && src_times[idx + 1] < t {
            idx += 1;
        }
        let t0 = src_times[idx];
        let t1 = src_times[(idx + 1).min(src_times.len() - 1)];
        let frac = if (t1 - t0).abs() < 1e-12 {
            0.0
        } else {
            ((t - t0) / (t1 - t0)) as f32
        };
        let a = mel.frame(idx);
        let b = mel.frame((idx + 1).min(mel.n_frames - 1));
        let dst = &mut data[j * n_mels..(j + 1) * n_mels];
        for m in 0..n_mels {
            dst[m] = a[m] * (1.0 - frac) + b[m] * frac;
        }
    }
    MelSpectrogram {
        data,
        n_frames: query.len(),
        n_mels,
        hop_size: mel.hop_size,
        sample_rate: mel.sample_rate,
    }
}

/// 便捷入口：使用默认配置初始化引擎并执行 UTAU 渲染。
pub fn run_pipeline_default(params: &UtauParams) -> Result<RenderStats> {
    let cfg = ResamplerConfig::default_yaml()?;
    let engine = Engine::new(cfg)?;
    engine.run_pipeline(params)
}

/// 载入配置文件并初始化引擎。
pub fn engine_from_config(config: Option<&Path>, models_dir: Option<&Path>) -> Result<Engine> {
    let mut cfg = match config {
        Some(p) => ResamplerConfig::load(p)?,
        None => ResamplerConfig::default_yaml()?,
    };
    if let Some(dir) = models_dir {
        cfg.models_dir = dir.to_path_buf();
    }
    Engine::new(cfg)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sine_wav(path: &Path, sr: u32, freq: f32, ms: f64) {
        let n = (sr as f64 * ms / 1000.0) as usize;
        let samples: Vec<f32> = (0..n)
            .map(|i| (2.0 * std::f32::consts::PI * freq * i as f32 / sr as f32).sin() * 0.4)
            .collect();
        crate::core::audio::write_wav(path, &samples, sr, 16).unwrap();
    }

    #[test]
    fn request_from_utau_uses_oto_fallback() {
        let params = UtauParams {
            pitch: "C4".into(),
            pitch_midi: Some(60.0),
            length_req: 500.0,
            ..UtauParams::default()
        };
        let oto = OtoEntry {
            offset: 50.0,
            consonant: 30.0,
            cutoff: -100.0,
            ..OtoEntry::default_for("a.wav", "a")
        };
        let req = RenderRequest::from_utau(&params, Some(oto)).unwrap();
        assert_eq!(req.offset_ms, 50.0);
        assert_eq!(req.consonant_ms, 30.0);
        assert_eq!(req.cutoff_ms, -100.0);
    }

    #[test]
    fn end_to_end_render_with_stub_backend() {
        let dir = std::env::temp_dir().join("nr-pipeline-test");
        std::fs::create_dir_all(&dir).unwrap();
        let input = dir.join("in.wav");
        let output = dir.join("out.wav");
        sine_wav(&input, 44100, 220.0, 800.0);

        // 指向空目录，强制走 Stub 后端（不受开发机上已放置的模型影响）
        let mut cfg = ResamplerConfig::default_yaml().unwrap();
        cfg.models_dir = std::env::temp_dir().join("nr-pipeline-no-models");
        let engine = Engine::new(cfg).unwrap();
        assert_eq!(engine.backend_name(), "stub");

        let req = RenderRequest {
            input: input.clone(),
            output: output.clone(),
            pitch_midi: 60.0,
            pitch_bends: vec![0.0; 32],
            velocity: 100.0,
            offset_ms: 50.0,
            length_ms: 500.0,
            consonant_ms: 60.0,
            cutoff_ms: -50.0,
            volume: 100.0,
            modulation: 0.0,
            tempo: 120.0,
            flags: HashMap::new(),
            oto: None,
        };
        let stats = engine.render(&req).unwrap();
        assert!(stats.frames > 1, "渲染帧数异常: {}", stats.frames);
        assert!(output.exists(), "未生成输出文件");

        let written = crate::core::audio::read_wav(&output).unwrap();
        assert_eq!(written.sample_rate, 44100);
        assert!(!written.is_empty());
        // 输出长度应接近 length + consonant
        let expected_ms = 500.0 + 60.0;
        assert!(
            (written.duration_ms() - expected_ms).abs() < 60.0,
            "输出时长 {}ms 与预期 {expected_ms}ms 偏差过大",
            written.duration_ms()
        );
    }

    #[test]
    fn interp_mel_keeps_bin_count() {
        let cfg = ResamplerConfig::default_yaml().unwrap();
        let audio: Vec<f32> = (0..22050).map(|i| (i as f32 / 20.0).sin() * 0.3).collect();
        let mel = compute_mel(&audio, &cfg.mel, cfg.mel.origin_hop_size, 0.0).unwrap();
        let src_times: Vec<f64> = (0..mel.n_frames)
            .map(|i| i as f64 * cfg.origin_frame_period())
            .collect();
        let query: Vec<f64> = (0..40).map(|i| i as f64 * 0.01).collect();
        let out = interp_mel(&mel, &src_times, &query);
        assert_eq!(out.n_frames, 40);
        assert_eq!(out.n_mels, mel.n_mels);
    }

    #[test]
    fn loop_frames_extends() {
        let cfg = ResamplerConfig::default_yaml().unwrap();
        let audio: Vec<f32> = (0..22050).map(|i| (i as f32 / 20.0).sin() * 0.3).collect();
        let mel = compute_mel(&audio, &cfg.mel, cfg.mel.origin_hop_size, 0.0).unwrap();
        let looped = loop_frames(&mel, 5, 20, 30);
        assert_eq!(looped.n_frames, 5 + 15 + 30);
        assert_eq!(looped.n_mels, mel.n_mels);
    }
}
