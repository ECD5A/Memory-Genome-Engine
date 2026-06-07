use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

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
        if !self.path.exists() {
            fs::write(&self.path, b"")?;
        }
        Ok(())
    }

    pub fn append_cell(&self, cell: &MemoryCell) -> Result<()> {
        self.ensure_exists()?;
        let record = rmp_serde::to_vec_named(cell)?;
        let record_len = u32::try_from(record.len()).map_err(|_| {
            MgeError::InvalidInput("hot memory record is larger than 4 GiB".to_string())
        })?;
        let mut file = OpenOptions::new()
            .append(true)
            .create(true)
            .open(&self.path)?;
        file.write_all(&record_len.to_le_bytes())?;
        file.write_all(&record)?;
        Ok(())
    }

    pub fn load_cells(&self) -> Result<Vec<MemoryCell>> {
        if !self.path.exists() {
            return Ok(Vec::new());
        }

        let content = fs::read(&self.path)?;
        let mut cells = Vec::new();
        let mut offset = 0usize;
        while offset < content.len() {
            if content.len() - offset < 4 {
                return Err(MgeError::InvalidInput(format!(
                    "truncated hot memory log at byte {offset}"
                )));
            }
            let mut len_bytes = [0u8; 4];
            len_bytes.copy_from_slice(&content[offset..offset + 4]);
            offset += 4;

            let record_len = u32::from_le_bytes(len_bytes) as usize;
            if content.len() - offset < record_len {
                return Err(MgeError::InvalidInput(format!(
                    "truncated hot memory record at byte {offset}"
                )));
            }
            cells.push(rmp_serde::from_slice(
                &content[offset..offset + record_len],
            )?);
            offset += record_len;
        }
        Ok(cells)
    }

    pub fn archive_and_clear(&self) -> Result<Option<PathBuf>> {
        self.ensure_exists()?;
        if fs::metadata(&self.path)?.len() == 0 {
            fs::write(&self.path, b"")?;
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
        fs::write(&self.path, b"")?;
        Ok(Some(archive_path))
    }
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
