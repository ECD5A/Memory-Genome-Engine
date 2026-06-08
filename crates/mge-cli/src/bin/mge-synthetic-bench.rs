use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use anyhow::{bail, Result};
use clap::Parser;
use mge_core::binary::{self, CodecId, FileKind};
use mge_core::compression::{decompress_with, CompressionKind};
use mge_core::packet::ContextScoreDebugItem;
use mge_core::pages::decode_page_with;
use mge_core::retrieval::RankedCell;
use mge_core::{
    build_context_packet, CandidateIndexData, ContextDebugInfo, IndexKind, InitOptions, MemoryCell,
    MemoryEngine, MemoryKind, MemoryPage, MemoryStatus, MemoryValue, NoSecurity, PageCatalogEntry,
    PageClustererKind, PageCodecKind, QueryMode, RecallMode, RecallRequest, RememberRequest,
    SecurityProvider, SensitivityLevel, TrustLevel, DEFAULT_MAX_CELLS_PER_PAGE,
};
use serde_json::json;

#[derive(Debug, Parser)]
#[command(
    name = "mge-synthetic-bench",
    about = "Synthetic Memory Genome Engine core benchmark/smoke harness"
)]
struct Args {
    #[arg(long, default_value_t = 2_000)]
    cells: usize,

    #[arg(long, default_value_t = 100)]
    pages: usize,

    #[arg(long, default_value_t = 16)]
    scopes: usize,

    #[arg(long, default_value_t = 8)]
    marker_groups: usize,

    #[arg(long, default_value_t = 5)]
    markers_per_cell: usize,

    #[arg(long, default_value_t = 8)]
    targeted_queries: usize,

    #[arg(long, default_value_t = 4)]
    noise_queries: usize,

    #[arg(long, default_value_t = 5)]
    repeats: usize,

    #[arg(long, default_value_t = 1)]
    seed: u64,

    #[arg(long)]
    store_root: Option<PathBuf>,
}

#[derive(Clone, Debug)]
struct CellSpec {
    storage_scope: String,
    subject: String,
    content: String,
    markers: Vec<String>,
}

#[derive(Clone, Debug)]
struct QuerySpec {
    name: String,
    query_text: String,
    query_type: &'static str,
    marker: String,
    full_scope_scope: String,
}

#[derive(Clone, Debug, Default)]
struct MetricSamples {
    samples: Vec<u64>,
}

#[derive(Debug)]
struct RecallBenchRun {
    recall_mode: RecallMode,
    latency_micros: MetricSamples,
    query_marker_extraction_micros: MetricSamples,
    hot_memory_lookup_micros: MetricSamples,
    candidate_page_index_lookup_micros: MetricSamples,
    page_file_read_load_micros: MetricSamples,
    page_decode_micros: MetricSamples,
    cell_filtering_micros: MetricSamples,
    reranking_micros: MetricSamples,
    context_packet_build_micros: MetricSamples,
    total_recall_micros: MetricSamples,
    hot_total_cells: MetricSamples,
    hot_candidate_cells: MetricSamples,
    hot_cells_scanned: MetricSamples,
    candidate_pages: MetricSamples,
    pages_considered: MetricSamples,
    pages_loaded: MetricSamples,
    pruned_candidate_pages: MetricSamples,
    cells_scanned: MetricSamples,
    cells_decoded: MetricSamples,
    cells_filtered: MetricSamples,
    cells_ranked: MetricSamples,
    sealed_cells_scanned: MetricSamples,
    returned_items: MetricSamples,
    first_repeat_candidate_pages: BTreeMap<String, Vec<u64>>,
}

#[derive(Debug)]
struct IndexLookupRun {
    latency_micros: MetricSamples,
    candidate_pages: MetricSamples,
}

#[derive(Debug)]
struct PageDecodeRun {
    latency_micros: MetricSamples,
    pages_decoded: usize,
    cells_decoded: usize,
}

