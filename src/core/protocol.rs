//! UTAU / OpenUtau resampler 命令行协议解析
//!
//! 协议格式（1 号参数起）：
//!
//! ```text
//! in_file out_file pitch velocity flags offset length consonant cutoff volume modulation tempo pitch_string
//! ```
//!
//! `pitch_string` 是 Base64 + RLE 编码的 pitchBend：每两个 Base64 字符组成一个
//! 有符号 12 位整数（-2048~2047），单位为 cent；`#` 之后紧跟重复次数（RLE）。
//! 例：`AB#5#CD` 表示 `AB` 解码出的值重复 5 次，再接 `CD` 解码出的值。

use anyhow::{anyhow, bail, Result};
use std::collections::HashMap;
use std::path::PathBuf;

/// 解析后的 UTAU 参数。
#[derive(Debug, Clone, PartialEq)]
pub struct UtauParams {
    /// 输入声库 WAV。
    pub input_file: PathBuf,
    /// 输出 WAV。
    pub output_file: PathBuf,
    /// 音名（如 `C4`）；无法解析时退化为 [`UtauParams::pitch_midi`] = None。
    pub pitch: String,
    /// 音高（MIDI 音符号），解析失败为 None。
    pub pitch_midi: Option<f32>,
    /// 力度（0~200）。
    pub velocity: f32,
    /// 原始 flags 字符串。
    pub flags: String,
    /// 解析后的 flags。
    pub flag_map: HashMap<String, Option<i32>>,
    /// 左空白（毫秒）。
    pub offset: f64,
    /// 要求长度（毫秒）。
    pub length_req: f64,
    /// 子音部（毫秒，固定不拉伸）。
    pub consonant: f64,
    /// 右侧空白（毫秒，通常 ≤ 0）。
    pub cutoff: f64,
    /// 音量（%）。
    pub volume: f64,
    /// 调制（%）。
    pub modulation: f64,
    /// 速度（BPM），`!120` 形式。
    pub tempo: f64,
    /// pitchBend 曲线（cent）。
    pub pitch_bends: Vec<f32>,
}

impl Default for UtauParams {
    fn default() -> Self {
        Self {
            input_file: PathBuf::new(),
            output_file: PathBuf::new(),
            pitch: "C4".into(),
            pitch_midi: Some(60.0),
            velocity: 100.0,
            flags: String::new(),
            flag_map: HashMap::new(),
            offset: 0.0,
            length_req: 1000.0,
            consonant: 0.0,
            cutoff: 0.0,
            volume: 100.0,
            modulation: 0.0,
            tempo: 120.0,
            pitch_bends: Vec::new(),
        }
    }
}

const NOTE_OFFSETS: [(&str, i32); 12] = [
    ("C", 0),
    ("C#", 1),
    ("D", 2),
    ("D#", 3),
    ("E", 4),
    ("F", 5),
    ("F#", 6),
    ("G", 7),
    ("G#", 8),
    ("A", 9),
    ("A#", 10),
    ("B", 11),
];

/// 音名 → MIDI 音符号（UTAU 约定：C4 = 60）。
pub fn note_to_midi(name: &str) -> Result<f32> {
    let name = name.trim();
    // 从尾部扫描八度数字（可能带负号）
    let mut idx = name.len();
    while idx > 0 {
        let b = name.as_bytes()[idx - 1];
        if b.is_ascii_digit() || b == b'-' {
            idx -= 1;
        } else {
            break;
        }
    }
    let (note_part, octave_part) = name.split_at(idx);
    let note_part = note_part.trim().to_uppercase();
    let semitone = NOTE_OFFSETS
        .iter()
        .find(|(n, _)| *n == note_part.as_str())
        .map(|(_, v)| *v)
        .ok_or_else(|| anyhow!("无法解析音名: {name}"))?;
    let octave: i32 = octave_part
        .trim()
        .parse()
        .map_err(|_| anyhow!("无法解析八度: {name}"))?;
    Ok(((octave + 1) * 12 + semitone) as f32)
}

/// MIDI 音符号 → 频率（Hz，A4 = 440）。
pub fn midi_to_hz(midi: f32) -> f32 {
    440.0 * 2f32.powf((midi - 69.0) / 12.0)
}

/// 频率（Hz）→ MIDI 音符号。
pub fn hz_to_midi(hz: f32) -> f32 {
    if hz <= 0.0 {
        return 0.0;
    }
    69.0 + 12.0 * (hz / 440.0).log2()
}

