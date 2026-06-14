use std::collections::BTreeMap;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;

use mge_core::binary::{self, CodecId, FileKind};
use mge_core::{
    build_context_packet, build_pages_from_cells, build_pages_with_clusterer, canonicalize_marker,
    marker_strings_for_cell_fields, score_cell_debug, tokenize_keywords, AgentCapabilities,
    AgentCapability, AuditEvent, AuditLogger, BinaryFusePageIndex, CandidateIndexData,
    CandidatePageIndex, CompressionKind, Compressor, ContextDebugInfo, DurabilityPolicy,
    ExactMarkerPageIndex, HotCandidateQuery, HotMemoryLayer, IndexKind, InitOptions, MarkerGenome,
    MarkerOverlapClusterer, MemoryEngine, MemoryKind, MemorySource, MemoryStatus, MemoryValue,
    MessagePackPageCodec, NoopAuditLogger, PageBuildOptions, PageCatalog, PageCatalogEntry,
    PageClustererKind, PageCodec, PageCodecKind, QueryMode, RecallMode, RecallPolicy,
    RecallRequest, RememberRequest, ScopeKindClusterer, SensitivityLevel, StorageConfigUpdate,
    TrustLevel, ZstdCompression,
};
use serde::Serialize;
use tempfile::tempdir;

#[test]
fn marker_canonicalization() {
    assert_eq!(
        canonicalize_marker("Kind: User Preference").unwrap(),
        "kind:user_preference"
    );
    assert_eq!(
        canonicalize_marker("answer style").unwrap(),
        "tag:answer_style"
    );
    assert_eq!(
        canonicalize_marker(" Scope: Rust APIs / Policies!! ").unwrap(),
        "scope:rust_apis_policies"
    );
}

#[test]
fn keyword_tokenization_normalizes_ascii_without_duplicates() {
    assert_eq!(
        tokenize_keywords("Rust APIs, APIs and policies with Tests"),
        vec!["rust", "api", "policy", "test"]
    );
}

#[test]
fn marker_dictionary_id_assignment_is_stable() {
    let mut dictionary = mge_core::MarkerDictionary::new();
    let first = dictionary.get_or_insert("kind:user_preference").unwrap();
    let second = dictionary.get_or_insert("Kind: User Preference").unwrap();
    let third = dictionary.get_or_insert("scope:global").unwrap();

    assert_eq!(first, second);
    assert_ne!(first, third);
    assert_eq!(dictionary.marker(first), Some("kind:user_preference"));
}

#[test]
fn zstd_compression_roundtrips_bytes() {
    let compressor = ZstdCompression::default();
    let input = b"marker genome page bytes marker genome page bytes marker genome page bytes";

    let compressed = compressor.compress(input).unwrap();
    let decompressed = compressor.decompress(&compressed).unwrap();

    assert_eq!(decompressed, input);
}

#[test]
fn memory_cell_creation() {
    let cell = mge_core::MemoryCell::new(
        1,
        MemoryKind::UserPreference,
        Some("answer_style".to_string()),
        MemoryValue::Symbol("concise_technical".to_string()),
        "global".to_string(),
        MemoryStatus::Active,
        TrustLevel::UserConfirmed,
        SensitivityLevel::Private,
        vec![1, 2, 3],
        None,
        Vec::new(),
    );

    assert_eq!(cell.id, 1);
    assert_eq!(cell.kind, MemoryKind::UserPreference);
    assert_eq!(cell.markers, vec![1, 2, 3]);
    assert_eq!(cell.marker_ids_for_indexing(), vec![1, 2, 3]);
    assert_eq!(cell.marker_genome.custom_marker_ids(), vec![1, 2, 3]);
}

#[test]
fn marker_genome_builds_from_existing_memory_input() {
    let explicit = vec![canonicalize_marker("tag:custom").unwrap()];
    let pairs = vec![
        ("scope:global".to_string(), 1),
        ("kind:user_preference".to_string(), 2),
        ("status:active".to_string(), 3),
        ("trust:user_confirmed".to_string(), 4),
        ("sensitivity:private".to_string(), 5),
        ("subject:answer_style".to_string(), 6),
        ("value:concise_technical".to_string(), 7),
        ("tag:custom".to_string(), 8),
        ("tag:technical".to_string(), 9),
    ];

    let genome = MarkerGenome::from_canonical_markers(pairs, &explicit);

    assert_eq!(genome.scope_marker(), Some(1));
    assert_eq!(genome.scope_marker_id(), Some(1));
    assert_eq!(genome.kind_marker(), Some(2));
    assert_eq!(genome.kind_marker_id(), Some(2));
    assert_eq!(genome.status_marker(), Some(3));
    assert_eq!(genome.status_marker_id(), Some(3));
    assert_eq!(genome.trust_marker(), Some(4));
    assert_eq!(genome.trust_marker_id(), Some(4));
    assert_eq!(genome.sensitivity_marker(), Some(5));
    assert_eq!(genome.sensitivity_marker_id(), Some(5));
    assert_eq!(genome.custom_marker_ids(), vec![8]);
    assert_eq!(genome.iter_custom_marker_ids().collect::<Vec<_>>(), vec![8]);
    assert_eq!(genome.system_marker_ids(), vec![1, 2, 3, 4, 5, 6, 7, 9]);
    assert_eq!(
        genome.iter_system_marker_ids().collect::<Vec<_>>(),
        vec![1, 2, 3, 4, 5, 6, 7, 9]
    );
    assert_eq!(genome.all_marker_ids(), vec![1, 2, 3, 4, 5, 6, 7, 8, 9]);
    assert_eq!(
        genome.iter_all_marker_ids().collect::<Vec<_>>(),
        vec![1, 2, 3, 4, 5, 6, 7, 9, 8]
    );
    assert_eq!(genome.fingerprint().len(), 64);
}

#[test]
fn marker_genome_separates_system_and_custom_markers_from_memory_input() {
    let marker_strings = marker_strings_for_cell_fields(
        &MemoryKind::UserPreference,
        Some("answer style"),
        &MemoryValue::Text("concise technical".to_string()),
        "global",
        &MemoryStatus::Active,
        &TrustLevel::UserConfirmed,
        &SensitivityLevel::Private,
        &["tag:custom".to_string()],
    )
    .unwrap();
    let mut dictionary = mge_core::MarkerDictionary::new();
    let pairs = marker_strings
        .iter()
        .map(|marker| Ok((marker.clone(), dictionary.get_or_insert(marker)?)))
        .collect::<mge_core::Result<Vec<_>>>()
        .unwrap();
    let explicit = vec![canonicalize_marker("tag:custom").unwrap()];
    let genome = MarkerGenome::from_canonical_markers(pairs, &explicit);
    let custom_id = dictionary.lookup("tag:custom").unwrap();

    assert!(genome.scope_marker().is_some());
    assert!(genome.kind_marker().is_some());
    assert!(genome.status_marker().is_some());
    assert!(genome.trust_marker().is_some());
    assert!(genome.sensitivity_marker().is_some());
    assert_eq!(genome.custom_marker_ids(), vec![custom_id]);
    assert!(genome.all_marker_ids().contains(&custom_id));
}

#[test]
fn old_vec_marker_cell_records_decode_without_marker_genome() {
    #[derive(Serialize)]
    struct OldMemoryCell {
        id: u64,
        kind: MemoryKind,
        subject: Option<String>,
        value: MemoryValue,
        scope: String,
        status: MemoryStatus,
        trust: TrustLevel,
        sensitivity: SensitivityLevel,
        created_at: i64,
        updated_at: i64,
        markers: Vec<u32>,
        source: Option<MemorySource>,
        links: Vec<u64>,
    }

    let old = OldMemoryCell {
        id: 77,
        kind: MemoryKind::ProjectFact,
        subject: Some("old style".to_string()),
        value: MemoryValue::Text("old vec marker cell".to_string()),
        scope: "compat".to_string(),
        status: MemoryStatus::Active,
        trust: TrustLevel::ToolObserved,
        sensitivity: SensitivityLevel::Public,
        created_at: 100,
        updated_at: 100,
        markers: vec![30, 10, 30],
        source: None,
        links: Vec::new(),
    };

    let bytes = rmp_serde::to_vec_named(&old).unwrap();
    let decoded: mge_core::MemoryCell = rmp_serde::from_slice(&bytes).unwrap();

    assert!(decoded.marker_genome.is_empty());
    assert_eq!(decoded.markers, vec![30, 10, 30]);
    assert_eq!(decoded.marker_ids_for_indexing(), vec![10, 30]);
    assert!(decoded.contains_marker(10));
    assert!(decoded.contains_marker(30));
}

#[test]
fn old_vec_marker_cells_remain_indexable_in_hot_ram() {
    let cell = mge_core::MemoryCell::new(
        77,
        MemoryKind::ProjectFact,
        None,
        MemoryValue::Text("old vec marker hot memory".to_string()),
        "compat".to_string(),
        MemoryStatus::Active,
        TrustLevel::ToolObserved,
        SensitivityLevel::Public,
        vec![30, 10, 30],
        None,
        Vec::new(),
    );
    let layer = HotMemoryLayer::from_cells(vec![cell]);
    let allowed_statuses = vec![MemoryStatus::Active];
    let candidates = layer.candidate_ids(HotCandidateQuery {
        marker_ids: &[10],
        marker_mode: QueryMode::Union,
        scope: None,
        kind: None,
        allowed_statuses: &allowed_statuses,
    });

    assert_eq!(candidates, vec![77]);
}

#[test]
fn automatic_marker_generation() {
    let markers = marker_strings_for_cell_fields(
        &MemoryKind::UserPreference,
        Some("answer style"),
        &MemoryValue::Symbol("concise technical".to_string()),
        "global",
        &MemoryStatus::Active,
        &TrustLevel::UserConfirmed,
        &SensitivityLevel::Private,
        &[],
    )
    .unwrap();

    assert!(markers.contains(&"kind:user_preference".to_string()));
    assert!(markers.contains(&"subject:answer_style".to_string()));
    assert!(markers.contains(&"value:concise_technical".to_string()));
    assert!(markers.contains(&"tag:technical".to_string()));
}

#[test]
fn structured_marker_generation_extracts_shallow_tags() {
    let markers = marker_strings_for_cell_fields(
        &MemoryKind::UserPreference,
        Some("answer style"),
        &MemoryValue::Structured(serde_json::json!({
            "style": "concise technical",
            "max_examples": 2,
            "nested": {
                "mode": "direct"
            }
        })),
        "global",
        &MemoryStatus::Active,
        &TrustLevel::UserConfirmed,
        &SensitivityLevel::Private,
        &[],
    )
    .unwrap();

    assert!(markers.contains(&"tag:style".to_string()));
    assert!(markers.contains(&"tag:concise".to_string()));
    assert!(markers.contains(&"tag:technical".to_string()));
    assert!(markers.contains(&"tag:max".to_string()));
    assert!(markers.contains(&"tag:example".to_string()));
    assert!(markers.contains(&"tag:nested".to_string()));
    assert!(markers.contains(&"tag:mode".to_string()));
}

#[test]
fn remember_into_hot_memory() {
    let dir = tempdir().unwrap();
    let mut engine = MemoryEngine::init_at(dir.path()).unwrap();
    remember_answer_style(&mut engine);

    let stats = engine.stats().unwrap();
    assert_eq!(stats.hot_cells, 1);
    assert_eq!(stats.sealed_pages, 0);
}

#[test]
fn init_options_are_saved_in_manifest() {
    let dir = tempdir().unwrap();
    let engine = MemoryEngine::init_with_options(
        dir.path(),
        InitOptions {
            page_codec: PageCodecKind::MessagePack,
            compression: CompressionKind::Zstd,
            index_kind: IndexKind::ExactMarkerPage,
            page_clusterer: PageClustererKind::ScopeKind,
            durability: DurabilityPolicy::Balanced,
        },
    )
    .unwrap();

    let inspect = engine.inspect().unwrap();

    assert_eq!(inspect.manifest.page_codec, PageCodecKind::MessagePack);
    assert_eq!(inspect.manifest.compression, CompressionKind::Zstd);
    assert_eq!(inspect.manifest.index_kind, IndexKind::ExactMarkerPage);
    assert_eq!(
        inspect.manifest.page_clusterer,
        PageClustererKind::ScopeKind
    );
}