#[derive(Debug)]
struct ContextBuildRun {
    latency_micros: MetricSamples,
    returned_items: MetricSamples,
}

#[derive(Debug)]
struct ModeRun {
    index_kind: IndexKind,
    total_sealed_pages: usize,
    total_cells: usize,
    storage_size_bytes: u64,
    post_seal_hot_cells: usize,
    remember_latency_micros: MetricSamples,
    seal_latency_micros: MetricSamples,
    hot_focused_recall: RecallBenchRun,
    hot_broad_recall: RecallBenchRun,
    hot_full_scope_recall: RecallBenchRun,
    focused_recall: RecallBenchRun,
    broad_recall: RecallBenchRun,
    full_scope_recall: RecallBenchRun,
    index_lookup: IndexLookupRun,
    page_decode: PageDecodeRun,
    context_packet_build: ContextBuildRun,
}

fn main() -> Result<()> {
    let args = Args::parse();
    validate_args(&args)?;

    let root = args.store_root.clone().unwrap_or(default_store_root()?);
    if root.exists() {
        bail!(
            "store root already exists: {}; pass a fresh --store-root",
            root.display()
        );
    }

    let cells = synthetic_cells(&args);
    let queries = synthetic_queries(&args);
    let exact = run_mode(&root, IndexKind::ExactMarkerPage, &args, &cells, &queries)?;
    let binary = run_mode(&root, IndexKind::BinaryFusePage, &args, &cells, &queries)?;
    let subset_check = exact_candidates_are_subset(&exact.focused_recall, &binary.focused_recall);
    if !subset_check {
        bail!("subset check failed: exact focused candidates must be included in binary_fuse candidates");
    }

    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "store_root": root,
            "synthetic_config": {
                "cells": args.cells,
                "target_sealed_pages": args.pages,
                "logical_scopes": args.scopes,
                "marker_groups": args.marker_groups,
                "markers_per_cell": args.markers_per_cell,
                "targeted_queries": args.targeted_queries,
                "noise_queries": args.noise_queries,
                "repeats": args.repeats,
                "seed": args.seed,
                "storage_scope_strategy": "one deterministic storage scope per target page; --scopes controls logical scope markers",
            },
            "subset_check": {
                "focused_exact_candidates_subset_of_binary_fuse_candidates": subset_check,
            },
            "comparison": comparison_to_json(&exact, &binary),
            "modes": [
                mode_to_json(&exact),
                mode_to_json(&binary),
            ],
        }))?
    );

    Ok(())
}

fn validate_args(args: &Args) -> Result<()> {
    if args.cells == 0 {
        bail!("--cells must be greater than 0");
    }
    if args.pages == 0 {
        bail!("--pages must be greater than 0");
    }
    if args.pages > args.cells {
        bail!("--pages must be less than or equal to --cells");
    }
    if args.scopes == 0 {
        bail!("--scopes must be greater than 0");
    }
    if args.marker_groups == 0 {
        bail!("--marker-groups must be greater than 0");
    }
    if args.markers_per_cell < 3 {
        bail!("--markers-per-cell must be at least 3 because group, scope, and page markers are reserved");
    }
    if args.repeats == 0 {
        bail!("--repeats must be greater than 0");
    }
    let max_cells_per_page = args.cells.div_ceil(args.pages);
    if max_cells_per_page > DEFAULT_MAX_CELLS_PER_PAGE {
        bail!(
            "--cells/--pages would require {max_cells_per_page} cells per page, above current seal limit {DEFAULT_MAX_CELLS_PER_PAGE}"
        );
    }
    Ok(())
}

fn default_store_root() -> Result<PathBuf> {
    let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
    Ok(std::env::current_dir()?
        .join("target")
        .join(format!("mge-synthetic-bench-{}-{now}", std::process::id())))
}