/// 解析 UTAU resampler 的 13 个位置参数（不含程序名）。
pub fn parse_utau_args(args: &[String]) -> Result<UtauParams> {
    if args.len() < 4 {
        bail!(
            "UTAU 参数不足（至少需要 in/out/pitch/velocity），实际 {} 个: {:?}",
            args.len(),
            args
        );
    }

    let input_file = PathBuf::from(&args[0]);
    let output_file = PathBuf::from(&args[1]);
    let pitch = args[2].clone();
    let pitch_midi = note_to_midi(&pitch).ok();
    let velocity = args
        .get(3)
        .and_then(|s| s.parse::<f32>().ok())
        .unwrap_or(100.0);
    let flags = args.get(4).cloned().unwrap_or_default();
    let offset = args
        .get(5)
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(0.0);
    let length_req = args
        .get(6)
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(1000.0);
    let consonant = args
        .get(7)
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(0.0);
    let cutoff = args
        .get(8)
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(0.0);
    let volume = args
        .get(9)
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(100.0);
    let modulation = args
        .get(10)
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(0.0);
    let tempo = parse_tempo(args.get(11).map(|s| s.as_str()).unwrap_or("!120"));
    let pitch_string = args.get(12).map(|s| s.as_str()).unwrap_or("AA");
    let pitch_bends = decode_pitch_bends(pitch_string)?;

    let flag_map = parse_flags(&flags);

    Ok(UtauParams {
        input_file,
        output_file,
        pitch,
        pitch_midi,
        velocity,
        flags,
        flag_map,
        offset,
        length_req,
        consonant,
        cutoff,
        volume,
        modulation,
        tempo,
        pitch_bends,
    })
}

/// 解析 `!120` 形式的 tempo。
pub fn parse_tempo(s: &str) -> f64 {
    let s = s.trim();
    let s = s.strip_prefix('!').unwrap_or(s);
    s.parse::<f64>().unwrap_or(120.0)
}

/// 将单个 Base64 字符转换为 6 位无符号整数（标准 Base64 字母表）。
pub fn to_uint6(c: char) -> Result<u16> {
    let v = c as u32;
    let val = match v {
        0x41..=0x5A => v - 65, // A-Z
        0x61..=0x7A => v - 71, // a-z
        0x30..=0x39 => v + 4,  // 0-9
        0x2B => 62,            // +
        0x2F => 63,            // /
        _ => bail!("非法 Base64 字符: {c}"),
    };
    Ok(val as u16)
}

/// 两个 Base64 字符 → 有符号 12 位整数。
pub fn to_int12(pair: &str) -> Result<i32> {
    let mut chars = pair.chars();
    let hi = chars.next().ok_or_else(|| anyhow!("Base64 数据不足"))?;
    let lo = chars.next().ok_or_else(|| anyhow!("Base64 数据不足"))?;
    let uint12 = (to_uint6(hi)? << 6) | to_uint6(lo)?;
    Ok(if (uint12 >> 11) & 1 == 1 {
        uint12 as i32 - 4096
    } else {
        uint12 as i32
    })
}

/// 解码 UTAU pitchBend 字符串 → 逐点 cent 偏移。
///
/// 与 UTAU 一致，末尾补一个 0，保证曲线长度大于 0。
pub fn decode_pitch_bends(pitch_string: &str) -> Result<Vec<f32>> {
    if pitch_string.trim().is_empty() {
        return Ok(vec![0.0]);
    }
    let mut out: Vec<f32> = Vec::new();
    // 以 '#' 分段：<b64>#<rle>#<b64>#<rle>#...#<b64>
    let parts: Vec<&str> = pitch_string.split('#').collect();
    let mut i = 0;
    while i < parts.len() {
        let chunk = parts[i];
        if chunk.is_empty() {
            i += 1;
            continue;
        }
        let mut chars = chunk.chars();
        while let Some(hi) = chars.next() {
            let lo = match chars.next() {
                Some(c) => c,
                None => {
                    // 长度不成对时丢弃末尾半个字符，避免整个音符渲染失败
                    log::warn!("pitchBend 段长度非偶数，丢弃尾字符: {chunk:?}");
                    break;
                }
            };
            if lo == '#' {
                bail!("pitchBend 数据格式错误: {chunk}");
            }
            let pair = format!("{hi}{lo}");
            let v = to_int12(&pair)?;
            out.push(v as f32);
        }
        // 跟随的 RLE 次数
        if let Some(rle) = parts.get(i + 1) {
            if let Ok(count) = rle.parse::<i32>() {
                if count > 0 {
                    if let Some(&last) = out.last() {
                        // 参考实现：RLE 重复最后一个解码值
                        for _ in 0..count {
                            out.push(last);
                        }
                    }
                }
                i += 2;
                continue;
            }
        }
        i += 1;
    }

    out.push(0.0);
    Ok(out)
}

/// HiFiSampler 兼容的 flags 集合。
pub const KNOWN_FLAGS: [&str; 24] = [
    "fe", "fl", "fo", "fv", "fp", "ve", "vo", "g", "t", "A", "B", "G", "P", "S", "p", "R", "D",
    "C", "Z", "Hv", "Hb", "Ht", "He", "HG",
];