#[test]
fn init_creates_binary_storage_layout() {
    let dir = tempdir().unwrap();
    MemoryEngine::init_at(dir.path()).unwrap();

    assert!(dir.path().join("manifest.mgm").is_file());
    assert!(dir.path().join("dictionary").join("markers.mgd").is_file());
    assert!(dir.path().join("hot").join("hot.mgl").is_file());
    assert!(dir.path().join("indexes").join("page_index.mgi").is_file());
    assert!(dir
        .path()
        .join("indexes")
        .join("marker_index.mgi")
        .is_file());
    assert!(dir.path().join("indexes").join("fuse_index.mgi").is_file());
    assert!(dir.path().join("exports").is_dir());

    assert!(!dir.path().join("manifest.json").exists());
    assert!(!dir.path().join("markers.json").exists());
    assert!(!dir.path().join("hot").join("hot_cells.jsonl").exists());
    assert!(!dir
        .path()
        .join("indexes")
        .join("page_catalog.json")
        .exists());
    assert!(!dir
        .path()
        .join("indexes")
        .join("marker_to_pages.json")
        .exists());
}

#[test]
fn recall_modes_do_not_create_json_runtime_storage() {
    let dir = tempdir().unwrap();
    let mut engine = MemoryEngine::init_at(dir.path()).unwrap();
    remember_text_cell(
        &mut engine,
        "runtime-check",
        MemoryStatus::Active,
        TrustLevel::ToolObserved,
        "runtime storage recall mode memory",
        &["tag:runtime".to_string()],
    );
    let checkpoint = engine.checkpoint().unwrap();
    assert_eq!(
        checkpoint
            .snapshot_path
            .file_name()
            .and_then(|name| name.to_str()),
        Some("snapshot.mgs")
    );
    engine.seal().unwrap();

    let focused = RecallRequest::new("runtime storage");
    engine.recall(focused).unwrap();
    let mut broad = RecallRequest::new("runtime storage");
    broad.mode = RecallMode::Broad;
    engine.recall(broad).unwrap();
    let mut full_scope = RecallRequest::new("");
    full_scope.mode = RecallMode::FullScope;
    full_scope.scope = Some("runtime-check".to_string());
    engine.recall(full_scope).unwrap();

    assert!(!dir.path().join("manifest.json").exists());
    assert!(!dir.path().join("markers.json").exists());
    assert!(!dir.path().join("hot").join("hot_cells.jsonl").exists());
    assert!(!dir.path().join("hot").join("snapshot.json").exists());
    assert!(!dir
        .path()
        .join("indexes")
        .join("page_catalog.json")
        .exists());
    assert!(!dir
        .path()
        .join("indexes")
        .join("marker_to_pages.json")
        .exists());
}

#[test]
fn init_rejects_json_runtime_page_codec() {
    let dir = tempdir().unwrap();
    let err = MemoryEngine::init_with_options(
        dir.path(),
        InitOptions {
            page_codec: PageCodecKind::Json,
            compression: CompressionKind::None,
            index_kind: IndexKind::ExactMarkerPage,
            page_clusterer: PageClustererKind::ScopeKind,
            durability: DurabilityPolicy::Balanced,
        },
    )
    .unwrap_err();

    assert!(err.to_string().contains("json page codec"));
}

#[test]
fn storage_config_update_changes_future_defaults() {
    let dir = tempdir().unwrap();
    let mut engine = MemoryEngine::init_at(dir.path()).unwrap();

    let report = engine
        .update_storage_config(StorageConfigUpdate {
            page_codec: Some(PageCodecKind::MessagePack),
            compression: Some(CompressionKind::Zstd),
            index_kind: None,
            page_clusterer: Some(PageClustererKind::MarkerOverlap),
            durability: None,
        })
        .unwrap();

    assert_eq!(report.previous.page_codec, PageCodecKind::MessagePack);
    assert_eq!(report.previous.compression, CompressionKind::None);
    assert_eq!(report.current.page_codec, PageCodecKind::MessagePack);
    assert_eq!(report.current.compression, CompressionKind::Zstd);
    assert_eq!(
        report.current.page_clusterer,
        PageClustererKind::MarkerOverlap
    );
    assert!(report.changed);

    let inspect = engine.inspect().unwrap();
    assert_eq!(inspect.manifest.page_codec, PageCodecKind::MessagePack);
    assert_eq!(inspect.manifest.compression, CompressionKind::Zstd);
    assert_eq!(
        inspect.manifest.page_clusterer,
        PageClustererKind::MarkerOverlap
    );
}

#[test]
fn recall_from_hot_memory() {
    let dir = tempdir().unwrap();
    let mut engine = MemoryEngine::init_at(dir.path()).unwrap();
    remember_answer_style(&mut engine);

    let packet = engine
        .recall(RecallRequest::new(
            "How should the agent answer technical questions?",
        ))
        .unwrap();

    assert_eq!(packet.relevant_memory.len(), 1);
    assert!(packet.relevant_memory[0]
        .content
        .contains("concise technical"));
    assert_eq!(packet.debug.hot_cells_scanned, 1);
    assert_eq!(packet.debug.score_details.len(), 1);
    assert_eq!(packet.debug.score_details[0].trust_bonus, 5);
    assert_eq!(packet.debug.score_details[0].status_bonus, 5);
}

#[test]
fn hot_ram_layer_indexes_candidates_and_recovers_from_log() {
    let dir = tempdir().unwrap();
    let mut engine = MemoryEngine::init_at(dir.path()).unwrap();
    remember_text_cell(
        &mut engine,
        "hot-ram",
        MemoryStatus::Active,
        TrustLevel::UserConfirmed,
        "alpha hot memory",
        &["tag:alpha".to_string()],
    );
    remember_text_cell(
        &mut engine,
        "hot-ram",
        MemoryStatus::Active,
        TrustLevel::UserConfirmed,
        "beta hot memory",
        &["tag:beta".to_string()],
    );
    remember_text_cell(
        &mut engine,
        "hot-ram",
        MemoryStatus::Active,
        TrustLevel::UserConfirmed,
        "gamma hot memory",
        &["tag:gamma".to_string()],
    );

    let mut request = RecallRequest::new("alpha");
    request.markers = vec!["tag:alpha".to_string()];
    let packet = engine.recall(request.clone()).unwrap();
    assert_eq!(packet.relevant_memory.len(), 1);
    assert_eq!(packet.debug.hot_total_cells, 3);
    assert_eq!(packet.debug.hot_candidate_cells, 1);
    assert_eq!(packet.debug.hot_cells_scanned, 1);

    let checkpoint = engine.checkpoint().unwrap();
    assert_eq!(checkpoint.hot_cells, 3);
    assert!(checkpoint.snapshot_path.is_file());

    let reopened = MemoryEngine::open_at(dir.path()).unwrap();
    let reopened_packet = reopened.recall(request.clone()).unwrap();
    assert_eq!(reopened_packet.relevant_memory.len(), 1);
    assert_eq!(reopened_packet.debug.hot_total_cells, 3);
    assert_eq!(reopened_packet.debug.hot_candidate_cells, 1);

    engine.seal().unwrap();
    assert_eq!(engine.stats().unwrap().hot_cells, 0);
    assert!(!dir.path().join("hot").join("snapshot.mgs").exists());
    assert_eq!(
        mge_core::HotStore::new(dir.path().join("hot").join("hot.mgl"))
            .load_cells()
            .unwrap()
            .len(),
        0
    );
    let sealed_packet = engine.recall(request).unwrap();
    assert_eq!(sealed_packet.relevant_memory.len(), 1);
    assert_eq!(sealed_packet.debug.hot_total_cells, 0);
    assert_eq!(sealed_packet.debug.hot_candidate_cells, 0);
    assert_eq!(sealed_packet.debug.hot_cells_scanned, 0);
    assert_eq!(sealed_packet.debug.loaded_pages, 1);
    assert_eq!(sealed_packet.debug.cells_ranked, 1);
}

#[test]
fn remember_is_visible_immediately_before_hot_log_flush() {
    let dir = tempdir().unwrap();
    let mut engine = MemoryEngine::init_at(dir.path()).unwrap();
    remember_text_cell(
        &mut engine,
        "ram-first",
        MemoryStatus::Active,
        TrustLevel::UserConfirmed,
        "ram first hot memory",
        &["tag:ram_first".to_string()],
    );

    let mut request = RecallRequest::new("ram first");
    request.markers = vec!["tag:ram_first".to_string()];
    let packet = engine.recall(request).unwrap();

    assert_eq!(packet.relevant_memory.len(), 1);
    assert_eq!(packet.debug.hot_total_cells, 1);
    assert_eq!(
        mge_core::HotStore::new(dir.path().join("hot").join("hot.mgl"))
            .load_cells()
            .unwrap()
            .len(),
        0
    );
}

#[test]
fn corrupted_last_hot_frame_does_not_destroy_valid_hot_memory() {
    let dir = tempdir().unwrap();
    let mut engine = MemoryEngine::init_at(dir.path()).unwrap();
    remember_text_cell(
        &mut engine,
        "tail-recovery",
        MemoryStatus::Active,
        TrustLevel::UserConfirmed,
        "first durable hot memory",
        &["tag:tail".to_string()],
    );
    remember_text_cell(
        &mut engine,
        "tail-recovery",
        MemoryStatus::Active,
        TrustLevel::UserConfirmed,
        "second durable hot memory",
        &["tag:tail".to_string()],
    );
    engine.checkpoint().unwrap();

    let hot_path = dir.path().join("hot").join("hot.mgl");
    let valid_len = fs::metadata(&hot_path).unwrap().len();
    let mut file = OpenOptions::new().append(true).open(&hot_path).unwrap();
    file.write_all(b"truncated-final-frame").unwrap();
    file.flush().unwrap();
    assert!(fs::metadata(&hot_path).unwrap().len() > valid_len);

    drop(engine);
    let reopened = MemoryEngine::open_at(dir.path()).unwrap();
    assert_eq!(fs::metadata(&hot_path).unwrap().len(), valid_len);

    let mut request = RecallRequest::new("durable hot memory");
    request.markers = vec!["tag:tail".to_string()];
    request.mode = RecallMode::Broad;
    let packet = reopened.recall(request).unwrap();

    assert_eq!(packet.relevant_memory.len(), 2);
    assert_eq!(packet.debug.hot_total_cells, 2);
}

#[test]
fn safe_and_balanced_durability_flush_paths_restore_hot_memory() {
    for durability in [DurabilityPolicy::Safe, DurabilityPolicy::Balanced] {
        let dir = tempdir().unwrap();
        let mut engine = MemoryEngine::init_with_options(
            dir.path(),
            InitOptions {
                page_codec: PageCodecKind::MessagePack,
                compression: CompressionKind::None,
                index_kind: IndexKind::ExactMarkerPage,
                page_clusterer: PageClustererKind::ScopeKind,
                durability,
            },
        )
        .unwrap();
        remember_text_cell(
            &mut engine,
            "durability-mode",
            MemoryStatus::Active,
            TrustLevel::UserConfirmed,
            "durability mode hot memory",
            &["tag:durability".to_string()],
        );
        let checkpoint = engine.checkpoint().unwrap();
        assert_eq!(checkpoint.durability, durability);

        drop(engine);
        let reopened = MemoryEngine::open_at(dir.path()).unwrap();
        let mut request = RecallRequest::new("durability mode");
        request.markers = vec!["tag:durability".to_string()];
        let packet = reopened.recall(request).unwrap();

        assert_eq!(packet.relevant_memory.len(), 1);
        assert_eq!(packet.debug.hot_total_cells, 1);
    }
}