fn synthetic_cells(args: &Args) -> Vec<CellSpec> {
    let base_cells_per_page = args.cells / args.pages;
    let extra_cells = args.cells % args.pages;
    let mut rng = DeterministicRng::new(args.seed);
    let mut cells = Vec::with_capacity(args.cells);
    let mut cell_index = 0usize;

    for page in 0..args.pages {
        let page_cells = base_cells_per_page + usize::from(page < extra_cells);

        for page_cell in 0..page_cells {
            let group = (page + rng.next_usize(args.marker_groups)) % args.marker_groups;
            let logical_scope = rng.next_usize(args.scopes);
            let mut markers = BTreeSet::new();
            markers.insert(group_marker(group));
            markers.insert(logical_scope_marker(logical_scope));
            markers.insert(page_marker(page));

            while markers.len() < args.markers_per_cell {
                let marker_space = args.marker_groups * args.markers_per_cell * 16 + 17;
                markers.insert(format!("bench_marker:m{:06}", rng.next_usize(marker_space)));
            }

            cells.push(CellSpec {
                storage_scope: page_scope(page),
                subject: format!("synthetic page {page:04} group {group:03}"),
                content: format!(
                    "synthetic memory group g{group:03} scope s{logical_scope:03} page p{page:04} cell c{cell_index:06} local l{page_cell:04}"
                ),
                markers: markers.into_iter().collect(),
            });
            cell_index += 1;
        }
    }

    cells
}

fn synthetic_queries(args: &Args) -> Vec<QuerySpec> {
    let mut queries = Vec::new();
    for index in 0..args.targeted_queries {
        let group = index % args.marker_groups;
        let page = index % args.pages;
        queries.push(QuerySpec {
            name: format!("target_{index:03}_group_{group:03}"),
            query_text: format!("synthetic memory group g{group:03}"),
            query_type: "targeted",
            marker: group_marker(group),
            full_scope_scope: page_scope(page),
        });
    }
    for index in 0..args.noise_queries {
        let page = (args.targeted_queries + index) % args.pages;
        queries.push(QuerySpec {
            name: format!("noise_{index:03}"),
            query_text: format!("qnoise unmatched {index:03}"),
            query_type: "noise",
            marker: format!("bench_missing:noise_{index:03}"),
            full_scope_scope: page_scope(page),
        });
    }
    queries
}

fn run_mode(
    root: &Path,
    index_kind: IndexKind,
    args: &Args,
    cells: &[CellSpec],
    queries: &[QuerySpec],
) -> Result<ModeRun> {
    let mode_root = root.join(index_kind.as_str());
    let mut engine = MemoryEngine::init_with_options(
        &mode_root,
        InitOptions {
            page_codec: PageCodecKind::MessagePack,
            compression: CompressionKind::None,
            index_kind,
            page_clusterer: PageClustererKind::ScopeKind,
            durability: mge_core::DurabilityPolicy::Balanced,
        },
    )?;

    let mut remember_latency_micros = MetricSamples::default();
    let mut remembered_cells = Vec::with_capacity(cells.len());
    for spec in cells {
        let request = remember_request_from_spec(spec);
        let started = Instant::now();
        let cell = engine.remember(request)?;
        remember_latency_micros.record_elapsed(started);
        remembered_cells.push(cell);
    }

    let hot_focused_recall = run_recall_bench(&engine, RecallMode::Focused, args, queries)?;
    let hot_broad_recall = run_recall_bench(&engine, RecallMode::Broad, args, queries)?;
    let hot_full_scope_recall = run_recall_bench(&engine, RecallMode::FullScope, args, queries)?;

    let mut seal_latency_micros = MetricSamples::default();
    let started = Instant::now();
    engine.seal()?;
    seal_latency_micros.record_elapsed(started);

    let stats = engine.stats()?;
    let post_seal_hot_cells = stats.hot_cells;
    if post_seal_hot_cells != 0 {
        bail!(
            "{} left {} hot cells after seal; expected RAM hot layer to be clear",
            index_kind,
            post_seal_hot_cells
        );
    }
    if stats.sealed_pages != args.pages {
        bail!(
            "{} produced {} sealed pages, expected {}",
            index_kind,
            stats.sealed_pages,
            args.pages
        );
    }

    let inspect = engine.inspect()?;
    let marker_ids = inspect
        .markers
        .iter()
        .map(|entry| (entry.marker.clone(), entry.id))
        .collect::<BTreeMap<_, _>>();

    let focused_recall = run_recall_bench(&engine, RecallMode::Focused, args, queries)?;
    let broad_recall = run_recall_bench(&engine, RecallMode::Broad, args, queries)?;
    let full_scope_recall = run_recall_bench(&engine, RecallMode::FullScope, args, queries)?;
    let index_lookup = run_index_lookup_bench(&inspect.index, &marker_ids, args, queries)?;
    let page_decode = run_page_decode_bench(&mode_root, &inspect.page_catalog.pages, args)?;
    let context_packet_build =
        run_context_packet_build_bench(&engine, &remembered_cells, args.repeats)?;

    Ok(ModeRun {
        index_kind,
        total_sealed_pages: stats.sealed_pages,
        total_cells: stats.sealed_cells,
        storage_size_bytes: stats.store_size_bytes,
        post_seal_hot_cells,
        remember_latency_micros,
        seal_latency_micros,
        hot_focused_recall,
        hot_broad_recall,
        hot_full_scope_recall,
        focused_recall,
        broad_recall,
        full_scope_recall,
        index_lookup,
        page_decode,
        context_packet_build,
    })
}

