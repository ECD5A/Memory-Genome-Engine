use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::errors::Result;
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
        let mut file = OpenOptions::new()
            .append(true)
            .create(true)
            .open(&self.path)?;
        writeln!(file, "{}", serde_json::to_string(cell)?)?;
        Ok(())
    }

    pub fn load_cells(&self) -> Result<Vec<MemoryCell>> {
        if !self.path.exists() {
            return Ok(Vec::new());
        }

        let content = fs::read_to_string(&self.path)?;
        let mut cells = Vec::new();
        for line in content.lines() {
            if line.trim().is_empty() {
                continue;
            }
            cells.push(serde_json::from_str(line)?);
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

        let archive_path = archive_dir.join(format!("hot_cells_{}.jsonl", current_timestamp()));
        fs::rename(&self.path, &archive_path)?;
        fs::write(&self.path, b"")?;
        Ok(Some(archive_path))
    }
}
