use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

use anyhow::{bail, Context, Result};
use clap::{Parser, ValueEnum};
use mge_core::{
    CompressionKind, DurabilityPolicy, IndexKind, InitOptions, MemoryEngine, MemoryKind,
    MemoryStatus, MemoryValue, PageClustererKind, PageCodecKind, RecallMode, RecallRequest,
    RememberRequest, SecurityMode, SensitivityLevel, TrustLevel,
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

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum BenchProfile {
    Small,
    Medium,
    CodeHeavy,
    DocsHeavy,
    Mixed,
}

#[derive(Debug, Parser)]
#[command(
    name = "mge-corpus-bench",
    about = "Local corpus Memory Genome Engine core benchmark harness"
)]
struct Args {
    #[arg(
        long = "corpus-root",
        visible_alias = "corpus",
        required_unless_present = "generated"
    )]
    corpus_root: Option<PathBuf>,

    #[arg(long)]
    store_root: PathBuf,

    #[arg(long)]
    generated: bool,

    #[arg(long, value_enum, default_value_t = BenchProfile::Mixed)]
    profile: BenchProfile,

    #[arg(long)]
    max_files: Option<usize>,

    #[arg(long)]
    max_bytes: Option<usize>,

    #[arg(long)]
    max_file_bytes: Option<usize>,

    #[arg(long)]
    chunk_bytes: Option<usize>,

    #[arg(long)]
    chunk_lines: Option<usize>,

    #[arg(long)]
    targeted_queries: Option<usize>,

    #[arg(long)]
    noise_queries: Option<usize>,

    #[arg(long)]
    repeats: Option<usize>,

    #[arg(long, default_value_t = 0)]
    seed: u64,
}

#[derive(Clone, Debug)]
struct BenchConfig {
    profile: BenchProfile,
    max_files: usize,
    max_bytes: usize,
    max_file_bytes: usize,
    chunk_bytes: usize,
    chunk_lines: Option<usize>,
    targeted_queries: usize,
    noise_queries: usize,
    repeats: usize,
    seed: u64,
}

#[derive(Clone, Debug, Default)]
struct GeneratedCorpusStats {
    files_written: usize,
    categories: BTreeSet<String>,
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
    let config = BenchConfig::from_args(&args);
    validate_args(&args, &config)?;

    let (corpus_root, generated_corpus) = prepare_corpus_root(&args, &config)?;
    if !args.generated {
        ensure_store_root_is_safe(&corpus_root, &args.store_root)?;
    }

    let (cells, corpus_stats) = load_corpus(&corpus_root, &config)?;
    if cells.is_empty() {
        bail!("corpus produced no importable chunks");
    }
    let queries = build_queries(&cells, &config);
    if queries.is_empty() {
        bail!("corpus produced no benchmark queries");
    }

    let exact = run_mode(
        &args.store_root,
        IndexKind::ExactMarkerPage,
        &config,
        &cells,
        &queries,
    )?;
    let binary = run_mode(
        &args.store_root,
        IndexKind::BinaryFusePage,
        &config,
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
                "profile": config.profile.as_str(),
                "generated": args.generated,
                "seed": config.seed,
                "max_files": config.max_files,
                "max_bytes": config.max_bytes,
                "max_file_bytes": config.max_file_bytes,
                "chunk_bytes": config.chunk_bytes,
                "chunk_lines": config.chunk_lines,
                "targeted_queries": config.targeted_queries,
                "noise_queries": config.noise_queries,
                "repeats": config.repeats,
                "allowed_extensions": ALLOWED_EXTENSIONS,
            },
            "generated_corpus": generated_corpus_to_json(&generated_corpus),
            "corpus": corpus_to_json(&corpus_stats),
            "subset_check": {
                "focused_exact_candidates_subset_of_binary_fuse_candidates": subset_check,
            },
            "comparison": comparison_to_json(&exact, &binary),
            "recommendation": recommendation_to_json(&exact, &binary),
            "modes": [
                mode_to_json(&exact),
                mode_to_json(&binary),
            ],
        }))?
    );

    Ok(())
}