fn remember_request_from_spec(spec: &CellSpec) -> RememberRequest {
    let mut request = RememberRequest::new(
        MemoryKind::ProjectFact,
        MemoryValue::Text(spec.content.clone()),
    );
    request.subject = Some(spec.subject.clone());
    request.scope = spec.storage_scope.clone();
    request.status = MemoryStatus::Active;
    request.trust = TrustLevel::ToolObserved;
    request.sensitivity = SensitivityLevel::Public;
    request.markers = spec.markers.clone();
    request
}

fn run_recall_bench(
    engine: &MemoryEngine,
    recall_mode: RecallMode,
    args: &Args,
    queries: &[QuerySpec],
) -> Result<RecallBenchRun> {
    let mut run = RecallBenchRun {
        recall_mode,
        latency_micros: MetricSamples::default(),
        query_marker_extraction_micros: MetricSamples::default(),
        hot_memory_lookup_micros: MetricSamples::default(),
        candidate_page_index_lookup_micros: MetricSamples::default(),
        page_file_read_load_micros: MetricSamples::default(),
        page_decode_micros: MetricSamples::default(),
        cell_filtering_micros: MetricSamples::default(),
        reranking_micros: MetricSamples::default(),
        context_packet_build_micros: MetricSamples::default(),
        total_recall_micros: MetricSamples::default(),
        hot_total_cells: MetricSamples::default(),
        hot_candidate_cells: MetricSamples::default(),
        hot_cells_scanned: MetricSamples::default(),
        candidate_pages: MetricSamples::default(),
        pages_considered: MetricSamples::default(),
        pages_loaded: MetricSamples::default(),
        pruned_candidate_pages: MetricSamples::default(),
        cells_scanned: MetricSamples::default(),
        cells_decoded: MetricSamples::default(),
        cells_filtered: MetricSamples::default(),
        cells_ranked: MetricSamples::default(),
        sealed_cells_scanned: MetricSamples::default(),
        returned_items: MetricSamples::default(),
        first_repeat_candidate_pages: BTreeMap::new(),
    };

    for repeat in 0..args.repeats {
        for spec in queries {
            let mut request = match recall_mode {
                RecallMode::Focused | RecallMode::Broad => {
                    let mut request = RecallRequest::new(spec.query_text.clone());
                    request.markers = vec![spec.marker.clone()];
                    request
                }
                RecallMode::FullScope => {
                    let mut request = RecallRequest::new("");
                    request.scope = Some(spec.full_scope_scope.clone());
                    request
                }
            };
            request.mode = recall_mode;
            request.max_items = 20;

            let started = Instant::now();
            let packet = engine.recall(request)?;
            run.latency_micros.record_elapsed(started);
            run.candidate_pages
                .record_usize(packet.debug.candidate_pages.len());
            run.query_marker_extraction_micros
                .record_u64(packet.debug.query_marker_extraction_micros);
            run.hot_memory_lookup_micros
                .record_u64(packet.debug.hot_memory_lookup_micros);
            run.candidate_page_index_lookup_micros
                .record_u64(packet.debug.candidate_page_index_lookup_micros);
            run.page_file_read_load_micros
                .record_u64(packet.debug.page_file_read_load_micros);
            run.page_decode_micros
                .record_u64(packet.debug.page_decode_micros);
            run.cell_filtering_micros
                .record_u64(packet.debug.cell_filtering_micros);
            run.reranking_micros
                .record_u64(packet.debug.reranking_micros);
            run.context_packet_build_micros
                .record_u64(packet.debug.context_packet_build_micros);
            run.total_recall_micros
                .record_u64(packet.debug.total_recall_micros);
            run.hot_total_cells
                .record_usize(packet.debug.hot_total_cells);
            run.hot_candidate_cells
                .record_usize(packet.debug.hot_candidate_cells);
            run.hot_cells_scanned
                .record_usize(packet.debug.hot_cells_scanned);
            run.pages_considered
                .record_usize(packet.debug.pages_considered);
            run.pages_loaded.record_usize(packet.debug.loaded_pages);
            run.pruned_candidate_pages
                .record_usize(packet.debug.pruned_candidate_pages);
            run.cells_scanned.record_usize(packet.debug.cells_scanned);
            run.cells_decoded.record_usize(packet.debug.cells_decoded);
            run.cells_filtered.record_usize(packet.debug.cells_filtered);
            run.cells_ranked.record_usize(packet.debug.cells_ranked);
            run.sealed_cells_scanned
                .record_usize(packet.debug.sealed_cells_scanned);
            run.returned_items.record_usize(packet.debug.returned_items);

            if repeat == 0 {
                run.first_repeat_candidate_pages
                    .insert(spec.name.clone(), packet.debug.candidate_pages);
            }
        }
    }

    Ok(run)
}

