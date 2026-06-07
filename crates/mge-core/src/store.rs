use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::compression::{compress_with, decompress_with, CompressionKind};
use crate::errors::{MgeError, Result};
use crate::hot::HotStore;
use crate::indexes::{
    BinaryFusePageIndex, CandidateIndexData, CandidatePageIndex, ExactMarkerPageIndex, IndexKind,
};
use crate::markers::{
    canonicalize_marker, extract_query_marker_strings, marker_strings_for_cell_fields,
    tokenize_keywords, MarkerDebugEntry, MarkerDictionary,
};
use crate::models::{
    current_timestamp, CellId, MarkerId, MemoryCell, MemoryKind, MemorySource, MemoryStatus,
    MemoryValue, PageId, SensitivityLevel, TrustLevel,
};
use crate::packet::{ContextDebugInfo, ContextPacket};
use crate::pages::{
    attach_page_checksum, build_pages_with_kind, decode_page_with, encode_page_with,
    page_checksum_matches, page_file_name, MemoryPage, PageBuildOptions, PageCatalog,
    PageCatalogEntry, PageClustererKind, PageCodecKind,
};
use crate::retrieval::{
    build_context_packet, score_cell_debug, RankedCell, RecallRequest, Retriever,
};
use crate::security::{AuditEvent, AuditLogger, NoSecurity, NoopAuditLogger, SecurityProvider};

pub const DEFAULT_STORE_DIR: &str = ".memory-genome";

pub trait Store {
    fn remember(&mut self, request: RememberRequest) -> Result<MemoryCell>;
    fn recall(&self, request: RecallRequest) -> Result<ContextPacket>;
    fn seal(&mut self) -> Result<SealReport>;
    fn stats(&self) -> Result<StoreStats>;
}