/// 解析 flags 字符串（形如 `g-3B50P60`）。
pub fn parse_flags(flags: &str) -> HashMap<String, Option<i32>> {
    let mut map = HashMap::new();
    let cleaned: String = flags.chars().filter(|&c| c != '/').collect();
    let known: Vec<&str> = {
        let mut v: Vec<&str> = KNOWN_FLAGS.to_vec();
        // 长标记优先匹配
        v.sort_by_key(|b| std::cmp::Reverse(b.len()));
        v
    };

    let chars: Vec<char> = cleaned.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let matched = known.iter().find(|&&f| {
            let fl: Vec<char> = f.chars().collect();
            chars[i..].len() >= fl.len() && chars[i..i + fl.len()] == fl[..]
        });
        match matched {
            Some(&flag) => {
                let flen = flag.chars().count();
                i += flen;
                // 读取可选整数
                let start = i;
                if i < chars.len() && (chars[i] == '+' || chars[i] == '-') {
                    i += 1;
                }
                while i < chars.len() && chars[i].is_ascii_digit() {
                    i += 1;
                }
                let num_str: String = chars[start..i].iter().collect();
                let value = if num_str.is_empty() {
                    None
                } else {
                    num_str.parse::<i32>().ok()
                };
                map.entry(flag.to_string()).or_insert(value);
            }
            None => {
                i += 1; // 跳过未知字符
            }
        }
    }
    map
}

/// 取 flag 整数值，缺省返回 `default`。
pub fn flag_value(params: &UtauParams, key: &str, default: i32) -> i32 {
    params
        .flag_map
        .get(key)
        .copied()
        .flatten()
        .unwrap_or(default)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn note_names_to_midi() {
        assert_eq!(note_to_midi("C4").unwrap(), 60.0);
        assert_eq!(note_to_midi("A4").unwrap(), 69.0);
        assert_eq!(note_to_midi("C#4").unwrap(), 61.0);
        assert_eq!(note_to_midi("B3").unwrap(), 59.0);
        assert_eq!(note_to_midi("C-1").unwrap(), 0.0);
    }

    #[test]
    fn midi_freq_roundtrip() {
        assert!((midi_to_hz(69.0) - 440.0).abs() < 1e-5);
        assert!((hz_to_midi(440.0) - 69.0).abs() < 1e-5);
    }

    #[test]
    fn base64_int12_mapping() {
        // 'A' = 0, 'A' = 0 -> 0
        assert_eq!(to_int12("AA").unwrap(), 0);
        // '/' = 63, '/' = 63 -> 4095 -> -1
        assert_eq!(to_int12("//").unwrap(), -1);
        // 'B' = 1, 'A' = 0 -> 1<<6 | 0 = 64
        assert_eq!(to_int12("BA").unwrap(), 64);
        // 最高位（bit 11）为 1 时按补码取负：2080 -> -2016
        assert_eq!(to_int12("gg").unwrap(), -2016);
    }

    #[test]
    fn decode_pitch_bends_basic() {
        // "AA" -> [0]，末尾补 0
        assert_eq!(decode_pitch_bends("AA").unwrap(), vec![0.0, 0.0]);
        // RLE：AA 后跟 3 次重复
        let out = decode_pitch_bends("AA#3").unwrap();
        assert_eq!(out.len(), 1 + 3 + 1);
        assert!(out.iter().all(|&v| v == 0.0));
    }

    #[test]
    fn decode_pitch_bends_with_values() {
        // "BA" -> 64，末尾补 0
        let out = decode_pitch_bends("BA").unwrap();
        assert_eq!(out, vec![64.0, 0.0]);
    }

    #[test]
    fn flags_parsing() {
        let map = parse_flags("g-3B50P60/t10");
        assert_eq!(map.get("g").copied().flatten(), Some(-3));
        assert_eq!(map.get("B").copied().flatten(), Some(50));
        assert_eq!(map.get("P").copied().flatten(), Some(60));
        assert_eq!(map.get("t").copied().flatten(), Some(10));
    }

    #[test]
    fn parses_full_utau_line() {
        let args: Vec<String> = vec![
            "in.wav", "out.wav", "C4", "100", "g-3", "50", "500", "100", "-200", "100", "0",
            "!120", "AA#3",
        ]
        .into_iter()
        .map(|s| s.to_string())
        .collect();
        let p = parse_utau_args(&args).unwrap();
        assert_eq!(p.input_file, PathBuf::from("in.wav"));
        assert_eq!(p.output_file, PathBuf::from("out.wav"));
        assert_eq!(p.pitch_midi, Some(60.0));
        assert_eq!(p.velocity, 100.0);
        assert_eq!(p.offset, 50.0);
        assert_eq!(p.length_req, 500.0);
        assert_eq!(p.consonant, 100.0);
        assert_eq!(p.cutoff, -200.0);
        assert_eq!(p.tempo, 120.0);
        assert_eq!(p.pitch_bends.len(), 5);
    }

    #[test]
    fn tempo_parsing() {
        assert_eq!(parse_tempo("!120"), 120.0);
        assert_eq!(parse_tempo("90"), 90.0);
        assert_eq!(parse_tempo("bad"), 120.0);
    }
}
