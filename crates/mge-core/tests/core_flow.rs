use mge_core::{
    build_context_packet, build_pages_from_cells, build_pages_with_clusterer, canonicalize_marker,
    marker_strings_for_cell_fields, score_cell_debug, CandidatePageIndex, CompressionKind,
    Compressor, ContextDebugInfo, ExactMarkerPageIndex, IndexKind, InitOptions,
    MarkerOverlapClusterer, MemoryEngine, MemoryKind, MemoryStatus, MemoryValue,
    MessagePackPageCodec, PageBuildOptions, PageCatalogEntry, PageCodec, PageCodecKind,
    RecallRequest, RememberRequest, ScopeKindClusterer, SensitivityLevel, StorageConfigUpdate,
    TrustLevel, ZstdCompression,
};
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
        },
    )
    .unwrap();

    let inspect = engine.inspect().unwrap();

    assert_eq!(inspect.manifest.page_codec, PageCodecKind::MessagePack);
    assert_eq!(inspect.manifest.compression, CompressionKind::Zstd);
    assert_eq!(inspect.manifest.index_kind, IndexKind::ExactMarkerPage);
}

#[test]
fn storage_config_update_changes_future_defaults() {
    let dir = tempdir().unwrap();
    let mut engine = MemoryEngine::init_at(dir.path()).unwrap();

    let report = engine
        .update_storage_config(StorageConfigUpdate {
            page_codec: Some(PageCodecKind::MessagePack),
            compression: Some(CompressionKind::Zstd),
        })
        .unwrap();

    assert_eq!(report.previous.page_codec, PageCodecKind::Json);
    assert_eq!(report.previous.compression, CompressionKind::None);
    assert_eq!(report.current.page_codec, PageCodecKind::MessagePack);
    assert_eq!(report.current.compression, CompressionKind::Zstd);
    assert!(report.changed);

    let inspect = engine.inspect().unwrap();
    assert_eq!(inspect.manifest.page_codec, PageCodecKind::MessagePack);
    assert_eq!(inspect.manifest.compression, CompressionKind::Zstd);
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
fn recall_from_messagepack_zstd_sealed_pages() {
    let dir = tempdir().unwrap();
    let mut engine = MemoryEngine::init_with_options(
        dir.path(),
        InitOptions {
            page_codec: PageCodecKind::MessagePack,
            compression: CompressionKind::Zstd,
            index_kind: IndexKind::ExactMarkerPage,
        },
    )
    .unwrap();
    remember_answer_style(&mut engine);
    engine.seal().unwrap();

    let inspect = engine.inspect().unwrap();
    let entry = inspect.page_catalog.pages.first().unwrap();
    assert_eq!(entry.page_codec, PageCodecKind::MessagePack);
    assert_eq!(entry.compression, CompressionKind::Zstd);

    let packet = engine
        .recall(RecallRequest::new(
            "How should the agent answer technical questions?",
        ))
        .unwrap();

    assert_eq!(packet.relevant_memory.len(), 1);
    assert_eq!(packet.debug.candidate_pages.len(), 1);
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
        PageCodecKind::Json
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
fn legacy_page_catalog_entries_default_to_json_without_compression() {
    let value = serde_json::json!({
        "page_id": 1,
        "file": "000001.mgp",
        "created_at": 100,
        "cell_count": 2,
        "marker_summary": [1, 2, 3]
    });

    let entry: PageCatalogEntry = serde_json::from_value(value).unwrap();

    assert_eq!(entry.page_codec, PageCodecKind::Json);
    assert_eq!(entry.compression, CompressionKind::None);
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
fn deprecated_and_rejected_memories_are_filtered() {
    let dir = tempdir().unwrap();
    let mut engine = MemoryEngine::init_at(dir.path()).unwrap();
    remember_with_status(
        &mut engine,
        MemoryStatus::Deprecated,
        "Use deprecated style",
    );
    remember_with_status(&mut engine, MemoryStatus::Rejected, "Use rejected style");

    let packet = engine.recall(RecallRequest::new("style")).unwrap();

    assert!(packet.relevant_memory.is_empty());
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
    assert!(text.contains("Do not use deprecated or rejected memories."));
    assert!(!text.contains("score"));
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

fn remember_answer_style(engine: &mut MemoryEngine) {
    let mut request = RememberRequest::new(
        MemoryKind::UserPreference,
        MemoryValue::Text("User prefers concise technical explanations".to_string()),
    );
    request.scope = "global".to_string();
    request.trust = TrustLevel::UserConfirmed;
    request.status = MemoryStatus::Active;
    engine.remember(request).unwrap();
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
