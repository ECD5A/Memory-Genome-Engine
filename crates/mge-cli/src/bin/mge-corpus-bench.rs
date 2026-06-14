use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

use anyhow::{bail, Context, Result};
use clap::Parser;
use mge_core::{
    CompressionKind, DurabilityPolicy, IndexKind, InitOptions, MemoryEngine, MemoryKind,
    MemoryStatus, MemoryValue, PageClustererKind, PageCodecKind, RecallMode, RecallRequest,
    RememberRequest, SensitivityLevel, TrustLevel,
};
use serde_json::json;

const ALLOWED_EXTENSIONS: &[&str] = &["txt", "md", "rs", "toml", "json", "py", "ts", "js"];
const SKIPPED_DIRS: &[&str] = &[
    ".git",
    ".hg",
    ".svn",
    "target",
    "node_modules",
    ".venv",
    "dist",
    "build",
];

#[derive(Debug, Parser)]
#[command(
    name = "mge-corpus-bench",
    about = "Local corpus Memory Genome Engine core benchmark harness"
)]
struct Args {
    #[arg(long)]
    corpus_root: PathBuf,

    #[arg(long)]
    store_root: PathBuf,

    #[arg(long, default_value_t = 200)]
    max_files: usize,

    #[arg(long, default_value_t = 8 * 1024 * 1024)]
    max_bytes: usize,

    #[arg(long, default_value_t = 512 * 1024)]
    max_file_bytes: usize,

    #[arg(long, default_value_t = 1_200)]
    chunk_bytes: usize,

    #[arg(long, default_value_t = 8)]
    targeted_queries: usize,

    #[arg(long, default_value_t = 2)]
    noise_queries: usize,

    #[arg(long, default_value_t = 3)]
    repeats: usize,
}

#[derive(Clone, Debug)]
struct CellSpec {
    scope: String,
    subject: String,
    content: String,
    markers: Vec<String>,
    query_text: String,
    query_marker: String,
}

#[derive(Clone, Debug)]
struct QuerySpec {
    name: String,
    query_text: String,
    marker: String,
    full_scope_scope: String,
}

#[derive(Clone, Debug, Default)]
struct CorpusStats {
    files_imported: usize,
    bytes_imported: usize,
    chunks_created: usize,
    chunk_bytes_total: usize,
    marker_count_total: usize,
    scopes: BTreeSet<String>,
    extensions: BTreeSet<String>,
    skipped_symlinks: usize,
    skipped_dirs: usize,
    skipped_unsupported_extensions: usize,
    skipped_oversized_files: usize,
    skipped_non_utf8_files: usize,
    skipped_empty_files: usize,
    skipped_by_byte_limit: usize,
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
    scoring_cache_build_micros: MetricSamples,
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
    pages_pruned_by_metadata: MetricSamples,
    cells_scanned: MetricSamples,
    cells_decoded: MetricSamples,
    cells_filtered: MetricSamples,
    cells_ranked: MetricSamples,
    sealed_cells_skipped_before_token_scoring: MetricSamples,
    sealed_cells_token_scored: MetricSamples,
    returned_items: MetricSamples,
    decoded_page_cache_hits: MetricSamples,
    decoded_page_cache_misses: MetricSamples,
    scoring_cache_hits: MetricSamples,
    scoring_cache_misses: MetricSamples,
    first_repeat_candidate_pages: BTreeMap<String, Vec<u64>>,
}

#[derive(Debug)]
struct ModeRun {
    index_kind: IndexKind,
    total_cells: usize,
    total_sealed_pages: usize,
    storage_size_bytes: u64,
    avg_cells_per_page: f64,
    avg_encoded_page_bytes: u64,
    remember_latency_micros: MetricSamples,
    seal_latency_micros: MetricSamples,
    hot_focused_recall: RecallBenchRun,
    hot_broad_recall: RecallBenchRun,
    hot_full_scope_recall: RecallBenchRun,
    sealed_cold_focused_recall: RecallBenchRun,
    sealed_cold_broad_recall: RecallBenchRun,
    sealed_cold_full_scope_recall: RecallBenchRun,
    sealed_repeated_focused_recall: RecallBenchRun,
    sealed_repeated_broad_recall: RecallBenchRun,
    sealed_repeated_full_scope_recall: RecallBenchRun,
    validate_deep_ok: bool,
    rebuild_indexes_ok: bool,
    validate_after_rebuild_ok: bool,
}

