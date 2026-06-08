use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use crate::binary::{self, CodecId, FileKind};
use crate::errors::{MgeError, Result};
use crate::indexes::QueryMode;
use crate::markers::canonicalize_marker_value;
use crate::models::{current_timestamp, CellId, MarkerId, MemoryCell, MemoryKind, MemoryStatus};
use crate::security::RecallPolicy;

pub type ScopeId = String;
pub type KindId = MemoryKind;

#[derive(Clone, Debug, Default)]
pub struct HotMemoryLayer {
    pub cells_by_id: BTreeMap<CellId, MemoryCell>,
    pub marker_to_cells: BTreeMap<MarkerId, Vec<CellId>>,
    pub scope_to_cells: BTreeMap<ScopeId, Vec<CellId>>,
    pub kind_to_cells: BTreeMap<KindId, Vec<CellId>>,
    pub status_to_cells: BTreeMap<MemoryStatus, Vec<CellId>>,
}

#[derive(Clone, Copy, Debug)]
pub struct HotCandidateQuery<'a> {
    pub marker_ids: &'a [MarkerId],
    pub marker_mode: QueryMode,
    pub scope: Option<&'a str>,
    pub kind: Option<MemoryKind>,
    pub allowed_statuses: &'a [MemoryStatus],
}

impl HotMemoryLayer {
    pub fn from_cells(cells: impl IntoIterator<Item = MemoryCell>) -> Self {
        let mut layer = Self::default();
        for cell in cells {
            layer.insert(cell);
        }
        layer
    }

    pub fn insert(&mut self, cell: MemoryCell) {
        if let Some(previous) = self.cells_by_id.insert(cell.id, cell.clone()) {
            self.remove_from_indexes(&previous);
        }
        self.add_to_indexes(&cell);
    }

    pub fn clear(&mut self) {
        self.cells_by_id.clear();
        self.marker_to_cells.clear();
        self.scope_to_cells.clear();
        self.kind_to_cells.clear();
        self.status_to_cells.clear();
    }

    pub fn len(&self) -> usize {
        self.cells_by_id.len()
    }

    pub fn is_empty(&self) -> bool {
        self.cells_by_id.is_empty()
    }

    pub fn cell(&self, cell_id: CellId) -> Option<&MemoryCell> {
        self.cells_by_id.get(&cell_id)
    }

    pub fn all_cells(&self) -> Vec<MemoryCell> {
        self.cells_by_id.values().cloned().collect()
    }

    pub fn candidate_ids(&self, query: HotCandidateQuery<'_>) -> Vec<CellId> {
        let mut candidates = self.marker_candidates(query.marker_ids, query.marker_mode);

        if let Some(scope) = query.scope {
            let scope = canonicalize_marker_value(scope);
            candidates = intersect_candidate_ids(candidates, self.scope_to_cells.get(&scope));
        }

        if let Some(kind) = query.kind {
            candidates = intersect_candidate_ids(candidates, self.kind_to_cells.get(&kind));
        }

        candidates =
            intersect_with_statuses(candidates, &self.status_to_cells, query.allowed_statuses);

        candidates
            .unwrap_or_else(|| self.cells_by_id.keys().copied().collect())
            .into_iter()
            .collect()
    }

    fn add_to_indexes(&mut self, cell: &MemoryCell) {
        for marker in cell.markers.iter().copied().collect::<BTreeSet<_>>() {
            push_unique(self.marker_to_cells.entry(marker).or_default(), cell.id);
        }
        let scope = canonicalize_marker_value(&cell.scope);
        push_unique(self.scope_to_cells.entry(scope).or_default(), cell.id);
        push_unique(self.kind_to_cells.entry(cell.kind).or_default(), cell.id);
        push_unique(
            self.status_to_cells.entry(cell.status).or_default(),
            cell.id,
        );
    }

    fn remove_from_indexes(&mut self, cell: &MemoryCell) {
        for marker in cell.markers.iter().copied().collect::<BTreeSet<_>>() {
            remove_cell_id_from_index(&mut self.marker_to_cells, marker, cell.id);
        }
        remove_cell_id_from_index(
            &mut self.scope_to_cells,
            canonicalize_marker_value(&cell.scope),
            cell.id,
        );
        remove_cell_id_from_index(&mut self.kind_to_cells, cell.kind, cell.id);
        remove_cell_id_from_index(&mut self.status_to_cells, cell.status, cell.id);
    }

