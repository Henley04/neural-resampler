//! oto.ini 解析
//!
//! UTAU 声库的 `oto.ini` 每行格式为：
//!
//! ```text
//! 文件名.wav=别名,左ブランク(offset),子音部(consonant),ブランク(cutoff),先行発声(preutterance),オーバーラップ(overlap)
//! ```
//!
//! 文件编码可能是 UTF-8（带/不带 BOM）或 Shift-JIS（日本声库最常见），此处按
//! `UTF-8 → Shift-JIS` 的顺序探测。

use anyhow::{Context as _, Result};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// 单条 oto 配置（时间单位：毫秒）。
#[derive(Debug, Clone, PartialEq)]
pub struct OtoEntry {
    /// WAV 文件名（如 `_あ.wav`）。
    pub file: String,
    /// 别名（如 `あ`）。
    pub alias: String,
    /// 左空白：从音频起点到发声起点。
    pub offset: f64,
    /// 子音部长度：固定不拉伸的部分。
    pub consonant: f64,
    /// 右侧空白（通常为负）：从音频末尾向内收缩的量。
    pub cutoff: f64,
    /// 先行发声：相对发声起点的前置量。
    pub preutterance: f64,
    /// 重叠：与前一音符交叠的长度。
    pub overlap: f64,
}

impl OtoEntry {
    /// 默认值（无 oto.ini 时使用）。
    pub fn default_for(file: &str, alias: &str) -> Self {
        Self {
            file: file.to_string(),
            alias: alias.to_string(),
            offset: 0.0,
            consonant: 0.0,
            cutoff: 0.0,
            preutterance: 0.0,
            overlap: 0.0,
        }
    }
}

/// oto.ini 内容。
#[derive(Debug, Default)]
pub struct Oto {
    /// key: (文件名, 别名)
    entries: HashMap<(String, String), OtoEntry>,
    /// key: 别名 → 第一个匹配项（多数声库别名唯一）。
    by_alias: HashMap<String, OtoEntry>,
}

impl Oto {
    pub fn new() -> Self {
        Self::default()
    }

    /// 解析 oto.ini 的字节内容（自动探测编码）。
    pub fn parse_bytes(bytes: &[u8]) -> Self {
        let text = decode_text(bytes);
        Self::parse_str(&text)
    }

    /// 解析 oto.ini 的文本内容。
    pub fn parse_str(text: &str) -> Self {
        let mut oto = Oto::new();
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') || line.starts_with('[') {
                continue;
            }
            if let Some(entry) = parse_line(line) {
                oto.by_alias
                    .entry(entry.alias.clone())
                    .or_insert_with(|| entry.clone());
                oto.entries
                    .insert((entry.file.clone(), entry.alias.clone()), entry);
            }
        }
        oto
    }

    /// 从文件载入；不存在时返回空表。
    pub fn load(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Oto::new());
        }
        let bytes = std::fs::read(path).with_context(|| format!("读取 oto.ini 失败: {path:?}"))?;
        Ok(Self::parse_bytes(&bytes))
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// 按 (文件名, 别名) 精确查找。
    pub fn get(&self, file: &str, alias: &str) -> Option<&OtoEntry> {
        self.entries.get(&(file.to_string(), alias.to_string()))
    }

    /// 按别名查找。
    pub fn get_by_alias(&self, alias: &str) -> Option<&OtoEntry> {
        self.by_alias.get(alias)
    }

    /// 按 WAV 路径查找：`oto.ini` 中的文件名与给定的 WAV 路径匹配。
    pub fn get_for_wav(&self, wav: &Path, alias: Option<&str>) -> Option<&OtoEntry> {
        let file_name = wav.file_name()?.to_string_lossy().to_string();
        if let Some(a) = alias {
            if let Some(e) = self.get(&file_name, a) {
                return Some(e);
            }
        }
        // 文件名即别名的情况（无 alias 或精确匹配失败）
        self.entries
            .iter()
            .find(|((file, _), _)| *file == file_name)
            .map(|(_, e)| e)
            .or_else(|| alias.and_then(|a| self.get_by_alias(a)))
    }

    /// 遍历全部条目。
    pub fn iter(&self) -> impl Iterator<Item = &OtoEntry> {
        self.entries.values()
    }
}

fn parse_line(line: &str) -> Option<OtoEntry> {
    let (file, rest) = line.split_once('=')?;
    let file = file.trim().to_string();
    let mut fields = rest.split(',').map(|s| s.trim());

    let alias = fields.next()?.to_string();
    let nums: Vec<f64> = fields.map(|s| s.parse::<f64>().unwrap_or(0.0)).collect();

    let get = |i: usize| nums.get(i).copied().unwrap_or(0.0);

    Some(OtoEntry {
        file,
        alias,
        offset: get(0),
        consonant: get(1),
        cutoff: get(2),
        preutterance: get(3),
        overlap: get(4),
    })
}

/// 探测文本编码：UTF-8（含 BOM）→ Shift-JIS。
fn decode_text(bytes: &[u8]) -> String {
    if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
        return String::from_utf8_lossy(&bytes[3..]).into_owned();
    }
    if let Ok(s) = std::str::from_utf8(bytes) {
        return s.to_string();
    }
    let (cow, _encoding, _had_errors) = encoding_rs::SHIFT_JIS.decode(bytes);
    cow.into_owned()
}

/// 在给定目录及其父目录中寻找 oto.ini。
pub fn find_oto_ini(start: &Path) -> Option<PathBuf> {
    let mut dir = if start.is_file() {
        start.parent()?.to_path_buf()
    } else {
        start.to_path_buf()
    };
    loop {
        let candidate = dir.join("oto.ini");
        if candidate.exists() {
            return Some(candidate);
        }
        if !dir.pop() {
            return None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_utf8_oto() {
        let text = "_あ.wav=あ,100.0,50.0,-200.0,150.0,30.0\n_い.wav=い,80,40,-150,120,20\n";
        let oto = Oto::parse_str(text);
        assert_eq!(oto.len(), 2);
        let e = oto.get("_あ.wav", "あ").unwrap();
        assert_eq!(e.offset, 100.0);
        assert_eq!(e.consonant, 50.0);
        assert_eq!(e.cutoff, -200.0);
        assert_eq!(e.preutterance, 150.0);
        assert_eq!(e.overlap, 30.0);
    }

    #[test]
    fn parse_shift_jis() {
        // "あ" 的 Shift-JIS 编码为 0x82 0xA0
        let mut bytes: Vec<u8> = vec![0x5F, 0x82, 0xA0, 0x2E, 0x77, 0x61, 0x76, 0x3D]; // "_あ.wav="
        bytes.extend_from_slice(&[0x82, 0xA0]); // あ
        bytes.extend_from_slice(b",10,5,-20,15,3");
        let oto = Oto::parse_bytes(&bytes);
        assert_eq!(oto.len(), 1);
        let e = oto.get_by_alias("あ").unwrap();
        assert_eq!(e.offset, 10.0);
        assert_eq!(e.cutoff, -20.0);
    }

    #[test]
    fn missing_fields_are_zero() {
        let oto = Oto::parse_str("a.wav=a\n");
        let e = oto.get_by_alias("a").unwrap();
        assert_eq!((e.offset, e.consonant, e.cutoff), (0.0, 0.0, 0.0));
    }

    #[test]
    fn lookup_by_wav_path() {
        let oto = Oto::parse_str("voice.wav=a,10,5,-20,15,3\n");
        let e = oto
            .get_for_wav(Path::new("/some/dir/voice.wav"), Some("a"))
            .unwrap();
        assert_eq!(e.offset, 10.0);
    }
}