fn main() -> Result<()> {
    let args = Args::parse();
    validate_args(&args)?;

    let corpus_root = fs::canonicalize(&args.corpus_root)
        .with_context(|| format!("failed to canonicalize {}", args.corpus_root.display()))?;
    ensure_store_root_is_safe(&corpus_root, &args.store_root)?;

    let (cells, corpus_stats) = load_corpus(&corpus_root, &args)?;
    if cells.is_empty() {
        bail!("corpus produced no importable chunks");
    }
    let queries = build_queries(&cells, &args);
    if queries.is_empty() {
        bail!("corpus produced no benchmark queries");
    }

    let exact = run_mode(
        &args.store_root,
        IndexKind::ExactMarkerPage,
        &args,
        &cells,
        &queries,
    )?;
    let binary = run_mode(
        &args.store_root,
        IndexKind::BinaryFusePage,
        &args,
        &cells,
        &queries,
    )?;
    let subset_check = exact_candidates_are_subset(
        &exact.sealed_repeated_focused_recall,
        &binary.sealed_repeated_focused_recall,
    );
    if !subset_check {
        bail!("subset check failed: exact focused candidates must be included in binary_fuse candidates");
    }

    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "corpus_root": corpus_root,
            "store_root": args.store_root,
            "corpus_config": {
                "max_files": args.max_files,
                "max_bytes": args.max_bytes,
                "max_file_bytes": args.max_file_bytes,
                "chunk_bytes": args.chunk_bytes,
                "targeted_queries": args.targeted_queries,
                "noise_queries": args.noise_queries,
                "repeats": args.repeats,
                "allowed_extensions": ALLOWED_EXTENSIONS,
            },
            "corpus": corpus_to_json(&corpus_stats),
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
    if args.max_files == 0 {
        bail!("--max-files must be greater than 0");
    }
    if args.max_bytes == 0 {
        bail!("--max-bytes must be greater than 0");
    }
    if args.max_file_bytes == 0 {
        bail!("--max-file-bytes must be greater than 0");
    }
    if args.chunk_bytes < 128 {
        bail!("--chunk-bytes must be at least 128");
    }
    if args.repeats == 0 {
        bail!("--repeats must be greater than 0");
    }
    if args.targeted_queries == 0 && args.noise_queries == 0 {
        bail!("at least one targeted or noise query is required");
    }
    if args.store_root.exists() {
        bail!(
            "store root already exists: {}; pass a fresh --store-root",
            args.store_root.display()
        );
    }
    Ok(())
}

fn ensure_store_root_is_safe(corpus_root: &Path, store_root: &Path) -> Result<()> {
    if !corpus_root.is_dir() {
        bail!(
            "--corpus-root must be a directory: {}",
            corpus_root.display()
        );
    }

    if let Some(parent) = store_root.parent() {
        if parent.exists() {
            let canonical_parent = fs::canonicalize(parent)?;
            if canonical_parent.starts_with(corpus_root) {
                bail!("--store-root must be outside --corpus-root to keep corpus files untouched");
            }
        }
    }
    Ok(())
}

fn load_corpus(corpus_root: &Path, args: &Args) -> Result<(Vec<CellSpec>, CorpusStats)> {
    let mut stats = CorpusStats::default();
    let mut cells = Vec::new();
    let mut stack = vec![corpus_root.to_path_buf()];

    while let Some(dir) = stack.pop() {
        if stats.files_imported >= args.max_files || stats.bytes_imported >= args.max_bytes {
            break;
        }

        let mut entries = fs::read_dir(&dir)?.collect::<std::result::Result<Vec<_>, _>>()?;
        entries.sort_by_key(|entry| entry.path());

        for entry in entries {
            if stats.files_imported >= args.max_files || stats.bytes_imported >= args.max_bytes {
                break;
            }

            let file_type = entry.file_type()?;
            let path = entry.path();
            if file_type.is_symlink() {
                stats.skipped_symlinks += 1;
                continue;
            }
            if file_type.is_dir() {
                let name = entry.file_name().to_string_lossy().to_string();
                if SKIPPED_DIRS.contains(&name.as_str()) {
                    stats.skipped_dirs += 1;
                    continue;
                }
                stack.push(path);
                continue;
            }
            if !file_type.is_file() {
                continue;
            }

            let Some(extension) = allowed_extension(&path) else {
                stats.skipped_unsupported_extensions += 1;
                continue;
            };

            let metadata = entry.metadata()?;
            let file_bytes = usize::try_from(metadata.len()).unwrap_or(usize::MAX);
            if file_bytes == 0 {
                stats.skipped_empty_files += 1;
                continue;
            }
            if file_bytes > args.max_file_bytes {
                stats.skipped_oversized_files += 1;
                continue;
            }
            if stats.bytes_imported.saturating_add(file_bytes) > args.max_bytes {
                stats.skipped_by_byte_limit += 1;
                continue;
            }

            let bytes = fs::read(&path)?;
            let Ok(text) = String::from_utf8(bytes) else {
                stats.skipped_non_utf8_files += 1;
                continue;
            };

            let rel_path = path
                .strip_prefix(corpus_root)
                .unwrap_or(&path)
                .to_path_buf();
            let chunks = chunk_text(&text, args.chunk_bytes);
            if chunks.is_empty() {
                stats.skipped_empty_files += 1;
                continue;
            }

            stats.files_imported += 1;
            stats.bytes_imported += file_bytes;
            stats.extensions.insert(extension.clone());

            let file_stem = path
                .file_stem()
                .map(|stem| slugify(&stem.to_string_lossy()))
                .unwrap_or_else(|| "file".to_string());
            let dir_slug = rel_path
                .parent()
                .map(|parent| slugify(&parent.to_string_lossy()))
                .unwrap_or_else(|| "root".to_string());
            let path_slug = slugify(&rel_path.to_string_lossy());
            let scope = format!("corpus:{extension}:{dir_slug}");
            stats.scopes.insert(scope.clone());

            for (chunk_index, chunk) in chunks.into_iter().enumerate() {
                let file_marker = format!("corpus_file:{file_stem}");
                let markers = vec![
                    format!("corpus_ext:{extension}"),
                    file_marker.clone(),
                    format!("corpus_dir:{dir_slug}"),
                    format!("corpus_path:{path_slug}"),
                ];
                stats.chunks_created += 1;
                stats.chunk_bytes_total += chunk.len();
                stats.marker_count_total += markers.len();
                cells.push(CellSpec {
                    scope: scope.clone(),
                    subject: format!("{} chunk {}", rel_path.display(), chunk_index),
                    content: format!(
                        "corpus file: {}\nextension: {}\nchunk: {}\n\n{}",
                        rel_path.display(),
                        extension,
                        chunk_index,
                        chunk
                    ),
                    markers,
                    query_text: format!("{} {}", file_stem.replace('_', " "), extension),
                    query_marker: file_marker,
                });
            }
        }
    }

    Ok((cells, stats))
}