#[test]
fn checkpoint_snapshot_replays_hot_log_after_snapshot_offset() {
    let dir = tempdir().unwrap();
    let mut engine = MemoryEngine::init_at(dir.path()).unwrap();
    remember_text_cell(
        &mut engine,
        "checkpoint-replay",
        MemoryStatus::Active,
        TrustLevel::UserConfirmed,
        "snapshot hot memory",
        &["tag:checkpoint".to_string()],
    );
    let checkpoint = engine.checkpoint().unwrap();
    assert_eq!(checkpoint.hot_cells, 1);

    remember_text_cell(
        &mut engine,
        "checkpoint-replay",
        MemoryStatus::Active,
        TrustLevel::UserConfirmed,
        "replayed hot memory",
        &["tag:checkpoint".to_string()],
    );
    drop(engine);

    let reopened = MemoryEngine::open_at(dir.path()).unwrap();
    let mut request = RecallRequest::new("");
    request.mode = RecallMode::FullScope;
    request.scope = Some("checkpoint-replay".to_string());
    let packet = reopened.recall(request).unwrap();
    let contents = packet
        .relevant_memory
        .iter()
        .map(|item| item.content.as_str())
        .collect::<Vec<_>>();

    assert_eq!(packet.relevant_memory.len(), 2);
    assert!(contents.contains(&"snapshot hot memory"));
    assert!(contents.contains(&"replayed hot memory"));
}

#[test]
fn full_scope_recall_uses_hot_ram_and_sealed_pages_together() {
    let dir = tempdir().unwrap();
    let mut engine = MemoryEngine::init_at(dir.path()).unwrap();
    remember_text_cell(
        &mut engine,
        "hybrid-scope",
        MemoryStatus::Active,
        TrustLevel::UserConfirmed,
        "sealed hybrid memory",
        &["tag:hybrid".to_string()],
    );
    engine.seal().unwrap();
    remember_text_cell(
        &mut engine,
        "hybrid-scope",
        MemoryStatus::Active,
        TrustLevel::UserConfirmed,
        "hot hybrid memory",
        &["tag:hybrid".to_string()],
    );

    let mut request = RecallRequest::new("");
    request.mode = RecallMode::FullScope;
    request.scope = Some("hybrid-scope".to_string());
    let packet = engine.recall(request).unwrap();
    let contents = packet
        .relevant_memory
        .iter()
        .map(|item| item.content.as_str())
        .collect::<Vec<_>>();

    assert_eq!(packet.relevant_memory.len(), 2);
    assert!(contents.contains(&"sealed hybrid memory"));
    assert!(contents.contains(&"hot hybrid memory"));
    assert_eq!(packet.debug.hot_total_cells, 1);
    assert_eq!(packet.debug.hot_candidate_cells, 1);
    assert_eq!(packet.debug.loaded_pages, 1);
}

#[test]
fn hot_ram_status_index_excludes_deprecated_rejected_and_superseded_before_scoring() {
    let dir = tempdir().unwrap();
    let mut engine = MemoryEngine::init_at(dir.path()).unwrap();
    remember_text_cell(
        &mut engine,
        "hot-policy",
        MemoryStatus::Active,
        TrustLevel::UserConfirmed,
        "hot active style",
        &["tag:style".to_string()],
    );
    remember_text_cell(
        &mut engine,
        "hot-policy",
        MemoryStatus::Deprecated,
        TrustLevel::UserConfirmed,
        "hot deprecated style",
        &["tag:style".to_string()],
    );
    remember_text_cell(
        &mut engine,
        "hot-policy",
        MemoryStatus::Rejected,
        TrustLevel::UserConfirmed,
        "hot rejected style",
        &["tag:style".to_string()],
    );
    remember_text_cell(
        &mut engine,
        "hot-policy",
        MemoryStatus::Superseded,
        TrustLevel::UserConfirmed,
        "hot superseded style",
        &["tag:style".to_string()],
    );

    let packet = engine.recall(RecallRequest::new("style")).unwrap();

    assert_eq!(packet.relevant_memory.len(), 1);
    assert!(packet.relevant_memory[0].content.contains("active style"));
    assert_eq!(packet.debug.hot_total_cells, 4);
    assert_eq!(packet.debug.hot_candidate_cells, 1);
    assert_eq!(packet.debug.hot_cells_scanned, 1);
    assert_eq!(packet.debug.cells_ranked, 1);
}

#[test]
fn recall_from_structured_value_markers() {
    let dir = tempdir().unwrap();
    let mut engine = MemoryEngine::init_at(dir.path()).unwrap();
    let mut request = RememberRequest::new(
        MemoryKind::UserPreference,
        MemoryValue::Structured(serde_json::json!({
            "style": "concise technical",
            "max_examples": 2
        })),
    );
    request.subject = Some("answer style".to_string());
    request.scope = "global".to_string();
    request.trust = TrustLevel::UserConfirmed;
    engine.remember(request).unwrap();

    let packet = engine.recall(RecallRequest::new("concise style")).unwrap();

    assert_eq!(packet.relevant_memory.len(), 1);
    assert!(packet.relevant_memory[0]
        .markers
        .contains(&"tag:style".to_string()));
    assert!(packet.relevant_memory[0]
        .markers
        .contains(&"tag:concise".to_string()));
}

#[test]
fn focused_recall_returns_top_relevant_items() {
    let dir = tempdir().unwrap();
    let mut engine = MemoryEngine::init_at(dir.path()).unwrap();
    remember_text_cell(
        &mut engine,
        "global",
        MemoryStatus::Active,
        TrustLevel::ExternalUntrusted,
        "critical topic lower trust background memory",
        &["tag:critical".to_string(), "tag:topic".to_string()],
    );
    remember_text_cell(
        &mut engine,
        "global",
        MemoryStatus::Verified,
        TrustLevel::UserConfirmed,
        "critical topic high signal memory",
        &["tag:critical".to_string(), "tag:topic".to_string()],
    );

    let mut request = RecallRequest::new("critical topic");
    request.max_items = 1;
    let packet = engine.recall(request).unwrap();

    assert_eq!(packet.debug.recall_mode, RecallMode::Focused);
    assert_eq!(packet.debug.max_items, 1);
    assert_eq!(packet.debug.returned_items, 1);
    assert_eq!(packet.relevant_memory.len(), 1);
    assert!(packet.relevant_memory[0].content.contains("high signal"));
}

#[test]
fn broad_recall_returns_more_relevant_items_than_focused() {
    let dir = tempdir().unwrap();
    let mut engine = MemoryEngine::init_at(dir.path()).unwrap();
    for index in 0..8 {
        remember_text_cell(
            &mut engine,
            "global",
            MemoryStatus::Active,
            TrustLevel::ToolObserved,
            &format!("broad recall topic item {index}"),
            &["tag:broad".to_string(), "tag:recall".to_string()],
        );
    }

    let focused = engine
        .recall(RecallRequest::new("broad recall topic"))
        .unwrap();
    let mut broad_request = RecallRequest::new("broad recall topic");
    broad_request.mode = RecallMode::Broad;
    let broad = engine.recall(broad_request).unwrap();

    assert_eq!(focused.debug.recall_mode, RecallMode::Focused);
    assert_eq!(broad.debug.recall_mode, RecallMode::Broad);
    assert_eq!(focused.relevant_memory.len(), 5);
    assert!(broad.relevant_memory.len() > focused.relevant_memory.len());
    assert_eq!(broad.relevant_memory.len(), 8);
    assert_eq!(broad.debug.max_items, 20);
}

#[test]
fn broad_page_pruning_does_not_create_false_negatives() {
    let dir = tempdir().unwrap();
    let mut engine = MemoryEngine::init_at(dir.path()).unwrap();
    remember_text_cell(
        &mut engine,
        "project-alpha",
        MemoryStatus::Active,
        TrustLevel::ToolObserved,
        "shared alpha memory",
        &["tag:shared".to_string()],
    );
    remember_text_cell(
        &mut engine,
        "project-beta",
        MemoryStatus::Active,
        TrustLevel::ToolObserved,
        "shared beta memory",
        &["tag:shared".to_string()],
    );
    engine.seal().unwrap();

    let mut request = RecallRequest::new("shared memory");
    request.mode = RecallMode::Broad;
    request.scope = Some("project-alpha".to_string());
    request.markers = vec!["tag:shared".to_string()];
    let packet = engine.recall(request).unwrap();

    assert_eq!(packet.relevant_memory.len(), 1);
    assert!(packet.relevant_memory[0].content.contains("alpha"));
    assert_eq!(packet.debug.candidate_pages_returned, 2);
    assert_eq!(packet.debug.loaded_pages, 1);
    assert_eq!(packet.debug.pruned_candidate_pages, 1);
    assert_eq!(packet.debug.pages_pruned_by_metadata, 1);
    assert_eq!(packet.debug.cells_decoded, 1);
}

#[test]
fn metadata_pruning_skips_pages_missing_explicit_markers() {
    let dir = tempdir().unwrap();
    let mut engine = MemoryEngine::init_at(dir.path()).unwrap();
    remember_text_cell(
        &mut engine,
        "explicit-alpha",
        MemoryStatus::Active,
        TrustLevel::ToolObserved,
        "shared alpha memory",
        &["tag:wanted".to_string()],
    );
    remember_text_cell(
        &mut engine,
        "explicit-beta",
        MemoryStatus::Active,
        TrustLevel::ToolObserved,
        "shared beta memory",
        &["tag:other".to_string()],
    );
    engine.seal().unwrap();

    let mut request = RecallRequest::new("shared memory");
    request.mode = RecallMode::Broad;
    request.markers = vec!["tag:wanted".to_string()];
    let packet = engine.recall(request).unwrap();

    assert_eq!(packet.relevant_memory.len(), 1);
    assert!(packet.relevant_memory[0].content.contains("alpha"));
    assert_eq!(packet.debug.candidate_pages_returned, 2);
    assert_eq!(packet.debug.loaded_pages, 1);
    assert_eq!(packet.debug.pruned_candidate_pages, 1);
    assert_eq!(packet.debug.pages_pruned_by_metadata, 1);
    assert_eq!(packet.debug.cells_decoded, 1);
}

#[test]
fn full_scope_returns_all_active_memory_inside_scope() {
    let dir = tempdir().unwrap();
    let mut engine = MemoryEngine::init_at(dir.path()).unwrap();
    remember_text_cell(
        &mut engine,
        "project-alpha",
        MemoryStatus::Active,
        TrustLevel::UserConfirmed,
        "alpha active memory",
        &["tag:alpha".to_string()],
    );
    remember_text_cell(
        &mut engine,
        "project-alpha",
        MemoryStatus::Verified,
        TrustLevel::ToolObserved,
        "alpha verified memory",
        &["tag:alpha".to_string()],
    );
    remember_text_cell(
        &mut engine,
        "project-beta",
        MemoryStatus::Active,
        TrustLevel::UserConfirmed,
        "beta active memory",
        &["tag:alpha".to_string()],
    );
    engine.seal().unwrap();

    let mut request = RecallRequest::new("");
    request.mode = RecallMode::FullScope;
    request.scope = Some("project-alpha".to_string());
    let packet = engine.recall(request).unwrap();

    let contents = packet
        .relevant_memory
        .iter()
        .map(|item| item.content.as_str())
        .collect::<Vec<_>>();
    assert_eq!(packet.debug.recall_mode, RecallMode::FullScope);
    assert!(packet.debug.full_scope_used);
    assert_eq!(packet.debug.returned_items, 2);
    assert_eq!(packet.relevant_memory.len(), 2);
    assert!(contents.contains(&"alpha active memory"));
    assert!(contents.contains(&"alpha verified memory"));
    assert!(!contents.contains(&"beta active memory"));
}

#[test]
fn full_scope_without_scope_fails() {
    let dir = tempdir().unwrap();
    let engine = MemoryEngine::init_at(dir.path()).unwrap();
    let mut request = RecallRequest::new("");
    request.mode = RecallMode::FullScope;

    let err = engine.recall(request).unwrap_err();

    assert!(err.to_string().contains("full-scope recall requires"));
}

#[test]
fn seal_hot_cells_into_pages() {
    let dir = tempdir().unwrap();
    let mut engine = MemoryEngine::init_at(dir.path()).unwrap();
    remember_answer_style(&mut engine);

    let report = engine.seal().unwrap();
    let stats = engine.stats().unwrap();

    assert_eq!(report.hot_cells_sealed, 1);
    assert_eq!(stats.hot_cells, 0);
    assert_eq!(stats.sealed_pages, 1);
    assert_eq!(stats.sealed_cells, 1);
    let exported = engine.export_json().unwrap();
    let page = serde_json::from_value::<Vec<mge_core::MemoryPage>>(exported["pages"].clone())
        .unwrap()
        .remove(0);
    assert!(page.checksum.is_some());
    assert!(mge_core::page_checksum_matches(&page).unwrap());
}