fn run_index_lookup_bench(
    index: &CandidateIndexData,
    marker_ids: &BTreeMap<String, u32>,
    args: &Args,
    queries: &[QuerySpec],
) -> Result<IndexLookupRun> {
    let mut run = IndexLookupRun {
        latency_micros: MetricSamples::default(),
        candidate_pages: MetricSamples::default(),
    };

    for _ in 0..args.repeats {
        for spec in queries
            .iter()
            .filter(|query| query.query_type == "targeted")
        {
            let Some(marker_id) = marker_ids.get(&spec.marker).copied() else {
                continue;
            };

            let started = Instant::now();
            let result = index.query_with_mode_stats(&[marker_id], QueryMode::Union)?;
            run.latency_micros.record_elapsed(started);
            run.candidate_pages.record_usize(result.page_ids.len());
        }
    }

    Ok(run)
}

fn run_page_decode_bench(
    mode_root: &Path,
    entries: &[PageCatalogEntry],
    args: &Args,
) -> Result<PageDecodeRun> {
    let mut run = PageDecodeRun {
        latency_micros: MetricSamples::default(),
        pages_decoded: 0,
        cells_decoded: 0,
    };

    for _ in 0..args.repeats {
        for entry in entries {
            let started = Instant::now();
            let page = decode_page_file(mode_root, entry)?;
            run.latency_micros.record_elapsed(started);
            run.pages_decoded += 1;
            run.cells_decoded += page.cells.len();
        }
    }

    Ok(run)
}