fn allowed_extension(path: &Path) -> Option<String> {
    let extension = path.extension()?.to_string_lossy().to_ascii_lowercase();
    ALLOWED_EXTENSIONS
        .contains(&extension.as_str())
        .then_some(extension)
}

fn chunk_text(text: &str, max_chunk_bytes: usize) -> Vec<String> {
    let mut chunks = Vec::new();
    let mut current = String::new();

    for line in text.lines() {
        if line.len() > max_chunk_bytes {
            push_non_empty(&mut chunks, &mut current);
            split_long_line(line, max_chunk_bytes, &mut chunks);
            continue;
        }

        let additional = line.len() + usize::from(!current.is_empty());
        if !current.is_empty() && current.len() + additional > max_chunk_bytes {
            push_non_empty(&mut chunks, &mut current);
        }
        if !current.is_empty() {
            current.push('\n');
        }
        current.push_str(line);
    }

    push_non_empty(&mut chunks, &mut current);
    chunks
}

fn push_non_empty(chunks: &mut Vec<String>, current: &mut String) {
    if !current.trim().is_empty() {
        chunks.push(std::mem::take(current));
    } else {
        current.clear();
    }
}

fn split_long_line(line: &str, max_chunk_bytes: usize, chunks: &mut Vec<String>) {
    let mut start = 0;
    while start < line.len() {
        let mut end = (start + max_chunk_bytes).min(line.len());
        while end > start && !line.is_char_boundary(end) {
            end -= 1;
        }
        if end == start {
            end = line[start..]
                .char_indices()
                .nth(1)
                .map(|(index, _)| start + index)
                .unwrap_or(line.len());
        }
        let chunk = line[start..end].trim();
        if !chunk.is_empty() {
            chunks.push(chunk.to_string());
        }
        start = end;
    }
}

