pub mod compression;
pub mod errors;
pub mod hot;
pub mod indexes;
pub mod markers;
pub mod models;
pub mod packet;
pub mod pages;
pub mod retrieval;
pub mod security;
pub mod store;

pub use compression::{CompressionKind, Compressor, NoCompression, ZstdCompression};
pub use errors::{MgeError, Result};
pub use indexes::{
    BinaryFusePageFilter, BinaryFusePageIndex, CandidateIndexData, CandidatePageIndex,
    CandidatePageQueryResult, ExactMarkerPageIndex, IndexKind, QueryMode,
};
pub use markers::{
    canonicalize_marker, canonicalize_marker_value, extract_query_marker_strings,
    marker_strings_for_cell_fields, tokenize_keywords, MarkerDebugEntry, MarkerDictionary,
};
pub use models::{
    CellId, MemoryCell, MemoryKind, MemorySource, MemoryStatus, MemoryValue, PageId,
    SensitivityLevel, TrustLevel,
};
pub use packet::{ContextDebugInfo, ContextMemoryItem, ContextPacket};
pub use pages::{
    build_pages_from_cells, build_pages_with_clusterer, build_pages_with_kind, JsonPageCodec,
    MarkerOverlapClusterer, MemoryPage, MessagePackPageCodec, PageBuildOptions, PageCatalog,
    PageCatalogEntry, PageClusterer, PageClustererKind, PageCodec, PageCodecKind,
    ScopeKindClusterer, DEFAULT_MAX_CELLS_PER_PAGE, DEFAULT_TARGET_PAGE_BYTES,
};
pub use retrieval::{build_context_packet, score_cell, score_cell_debug, RecallRequest, Retriever};
pub use security::{
    AgentCapabilities, AgentCapability, AuditEvent, AuditLogger, NoSecurity, NoopAuditLogger,
    RecallPolicy, SecurityProvider,
};
pub use store::{
    InitOptions, InspectReport, MemoryEngine, RememberRequest, SealReport, StorageConfig,
    StorageConfigUpdate, StorageConfigUpdateReport, Store, StoreStats, DEFAULT_STORE_DIR,
};