    fn marker_candidates(
        &self,
        marker_ids: &[MarkerId],
        marker_mode: QueryMode,
    ) -> Option<BTreeSet<CellId>> {
        if marker_ids.is_empty() {
            return None;
        }

        match marker_mode {
            QueryMode::Intersection => {
                Some(intersection_for_markers(&self.marker_to_cells, marker_ids))
            }
            QueryMode::Union => Some(union_for_markers(&self.marker_to_cells, marker_ids)),
            QueryMode::PreferIntersection => {
                let intersection = intersection_for_markers(&self.marker_to_cells, marker_ids);
                if intersection.is_empty() {
                    Some(union_for_markers(&self.marker_to_cells, marker_ids))
                } else {
                    Some(intersection)
                }
            }
        }
    }
}

pub fn allowed_statuses_for_policy(policy: &RecallPolicy) -> Vec<MemoryStatus> {
    let mut statuses = vec![
        MemoryStatus::Active,
        MemoryStatus::Temporary,
        MemoryStatus::Unverified,
        MemoryStatus::Verified,
    ];
    if policy.include_deprecated {
        statuses.push(MemoryStatus::Deprecated);
        statuses.push(MemoryStatus::Superseded);
    }
    if policy.include_rejected {
        statuses.push(MemoryStatus::Rejected);
    }
    statuses
}

fn intersection_for_markers(
    marker_to_cells: &BTreeMap<MarkerId, Vec<CellId>>,
    marker_ids: &[MarkerId],
) -> BTreeSet<CellId> {
    let mut iter = marker_ids.iter();
    let Some(first_marker) = iter.next() else {
        return BTreeSet::new();
    };
    let Some(first_cells) = marker_to_cells.get(first_marker) else {
        return BTreeSet::new();
    };

    let mut result = first_cells.iter().copied().collect::<BTreeSet<_>>();
    for marker in iter {
        let Some(cells) = marker_to_cells.get(marker) else {
            return BTreeSet::new();
        };
        let cells = cells.iter().copied().collect::<BTreeSet<_>>();
        result = result.intersection(&cells).copied().collect();
        if result.is_empty() {
            break;
        }
    }
    result
}

fn union_for_markers(
    marker_to_cells: &BTreeMap<MarkerId, Vec<CellId>>,
    marker_ids: &[MarkerId],
) -> BTreeSet<CellId> {
    let mut result = BTreeSet::new();
    for marker in marker_ids {
        if let Some(cells) = marker_to_cells.get(marker) {
            result.extend(cells.iter().copied());
        }
    }
    result
}

fn intersect_with_statuses(
    candidates: Option<BTreeSet<CellId>>,
    status_to_cells: &BTreeMap<MemoryStatus, Vec<CellId>>,
    statuses: &[MemoryStatus],
) -> Option<BTreeSet<CellId>> {
    if statuses.is_empty() {
        return Some(BTreeSet::new());
    }

    let mut allowed = BTreeSet::new();
    for status in statuses {
        if let Some(cells) = status_to_cells.get(status) {
            allowed.extend(cells.iter().copied());
        }
    }
    Some(match candidates {
        Some(candidates) => candidates.intersection(&allowed).copied().collect(),
        None => allowed,
    })
}

fn intersect_candidate_ids(
    candidates: Option<BTreeSet<CellId>>,
    indexed_ids: Option<&Vec<CellId>>,
) -> Option<BTreeSet<CellId>> {
    let indexed_ids = indexed_ids
        .map(|ids| ids.iter().copied().collect::<BTreeSet<_>>())
        .unwrap_or_default();
    Some(match candidates {
        Some(candidates) => candidates.intersection(&indexed_ids).copied().collect(),
        None => indexed_ids,
    })
}

fn push_unique(ids: &mut Vec<CellId>, cell_id: CellId) {
    if !ids.contains(&cell_id) {
        ids.push(cell_id);
    }
}

fn remove_cell_id_from_index<K: Ord + Clone>(
    index: &mut BTreeMap<K, Vec<CellId>>,
    key: K,
    cell_id: CellId,
) {
    if let Some(ids) = index.get_mut(&key) {
        ids.retain(|id| *id != cell_id);
        if ids.is_empty() {
            index.remove(&key);
        }
    }
}

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
