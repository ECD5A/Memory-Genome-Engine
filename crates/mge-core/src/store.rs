use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::compression::{compress_with, decompress_with, CompressionKind};
use crate::errors::{MgeError, Result};
use crate::hot::HotStore;
use crate::indexes::{CandidatePageIndex, ExactMarkerPageIndex, IndexKind};
use crate::markers::{
    canonicalize_marker, extract_query_marker_strings, marker_strings_for_cell_fields,
    tokenize_keywords, MarkerDebugEntry, MarkerDictionary,
};
use crate::models::{
    current_timestamp, CellId, MemoryCell, MemoryKind, MemorySource, MemoryStatus, MemoryValue,
    PageId, SensitivityLevel, TrustLevel,
};
use crate::packet::{ContextDebugInfo, ContextPacket};
use crate::pages::{
    build_pages_from_cells, decode_page_with, encode_page_with, page_file_name, MemoryPage,
    PageCatalog, PageCatalogEntry, PageCodecKind,
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
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct InitOptions {
    pub page_codec: PageCodecKind,
    pub compression: CompressionKind,
    pub index_kind: IndexKind,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct StorageConfig {
    pub page_codec: PageCodecKind,
    pub compression: CompressionKind,
    pub index_kind: IndexKind,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct StorageConfigUpdate {
    pub page_codec: Option<PageCodecKind>,
    pub compression: Option<CompressionKind>,
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
            self.index_type,
            self.last_seal_time
                .map(|value| value.to_string())
                .unwrap_or_else(|| "never".to_string()),
            self.store_size_bytes
        )
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct InspectReport {
    pub manifest: Manifest,
    pub markers: Vec<MarkerDebugEntry>,
    pub page_catalog: PageCatalog,
    pub index: ExactMarkerPageIndex,
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
        if !root.join("indexes").join("marker_to_pages.json").exists() {
            ExactMarkerPageIndex::default()
                .save_to_path(root.join("indexes").join("marker_to_pages.json"))?;
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
        }
    }

    pub fn update_storage_config(
        &mut self,
        update: StorageConfigUpdate,
    ) -> Result<StorageConfigUpdateReport> {
        if update.page_codec.is_none() && update.compression.is_none() {
            return Err(MgeError::InvalidInput(
                "storage config update requires page_codec or compression".to_string(),
            ));
        }

        let previous = self.storage_config();
        if let Some(page_codec) = update.page_codec {
            self.manifest.page_codec = page_codec;
        }
        if let Some(compression) = update.compression {
            self.manifest.compression = compression;
        }

        let current = self.storage_config();
        let changed = previous != current;
        if changed {
            self.manifest.updated_at = current_timestamp();
            self.save_manifest()?;
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

        let index = ExactMarkerPageIndex::load_from_path(self.marker_index_path())?;
        let candidate_pages = if query_marker_ids.is_empty() {
            Vec::new()
        } else {
            index.query(&query_marker_ids)?
        };

        let catalog = self.load_page_catalog()?;
        let entries_by_id = catalog
            .pages
            .iter()
            .map(|entry| (entry.page_id, entry))
            .collect::<BTreeMap<_, _>>();

        let mut sealed_cells_scanned = 0;
        for page_id in &candidate_pages {
            let Some(entry) = entries_by_id.get(page_id) else {
                continue;
            };
            let page = self.read_page(entry)?;
            sealed_cells_scanned += page.cells.len();
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
        }

        ranked.sort_by(|left, right| {
            right
                .score
                .cmp(&left.score)
                .then_with(|| right.cell.updated_at.cmp(&left.cell.updated_at))
                .then_with(|| left.cell.id.cmp(&right.cell.id))
        });

        let debug = ContextDebugInfo {
            hot_cells_scanned: hot_cells.len(),
            candidate_pages,
            sealed_cells_scanned,
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

        let pages = build_pages_from_cells(&hot_cells, self.manifest.next_page_id);
        let mut catalog = self.load_page_catalog()?;
        for page in &pages {
            self.write_page(page)?;
            catalog.pages.push(PageCatalogEntry {
                page_id: page.page_id,
                file: page_file_name(page.page_id),
                page_codec: self.manifest.page_codec,
                compression: self.manifest.compression,
                created_at: page.created_at,
                cell_count: page.cell_count,
                marker_summary: page.marker_summary.clone(),
            });
            self.manifest.next_page_id = self.manifest.next_page_id.max(page.page_id + 1);
        }
        self.save_page_catalog(&catalog)?;

        let all_pages = self.load_all_pages()?;
        let index = self.build_candidate_index(&all_pages)?;
        index.save_to_path(self.marker_index_path())?;

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
            index: ExactMarkerPageIndex::load_from_path(self.marker_index_path())?,
        })
    }

    pub fn export_json(&self) -> Result<serde_json::Value> {
        let hot_cells = HotStore::new(self.hot_cells_path()).load_cells()?;
        let page_catalog = self.load_page_catalog()?;
        let pages = self.load_all_pages()?;
        let index = ExactMarkerPageIndex::load_from_path(self.marker_index_path())?;

        Ok(serde_json::json!({
            "manifest": self.manifest,
            "markers": self.dictionary.debug_view(),
            "hot_cells": hot_cells,
            "page_catalog": page_catalog,
            "index": index,
            "pages": pages,
        }))
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

    fn build_candidate_index(&self, pages: &[MemoryPage]) -> Result<ExactMarkerPageIndex> {
        match self.manifest.index_kind {
            IndexKind::ExactMarkerPage => ExactMarkerPageIndex::build(pages),
        }
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