impl BenchProfile {
    fn defaults(self) -> BenchConfig {
        match self {
            Self::Small => BenchConfig {
                profile: self,
                max_files: 18,
                max_bytes: 512 * 1024,
                max_file_bytes: 96 * 1024,
                chunk_bytes: 900,
                chunk_lines: Some(18),
                targeted_queries: 4,
                noise_queries: 1,
                repeats: 2,
                seed: 0,
            },
            Self::Medium => BenchConfig {
                profile: self,
                max_files: 80,
                max_bytes: 4 * 1024 * 1024,
                max_file_bytes: 256 * 1024,
                chunk_bytes: 1_000,
                chunk_lines: Some(24),
                targeted_queries: 8,
                noise_queries: 2,
                repeats: 3,
                seed: 0,
            },
            Self::CodeHeavy => BenchConfig {
                profile: self,
                max_files: 96,
                max_bytes: 4 * 1024 * 1024,
                max_file_bytes: 192 * 1024,
                chunk_bytes: 900,
                chunk_lines: Some(32),
                targeted_queries: 10,
                noise_queries: 2,
                repeats: 3,
                seed: 0,
            },
            Self::DocsHeavy => BenchConfig {
                profile: self,
                max_files: 72,
                max_bytes: 5 * 1024 * 1024,
                max_file_bytes: 384 * 1024,
                chunk_bytes: 1_600,
                chunk_lines: Some(18),
                targeted_queries: 8,
                noise_queries: 2,
                repeats: 3,
                seed: 0,
            },
            Self::Mixed => BenchConfig {
                profile: self,
                max_files: 200,
                max_bytes: 8 * 1024 * 1024,
                max_file_bytes: 512 * 1024,
                chunk_bytes: 1_200,
                chunk_lines: None,
                targeted_queries: 8,
                noise_queries: 2,
                repeats: 3,
                seed: 0,
            },
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Small => "small",
            Self::Medium => "medium",
            Self::CodeHeavy => "code-heavy",
            Self::DocsHeavy => "docs-heavy",
            Self::Mixed => "mixed",
        }
    }
}

impl BenchConfig {
    fn from_args(args: &Args) -> Self {
        let mut config = args.profile.defaults();
        config.max_files = args.max_files.unwrap_or(config.max_files);
        config.max_bytes = args.max_bytes.unwrap_or(config.max_bytes);
        config.max_file_bytes = args.max_file_bytes.unwrap_or(config.max_file_bytes);
        config.chunk_bytes = args.chunk_bytes.unwrap_or(config.chunk_bytes);
        config.chunk_lines = args.chunk_lines.or(config.chunk_lines);
        config.targeted_queries = args.targeted_queries.unwrap_or(config.targeted_queries);
        config.noise_queries = args.noise_queries.unwrap_or(config.noise_queries);
        config.repeats = args.repeats.unwrap_or(config.repeats);
        config.seed = args.seed;
        config
    }
}

