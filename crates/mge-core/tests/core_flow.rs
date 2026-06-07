use std::collections::BTreeMap;
use std::fs;

use mge_core::{
    build_context_packet, build_pages_from_cells, build_pages_with_clusterer, canonicalize_marker,
    marker_strings_for_cell_fields, score_cell_debug, AgentCapabilities, AgentCapability,
    AuditEvent, AuditLogger, BinaryFusePageIndex, CandidateIndexData, CandidatePageIndex,
    CompressionKind, Compressor, ContextDebugInfo, ExactMarkerPageIndex, IndexKind, InitOptions,
    MarkerOverlapClusterer, MemoryEngine, MemoryKind, MemorySource, MemoryStatus, MemoryValue,
    MessagePackPageCodec, NoopAuditLogger, PageBuildOptions, PageCatalogEntry, PageClustererKind,
    PageCodec, PageCodecKind, RecallPolicy, RecallRequest, RememberRequest, ScopeKindClusterer,
    SensitivityLevel, StorageConfigUpdate, TrustLevel, ZstdCompression,
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
fn init_rejects_json_runtime_page_codec() {
    let dir = tempdir().unwrap();
    let err = MemoryEngine::init_with_options(
        dir.path(),
        InitOptions {
            page_codec: PageCodecKind::Json,
            compression: CompressionKind::None,
            index_kind: IndexKind::ExactMarkerPage,
            page_clusterer: PageClustererKind::ScopeKind,
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
fn recall_from_messagepack_zstd_sealed_pages() {
    let dir = tempdir().unwrap();
    let mut engine = MemoryEngine::init_with_options(
        dir.path(),
        InitOptions {
            page_codec: PageCodecKind::MessagePack,
            compression: CompressionKind::Zstd,
            index_kind: IndexKind::ExactMarkerPage,
            page_clusterer: PageClustererKind::ScopeKind,
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
    assert!(text.contains("Do not use deprecated or rejected memories."));
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

    assert!(report.ok);
    assert!(report.errors.is_empty());
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
    let mut page: mge_core::MemoryPage = codec.decode(&fs::read(&page_path).unwrap()).unwrap();
    page.checksum = Some("bad-checksum".to_string());
    fs::write(&page_path, codec.encode(&page).unwrap()).unwrap();

    let report = engine.validate().unwrap();

    assert!(!report.ok);
    assert!(report
        .errors
        .iter()
        .any(|error| error.contains("checksum mismatch")));
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
    fs::write(&markers_path, rmp_serde::to_vec_named(&broken).unwrap()).unwrap();
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