fn decode_page_file(mode_root: &Path, entry: &PageCatalogEntry) -> Result<MemoryPage> {
    let bytes = fs::read(mode_root.join("pages").join(&entry.file))?;
    let frame = binary::decode_frame(&bytes, FileKind::Page)?;
    let expected_codec = expected_page_codec(entry.page_codec, entry.compression)?;
    if frame.codec != expected_codec {
        bail!(
            "wrong codec for page {}: expected {}, found {}",
            entry.page_id,
            expected_codec.as_str(),
            frame.codec.as_str()
        );
    }

    let security = NoSecurity;
    let opened = security.open_page_bytes(&frame.payload)?;
    let decompressed = decompress_with(entry.compression, &opened)?;
    Ok(decode_page_with(entry.page_codec, &decompressed)?)
}

fn expected_page_codec(page_codec: PageCodecKind, compression: CompressionKind) -> Result<CodecId> {
    match (page_codec, compression) {
        (PageCodecKind::MessagePack, CompressionKind::None) => Ok(CodecId::MessagePack),
        (PageCodecKind::MessagePack, CompressionKind::Zstd) => Ok(CodecId::MessagePackZstd),
        (PageCodecKind::Json, _) => bail!("json page codec is not benchmark runtime storage"),
    }
}

fn run_context_packet_build_bench(
    engine: &MemoryEngine,
    remembered_cells: &[MemoryCell],
    repeats: usize,
) -> Result<ContextBuildRun> {
    let max_items = remembered_cells.len().min(100);
    let ranked = remembered_cells
        .iter()
        .take(max_items)
        .enumerate()
        .map(|(index, cell)| {
            let score = i64::try_from(max_items.saturating_sub(index)).unwrap_or(i64::MAX);
            RankedCell {
                cell: cell.clone(),
                score,
                score_detail: ContextScoreDebugItem {
                    cell_id: cell.id,
                    score,
                    ..Default::default()
                },
            }
        })
        .collect::<Vec<_>>();

    let mut run = ContextBuildRun {
        latency_micros: MetricSamples::default(),
        returned_items: MetricSamples::default(),
    };

    for _ in 0..repeats {
        let started = Instant::now();
        let packet = build_context_packet(
            "synthetic context packet build".to_string(),
            &ranked,
            engine.dictionary(),
            ContextDebugInfo {
                max_items,
                ..Default::default()
            },
            max_items,
        );
        run.latency_micros.record_elapsed(started);
        run.returned_items.record_usize(packet.debug.returned_items);
    }

    Ok(run)
}

fn exact_candidates_are_subset(exact: &RecallBenchRun, binary: &RecallBenchRun) -> bool {
    exact
        .first_repeat_candidate_pages
        .iter()
        .all(|(query_name, exact_pages)| {
            let binary_pages = binary
                .first_repeat_candidate_pages
                .get(query_name)
                .map(|pages| pages.iter().copied().collect::<BTreeSet<_>>())
                .unwrap_or_default();
            exact_pages
                .iter()
                .all(|page_id| binary_pages.contains(page_id))
        })
}

fn comparison_to_json(exact: &ModeRun, binary: &ModeRun) -> serde_json::Value {
    json!({
        "remember_avg_micros": pair_json(&exact.remember_latency_micros, &binary.remember_latency_micros),
        "seal_avg_micros": pair_json(&exact.seal_latency_micros, &binary.seal_latency_micros),
        "hot_focused_recall_avg_micros": pair_json(&exact.hot_focused_recall.latency_micros, &binary.hot_focused_recall.latency_micros),
        "hot_broad_recall_avg_micros": pair_json(&exact.hot_broad_recall.latency_micros, &binary.hot_broad_recall.latency_micros),
        "hot_full_scope_recall_avg_micros": pair_json(&exact.hot_full_scope_recall.latency_micros, &binary.hot_full_scope_recall.latency_micros),
        "focused_recall_avg_micros": pair_json(&exact.focused_recall.latency_micros, &binary.focused_recall.latency_micros),
        "broad_recall_avg_micros": pair_json(&exact.broad_recall.latency_micros, &binary.broad_recall.latency_micros),
        "full_scope_recall_avg_micros": pair_json(&exact.full_scope_recall.latency_micros, &binary.full_scope_recall.latency_micros),
        "hot_vs_sealed_recall_avg_micros": {
            "exact_marker_page": hot_vs_sealed_json(exact),
            "binary_fuse_page": hot_vs_sealed_json(binary),
        },
        "index_lookup_avg_micros": pair_json(&exact.index_lookup.latency_micros, &binary.index_lookup.latency_micros),
        "page_decode_avg_micros": pair_json(&exact.page_decode.latency_micros, &binary.page_decode.latency_micros),
        "context_packet_build_avg_micros": pair_json(&exact.context_packet_build.latency_micros, &binary.context_packet_build.latency_micros),
        "storage_size_bytes": {
            "exact_marker_page": exact.storage_size_bytes,
            "binary_fuse_page": binary.storage_size_bytes,
        },
    })
}

