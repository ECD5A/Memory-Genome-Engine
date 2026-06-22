// Memory Genome Engine
// Copyright (c) 2026 ECD5A
// Project: https://github.com/ECD5A/Memory-Genome-Engine
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at:
//
//     https://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.

use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::binary::{self, CodecId, FileKind};
use crate::errors::{MgeError, Result};
use crate::indexes::QueryMode;
use crate::markers::canonicalize_marker_value;
use crate::models::{current_timestamp, CellId, MarkerId, MemoryCell, MemoryKind, MemoryStatus};
use crate::retrieval::CachedCellScoringData;
use crate::security::{
    decrypt_payload, encrypt_payload, EncryptedPayload, RecallPolicy, SessionKey,
};

pub type ScopeId = String;
pub type KindId = MemoryKind;
const HOT_SNAPSHOT_FILE: &str = "snapshot.mgs";
const HOT_SNAPSHOT_VERSION: u32 = 1;
const HOT_RECORD_AAD: &[u8] = b"mge:hot_record:v1";
const HOT_SNAPSHOT_AAD: &[u8] = b"mge:hot_snapshot:v1";

#[derive(Clone, Debug, Default)]
pub struct HotMemoryLayer {
    pub cells_by_id: BTreeMap<CellId, MemoryCell>,
    scoring_by_id: BTreeMap<CellId, CachedCellScoringData>,
    token_to_cells: BTreeMap<String, Vec<CellId>>,
    token_count_by_cell: BTreeMap<CellId, usize>,
    pub marker_to_cells: BTreeMap<MarkerId, Vec<CellId>>,
    pub scope_to_cells: BTreeMap<ScopeId, Vec<CellId>>,
    pub kind_to_cells: BTreeMap<KindId, Vec<CellId>>,
    pub status_to_cells: BTreeMap<MemoryStatus, Vec<CellId>>,
}

#[derive(Clone, Copy, Debug)]
pub struct HotCandidateQuery<'a> {
    pub marker_ids: &'a [MarkerId],
    pub lexical_candidate_ids: &'a [CellId],
    pub marker_mode: QueryMode,
    pub scope: Option<&'a str>,
    pub kind: Option<MemoryKind>,
    pub allowed_statuses: &'a [MemoryStatus],
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct HotSnapshot {
    pub version: u32,
    pub created_at: i64,
    pub hot_log_offset: u64,
    pub cells: Vec<MemoryCell>,
}