#[test]
fn seal_preserves_source_and_links() {
    let dir = tempdir().unwrap();
    let mut engine = MemoryEngine::init_at(dir.path()).unwrap();
    let first = remember_answer_style(&mut engine);
    let mut second = RememberRequest::new(
        MemoryKind::Decision,
        MemoryValue::Text("Use concise technical answers".to_string()),
    );
    second.source = Some(MemorySource {
        source_type: "issue".to_string(),
        reference: "MGE-1".to_string(),
    });
    second.links = vec![first.id];
    let second = engine.remember(second).unwrap();

    engine.seal().unwrap();

    let exported = engine.export_json().unwrap();
    let pages =
        serde_json::from_value::<Vec<mge_core::MemoryPage>>(exported["pages"].clone()).unwrap();
    let sealed_cell = pages
        .iter()
        .flat_map(|page| &page.cells)
        .find(|cell| cell.id == second.id)
        .unwrap();

    assert_eq!(
        sealed_cell.source,
        Some(MemorySource {
            source_type: "issue".to_string(),
            reference: "MGE-1".to_string(),
        })
    );
    assert_eq!(sealed_cell.links, vec![first.id]);
}

#[test]
fn markdown_export_writes_human_readable_memory_file() {
    let dir = tempdir().unwrap();
    let mut engine = MemoryEngine::init_at(dir.path()).unwrap();
    remember_answer_style(&mut engine);
    engine.seal().unwrap();

    let path = engine.export_markdown_to_default_path().unwrap();
    let markdown = fs::read_to_string(&path).unwrap();

    assert_eq!(path, dir.path().join("exports").join("memory.md"));
    assert!(markdown.contains("# Memory Genome Export"));
    assert!(markdown.contains("## Sealed Pages"));
    assert!(markdown.contains("User prefers concise technical explanations"));
}

#[test]
fn recall_from_sealed_pages() {
    let dir = tempdir().unwrap();
    let mut engine = MemoryEngine::init_at(dir.path()).unwrap();
    remember_answer_style(&mut engine);
    engine.seal().unwrap();

    let packet = engine
        .recall(RecallRequest::new(
            "How should the agent answer technical questions?",
        ))
        .unwrap();

    assert_eq!(packet.relevant_memory.len(), 1);
    assert_eq!(packet.debug.hot_cells_scanned, 0);
    assert_eq!(packet.debug.candidate_pages.len(), 1);
    assert_eq!(packet.debug.sealed_cells_scanned, 1);
}

#[test]
fn sealed_recall_context_output_is_stable_after_cache_hit() {
    let dir = tempdir().unwrap();
    let mut engine = MemoryEngine::init_at(dir.path()).unwrap();
    remember_answer_style(&mut engine);
    engine.seal().unwrap();

    let request = RecallRequest::new("How should the agent answer technical questions?");
    let first = engine.recall(request.clone()).unwrap();
    let second = engine.recall(request.clone()).unwrap();
    let third = engine.recall(request).unwrap();

    assert_eq!(first.relevant_memory, second.relevant_memory);
    assert_eq!(first.constraints, second.constraints);
    assert_eq!(first.warnings, second.warnings);
    assert_eq!(first.debug.score_details, second.debug.score_details);
    assert_eq!(second.debug.loaded_pages, 1);
    assert_eq!(second.debug.decoded_page_cache_hits, 1);
    assert_eq!(second.debug.decoded_page_cache_misses, 0);
    assert_eq!(second.debug.scoring_cache_hits, 1);
    assert_eq!(second.debug.scoring_cache_misses, 0);
    assert_eq!(second.debug.returned_items, 1);
    assert_eq!(first.relevant_memory, third.relevant_memory);
    assert_eq!(first.debug.score_details, third.debug.score_details);
    assert_eq!(third.debug.scoring_cache_hits, 1);
    assert_eq!(third.debug.scoring_cache_misses, 0);
}

#[test]
fn recall_debug_includes_timing_breakdown_and_counters() {
    let dir = tempdir().unwrap();
    let mut engine = MemoryEngine::init_at(dir.path()).unwrap();
    remember_answer_style(&mut engine);
    engine.seal().unwrap();

    let packet = engine
        .recall(RecallRequest::new(
            "How should the agent answer technical questions?",
        ))
        .unwrap();

    assert_eq!(packet.debug.pages_considered, 1);
    assert_eq!(packet.debug.loaded_pages, 1);
    assert_eq!(packet.debug.pruned_candidate_pages, 0);
    assert_eq!(packet.debug.cells_decoded, 1);
    assert_eq!(packet.debug.cells_ranked, 1);
    assert_eq!(packet.debug.returned_items, 1);
    assert!(packet.debug.total_recall_micros >= packet.debug.context_packet_build_micros);
    assert!(packet.debug.total_recall_micros >= packet.debug.reranking_micros);
    let debug_json = serde_json::to_value(&packet.debug).unwrap();
    for field in [
        "page_file_read_load_micros",
        "page_decode_micros",
        "scoring_cache_build_micros",
        "cell_filtering_micros",
        "reranking_micros",
        "context_packet_build_micros",
        "decoded_page_cache_hits",
        "decoded_page_cache_misses",
        "scoring_cache_hits",
        "scoring_cache_misses",
        "sealed_cells_skipped_before_token_scoring",
        "sealed_cells_token_scored",
    ] {
        assert!(
            debug_json
                .get(field)
                .and_then(|value| value.as_u64())
                .is_some(),
            "missing non-negative debug field {field}"
        );
    }
}

#[test]
fn recall_from_messagepack_zstd_sealed_pages() {
    let dir = tempdir().unwrap();
    let mut engine = MemoryEngine::init_with_options(
        dir.path(),
        InitOptions {
            page_codec: PageCodecKind::MessagePack,
            compression: CompressionKind::Zstd,
            index_kind: IndexKind::ExactMarkerPage,
            page_clusterer: PageClustererKind::ScopeKind,
            durability: DurabilityPolicy::Balanced,
        },
    )
    .unwrap();
    remember_answer_style(&mut engine);
    engine.seal().unwrap();

    let inspect = engine.inspect().unwrap();
    let entry = inspect.page_catalog.pages.first().unwrap();
    assert_eq!(entry.page_codec, PageCodecKind::MessagePack);
    assert_eq!(entry.compression, CompressionKind::Zstd);
    let exported = engine.export_json().unwrap();
    let page = serde_json::from_value::<Vec<mge_core::MemoryPage>>(exported["pages"].clone())
        .unwrap()
        .remove(0);
    assert!(page.checksum.is_some());
    assert!(mge_core::page_checksum_matches(&page).unwrap());

    let packet = engine
        .recall(RecallRequest::new(
            "How should the agent answer technical questions?",
        ))
        .unwrap();

    assert_eq!(packet.relevant_memory.len(), 1);
    assert_eq!(packet.debug.candidate_pages.len(), 1);
}

#[test]
fn page_catalog_stores_lightweight_metadata_summaries() {
    let dir = tempdir().unwrap();
    let mut engine = MemoryEngine::init_at(dir.path()).unwrap();
    remember_text_cell(
        &mut engine,
        "catalog-meta",
        MemoryStatus::Active,
        TrustLevel::UserConfirmed,
        "catalog metadata active memory",
        &["tag:metadata".to_string()],
    );
    engine.seal().unwrap();

    let inspect = engine.inspect().unwrap();
    let entry = inspect.page_catalog.pages.first().unwrap();
    let scope_marker_name = canonicalize_marker("scope:catalog-meta").unwrap();
    let kind_marker_name = canonicalize_marker("kind:project_fact").unwrap();
    let scope_marker = inspect
        .markers
        .iter()
        .find(|marker| marker.marker == scope_marker_name)
        .unwrap()
        .id;
    let kind_marker = inspect
        .markers
        .iter()
        .find(|marker| marker.marker == kind_marker_name)
        .unwrap()
        .id;

    assert_eq!(entry.cell_count, 1);
    assert!(entry.marker_summary.contains(&scope_marker));
    assert!(entry.scope_marker_summary.contains(&scope_marker));
    assert!(entry.kind_marker_summary.contains(&kind_marker));
    assert_eq!(entry.status_summary, vec![MemoryStatus::Active]);
    assert_eq!(entry.sensitivity_summary, vec![SensitivityLevel::Public]);
    assert_eq!(entry.trust_summary, vec![TrustLevel::UserConfirmed]);
    assert!(entry.encoded_size_bytes > 0);
}

#[test]
fn storage_config_update_keeps_existing_pages_readable() {
    let dir = tempdir().unwrap();
    let mut engine = MemoryEngine::init_at(dir.path()).unwrap();
    remember_answer_style(&mut engine);
    engine.seal().unwrap();

    let report = engine
        .update_storage_config(StorageConfigUpdate {
            page_codec: Some(PageCodecKind::MessagePack),
            compression: Some(CompressionKind::Zstd),
            index_kind: None,
            page_clusterer: None,
            durability: None,
        })
        .unwrap();
    assert_eq!(report.existing_pages_unchanged, 1);

    let mut request = RememberRequest::new(
        MemoryKind::UserPreference,
        MemoryValue::Text("User prefers direct technical answers".to_string()),
    );
    request.scope = "global".to_string();
    request.trust = TrustLevel::UserConfirmed;
    engine.remember(request).unwrap();
    engine.seal().unwrap();

    let inspect = engine.inspect().unwrap();
    assert_eq!(inspect.page_catalog.pages.len(), 2);
    assert_eq!(
        inspect.page_catalog.pages[0].page_codec,
        PageCodecKind::MessagePack
    );
    assert_eq!(
        inspect.page_catalog.pages[0].compression,
        CompressionKind::None
    );
    assert_eq!(
        inspect.page_catalog.pages[1].page_codec,
        PageCodecKind::MessagePack
    );
    assert_eq!(
        inspect.page_catalog.pages[1].compression,
        CompressionKind::Zstd
    );

    let packet = engine.recall(RecallRequest::new("technical")).unwrap();
    assert_eq!(packet.relevant_memory.len(), 2);
    assert_eq!(packet.debug.candidate_pages.len(), 2);
}

#[test]
fn seal_uses_marker_overlap_clusterer_when_configured() {
    let dir = tempdir().unwrap();
    let mut engine = MemoryEngine::init_at(dir.path()).unwrap();
    engine
        .update_storage_config(StorageConfigUpdate {
            page_codec: None,
            compression: None,
            index_kind: None,
            page_clusterer: Some(PageClustererKind::MarkerOverlap),
            durability: None,
        })
        .unwrap();

    let mut first = RememberRequest::new(
        MemoryKind::ProjectFact,
        MemoryValue::Text("alpha technical memory".to_string()),
    );
    first.scope = "project".to_string();
    first.status = MemoryStatus::Active;
    first.trust = TrustLevel::UserConfirmed;
    first.sensitivity = SensitivityLevel::Private;
    first.markers = vec!["tag:alpha".to_string(), "tag:technical".to_string()];
    engine.remember(first).unwrap();

    let mut second = RememberRequest::new(
        MemoryKind::ProjectFact,
        MemoryValue::Text("alpha technical followup".to_string()),
    );
    second.scope = "project".to_string();
    second.status = MemoryStatus::Active;
    second.trust = TrustLevel::UserConfirmed;
    second.sensitivity = SensitivityLevel::Private;
    second.markers = vec!["tag:alpha".to_string(), "tag:technical".to_string()];
    engine.remember(second).unwrap();

    let mut third = RememberRequest::new(
        MemoryKind::ProjectFact,
        MemoryValue::Text("unrelated archival note".to_string()),
    );
    third.scope = "project".to_string();
    third.status = MemoryStatus::Temporary;
    third.trust = TrustLevel::ExternalUntrusted;
    third.sensitivity = SensitivityLevel::Confidential;
    third.markers = vec!["tag:unrelated".to_string(), "tag:archive".to_string()];
    engine.remember(third).unwrap();

    engine.seal().unwrap();
    let inspect = engine.inspect().unwrap();

    assert_eq!(inspect.page_catalog.pages.len(), 2);
    assert!(inspect
        .page_catalog
        .pages
        .iter()
        .all(|entry| entry.page_clusterer == PageClustererKind::MarkerOverlap));
    assert_eq!(inspect.page_catalog.pages[0].cell_count, 2);
    assert_eq!(inspect.page_catalog.pages[1].cell_count, 1);
}