fn validate_args(args: &Args, config: &BenchConfig) -> Result<()> {
    if !args.generated && args.corpus_root.is_none() {
        bail!("--corpus-root/--corpus is required unless --generated is used");
    }
    if args.generated && args.corpus_root.is_some() {
        bail!("--generated cannot be combined with --corpus-root/--corpus");
    }
    if config.max_files == 0 {
        bail!("--max-files must be greater than 0");
    }
    if config.max_bytes == 0 {
        bail!("--max-bytes must be greater than 0");
    }
    if config.max_file_bytes == 0 {
        bail!("--max-file-bytes must be greater than 0");
    }
    if config.chunk_bytes < 128 {
        bail!("--chunk-bytes must be at least 128");
    }
    if config.chunk_lines == Some(0) {
        bail!("--chunk-lines must be greater than 0");
    }
    if config.repeats == 0 {
        bail!("--repeats must be greater than 0");
    }
    if config.targeted_queries == 0 && config.noise_queries == 0 {
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

fn prepare_corpus_root(
    args: &Args,
    config: &BenchConfig,
) -> Result<(PathBuf, Option<GeneratedCorpusStats>)> {
    if args.generated {
        let corpus_root = args.store_root.join("generated-corpus");
        let stats = generate_diverse_corpus(&corpus_root, config)?;
        return Ok((corpus_root, Some(stats)));
    }

    let corpus_root = args
        .corpus_root
        .as_ref()
        .context("--corpus-root/--corpus is required")?;
    let corpus_root = fs::canonicalize(corpus_root)
        .with_context(|| format!("failed to canonicalize {}", corpus_root.display()))?;
    Ok((corpus_root, None))
}

fn ensure_store_root_is_safe(corpus_root: &Path, store_root: &Path) -> Result<()> {
    if !corpus_root.is_dir() {
        bail!(
            "--corpus-root must be a directory: {}",
            corpus_root.display()
        );
    }

    let absolute_store_root = if store_root.is_absolute() {
        store_root.to_path_buf()
    } else {
        env::current_dir()?.join(store_root)
    };
    if absolute_store_root.starts_with(corpus_root) {
        bail!("--store-root must be outside --corpus-root to keep corpus files untouched");
    }

    for ancestor in absolute_store_root.ancestors().skip(1) {
        if ancestor.exists() {
            let canonical_ancestor = fs::canonicalize(ancestor)?;
            if canonical_ancestor.starts_with(corpus_root) {
                bail!("--store-root must be outside --corpus-root to keep corpus files untouched");
            }
            break;
        }
    }
    Ok(())
}

fn generate_diverse_corpus(
    corpus_root: &Path,
    config: &BenchConfig,
) -> Result<GeneratedCorpusStats> {
    fs::create_dir_all(corpus_root)?;
    let mut stats = GeneratedCorpusStats::default();
    let target_files = config.max_files.min(match config.profile {
        BenchProfile::Small => 18,
        BenchProfile::Medium => 48,
        BenchProfile::CodeHeavy => 54,
        BenchProfile::DocsHeavy => 42,
        BenchProfile::Mixed => 60,
    });

    let templates: &[(&str, &str, &str)] = match config.profile {
        BenchProfile::CodeHeavy => &[
            ("rust", "src/rust", "rs"),
            ("python", "src/python", "py"),
            ("typescript", "web", "ts"),
            ("javascript", "web", "js"),
            ("config", "config", "toml"),
            ("markdown", "docs", "md"),
            ("noise", "noise", "txt"),
        ],
        BenchProfile::DocsHeavy => &[
            ("markdown", "docs/notes", "md"),
            ("long_text", "docs/long", "txt"),
            ("fragment", "fragments", "md"),
            ("config", "config", "json"),
            ("rust", "src/rust", "rs"),
            ("noise", "noise", "txt"),
        ],
        _ => &[
            ("markdown", "docs/notes", "md"),
            ("rust", "src/rust", "rs"),
            ("python", "src/python", "py"),
            ("typescript", "web", "ts"),
            ("javascript", "web", "js"),
            ("config", "config", "toml"),
            ("long_text", "docs/long", "txt"),
            ("fragment", "fragments", "md"),
            ("noise", "noise", "txt"),
        ],
    };

    for index in 0..target_files {
        let template =
            templates[(index + seed_offset(config.seed, templates.len())) % templates.len()];
        write_generated_file(corpus_root, template, index, config.seed)?;
        stats.files_written += 1;
        stats.categories.insert(template.0.to_string());
    }

    let binary_dir = corpus_root.join("noise");
    fs::create_dir_all(&binary_dir)?;
    fs::write(binary_dir.join("ignored-binary.bin"), [0, 159, 146, 150])?;
    stats.files_written += 1;
    stats.categories.insert("binary_skipped".to_string());

    Ok(stats)
}

fn write_generated_file(
    corpus_root: &Path,
    template: (&str, &str, &str),
    index: usize,
    seed: u64,
) -> Result<()> {
    let (category, dir, extension) = template;
    let dir = corpus_root.join(dir);
    fs::create_dir_all(&dir)?;
    let path = dir.join(format!("{category}-{index:03}.{extension}"));
    let content = generated_content(category, extension, index, seed);
    fs::write(path, content)?;
    Ok(())
}

fn generated_content(category: &str, extension: &str, index: usize, seed: u64) -> String {
    let topic = [
        "marker genome",
        "sealed recall",
        "page catalog",
        "context packet",
        "hot memory",
        "binary fuse",
        "validation rebuild",
        "runtime cache",
    ][(index + seed_offset(seed, 8)) % 8];
    let scope = format!("generated_scope_{}", index % 6);
    let repeated =
        format!("{topic} {topic} {scope} benchmark recall scoring cache candidate page metadata ");

    match (category, extension) {
        ("rust", "rs") => format!(
            "pub fn generated_case_{index}() -> &'static str {{\n    \"{repeated}\"\n}}\n\n#[test]\nfn generated_test_{index}() {{ assert!(generated_case_{index}().contains(\"recall\")); }}\n"
        ),
        ("python", "py") => format!(
            "def generated_case_{index}():\n    value = \"{repeated}\"\n    return value.split()\n\nclass MemoryCase{index}:\n    scope = \"{scope}\"\n"
        ),
        ("typescript", "ts") => format!(
            "export const generatedCase{index} = {{ scope: \"{scope}\", topic: \"{topic}\", text: \"{repeated}\" }};\nexport function recallCase{index}() {{ return generatedCase{index}.text; }}\n"
        ),
        ("javascript", "js") => format!(
            "const generatedCase{index} = {{ scope: \"{scope}\", topic: \"{topic}\", text: \"{repeated}\" }};\nmodule.exports = generatedCase{index};\n"
        ),
        ("config", "toml") => format!(
            "[generated.case_{index}]\nscope = \"{scope}\"\ntopic = \"{topic}\"\nnotes = \"{repeated}\"\n"
        ),
        ("config", "json") => format!(
            "{{\n  \"scope\": \"{scope}\",\n  \"topic\": \"{topic}\",\n  \"notes\": \"{repeated}\"\n}}\n"
        ),
        ("long_text", "txt") => {
            let mut text = String::new();
            for line in 0..48 {
                text.push_str(&format!(
                    "Long document {index} line {line}: {repeated} unrelated background terms alpha beta gamma delta.\n"
                ));
            }
            text
        }
        ("fragment", "md") => (0..18)
            .map(|line| format!("- short note {index}.{line}: {topic} {scope}\n"))
            .collect(),
        ("noise", "txt") => (0..24)
            .map(|line| {
                format!("unrelated noise {index}.{line}: weather music archive random phrase without project markers\n")
            })
            .collect(),
        _ => format!(
            "# Generated note {index}\n\nScope `{scope}` keeps {topic} benchmark material.\n\n{repeated}\n\n## Details\n\n- candidate pages\n- scoring cache\n- context packet\n"
        ),
    }
}

fn seed_offset(seed: u64, modulo: usize) -> usize {
    if modulo == 0 {
        0
    } else {
        usize::try_from(seed % u64::try_from(modulo).unwrap_or(1)).unwrap_or(0)
    }
}

fn load_corpus(corpus_root: &Path, config: &BenchConfig) -> Result<(Vec<CellSpec>, CorpusStats)> {
    let mut stats = CorpusStats::default();
    let mut cells = Vec::new();
    let mut stack = vec![corpus_root.to_path_buf()];

    while let Some(dir) = stack.pop() {
        if stats.files_imported >= config.max_files || stats.bytes_imported >= config.max_bytes {
            break;
        }

        let mut entries = fs::read_dir(&dir)?.collect::<std::result::Result<Vec<_>, _>>()?;
        entries.sort_by_key(|entry| entry.path());

        for entry in entries {
            if stats.files_imported >= config.max_files || stats.bytes_imported >= config.max_bytes
            {
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
            if file_bytes > config.max_file_bytes {
                stats.skipped_oversized_files += 1;
                continue;
            }
            if stats.bytes_imported.saturating_add(file_bytes) > config.max_bytes {
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
            let chunks = chunk_text(&text, config);
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

fn chunk_text(text: &str, config: &BenchConfig) -> Vec<String> {
    if let Some(max_chunk_lines) = config.chunk_lines {
        return chunk_text_by_lines(text, max_chunk_lines, config.chunk_bytes);
    }

    chunk_text_by_bytes(text, config.chunk_bytes)
}

fn chunk_text_by_lines(text: &str, max_chunk_lines: usize, max_chunk_bytes: usize) -> Vec<String> {
    let mut chunks = Vec::new();
    let mut current = String::new();
    let mut current_lines = 0usize;

    for line in text.lines() {
        if line.len() > max_chunk_bytes {
            push_non_empty(&mut chunks, &mut current);
            current_lines = 0;
            split_long_line(line, max_chunk_bytes, &mut chunks);
            continue;
        }

        let additional = line.len() + usize::from(!current.is_empty());
        if (!current.is_empty() && current_lines >= max_chunk_lines)
            || (!current.is_empty() && current.len() + additional > max_chunk_bytes)
        {
            push_non_empty(&mut chunks, &mut current);
            current_lines = 0;
        }
        if !current.is_empty() {
            current.push('\n');
        }
        current.push_str(line);
        current_lines += 1;
    }

    push_non_empty(&mut chunks, &mut current);
    chunks
}

fn chunk_text_by_bytes(text: &str, max_chunk_bytes: usize) -> Vec<String> {
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

fn build_queries(cells: &[CellSpec], config: &BenchConfig) -> Vec<QuerySpec> {
    let mut queries = Vec::new();
    let mut unique_cells = Vec::new();
    let mut seen_markers = BTreeSet::new();
    for cell in cells {
        if !seen_markers.insert(cell.query_marker.clone()) {
            continue;
        }
        unique_cells.push(cell);
    }

    let offset = seed_offset(config.seed, unique_cells.len());
    for index in 0..unique_cells.len() {
        if queries.len() >= config.targeted_queries {
            break;
        }
        let cell = unique_cells[(offset + index) % unique_cells.len()];
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
    for index in 0..config.noise_queries {
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
    config: &BenchConfig,
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
            security_mode: SecurityMode::Unencrypted,
        },
    )?;

    let mut remember_latency_micros = MetricSamples::default();
    for spec in cells {
        let request = remember_request_from_spec(spec);
        let started = Instant::now();
        engine.remember(request)?;
        remember_latency_micros.record_elapsed(started);
    }

    let hot_focused_recall =
        run_recall_bench(&engine, RecallMode::Focused, config.repeats, queries)?;
    let hot_broad_recall = run_recall_bench(&engine, RecallMode::Broad, config.repeats, queries)?;
    let hot_full_scope_recall =
        run_recall_bench(&engine, RecallMode::FullScope, config.repeats, queries)?;

    let mut seal_latency_micros = MetricSamples::default();
    let seal_started = Instant::now();
    engine.seal()?;
    seal_latency_micros.record_elapsed(seal_started);

    let validate_deep_ok = engine.validate_deep()?.ok;
    let rebuild_indexes_ok = engine.rebuild_catalog_and_indexes().is_ok();
    let validate_after_rebuild_ok = engine.validate_deep()?.ok;

    let sealed_repeated_focused_recall =
        run_recall_bench(&engine, RecallMode::Focused, config.repeats, queries)?;
    let sealed_repeated_broad_recall =
        run_recall_bench(&engine, RecallMode::Broad, config.repeats, queries)?;
    let sealed_repeated_full_scope_recall =
        run_recall_bench(&engine, RecallMode::FullScope, config.repeats, queries)?;

    let stats = engine.stats()?;
    let catalog = engine.inspect()?.page_catalog;
    drop(engine);

    let sealed_cold_focused_recall =
        run_cold_recall_bench(&mode_root, RecallMode::Focused, config.repeats, queries)?;
    let sealed_cold_broad_recall =
        run_cold_recall_bench(&mode_root, RecallMode::Broad, config.repeats, queries)?;
    let sealed_cold_full_scope_recall =
        run_cold_recall_bench(&mode_root, RecallMode::FullScope, config.repeats, queries)?;
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

fn generated_corpus_to_json(stats: &Option<GeneratedCorpusStats>) -> serde_json::Value {
    match stats {
        Some(stats) => json!({
            "enabled": true,
            "files_written": stats.files_written,
            "categories": stats.categories,
        }),
        None => json!({
            "enabled": false,
            "files_written": 0,
            "categories": [],
        }),
    }
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

fn recommendation_to_json(exact: &ModeRun, binary: &ModeRun) -> serde_json::Value {
    let hot_focused = exact.hot_focused_recall.total_recall_micros.avg();
    let sealed_cold = exact.sealed_cold_focused_recall.total_recall_micros.avg();
    let sealed_repeated = exact
        .sealed_repeated_focused_recall
        .total_recall_micros
        .avg();
    let binary_repeated = binary
        .sealed_repeated_focused_recall
        .total_recall_micros
        .avg();

    let page_decode = exact
        .sealed_repeated_focused_recall
        .page_decode_micros
        .avg();
    let scoring_cache_build = exact
        .sealed_repeated_focused_recall
        .scoring_cache_build_micros
        .avg();
    let cell_filtering = exact
        .sealed_repeated_focused_recall
        .cell_filtering_micros
        .avg();
    let context_packet_build = exact
        .sealed_repeated_focused_recall
        .context_packet_build_micros
        .avg();

    let page_decode_share = percent_of(page_decode, sealed_repeated);
    let scoring_cache_build_share = percent_of(scoring_cache_build, sealed_repeated);
    let cell_filtering_share = percent_of(cell_filtering, sealed_repeated);
    let context_packet_build_share = percent_of(context_packet_build, sealed_repeated);
    let scoring_filtering_inclusive = scoring_cache_build.max(cell_filtering);
    let scoring_filtering_share = percent_of(scoring_filtering_inclusive, sealed_repeated);
    let repeated_locality_benefit = percent_reduction(sealed_cold, sealed_repeated);
    let binary_fuse_delta = signed_percent_delta(binary_repeated, sealed_repeated);

    let hot_bottleneck = hot_focused > sealed_repeated.saturating_mul(80) / 100;
    let binary_fuse_helped = binary_repeated > 0 && binary_repeated < sealed_repeated * 95 / 100;
    let page_decode_dominates = page_decode_share >= 40;
    let scoring_filtering_dominates = scoring_filtering_share >= 45;
    let context_packet_build_dominates = context_packet_build_share >= 30;

    let repeated_top = top_component(&[
        ("page_decode", page_decode),
        ("scoring_cache_build", scoring_cache_build),
        ("cell_filtering", cell_filtering),
        ("context_packet_build", context_packet_build),
    ]);
    let cold_top = top_component(&[
        (
            "page_decode",
            exact.sealed_cold_focused_recall.page_decode_micros.avg(),
        ),
        (
            "scoring_cache_build",
            exact
                .sealed_cold_focused_recall
                .scoring_cache_build_micros
                .avg(),
        ),
        (
            "cell_filtering",
            exact.sealed_cold_focused_recall.cell_filtering_micros.avg(),
        ),
        (
            "context_packet_build",
            exact
                .sealed_cold_focused_recall
                .context_packet_build_micros
                .avg(),
        ),
    ]);

    let suggested_next_core_step = if hot_bottleneck {
        "measure L1 Hot RAM on a larger real corpus before changing sealed-page policy"
    } else if page_decode_dominates {
        "evaluate decoded page cache policy on real workloads before custom page codec design"
    } else if scoring_filtering_dominates {
        "profile scoring/filtering on real workloads before considering page format changes"
    } else if context_packet_build_dominates {
        "profile ContextPacket construction and output shaping on real workloads"
    } else if repeated_locality_benefit < 20 {
        "collect a larger real corpus benchmark; cache locality is not yet proving much benefit"
    } else {
        "run the same profile against a larger local corpus before changing core architecture"
    };

    let human_summary = vec![
        format!(
            "Hot focused recall avg is {hot_focused} us; sealed repeated focused avg is {sealed_repeated} us."
        ),
        format!(
            "Sealed cold focused avg is {sealed_cold} us; repeated locality benefit is {repeated_locality_benefit}%."
        ),
        format!(
            "Repeated focused shares: page_decode={page_decode_share}%, scoring_cache_build={scoring_cache_build_share}%, cell_filtering={cell_filtering_share}%, context_packet_build={context_packet_build_share}%."
        ),
        format!(
            "BinaryFuse focused repeated delta is {binary_fuse_delta}% vs exact; helped={binary_fuse_helped}."
        ),
        format!("Suggested next core step: {suggested_next_core_step}."),
    ];

    json!({
        "main_bottleneck": repeated_top.0,
        "sealed_cold_bottleneck": cold_top.0,
        "sealed_repeated_bottleneck": repeated_top.0,
        "suggested_next_core_step": suggested_next_core_step,
        "signals": {
            "hot_recall_bottleneck": hot_bottleneck,
            "binary_fuse_helped": binary_fuse_helped,
            "binary_fuse_delta_percent": binary_fuse_delta,
            "page_decode_dominates": page_decode_dominates,
            "scoring_filtering_dominates": scoring_filtering_dominates,
            "context_packet_build_dominates": context_packet_build_dominates,
            "repeated_recall_locality_benefit_percent": repeated_locality_benefit,
        },
        "shares_percent": {
            "sealed_repeated_focused_exact": {
                "page_decode": page_decode_share,
                "scoring_cache_build": scoring_cache_build_share,
                "cell_filtering": cell_filtering_share,
                "scoring_filtering_inclusive": scoring_filtering_share,
                "context_packet_build": context_packet_build_share,
            },
        },
        "workload_shape": {
            "storage_size_bytes": {
                "exact_marker_page": exact.storage_size_bytes,
                "binary_fuse_page": binary.storage_size_bytes,
            },
            "avg_page_size_bytes": {
                "exact_marker_page": exact.avg_encoded_page_bytes,
                "binary_fuse_page": binary.avg_encoded_page_bytes,
            },
            "avg_cells_per_page": {
                "exact_marker_page": exact.avg_cells_per_page,
                "binary_fuse_page": binary.avg_cells_per_page,
            },
        },
        "human_summary": human_summary,
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
    u64::try_from(total.checked_div(count).unwrap_or(0)).unwrap_or(u64::MAX)
}

fn percent_of(part: u64, total: u64) -> u64 {
    part.saturating_mul(100).checked_div(total).unwrap_or(0)
}

fn percent_reduction(before: u64, after: u64) -> i64 {
    if before == 0 {
        return 0;
    }
    let before = i128::from(before);
    let after = i128::from(after);
    i64::try_from(((before - after) * 100) / before).unwrap_or(0)
}

fn signed_percent_delta(value: u64, baseline: u64) -> i64 {
    if baseline == 0 {
        return 0;
    }
    let value = i128::from(value);
    let baseline = i128::from(baseline);
    i64::try_from(((value - baseline) * 100) / baseline).unwrap_or(0)
}

fn top_component<'a>(components: &[(&'a str, u64)]) -> (&'a str, u64) {
    components
        .iter()
        .copied()
        .max_by(|left, right| left.1.cmp(&right.1).then_with(|| right.0.cmp(left.0)))
        .unwrap_or(("none", 0))
}

fn elapsed_micros(started: Instant) -> u64 {
    u64::try_from(started.elapsed().as_micros()).unwrap_or(u64::MAX)
}