#[derive(Clone, Debug, Default)]
pub struct HotRecovery {
    pub cells: Vec<MemoryCell>,
    pub valid_log_offset: usize,
    pub recovered_bad_tail: bool,
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
        let scoring = CachedCellScoringData::from_cell(&cell);
        if let Some(previous) = self.cells_by_id.insert(cell.id, cell.clone()) {
            if let Some(previous_scoring) = self.scoring_by_id.remove(&previous.id) {
                self.remove_from_text_index(previous.id, &previous_scoring);
            }
            self.remove_from_indexes(&previous);
        }
        self.add_to_text_index(cell.id, &scoring);
        self.scoring_by_id.insert(cell.id, scoring);
        self.add_to_indexes(&cell);
    }

    pub fn clear(&mut self) {
        self.cells_by_id.clear();
        self.scoring_by_id.clear();
        self.token_to_cells.clear();
        self.token_count_by_cell.clear();
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

    pub(crate) fn scoring(&self, cell_id: CellId) -> Option<&CachedCellScoringData> {
        self.scoring_by_id.get(&cell_id)
    }

    pub(crate) fn lexical_total_token_count(&self) -> usize {
        self.scoring_by_id
            .values()
            .map(CachedCellScoringData::document_len)
            .sum()
    }

    pub(crate) fn lexical_document_frequency(&self, token: &str) -> usize {
        self.token_to_cells.get(token).map_or(0, Vec::len)
    }

    pub fn all_cells(&self) -> Vec<MemoryCell> {
        self.cells_by_id.values().cloned().collect()
    }

    pub fn candidate_ids(&self, query: HotCandidateQuery<'_>) -> Vec<CellId> {
        let mut candidates = self.marker_candidates(query.marker_ids, query.marker_mode);
        let lexical_candidates = if query.lexical_candidate_ids.is_empty() {
            None
        } else {
            Some(query.lexical_candidate_ids.iter().copied().collect())
        };
        candidates = combine_candidate_ids(candidates, lexical_candidates, query.marker_mode);

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

    pub(crate) fn lexical_scores(
        &self,
        query_tokens: &[String],
        scope: Option<&str>,
        kind: Option<MemoryKind>,
        allowed_statuses: &[MemoryStatus],
    ) -> BTreeMap<CellId, i64> {
        if query_tokens.is_empty() || self.cells_by_id.is_empty() {
            return BTreeMap::new();
        }

        let mut allowed = None;
        if let Some(scope) = scope {
            let scope = canonicalize_marker_value(scope);
            allowed = intersect_candidate_ids(allowed, self.scope_to_cells.get(&scope));
        }
        if let Some(kind) = kind {
            allowed = intersect_candidate_ids(allowed, self.kind_to_cells.get(&kind));
        }
        allowed = intersect_with_statuses(allowed, &self.status_to_cells, allowed_statuses);
        let allowed = allowed.unwrap_or_default();
        if allowed.is_empty() {
            return BTreeMap::new();
        }

        let document_count = allowed.len() as f64;
        let allowed_token_count = allowed
            .iter()
            .map(|cell_id| self.token_count_by_cell.get(cell_id).copied().unwrap_or(0))
            .sum::<usize>();
        let average_document_length = allowed_token_count as f64 / allowed.len().max(1) as f64;
        let mut raw_scores = BTreeMap::<CellId, f64>::new();
        let mut matched_terms = BTreeMap::<CellId, usize>::new();

        let query_terms = query_tokens
            .iter()
            .map(String::as_str)
            .collect::<BTreeSet<_>>();
        for token in &query_terms {
            let Some(cell_ids) = self.token_to_cells.get(*token) else {
                continue;
            };
            let document_frequency = cell_ids
                .iter()
                .filter(|cell_id| allowed.contains(cell_id))
                .count();
            if document_frequency == 0 {
                continue;
            }
            let document_frequency = document_frequency as f64;
            let idf = ((document_count - document_frequency + 0.5) / (document_frequency + 0.5)
                + 1.0)
                .ln();
            for cell_id in cell_ids.iter().copied() {
                if !allowed.contains(&cell_id) {
                    continue;
                }
                let document_length = self
                    .token_count_by_cell
                    .get(&cell_id)
                    .copied()
                    .unwrap_or(1)
                    .max(1) as f64;
                let k1 = 1.2;
                let b = 0.75;
                let normalization =
                    1.0 + k1 * (1.0 - b + b * document_length / average_document_length.max(1.0));
                *raw_scores.entry(cell_id).or_insert(0.0) += idf * (k1 + 1.0) / normalization;
                *matched_terms.entry(cell_id).or_insert(0) += 1;
            }
        }

        raw_scores
            .into_iter()
            .map(|(cell_id, score)| {
                let mut score = (score * 8.0).round() as i64;
                if matched_terms.get(&cell_id).copied().unwrap_or(0) == query_terms.len() {
                    score += 4;
                }
                (cell_id, score)
            })
            .collect()
    }

    fn add_to_text_index(&mut self, cell_id: CellId, scoring: &CachedCellScoringData) {
        let tokens = scoring_tokens(scoring);
        self.token_count_by_cell.insert(cell_id, tokens.len());
        for token in tokens {
            push_unique(self.token_to_cells.entry(token).or_default(), cell_id);
        }
    }

    fn remove_from_text_index(&mut self, cell_id: CellId, scoring: &CachedCellScoringData) {
        let tokens = scoring_tokens(scoring);
        self.token_count_by_cell.remove(&cell_id);
        for token in tokens {
            remove_cell_id_from_index(&mut self.token_to_cells, token, cell_id);
        }
    }

    fn add_to_indexes(&mut self, cell: &MemoryCell) {
        let mut markers = BTreeSet::new();
        cell.for_each_marker_id_for_indexing(|marker_id| {
            markers.insert(marker_id);
        });
        for marker in markers {
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
        let mut markers = BTreeSet::new();
        cell.for_each_marker_id_for_indexing(|marker_id| {
            markers.insert(marker_id);
        });
        for marker in markers {
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

fn combine_candidate_ids(
    candidates: Option<BTreeSet<CellId>>,
    additional: Option<BTreeSet<CellId>>,
    mode: QueryMode,
) -> Option<BTreeSet<CellId>> {
    match (candidates, additional) {
        (Some(candidates), Some(additional)) => match mode {
            QueryMode::Intersection => {
                Some(candidates.intersection(&additional).copied().collect())
            }
            QueryMode::PreferIntersection => {
                // Focused recall uses lexical evidence to narrow marker matches. Falling back to
                // the marker union here can surface a different cell on one generic text token.
                Some(candidates.intersection(&additional).copied().collect())
            }
            QueryMode::Union => {
                let mut union = candidates;
                union.extend(additional);
                Some(union)
            }
        },
        (Some(candidates), None) => Some(candidates),
        (None, Some(additional)) => Some(additional),
        (None, None) => None,
    }
}

fn scoring_tokens(scoring: &CachedCellScoringData) -> BTreeSet<String> {
    scoring
        .subject_tokens
        .iter()
        .chain(scoring.value_tokens.iter())
        .cloned()
        .collect()
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

    pub fn append_cell(&self, cell: &MemoryCell, sync_after_write: bool) -> Result<()> {
        self.append_cell_with_key(cell, sync_after_write, None)
    }

    pub fn append_cell_with_key(
        &self,
        cell: &MemoryCell,
        sync_after_write: bool,
        key: Option<&SessionKey>,
    ) -> Result<()> {
        self.ensure_exists()?;
        let plaintext = rmp_serde::to_vec_named(cell)?;
        let (codec, payload) = match key {
            Some(key) => {
                let envelope = encrypt_payload(key, HOT_RECORD_AAD, &plaintext)?;
                (
                    CodecId::MessagePackEncrypted,
                    rmp_serde::to_vec_named(&envelope)?,
                )
            }
            None => (CodecId::MessagePack, plaintext),
        };
        let record = binary::encode_frame(FileKind::HotRecord, codec, &payload)?;
        self.validate_log_header()?;

        let mut file = OpenOptions::new().append(true).open(&self.path)?;
        file.write_all(&record)?;
        if sync_after_write {
            file.flush()?;
            file.sync_all()?;
        }
        Ok(())
    }

    fn validate_log_header(&self) -> Result<()> {
        let mut header = [0u8; binary::HEADER_LEN];
        let mut file = OpenOptions::new().read(true).open(&self.path)?;
        file.read_exact(&mut header)?;
        binary::decode_frame(&header, FileKind::HotLog)?;
        Ok(())
    }

    pub fn load_cells(&self) -> Result<Vec<MemoryCell>> {
        Ok(self.load_recovering()?.cells)
    }

    pub fn load_recovering(&self) -> Result<HotRecovery> {
        self.load_recovering_with_key(None)
    }

    pub fn load_recovering_with_key(&self, key: Option<&SessionKey>) -> Result<HotRecovery> {
        if !self.path.exists() {
            return Ok(HotRecovery::default());
        }

        let content = fs::read(&self.path)?;
        if content.is_empty() {
            return Ok(HotRecovery::default());
        }

        let (_, log_start_offset) = binary::decode_frame_at(&content, 0, FileKind::HotLog)?;
        let mut cells = Vec::new();
        let mut replay_offset = log_start_offset;

        if let Some(snapshot) = self.load_usable_snapshot(content.len(), log_start_offset, key)? {
            replay_offset = usize::try_from(snapshot.hot_log_offset).map_err(|_| {
                MgeError::StorageFormat(
                    "hot snapshot offset does not fit this platform".to_string(),
                )
            })?;
            cells = snapshot.cells;
        }

        let replay = decode_hot_records_from(&content, replay_offset, key)?;
        cells.extend(replay.cells);

        Ok(HotRecovery {
            cells,
            valid_log_offset: replay.valid_log_offset,
            recovered_bad_tail: replay.recovered_bad_tail,
        })
    }

    pub fn truncate_to_valid_offset(&self, offset: usize) -> Result<()> {
        if !self.path.exists() {
            return Ok(());
        }
        let file = OpenOptions::new().write(true).open(&self.path)?;
        file.set_len(u64::try_from(offset).map_err(|_| {
            MgeError::InvalidInput("hot log truncate offset is larger than u64".to_string())
        })?)?;
        file.sync_all()?;
        Ok(())
    }

    pub fn sync(&self) -> Result<()> {
        self.ensure_exists()?;
        let file = OpenOptions::new().read(true).write(true).open(&self.path)?;
        file.sync_all()?;
        Ok(())
    }

    pub fn write_snapshot(&self, cells: &[MemoryCell]) -> Result<HotSnapshot> {
        self.write_snapshot_with_key(cells, None)
    }

    pub fn write_snapshot_with_key(
        &self,
        cells: &[MemoryCell],
        key: Option<&SessionKey>,
    ) -> Result<HotSnapshot> {
        self.ensure_exists()?;
        self.sync()?;
        let hot_log_offset = fs::metadata(&self.path)?.len();
        let snapshot = HotSnapshot {
            version: HOT_SNAPSHOT_VERSION,
            created_at: current_timestamp(),
            hot_log_offset,
            cells: cells.to_vec(),
        };
        write_hot_snapshot_file(&self.snapshot_path(), &snapshot, key)?;
        Ok(snapshot)
    }

    pub fn remove_snapshot(&self) -> Result<()> {
        let path = self.snapshot_path();
        if path.exists() {
            fs::remove_file(path)?;
        }
        Ok(())
    }

    fn load_usable_snapshot(
        &self,
        hot_log_len: usize,
        log_start_offset: usize,
        key: Option<&SessionKey>,
    ) -> Result<Option<HotSnapshot>> {
        let path = self.snapshot_path();
        if !path.exists() {
            return Ok(None);
        }

        let snapshot = match read_hot_snapshot_file(&path, key) {
            Ok(snapshot) => snapshot,
            Err(_) => return Ok(None),
        };
        if snapshot.version != HOT_SNAPSHOT_VERSION {
            return Ok(None);
        }
        let Ok(offset) = usize::try_from(snapshot.hot_log_offset) else {
            return Ok(None);
        };
        if offset < log_start_offset || offset > hot_log_len {
            return Ok(None);
        }
        Ok(Some(snapshot))
    }

    fn snapshot_path(&self) -> PathBuf {
        self.path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join(HOT_SNAPSHOT_FILE)
    }

    pub fn archive_and_clear(&self) -> Result<Option<PathBuf>> {
        self.ensure_exists()?;
        self.sync()?;
        let bytes = fs::read(&self.path)?;
        let (_, offset) = binary::decode_frame_at(&bytes, 0, FileKind::HotLog)?;
        if bytes.len() == offset {
            binary::atomic_write_bytes(&self.path, &empty_hot_log_bytes()?)?;
            self.remove_snapshot()?;
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
        if let Err(error) = binary::atomic_write_bytes(&self.path, &empty_hot_log_bytes()?) {
            if let Err(rollback_error) = fs::rename(&archive_path, &self.path) {
                return Err(MgeError::StorageFormat(format!(
                    "failed to reset hot log after archive: {error}; failed to restore archived log: {rollback_error}"
                )));
            }
            return Err(error);
        }
        // A stale snapshot is ignored when its log offset exceeds the new log length.
        let _ = self.remove_snapshot();
        Ok(Some(archive_path))
    }
}

fn decode_hot_records_from(
    content: &[u8],
    mut offset: usize,
    key: Option<&SessionKey>,
) -> Result<HotRecovery> {
    let mut cells = Vec::new();
    while offset < content.len() {
        let (record, next_offset) =
            match binary::decode_frame_at(content, offset, FileKind::HotRecord) {
                Ok(decoded) => decoded,
                Err(err) => {
                    if binary::valid_frame_exists_after(
                        content,
                        offset.saturating_add(1),
                        FileKind::HotRecord,
                    ) {
                        return Err(err);
                    }
                    return Ok(HotRecovery {
                        cells,
                        valid_log_offset: offset,
                        recovered_bad_tail: true,
                    });
                }
            };
        let plaintext = match decode_hot_record_payload(&record.payload, record.codec, key) {
            Ok(payload) => payload,
            Err(err)
                if next_offset == content.len()
                    && matches!(
                        err,
                        MgeError::AuthenticationFailed(_) | MgeError::MessagePackDecode(_)
                    ) =>
            {
                return Ok(HotRecovery {
                    cells,
                    valid_log_offset: offset,
                    recovered_bad_tail: true,
                });
            }
            Err(err) => return Err(err),
        };
        let cell = match rmp_serde::from_slice(&plaintext) {
            Ok(cell) => cell,
            Err(_) => {
                return Ok(HotRecovery {
                    cells,
                    valid_log_offset: offset,
                    recovered_bad_tail: true,
                });
            }
        };
        cells.push(cell);
        offset = next_offset;
    }

    Ok(HotRecovery {
        cells,
        valid_log_offset: offset,
        recovered_bad_tail: false,
    })
}

fn empty_hot_log_bytes() -> Result<Vec<u8>> {
    binary::encode_frame(FileKind::HotLog, CodecId::None, &[])
}

fn decode_hot_record_payload(
    payload: &[u8],
    codec: CodecId,
    key: Option<&SessionKey>,
) -> Result<Vec<u8>> {
    match codec {
        CodecId::MessagePack => Ok(payload.to_vec()),
        CodecId::MessagePackEncrypted => {
            let key = key.ok_or_else(|| {
                MgeError::StoreLocked("encrypted hot record requires session unlock".to_string())
            })?;
            let envelope: EncryptedPayload = rmp_serde::from_slice(payload)?;
            decrypt_payload(key, HOT_RECORD_AAD, &envelope)
        }
        other => Err(MgeError::StorageFormat(format!(
            "wrong codec for hot record: expected messagepack or messagepack_encrypted, found {}",
            other.as_str()
        ))),
    }
}

fn write_hot_snapshot_file(
    path: &Path,
    snapshot: &HotSnapshot,
    key: Option<&SessionKey>,
) -> Result<()> {
    let plaintext = rmp_serde::to_vec_named(snapshot)?;
    let (codec, payload) = match key {
        Some(key) => {
            let envelope = encrypt_payload(key, HOT_SNAPSHOT_AAD, &plaintext)?;
            (
                CodecId::MessagePackEncrypted,
                rmp_serde::to_vec_named(&envelope)?,
            )
        }
        None => (CodecId::MessagePack, plaintext),
    };
    let bytes = binary::encode_frame(FileKind::HotSnapshot, codec, &payload)?;
    binary::atomic_write_bytes(path, &bytes)
}

fn read_hot_snapshot_file(path: &Path, key: Option<&SessionKey>) -> Result<HotSnapshot> {
    let bytes = fs::read(path)?;
    let frame = binary::decode_frame(&bytes, FileKind::HotSnapshot)?;
    let plaintext = match frame.codec {
        CodecId::MessagePack => frame.payload,
        CodecId::MessagePackEncrypted => {
            let key = key.ok_or_else(|| {
                MgeError::StoreLocked("encrypted hot snapshot requires session unlock".to_string())
            })?;
            let envelope: EncryptedPayload = rmp_serde::from_slice(&frame.payload)?;
            decrypt_payload(key, HOT_SNAPSHOT_AAD, &envelope)?
        }
        other => {
            return Err(MgeError::StorageFormat(format!(
                "wrong codec for hot snapshot: expected messagepack or messagepack_encrypted, found {}",
                other.as_str()
            )));
        }
    };
    Ok(rmp_serde::from_slice(&plaintext)?)
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
    use crate::models::{MemoryValue, SensitivityLevel, TrustLevel};

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

    #[test]
    fn hot_layer_builds_and_clears_runtime_scoring_cache() {
        let cell = MemoryCell::new(
            1,
            MemoryKind::ProjectFact,
            Some("answer style".to_string()),
            MemoryValue::Text("concise technical response".to_string()),
            "global".to_string(),
            MemoryStatus::Active,
            TrustLevel::ToolObserved,
            SensitivityLevel::Public,
            vec![11, 12],
            None,
            Vec::new(),
        );
        let mut layer = HotMemoryLayer::from_cells(vec![cell]);

        let scoring = layer.scoring(1).expect("hot scoring cache exists");
        assert!(scoring.value_tokens.iter().any(|token| token == "concise"));
        assert!(scoring.subject_tokens.iter().any(|token| token == "answer"));

        layer.clear();
        assert!(layer.scoring(1).is_none());
        assert!(layer.is_empty());
    }

    #[test]
    fn hot_layer_indexes_text_tokens_and_scores_candidates() {
        let cell = MemoryCell::new(
            7,
            MemoryKind::ProjectFact,
            Some("deployment policy".to_string()),
            MemoryValue::Text("rotate api credentials weekly".to_string()),
            "project".to_string(),
            MemoryStatus::Active,
            TrustLevel::ToolObserved,
            SensitivityLevel::Public,
            vec![11],
            None,
            Vec::new(),
        );
        let mut layer = HotMemoryLayer::from_cells(vec![cell]);
        let allowed_statuses = vec![MemoryStatus::Active];
        let query_tokens = vec!["credentials".to_string(), "weekly".to_string()];

        let candidates = layer.candidate_ids(HotCandidateQuery {
            marker_ids: &[],
            lexical_candidate_ids: &[7],
            marker_mode: QueryMode::Union,
            scope: Some("project"),
            kind: None,
            allowed_statuses: &allowed_statuses,
        });
        let scores = layer.lexical_scores(&query_tokens, Some("project"), None, &allowed_statuses);

        assert_eq!(candidates, vec![7]);
        assert!(scores.get(&7).copied().unwrap_or(0) > 0);
        layer.clear();
        assert!(layer
            .lexical_scores(&query_tokens, Some("project"), None, &allowed_statuses)
            .is_empty());
    }

    #[test]
    fn hot_log_header_validation_ignores_trailing_bytes() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("hot.mgl");
        let store = HotStore::new(&path);
        store.ensure_exists().unwrap();

        let mut file = OpenOptions::new().append(true).open(&path).unwrap();
        file.write_all(&vec![0xff; 1024 * 1024]).unwrap();
        drop(file);

        store.validate_log_header().unwrap();
    }

    #[test]
    fn hot_recovery_does_not_silently_discard_records_after_middle_corruption() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("hot.mgl");
        let store = HotStore::new(&path);
        store.ensure_exists().unwrap();
        for id in 1..=3 {
            let cell = MemoryCell::new(
                id,
                MemoryKind::ProjectFact,
                None,
                MemoryValue::Text(format!("memory {id}")),
                "global".to_string(),
                MemoryStatus::Active,
                TrustLevel::ToolObserved,
                SensitivityLevel::Public,
                Vec::new(),
                None,
                Vec::new(),
            );
            store.append_cell(&cell, false).unwrap();
        }

        let mut bytes = fs::read(&path).unwrap();
        let (_, first_record) = binary::decode_frame_at(&bytes, 0, FileKind::HotLog).unwrap();
        let (_, second_record) =
            binary::decode_frame_at(&bytes, first_record, FileKind::HotRecord).unwrap();
        let (_, third_record) =
            binary::decode_frame_at(&bytes, second_record, FileKind::HotRecord).unwrap();
        bytes[third_record - 1] ^= 0xff;
        fs::write(&path, bytes).unwrap();

        let err = store.load_recovering().unwrap_err();
        assert!(err
            .to_string()
            .contains("corrupted hot_record payload checksum"));
    }
}