fn build_queries(cells: &[CellSpec], args: &Args) -> Vec<QuerySpec> {
    let mut queries = Vec::new();
    let mut seen_markers = BTreeSet::new();
    for cell in cells {
        if queries.len() >= args.targeted_queries {
            break;
        }
        if !seen_markers.insert(cell.query_marker.clone()) {
            continue;
        }
        queries.push(QuerySpec {
            name: format!("target_{:03}", queries.len()),
            query_text: cell.query_text.clone(),
            marker: cell.query_marker.clone(),
            full_scope_scope: cell.scope.clone(),
        });
    }

    let fallback_scope = cells
        .first()
        .map(|cell| cell.scope.clone())
        .unwrap_or_else(|| "corpus:empty".to_string());
    for index in 0..args.noise_queries {
        queries.push(QuerySpec {
            name: format!("noise_{index:03}"),
            query_text: format!("corpus unmatched noise {index:03}"),
            marker: format!("corpus_missing:noise_{index:03}"),
            full_scope_scope: fallback_scope.clone(),
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
            durability: DurabilityPolicy::Balanced,
        },
    )?;

    let mut remember_latency_micros = MetricSamples::default();
    for spec in cells {
        let request = remember_request_from_spec(spec);
        let started = Instant::now();
        engine.remember(request)?;
        remember_latency_micros.record_elapsed(started);
    }

    let hot_focused_recall = run_recall_bench(&engine, RecallMode::Focused, args.repeats, queries)?;
    let hot_broad_recall = run_recall_bench(&engine, RecallMode::Broad, args.repeats, queries)?;
    let hot_full_scope_recall =
        run_recall_bench(&engine, RecallMode::FullScope, args.repeats, queries)?;

    let mut seal_latency_micros = MetricSamples::default();
    let seal_started = Instant::now();
    engine.seal()?;
    seal_latency_micros.record_elapsed(seal_started);

    let validate_deep_ok = engine.validate_deep()?.ok;
    let rebuild_indexes_ok = engine.rebuild_catalog_and_indexes().is_ok();
    let validate_after_rebuild_ok = engine.validate_deep()?.ok;

    let sealed_cold_focused_recall =
        run_cold_recall_bench(&mode_root, RecallMode::Focused, args.repeats, queries)?;
    let sealed_cold_broad_recall =
        run_cold_recall_bench(&mode_root, RecallMode::Broad, args.repeats, queries)?;
    let sealed_cold_full_scope_recall =
        run_cold_recall_bench(&mode_root, RecallMode::FullScope, args.repeats, queries)?;
    let sealed_repeated_focused_recall =
        run_recall_bench(&engine, RecallMode::Focused, args.repeats, queries)?;
    let sealed_repeated_broad_recall =
        run_recall_bench(&engine, RecallMode::Broad, args.repeats, queries)?;
    let sealed_repeated_full_scope_recall =
        run_recall_bench(&engine, RecallMode::FullScope, args.repeats, queries)?;

    let stats = engine.stats()?;
    let catalog = engine.inspect()?.page_catalog;
    let encoded_total = catalog
        .pages
        .iter()
        .map(|entry| entry.encoded_size_bytes)
        .sum::<u64>();
    let avg_encoded_page_bytes = if catalog.pages.is_empty() {
        0
    } else {
        encoded_total / u64::try_from(catalog.pages.len()).unwrap_or(1)
    };
    let avg_cells_per_page = if stats.sealed_pages == 0 {
        0.0
    } else {
        stats.sealed_cells as f64 / stats.sealed_pages as f64
    };

    Ok(ModeRun {
        index_kind,
        total_cells: stats.sealed_cells,
        total_sealed_pages: stats.sealed_pages,
        storage_size_bytes: stats.store_size_bytes,
        avg_cells_per_page,
        avg_encoded_page_bytes,
        remember_latency_micros,
        seal_latency_micros,
        hot_focused_recall,
        hot_broad_recall,
        hot_full_scope_recall,
        sealed_cold_focused_recall,
        sealed_cold_broad_recall,
        sealed_cold_full_scope_recall,
        sealed_repeated_focused_recall,
        sealed_repeated_broad_recall,
        sealed_repeated_full_scope_recall,
        validate_deep_ok,
        rebuild_indexes_ok,
        validate_after_rebuild_ok,
    })
}

fn remember_request_from_spec(spec: &CellSpec) -> RememberRequest {
    let mut request = RememberRequest::new(
        MemoryKind::ProjectFact,
        MemoryValue::Text(spec.content.clone()),
    );
    request.subject = Some(spec.subject.clone());
    request.scope = spec.scope.clone();
    request.status = MemoryStatus::Active;
    request.trust = TrustLevel::ToolObserved;
    request.sensitivity = SensitivityLevel::Public;
    request.markers = spec.markers.clone();
    request
}

fn run_cold_recall_bench(
    root: &Path,
    recall_mode: RecallMode,
    repeats: usize,
    queries: &[QuerySpec],
) -> Result<RecallBenchRun> {
    let mut run = RecallBenchRun::new(recall_mode);
    for repeat in 0..repeats {
        for spec in queries {
            let engine = MemoryEngine::open_at(root)?;
            let packet = recall_once(&engine, recall_mode, spec)?;
            run.record_packet(&packet, repeat, spec);
        }
    }
    Ok(run)
}

fn run_recall_bench(
    engine: &MemoryEngine,
    recall_mode: RecallMode,
    repeats: usize,
    queries: &[QuerySpec],
) -> Result<RecallBenchRun> {
    let mut run = RecallBenchRun::new(recall_mode);
    for repeat in 0..repeats {
        for spec in queries {
            let packet = recall_once(engine, recall_mode, spec)?;
            run.record_packet(&packet, repeat, spec);
        }
    }
    Ok(run)
}

fn recall_once(
    engine: &MemoryEngine,
    recall_mode: RecallMode,
    spec: &QuerySpec,
) -> Result<mge_core::ContextPacket> {
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
    let mut packet = engine.recall(request)?;
    let outer_latency = elapsed_micros(started);
    if packet.debug.total_recall_micros == 0 {
        packet.debug.total_recall_micros = outer_latency;
    }
    Ok(packet)
}

impl RecallBenchRun {
    fn new(recall_mode: RecallMode) -> Self {
        Self {
            recall_mode,
            latency_micros: MetricSamples::default(),
            query_marker_extraction_micros: MetricSamples::default(),
            hot_memory_lookup_micros: MetricSamples::default(),
            candidate_page_index_lookup_micros: MetricSamples::default(),
            page_file_read_load_micros: MetricSamples::default(),
            page_decode_micros: MetricSamples::default(),
            scoring_cache_build_micros: MetricSamples::default(),
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
            pages_pruned_by_metadata: MetricSamples::default(),
            cells_scanned: MetricSamples::default(),
            cells_decoded: MetricSamples::default(),
            cells_filtered: MetricSamples::default(),
            cells_ranked: MetricSamples::default(),
            sealed_cells_skipped_before_token_scoring: MetricSamples::default(),
            sealed_cells_token_scored: MetricSamples::default(),
            returned_items: MetricSamples::default(),
            decoded_page_cache_hits: MetricSamples::default(),
            decoded_page_cache_misses: MetricSamples::default(),
            scoring_cache_hits: MetricSamples::default(),
            scoring_cache_misses: MetricSamples::default(),
            first_repeat_candidate_pages: BTreeMap::new(),
        }
    }

    fn record_packet(&mut self, packet: &mge_core::ContextPacket, repeat: usize, spec: &QuerySpec) {
        self.latency_micros
            .record_u64(packet.debug.total_recall_micros);
        self.candidate_pages
            .record_usize(packet.debug.candidate_pages.len());
        self.query_marker_extraction_micros
            .record_u64(packet.debug.query_marker_extraction_micros);
        self.hot_memory_lookup_micros
            .record_u64(packet.debug.hot_memory_lookup_micros);
        self.candidate_page_index_lookup_micros
            .record_u64(packet.debug.candidate_page_index_lookup_micros);
        self.page_file_read_load_micros
            .record_u64(packet.debug.page_file_read_load_micros);
        self.page_decode_micros
            .record_u64(packet.debug.page_decode_micros);
        self.scoring_cache_build_micros
            .record_u64(packet.debug.scoring_cache_build_micros);
        self.cell_filtering_micros
            .record_u64(packet.debug.cell_filtering_micros);
        self.reranking_micros
            .record_u64(packet.debug.reranking_micros);
        self.context_packet_build_micros
            .record_u64(packet.debug.context_packet_build_micros);
        self.total_recall_micros
            .record_u64(packet.debug.total_recall_micros);
        self.hot_total_cells
            .record_usize(packet.debug.hot_total_cells);
        self.hot_candidate_cells
            .record_usize(packet.debug.hot_candidate_cells);
        self.hot_cells_scanned
            .record_usize(packet.debug.hot_cells_scanned);
        self.pages_considered
            .record_usize(packet.debug.pages_considered);
        self.pages_loaded.record_usize(packet.debug.loaded_pages);
        self.pages_pruned_by_metadata
            .record_usize(packet.debug.pages_pruned_by_metadata);
        self.cells_scanned.record_usize(packet.debug.cells_scanned);
        self.cells_decoded.record_usize(packet.debug.cells_decoded);
        self.cells_filtered
            .record_usize(packet.debug.cells_filtered);
        self.cells_ranked.record_usize(packet.debug.cells_ranked);
        self.sealed_cells_skipped_before_token_scoring
            .record_usize(packet.debug.sealed_cells_skipped_before_token_scoring);
        self.sealed_cells_token_scored
            .record_usize(packet.debug.sealed_cells_token_scored);
        self.returned_items
            .record_usize(packet.debug.returned_items);
        self.decoded_page_cache_hits
            .record_usize(packet.debug.decoded_page_cache_hits);
        self.decoded_page_cache_misses
            .record_usize(packet.debug.decoded_page_cache_misses);
        self.scoring_cache_hits
            .record_usize(packet.debug.scoring_cache_hits);
        self.scoring_cache_misses
            .record_usize(packet.debug.scoring_cache_misses);

        if repeat == 0 {
            self.first_repeat_candidate_pages
                .insert(spec.name.clone(), packet.debug.candidate_pages.clone());
        }
    }
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

fn corpus_to_json(stats: &CorpusStats) -> serde_json::Value {
    json!({
        "files_imported": stats.files_imported,
        "bytes_imported": stats.bytes_imported,
        "chunks_created": stats.chunks_created,
        "avg_chunk_bytes": average_usize(stats.chunk_bytes_total, stats.chunks_created),
        "avg_markers_per_cell": average_usize(stats.marker_count_total, stats.chunks_created),
        "scopes_count": stats.scopes.len(),
        "extensions_count": stats.extensions.len(),
        "extensions": stats.extensions,
        "skipped": {
            "symlinks": stats.skipped_symlinks,
            "dirs": stats.skipped_dirs,
            "unsupported_extensions": stats.skipped_unsupported_extensions,
            "oversized_files": stats.skipped_oversized_files,
            "non_utf8_files": stats.skipped_non_utf8_files,
            "empty_files": stats.skipped_empty_files,
            "by_byte_limit": stats.skipped_by_byte_limit,
        },
    })
}

fn mode_to_json(mode: &ModeRun) -> serde_json::Value {
    json!({
        "index_kind": mode.index_kind,
        "total_cells": mode.total_cells,
        "total_sealed_pages": mode.total_sealed_pages,
        "storage_size_bytes": mode.storage_size_bytes,
        "avg_cells_per_page": mode.avg_cells_per_page,
        "avg_encoded_page_bytes": mode.avg_encoded_page_bytes,
        "validation": {
            "validate_deep_ok": mode.validate_deep_ok,
            "rebuild_indexes_ok": mode.rebuild_indexes_ok,
            "validate_after_rebuild_ok": mode.validate_after_rebuild_ok,
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
        "sealed_recall_modes": {
            "cold": {
                "focused": recall_to_json(&mode.sealed_cold_focused_recall),
                "broad": recall_to_json(&mode.sealed_cold_broad_recall),
                "full_scope": recall_to_json(&mode.sealed_cold_full_scope_recall),
            },
            "repeated": {
                "focused": recall_to_json(&mode.sealed_repeated_focused_recall),
                "broad": recall_to_json(&mode.sealed_repeated_broad_recall),
                "full_scope": recall_to_json(&mode.sealed_repeated_full_scope_recall),
            },
        },
    })
}

fn comparison_to_json(exact: &ModeRun, binary: &ModeRun) -> serde_json::Value {
    json!({
        "remember_avg_micros": pair_json(&exact.remember_latency_micros, &binary.remember_latency_micros),
        "seal_avg_micros": pair_json(&exact.seal_latency_micros, &binary.seal_latency_micros),
        "hot_avg_micros": recall_totals_by_mode_json(
            &exact.hot_focused_recall,
            &exact.hot_broad_recall,
            &exact.hot_full_scope_recall,
            &binary.hot_focused_recall,
            &binary.hot_broad_recall,
            &binary.hot_full_scope_recall,
        ),
        "sealed_cold_avg_micros": recall_totals_by_mode_json(
            &exact.sealed_cold_focused_recall,
            &exact.sealed_cold_broad_recall,
            &exact.sealed_cold_full_scope_recall,
            &binary.sealed_cold_focused_recall,
            &binary.sealed_cold_broad_recall,
            &binary.sealed_cold_full_scope_recall,
        ),
        "sealed_repeated_avg_micros": recall_totals_by_mode_json(
            &exact.sealed_repeated_focused_recall,
            &exact.sealed_repeated_broad_recall,
            &exact.sealed_repeated_full_scope_recall,
            &binary.sealed_repeated_focused_recall,
            &binary.sealed_repeated_broad_recall,
            &binary.sealed_repeated_full_scope_recall,
        ),
        "sealed_repeated_timing_avg_micros": {
            "focused": recall_timing_pair_json(
                &exact.sealed_repeated_focused_recall,
                &binary.sealed_repeated_focused_recall,
            ),
            "broad": recall_timing_pair_json(
                &exact.sealed_repeated_broad_recall,
                &binary.sealed_repeated_broad_recall,
            ),
            "full_scope": recall_timing_pair_json(
                &exact.sealed_repeated_full_scope_recall,
                &binary.sealed_repeated_full_scope_recall,
            ),
        },
        "sealed_repeated_locality": {
            "focused": recall_locality_pair_json(
                &exact.sealed_repeated_focused_recall,
                &binary.sealed_repeated_focused_recall,
            ),
            "broad": recall_locality_pair_json(
                &exact.sealed_repeated_broad_recall,
                &binary.sealed_repeated_broad_recall,
            ),
            "full_scope": recall_locality_pair_json(
                &exact.sealed_repeated_full_scope_recall,
                &binary.sealed_repeated_full_scope_recall,
            ),
        },
        "top_bottlenecks_avg_micros": {
            "exact_marker_page": {
                "hot_focused": top_bottlenecks_json(&exact.hot_focused_recall),
                "sealed_cold_focused": top_bottlenecks_json(&exact.sealed_cold_focused_recall),
                "sealed_repeated_focused": top_bottlenecks_json(&exact.sealed_repeated_focused_recall),
                "sealed_repeated_broad": top_bottlenecks_json(&exact.sealed_repeated_broad_recall),
            },
            "binary_fuse_page": {
                "hot_focused": top_bottlenecks_json(&binary.hot_focused_recall),
                "sealed_cold_focused": top_bottlenecks_json(&binary.sealed_cold_focused_recall),
                "sealed_repeated_focused": top_bottlenecks_json(&binary.sealed_repeated_focused_recall),
                "sealed_repeated_broad": top_bottlenecks_json(&binary.sealed_repeated_broad_recall),
            },
        },
        "sealed_cold_focused_avg_micros": pair_json(&exact.sealed_cold_focused_recall.total_recall_micros, &binary.sealed_cold_focused_recall.total_recall_micros),
        "sealed_repeated_focused_avg_micros": pair_json(&exact.sealed_repeated_focused_recall.total_recall_micros, &binary.sealed_repeated_focused_recall.total_recall_micros),
        "sealed_repeated_broad_avg_micros": pair_json(&exact.sealed_repeated_broad_recall.total_recall_micros, &binary.sealed_repeated_broad_recall.total_recall_micros),
        "page_decode_avg_micros": pair_json(&exact.sealed_repeated_focused_recall.page_decode_micros, &binary.sealed_repeated_focused_recall.page_decode_micros),
        "scoring_cache_build_avg_micros": pair_json(&exact.sealed_repeated_focused_recall.scoring_cache_build_micros, &binary.sealed_repeated_focused_recall.scoring_cache_build_micros),
        "cell_filtering_avg_micros": pair_json(&exact.sealed_repeated_focused_recall.cell_filtering_micros, &binary.sealed_repeated_focused_recall.cell_filtering_micros),
        "context_packet_build_avg_micros": pair_json(&exact.sealed_repeated_focused_recall.context_packet_build_micros, &binary.sealed_repeated_focused_recall.context_packet_build_micros),
        "page_shape": {
            "avg_cells_per_page": {
                "exact_marker_page": exact.avg_cells_per_page,
                "binary_fuse_page": binary.avg_cells_per_page,
            },
            "avg_encoded_page_bytes": {
                "exact_marker_page": exact.avg_encoded_page_bytes,
                "binary_fuse_page": binary.avg_encoded_page_bytes,
            },
        },
        "storage_size_bytes": {
            "exact_marker_page": exact.storage_size_bytes,
            "binary_fuse_page": binary.storage_size_bytes,
        },
    })
}

fn recall_totals_by_mode_json(
    exact_focused: &RecallBenchRun,
    exact_broad: &RecallBenchRun,
    exact_full_scope: &RecallBenchRun,
    binary_focused: &RecallBenchRun,
    binary_broad: &RecallBenchRun,
    binary_full_scope: &RecallBenchRun,
) -> serde_json::Value {
    json!({
        "focused": pair_json(&exact_focused.total_recall_micros, &binary_focused.total_recall_micros),
        "broad": pair_json(&exact_broad.total_recall_micros, &binary_broad.total_recall_micros),
        "full_scope": pair_json(&exact_full_scope.total_recall_micros, &binary_full_scope.total_recall_micros),
    })
}

fn recall_timing_pair_json(exact: &RecallBenchRun, binary: &RecallBenchRun) -> serde_json::Value {
    json!({
        "total_recall": pair_json(&exact.total_recall_micros, &binary.total_recall_micros),
        "query_marker_extraction": pair_json(&exact.query_marker_extraction_micros, &binary.query_marker_extraction_micros),
        "hot_memory_lookup": pair_json(&exact.hot_memory_lookup_micros, &binary.hot_memory_lookup_micros),
        "candidate_page_index_lookup": pair_json(&exact.candidate_page_index_lookup_micros, &binary.candidate_page_index_lookup_micros),
        "page_file_read_load": pair_json(&exact.page_file_read_load_micros, &binary.page_file_read_load_micros),
        "page_decode": pair_json(&exact.page_decode_micros, &binary.page_decode_micros),
        "scoring_cache_build": pair_json(&exact.scoring_cache_build_micros, &binary.scoring_cache_build_micros),
        "cell_filtering": pair_json(&exact.cell_filtering_micros, &binary.cell_filtering_micros),
        "reranking": pair_json(&exact.reranking_micros, &binary.reranking_micros),
        "context_packet_build": pair_json(&exact.context_packet_build_micros, &binary.context_packet_build_micros),
    })
}

fn recall_locality_pair_json(exact: &RecallBenchRun, binary: &RecallBenchRun) -> serde_json::Value {
    json!({
        "decoded_page_cache_hits": pair_json(&exact.decoded_page_cache_hits, &binary.decoded_page_cache_hits),
        "decoded_page_cache_misses": pair_json(&exact.decoded_page_cache_misses, &binary.decoded_page_cache_misses),
        "scoring_cache_hits": pair_json(&exact.scoring_cache_hits, &binary.scoring_cache_hits),
        "scoring_cache_misses": pair_json(&exact.scoring_cache_misses, &binary.scoring_cache_misses),
        "pages_loaded": pair_json(&exact.pages_loaded, &binary.pages_loaded),
        "pages_pruned_by_metadata": pair_json(&exact.pages_pruned_by_metadata, &binary.pages_pruned_by_metadata),
        "cells_decoded": pair_json(&exact.cells_decoded, &binary.cells_decoded),
        "cells_ranked": pair_json(&exact.cells_ranked, &binary.cells_ranked),
        "sealed_cells_skipped_before_token_scoring": pair_json(
            &exact.sealed_cells_skipped_before_token_scoring,
            &binary.sealed_cells_skipped_before_token_scoring,
        ),
        "sealed_cells_token_scored": pair_json(
            &exact.sealed_cells_token_scored,
            &binary.sealed_cells_token_scored,
        ),
        "returned_items": pair_json(&exact.returned_items, &binary.returned_items),
    })
}

fn top_bottlenecks_json(run: &RecallBenchRun) -> serde_json::Value {
    let mut bottlenecks = vec![
        (
            "query_marker_extraction",
            run.query_marker_extraction_micros.avg(),
        ),
        ("hot_memory_lookup", run.hot_memory_lookup_micros.avg()),
        (
            "candidate_page_index_lookup",
            run.candidate_page_index_lookup_micros.avg(),
        ),
        ("page_file_read_load", run.page_file_read_load_micros.avg()),
        ("page_decode", run.page_decode_micros.avg()),
        ("scoring_cache_build", run.scoring_cache_build_micros.avg()),
        ("cell_filtering", run.cell_filtering_micros.avg()),
        ("reranking", run.reranking_micros.avg()),
        (
            "context_packet_build",
            run.context_packet_build_micros.avg(),
        ),
    ];
    bottlenecks.sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(right.0)));
    json!(bottlenecks
        .into_iter()
        .filter(|(_, avg_micros)| *avg_micros > 0)
        .take(5)
        .map(|(component, avg_micros)| json!({
            "component": component,
            "avg_micros": avg_micros,
        }))
        .collect::<Vec<_>>())
}

fn pair_json(exact: &MetricSamples, binary: &MetricSamples) -> serde_json::Value {
    json!({
        "exact_marker_page": exact.avg(),
        "binary_fuse_page": binary.avg(),
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
            "scoring_cache_build": run.scoring_cache_build_micros.to_json(),
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
        "pages_pruned_by_metadata": run.pages_pruned_by_metadata.to_json(),
        "cells_scanned": run.cells_scanned.to_json(),
        "cells_decoded": run.cells_decoded.to_json(),
        "cells_filtered": run.cells_filtered.to_json(),
        "cells_ranked": run.cells_ranked.to_json(),
        "sealed_cells_skipped_before_token_scoring": run.sealed_cells_skipped_before_token_scoring.to_json(),
        "sealed_cells_token_scored": run.sealed_cells_token_scored.to_json(),
        "returned_items": run.returned_items.to_json(),
        "decoded_page_cache_hits": run.decoded_page_cache_hits.to_json(),
        "decoded_page_cache_misses": run.decoded_page_cache_misses.to_json(),
        "scoring_cache_hits": run.scoring_cache_hits.to_json(),
        "scoring_cache_misses": run.scoring_cache_misses.to_json(),
    })
}

impl MetricSamples {
    fn record_elapsed(&mut self, started: Instant) {
        self.record_u64(elapsed_micros(started));
    }

    fn record_u64(&mut self, value: u64) {
        self.samples.push(value);
    }

    fn record_usize(&mut self, value: usize) {
        self.record_u64(u64::try_from(value).unwrap_or(u64::MAX));
    }

    fn avg(&self) -> u64 {
        if self.samples.is_empty() {
            return 0;
        }
        self.samples.iter().sum::<u64>() / u64::try_from(self.samples.len()).unwrap_or(1)
    }

    fn percentile(&self, numerator: usize, denominator: usize) -> u64 {
        if self.samples.is_empty() {
            return 0;
        }
        let mut sorted = self.samples.clone();
        sorted.sort_unstable();
        let index = ((sorted.len() - 1) * numerator).div_ceil(denominator);
        sorted[index.min(sorted.len() - 1)]
    }

    fn to_json(&self) -> serde_json::Value {
        if self.samples.is_empty() {
            return json!({
                "count": 0,
                "min": 0,
                "max": 0,
                "avg": 0,
                "p50": 0,
                "p95": 0,
            });
        }
        json!({
            "count": self.samples.len(),
            "min": self.samples.iter().min().copied().unwrap_or(0),
            "max": self.samples.iter().max().copied().unwrap_or(0),
            "avg": self.avg(),
            "p50": self.percentile(50, 100),
            "p95": self.percentile(95, 100),
        })
    }
}

fn slugify(input: &str) -> String {
    let mut output = String::new();
    let mut last_was_separator = false;
    for ch in input.chars().flat_map(|ch| ch.to_lowercase()) {
        if ch.is_ascii_alphanumeric() {
            output.push(ch);
            last_was_separator = false;
        } else if !last_was_separator {
            output.push('_');
            last_was_separator = true;
        }
        if output.len() >= 96 {
            break;
        }
    }
    let output = output.trim_matches('_').to_string();
    if output.is_empty() {
        "unknown".to_string()
    } else {
        output
    }
}

fn average_usize(total: usize, count: usize) -> u64 {
    if count == 0 {
        0
    } else {
        u64::try_from(total / count).unwrap_or(u64::MAX)
    }
}

fn elapsed_micros(started: Instant) -> u64 {
    u64::try_from(started.elapsed().as_micros()).unwrap_or(u64::MAX)
}