#[test]
fn marker_to_page_index_queries_candidates() {
    let cell = mge_core::MemoryCell::new(
        1,
        MemoryKind::ProjectFact,
        None,
        MemoryValue::Text("Rust workspace exists".to_string()),
        "project".to_string(),
        MemoryStatus::Active,
        TrustLevel::ToolObserved,
        SensitivityLevel::Public,
        vec![10, 20],
        None,
        Vec::new(),
    );
    let pages = build_pages_from_cells(&[cell], 1);
    let index = ExactMarkerPageIndex::build(&pages).unwrap();

    assert_eq!(index.kind(), IndexKind::ExactMarkerPage);
    assert_eq!(index.index_kind, IndexKind::ExactMarkerPage);
    assert_eq!(index.query(&[10]).unwrap(), vec![1]);
    assert_eq!(index.query(&[999]).unwrap(), Vec::<u64>::new());
}

#[test]
fn binary_fuse_page_index_queries_candidates() {
    let first = page_test_cell(1, vec![10, 20, 30, 40]);
    let second = page_test_cell(2, vec![50, 60, 70, 80]);
    let pages = build_pages_with_clusterer(
        &[first, second],
        1,
        &ScopeKindClusterer,
        PageBuildOptions {
            target_page_bytes: 64 * 1024,
            max_cells_per_page: 1,
        },
    );
    let index = BinaryFusePageIndex::build(&pages).unwrap();

    assert_eq!(index.kind(), IndexKind::BinaryFusePage);
    assert_eq!(index.query(&[10]).unwrap(), vec![1]);
    assert_eq!(index.query(&[50]).unwrap(), vec![2]);
    assert_eq!(index.query(&[10, 20]).unwrap(), vec![1]);
    let stats = index.query_with_stats(&[10]).unwrap();
    assert_eq!(stats.page_filters_scanned, 2);
    assert!(stats.candidate_pages_returned >= 1);
}

#[test]
fn binary_fuse_candidates_cover_exact_candidates_for_same_pages() {
    let pages = build_pages_with_clusterer(
        &[
            page_test_cell(1, vec![10, 20, 30, 40]),
            page_test_cell(2, vec![50, 60, 70, 80]),
            page_test_cell(3, vec![10, 90, 91, 92]),
        ],
        1,
        &ScopeKindClusterer,
        PageBuildOptions {
            target_page_bytes: 64 * 1024,
            max_cells_per_page: 1,
        },
    );
    let exact = ExactMarkerPageIndex::build(&pages).unwrap();
    let binary = BinaryFusePageIndex::build(&pages).unwrap();

    let exact_candidates = exact.query(&[10, 50]).unwrap();
    let binary_candidates = binary.query(&[10, 50]).unwrap();

    assert_eq!(exact_candidates, vec![1, 2, 3]);

    for page_id in exact_candidates {
        assert!(
            binary_candidates.contains(&page_id),
            "binary fuse candidates must include exact page {page_id}"
        );
    }
}

#[test]
fn recall_from_binary_fuse_page_index() {
    let dir = tempdir().unwrap();
    let mut engine = MemoryEngine::init_with_options(
        dir.path(),
        InitOptions {
            page_codec: PageCodecKind::MessagePack,
            compression: CompressionKind::None,
            index_kind: IndexKind::BinaryFusePage,
            page_clusterer: PageClustererKind::ScopeKind,
            durability: DurabilityPolicy::Balanced,
        },
    )
    .unwrap();
    remember_answer_style(&mut engine);
    engine.seal().unwrap();

    let inspect = engine.inspect().unwrap();
    assert_eq!(inspect.manifest.index_kind, IndexKind::BinaryFusePage);
    assert!(matches!(
        inspect.index,
        CandidateIndexData::BinaryFusePage(_)
    ));

    let packet = engine
        .recall(RecallRequest::new(
            "How should the agent answer technical questions?",
        ))
        .unwrap();

    assert_eq!(packet.relevant_memory.len(), 1);
    assert_eq!(packet.debug.index_kind, IndexKind::BinaryFusePage);
    assert_eq!(packet.debug.candidate_pages, vec![1]);
    assert_eq!(packet.debug.page_filters_scanned, 1);
    assert_eq!(packet.debug.candidate_pages_returned, 1);
    assert_eq!(packet.debug.loaded_pages, 1);
    assert_eq!(packet.debug.sealed_cells_scanned, 1);
}

#[test]
fn changing_index_kind_rebuilds_candidate_index_without_rewriting_pages() {
    let dir = tempdir().unwrap();
    let mut engine = MemoryEngine::init_at(dir.path()).unwrap();
    remember_answer_style(&mut engine);
    engine.seal().unwrap();

    let before = engine.inspect().unwrap();
    assert!(matches!(
        before.index,
        CandidateIndexData::ExactMarkerPage(_)
    ));
    let first_page_file = before.page_catalog.pages[0].file.clone();

    let report = engine
        .update_storage_config(StorageConfigUpdate {
            page_codec: None,
            compression: None,
            index_kind: Some(IndexKind::BinaryFusePage),
            page_clusterer: None,
            durability: None,
        })
        .unwrap();

    assert_eq!(report.existing_pages_unchanged, 1);
    assert_eq!(report.current.index_kind, IndexKind::BinaryFusePage);

    let after = engine.inspect().unwrap();
    assert!(matches!(after.index, CandidateIndexData::BinaryFusePage(_)));
    assert_eq!(after.page_catalog.index_kind, IndexKind::BinaryFusePage);
    assert_eq!(after.page_catalog.pages[0].file, first_page_file);

    let packet = engine
        .recall(RecallRequest::new(
            "How should the agent answer technical questions?",
        ))
        .unwrap();
    assert_eq!(packet.relevant_memory.len(), 1);
}

#[test]
fn page_catalog_defaults_to_exact_marker_page_index_kind() {
    let catalog: mge_core::PageCatalog =
        serde_json::from_value(serde_json::json!({ "pages": [] })).unwrap();

    assert_eq!(catalog.index_kind, IndexKind::ExactMarkerPage);
}

#[test]
fn page_builder_respects_max_cells_per_page() {
    let cells = vec![
        page_test_cell(1, vec![1, 2, 3]),
        page_test_cell(2, vec![1, 2, 4]),
    ];

    let pages = build_pages_with_clusterer(
        &cells,
        1,
        &ScopeKindClusterer,
        PageBuildOptions {
            target_page_bytes: 64 * 1024,
            max_cells_per_page: 1,
        },
    );

    assert_eq!(pages.len(), 2);
    assert_eq!(pages[0].cell_count, 1);
    assert_eq!(pages[1].cell_count, 1);
}

#[test]
fn marker_overlap_clusterer_groups_similar_cells() {
    let cells = vec![
        page_test_cell(1, vec![1, 2, 3, 10]),
        page_test_cell(2, vec![1, 2, 3, 10, 11]),
        page_test_cell(3, vec![1, 2, 3, 90, 91]),
    ];

    let pages = build_pages_with_clusterer(
        &cells,
        10,
        &MarkerOverlapClusterer::new(4),
        PageBuildOptions {
            target_page_bytes: 64 * 1024,
            max_cells_per_page: 10,
        },
    );

    assert_eq!(pages.len(), 2);
    assert_eq!(pages[0].page_id, 10);
    assert_eq!(
        pages[0]
            .cells
            .iter()
            .map(|cell| cell.id)
            .collect::<Vec<_>>(),
        vec![1, 2]
    );
    assert_eq!(
        pages[1]
            .cells
            .iter()
            .map(|cell| cell.id)
            .collect::<Vec<_>>(),
        vec![3]
    );
}

#[test]
fn page_catalog_entries_default_to_binary_without_compression() {
    let value = serde_json::json!({
        "page_id": 1,
        "file": "000001.mgp",
        "created_at": 100,
        "cell_count": 2,
        "marker_summary": [1, 2, 3]
    });

    let entry: PageCatalogEntry = serde_json::from_value(value).unwrap();

    assert_eq!(entry.page_codec, PageCodecKind::MessagePack);
    assert_eq!(entry.compression, CompressionKind::None);
    assert_eq!(entry.page_clusterer, PageClustererKind::ScopeKind);
    assert!(entry.scope_marker_summary.is_empty());
    assert!(entry.kind_marker_summary.is_empty());
    assert!(entry.status_summary.is_empty());
    assert!(entry.sensitivity_summary.is_empty());
    assert!(entry.trust_summary.is_empty());
    assert_eq!(entry.encoded_size_bytes, 0);
}

#[test]
fn messagepack_page_codec_roundtrips_page() {
    let cell = mge_core::MemoryCell::new(
        1,
        MemoryKind::ProjectFact,
        Some("codec".to_string()),
        MemoryValue::Text("MessagePack codec roundtrip".to_string()),
        "project".to_string(),
        MemoryStatus::Active,
        TrustLevel::SystemGenerated,
        SensitivityLevel::Public,
        vec![1, 2],
        None,
        Vec::new(),
    );
    let page = build_pages_from_cells(&[cell], 42)
        .into_iter()
        .next()
        .unwrap();
    let codec = MessagePackPageCodec;

    let encoded = codec.encode(&page).unwrap();
    let decoded = codec.decode(&encoded).unwrap();

    assert_eq!(decoded.page_id, 42);
    assert_eq!(decoded.cells.len(), 1);
    assert_eq!(decoded.cells[0].value, page.cells[0].value);
}

#[test]
fn page_checksum_is_independent_of_page_codec() {
    let cell = mge_core::MemoryCell::new(
        1,
        MemoryKind::ProjectFact,
        Some("checksum".to_string()),
        MemoryValue::Text("Codec-independent checksum".to_string()),
        "project".to_string(),
        MemoryStatus::Active,
        TrustLevel::SystemGenerated,
        SensitivityLevel::Public,
        vec![1, 2],
        None,
        Vec::new(),
    );
    let mut page = build_pages_from_cells(&[cell], 42)
        .into_iter()
        .next()
        .unwrap();
    mge_core::attach_page_checksum(&mut page).unwrap();

    let json_codec = mge_core::JsonPageCodec;
    let msgpack_codec = MessagePackPageCodec;
    let json_decoded = json_codec
        .decode(&json_codec.encode(&page).unwrap())
        .unwrap();
    let msgpack_decoded = msgpack_codec
        .decode(&msgpack_codec.encode(&page).unwrap())
        .unwrap();

    assert_eq!(
        mge_core::page_content_checksum(&json_decoded).unwrap(),
        mge_core::page_content_checksum(&msgpack_decoded).unwrap()
    );
    assert!(mge_core::page_checksum_matches(&json_decoded).unwrap());
    assert!(mge_core::page_checksum_matches(&msgpack_decoded).unwrap());
}

#[test]
fn deprecated_rejected_and_superseded_memories_are_filtered_by_default() {
    let dir = tempdir().unwrap();
    let mut engine = MemoryEngine::init_at(dir.path()).unwrap();
    remember_with_status(&mut engine, MemoryStatus::Active, "Use active style");
    remember_with_status(
        &mut engine,
        MemoryStatus::Deprecated,
        "Use deprecated style",
    );
    remember_with_status(&mut engine, MemoryStatus::Rejected, "Use rejected style");
    remember_with_status(
        &mut engine,
        MemoryStatus::Superseded,
        "Use superseded style",
    );

    let packet = engine.recall(RecallRequest::new("style")).unwrap();

    assert_eq!(packet.relevant_memory.len(), 1);
    assert!(packet.relevant_memory[0].content.contains("active style"));
}

