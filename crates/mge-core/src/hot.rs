use std::fs;
use std::path::{Path, PathBuf};

use crate::binary::{self, CodecId, FileKind};
use crate::errors::{MgeError, Result};
use crate::models::{current_timestamp, MemoryCell};

#[derive(Clone, Debug)]
pub struct HotStore {
    path: PathBuf,
}

impl HotStore {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    pub fn ensure_exists(&self) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }
        if !self.path.exists() || fs::metadata(&self.path)?.len() == 0 {
            binary::atomic_write_bytes(&self.path, &empty_hot_log_bytes()?)?;
        }
        Ok(())
    }

    pub fn append_cell(&self, cell: &MemoryCell) -> Result<()> {
        self.ensure_exists()?;
        let record = rmp_serde::to_vec_named(cell)?;
        let record = binary::encode_frame(FileKind::HotRecord, CodecId::MessagePack, &record)?;
        let mut bytes = fs::read(&self.path)?;
        binary::decode_frame_at(&bytes, 0, FileKind::HotLog)?;
        bytes.extend_from_slice(&record);
        binary::atomic_write_bytes(&self.path, &bytes)?;
        Ok(())
    }

    pub fn load_cells(&self) -> Result<Vec<MemoryCell>> {
        if !self.path.exists() {
            return Ok(Vec::new());
        }

        let content = fs::read(&self.path)?;
        let mut cells = Vec::new();
        if content.is_empty() {
            return Ok(cells);
        }
        let (_, mut offset) = binary::decode_frame_at(&content, 0, FileKind::HotLog)?;
        while offset < content.len() {
            let (record, next_offset) =
                binary::decode_frame_at(&content, offset, FileKind::HotRecord)?;
            if record.codec != CodecId::MessagePack {
                return Err(MgeError::StorageFormat(format!(
                    "wrong codec for hot record: expected {}, found {}",
                    CodecId::MessagePack.as_str(),
                    record.codec.as_str()
                )));
            }
            cells.push(rmp_serde::from_slice(&record.payload)?);
            offset = next_offset;
        }
        Ok(cells)
    }

    pub fn archive_and_clear(&self) -> Result<Option<PathBuf>> {
        self.ensure_exists()?;
        let bytes = fs::read(&self.path)?;
        let (_, offset) = binary::decode_frame_at(&bytes, 0, FileKind::HotLog)?;
        if bytes.len() == offset {
            binary::atomic_write_bytes(&self.path, &empty_hot_log_bytes()?)?;
            return Ok(None);
        }

        let archive_dir = self
            .path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join("archive");
        fs::create_dir_all(&archive_dir)?;

        let archive_path = unique_archive_path(&archive_dir, current_timestamp());
        fs::rename(&self.path, &archive_path)?;
        binary::atomic_write_bytes(&self.path, &empty_hot_log_bytes()?)?;
        Ok(Some(archive_path))
    }
}

fn empty_hot_log_bytes() -> Result<Vec<u8>> {
    binary::encode_frame(FileKind::HotLog, CodecId::None, &[])
}

fn unique_archive_path(archive_dir: &Path, timestamp: i64) -> PathBuf {
    let first = archive_dir.join(format!("hot_{timestamp}.mgl"));
    if !first.exists() {
        return first;
    }

    for suffix in 1.. {
        let candidate = archive_dir.join(format!("hot_{timestamp}_{suffix}.mgl"));
        if !candidate.exists() {
            return candidate;
        }
    }

    unreachable!("unbounded archive suffix loop must return")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn archive_path_uses_suffix_when_timestamp_name_exists() {
        let dir = tempfile::tempdir().unwrap();
        let timestamp = 123_456;

        let first = unique_archive_path(dir.path(), timestamp);
        assert_eq!(
            first.file_name().and_then(|name| name.to_str()),
            Some("hot_123456.mgl")
        );
        fs::write(&first, b"first").unwrap();

        let second = unique_archive_path(dir.path(), timestamp);
        assert_eq!(
            second.file_name().and_then(|name| name.to_str()),
            Some("hot_123456_1.mgl")
        );
        fs::write(&second, b"second").unwrap();

        let third = unique_archive_path(dir.path(), timestamp);
        assert_eq!(
            third.file_name().and_then(|name| name.to_str()),
            Some("hot_123456_2.mgl")
        );
    }
}