fn pair_json(exact: &MetricSamples, binary: &MetricSamples) -> serde_json::Value {
    json!({
        "exact_marker_page": exact.avg(),
        "binary_fuse_page": binary.avg(),
    })
}

fn hot_vs_sealed_json(mode: &ModeRun) -> serde_json::Value {
    json!({
        "focused": {
            "hot": mode.hot_focused_recall.latency_micros.avg(),
            "sealed": mode.focused_recall.latency_micros.avg(),
        },
        "broad": {
            "hot": mode.hot_broad_recall.latency_micros.avg(),
            "sealed": mode.broad_recall.latency_micros.avg(),
        },
        "full_scope": {
            "hot": mode.hot_full_scope_recall.latency_micros.avg(),
            "sealed": mode.full_scope_recall.latency_micros.avg(),
        },
    })
}

fn mode_to_json(mode: &ModeRun) -> serde_json::Value {
    json!({
        "index_kind": mode.index_kind,
        "total_sealed_pages": mode.total_sealed_pages,
        "total_cells": mode.total_cells,
        "storage_size_bytes": mode.storage_size_bytes,
        "seal_correctness": {
            "post_seal_hot_cells": mode.post_seal_hot_cells,
            "hot_cleared_after_seal": mode.post_seal_hot_cells == 0,
        },
        "build": {
            "remember_latency_micros": mode.remember_latency_micros.to_json(),
            "seal_latency_micros": mode.seal_latency_micros.to_json(),
        },
        "hot_recall_modes": {
            "focused": recall_to_json(&mode.hot_focused_recall),
            "broad": recall_to_json(&mode.hot_broad_recall),
            "full_scope": recall_to_json(&mode.hot_full_scope_recall),
        },
        "recall_modes": {
            "focused": recall_to_json(&mode.focused_recall),
            "broad": recall_to_json(&mode.broad_recall),
            "full_scope": recall_to_json(&mode.full_scope_recall),
        },
        "index_lookup": index_lookup_to_json(&mode.index_lookup),
        "page_decode": page_decode_to_json(&mode.page_decode),
        "context_packet_build": context_build_to_json(&mode.context_packet_build),
    })
}

fn recall_to_json(run: &RecallBenchRun) -> serde_json::Value {
    json!({
        "recall_mode": run.recall_mode,
        "latency_micros": run.latency_micros.to_json(),
        "timing_breakdown_micros": {
            "query_marker_extraction": run.query_marker_extraction_micros.to_json(),
            "hot_memory_lookup": run.hot_memory_lookup_micros.to_json(),
            "candidate_page_index_lookup": run.candidate_page_index_lookup_micros.to_json(),
            "page_file_read_load": run.page_file_read_load_micros.to_json(),
            "page_decode": run.page_decode_micros.to_json(),
            "cell_filtering": run.cell_filtering_micros.to_json(),
            "reranking": run.reranking_micros.to_json(),
            "context_packet_build": run.context_packet_build_micros.to_json(),
            "total_recall": run.total_recall_micros.to_json(),
        },
        "hot_total_cells": run.hot_total_cells.to_json(),
        "hot_candidate_cells": run.hot_candidate_cells.to_json(),
        "hot_cells_scanned": run.hot_cells_scanned.to_json(),
        "candidate_pages": run.candidate_pages.to_json(),
        "pages_considered": run.pages_considered.to_json(),
        "pages_loaded": run.pages_loaded.to_json(),
        "pruned_candidate_pages": run.pruned_candidate_pages.to_json(),
        "cells_scanned": run.cells_scanned.to_json(),
        "cells_decoded": run.cells_decoded.to_json(),
        "cells_filtered": run.cells_filtered.to_json(),
        "cells_ranked": run.cells_ranked.to_json(),
        "sealed_cells_scanned": run.sealed_cells_scanned.to_json(),
        "returned_items": run.returned_items.to_json(),
    })
}