#[test]
fn deprecated_rejected_and_superseded_are_excluded_before_scoring() {
    let dir = tempdir().unwrap();
    let mut engine = MemoryEngine::init_at(dir.path()).unwrap();
    remember_text_cell(
        &mut engine,
        "policy-check",
        MemoryStatus::Active,
        TrustLevel::UserConfirmed,
        "policy active style",
        &["tag:style".to_string()],
    );
    remember_text_cell(
        &mut engine,
        "policy-check",
        MemoryStatus::Deprecated,
        TrustLevel::UserConfirmed,
        "policy deprecated style",
        &["tag:style".to_string()],
    );
    remember_text_cell(
        &mut engine,
        "policy-check",
        MemoryStatus::Rejected,
        TrustLevel::UserConfirmed,
        "policy rejected style",
        &["tag:style".to_string()],
    );
    remember_text_cell(
        &mut engine,
        "policy-check",
        MemoryStatus::Superseded,
        TrustLevel::UserConfirmed,
        "policy superseded style",
        &["tag:style".to_string()],
    );
    engine.seal().unwrap();

    let packet = engine.recall(RecallRequest::new("policy style")).unwrap();

    assert_eq!(packet.relevant_memory.len(), 1);
    assert!(packet.relevant_memory[0].content.contains("active style"));
    assert_eq!(packet.debug.cells_ranked, 1);
    assert_eq!(packet.debug.score_details.len(), 1);
    assert!(packet.debug.cells_filtered >= 3);
}

#[test]
fn metadata_status_summary_prunes_disallowed_status_pages() {
    let dir = tempdir().unwrap();
    let mut engine = MemoryEngine::init_at(dir.path()).unwrap();
    remember_text_cell(
        &mut engine,
        "status-active",
        MemoryStatus::Active,
        TrustLevel::UserConfirmed,
        "status active style",
        &["tag:style".to_string()],
    );
    remember_text_cell(
        &mut engine,
        "status-deprecated",
        MemoryStatus::Deprecated,
        TrustLevel::UserConfirmed,
        "status deprecated style",
        &["tag:style".to_string()],
    );
    engine.seal().unwrap();

    let mut request = RecallRequest::new("style");
    request.mode = RecallMode::Broad;
    request.markers = vec!["tag:style".to_string()];
    let packet = engine.recall(request).unwrap();

    assert_eq!(packet.relevant_memory.len(), 1);
    assert!(packet.relevant_memory[0].content.contains("active style"));
    assert_eq!(packet.debug.candidate_pages_returned, 2);
    assert_eq!(packet.debug.loaded_pages, 1);
    assert_eq!(packet.debug.pruned_candidate_pages, 1);
    assert_eq!(packet.debug.pages_pruned_by_metadata, 1);
    assert_eq!(packet.debug.cells_ranked, 1);
}

#[test]
fn metadata_sensitivity_summary_prunes_disallowed_sensitivity_pages() {
    let dir = tempdir().unwrap();
    let mut engine = MemoryEngine::init_at(dir.path()).unwrap();
    remember_text_cell(
        &mut engine,
        "sensitivity-public",
        MemoryStatus::Active,
        TrustLevel::UserConfirmed,
        "sensitivity public style",
        &["tag:style".to_string()],
    );
    let mut secret = RememberRequest::new(
        MemoryKind::ProjectFact,
        MemoryValue::Reference("vault://style-secret".to_string()),
    );
    secret.scope = "sensitivity-secret".to_string();
    secret.status = MemoryStatus::Active;
    secret.trust = TrustLevel::UserConfirmed;
    secret.sensitivity = SensitivityLevel::SecretReference;
    secret.markers = vec!["tag:style".to_string()];
    engine.remember(secret).unwrap();
    engine.seal().unwrap();

    let mut request = RecallRequest::new("style");
    request.mode = RecallMode::Broad;
    request.markers = vec!["tag:style".to_string()];
    let packet = engine.recall(request).unwrap();

    assert_eq!(packet.relevant_memory.len(), 1);
    assert!(packet.relevant_memory[0].content.contains("public style"));
    assert_eq!(packet.debug.candidate_pages_returned, 2);
    assert_eq!(packet.debug.loaded_pages, 1);
    assert_eq!(packet.debug.pruned_candidate_pages, 1);
    assert_eq!(packet.debug.pages_pruned_by_metadata, 1);
    assert_eq!(packet.debug.cells_ranked, 1);
}

#[test]
fn secret_reference_memories_are_filtered() {
    let dir = tempdir().unwrap();
    let mut engine = MemoryEngine::init_at(dir.path()).unwrap();
    let mut request = RememberRequest::new(
        MemoryKind::ProjectFact,
        MemoryValue::Reference("vault://api-key".to_string()),
    );
    request.sensitivity = SensitivityLevel::SecretReference;
    request.markers = vec!["tag:api".to_string(), "tag:key".to_string()];
    engine.remember(request).unwrap();

    let packet = engine.recall(RecallRequest::new("api key")).unwrap();

    assert!(packet.relevant_memory.is_empty());
}

#[test]
fn policy_capabilities_can_allow_secret_references_explicitly() {
    let dir = tempdir().unwrap();
    let mut engine = MemoryEngine::init_at(dir.path()).unwrap();
    let mut request = RememberRequest::new(
        MemoryKind::ProjectFact,
        MemoryValue::Reference("vault://api-key".to_string()),
    );
    request.sensitivity = SensitivityLevel::SecretReference;
    request.markers = vec!["tag:api".to_string(), "tag:key".to_string()];
    engine.remember(request).unwrap();

    let mut recall = RecallRequest::new("api key");
    recall.capabilities = AgentCapabilities::new([AgentCapability::ReadSecretReferences]);
    let packet = engine.recall(recall).unwrap();

    assert_eq!(packet.relevant_memory.len(), 1);
    assert_eq!(
        packet.relevant_memory[0].sensitivity,
        SensitivityLevel::SecretReference
    );
    assert!(!packet
        .constraints
        .iter()
        .any(|constraint| constraint.contains("secret_reference")));
    assert!(packet
        .warnings
        .iter()
        .any(|warning| warning.contains("SecretReference cells were included")));
}

#[test]
fn recall_policy_defaults_are_restrictive() {
    let policy = RecallPolicy::default();

    assert!(!policy.include_deprecated);
    assert!(!policy.include_rejected);
    assert!(!policy.allow_secret_references);
}

#[test]
fn noop_audit_logger_accepts_events() {
    NoopAuditLogger
        .record(&AuditEvent {
            event_type: "test".to_string(),
            summary: "policy audit hook".to_string(),
        })
        .unwrap();
}

#[test]
fn context_packet_prompt_text() {
    let cell = mge_core::MemoryCell::new(
        1,
        MemoryKind::UserPreference,
        None,
        MemoryValue::Text("User prefers concise technical explanations".to_string()),
        "global".to_string(),
        MemoryStatus::Active,
        TrustLevel::UserConfirmed,
        SensitivityLevel::Private,
        vec![],
        None,
        Vec::new(),
    );
    let ranked = vec![mge_core::retrieval::RankedCell {
        cell,
        score: 10,
        score_detail: Default::default(),
    }];
    let dictionary = mge_core::MarkerDictionary::new();
    let packet = build_context_packet(
        "technical answer".to_string(),
        &ranked,
        &dictionary,
        ContextDebugInfo::default(),
        5,
    );
    let text = packet.to_prompt_text();

    assert!(text.contains("Relevant memory:"));
    assert!(text.contains("Do not use deprecated, rejected, or superseded memories."));
    assert!(!text.contains("score"));
}

#[test]
fn context_packet_deduplicates_ranked_cells_by_id() {
    let cell = mge_core::MemoryCell::new(
        1,
        MemoryKind::UserPreference,
        None,
        MemoryValue::Text("User prefers concise technical explanations".to_string()),
        "global".to_string(),
        MemoryStatus::Active,
        TrustLevel::UserConfirmed,
        SensitivityLevel::Private,
        vec![],
        None,
        Vec::new(),
    );
    let ranked = vec![
        mge_core::retrieval::RankedCell {
            cell: cell.clone(),
            score: 20,
            score_detail: mge_core::packet::ContextScoreDebugItem {
                cell_id: 1,
                score: 20,
                ..Default::default()
            },
        },
        mge_core::retrieval::RankedCell {
            cell,
            score: 10,
            score_detail: mge_core::packet::ContextScoreDebugItem {
                cell_id: 1,
                score: 10,
                ..Default::default()
            },
        },
    ];
    let dictionary = mge_core::MarkerDictionary::new();

    let packet = build_context_packet(
        "technical answer".to_string(),
        &ranked,
        &dictionary,
        ContextDebugInfo {
            total_candidates: 2,
            ..Default::default()
        },
        5,
    );

    assert_eq!(packet.relevant_memory.len(), 1);
    assert_eq!(packet.debug.score_details.len(), 1);
    assert_eq!(packet.debug.score_details[0].score, 20);
    assert_eq!(packet.debug.total_candidates, 1);
}

#[test]
fn score_debug_explains_status_trust_and_sensitivity() {
    let cell = mge_core::MemoryCell::new(
        1,
        MemoryKind::ProjectFact,
        Some("security posture".to_string()),
        MemoryValue::Text("Security posture is confidential".to_string()),
        "project".to_string(),
        MemoryStatus::Unverified,
        TrustLevel::ExternalUntrusted,
        SensitivityLevel::Confidential,
        vec![10],
        None,
        Vec::new(),
    );

    let detail = score_cell_debug(
        &cell,
        &RecallRequest::new("security posture"),
        &[10],
        &["security".to_string(), "posture".to_string()],
    )
    .unwrap();

    assert_eq!(detail.marker_overlap, 1);
    assert_eq!(detail.marker_overlap_score, 10);
    assert!(detail.exact_subject_match);
    assert_eq!(detail.exact_subject_score, 5);
    assert_eq!(detail.trust_bonus, -3);
    assert_eq!(detail.status_bonus, -1);
    assert_eq!(detail.sensitivity_penalty, 2);
    assert_eq!(
        detail.score,
        detail.marker_overlap_score
            + detail.exact_subject_score
            + detail.value_overlap_score
            + detail.exact_value_score
            + detail.trust_bonus
            + detail.status_bonus
            - detail.sensitivity_penalty
    );
}

#[test]
fn score_debug_explains_exact_value_match() {
    let cell = mge_core::MemoryCell::new(
        1,
        MemoryKind::UserPreference,
        Some("answer style".to_string()),
        MemoryValue::Symbol("concise technical".to_string()),
        "global".to_string(),
        MemoryStatus::Active,
        TrustLevel::UserConfirmed,
        SensitivityLevel::Private,
        vec![10],
        None,
        Vec::new(),
    );

    let detail = score_cell_debug(
        &cell,
        &RecallRequest::new("concise technical"),
        &[10],
        &["concise".to_string(), "technical".to_string()],
    )
    .unwrap();

    assert!(detail.exact_value_match);
    assert_eq!(detail.exact_value_score, 3);
    assert_eq!(detail.value_overlap, 2);
    assert_eq!(
        detail.score,
        detail.marker_overlap_score
            + detail.exact_subject_score
            + detail.value_overlap_score
            + detail.exact_value_score
            + detail.trust_bonus
            + detail.status_bonus
            - detail.sensitivity_penalty
    );
}

#[test]
fn stats_output_contains_required_fields() {
    let dir = tempdir().unwrap();
    let mut engine = MemoryEngine::init_at(dir.path()).unwrap();
    remember_answer_style(&mut engine);

    let stats_text = engine.stats().unwrap().to_human_text();

    assert!(stats_text.contains("hot cells: 1"));
    assert!(stats_text.contains("marker count:"));
    assert!(stats_text.contains("index type: exact_marker_page"));
    assert!(stats_text.contains("current index kind: exact_marker_page"));
}

#[test]
fn validate_clean_exact_store_passes() {
    let dir = tempdir().unwrap();
    let mut engine = MemoryEngine::init_at(dir.path()).unwrap();
    remember_answer_style(&mut engine);
    engine.seal().unwrap();

    let report = engine.validate().unwrap();
    let deep_report = engine.validate_deep().unwrap();

    assert!(report.ok);
    assert!(report.errors.is_empty());
    assert!(deep_report.ok);
    assert!(deep_report.errors.is_empty());
    assert_eq!(report.index_kind, IndexKind::ExactMarkerPage);
    assert_eq!(report.checked_sealed_pages, 1);
    assert_eq!(report.checked_sealed_cells, 1);
}

