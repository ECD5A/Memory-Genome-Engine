use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::{Arc, OnceLock};
use std::time::Instant;

use serde::{Deserialize, Serialize};

use crate::binary::{self, CodecId, FileKind};
use crate::compression::{compress_with, decompress_with, CompressionKind};
use crate::errors::{MgeError, Result};
use crate::hot::{allowed_statuses_for_policy, HotCandidateQuery, HotMemoryLayer, HotStore};
use crate::indexes::{
    BinaryFusePageIndex, CandidateIndexData, CandidatePageIndex, ExactMarkerPageIndex, IndexKind,
    QueryMode,
};
use crate::markers::{
    canonicalize_marker, extract_query_marker_strings, marker_strings_for_cell_fields,
    tokenize_keywords, MarkerDebugEntry, MarkerDictionary,
};
use crate::models::{
    current_timestamp, CellId, MarkerGenome, MarkerId, MemoryCell, MemoryKind, MemorySource,
    MemoryStatus, MemoryValue, PageId, RecallMode, SensitivityLevel, TrustLevel,
};
use crate::packet::{ContextDebugInfo, ContextMemoryItem, ContextPacket, ContextScoreDebugItem};
use crate::pages::{
    attach_page_checksum, build_pages_with_kind, decode_page_with, encode_page_with,
    page_checksum_matches, page_file_name, MemoryPage, PageBuildOptions, PageCatalog,
    PageCatalogEntry, PageClustererKind, PageCodecKind,
};
use crate::retrieval::{
    full_scope_cell_debug_with_filter, score_cell_debug_with_cached_context,
    score_cell_debug_with_context, score_permitted_cell_debug_with_cached_context,
    score_permitted_cell_debug_with_context, CachedCellScoringData, RecallFilterContext,
    RecallRequest, Retriever, ScoringContext,
};
use crate::security::{AuditEvent, AuditLogger, NoSecurity, NoopAuditLogger, SecurityProvider};

pub const DEFAULT_STORE_DIR: &str = ".memory-genome";
const MANIFEST_FILE: &str = "manifest.mgm";
const MARKER_DICTIONARY_FILE: &str = "markers.mgd";
const HOT_LOG_FILE: &str = "hot.mgl";
const PAGE_CATALOG_FILE: &str = "page_index.mgi";
const EXACT_MARKER_INDEX_FILE: &str = "marker_index.mgi";
const BINARY_FUSE_INDEX_FILE: &str = "fuse_index.mgi";
const DECODED_PAGE_CACHE_CAPACITY: usize = 64;

pub trait Store {
    fn remember(&mut self, request: RememberRequest) -> Result<MemoryCell>;
    fn recall(&self, request: RecallRequest) -> Result<ContextPacket>;
    fn seal(&mut self) -> Result<SealReport>;
    fn stats(&self) -> Result<StoreStats>;
}

#[derive(Debug)]
pub struct MemoryEngine {
    root: PathBuf,
    manifest: Manifest,
    dictionary: MarkerDictionary,
    hot: HotMemoryLayer,
    pending_hot_cells: Vec<MemoryCell>,
    hot_metadata_dirty: bool,
    hot_unsynced_events: u64,
    last_hot_sync: Instant,
    page_cache: RefCell<DecodedPageCache>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DurabilityPolicy {
    Fast,
    Balanced,
    Safe,
}

impl DurabilityPolicy {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Fast => "fast",
            Self::Balanced => "balanced",
            Self::Safe => "safe",
        }
    }
}

impl Default for DurabilityPolicy {
    fn default() -> Self {
        Self::Balanced
    }
}

impl fmt::Display for DurabilityPolicy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for DurabilityPolicy {
    type Err = MgeError;

