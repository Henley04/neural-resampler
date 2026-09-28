//! F0 / Mel 特征缓存（zstd 压缩）
//!
//! 参考 Organum 的 `.ogc` 思路：预先分析声库并落盘，重复渲染时直接复用，
//! 缓存键包含文件路径、大小、修改时间与配置指纹，任一变化即自动失效。

use crate::core::f0::F0Track;
use anyhow::{Context as _, Result};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// 缓存文件魔数（"NRCACHE1"）。
pub const CACHE_MAGIC: &[u8; 8] = b"NRCACHE1";

/// 缓存键：源音频 + 配置指纹。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CacheKey {
    pub source: PathBuf,
    pub size: u64,
    pub mtime: u64,
    pub config_digest: String,
}

impl CacheKey {
    /// 从源音频路径与配置指纹构造（读取 stat 信息）。
    pub fn from_source(source: &Path, config_digest: &str) -> Result<Self> {
        let meta =
            fs::metadata(source).with_context(|| format!("读取文件元信息失败: {source:?}"))?;
        let mtime = meta
            .modified()
            .unwrap_or(SystemTime::UNIX_EPOCH)
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        Ok(Self {
            source: source.to_path_buf(),
            size: meta.len(),
            mtime,
            config_digest: config_digest.to_string(),
        })
    }

    fn to_bytes(&self) -> Vec<u8> {
        serde_json::json!({
            "source": self.source,
            "size": self.size,
            "mtime": self.mtime,
            "config": self.config_digest,
        })
        .to_string()
        .into_bytes()
    }
}

/// F0 缓存读写器。
#[derive(Debug, Clone)]
pub struct F0Cache {
    /// 缓存目录（None 表示与源音频同目录）。
    dir: Option<PathBuf>,
    /// 缓存文件后缀（不含点）。
    extension: String,
    /// zstd 压缩等级。
    level: i32,
}

/// 一条缓存记录。
#[derive(Debug, Clone)]
pub struct CacheEntry {
    pub f0: Vec<f32>,
    pub frame_period_ms: f32,
}

impl F0Cache {
    pub fn new(dir: Option<PathBuf>, extension: &str, level: i32) -> Self {
        Self {
            dir,
            extension: extension.trim_start_matches('.').to_string(),
            level,
        }
    }

    /// 缓存文件路径。
    pub fn path_for(&self, source: &Path) -> PathBuf {
        let stem = source
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "audio".to_string());
        let name = format!("{stem}.{}", self.extension);
        match &self.dir {
            Some(d) => d.join(name),
            None => source.with_file_name(name),
        }
    }

    /// 读取缓存；不存在或与键不匹配时返回 None。
    pub fn load(&self, source: &Path, config_digest: &str) -> Option<CacheEntry> {
        let path = self.path_for(source);
        if !path.exists() {
            return None;
        }
        let key = CacheKey::from_source(source, config_digest).ok()?;
        match self.read_entry(&path, &key) {
            Ok(entry) => {
                log::debug!("命中 F0 缓存: {path:?}");
                Some(entry)
            }
            Err(err) => {
                log::debug!("F0 缓存不可用（{err}），将重新分析");
                None
            }
        }
    }

    /// 写入缓存（失败只告警，不影响渲染）。
    pub fn store(&self, source: &Path, config_digest: &str, track: &F0Track) -> Result<PathBuf> {
        let path = self.path_for(source);
        let key = CacheKey::from_source(source, config_digest)?;
        self.write_entry(&path, &key, track)?;
        log::debug!("已写入 F0 缓存: {path:?}");
        Ok(path)
    }

    fn write_entry(&self, path: &Path, key: &CacheKey, track: &F0Track) -> Result<()> {
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent)?;
            }
        }
        let header = key.to_bytes();
        let mut payload: Vec<u8> = Vec::new();
        payload.extend_from_slice(&(header.len() as u32).to_le_bytes());
        payload.extend_from_slice(&header);
        payload.extend_from_slice(&(track.frame_period_ms).to_le_bytes());
        payload.extend_from_slice(&(track.f0.len() as u32).to_le_bytes());
        for v in &track.f0 {
            payload.extend_from_slice(&v.to_le_bytes());
        }
        let compressed = zstd::encode_all(&payload[..], self.level).context("zstd 压缩失败")?;

        let mut file = fs::File::create(path)?;
        file.write_all(CACHE_MAGIC)?;
        file.write_all(&compressed)?;
        Ok(())
    }

    fn read_entry(&self, path: &Path, key: &CacheKey) -> Result<CacheEntry> {
        let raw = fs::read(path).with_context(|| format!("读取缓存失败: {path:?}"))?;
        if raw.len() < CACHE_MAGIC.len() {
            anyhow::bail!("缓存文件过短");
        }
        if &raw[..CACHE_MAGIC.len()] != CACHE_MAGIC {
            anyhow::bail!("缓存魔数不匹配");
        }
        let payload = zstd::decode_all(&raw[CACHE_MAGIC.len()..]).context("zstd 解压失败")?;
        let mut pos = 0usize;
        let header_len = read_u32(&payload, &mut pos)? as usize;
        let header = &payload[pos..pos + header_len];
        pos += header_len;
        let stored: serde_json::Value = serde_json::from_slice(header).context("解析缓存头失败")?;
        let expected: serde_json::Value =
            serde_json::from_slice(&key.to_bytes()).context("序列化缓存键失败")?;
        if stored != expected {
            anyhow::bail!("缓存键不匹配");
        }
        let frame_period_ms = read_f32(&payload, &mut pos)?;
        let len = read_u32(&payload, &mut pos)? as usize;
        let mut f0 = Vec::with_capacity(len);
        for _ in 0..len {
            f0.push(read_f32(&payload, &mut pos)?);
        }
        Ok(CacheEntry {
            f0,
            frame_period_ms,
        })
    }
}