#[test]
fn validate_clean_binary_fuse_store_passes() {
    let dir = tempdir().unwrap();
    let mut engine = MemoryEngine::init_with_options(
        dir.path(),
        InitOptions {
            page_codec: PageCodecKind::MessagePack,
            compression: CompressionKind::None,
            index_kind: IndexKind::BinaryFusePage,
            page_clusterer: PageClustererKind::ScopeKind,
            durability: DurabilityPolicy::Balanced,
        },
    )
    .unwrap();
    remember_answer_style(&mut engine);
    engine.seal().unwrap();

    let report = engine.validate().unwrap();

    assert!(report.ok);
    assert!(report.errors.is_empty());
    assert_eq!(report.index_kind, IndexKind::BinaryFusePage);
    assert_eq!(report.checked_sealed_pages, 1);
    assert_eq!(report.checked_sealed_cells, 1);
}

#[test]
fn validate_reports_missing_page_file() {
    let dir = tempdir().unwrap();
    let mut engine = MemoryEngine::init_at(dir.path()).unwrap();
    remember_answer_style(&mut engine);
    engine.seal().unwrap();

    let page_file = engine.inspect().unwrap().page_catalog.pages[0].file.clone();
    fs::remove_file(dir.path().join("pages").join(page_file)).unwrap();

    let report = engine.validate().unwrap();

    assert!(!report.ok);
    assert!(report
        .errors
        .iter()
        .any(|error| error.contains("missing page file")));
}

#[test]
fn validate_reports_page_checksum_mismatch() {
    let dir = tempdir().unwrap();
    let mut engine = MemoryEngine::init_at(dir.path()).unwrap();
    remember_answer_style(&mut engine);
    engine.seal().unwrap();

    let page_file = engine.inspect().unwrap().page_catalog.pages[0].file.clone();
    let page_path = dir.path().join("pages").join(page_file);
    let codec = MessagePackPageCodec;
    let frame = binary::decode_frame(&fs::read(&page_path).unwrap(), FileKind::Page).unwrap();
    let mut page: mge_core::MemoryPage = codec.decode(&frame.payload).unwrap();
    page.checksum = Some("bad-checksum".to_string());
    let page_payload = codec.encode(&page).unwrap();
    let framed = binary::encode_frame(FileKind::Page, CodecId::MessagePack, &page_payload).unwrap();
    fs::write(&page_path, framed).unwrap();

    let report = engine.validate().unwrap();

    assert!(!report.ok);
    assert!(report
        .errors
        .iter()
        .any(|error| error.contains("checksum mismatch")));
}

#[test]
fn validate_reports_wrong_page_file_magic() {
    assert_page_storage_error_after_corruption(|bytes| bytes[0] ^= 0xff, "wrong magic");
}

#[test]
fn validate_reports_wrong_page_file_kind() {
    assert_page_storage_error_after_corruption(
        |bytes| bytes[8] = FileKind::Manifest as u8,
        "wrong file kind",
    );
}

#[test]
fn validate_reports_unsupported_page_file_version() {
    assert_page_storage_error_after_corruption(|bytes| bytes[9] = 2, "unsupported version");
}

#[test]
fn validate_reports_truncated_page_file_payload() {
    assert_page_storage_error_after_corruption(
        |bytes| {
            bytes.pop();
        },
        "truncated page payload",
    );
}

#[test]
fn validate_reports_corrupted_page_file_payload() {
    assert_page_storage_error_after_corruption(
        |bytes| {
            let last = bytes.len() - 1;
            bytes[last] ^= 0xff;
        },
        "corrupted page payload checksum",
    );
}

#[test]
fn validate_deep_reads_page_files_not_decoded_page_cache() {
    let dir = tempdir().unwrap();
    let mut engine = MemoryEngine::init_at(dir.path()).unwrap();
    remember_answer_style(&mut engine);
    engine.seal().unwrap();

    let packet = engine
        .recall(RecallRequest::new(
            "How should the agent answer technical questions?",
        ))
        .unwrap();
    assert_eq!(packet.debug.loaded_pages, 1);

    let page_file = engine.inspect().unwrap().page_catalog.pages[0].file.clone();
    let page_path = dir.path().join("pages").join(page_file);
    let mut bytes = fs::read(&page_path).unwrap();
    let last = bytes.len() - 1;
    bytes[last] ^= 0xff;
    fs::write(&page_path, bytes).unwrap();

    let report = engine.validate_deep().unwrap();

    assert!(!report.ok);
    assert!(report
        .errors
        .iter()
        .any(|error| error.contains("corrupted page payload checksum")));
}

#[test]
fn rebuild_indexes_reads_page_files_not_decoded_page_cache() {
    let dir = tempdir().unwrap();
    let mut engine = MemoryEngine::init_at(dir.path()).unwrap();
    remember_answer_style(&mut engine);
    engine.seal().unwrap();

    let packet = engine
        .recall(RecallRequest::new(
            "How should the agent answer technical questions?",
        ))
        .unwrap();
    assert_eq!(packet.debug.loaded_pages, 1);

    let page_file = engine.inspect().unwrap().page_catalog.pages[0].file.clone();
    let page_path = dir.path().join("pages").join(page_file);
    let mut bytes = fs::read(&page_path).unwrap();
    let last = bytes.len() - 1;
    bytes[last] ^= 0xff;
    fs::write(&page_path, bytes).unwrap();

    let err = engine.rebuild_catalog_and_indexes().unwrap_err();

    assert!(err.to_string().contains("corrupted page payload checksum"));
}

#[test]
fn validate_reports_wrong_hot_log_magic() {
    let dir = tempdir().unwrap();
    let mut engine = MemoryEngine::init_at(dir.path()).unwrap();
    remember_answer_style(&mut engine);
    let hot_path = dir.path().join("hot").join("hot.mgl");
    let mut bytes = fs::read(&hot_path).unwrap();
    bytes[0] ^= 0xff;
    fs::write(&hot_path, bytes).unwrap();

    let report = engine.validate().unwrap();

    assert!(!report.ok);
    assert!(report.errors.iter().any(|error| {
        error.contains("hot memory load failed") && error.contains("wrong magic")
    }));
}

#[test]
fn validate_reports_wrong_index_magic() {
    let dir = tempdir().unwrap();
    let mut engine = MemoryEngine::init_at(dir.path()).unwrap();
    remember_answer_style(&mut engine);
    engine.seal().unwrap();
    let index_path = dir.path().join("indexes").join("marker_index.mgi");
    let mut bytes = fs::read(&index_path).unwrap();
    bytes[0] ^= 0xff;
    fs::write(&index_path, bytes).unwrap();

    let report = engine.validate().unwrap();

    assert!(!report.ok);
    assert!(report.errors.iter().any(|error| {
        error.contains("candidate index load failed") && error.contains("wrong magic")
    }));
}

#[test]
fn validate_deep_reports_orphan_page_file_as_error() {
    let dir = tempdir().unwrap();
    let mut engine = MemoryEngine::init_at(dir.path()).unwrap();
    remember_answer_style(&mut engine);
    engine.seal().unwrap();
    fs::write(dir.path().join("pages").join("999999.mgp"), b"orphan").unwrap();

    let report = engine.validate_deep().unwrap();

    assert!(!report.ok);
    assert!(report
        .errors
        .iter()
        .any(|error| error.contains("orphan page file")));
}

#[test]
fn validate_warns_about_orphan_page_file() {
    let dir = tempdir().unwrap();
    let mut engine = MemoryEngine::init_at(dir.path()).unwrap();
    remember_answer_style(&mut engine);
    engine.seal().unwrap();
    fs::write(dir.path().join("pages").join("999999.mgp"), b"orphan").unwrap();

    let report = engine.validate().unwrap();

    assert!(report.ok);
    assert!(report
        .warnings
        .iter()
        .any(|warning| warning.contains("orphan page file")));
}

#[test]
fn validate_warns_about_unknown_index_file() {
    let dir = tempdir().unwrap();
    let mut engine = MemoryEngine::init_at(dir.path()).unwrap();
    remember_answer_style(&mut engine);
    engine.seal().unwrap();
    fs::write(
        dir.path().join("indexes").join("scratch-index.mgi"),
        b"not managed",
    )
    .unwrap();

    let report = engine.validate().unwrap();

    assert!(report.ok);
    assert!(report
        .warnings
        .iter()
        .any(|warning| warning.contains("unknown index file")));
}

#[test]
fn validate_reports_marker_dictionary_inconsistency() {
    let dir = tempdir().unwrap();
    let mut engine = MemoryEngine::init_at(dir.path()).unwrap();
    remember_answer_style(&mut engine);
    let markers_path = dir.path().join("dictionary").join("markers.mgd");
    #[derive(serde::Serialize)]
    struct BrokenMarkerDictionary {
        marker_to_id: BTreeMap<String, u32>,
        id_to_marker: BTreeMap<u32, String>,
        next_id: u32,
    }
    let broken = BrokenMarkerDictionary {
        marker_to_id: BTreeMap::from([("kind:user_preference".to_string(), 1)]),
        id_to_marker: BTreeMap::new(),
        next_id: 2,
    };
    binary::write_messagepack_file(&markers_path, FileKind::MarkerDictionary, &broken).unwrap();
    let engine = MemoryEngine::open_at(dir.path()).unwrap();

    let report = engine.validate().unwrap();

    assert!(!report.ok);
    assert!(report
        .errors
        .iter()
        .any(|error| error.contains("marker dictionary inconsistency")));
}

#[test]
fn validate_accepts_existing_cell_link() {
    let dir = tempdir().unwrap();
    let mut engine = MemoryEngine::init_at(dir.path()).unwrap();
    let first = remember_answer_style(&mut engine);
    let mut second = RememberRequest::new(
        MemoryKind::Decision,
        MemoryValue::Text("Use concise technical answers".to_string()),
    );
    second.links = vec![first.id];
    engine.remember(second).unwrap();

    let report = engine.validate().unwrap();

    assert!(report.ok);
    assert!(report.errors.is_empty());
}

#[test]
fn validate_reports_unknown_cell_link() {
    let dir = tempdir().unwrap();
    let mut engine = MemoryEngine::init_at(dir.path()).unwrap();
    let mut request = RememberRequest::new(
        MemoryKind::Decision,
        MemoryValue::Text("Use concise technical answers".to_string()),
    );
    request.links = vec![999];
    engine.remember(request).unwrap();

    let report = engine.validate().unwrap();

    assert!(!report.ok);
    assert!(report
        .errors
        .iter()
        .any(|error| error.contains("links to unknown cell 999")));
}

#[test]
fn rebuild_indexes_restores_missing_exact_index_and_preserves_pages() {
    let dir = tempdir().unwrap();
    let mut engine = MemoryEngine::init_at(dir.path()).unwrap();
    remember_answer_style(&mut engine);
    engine.seal().unwrap();

    let before_catalog = engine.inspect().unwrap().page_catalog;
    let before_pages = page_file_bytes(dir.path(), &before_catalog);
    fs::remove_file(dir.path().join("indexes").join("marker_index.mgi")).unwrap();

    let broken = engine.validate_deep().unwrap();
    assert!(!broken.ok);
    assert!(broken
        .errors
        .iter()
        .any(|error| error.contains("active candidate index file missing")));

    let report = engine.rebuild_catalog_and_indexes().unwrap();

    assert_eq!(report.index_kind, IndexKind::ExactMarkerPage);
    assert_eq!(report.pages_scanned, 1);
    assert!(report.exact_index_written);
    assert!(!report.binary_fuse_index_written);
    assert!(report.pages_unchanged);
    assert_eq!(
        page_file_bytes(dir.path(), &engine.inspect().unwrap().page_catalog),
        before_pages
    );
    assert!(dir
        .path()
        .join("indexes")
        .join("marker_index.mgi")
        .is_file());
    assert!(engine.validate_deep().unwrap().ok);

    let packet = engine
        .recall(RecallRequest::new(
            "How should the agent answer technical questions?",
        ))
        .unwrap();
    assert_eq!(packet.relevant_memory.len(), 1);
}

