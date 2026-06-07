use std::collections::BTreeSet;
use std::path::PathBuf;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use anyhow::{bail, Result};
use clap::Parser;
use mge_core::{
    CompressionKind, IndexKind, InitOptions, MemoryEngine, MemoryKind, MemoryStatus, MemoryValue,
    PageClustererKind, PageCodecKind, RecallRequest, RememberRequest, SensitivityLevel, TrustLevel,
    DEFAULT_MAX_CELLS_PER_PAGE,
};
use serde_json::json;

#[derive(Debug, Parser)]
#[command(
    name = "mge-synthetic-bench",
    about = "Synthetic exact vs Binary Fuse candidate-index smoke benchmark"
)]
struct Args {
    #[arg(long, default_value_t = 2_000)]
    cells: usize,

    #[arg(long, default_value_t = 100)]
    pages: usize,

    #[arg(long, default_value_t = 8)]
    marker_groups: usize,

    #[arg(long, default_value_t = 8)]
    targeted_queries: usize,

    #[arg(long, default_value_t = 4)]
    noise_queries: usize,

    #[arg(long)]
    store_root: Option<PathBuf>,
}

#[derive(Clone, Debug)]
struct QuerySpec {
    name: String,
    query_text: String,
    query_type: &'static str,
    marker: String,
}

#[derive(Clone, Debug)]
struct QueryRun {
    spec: QuerySpec,
    latency_micros: u128,
    candidate_pages: Vec<u64>,
    pages_loaded: usize,
    sealed_cells_scanned: usize,
    result_count: usize,
    false_positive_candidate_pages: usize,
}

#[derive(Debug)]
struct ModeRun {
    index_kind: IndexKind,
    total_sealed_pages: usize,
    total_cells: usize,
    queries: Vec<QueryRun>,
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

    let queries = synthetic_queries(&args);
    let exact = run_mode(&root, IndexKind::ExactMarkerPage, &args, &queries)?;
    let binary = run_mode(&root, IndexKind::BinaryFusePage, &args, &queries)?;
    let subset_check = exact_candidates_are_subset(&exact, &binary);
    if !subset_check {
        bail!("subset check failed: exact_candidates must be included in binary_fuse_candidates");
    }

    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "store_root": root,
            "synthetic_config": {
                "cells": args.cells,
                "pages": args.pages,
                "marker_groups": args.marker_groups,
                "targeted_queries": args.targeted_queries,
                "noise_queries": args.noise_queries,
            },
            "subset_check": {
                "exact_candidates_subset_of_binary_fuse_candidates": subset_check,
            },
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
    if args.marker_groups == 0 {
        bail!("--marker-groups must be greater than 0");
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

fn synthetic_queries(args: &Args) -> Vec<QuerySpec> {
    let mut queries = Vec::new();
    for index in 0..args.targeted_queries {
        let group = index % args.marker_groups;
        queries.push(QuerySpec {
            name: format!("target_group_{group:03}"),
            query_text: format!("qtarget{index:03}"),
            query_type: "targeted",
            marker: group_marker(group),
        });
    }
    for index in 0..args.noise_queries {
        queries.push(QuerySpec {
            name: format!("noise_{index:03}"),
            query_text: format!("qnoise{index:03}"),
            query_type: "noise",
            marker: format!("bench_missing:noise_{index:03}"),
        });
    }
    queries
}

fn run_mode(
    root: &std::path::Path,
    index_kind: IndexKind,
    args: &Args,
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
        },
    )?;

    remember_synthetic_cells(&mut engine, args)?;
    engine.seal()?;

    let stats = engine.stats()?;
    if stats.sealed_pages != args.pages {
        bail!(
            "{} produced {} sealed pages, expected {}",
            index_kind,
            stats.sealed_pages,
            args.pages
        );
    }

    let mut query_runs = Vec::with_capacity(queries.len());
    for spec in queries {
        let mut request = RecallRequest::new(spec.query_text.clone());
        request.markers = vec![spec.marker.clone()];
        request.max_items = 20;

        let started = Instant::now();
        let packet = engine.recall(request)?;
        let latency_micros = started.elapsed().as_micros();

        query_runs.push(QueryRun {
            spec: spec.clone(),
            latency_micros,
            candidate_pages: packet.debug.candidate_pages,
            pages_loaded: packet.debug.loaded_pages,
            sealed_cells_scanned: packet.debug.sealed_cells_scanned,
            result_count: packet.relevant_memory.len(),
            false_positive_candidate_pages: packet.debug.false_positive_candidate_pages,
        });
    }

    Ok(ModeRun {
        index_kind,
        total_sealed_pages: stats.sealed_pages,
        total_cells: stats.sealed_cells,
        queries: query_runs,
    })
}

fn remember_synthetic_cells(engine: &mut MemoryEngine, args: &Args) -> Result<()> {
    let base_cells_per_page = args.cells / args.pages;
    let extra_cells = args.cells % args.pages;
    let mut cell_index = 0usize;

    for page in 0..args.pages {
        let page_cells = base_cells_per_page + usize::from(page < extra_cells);
        let group = page % args.marker_groups;

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
                group_marker(group),
                page_marker(page),
                format!("bench_bucket:b{:02}", page_cell % 16),
            ];
            engine.remember(request)?;
            cell_index += 1;
        }
    }

    Ok(())
}

fn exact_candidates_are_subset(exact: &ModeRun, binary: &ModeRun) -> bool {
    exact
        .queries
        .iter()
        .zip(&binary.queries)
        .all(|(exact_query, binary_query)| {
            let binary_pages = binary_query
                .candidate_pages
                .iter()
                .copied()
                .collect::<BTreeSet<_>>();
            exact_query
                .candidate_pages
                .iter()
                .all(|page_id| binary_pages.contains(page_id))
        })
}

fn mode_to_json(mode: &ModeRun) -> serde_json::Value {
    let total_latency_micros = mode
        .queries
        .iter()
        .map(|query| query.latency_micros)
        .sum::<u128>();
    let avg_latency_micros = if mode.queries.is_empty() {
        0
    } else {
        total_latency_micros / mode.queries.len() as u128
    };

    json!({
        "index_kind": mode.index_kind,
        "total_sealed_pages": mode.total_sealed_pages,
        "total_cells": mode.total_cells,
        "summary": {
            "queries": mode.queries.len(),
            "total_recall_latency_micros": total_latency_micros,
            "avg_recall_latency_micros": avg_latency_micros,
        },
        "queries": mode.queries.iter().map(query_to_json).collect::<Vec<_>>(),
    })
}

fn query_to_json(query: &QueryRun) -> serde_json::Value {
    json!({
        "name": query.spec.name,
        "query": query.spec.query_text,
        "query_type": query.spec.query_type,
        "marker": query.spec.marker,
        "recall_latency_micros": query.latency_micros,
        "candidate_pages_returned": query.candidate_pages.len(),
        "pages_loaded": query.pages_loaded,
        "sealed_cells_scanned": query.sealed_cells_scanned,
        "result_count": query.result_count,
        "false_positive_candidate_pages_after_page_load": query.false_positive_candidate_pages,
    })
}

fn group_marker(group: usize) -> String {
    format!("bench_group:g{group:03}")
}

fn page_marker(page: usize) -> String {
    format!("bench_page_marker:p{page:04}")
}