fn index_lookup_to_json(run: &IndexLookupRun) -> serde_json::Value {
    json!({
        "latency_micros": run.latency_micros.to_json(),
        "candidate_pages": run.candidate_pages.to_json(),
    })
}

fn page_decode_to_json(run: &PageDecodeRun) -> serde_json::Value {
    json!({
        "latency_micros": run.latency_micros.to_json(),
        "pages_decoded": run.pages_decoded,
        "cells_decoded": run.cells_decoded,
    })
}

fn context_build_to_json(run: &ContextBuildRun) -> serde_json::Value {
    json!({
        "latency_micros": run.latency_micros.to_json(),
        "returned_items": run.returned_items.to_json(),
    })
}

impl MetricSamples {
    fn record_elapsed(&mut self, started: Instant) {
        self.samples
            .push(u128_to_u64(started.elapsed().as_micros()));
    }

    fn record_usize(&mut self, value: usize) {
        self.samples.push(u64::try_from(value).unwrap_or(u64::MAX));
    }

    fn record_u64(&mut self, value: u64) {
        self.samples.push(value);
    }

    fn avg(&self) -> u64 {
        if self.samples.is_empty() {
            return 0;
        }
        self.samples.iter().sum::<u64>() / self.samples.len() as u64
    }

    fn to_json(&self) -> serde_json::Value {
        if self.samples.is_empty() {
            return json!({
                "count": 0,
                "avg": 0,
                "p50": 0,
                "p95": 0,
                "min": 0,
                "max": 0,
            });
        }

        let mut sorted = self.samples.clone();
        sorted.sort_unstable();
        json!({
            "count": sorted.len(),
            "avg": self.avg(),
            "p50": percentile(&sorted, 50),
            "p95": percentile(&sorted, 95),
            "min": sorted[0],
            "max": sorted[sorted.len() - 1],
        })
    }
}

fn percentile(sorted: &[u64], percentile: usize) -> u64 {
    if sorted.is_empty() {
        return 0;
    }
    let rank = (sorted.len() * percentile).div_ceil(100).saturating_sub(1);
    sorted[rank.min(sorted.len() - 1)]
}

fn u128_to_u64(value: u128) -> u64 {
    u64::try_from(value).unwrap_or(u64::MAX)
}

#[derive(Debug)]
struct DeterministicRng {
    state: u64,
}

impl DeterministicRng {
    fn new(seed: u64) -> Self {
        Self {
            state: seed ^ 0x9e37_79b9_7f4a_7c15,
        }
    }

    fn next_u64(&mut self) -> u64 {
        self.state = self
            .state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        self.state
    }

    fn next_usize(&mut self, upper_exclusive: usize) -> usize {
        if upper_exclusive == 0 {
            return 0;
        }
        (self.next_u64() as usize) % upper_exclusive
    }
}

fn group_marker(group: usize) -> String {
    format!("bench_group:g{group:03}")
}

fn logical_scope_marker(scope: usize) -> String {
    format!("bench_scope:s{scope:03}")
}

fn page_marker(page: usize) -> String {
    format!("bench_page_marker:p{page:04}")
}

fn page_scope(page: usize) -> String {
    format!("bench_page_{page:04}")
}