#[test]
fn rebuild_indexes_restores_outdated_catalog_summaries() {
    let dir = tempdir().unwrap();
    let mut engine = MemoryEngine::init_at(dir.path()).unwrap();
    remember_answer_style(&mut engine);
    engine.seal().unwrap();

    let mut catalog = engine.inspect().unwrap().page_catalog;
    catalog.pages[0].marker_summary.clear();
    catalog.pages[0].scope_marker_summary.clear();
    catalog.pages[0].kind_marker_summary.clear();
    catalog.pages[0].status_summary.clear();
    catalog.pages[0].sensitivity_summary.clear();
    catalog.pages[0].trust_summary.clear();
    catalog.pages[0].encoded_size_bytes = 1;
    binary::write_messagepack_file(
        dir.path().join("indexes").join("page_index.mgi"),
        FileKind::PageIndex,
        &catalog,
    )
    .unwrap();

    let broken = engine.validate_deep().unwrap();
    assert!(!broken.ok);
    assert!(broken
        .errors
        .iter()
        .any(|error| error.contains("marker_summary differs")));
    assert!(broken
        .errors
        .iter()
        .any(|error| error.contains("encoded_size_bytes")));

    let report = engine.rebuild_catalog_and_indexes().unwrap();
    let rebuilt = engine.inspect().unwrap().page_catalog;

    assert_eq!(report.catalog_entries_written, 1);
    assert!(!rebuilt.pages[0].marker_summary.is_empty());
    assert!(!rebuilt.pages[0].scope_marker_summary.is_empty());
    assert!(!rebuilt.pages[0].kind_marker_summary.is_empty());
    assert!(!rebuilt.pages[0].status_summary.is_empty());
    assert!(!rebuilt.pages[0].sensitivity_summary.is_empty());
    assert!(!rebuilt.pages[0].trust_summary.is_empty());
    assert!(rebuilt.pages[0].encoded_size_bytes > 1);
    assert!(engine.validate_deep().unwrap().ok);
}

#[test]
fn rebuild_indexes_restores_binary_fuse_index_when_configured() {
    let dir = tempdir().unwrap();
    let mut engine = MemoryEngine::init_with_options(
        dir.path(),
        InitOptions {
            page_codec: PageCodecKind::MessagePack,
            compression: CompressionKind::None,
            index_kind: IndexKind::BinaryFusePage,
            page_clusterer: PageClustererKind::ScopeKind,
            durability: DurabilityPolicy::Balanced,
        },
    )
    .unwrap();
    remember_answer_style(&mut engine);
    engine.seal().unwrap();

    let before_catalog = engine.inspect().unwrap().page_catalog;
    let before_pages = page_file_bytes(dir.path(), &before_catalog);
    fs::remove_file(dir.path().join("indexes").join("fuse_index.mgi")).unwrap();
    let broken = engine.validate_deep().unwrap();
    assert!(!broken.ok);
    assert!(broken
        .errors
        .iter()
        .any(|error| error.contains("active candidate index file missing")));

    let report = engine.rebuild_catalog_and_indexes().unwrap();

    assert_eq!(report.index_kind, IndexKind::BinaryFusePage);
    assert!(report.exact_index_written);
    assert!(report.binary_fuse_index_written);
    assert_eq!(report.active_index_file, "fuse_index.mgi");
    assert_eq!(
        page_file_bytes(dir.path(), &engine.inspect().unwrap().page_catalog),
        before_pages
    );
    assert!(matches!(
        engine.inspect().unwrap().index,
        CandidateIndexData::BinaryFusePage(_)
    ));
    assert!(engine.validate_deep().unwrap().ok);

    let packet = engine
        .recall(RecallRequest::new(
            "How should the agent answer technical questions?",
        ))
        .unwrap();
    assert_eq!(packet.relevant_memory.len(), 1);
    assert_eq!(packet.debug.index_kind, IndexKind::BinaryFusePage);
}

#[test]
fn rebuild_indexes_leaves_hot_memory_unaffected() {
    let dir = tempdir().unwrap();
    let mut engine = MemoryEngine::init_at(dir.path()).unwrap();
    remember_answer_style(&mut engine);
    engine.seal().unwrap();
    remember_text_cell(
        &mut engine,
        "hot-rebuild",
        MemoryStatus::Active,
        TrustLevel::UserConfirmed,
        "hot memory survives index rebuild",
        &["tag:hot_rebuild".to_string()],
    );
    let hot_log_path = dir.path().join("hot").join("hot.mgl");
    let hot_log_len_before = fs::metadata(&hot_log_path).unwrap().len();

    let report = engine.rebuild_catalog_and_indexes().unwrap();

    assert_eq!(report.hot_cells_unchanged, 1);
    assert_eq!(engine.stats().unwrap().hot_cells, 1);
    assert_eq!(
        fs::metadata(&hot_log_path).unwrap().len(),
        hot_log_len_before
    );
    let mut request = RecallRequest::new("hot memory survives");
    request.markers = vec!["tag:hot_rebuild".to_string()];
    let packet = engine.recall(request).unwrap();
    assert_eq!(packet.relevant_memory.len(), 1);
    assert_eq!(packet.debug.hot_total_cells, 1);
}

#[test]
fn rebuild_indexes_does_not_create_json_runtime_storage() {
    let dir = tempdir().unwrap();
    let mut engine = MemoryEngine::init_at(dir.path()).unwrap();
    remember_answer_style(&mut engine);
    engine.seal().unwrap();

    engine.rebuild_catalog_and_indexes().unwrap();

    assert!(!dir.path().join("manifest.json").exists());
    assert!(!dir.path().join("markers.json").exists());
    assert!(!dir.path().join("hot").join("hot_cells.jsonl").exists());
    assert!(!dir
        .path()
        .join("indexes")
        .join("page_catalog.json")
        .exists());
    assert!(!dir
        .path()
        .join("indexes")
        .join("marker_to_pages.json")
        .exists());
}

#[test]
fn synthetic_binary_fuse_candidates_cover_exact_candidates() {
    let exact_dir = tempdir().unwrap();
    let binary_dir = tempdir().unwrap();
    let mut exact = MemoryEngine::init_with_options(
        exact_dir.path(),
        InitOptions {
            page_codec: PageCodecKind::MessagePack,
            compression: CompressionKind::None,
            index_kind: IndexKind::ExactMarkerPage,
            page_clusterer: PageClustererKind::ScopeKind,
            durability: DurabilityPolicy::Balanced,
        },
    )
    .unwrap();
    let mut binary = MemoryEngine::init_with_options(
        binary_dir.path(),
        InitOptions {
            page_codec: PageCodecKind::MessagePack,
            compression: CompressionKind::None,
            index_kind: IndexKind::BinaryFusePage,
            page_clusterer: PageClustererKind::ScopeKind,
            durability: DurabilityPolicy::Balanced,
        },
    )
    .unwrap();

    remember_synthetic_cells(&mut exact, 96, 12, 4);
    remember_synthetic_cells(&mut binary, 96, 12, 4);
    exact.seal().unwrap();
    binary.seal().unwrap();

    assert_eq!(exact.stats().unwrap().sealed_pages, 12);
    assert_eq!(binary.stats().unwrap().sealed_pages, 12);

    for marker in [
        synthetic_group_marker(0),
        synthetic_group_marker(1),
        synthetic_group_marker(2),
        synthetic_group_marker(3),
        "bench_missing:noise_000".to_string(),
    ] {
        let exact_packet = exact
            .recall(synthetic_recall_request(&marker, "qsubset"))
            .unwrap();
        let binary_packet = binary
            .recall(synthetic_recall_request(&marker, "qsubset"))
            .unwrap();

        for page_id in exact_packet.debug.candidate_pages {
            assert!(
                binary_packet.debug.candidate_pages.contains(&page_id),
                "binary_fuse_page must include exact candidate page {page_id} for marker {marker}"
            );
        }
    }
}

fn assert_page_storage_error_after_corruption(
    mutate: impl FnOnce(&mut Vec<u8>),
    expected_message: &str,
) {
    let dir = tempdir().unwrap();
    let mut engine = MemoryEngine::init_at(dir.path()).unwrap();
    remember_answer_style(&mut engine);
    engine.seal().unwrap();

    let page_file = engine.inspect().unwrap().page_catalog.pages[0].file.clone();
    let page_path = dir.path().join("pages").join(page_file);
    let mut bytes = fs::read(&page_path).unwrap();
    mutate(&mut bytes);
    fs::write(&page_path, bytes).unwrap();

    let report = engine.validate().unwrap();

    assert!(!report.ok);
    assert!(
        report
            .errors
            .iter()
            .any(|error| error.contains(expected_message)),
        "expected validation error containing {expected_message:?}, got {:?}",
        report.errors
    );
}

fn page_file_bytes(root: &Path, catalog: &PageCatalog) -> BTreeMap<String, Vec<u8>> {
    catalog
        .pages
        .iter()
        .map(|entry| {
            (
                entry.file.clone(),
                fs::read(root.join("pages").join(&entry.file)).unwrap(),
            )
        })
        .collect()
}

fn remember_answer_style(engine: &mut MemoryEngine) -> mge_core::MemoryCell {
    let mut request = RememberRequest::new(
        MemoryKind::UserPreference,
        MemoryValue::Text("User prefers concise technical explanations".to_string()),
    );
    request.scope = "global".to_string();
    request.trust = TrustLevel::UserConfirmed;
    request.status = MemoryStatus::Active;
    engine.remember(request).unwrap()
}

fn remember_text_cell(
    engine: &mut MemoryEngine,
    scope: &str,
    status: MemoryStatus,
    trust: TrustLevel,
    content: &str,
    markers: &[String],
) -> mge_core::MemoryCell {
    let mut request = RememberRequest::new(
        MemoryKind::ProjectFact,
        MemoryValue::Text(content.to_string()),
    );
    request.scope = scope.to_string();
    request.status = status;
    request.trust = trust;
    request.sensitivity = SensitivityLevel::Public;
    request.markers = markers.to_vec();
    engine.remember(request).unwrap()
}

fn remember_synthetic_cells(
    engine: &mut MemoryEngine,
    cells: usize,
    pages: usize,
    marker_groups: usize,
) {
    let base_cells_per_page = cells / pages;
    let extra_cells = cells % pages;
    let mut cell_index = 0usize;

    for page in 0..pages {
        let page_cells = base_cells_per_page + usize::from(page < extra_cells);
        let group = page % marker_groups;

        for page_cell in 0..page_cells {
            let mut request = RememberRequest::new(
                MemoryKind::ProjectFact,
                MemoryValue::Text(format!(
                    "synthetic memory group g{group:03} page p{page:04} cell c{cell_index:06}"
                )),
            );
            request.subject = Some(format!("synthetic page {page:04} group {group:03}"));
            request.scope = format!("bench_page_{page:04}");
            request.status = MemoryStatus::Active;
            request.trust = TrustLevel::ToolObserved;
            request.sensitivity = SensitivityLevel::Public;
            request.markers = vec![
                synthetic_group_marker(group),
                format!("bench_page_marker:p{page:04}"),
                format!("bench_bucket:b{:02}", page_cell % 16),
            ];
            engine.remember(request).unwrap();
            cell_index += 1;
        }
    }
}

fn synthetic_recall_request(marker: &str, query: &str) -> RecallRequest {
    let mut request = RecallRequest::new(query);
    request.markers = vec![marker.to_string()];
    request.max_items = 20;
    request
}

fn synthetic_group_marker(group: usize) -> String {
    format!("bench_group:g{group:03}")
}

fn remember_with_status(engine: &mut MemoryEngine, status: MemoryStatus, content: &str) {
    let mut request = RememberRequest::new(
        MemoryKind::UserPreference,
        MemoryValue::Text(content.to_string()),
    );
    request.status = status;
    request.trust = TrustLevel::UserConfirmed;
    request.markers = vec!["tag:style".to_string()];
    engine.remember(request).unwrap();
}

fn page_test_cell(id: u64, markers: Vec<u32>) -> mge_core::MemoryCell {
    mge_core::MemoryCell::new(
        id,
        MemoryKind::ProjectFact,
        Some(format!("cell_{id}")),
        MemoryValue::Text(format!("Page clustering test cell {id}")),
        "project".to_string(),
        MemoryStatus::Active,
        TrustLevel::SystemGenerated,
        SensitivityLevel::Public,
        markers,
        None,
        Vec::new(),
    )
}