    fn from_str(input: &str) -> std::result::Result<Self, Self::Err> {
        match input.trim().to_ascii_lowercase().as_str() {
            "fast" => Ok(Self::Fast),
            "balanced" | "default" => Ok(Self::Balanced),
            "safe" => Ok(Self::Safe),
            other => Err(MgeError::InvalidInput(format!(
                "unknown durability policy: {other}; supported: fast, balanced, safe"
            ))),
        }
    }
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
    #[serde(default)]
    pub durability: DurabilityPolicy,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct InitOptions {
    pub page_codec: PageCodecKind,
    pub compression: CompressionKind,
    pub index_kind: IndexKind,
    pub page_clusterer: PageClustererKind,
    pub durability: DurabilityPolicy,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct StorageConfig {
    pub page_codec: PageCodecKind,
    pub compression: CompressionKind,
    pub index_kind: IndexKind,
    pub page_clusterer: PageClustererKind,
    pub durability: DurabilityPolicy,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct StorageConfigUpdate {
    pub page_codec: Option<PageCodecKind>,
    pub compression: Option<CompressionKind>,
    pub index_kind: Option<IndexKind>,
    pub page_clusterer: Option<PageClustererKind>,
    pub durability: Option<DurabilityPolicy>,
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
pub struct HotCheckpointReport {
    pub hot_cells: usize,
    pub snapshot_path: PathBuf,
    pub hot_log_offset: u64,
    pub durability: DurabilityPolicy,
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
    pub current_durability: DurabilityPolicy,
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
current durability: {}
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
            self.current_durability,
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

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RebuildIndexesReport {
    pub index_kind: IndexKind,
    pub pages_scanned: usize,
    pub catalog_entries_written: usize,
    pub exact_index_written: bool,
    pub binary_fuse_index_written: bool,
    pub active_index_file: String,
    pub pages_unchanged: bool,
    pub hot_cells_unchanged: usize,
}

impl RebuildIndexesReport {
    pub fn to_human_text(&self) -> String {
        format!(
            "\
rebuild indexes: ok
index kind: {}
pages scanned: {}
catalog entries written: {}
exact index written: {}
binary fuse index written: {}
active index file: {}
pages unchanged: {}
hot cells unchanged: {}
",
            self.index_kind,
            self.pages_scanned,
            self.catalog_entries_written,
            self.exact_index_written,
            self.binary_fuse_index_written,
            self.active_index_file,
            self.pages_unchanged,
            self.hot_cells_unchanged
        )
    }
}

#[derive(Clone, Debug)]
struct TimedPageRead {
    page: Arc<MemoryPage>,
    scoring: Option<Arc<PageScoringCache>>,
    file_read_micros: u64,
    decode_micros: u64,
    scoring_cache_build_micros: u64,
    decoded_page_cache_hit: bool,
    scoring_cache_hit: bool,
    scoring_cache_miss: bool,
}

#[derive(Debug)]
struct DecodedPageCache {
    capacity: usize,
    pages: BTreeMap<PageId, CachedDecodedPage>,
    order: VecDeque<PageId>,
}

#[derive(Clone, Debug)]
struct CachedDecodedPage {
    page: Arc<MemoryPage>,
    scoring: Option<Arc<PageScoringCache>>,
}

#[derive(Debug)]
struct PageScoringCache {
    cells: Vec<OnceLock<CachedCellScoringData>>,
}

impl PageScoringCache {
    fn for_page(page: &MemoryPage) -> Self {
        Self {
            cells: page.cells.iter().map(|_| OnceLock::new()).collect(),
        }
    }

    fn cell_with_timing(
        &self,
        cell_index: usize,
        cell: &MemoryCell,
    ) -> Option<(&CachedCellScoringData, u64)> {
        let slot = self.cells.get(cell_index)?;
        if let Some(cached) = slot.get() {
            return Some((cached, 0));
        }

        let started = Instant::now();
        let cached = slot.get_or_init(|| CachedCellScoringData::from_cell(cell));
        Some((cached, elapsed_micros(started)))
    }
}

impl DecodedPageCache {
    fn new(capacity: usize) -> Self {
        Self {
            capacity,
            pages: BTreeMap::new(),
            order: VecDeque::new(),
        }
    }

    fn get(&mut self, page_id: PageId) -> Option<CachedDecodedPage> {
        let page = self.pages.get(&page_id)?.clone();
        self.touch(page_id);
        Some(page)
    }

    fn insert(&mut self, page: Arc<MemoryPage>, scoring: Option<Arc<PageScoringCache>>) {
        if self.capacity == 0 {
            return;
        }

        let page_id = page.page_id;
        self.pages
            .insert(page_id, CachedDecodedPage { page, scoring });
        self.touch(page_id);

        while self.pages.len() > self.capacity {
            let Some(oldest) = self.order.pop_front() else {
                break;
            };
            self.pages.remove(&oldest);
        }
    }

    fn touch(&mut self, page_id: PageId) {
        self.order.retain(|existing| *existing != page_id);
        self.order.push_back(page_id);
    }

    fn set_scoring(&mut self, page_id: PageId, scoring: Arc<PageScoringCache>) {
        if let Some(page) = self.pages.get_mut(&page_id) {
            page.scoring = Some(scoring);
        }
    }
}

#[derive(Clone, Debug)]
struct RebuildPageRead {
    page: MemoryPage,
    entry: PageCatalogEntry,
}

#[derive(Clone, Debug)]
struct RankedCellHandle {
    source: RankedCellSource,
    cell_id: CellId,
    updated_at: i64,
    score: i64,
    score_detail: ContextScoreDebugItem,
}

#[derive(Clone, Copy, Debug)]
enum RankedCellSource {
    Hot(CellId),
    Sealed { page_id: PageId, cell_index: usize },
}

#[derive(Clone, Debug)]
struct PagePruneContext {
    query_marker_ids: Vec<MarkerId>,
    explicit_marker_ids: Vec<MarkerId>,
    required_page_marker_ids: Vec<MarkerId>,
    allowed_statuses: Vec<MemoryStatus>,
    all_status_marker_ids: Vec<MarkerId>,
    allowed_status_marker_ids: Vec<MarkerId>,
    allowed_sensitivities: Vec<SensitivityLevel>,
    all_sensitivity_marker_ids: Vec<MarkerId>,
    allowed_sensitivity_marker_ids: Vec<MarkerId>,
}

impl PagePruneContext {
    fn new(
        dictionary: &MarkerDictionary,
        query_marker_ids: &[MarkerId],
        explicit_marker_ids: &[MarkerId],
        required_page_marker_ids: &[MarkerId],
        policy: &crate::security::RecallPolicy,
    ) -> Self {
        let all_statuses = [
            MemoryStatus::Active,
            MemoryStatus::Temporary,
            MemoryStatus::Deprecated,
            MemoryStatus::Rejected,
            MemoryStatus::Superseded,
            MemoryStatus::Unverified,
            MemoryStatus::Verified,
        ];
        let mut allowed_statuses = vec![
            MemoryStatus::Active,
            MemoryStatus::Temporary,
            MemoryStatus::Unverified,
            MemoryStatus::Verified,
        ];
        if policy.include_deprecated {
            allowed_statuses.push(MemoryStatus::Deprecated);
            allowed_statuses.push(MemoryStatus::Superseded);
        }
        if policy.include_rejected {
            allowed_statuses.push(MemoryStatus::Rejected);
        }

        let all_sensitivities = [
            SensitivityLevel::Public,
            SensitivityLevel::Private,
            SensitivityLevel::Confidential,
            SensitivityLevel::SecretReference,
        ];
        let mut allowed_sensitivities = vec![
            SensitivityLevel::Public,
            SensitivityLevel::Private,
            SensitivityLevel::Confidential,
        ];
        if policy.allow_secret_references {
            allowed_sensitivities.push(SensitivityLevel::SecretReference);
        }

        Self {
            query_marker_ids: query_marker_ids.to_vec(),
            explicit_marker_ids: explicit_marker_ids.to_vec(),
            required_page_marker_ids: required_page_marker_ids.to_vec(),
            allowed_statuses: allowed_statuses.clone(),
            all_status_marker_ids: status_marker_ids(dictionary, &all_statuses),
            allowed_status_marker_ids: status_marker_ids(dictionary, &allowed_statuses),
            allowed_sensitivities: allowed_sensitivities.clone(),
            all_sensitivity_marker_ids: sensitivity_marker_ids(dictionary, &all_sensitivities),
            allowed_sensitivity_marker_ids: sensitivity_marker_ids(
                dictionary,
                &allowed_sensitivities,
            ),
        }
    }
}

impl MemoryEngine {
    pub fn init_at(store_root: impl AsRef<Path>) -> Result<Self> {
        Self::init_with_options(store_root, InitOptions::default())
    }

    pub fn init_with_options(store_root: impl AsRef<Path>, options: InitOptions) -> Result<Self> {
        ensure_runtime_page_codec(options.page_codec)?;

        let root = store_root.as_ref().to_path_buf();
        fs::create_dir_all(root.join("dictionary"))?;
        fs::create_dir_all(root.join("hot"))?;
        fs::create_dir_all(root.join("pages"))?;
        fs::create_dir_all(root.join("indexes"))?;
        fs::create_dir_all(root.join("exports"))?;

        let manifest_path = root.join(MANIFEST_FILE);
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
                durability: options.durability,
            };
            binary::write_messagepack_file(&manifest_path, FileKind::Manifest, &manifest)?;
        }

        let markers_path = root.join("dictionary").join(MARKER_DICTIONARY_FILE);
        let dictionary = MarkerDictionary::load_from_path(&markers_path)?;
        dictionary.save_to_path(&markers_path)?;

        HotStore::new(root.join("hot").join(HOT_LOG_FILE)).ensure_exists()?;
        if !root.join("indexes").join(PAGE_CATALOG_FILE).exists() {
            binary::write_messagepack_file(
                root.join("indexes").join(PAGE_CATALOG_FILE),
                FileKind::PageIndex,
                &PageCatalog::default(),
            )?;
        }
        if !root.join("indexes").join(EXACT_MARKER_INDEX_FILE).exists() {
            ExactMarkerPageIndex::default()
                .save_to_path(root.join("indexes").join(EXACT_MARKER_INDEX_FILE))?;
        }
        if !root.join("indexes").join(BINARY_FUSE_INDEX_FILE).exists() {
            BinaryFusePageIndex::default()
                .save_to_path(root.join("indexes").join(BINARY_FUSE_INDEX_FILE))?;
        }

        Self::open_at(root)
    }

    pub fn open_at(store_root: impl AsRef<Path>) -> Result<Self> {
        let root = store_root.as_ref().to_path_buf();
        let manifest_path = root.join(MANIFEST_FILE);
        if !manifest_path.exists() {
            return Err(MgeError::NotInitialized(root.display().to_string()));
        }

        let mut manifest: Manifest =
            binary::read_messagepack_file(&manifest_path, FileKind::Manifest)?;
        ensure_runtime_page_codec(manifest.page_codec)?;
        let dictionary =
            MarkerDictionary::load_from_path(root.join("dictionary").join(MARKER_DICTIONARY_FILE))?;
        let hot_store = HotStore::new(root.join("hot").join(HOT_LOG_FILE));
        let hot_recovery = hot_store.load_recovering()?;
        if hot_recovery.recovered_bad_tail {
            hot_store.truncate_to_valid_offset(hot_recovery.valid_log_offset)?;
        }
        if let Some(next_cell_id) = hot_recovery
            .cells
            .iter()
            .map(|cell| cell.id.saturating_add(1))
            .max()
        {
            manifest.next_cell_id = manifest.next_cell_id.max(next_cell_id);
        }
        let hot = HotMemoryLayer::from_cells(hot_recovery.cells);

        Ok(Self {
            root,
            manifest,
            dictionary,
            hot,
            pending_hot_cells: Vec::new(),
            hot_metadata_dirty: false,
            hot_unsynced_events: 0,
            last_hot_sync: Instant::now(),
            page_cache: RefCell::new(DecodedPageCache::new(DECODED_PAGE_CACHE_CAPACITY)),
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
            durability: self.manifest.durability,
        }
    }

    pub fn update_storage_config(
        &mut self,
        update: StorageConfigUpdate,
    ) -> Result<StorageConfigUpdateReport> {
        if let Some(page_codec) = update.page_codec {
            ensure_runtime_page_codec(page_codec)?;
        }
        if update.page_codec.is_none()
            && update.compression.is_none()
            && update.index_kind.is_none()
            && update.page_clusterer.is_none()
            && update.durability.is_none()
        {
            return Err(MgeError::InvalidInput(
                "storage config update requires page_codec, compression, index_kind, page_clusterer, or durability"
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
        if let Some(durability) = update.durability {
            self.manifest.durability = durability;
        }

        let current = self.storage_config();
        let changed = previous != current;
        if changed {
            self.manifest.updated_at = current_timestamp();
            self.save_manifest()?;
        }

        if previous.index_kind != current.index_kind {
            let pages = self.load_all_pages()?;
            self.rebuild_candidate_indexes_for_pages(&pages)?;
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

        let explicit_marker_strings = request
            .markers
            .iter()
            .map(|marker| canonicalize_marker(marker))
            .collect::<Result<Vec<_>>>()?;
        let mut marker_pairs = Vec::with_capacity(marker_strings.len());
        for marker in marker_strings {
            let marker_id = self.dictionary.get_or_insert(&marker)?;
            marker_pairs.push((marker, marker_id));
        }
        let marker_genome =
            MarkerGenome::from_canonical_markers(marker_pairs, &explicit_marker_strings);

        let cell_id = self.manifest.next_cell_id;
        self.manifest.next_cell_id = self
            .manifest
            .next_cell_id
            .checked_add(1)
            .ok_or_else(|| MgeError::InvalidInput("cell id overflow".to_string()))?;

        let cell = MemoryCell::new_with_marker_genome(
            cell_id,
            request.kind,
            request.subject,
            request.value,
            request.scope,
            request.status,
            request.trust,
            request.sensitivity,
            marker_genome,
            request.source,
            request.links,
        );

        self.hot.insert(cell.clone());
        self.pending_hot_cells.push(cell.clone());
        self.hot_metadata_dirty = true;
        self.manifest.updated_at = current_timestamp();

        Ok(cell)
    }

    pub fn recall(&self, request: RecallRequest) -> Result<ContextPacket> {
        let total_recall_started = Instant::now();
        if request.mode == RecallMode::FullScope && request.scope.is_none() {
            return Err(MgeError::InvalidInput(
                "full-scope recall requires an explicit scope (--scope <scope>)".to_string(),
            ));
        }

        let query_marker_started = Instant::now();
        let mut marker_strings = extract_query_marker_strings(&request.query);
        let mut required_page_marker_strings = Vec::new();
        let mut explicit_marker_strings = Vec::new();
        let mut scope_marker_string = None;
        for explicit in &request.markers {
            let marker = canonicalize_marker(explicit)?;
            explicit_marker_strings.push(marker.clone());
            marker_strings.push(marker);
        }
        if let Some(scope) = &request.scope {
            let marker = canonicalize_marker(&format!("scope:{scope}"))?;
            scope_marker_string = Some(marker.clone());
            required_page_marker_strings.push(marker.clone());
            marker_strings.push(marker);
        }
        if let Some(kind) = request.kind {
            let marker = canonicalize_marker(&format!("kind:{}", kind.as_str()))?;
            required_page_marker_strings.push(marker.clone());
            marker_strings.push(marker);
        }
        marker_strings.sort();
        marker_strings.dedup();

        let query_marker_ids = marker_strings
            .iter()
            .filter_map(|marker| self.dictionary.lookup(marker))
            .collect::<Vec<_>>();
        let required_page_marker_ids = required_page_marker_strings
            .iter()
            .filter_map(|marker| self.dictionary.lookup(marker))
            .collect::<Vec<_>>();
        let explicit_marker_ids = explicit_marker_strings
            .iter()
            .filter_map(|marker| self.dictionary.lookup(marker))
            .collect::<Vec<_>>();
        let scope_marker_id = scope_marker_string
            .as_ref()
            .and_then(|marker| self.dictionary.lookup(marker));
        let query_tokens = tokenize_keywords(&request.query);
        let filter_context = RecallFilterContext::new_with_marker_filters(
            &request,
            scope_marker_id,
            explicit_marker_ids.clone(),
        );
        let scoring_context = ScoringContext::new_with_filter(
            &request,
            filter_context.clone(),
            &query_marker_ids,
            &query_tokens,
        );
        let page_prune_context = PagePruneContext::new(
            &self.dictionary,
            &query_marker_ids,
            &explicit_marker_ids,
            &required_page_marker_ids,
            &request.effective_policy(),
        );
        let query_marker_extraction_micros = elapsed_micros(query_marker_started);

        let hot_memory_started = Instant::now();
        let hot_query_mode = match request.mode {
            RecallMode::Focused => QueryMode::PreferIntersection,
            RecallMode::Broad | RecallMode::FullScope => QueryMode::Union,
        };
        let allowed_hot_statuses = allowed_statuses_for_policy(&request.effective_policy());
        let hot_candidate_ids = self.hot.candidate_ids(HotCandidateQuery {
            marker_ids: &query_marker_ids,
            marker_mode: hot_query_mode,
            scope: request.scope.as_deref(),
            kind: request.kind,
            allowed_statuses: &allowed_hot_statuses,
        });
        let hot_total_cells = self.hot.len();
        let hot_candidate_cells = hot_candidate_ids.len();
        let hot_memory_lookup_micros = elapsed_micros(hot_memory_started);

        let mut ranked = Vec::new();
        let mut cells_evaluated = 0usize;
        let filtering_started = Instant::now();
        for cell_id in hot_candidate_ids {
            let Some(cell) = self.hot.cell(cell_id) else {
                continue;
            };
            if let Some(score_detail) = match request.mode {
                RecallMode::FullScope => full_scope_cell_debug_with_filter(cell, &filter_context),
                RecallMode::Focused | RecallMode::Broad => {
                    if let Some(cached) = self.hot.scoring(cell_id) {
                        score_cell_debug_with_cached_context(cell, &scoring_context, cached)
                    } else {
                        score_cell_debug_with_context(cell, &scoring_context)
                    }
                }
            } {
                ranked.push(RankedCellHandle {
                    source: RankedCellSource::Hot(cell.id),
                    cell_id: cell.id,
                    updated_at: cell.updated_at,
                    score: score_detail.score,
                    score_detail,
                });
            }
            cells_evaluated += 1;
        }
        let mut cell_filtering_micros = elapsed_micros(filtering_started);

        let catalog = if self.manifest.next_page_id == 1 {
            PageCatalog::default()
        } else {
            self.load_page_catalog()?
        };

        let candidate_page_index_started = Instant::now();
        let candidate_query = if query_marker_ids.is_empty() || catalog.pages.is_empty() {
            Default::default()
        } else {
            let query_mode = match request.mode {
                RecallMode::Focused => QueryMode::PreferIntersection,
                RecallMode::Broad | RecallMode::FullScope => QueryMode::Union,
            };
            let index = self.load_candidate_index()?;
            index.query_with_mode_stats(&query_marker_ids, query_mode)?
        };
        let candidate_page_index_lookup_micros = elapsed_micros(candidate_page_index_started);
        let candidate_pages = candidate_query.page_ids;

        let entries_by_id = catalog
            .pages
            .iter()
            .map(|entry| (entry.page_id, entry))
            .collect::<BTreeMap<_, _>>();

        let mut sealed_cells_scanned = 0;
        let mut cells_decoded = 0;
        let mut loaded_pages = 0;
        let mut pruned_candidate_pages = 0;
        let mut false_positive_candidate_pages = 0;
        let mut page_file_read_load_micros = 0u64;
        let mut page_decode_micros = 0u64;
        let mut scoring_cache_build_micros = 0u64;
        let mut decoded_page_cache_hits = 0usize;
        let mut decoded_page_cache_misses = 0usize;
        let mut scoring_cache_hits = 0usize;
        let mut scoring_cache_misses = 0usize;
        let mut loaded_pages_by_id = BTreeMap::new();
        let include_scoring_cache = !matches!(request.mode, RecallMode::FullScope);
        for page_id in &candidate_pages {
            let Some(entry) = entries_by_id.get(page_id) else {
                continue;
            };
            if should_prune_candidate_page(entry, &page_prune_context) {
                pruned_candidate_pages += 1;
                continue;
            }

            let timed_page = self.read_page_with_timing_cached(entry, include_scoring_cache)?;
            page_file_read_load_micros =
                page_file_read_load_micros.saturating_add(timed_page.file_read_micros);
            page_decode_micros = page_decode_micros.saturating_add(timed_page.decode_micros);
            scoring_cache_build_micros =
                scoring_cache_build_micros.saturating_add(timed_page.scoring_cache_build_micros);
            if timed_page.decoded_page_cache_hit {
                decoded_page_cache_hits += 1;
            } else {
                decoded_page_cache_misses += 1;
            }
            if timed_page.scoring_cache_hit {
                scoring_cache_hits += 1;
            }
            if timed_page.scoring_cache_miss {
                scoring_cache_misses += 1;
            }
            let page = timed_page.page;
            let scoring_cache = timed_page.scoring;
            loaded_pages += 1;
            sealed_cells_scanned += page.cells.len();
            cells_decoded += page.cells.len();
            let before_page_candidates = ranked.len();
            let filtering_started = Instant::now();
            for (cell_index, cell) in page.cells.iter().enumerate() {
                if let Some(score_detail) = match request.mode {
                    RecallMode::FullScope => {
                        full_scope_cell_debug_with_filter(cell, &filter_context)
                    }
                    RecallMode::Focused | RecallMode::Broad => {
                        if !scoring_context.permits_cell(cell) {
                            None
                        } else if let Some((cached, build_micros)) = scoring_cache
                            .as_ref()
                            .and_then(|cache| cache.cell_with_timing(cell_index, cell))
                        {
                            scoring_cache_build_micros =
                                scoring_cache_build_micros.saturating_add(build_micros);
                            score_permitted_cell_debug_with_cached_context(
                                cell,
                                &scoring_context,
                                cached,
                            )
                        } else {
                            score_permitted_cell_debug_with_context(cell, &scoring_context)
                        }
                    }
                } {
                    ranked.push(RankedCellHandle {
                        source: RankedCellSource::Sealed {
                            page_id: page.page_id,
                            cell_index,
                        },
                        cell_id: cell.id,
                        updated_at: cell.updated_at,
                        score: score_detail.score,
                        score_detail,
                    });
                }
                cells_evaluated += 1;
            }
            loaded_pages_by_id.insert(page.page_id, page);
            cell_filtering_micros =
                cell_filtering_micros.saturating_add(elapsed_micros(filtering_started));
            if ranked.len() == before_page_candidates {
                false_positive_candidate_pages += 1;
            }
        }

        let cells_ranked = ranked.len();
        let cells_scanned = hot_candidate_cells + sealed_cells_scanned;
        let cells_filtered = cells_evaluated.saturating_sub(cells_ranked);

        let reranking_started = Instant::now();
        ranked.sort_by(|left, right| {
            right
                .score
                .cmp(&left.score)
                .then_with(|| right.updated_at.cmp(&left.updated_at))
                .then_with(|| left.cell_id.cmp(&right.cell_id))
        });
        let reranking_micros = elapsed_micros(reranking_started);

        let max_items = request.effective_max_items(ranked.len());
        let debug = ContextDebugInfo {
            recall_mode: request.mode,
            max_items,
            index_kind: self.manifest.index_kind,
            hot_total_cells,
            hot_candidate_cells,
            hot_cells_scanned: hot_candidate_cells,
            cells_scanned,
            candidate_pages,
            pages_considered: candidate_query.candidate_pages_returned,
            page_filters_scanned: candidate_query.page_filters_scanned,
            candidate_pages_returned: candidate_query.candidate_pages_returned,
            loaded_pages,
            pruned_candidate_pages,
            pages_pruned_by_metadata: pruned_candidate_pages,
            sealed_cells_scanned,
            cells_decoded,
            cells_filtered,
            cells_ranked,
            false_positive_candidate_pages,
            total_candidates: ranked.len(),
            returned_items: 0,
            full_scope_used: request.mode == RecallMode::FullScope,
            query_marker_extraction_micros,
            hot_memory_lookup_micros,
            candidate_page_index_lookup_micros,
            page_file_read_load_micros,
            page_decode_micros,
            scoring_cache_build_micros,
            cell_filtering_micros,
            reranking_micros,
            context_packet_build_micros: 0,
            total_recall_micros: 0,
            decoded_page_cache_hits,
            decoded_page_cache_misses,
            scoring_cache_hits,
            scoring_cache_misses,
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

        let context_packet_started = Instant::now();
        let mut packet = self.build_context_packet_from_ranked_handles(
            request.query,
            &ranked,
            &loaded_pages_by_id,
            debug,
            max_items,
        )?;
        packet.debug.context_packet_build_micros = elapsed_micros(context_packet_started);
        packet.debug.total_recall_micros = elapsed_micros(total_recall_started);

        Ok(packet)
    }

    fn build_context_packet_from_ranked_handles(
        &self,
        query: String,
        ranked: &[RankedCellHandle],
        sealed_pages: &BTreeMap<PageId, Arc<MemoryPage>>,
        debug: ContextDebugInfo,
        max_items: usize,
    ) -> Result<ContextPacket> {
        let mut seen_cell_ids = BTreeSet::new();
        let mut total_candidates = 0;
        let mut relevant_memory = Vec::with_capacity(max_items.min(ranked.len()));
        let mut score_details = Vec::with_capacity(max_items.min(ranked.len()));

        for ranked in ranked {
            if !seen_cell_ids.insert(ranked.cell_id) {
                continue;
            }
            total_candidates += 1;

            if relevant_memory.len() < max_items {
                let cell = self.resolve_ranked_cell_handle(ranked, sealed_pages)?;
                let mut seen_marker_ids = BTreeSet::new();
                let mut markers = Vec::new();
                cell.for_each_marker_id_for_indexing(|marker_id| {
                    if seen_marker_ids.insert(marker_id) {
                        if let Some(marker) = self.dictionary.marker(marker_id) {
                            markers.push(marker.to_string());
                        }
                    }
                });

                relevant_memory.push(ContextMemoryItem {
                    kind: cell.kind,
                    content: cell.value.to_plain_text(),
                    trust: cell.trust,
                    status: cell.status,
                    scope: cell.scope.clone(),
                    sensitivity: cell.sensitivity,
                    markers,
                });
                score_details.push(ranked.score_detail.clone());
            }
        }

        let includes_deprecated_or_rejected = relevant_memory.iter().any(|item| {
            matches!(
                item.status,
                MemoryStatus::Deprecated | MemoryStatus::Rejected | MemoryStatus::Superseded
            )
        });
        let includes_secret_references = relevant_memory
            .iter()
            .any(|item| item.sensitivity == SensitivityLevel::SecretReference);

        let mut constraints = Vec::new();
        let mut warnings = Vec::new();
        if relevant_memory.is_empty() {
            warnings.push("No relevant memory matched the query.".to_string());
        }
        if includes_deprecated_or_rejected {
            warnings.push(
                "Deprecated, rejected, or superseded memories were included by explicit policy."
                    .to_string(),
            );
        } else {
            constraints
                .push("Do not use deprecated, rejected, or superseded memories.".to_string());
        }
        if includes_secret_references {
            warnings.push("SecretReference cells were included by explicit policy.".to_string());
        } else {
            constraints.push("Do not expose secret_reference cells.".to_string());
        }
        let returned_items = relevant_memory.len();

        Ok(ContextPacket {
            query,
            relevant_memory,
            constraints,
            warnings,
            debug: ContextDebugInfo {
                total_candidates,
                returned_items,
                score_details,
                ..debug
            },
        })
    }

    fn resolve_ranked_cell_handle<'a>(
        &'a self,
        ranked: &RankedCellHandle,
        sealed_pages: &'a BTreeMap<PageId, Arc<MemoryPage>>,
    ) -> Result<&'a MemoryCell> {
        match ranked.source {
            RankedCellSource::Hot(cell_id) => self.hot.cell(cell_id).ok_or_else(|| {
                MgeError::InvalidInput(format!("ranked hot cell {cell_id} is missing"))
            }),
            RankedCellSource::Sealed {
                page_id,
                cell_index,
            } => {
                let page = sealed_pages.get(&page_id).ok_or_else(|| {
                    MgeError::InvalidInput(format!("ranked sealed page {page_id} is missing"))
                })?;
                let cell = page.cells.get(cell_index).ok_or_else(|| {
                    MgeError::InvalidInput(format!(
                        "ranked sealed cell index {cell_index} is missing from page {page_id}"
                    ))
                })?;
                if cell.id != ranked.cell_id {
                    return Err(MgeError::InvalidInput(format!(
                        "ranked sealed cell id mismatch on page {page_id}: expected {}, found {}",
                        ranked.cell_id, cell.id
                    )));
                }
                Ok(cell)
            }
        }
    }

    pub fn seal(&mut self) -> Result<SealReport> {
        let hot_store = HotStore::new(self.hot_cells_path());
        self.flush_pending_hot(true)?;
        let hot_cells = self.hot.all_cells();
        if hot_cells.is_empty() {
            return Ok(SealReport {
                hot_cells_sealed: 0,
                pages_written: 0,
                archived_hot_log: None,
            });
        }

        for cell in &hot_cells {
            let mut invalid_marker = None;
            cell.for_each_marker_id_for_indexing(|marker| {
                if self.dictionary.marker(marker).is_none() {
                    invalid_marker = Some(marker);
                }
            });
            if let Some(marker) = invalid_marker {
                return Err(MgeError::InvalidInput(format!(
                    "cell {} references unknown marker {}",
                    cell.id, marker
                )));
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
            let encoded_size_bytes = self.write_page(page)?;
            catalog
                .pages
                .push(self.page_catalog_entry_for_page(page, encoded_size_bytes)?);
            self.manifest.next_page_id = self.manifest.next_page_id.max(page.page_id + 1);
        }
        self.save_page_catalog(&catalog)?;

        let all_pages = self.load_all_pages()?;
        self.rebuild_candidate_indexes_for_pages(&all_pages)?;

        let archived_hot_log = hot_store.archive_and_clear()?;
        self.hot.clear();
        self.manifest.last_seal_time = Some(current_timestamp());
        self.manifest.updated_at = current_timestamp();
        self.save_manifest()?;

        Ok(SealReport {
            hot_cells_sealed: hot_cells.len(),
            pages_written: pages.len(),
            archived_hot_log,
        })
    }

    pub fn checkpoint(&mut self) -> Result<HotCheckpointReport> {
        self.flush_pending_hot(true)?;
        let hot_store = HotStore::new(self.hot_cells_path());
        let cells = self.hot.all_cells();
        let snapshot = hot_store.write_snapshot(&cells)?;
        self.hot_unsynced_events = 0;
        self.last_hot_sync = Instant::now();

        Ok(HotCheckpointReport {
            hot_cells: cells.len(),
            snapshot_path: self.hot_snapshot_path(),
            hot_log_offset: snapshot.hot_log_offset,
            durability: self.manifest.durability,
        })
    }

    pub fn stats(&self) -> Result<StoreStats> {
        let hot_cells = self.hot.len();
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
            current_durability: self.manifest.durability,
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

    pub fn rebuild_catalog_and_indexes(&self) -> Result<RebuildIndexesReport> {
        let hot_cells_unchanged = self.hot.len();
        let mut reads = self.read_all_page_files_for_rebuild()?;
        reads.sort_by_key(|read| (read.page.page_id, read.entry.file.clone()));

        let mut seen_page_ids = BTreeSet::new();
        for read in &reads {
            if !seen_page_ids.insert(read.page.page_id) {
                return Err(MgeError::InvalidInput(format!(
                    "duplicate sealed page id {} while rebuilding catalog/indexes",
                    read.page.page_id
                )));
            }
        }

        let pages = reads
            .iter()
            .map(|read| read.page.clone())
            .collect::<Vec<_>>();
        let catalog = PageCatalog {
            index_kind: self.manifest.index_kind,
            pages: reads.into_iter().map(|read| read.entry).collect(),
        };

        self.save_page_catalog(&catalog)?;
        let active_index = self.rebuild_candidate_indexes_for_pages(&pages)?;

        Ok(RebuildIndexesReport {
            index_kind: self.manifest.index_kind,
            pages_scanned: pages.len(),
            catalog_entries_written: catalog.pages.len(),
            exact_index_written: true,
            binary_fuse_index_written: self.manifest.index_kind == IndexKind::BinaryFusePage,
            active_index_file: self
                .candidate_index_file_name(active_index.kind())
                .to_string(),
            pages_unchanged: true,
            hot_cells_unchanged,
        })
    }

    pub fn export_json(&self) -> Result<serde_json::Value> {
        let hot_cells = self.hot.all_cells();
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

    pub fn export_markdown(&self) -> Result<String> {
        let hot_cells = self.hot.all_cells();
        let catalog = self.load_page_catalog()?;
        let pages = self.load_all_pages()?;

        let mut output = String::new();
        output.push_str("# Memory Genome Export\n\n");
        output.push_str("## Store\n\n");
        output.push_str(&format!("- version: {}\n", self.manifest.version));
        output.push_str(&format!("- markers: {}\n", self.dictionary.len()));
        output.push_str(&format!("- hot cells: {}\n", hot_cells.len()));
        output.push_str(&format!("- sealed pages: {}\n", catalog.pages.len()));
        output.push_str(&format!("- index kind: {}\n\n", self.manifest.index_kind));

        output.push_str("## Hot Memory\n\n");
        if hot_cells.is_empty() {
            output.push_str("_No hot cells._\n\n");
        } else {
            for cell in &hot_cells {
                append_cell_markdown(&mut output, cell, &self.dictionary);
            }
        }

        output.push_str("## Sealed Pages\n\n");
        if pages.is_empty() {
            output.push_str("_No sealed pages._\n");
        } else {
            for page in &pages {
                output.push_str(&format!(
                    "### Page {}\n\n- cells: {}\n- markers: {}\n\n",
                    page.page_id,
                    page.cells.len(),
                    page.marker_summary.len()
                ));
                for cell in &page.cells {
                    append_cell_markdown(&mut output, cell, &self.dictionary);
                }
            }
        }

        Ok(output)
    }

    pub fn export_markdown_to_default_path(&self) -> Result<PathBuf> {
        let path = self.export_markdown_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&path, self.export_markdown()?)?;
        Ok(path)
    }

    pub fn validate(&self) -> Result<ValidationReport> {
        self.validate_with_options(false)
    }

    pub fn validate_deep(&self) -> Result<ValidationReport> {
        self.validate_with_options(true)
    }

    fn validate_with_options(&self, deep: bool) -> Result<ValidationReport> {
        let mut report = ValidationReport::new(self.manifest.index_kind);
        let page_catalog_path = self.page_catalog_path();
        if !page_catalog_path.exists() {
            let message = format!("page catalog file missing: {}", page_catalog_path.display());
            if deep {
                report.error(message);
            } else {
                report.warning(message);
            }
        }

        let catalog = match self.load_page_catalog() {
            Ok(catalog) => catalog,
            Err(err) => {
                report.error(format!("page catalog load failed: {err}"));
                PageCatalog::default()
            }
        };

        for error in self.dictionary.consistency_errors() {
            report.error(format!("marker dictionary inconsistency: {error}"));
        }

        if catalog.index_kind != self.manifest.index_kind {
            report.error(format!(
                "page catalog index kind {} does not match manifest index kind {}",
                catalog.index_kind, self.manifest.index_kind
            ));
        }

        let active_index_path = self.candidate_index_path(self.manifest.index_kind);
        let index = if !active_index_path.exists() {
            report.error(format!(
                "active candidate index file missing: {}",
                active_index_path.display()
            ));
            None
        } else {
            match self.load_candidate_index() {
                Ok(index) => Some(index),
                Err(err) => {
                    report.error(format!("candidate index load failed: {err}"));
                    None
                }
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

        let mut cell_ids = BTreeSet::new();
        let mut cell_links = Vec::new();
        if let Err(err) = HotStore::new(self.hot_cells_path()).load_cells() {
            report.error(format!("hot memory load failed: {err}"));
        }
        let hot_cells = self.hot.all_cells();
        report.checked_hot_cells = hot_cells.len();
        let mut max_cell_id = 0;
        for cell in &hot_cells {
            max_cell_id = max_cell_id.max(cell.id);
            if !cell_ids.insert(cell.id) {
                report.error(format!("duplicate cell id {}", cell.id));
            }
            cell_links.push(("hot cell".to_string(), cell.id, cell.links.clone()));
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
            if entry.encoded_size_bytes > 0 {
                match fs::metadata(&page_path) {
                    Ok(metadata) if metadata.len() != entry.encoded_size_bytes => {
                        report.error(format!(
                            "catalog page {} encoded_size_bytes {} does not match file size {}",
                            entry.page_id,
                            entry.encoded_size_bytes,
                            metadata.len()
                        ));
                    }
                    Ok(_) => {}
                    Err(err) => report.error(format!(
                        "page file metadata failed for page {}: {err}",
                        entry.page_id
                    )),
                }
            }

            match self.read_page(entry) {
                Ok(page) => {
                    max_cell_id =
                        max_cell_id.max(page.cells.iter().map(|cell| cell.id).max().unwrap_or(0));
                    for cell in &page.cells {
                        if !cell_ids.insert(cell.id) {
                            report.error(format!("duplicate cell id {}", cell.id));
                        }
                        cell_links.push(("sealed cell".to_string(), cell.id, cell.links.clone()));
                    }
                    self.validate_page(entry, &page, &mut report);
                }
                Err(err) => {
                    report.error(format!("failed to read page {}: {err}", entry.page_id));
                }
            }
        }

        self.validate_orphan_storage_files(&page_files, deep, &mut report)?;

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
        validate_cell_links(&cell_ids, &cell_links, &mut report);

        if catalog.pages.is_empty() && hot_cells.is_empty() {
            report.warning("store contains no hot or sealed cells");
        }

        Ok(report)
    }

    fn validate_orphan_storage_files(
        &self,
        catalog_page_files: &BTreeSet<String>,
        deep: bool,
        report: &mut ValidationReport,
    ) -> Result<()> {
        let pages_dir = self.pages_dir();
        if pages_dir.exists() {
            for entry in fs::read_dir(pages_dir)? {
                let entry = entry?;
                if !entry.file_type()?.is_file() {
                    continue;
                }
                let file_name = entry.file_name().to_string_lossy().to_string();
                if !catalog_page_files.contains(&file_name) {
                    let message =
                        format!("orphan page file not referenced by catalog: {file_name}");
                    if deep && file_name.ends_with(".mgp") {
                        report.error(message);
                    } else {
                        report.warning(message);
                    }
                }
            }
        }

        let indexes_dir = self.indexes_dir();
        if indexes_dir.exists() {
            for entry in fs::read_dir(indexes_dir)? {
                let entry = entry?;
                if !entry.file_type()?.is_file() {
                    continue;
                }
                let file_name = entry.file_name().to_string_lossy().to_string();
                if !is_known_index_file(&file_name) {
                    report.warning(format!(
                        "unknown index file not managed by store: {file_name}"
                    ));
                }
            }
        }

        Ok(())
    }

    fn flush_pending_hot(&mut self, force_sync: bool) -> Result<()> {
        if self.pending_hot_cells.is_empty() && !self.hot_metadata_dirty {
            if force_sync {
                HotStore::new(self.hot_cells_path()).sync()?;
            }
            return Ok(());
        }

        self.save_manifest()?;
        self.dictionary.save_to_path(self.markers_path())?;

        let hot_store = HotStore::new(self.hot_cells_path());
        let pending = std::mem::take(&mut self.pending_hot_cells);
        for (index, cell) in pending.iter().enumerate() {
            let sync_each_record = self.manifest.durability == DurabilityPolicy::Safe;
            if let Err(err) = hot_store.append_cell(cell, sync_each_record) {
                self.pending_hot_cells = pending[index..].to_vec();
                return Err(err);
            }
        }

        let should_sync = force_sync
            || matches!(
                self.manifest.durability,
                DurabilityPolicy::Balanced | DurabilityPolicy::Safe
            );
        if should_sync {
            hot_store.sync()?;
            self.hot_unsynced_events = 0;
            self.last_hot_sync = Instant::now();
        } else {
            self.hot_unsynced_events = self
                .hot_unsynced_events
                .saturating_add(u64::try_from(pending.len()).unwrap_or(u64::MAX));
        }
        self.hot_metadata_dirty = false;
        Ok(())
    }

    fn save_manifest(&self) -> Result<()> {
        binary::write_messagepack_file(self.manifest_path(), FileKind::Manifest, &self.manifest)
    }

    fn load_page_catalog(&self) -> Result<PageCatalog> {
        let path = self.page_catalog_path();
        if !path.exists() {
            return Ok(PageCatalog::default());
        }
        binary::read_messagepack_file(path, FileKind::PageIndex)
    }

    fn save_page_catalog(&self, catalog: &PageCatalog) -> Result<()> {
        let mut catalog = catalog.clone();
        catalog.index_kind = self.manifest.index_kind;
        binary::write_messagepack_file(self.page_catalog_path(), FileKind::PageIndex, &catalog)
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
        Ok((*self.read_page_with_timing(entry)?.page).clone())
    }

    fn read_page_with_timing(&self, entry: &PageCatalogEntry) -> Result<TimedPageRead> {
        let file_read_started = Instant::now();
        let bytes = fs::read(self.pages_dir().join(&entry.file))?;
        let file_read_micros = elapsed_micros(file_read_started);

        let decode_started = Instant::now();
        let frame = binary::decode_frame(&bytes, FileKind::Page)?;
        let expected_codec = page_storage_codec(entry.page_codec, entry.compression)?;
        if frame.codec != expected_codec {
            return Err(MgeError::StorageFormat(format!(
                "wrong codec for page {}: expected {}, found {}",
                entry.page_id,
                expected_codec.as_str(),
                frame.codec.as_str()
            )));
        }
        let security = NoSecurity;
        let opened = security.open_page_bytes(&frame.payload)?;
        let decoded = decompress_with(entry.compression, &opened)?;
        let page = decode_page_with(entry.page_codec, &decoded)?;
        let decode_micros = elapsed_micros(decode_started);

        Ok(TimedPageRead {
            page: Arc::new(page),
            scoring: None,
            file_read_micros,
            decode_micros,
            scoring_cache_build_micros: 0,
            decoded_page_cache_hit: false,
            scoring_cache_hit: false,
            scoring_cache_miss: false,
        })
    }

    fn read_page_with_timing_cached(
        &self,
        entry: &PageCatalogEntry,
        include_scoring: bool,
    ) -> Result<TimedPageRead> {
        let cached_page = { self.page_cache.borrow_mut().get(entry.page_id) };
        if let Some(cached) = cached_page {
            let (scoring, scoring_cache_build_micros, scoring_cache_hit, scoring_cache_miss) =
                if include_scoring {
                    match cached.scoring {
                        Some(scoring) => (Some(scoring), 0, true, false),
                        None => {
                            let scoring = Arc::new(PageScoringCache::for_page(&cached.page));
                            self.page_cache
                                .borrow_mut()
                                .set_scoring(entry.page_id, Arc::clone(&scoring));
                            (Some(scoring), 0, false, true)
                        }
                    }
                } else {
                    (None, 0, false, false)
                };
            return Ok(TimedPageRead {
                page: cached.page,
                scoring,
                file_read_micros: 0,
                decode_micros: 0,
                scoring_cache_build_micros,
                decoded_page_cache_hit: true,
                scoring_cache_hit,
                scoring_cache_miss,
            });
        }

        let mut timed_page = self.read_page_with_timing(entry)?;
        if include_scoring {
            timed_page.scoring = Some(Arc::new(PageScoringCache::for_page(&timed_page.page)));
        }
        timed_page.scoring_cache_miss = include_scoring;
        self.page_cache
            .borrow_mut()
            .insert(Arc::clone(&timed_page.page), timed_page.scoring.clone());
        Ok(timed_page)
    }

    fn write_page(&self, page: &MemoryPage) -> Result<u64> {
        let security = NoSecurity;

        let encoded = encode_page_with(self.manifest.page_codec, page)?;
        let compressed = compress_with(self.manifest.compression, &encoded)?;
        let stored = security.seal_page_bytes(&compressed)?;
        let stored = binary::encode_frame(
            FileKind::Page,
            page_storage_codec(self.manifest.page_codec, self.manifest.compression)?,
            &stored,
        )?;
        // Future order remains: encode page -> compress page -> encrypt page -> write page.
        let encoded_size_bytes = u64::try_from(stored.len())
            .map_err(|_| MgeError::InvalidInput("page frame size overflow".to_string()))?;
        binary::atomic_write_bytes(self.pages_dir().join(page_file_name(page.page_id)), &stored)?;
        Ok(encoded_size_bytes)
    }

    fn read_all_page_files_for_rebuild(&self) -> Result<Vec<RebuildPageRead>> {
        let pages_dir = self.pages_dir();
        if !pages_dir.exists() {
            return Ok(Vec::new());
        }

        let mut reads = Vec::new();
        for entry in fs::read_dir(pages_dir)? {
            let entry = entry?;
            if !entry.file_type()?.is_file() {
                continue;
            }
            let file_name = entry.file_name().to_string_lossy().to_string();
            if !file_name.ends_with(".mgp") {
                continue;
            }

            let read = self.read_page_file_for_rebuild(&file_name).map_err(|err| {
                MgeError::StorageFormat(format!(
                    "failed to rebuild from page file {file_name}: {err}"
                ))
            })?;
            reads.push(read);
        }

        Ok(reads)
    }

    fn read_page_file_for_rebuild(&self, file_name: &str) -> Result<RebuildPageRead> {
        let path = self.pages_dir().join(file_name);
        let bytes = fs::read(&path)?;
        let encoded_size_bytes = u64::try_from(bytes.len())
            .map_err(|_| MgeError::InvalidInput("page frame size overflow".to_string()))?;
        let frame = binary::decode_frame(&bytes, FileKind::Page)?;
        let (page_codec, compression) = page_storage_details_from_codec(frame.codec)?;
        let security = NoSecurity;
        let opened = security.open_page_bytes(&frame.payload)?;
        let decoded = decompress_with(compression, &opened)?;
        let page = decode_page_with(page_codec, &decoded)?;
        let entry = self.page_catalog_entry_for_existing_page(
            &page,
            file_name.to_string(),
            page_codec,
            compression,
            encoded_size_bytes,
        )?;

        Ok(RebuildPageRead { page, entry })
    }

    fn page_catalog_entry_for_page(
        &self,
        page: &MemoryPage,
        encoded_size_bytes: u64,
    ) -> Result<PageCatalogEntry> {
        self.page_catalog_entry_for_existing_page(
            page,
            page_file_name(page.page_id),
            self.manifest.page_codec,
            self.manifest.compression,
            encoded_size_bytes,
        )
    }

    fn page_catalog_entry_for_existing_page(
        &self,
        page: &MemoryPage,
        file: String,
        page_codec: PageCodecKind,
        compression: CompressionKind,
        encoded_size_bytes: u64,
    ) -> Result<PageCatalogEntry> {
        Ok(PageCatalogEntry {
            page_id: page.page_id,
            file,
            page_codec,
            compression,
            page_clusterer: self.manifest.page_clusterer,
            created_at: page.created_at,
            cell_count: page.cell_count,
            marker_summary: page.marker_summary.clone(),
            scope_marker_summary: category_marker_summary(
                &page.cells,
                |cell| cell.marker_genome.scope_marker(),
                |cell| canonicalize_marker(&format!("scope:{}", cell.scope)),
                &self.dictionary,
            )?,
            kind_marker_summary: category_marker_summary(
                &page.cells,
                |cell| cell.marker_genome.kind_marker(),
                |cell| canonicalize_marker(&format!("kind:{}", cell.kind.as_str())),
                &self.dictionary,
            )?,
            status_summary: enum_summary(page.cells.iter().map(|cell| cell.status)),
            sensitivity_summary: enum_summary(page.cells.iter().map(|cell| cell.sensitivity)),
            trust_summary: enum_summary(page.cells.iter().map(|cell| cell.trust)),
            encoded_size_bytes,
        })
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

    fn rebuild_candidate_indexes_for_pages(
        &self,
        pages: &[MemoryPage],
    ) -> Result<CandidateIndexData> {
        let exact = ExactMarkerPageIndex::build(pages)?;
        exact.save_to_path(self.marker_index_path())?;

        match self.manifest.index_kind {
            IndexKind::ExactMarkerPage => Ok(CandidateIndexData::ExactMarkerPage(exact)),
            IndexKind::BinaryFusePage => {
                let binary = BinaryFusePageIndex::build(pages)?;
                binary.save_to_path(self.binary_fuse_index_path())?;
                Ok(CandidateIndexData::BinaryFusePage(binary))
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
        match category_marker_summary(
            &page.cells,
            |cell| cell.marker_genome.scope_marker(),
            |cell| canonicalize_marker(&format!("scope:{}", cell.scope)),
            &self.dictionary,
        ) {
            Ok(expected) => validate_optional_catalog_summary(
                "scope_marker_summary",
                entry.page_id,
                &entry.scope_marker_summary,
                &expected,
                report,
            ),
            Err(err) => report.error(format!(
                "page {} scope marker summary validation failed: {err}",
                entry.page_id
            )),
        }
        match category_marker_summary(
            &page.cells,
            |cell| cell.marker_genome.kind_marker(),
            |cell| canonicalize_marker(&format!("kind:{}", cell.kind.as_str())),
            &self.dictionary,
        ) {
            Ok(expected) => validate_optional_catalog_summary(
                "kind_marker_summary",
                entry.page_id,
                &entry.kind_marker_summary,
                &expected,
                report,
            ),
            Err(err) => report.error(format!(
                "page {} kind marker summary validation failed: {err}",
                entry.page_id
            )),
        }
        validate_optional_catalog_summary(
            "status_summary",
            entry.page_id,
            &entry.status_summary,
            &enum_summary(page.cells.iter().map(|cell| cell.status)),
            report,
        );
        validate_optional_catalog_summary(
            "sensitivity_summary",
            entry.page_id,
            &entry.sensitivity_summary,
            &enum_summary(page.cells.iter().map(|cell| cell.sensitivity)),
            report,
        );
        validate_optional_catalog_summary(
            "trust_summary",
            entry.page_id,
            &entry.trust_summary,
            &enum_summary(page.cells.iter().map(|cell| cell.trust)),
            report,
        );

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
        cell.for_each_marker_id_for_indexing(|marker| {
            if self.dictionary.marker(marker).is_none() {
                report.error(format!(
                    "{label} {} references unknown marker {}",
                    cell.id, marker
                ));
            }
        });
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
        self.root.join(MANIFEST_FILE)
    }

    fn markers_path(&self) -> PathBuf {
        self.root.join("dictionary").join(MARKER_DICTIONARY_FILE)
    }

    fn hot_cells_path(&self) -> PathBuf {
        self.root.join("hot").join(HOT_LOG_FILE)
    }

    fn hot_snapshot_path(&self) -> PathBuf {
        self.root.join("hot").join("snapshot.mgs")
    }

    fn pages_dir(&self) -> PathBuf {
        self.root.join("pages")
    }

    fn page_catalog_path(&self) -> PathBuf {
        self.root.join("indexes").join(PAGE_CATALOG_FILE)
    }

    fn indexes_dir(&self) -> PathBuf {
        self.root.join("indexes")
    }

    fn export_markdown_path(&self) -> PathBuf {
        self.root.join("exports").join("memory.md")
    }

    fn marker_index_path(&self) -> PathBuf {
        self.root.join("indexes").join(EXACT_MARKER_INDEX_FILE)
    }

    fn binary_fuse_index_path(&self) -> PathBuf {
        self.root.join("indexes").join(BINARY_FUSE_INDEX_FILE)
    }

    fn candidate_index_path(&self, kind: IndexKind) -> PathBuf {
        match kind {
            IndexKind::ExactMarkerPage => self.marker_index_path(),
            IndexKind::BinaryFusePage => self.binary_fuse_index_path(),
        }
    }

    fn candidate_index_file_name(&self, kind: IndexKind) -> &'static str {
        match kind {
            IndexKind::ExactMarkerPage => EXACT_MARKER_INDEX_FILE,
            IndexKind::BinaryFusePage => BINARY_FUSE_INDEX_FILE,
        }
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

impl Drop for MemoryEngine {
    fn drop(&mut self) {
        let force_sync = matches!(
            self.manifest.durability,
            DurabilityPolicy::Balanced | DurabilityPolicy::Safe
        );
        let _ = self.flush_pending_hot(force_sync);
    }
}

fn append_cell_markdown(output: &mut String, cell: &MemoryCell, dictionary: &MarkerDictionary) {
    output.push_str(&format!("#### Cell {}\n\n", cell.id));
    output.push_str(&format!("- kind: {}\n", cell.kind));
    output.push_str(&format!("- scope: {}\n", cell.scope));
    output.push_str(&format!("- status: {}\n", cell.status));
    output.push_str(&format!("- trust: {}\n", cell.trust));
    output.push_str(&format!("- sensitivity: {}\n", cell.sensitivity));
    if let Some(subject) = &cell.subject {
        output.push_str(&format!("- subject: {}\n", subject));
    }
    let mut seen_marker_ids = BTreeSet::new();
    let mut markers = Vec::new();
    cell.for_each_marker_id_for_indexing(|marker_id| {
        if seen_marker_ids.insert(marker_id) {
            if let Some(marker) = dictionary.marker(marker_id) {
                markers.push(marker);
            }
        }
    });
    if !markers.is_empty() {
        output.push_str(&format!("- markers: `{}`\n", markers.join("`, `")));
    }
    output.push_str("\n");
    output.push_str(&cell.value.to_plain_text());
    output.push_str("\n\n");
}

fn page_storage_codec(page_codec: PageCodecKind, compression: CompressionKind) -> Result<CodecId> {
    match (page_codec, compression) {
        (PageCodecKind::MessagePack, CompressionKind::None) => Ok(CodecId::MessagePack),
        (PageCodecKind::MessagePack, CompressionKind::Zstd) => Ok(CodecId::MessagePackZstd),
        (PageCodecKind::Json, _) => Err(MgeError::InvalidInput(
            "json page codec is only allowed for optional debug/export paths, not runtime storage"
                .to_string(),
        )),
    }
}

fn page_storage_details_from_codec(codec: CodecId) -> Result<(PageCodecKind, CompressionKind)> {
    match codec {
        CodecId::MessagePack => Ok((PageCodecKind::MessagePack, CompressionKind::None)),
        CodecId::MessagePackZstd => Ok((PageCodecKind::MessagePack, CompressionKind::Zstd)),
        CodecId::None => Err(MgeError::StorageFormat(
            "page frame codec none is not valid for runtime page storage".to_string(),
        )),
    }
}

fn ensure_runtime_page_codec(page_codec: PageCodecKind) -> Result<()> {
    if page_codec == PageCodecKind::Json {
        return Err(MgeError::InvalidInput(
            "json page codec is only allowed for optional debug/export paths, not runtime storage"
                .to_string(),
        ));
    }
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

fn validate_cell_links(
    cell_ids: &BTreeSet<CellId>,
    cell_links: &[(String, CellId, Vec<CellId>)],
    report: &mut ValidationReport,
) {
    for (label, cell_id, links) in cell_links {
        let mut seen_links = BTreeSet::new();
        for link in links {
            if *link == *cell_id {
                report.error(format!("{label} {cell_id} links to itself"));
            }
            if !seen_links.insert(*link) {
                report.warning(format!("{label} {cell_id} repeats link {link}"));
            }
            if !cell_ids.contains(link) {
                report.error(format!("{label} {cell_id} links to unknown cell {link}"));
            }
        }
    }
}

fn validate_optional_catalog_summary<T>(
    label: &str,
    page_id: PageId,
    actual: &[T],
    expected: &[T],
    report: &mut ValidationReport,
) where
    T: Eq + std::fmt::Debug,
{
    if !actual.is_empty() && actual != expected {
        report.error(format!(
            "catalog {label} differs from page cells for page {page_id}: expected {expected:?}, found {actual:?}"
        ));
    }
}

fn should_prune_candidate_page(entry: &PageCatalogEntry, context: &PagePruneContext) -> bool {
    for required_marker in &context.required_page_marker_ids {
        if !entry.marker_summary.contains(required_marker) {
            return true;
        }
    }

    if !context.explicit_marker_ids.is_empty()
        && !entry
            .marker_summary
            .iter()
            .any(|marker| context.explicit_marker_ids.contains(marker))
    {
        return true;
    }

    if !entry.status_summary.is_empty()
        && entry
            .status_summary
            .iter()
            .all(|status| !context.allowed_statuses.contains(status))
    {
        return true;
    }

    if entry.status_summary.is_empty()
        && summary_has_known_without_allowed(
            &entry.marker_summary,
            &context.all_status_marker_ids,
            &context.allowed_status_marker_ids,
        )
    {
        return true;
    }

    if !entry.sensitivity_summary.is_empty()
        && entry
            .sensitivity_summary
            .iter()
            .all(|sensitivity| !context.allowed_sensitivities.contains(sensitivity))
    {
        return true;
    }

    if entry.sensitivity_summary.is_empty()
        && summary_has_known_without_allowed(
            &entry.marker_summary,
            &context.all_sensitivity_marker_ids,
            &context.allowed_sensitivity_marker_ids,
        )
    {
        return true;
    }

    !context.query_marker_ids.is_empty()
        && !entry
            .marker_summary
            .iter()
            .any(|marker| context.query_marker_ids.contains(marker))
}

fn summary_has_known_without_allowed(
    marker_summary: &[MarkerId],
    known_marker_ids: &[MarkerId],
    allowed_marker_ids: &[MarkerId],
) -> bool {
    marker_summary
        .iter()
        .any(|marker| known_marker_ids.contains(marker))
        && !marker_summary
            .iter()
            .any(|marker| allowed_marker_ids.contains(marker))
}

fn status_marker_ids(dictionary: &MarkerDictionary, statuses: &[MemoryStatus]) -> Vec<MarkerId> {
    statuses
        .iter()
        .filter_map(|status| dictionary.lookup(&format!("status:{}", status.as_str())))
        .collect()
}

fn sensitivity_marker_ids(
    dictionary: &MarkerDictionary,
    sensitivities: &[SensitivityLevel],
) -> Vec<MarkerId> {
    sensitivities
        .iter()
        .filter_map(|sensitivity| {
            dictionary.lookup(&format!("sensitivity:{}", sensitivity.as_str()))
        })
        .collect()
}

fn category_marker_summary<F, G>(
    cells: &[MemoryCell],
    mut genome_marker_for_cell: F,
    mut marker_for_cell: G,
    dictionary: &MarkerDictionary,
) -> Result<Vec<MarkerId>>
where
    F: FnMut(&MemoryCell) -> Option<MarkerId>,
    G: FnMut(&MemoryCell) -> Result<String>,
{
    let mut summary = BTreeSet::new();
    for cell in cells {
        if let Some(marker_id) = genome_marker_for_cell(cell) {
            summary.insert(marker_id);
        } else if let Some(marker_id) = dictionary.lookup(&marker_for_cell(cell)?) {
            summary.insert(marker_id);
        }
    }
    Ok(summary.into_iter().collect())
}

fn enum_summary<T: Copy + Ord>(values: impl IntoIterator<Item = T>) -> Vec<T> {
    values
        .into_iter()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn elapsed_micros(started: Instant) -> u64 {
    u64::try_from(started.elapsed().as_micros()).unwrap_or(u64::MAX)
}

fn is_known_index_file(file_name: &str) -> bool {
    matches!(
        file_name,
        PAGE_CATALOG_FILE | EXACT_MARKER_INDEX_FILE | BINARY_FUSE_INDEX_FILE
    )
}

fn marker_summary_for_cells(cells: &[MemoryCell]) -> Vec<MarkerId> {
    MarkerGenome::marker_summary(cells)
}