fn read_u32(buf: &[u8], pos: &mut usize) -> Result<u32> {
    let bytes: [u8; 4] = buf
        .get(*pos..*pos + 4)
        .and_then(|s| s.try_into().ok())
        .context("缓存数据不完整")?;
    *pos += 4;
    Ok(u32::from_le_bytes(bytes))
}

fn read_f32(buf: &[u8], pos: &mut usize) -> Result<f32> {
    let bytes: [u8; 4] = buf
        .get(*pos..*pos + 4)
        .and_then(|s| s.try_into().ok())
        .context("缓存数据不完整")?;
    *pos += 4;
    Ok(f32::from_le_bytes(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("nr-cache-{name}"));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn store_and_load_roundtrip() {
        let dir = tmp_dir("roundtrip");
        let src = dir.join("voice.wav");
        fs::write(&src, b"dummy").unwrap();

        let cache = F0Cache::new(Some(dir.clone()), "nrc", 3);
        let track = F0Track::new(vec![220.0, 0.0, 330.5], 10.0);
        cache.store(&src, "digest-1", &track).unwrap();

        let loaded = cache.load(&src, "digest-1").expect("应命中缓存");
        assert_eq!(loaded.f0, track.f0);
        assert_eq!(loaded.frame_period_ms, 10.0);
    }

    #[test]
    fn invalidates_on_config_change() {
        let dir = tmp_dir("digest");
        let src = dir.join("voice.wav");
        fs::write(&src, b"dummy").unwrap();
        let cache = F0Cache::new(Some(dir.clone()), "nrc", 3);
        cache
            .store(&src, "digest-1", &F0Track::new(vec![1.0], 10.0))
            .unwrap();
        assert!(cache.load(&src, "digest-2").is_none());
    }

    #[test]
    fn invalidates_on_source_change() {
        let dir = tmp_dir("mtime");
        let src = dir.join("voice.wav");
        fs::write(&src, b"dummy").unwrap();
        let cache = F0Cache::new(Some(dir.clone()), "nrc", 3);
        cache
            .store(&src, "digest-1", &F0Track::new(vec![1.0], 10.0))
            .unwrap();
        fs::write(&src, b"dummy-changed").unwrap();
        assert!(cache.load(&src, "digest-1").is_none());
    }

    #[test]
    fn cache_path_uses_extension() {
        let cache = F0Cache::new(None, ".nrc", 3);
        let p = cache.path_for(Path::new("/vb/_あ.wav"));
        assert_eq!(p.file_name().unwrap().to_string_lossy(), "_あ.nrc");
    }
}
