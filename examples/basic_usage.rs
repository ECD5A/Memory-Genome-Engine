use std::time::{SystemTime, UNIX_EPOCH};

use mge_core::{
    CompressionKind, DurabilityPolicy, IndexKind, InitOptions, MemoryEngine, MemoryKind,
    MemoryStatus, MemoryValue, PageClustererKind, PageCodecKind, RecallMode, RecallRequest,
    RememberRequest, SensitivityLevel, TrustLevel,
};

fn main() -> mge_core::Result<()> {
    let run_id = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0);
    let store_root = std::env::temp_dir().join(format!("mge-basic-usage-{run_id}"));

    let mut engine = MemoryEngine::init_with_options(
        &store_root,
        InitOptions {
            page_codec: PageCodecKind::MessagePack,
            compression: CompressionKind::Zstd,
            index_kind: IndexKind::ExactMarkerPage,
            page_clusterer: PageClustererKind::ScopeKind,
            durability: DurabilityPolicy::Balanced,
        },
    )?;

    let mut remember = RememberRequest::new(
        MemoryKind::ProjectFact,
        MemoryValue::Text(
            "Memory Genome Engine stores task memory as cells, marker genomes, and sealed pages."
                .to_string(),
        ),
    );
    remember.subject = Some("core flow".to_string());
    remember.scope = "mandate_1".to_string();
    remember.status = MemoryStatus::Active;
    remember.trust = TrustLevel::UserConfirmed;
    remember.sensitivity = SensitivityLevel::Public;
    remember.markers = vec![
        "topic:developer_ready_core".to_string(),
        "component:sealed_pages".to_string(),
    ];
    let cell = engine.remember(remember)?;

    let mut focused = RecallRequest::new("developer ready core sealed pages");
    focused.mode = RecallMode::Focused;
    focused.scope = Some("mandate_1".to_string());
    focused.max_items = 5;
    let hot_packet = engine.recall(focused.clone())?;
    assert!(!hot_packet.relevant_memory.is_empty());

    let checkpoint = engine.checkpoint()?;
    assert_eq!(checkpoint.hot_cells, 1);

    let seal = engine.seal()?;
    assert_eq!(seal.hot_cells_sealed, 1);

    let sealed_packet = engine.recall(focused)?;
    assert!(!sealed_packet.relevant_memory.is_empty());

    let validation = engine.validate_deep()?;
    assert!(validation.ok, "validation errors: {:?}", validation.errors);

    let rebuild = engine.rebuild_catalog_and_indexes()?;
    assert!(rebuild.pages_unchanged);
    assert_eq!(rebuild.hot_cells_unchanged, 0);

    let reopened = MemoryEngine::open_at(&store_root)?;
    let stats = reopened.stats()?;
    assert_eq!(stats.hot_cells, 0);
    assert!(stats.sealed_cells >= 1);

    println!(
        "stored cell {}, sealed pages {}, store {}",
        cell.id,
        stats.sealed_pages,
        store_root.display()
    );

    Ok(())
}