#[derive(Clone, Debug)]
pub struct MemoryEngine {
    root: PathBuf,
    manifest: Manifest,
    dictionary: MarkerDictionary,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Manifest {
    pub version: u32,
    pub created_at: i64,
    pub updated_at: i64,
    pub next_cell_id: CellId,
    pub next_page_id: PageId,
    pub last_seal_time: Option<i64>,
    #[serde(default)]
    pub page_codec: PageCodecKind,
    #[serde(default)]
    pub compression: CompressionKind,
    #[serde(default)]
    pub index_kind: IndexKind,
    #[serde(default)]
    pub page_clusterer: PageClustererKind,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct InitOptions {
    pub page_codec: PageCodecKind,
    pub compression: CompressionKind,
    pub index_kind: IndexKind,
    pub page_clusterer: PageClustererKind,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct StorageConfig {
    pub page_codec: PageCodecKind,
    pub compression: CompressionKind,
    pub index_kind: IndexKind,
    pub page_clusterer: PageClustererKind,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct StorageConfigUpdate {
    pub page_codec: Option<PageCodecKind>,
    pub compression: Option<CompressionKind>,
    pub index_kind: Option<IndexKind>,
    pub page_clusterer: Option<PageClustererKind>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct StorageConfigUpdateReport {
    pub previous: StorageConfig,
    pub current: StorageConfig,
    pub changed: bool,
    pub existing_pages_unchanged: usize,
}

#[derive(Clone, Debug)]
pub struct RememberRequest {
    pub kind: MemoryKind,
    pub subject: Option<String>,
    pub value: MemoryValue,
    pub scope: String,
    pub status: MemoryStatus,
    pub trust: TrustLevel,
    pub sensitivity: SensitivityLevel,
    pub markers: Vec<String>,
    pub source: Option<MemorySource>,
    pub links: Vec<CellId>,
}

impl RememberRequest {
    pub fn new(kind: MemoryKind, value: MemoryValue) -> Self {
        Self {
            kind,
            subject: None,
            value,
            scope: "global".to_string(),
            status: MemoryStatus::Active,
            trust: TrustLevel::AgentInferred,
            sensitivity: SensitivityLevel::Private,
            markers: Vec::new(),
            source: None,
            links: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SealReport {
    pub hot_cells_sealed: usize,
    pub pages_written: usize,
    pub archived_hot_log: Option<PathBuf>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct StoreStats {
    pub hot_cells: usize,
    pub sealed_pages: usize,
    pub sealed_cells: usize,
    pub marker_count: usize,
    pub page_count: usize,
    pub current_page_codec: PageCodecKind,
    pub current_compression: CompressionKind,
    pub current_index_kind: IndexKind,
    pub current_page_clusterer: PageClustererKind,
    pub index_type: String,
    pub last_seal_time: Option<i64>,
    pub store_size_bytes: u64,
}

impl StoreStats {
    pub fn to_human_text(&self) -> String {
        format!(
            "\
hot cells: {}
sealed pages: {}
sealed cells: {}
marker count: {}
page count: {}
current page codec: {}
current compression: {}
current index kind: {}
current page clusterer: {}
index type: {}
last seal time: {}
store size bytes: {}
",
            self.hot_cells,
            self.sealed_pages,
            self.sealed_cells,
            self.marker_count,
            self.page_count,
            self.current_page_codec,
            self.current_compression,
            self.current_index_kind,
            self.current_page_clusterer,
            self.index_type,
            self.last_seal_time
                .map(|value| value.to_string())
                .unwrap_or_else(|| "never".to_string()),
            self.store_size_bytes
        )
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct InspectReport {
    pub manifest: Manifest,
    pub markers: Vec<MarkerDebugEntry>,
    pub page_catalog: PageCatalog,
    pub index: CandidateIndexData,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ValidationReport {
    pub ok: bool,
    pub index_kind: IndexKind,
    pub checked_hot_cells: usize,
    pub checked_sealed_pages: usize,
    pub checked_sealed_cells: usize,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}

impl ValidationReport {
    fn new(index_kind: IndexKind) -> Self {
        Self {
            ok: true,
            index_kind,
            checked_hot_cells: 0,
            checked_sealed_pages: 0,
            checked_sealed_cells: 0,
            errors: Vec::new(),
            warnings: Vec::new(),
        }
    }

    fn error(&mut self, message: impl Into<String>) {
        self.ok = false;
        self.errors.push(message.into());
    }

    fn warning(&mut self, message: impl Into<String>) {
        self.warnings.push(message.into());
    }

    pub fn to_human_text(&self) -> String {
        let mut output = format!(
            "\
store valid: {}
index kind: {}
checked hot cells: {}
checked sealed pages: {}
checked sealed cells: {}
errors: {}
warnings: {}
",
            self.ok,
            self.index_kind,
            self.checked_hot_cells,
            self.checked_sealed_pages,
            self.checked_sealed_cells,
            self.errors.len(),
            self.warnings.len()
        );

        for error in &self.errors {
            output.push_str("- error: ");
            output.push_str(error);
            output.push('\n');
        }
        for warning in &self.warnings {
            output.push_str("- warning: ");
            output.push_str(warning);
            output.push('\n');
        }

        output
    }
}

impl MemoryEngine {
    pub fn init_at(store_root: impl AsRef<Path>) -> Result<Self> {
        Self::init_with_options(store_root, InitOptions::default())
    }

    pub fn init_with_options(store_root: impl AsRef<Path>, options: InitOptions) -> Result<Self> {
        let root = store_root.as_ref().to_path_buf();
        fs::create_dir_all(root.join("hot"))?;
        fs::create_dir_all(root.join("pages"))?;
        fs::create_dir_all(root.join("indexes"))?;
        fs::create_dir_all(root.join("debug"))?;

        let manifest_path = root.join("manifest.json");
        if !manifest_path.exists() {
            let now = current_timestamp();
            let manifest = Manifest {
                version: 1,
                created_at: now,
                updated_at: now,
                next_cell_id: 1,
                next_page_id: 1,
                last_seal_time: None,
                page_codec: options.page_codec,
                compression: options.compression,
                index_kind: options.index_kind,
                page_clusterer: options.page_clusterer,
            };
            fs::write(&manifest_path, serde_json::to_vec_pretty(&manifest)?)?;
        }

        let dictionary = MarkerDictionary::load_from_path(root.join("markers.json"))?;
        dictionary.save_to_path(root.join("markers.json"))?;

        HotStore::new(root.join("hot").join("hot_cells.jsonl")).ensure_exists()?;
        if !root.join("indexes").join("page_catalog.json").exists() {
            save_json(
                root.join("indexes").join("page_catalog.json"),
                &PageCatalog::default(),
            )?;
        }
        match options.index_kind {
            IndexKind::ExactMarkerPage => {
                if !root.join("indexes").join("marker_to_pages.json").exists() {
                    ExactMarkerPageIndex::default()
                        .save_to_path(root.join("indexes").join("marker_to_pages.json"))?;
                }
            }
            IndexKind::BinaryFusePage => {
                if !root.join("indexes").join("binary_fuse_pages.json").exists() {
                    BinaryFusePageIndex::default()
                        .save_to_path(root.join("indexes").join("binary_fuse_pages.json"))?;
                }
            }
        }

        Self::open_at(root)
    }

    pub fn open_at(store_root: impl AsRef<Path>) -> Result<Self> {
        let root = store_root.as_ref().to_path_buf();
        let manifest_path = root.join("manifest.json");
        if !manifest_path.exists() {
            return Err(MgeError::NotInitialized(root.display().to_string()));
        }

        let manifest: Manifest = serde_json::from_slice(&fs::read(manifest_path)?)?;
        let dictionary = MarkerDictionary::load_from_path(root.join("markers.json"))?;

        Ok(Self {
            root,
            manifest,
            dictionary,
        })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn dictionary(&self) -> &MarkerDictionary {
        &self.dictionary
    }

    pub fn storage_config(&self) -> StorageConfig {
        StorageConfig {
            page_codec: self.manifest.page_codec,
            compression: self.manifest.compression,
            index_kind: self.manifest.index_kind,
            page_clusterer: self.manifest.page_clusterer,
        }
    }

    pub fn update_storage_config(
        &mut self,
        update: StorageConfigUpdate,
    ) -> Result<StorageConfigUpdateReport> {
        if update.page_codec.is_none()
            && update.compression.is_none()
            && update.index_kind.is_none()
            && update.page_clusterer.is_none()
        {
            return Err(MgeError::InvalidInput(
                "storage config update requires page_codec, compression, index_kind, or page_clusterer"
                    .to_string(),
            ));
        }

        let previous = self.storage_config();
        if let Some(page_codec) = update.page_codec {
            self.manifest.page_codec = page_codec;
        }
        if let Some(compression) = update.compression {
            self.manifest.compression = compression;
        }
        if let Some(index_kind) = update.index_kind {
            self.manifest.index_kind = index_kind;
        }
        if let Some(page_clusterer) = update.page_clusterer {
            self.manifest.page_clusterer = page_clusterer;
        }

        let current = self.storage_config();
        let changed = previous != current;
        if changed {
            self.manifest.updated_at = current_timestamp();
            self.save_manifest()?;
        }

        if previous.index_kind != current.index_kind {
            let pages = self.load_all_pages()?;
            let index = self.build_candidate_index(&pages)?;
            self.save_candidate_index(&index)?;
            let catalog = self.load_page_catalog()?;
            self.save_page_catalog(&catalog)?;
        }

        Ok(StorageConfigUpdateReport {
            previous,
            current,
            changed,
            existing_pages_unchanged: self.load_page_catalog()?.pages.len(),
        })
    }

    pub fn remember(&mut self, request: RememberRequest) -> Result<MemoryCell> {
        let marker_strings = marker_strings_for_cell_fields(
            &request.kind,
            request.subject.as_deref(),
            &request.value,
            &request.scope,
            &request.status,
            &request.trust,
            &request.sensitivity,
            &request.markers,
        )?;

        let mut marker_ids = Vec::with_capacity(marker_strings.len());
        for marker in marker_strings {
            marker_ids.push(self.dictionary.get_or_insert(&marker)?);
        }

        let cell_id = self.manifest.next_cell_id;
        self.manifest.next_cell_id = self
            .manifest
            .next_cell_id
            .checked_add(1)
            .ok_or_else(|| MgeError::InvalidInput("cell id overflow".to_string()))?;

        let cell = MemoryCell::new(
            cell_id,
            request.kind,
            request.subject,
            request.value,
            request.scope,
            request.status,
            request.trust,
            request.sensitivity,
            marker_ids,
            request.source,
            request.links,
        );

        HotStore::new(self.hot_cells_path()).append_cell(&cell)?;
        self.manifest.updated_at = current_timestamp();
        self.save_manifest()?;
        self.dictionary.save_to_path(self.markers_path())?;

        Ok(cell)
    }

    pub fn recall(&self, request: RecallRequest) -> Result<ContextPacket> {
        let mut marker_strings = extract_query_marker_strings(&request.query);
        for explicit in &request.markers {
            marker_strings.push(canonicalize_marker(explicit)?);
        }
        if let Some(scope) = &request.scope {
            marker_strings.push(canonicalize_marker(&format!("scope:{scope}"))?);
        }
        if let Some(kind) = request.kind {
            marker_strings.push(canonicalize_marker(&format!("kind:{}", kind.as_str()))?);
        }
        marker_strings.sort();
        marker_strings.dedup();

        let query_marker_ids = marker_strings
            .iter()
            .filter_map(|marker| self.dictionary.lookup(marker))
            .collect::<Vec<_>>();
        let query_tokens = tokenize_keywords(&request.query);

        let hot_cells = HotStore::new(self.hot_cells_path()).load_cells()?;
        let mut ranked = Vec::new();
        for cell in &hot_cells {
            if let Some(score_detail) =
                score_cell_debug(cell, &request, &query_marker_ids, &query_tokens)
            {
                ranked.push(RankedCell {
                    cell: cell.clone(),
                    score: score_detail.score,
                    score_detail,
                });
            }
        }

        let index = self.load_candidate_index()?;
        let candidate_query = if query_marker_ids.is_empty() {
            Default::default()
        } else {
            index.query_with_stats(&query_marker_ids)?
        };
        let candidate_pages = candidate_query.page_ids;

        let catalog = self.load_page_catalog()?;
        let entries_by_id = catalog
            .pages
            .iter()
            .map(|entry| (entry.page_id, entry))
            .collect::<BTreeMap<_, _>>();

        let mut sealed_cells_scanned = 0;
        let mut loaded_pages = 0;
        let mut false_positive_candidate_pages = 0;
        for page_id in &candidate_pages {
            let Some(entry) = entries_by_id.get(page_id) else {
                continue;
            };
            let page = self.read_page(entry)?;
            loaded_pages += 1;
            sealed_cells_scanned += page.cells.len();
            let before_page_candidates = ranked.len();
            for cell in &page.cells {
                if let Some(score_detail) =
                    score_cell_debug(cell, &request, &query_marker_ids, &query_tokens)
                {
                    ranked.push(RankedCell {
                        cell: cell.clone(),
                        score: score_detail.score,
                        score_detail,
                    });
                }
            }
            if ranked.len() == before_page_candidates {
                false_positive_candidate_pages += 1;
            }
        }

        ranked.sort_by(|left, right| {
            right
                .score
                .cmp(&left.score)
                .then_with(|| right.cell.updated_at.cmp(&left.cell.updated_at))
                .then_with(|| left.cell.id.cmp(&right.cell.id))
        });

        let debug = ContextDebugInfo {
            index_kind: self.manifest.index_kind,
            hot_cells_scanned: hot_cells.len(),
            candidate_pages,
            page_filters_scanned: candidate_query.page_filters_scanned,
            candidate_pages_returned: candidate_query.candidate_pages_returned,
            loaded_pages,
            sealed_cells_scanned,
            false_positive_candidate_pages,
            total_candidates: ranked.len(),
            score_details: Vec::new(),
        };

        NoopAuditLogger.record(&AuditEvent {
            event_type: "recall".to_string(),
            summary: format!(
                "query markers={}, candidates={}",
                query_marker_ids.len(),
                ranked.len()
            ),
        })?;

        Ok(build_context_packet(
            request.query,
            &ranked,
            &self.dictionary,
            debug,
            request.max_items,
        ))
    }

    pub fn seal(&mut self) -> Result<SealReport> {
        let hot_store = HotStore::new(self.hot_cells_path());
        let hot_cells = hot_store.load_cells()?;
        if hot_cells.is_empty() {
            return Ok(SealReport {
                hot_cells_sealed: 0,
                pages_written: 0,
                archived_hot_log: None,
            });
        }

        for cell in &hot_cells {
            for marker in &cell.markers {
                if self.dictionary.marker(*marker).is_none() {
                    return Err(MgeError::InvalidInput(format!(
                        "cell {} references unknown marker {}",
                        cell.id, marker
                    )));
                }
            }
        }

        let mut pages = build_pages_with_kind(
            &hot_cells,
            self.manifest.next_page_id,
            self.manifest.page_clusterer,
            PageBuildOptions::default(),
        );
        for page in &mut pages {
            attach_page_checksum(page)?;
        }
        let mut catalog = self.load_page_catalog()?;
        for page in &pages {
            self.write_page(page)?;
            catalog.pages.push(PageCatalogEntry {
                page_id: page.page_id,
                file: page_file_name(page.page_id),
                page_codec: self.manifest.page_codec,
                compression: self.manifest.compression,
                page_clusterer: self.manifest.page_clusterer,
                created_at: page.created_at,
                cell_count: page.cell_count,
                marker_summary: page.marker_summary.clone(),
            });
            self.manifest.next_page_id = self.manifest.next_page_id.max(page.page_id + 1);
        }
        self.save_page_catalog(&catalog)?;

        let all_pages = self.load_all_pages()?;
        let index = self.build_candidate_index(&all_pages)?;
        self.save_candidate_index(&index)?;

        let archived_hot_log = hot_store.archive_and_clear()?;
        self.manifest.last_seal_time = Some(current_timestamp());
        self.manifest.updated_at = current_timestamp();
        self.save_manifest()?;

        Ok(SealReport {
            hot_cells_sealed: hot_cells.len(),
            pages_written: pages.len(),
            archived_hot_log,
        })
    }

    pub fn stats(&self) -> Result<StoreStats> {
        let hot_cells = HotStore::new(self.hot_cells_path()).load_cells()?.len();
        let catalog = self.load_page_catalog()?;
        let sealed_cells = catalog.pages.iter().map(|entry| entry.cell_count).sum();

        Ok(StoreStats {
            hot_cells,
            sealed_pages: catalog.pages.len(),
            sealed_cells,
            marker_count: self.dictionary.len(),
            page_count: catalog.pages.len(),
            current_page_codec: self.manifest.page_codec,
            current_compression: self.manifest.compression,
            current_index_kind: self.manifest.index_kind,
            current_page_clusterer: self.manifest.page_clusterer,
            index_type: self.manifest.index_kind.to_string(),
            last_seal_time: self.manifest.last_seal_time,
            store_size_bytes: store_size_bytes(&self.root)?,
        })
    }

    pub fn inspect(&self) -> Result<InspectReport> {
        Ok(InspectReport {
            manifest: self.manifest.clone(),
            markers: self.dictionary.debug_view(),
            page_catalog: self.load_page_catalog()?,
            index: self.load_candidate_index()?,
        })
    }

    pub fn export_json(&self) -> Result<serde_json::Value> {
        let hot_cells = HotStore::new(self.hot_cells_path()).load_cells()?;
        let page_catalog = self.load_page_catalog()?;
        let pages = self.load_all_pages()?;
        let index = self.load_candidate_index()?;

        Ok(serde_json::json!({
            "manifest": self.manifest,
            "markers": self.dictionary.debug_view(),
            "hot_cells": hot_cells,
            "page_catalog": page_catalog,
            "index": index,
            "pages": pages,
        }))
    }

    pub fn validate(&self) -> Result<ValidationReport> {
        let mut report = ValidationReport::new(self.manifest.index_kind);
        let catalog = self.load_page_catalog()?;

        if catalog.index_kind != self.manifest.index_kind {
            report.error(format!(
                "page catalog index kind {} does not match manifest index kind {}",
                catalog.index_kind, self.manifest.index_kind
            ));
        }

        let index = match self.load_candidate_index() {
            Ok(index) => Some(index),
            Err(err) => {
                report.error(format!("candidate index load failed: {err}"));
                None
            }
        };
        if let Some(index) = &index {
            if index.kind() != self.manifest.index_kind {
                report.error(format!(
                    "candidate index kind {} does not match manifest index kind {}",
                    index.kind(),
                    self.manifest.index_kind
                ));
            }
        }

        let hot_cells = HotStore::new(self.hot_cells_path()).load_cells()?;
        report.checked_hot_cells = hot_cells.len();
        let mut max_cell_id = 0;
        for cell in &hot_cells {
            max_cell_id = max_cell_id.max(cell.id);
            self.validate_cell_markers("hot cell", cell, &mut report);
        }

        let mut page_ids = BTreeSet::new();
        let mut page_files = BTreeSet::new();
        let mut max_page_id = 0;
        for entry in &catalog.pages {
            if !page_ids.insert(entry.page_id) {
                report.error(format!(
                    "duplicate page id {} in page catalog",
                    entry.page_id
                ));
            }
            if !page_files.insert(entry.file.clone()) {
                report.error(format!(
                    "duplicate page file {} in page catalog",
                    entry.file
                ));
            }
            max_page_id = max_page_id.max(entry.page_id);

            let page_path = self.pages_dir().join(&entry.file);
            if !page_path.exists() {
                report.error(format!(
                    "missing page file for page {}: {}",
                    entry.page_id,
                    page_path.display()
                ));
                continue;
            }

            match self.read_page(entry) {
                Ok(page) => {
                    max_cell_id =
                        max_cell_id.max(page.cells.iter().map(|cell| cell.id).max().unwrap_or(0));
                    self.validate_page(entry, &page, &mut report);
                }
                Err(err) => {
                    report.error(format!("failed to read page {}: {err}", entry.page_id));
                }
            }
        }

        if !catalog.pages.is_empty() && self.manifest.next_page_id <= max_page_id {
            report.error(format!(
                "manifest next_page_id {} must be greater than max sealed page id {}",
                self.manifest.next_page_id, max_page_id
            ));
        }
        if max_cell_id > 0 && self.manifest.next_cell_id <= max_cell_id {
            report.error(format!(
                "manifest next_cell_id {} must be greater than max cell id {}",
                self.manifest.next_cell_id, max_cell_id
            ));
        }

        if let Some(index) = &index {
            self.validate_candidate_index(&catalog, index, &mut report)?;
        }

        if catalog.pages.is_empty() && hot_cells.is_empty() {
            report.warning("store contains no hot or sealed cells");
        }

        Ok(report)
    }

    fn save_manifest(&self) -> Result<()> {
        save_json(self.manifest_path(), &self.manifest)
    }

    fn load_page_catalog(&self) -> Result<PageCatalog> {
        let path = self.page_catalog_path();
        if !path.exists() {
            return Ok(PageCatalog::default());
        }
        Ok(serde_json::from_slice(&fs::read(path)?)?)
    }

    fn save_page_catalog(&self, catalog: &PageCatalog) -> Result<()> {
        let mut catalog = catalog.clone();
        catalog.index_kind = self.manifest.index_kind;
        save_json(self.page_catalog_path(), &catalog)
    }

    fn load_all_pages(&self) -> Result<Vec<MemoryPage>> {
        let catalog = self.load_page_catalog()?;
        catalog
            .pages
            .iter()
            .map(|entry| self.read_page(entry))
            .collect()
    }

    fn read_page(&self, entry: &PageCatalogEntry) -> Result<MemoryPage> {
        let stored = fs::read(self.pages_dir().join(&entry.file))?;
        let security = NoSecurity;
        let opened = security.open_page_bytes(&stored)?;
        let decoded = decompress_with(entry.compression, &opened)?;
        decode_page_with(entry.page_codec, &decoded)
    }

    fn write_page(&self, page: &MemoryPage) -> Result<()> {
        let security = NoSecurity;

        let encoded = encode_page_with(self.manifest.page_codec, page)?;
        let compressed = compress_with(self.manifest.compression, &encoded)?;
        let stored = security.seal_page_bytes(&compressed)?;
        // Future order remains: encode page -> compress page -> encrypt page -> write page.
        fs::write(self.pages_dir().join(page_file_name(page.page_id)), stored)?;
        Ok(())
    }

    fn build_candidate_index(&self, pages: &[MemoryPage]) -> Result<CandidateIndexData> {
        match self.manifest.index_kind {
            IndexKind::ExactMarkerPage => Ok(CandidateIndexData::ExactMarkerPage(
                ExactMarkerPageIndex::build(pages)?,
            )),
            IndexKind::BinaryFusePage => Ok(CandidateIndexData::BinaryFusePage(
                BinaryFusePageIndex::build(pages)?,
            )),
        }
    }

    fn load_candidate_index(&self) -> Result<CandidateIndexData> {
        match self.manifest.index_kind {
            IndexKind::ExactMarkerPage => Ok(CandidateIndexData::ExactMarkerPage(
                ExactMarkerPageIndex::load_from_path(self.marker_index_path())?,
            )),
            IndexKind::BinaryFusePage => Ok(CandidateIndexData::BinaryFusePage(
                BinaryFusePageIndex::load_from_path(self.binary_fuse_index_path())?,
            )),
        }
    }

    fn save_candidate_index(&self, index: &CandidateIndexData) -> Result<()> {
        match index {
            CandidateIndexData::ExactMarkerPage(index) => {
                index.save_to_path(self.marker_index_path())
            }
            CandidateIndexData::BinaryFusePage(index) => {
                index.save_to_path(self.binary_fuse_index_path())
            }
        }
    }

    fn validate_page(
        &self,
        entry: &PageCatalogEntry,
        page: &MemoryPage,
        report: &mut ValidationReport,
    ) {
        report.checked_sealed_pages += 1;
        report.checked_sealed_cells += page.cells.len();

        if page.page_id != entry.page_id {
            report.error(format!(
                "page file {} contains page id {}, catalog expects {}",
                entry.file, page.page_id, entry.page_id
            ));
        }
        if page.cell_count != page.cells.len() {
            report.error(format!(
                "page {} cell_count {} does not match actual cells {}",
                entry.page_id,
                page.cell_count,
                page.cells.len()
            ));
        }
        if entry.cell_count != page.cells.len() {
            report.error(format!(
                "catalog page {} cell_count {} does not match actual cells {}",
                entry.page_id,
                entry.cell_count,
                page.cells.len()
            ));
        }
        if entry.marker_summary != page.marker_summary {
            report.error(format!(
                "catalog marker_summary differs from page marker_summary for page {}",
                entry.page_id
            ));
        }

        let computed_marker_summary = marker_summary_for_cells(&page.cells);
        if computed_marker_summary != page.marker_summary {
            report.error(format!(
                "page {} marker_summary does not match page cells",
                entry.page_id
            ));
        }
        match &page.checksum {
            Some(_) => match page_checksum_matches(page) {
                Ok(true) => {}
                Ok(false) => report.error(format!("page {} checksum mismatch", entry.page_id)),
                Err(err) => report.error(format!(
                    "page {} checksum verification failed: {err}",
                    entry.page_id
                )),
            },
            None => report.warning(format!("page {} has no checksum", entry.page_id)),
        }

        for cell in &page.cells {
            self.validate_cell_markers("sealed cell", cell, report);
        }
    }

    fn validate_cell_markers(&self, label: &str, cell: &MemoryCell, report: &mut ValidationReport) {
        for marker in &cell.markers {
            if self.dictionary.marker(*marker).is_none() {
                report.error(format!(
                    "{label} {} references unknown marker {}",
                    cell.id, marker
                ));
            }
        }
    }

    fn validate_candidate_index(
        &self,
        catalog: &PageCatalog,
        index: &CandidateIndexData,
        report: &mut ValidationReport,
    ) -> Result<()> {
        let catalog_page_ids = catalog
            .pages
            .iter()
            .map(|entry| entry.page_id)
            .collect::<BTreeSet<_>>();
        let entries_by_id = catalog
            .pages
            .iter()
            .map(|entry| (entry.page_id, entry))
            .collect::<BTreeMap<_, _>>();

        match index {
            CandidateIndexData::ExactMarkerPage(index) => {
                validate_page_id_set(
                    "exact index all_pages",
                    index.all_pages.iter().copied().collect(),
                    &catalog_page_ids,
                    report,
                );
                for (marker, page_ids) in &index.marker_to_pages {
                    if self.dictionary.marker(*marker).is_none() {
                        report.error(format!("exact index references unknown marker {marker}"));
                    }
                    for page_id in page_ids {
                        if !catalog_page_ids.contains(page_id) {
                            report.error(format!(
                                "exact index marker {} references unknown page {}",
                                marker, page_id
                            ));
                        }
                    }
                }
            }
            CandidateIndexData::BinaryFusePage(index) => {
                validate_page_id_set(
                    "binary fuse index all_pages",
                    index.all_pages.iter().copied().collect(),
                    &catalog_page_ids,
                    report,
                );
                validate_page_id_set(
                    "binary fuse page filters",
                    index
                        .page_filters
                        .iter()
                        .map(|filter| filter.page_id)
                        .collect(),
                    &catalog_page_ids,
                    report,
                );
                for filter in &index.page_filters {
                    let Some(entry) = entries_by_id.get(&filter.page_id) else {
                        continue;
                    };
                    let marker_count = entry
                        .marker_summary
                        .iter()
                        .copied()
                        .collect::<BTreeSet<_>>()
                        .len();
                    if filter.marker_count != marker_count {
                        report.error(format!(
                            "binary fuse page {} marker_count {} does not match catalog marker count {}",
                            filter.page_id, filter.marker_count, marker_count
                        ));
                    }
                    if marker_count == 0 && filter.filter.is_some() {
                        report.error(format!(
                            "binary fuse page {} has a filter for an empty marker_summary",
                            filter.page_id
                        ));
                    }
                    if marker_count > 0 && filter.filter.is_none() {
                        report.error(format!(
                            "binary fuse page {} is missing filter for non-empty marker_summary",
                            filter.page_id
                        ));
                    }
                }
            }
        }

        for entry in &catalog.pages {
            for marker in &entry.marker_summary {
                let candidate_pages = index.query(&[*marker])?;
                if !candidate_pages.contains(&entry.page_id) {
                    report.error(format!(
                        "candidate index misses page {} for marker {}",
                        entry.page_id, marker
                    ));
                }
            }
        }

        Ok(())
    }

    fn manifest_path(&self) -> PathBuf {
        self.root.join("manifest.json")
    }

    fn markers_path(&self) -> PathBuf {
        self.root.join("markers.json")
    }

    fn hot_cells_path(&self) -> PathBuf {
        self.root.join("hot").join("hot_cells.jsonl")
    }

    fn pages_dir(&self) -> PathBuf {
        self.root.join("pages")
    }

    fn page_catalog_path(&self) -> PathBuf {
        self.root.join("indexes").join("page_catalog.json")
    }

    fn marker_index_path(&self) -> PathBuf {
        self.root.join("indexes").join("marker_to_pages.json")
    }

    fn binary_fuse_index_path(&self) -> PathBuf {
        self.root.join("indexes").join("binary_fuse_pages.json")
    }
}

impl Store for MemoryEngine {
    fn remember(&mut self, request: RememberRequest) -> Result<MemoryCell> {
        MemoryEngine::remember(self, request)
    }

    fn recall(&self, request: RecallRequest) -> Result<ContextPacket> {
        MemoryEngine::recall(self, request)
    }

    fn seal(&mut self) -> Result<SealReport> {
        MemoryEngine::seal(self)
    }

    fn stats(&self) -> Result<StoreStats> {
        MemoryEngine::stats(self)
    }
}

impl Retriever for MemoryEngine {
    fn recall(&self, request: RecallRequest) -> Result<ContextPacket> {
        MemoryEngine::recall(self, request)
    }
}

fn save_json(path: impl AsRef<Path>, value: &impl Serialize) -> Result<()> {
    if let Some(parent) = path.as_ref().parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, serde_json::to_vec_pretty(value)?)?;
    Ok(())
}

fn store_size_bytes(path: &Path) -> Result<u64> {
    if !path.exists() {
        return Ok(0);
    }

    let metadata = fs::metadata(path)?;
    if metadata.is_file() {
        return Ok(metadata.len());
    }

    let mut total = 0;
    for entry in fs::read_dir(path)? {
        total += store_size_bytes(&entry?.path())?;
    }
    Ok(total)
}

fn validate_page_id_set(
    label: &str,
    actual: BTreeSet<PageId>,
    expected: &BTreeSet<PageId>,
    report: &mut ValidationReport,
) {
    for page_id in expected.difference(&actual) {
        report.error(format!("{label} is missing page {page_id}"));
    }
    for page_id in actual.difference(expected) {
        report.error(format!("{label} contains unknown page {page_id}"));
    }
}

fn marker_summary_for_cells(cells: &[MemoryCell]) -> Vec<MarkerId> {
    let mut summary = BTreeSet::new();
    for cell in cells {
        summary.extend(cell.markers.iter().copied());
    }
    summary.into_iter().collect()
}
