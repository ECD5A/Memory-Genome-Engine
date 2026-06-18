// Memory Genome Engine
// Copyright (c) 2026 ECD5A
// Project: https://github.com/ECD5A/Memory-Genome-Engine
//
// Licensed under the Apache License, Version 2.0.

use std::collections::{BTreeSet, HashMap, HashSet};
use std::fs;
use std::path::PathBuf;
use std::str::FromStr;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use anyhow::{anyhow, Context, Result};
use clap::{Parser, ValueEnum};
use mge_core::{
    CompressionKind, DurabilityPolicy, IndexKind, InitOptions, MemoryEngine, MemoryKind,
    MemorySource, MemoryStatus, MemoryValue, PageClustererKind, PageCodecKind, RecallMode,
    RecallRequest, RememberRequest, SecurityMode, SensitivityLevel, TrustLevel,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Parser)]
#[command(
    name = "mge-agent-memory-eval",
    about = "Developer-only agent memory evaluation harness for Memory Genome Engine"
)]
struct Args {
    /// Optional neutral JSON dataset. If omitted, a deterministic generated dataset is used.
    #[arg(long)]
    input: Option<PathBuf>,

    /// Generated dataset profile.
    #[arg(long, value_enum, default_value_t = GeneratedProfile::Small)]
    profile: GeneratedProfile,

    /// Temporary/evaluation store root. If omitted, a unique temp directory is used.
    #[arg(long)]
    store_root: Option<PathBuf>,

    /// Recall result budget.
    #[arg(long, default_value_t = 5)]
    top_k: usize,

    /// Repeat each query this many times for timing.
    #[arg(long, default_value_t = 2)]
    repeats: usize,

    /// Which MGE sealed-page index kind to evaluate.
    #[arg(long, value_enum, default_value_t = IndexSelection::Both)]
    index: IndexSelection,

    /// Which recall modes to evaluate.
    #[arg(long, value_enum, default_value_t = ModeSelection::FocusedBroad)]
    modes: ModeSelection,

    /// Output format.
    #[arg(long, value_enum, default_value_t = OutputFormat::Text)]
    output: OutputFormat,

    /// Keep the generated/evaluation store after the run.
    #[arg(long)]
    keep_store: bool,

    /// Print the neutral EvalDataset JSON shape and exit.
    #[arg(long)]
    print_schema: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum GeneratedProfile {
    Tiny,
    Small,
    Medium,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum IndexSelection {
    Exact,
    BinaryFuse,
    Both,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum ModeSelection {
    Focused,
    Broad,
    FocusedBroad,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum OutputFormat {
    Text,
    Json,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct EvalDataset {
    name: String,
    #[serde(default)]
    source: String,
    memories: Vec<EvalMemory>,
    queries: Vec<EvalQuery>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct EvalMemory {
    id: String,
    #[serde(default = "default_scope")]
    scope: String,
    #[serde(default)]
    subject: Option<String>,
    text: String,
    #[serde(default)]
    markers: Vec<String>,
    #[serde(default = "default_kind")]
    kind: String,
    #[serde(default = "default_status")]
    status: String,
    #[serde(default = "default_trust")]
    trust: String,
    #[serde(default = "default_sensitivity")]
    sensitivity: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct EvalQuery {
    id: String,
    query: String,
    #[serde(default)]
    scope: Option<String>,
    relevant_ids: Vec<String>,
    #[serde(default)]
    category: String,
}

#[derive(Clone, Debug, Serialize)]
struct EvalReport {
    dataset: DatasetSummary,
    runs: Vec<RunSummary>,
    notes: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
struct DatasetSummary {
    name: String,
    source: String,
    memories: usize,
    queries: usize,
    avg_memory_bytes: f64,
    avg_markers_per_memory: f64,
}

#[derive(Clone, Debug, Serialize)]
struct RunSummary {
    system: String,
    layer: String,
    index_kind: String,
    mode: String,
    queries: usize,
    top_k: usize,
    hit_at_k: f64,
    recall_at_k: f64,
    precision_at_k: f64,
    avg_latency_micros: u64,
    p50_latency_micros: u64,
    p95_latency_micros: u64,
    avg_context_bytes: usize,
    avg_returned_items: f64,
}

#[derive(Clone, Debug)]
struct QueryResult {
    returned_ids: Vec<String>,
    latency_micros: u64,
    context_bytes: usize,
}

fn main() -> Result<()> {
    let args = Args::parse();
    if args.print_schema {
        println!("{}", serde_json::to_string_pretty(&schema_example())?);
        return Ok(());
    }

    if args.top_k == 0 {
        return Err(anyhow!("--top-k must be greater than zero"));
    }
    if args.repeats == 0 {
        return Err(anyhow!("--repeats must be greater than zero"));
    }

    let dataset = load_dataset(&args)?;
    validate_dataset(&dataset)?;
    let report = run_eval(&args, dataset)?;

    match args.output {
        OutputFormat::Json => {
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        OutputFormat::Text => {
            print_text_report(&report);
        }
    }

    Ok(())
}

fn load_dataset(args: &Args) -> Result<EvalDataset> {
    if let Some(input) = &args.input {
        let bytes = fs::read(input)
            .with_context(|| format!("failed to read dataset {}", input.display()))?;
        let dataset: EvalDataset = serde_json::from_slice(&bytes).with_context(|| {
            format!(
                "failed to parse dataset {}; expected neutral EvalDataset JSON",
                input.display()
            )
        })?;
        return Ok(dataset);
    }

    Ok(generate_dataset(args.profile))
}

fn validate_dataset(dataset: &EvalDataset) -> Result<()> {
    if dataset.memories.is_empty() {
        return Err(anyhow!("dataset contains no memories"));
    }
    if dataset.queries.is_empty() {
        return Err(anyhow!("dataset contains no queries"));
    }

    let mut memory_ids = HashSet::new();
    for memory in &dataset.memories {
        if memory.id.trim().is_empty() {
            return Err(anyhow!("memory id must not be empty"));
        }
        if !memory_ids.insert(memory.id.as_str()) {
            return Err(anyhow!("duplicate memory id {}", memory.id));
        }
        if memory.text.trim().is_empty() {
            return Err(anyhow!("memory {} has empty text", memory.id));
        }
    }

    for query in &dataset.queries {
        if query.id.trim().is_empty() {
            return Err(anyhow!("query id must not be empty"));
        }
        if query.query.trim().is_empty() {
            return Err(anyhow!("query {} has empty text", query.id));
        }
        if query.relevant_ids.is_empty() {
            return Err(anyhow!("query {} has no relevant_ids", query.id));
        }
        for relevant in &query.relevant_ids {
            if !memory_ids.contains(relevant.as_str()) {
                return Err(anyhow!(
                    "query {} references unknown relevant memory {}",
                    query.id,
                    relevant
                ));
            }
        }
    }

    Ok(())
}

fn run_eval(args: &Args, dataset: EvalDataset) -> Result<EvalReport> {
    let summary = summarize_dataset(&dataset);
    let mut runs = Vec::new();

    let indexes = selected_indexes(args.index);
    let modes = selected_modes(args.modes);
    let base_store = args.store_root.clone().unwrap_or_else(default_store_root);

    for index_kind in indexes {
        let store_root = base_store.join(index_kind.as_str());
        if store_root.exists() {
            fs::remove_dir_all(&store_root)
                .with_context(|| format!("failed to clear {}", store_root.display()))?;
        }

        let mut engine = MemoryEngine::init_with_options(
            &store_root,
            InitOptions {
                page_codec: PageCodecKind::MessagePack,
                compression: CompressionKind::Zstd,
                index_kind,
                page_clusterer: PageClustererKind::ScopeKind,
                durability: DurabilityPolicy::Fast,
                security_mode: SecurityMode::Unencrypted,
            },
        )?;
        let cell_to_eval_id = ingest_dataset(&mut engine, &dataset)?;

        for mode in &modes {
            runs.push(evaluate_mge(
                &engine,
                &dataset,
                &cell_to_eval_id,
                *mode,
                args.top_k,
                args.repeats,
                "hot",
                index_kind,
            )?);
        }

        engine.checkpoint()?;
        engine.seal()?;

        for mode in &modes {
            runs.push(evaluate_mge(
                &engine,
                &dataset,
                &cell_to_eval_id,
                *mode,
                args.top_k,
                args.repeats,
                "sealed",
                index_kind,
            )?);
        }

        if !args.keep_store {
            let _ = fs::remove_dir_all(&store_root);
        }
    }

    for mode in modes {
        runs.push(evaluate_scan_baseline(
            &dataset,
            mode,
            args.top_k,
            args.repeats,
        ));
    }

    if !args.keep_store && args.store_root.is_none() {
        let _ = fs::remove_dir_all(&base_store);
    }

    Ok(EvalReport {
        dataset: summary,
        runs,
        notes: vec![
            "Developer-only harness; datasets are not bundled and JSON is eval input/output only."
                .to_string(),
            "Known benchmarks such as LongMemEval or LoCoMo should be converted into the neutral EvalDataset JSON before running this tool."
                .to_string(),
            "This measures retrieval behavior, not final LLM answer quality.".to_string(),
        ],
    })
}

fn ingest_dataset(
    engine: &mut MemoryEngine,
    dataset: &EvalDataset,
) -> Result<HashMap<u64, String>> {
    let mut cell_to_eval_id = HashMap::new();
    for memory in &dataset.memories {
        let mut request = RememberRequest::new(
            MemoryKind::from_str(&memory.kind).unwrap_or(MemoryKind::ProjectFact),
            MemoryValue::Text(memory.text.clone()),
        );
        request.subject = memory.subject.clone();
        request.scope = memory.scope.clone();
        request.status = MemoryStatus::from_str(&memory.status).unwrap_or(MemoryStatus::Active);
        request.trust = TrustLevel::from_str(&memory.trust).unwrap_or(TrustLevel::UserConfirmed);
        request.sensitivity =
            SensitivityLevel::from_str(&memory.sensitivity).unwrap_or(SensitivityLevel::Private);
        request.markers = memory.markers.clone();
        request.source = Some(MemorySource {
            source_type: "agent_memory_eval".to_string(),
            reference: memory.id.clone(),
        });
        let cell = engine.remember(request)?;
        cell_to_eval_id.insert(cell.id, memory.id.clone());
    }
    Ok(cell_to_eval_id)
}

fn evaluate_mge(
    engine: &MemoryEngine,
    dataset: &EvalDataset,
    cell_to_eval_id: &HashMap<u64, String>,
    mode: RecallMode,
    top_k: usize,
    repeats: usize,
    layer: &str,
    index_kind: IndexKind,
) -> Result<RunSummary> {
    let mut results = Vec::new();
    for query in &dataset.queries {
        for _ in 0..repeats {
            let mut request = RecallRequest::new(query.query.clone());
            request.mode = mode;
            request.scope = query.scope.clone();
            request.max_items = top_k;
            let started = Instant::now();
            let packet = engine.recall(request)?;
            let latency_micros = elapsed_micros(started);
            let mut returned_ids = Vec::new();
            for score in packet
                .debug
                .score_details
                .iter()
                .take(packet.relevant_memory.len())
            {
                if let Some(eval_id) = cell_to_eval_id.get(&score.cell_id) {
                    returned_ids.push(eval_id.clone());
                }
            }
            results.push(QueryResult {
                returned_ids,
                latency_micros,
                context_bytes: packet.to_prompt_text().len(),
            });
        }
    }

    Ok(summarize_run(
        format!("mge_{}", index_kind.as_str()),
        layer.to_string(),
        index_kind.as_str().to_string(),
        mode.as_str().to_string(),
        top_k,
        repeats,
        &dataset.queries,
        &results,
    ))
}

fn evaluate_scan_baseline(
    dataset: &EvalDataset,
    mode: RecallMode,
    top_k: usize,
    repeats: usize,
) -> RunSummary {
    let mut results = Vec::new();
    for query in &dataset.queries {
        for _ in 0..repeats {
            let started = Instant::now();
            let returned_ids = scan_baseline(dataset, query, top_k, mode);
            let context_bytes = returned_ids
                .iter()
                .filter_map(|id| dataset.memories.iter().find(|memory| memory.id == *id))
                .map(|memory| memory.text.len())
                .sum::<usize>();
            results.push(QueryResult {
                returned_ids,
                latency_micros: elapsed_micros(started),
                context_bytes,
            });
        }
    }

    summarize_run(
        "scan_keyword_baseline".to_string(),
        "full_scan".to_string(),
        "none".to_string(),
        mode.as_str().to_string(),
        top_k,
        repeats,
        &dataset.queries,
        &results,
    )
}

fn scan_baseline(
    dataset: &EvalDataset,
    query: &EvalQuery,
    top_k: usize,
    mode: RecallMode,
) -> Vec<String> {
    let query_tokens = tokenize(&query.query);
    let mut scored = Vec::new();
    for memory in &dataset.memories {
        if let Some(scope) = &query.scope {
            if memory.scope != *scope {
                continue;
            }
        }
        let text_tokens = tokenize(&format!(
            "{} {} {}",
            memory.subject.clone().unwrap_or_default(),
            memory.text,
            memory.markers.join(" ")
        ));
        let overlap = query_tokens
            .iter()
            .filter(|token| text_tokens.contains(*token))
            .count();
        if overlap > 0 || matches!(mode, RecallMode::Broad) {
            scored.push((memory.id.clone(), overlap, memory.text.len()));
        }
    }
    scored.sort_by(|a, b| {
        b.1.cmp(&a.1)
            .then_with(|| a.2.cmp(&b.2))
            .then_with(|| a.0.cmp(&b.0))
    });
    scored
        .into_iter()
        .take(match mode {
            RecallMode::Broad => top_k.max(20),
            _ => top_k,
        })
        .map(|(id, _, _)| id)
        .collect()
}

fn summarize_run(
    system: String,
    layer: String,
    index_kind: String,
    mode: String,
    top_k: usize,
    repeats: usize,
    queries: &[EvalQuery],
    results: &[QueryResult],
) -> RunSummary {
    let mut hit_sum = 0.0;
    let mut recall_sum = 0.0;
    let mut precision_sum = 0.0;
    let mut context_bytes_sum = 0usize;
    let mut returned_items_sum = 0usize;
    let mut latencies = Vec::with_capacity(results.len());

    for (query_index, query) in queries.iter().enumerate() {
        let expected = query
            .relevant_ids
            .iter()
            .cloned()
            .collect::<BTreeSet<String>>();
        for repeat_index in 0..repeats {
            let result = &results[query_index * repeats + repeat_index];
            let returned = result
                .returned_ids
                .iter()
                .cloned()
                .collect::<BTreeSet<String>>();
            let relevant_returned = returned.intersection(&expected).count();
            hit_sum += if relevant_returned > 0 { 1.0 } else { 0.0 };
            recall_sum += relevant_returned as f64 / expected.len() as f64;
            precision_sum += if returned.is_empty() {
                0.0
            } else {
                relevant_returned as f64 / returned.len() as f64
            };
            context_bytes_sum += result.context_bytes;
            returned_items_sum += result.returned_ids.len();
            latencies.push(result.latency_micros);
        }
    }

    latencies.sort_unstable();
    let samples = results.len().max(1);
    RunSummary {
        system,
        layer,
        index_kind,
        mode,
        queries: queries.len(),
        top_k,
        hit_at_k: hit_sum / samples as f64,
        recall_at_k: recall_sum / samples as f64,
        precision_at_k: precision_sum / samples as f64,
        avg_latency_micros: average_u64(&latencies),
        p50_latency_micros: percentile(&latencies, 50.0),
        p95_latency_micros: percentile(&latencies, 95.0),
        avg_context_bytes: context_bytes_sum / samples,
        avg_returned_items: returned_items_sum as f64 / samples as f64,
    }
}

fn summarize_dataset(dataset: &EvalDataset) -> DatasetSummary {
    let total_bytes = dataset
        .memories
        .iter()
        .map(|memory| memory.text.len())
        .sum::<usize>();
    let total_markers = dataset
        .memories
        .iter()
        .map(|memory| memory.markers.len())
        .sum::<usize>();
    DatasetSummary {
        name: dataset.name.clone(),
        source: dataset.source.clone(),
        memories: dataset.memories.len(),
        queries: dataset.queries.len(),
        avg_memory_bytes: total_bytes as f64 / dataset.memories.len() as f64,
        avg_markers_per_memory: total_markers as f64 / dataset.memories.len() as f64,
    }
}

fn generate_dataset(profile: GeneratedProfile) -> EvalDataset {
    let (projects, records_per_project, query_stride) = match profile {
        GeneratedProfile::Tiny => (2, 12, 4),
        GeneratedProfile::Small => (4, 48, 8),
        GeneratedProfile::Medium => (8, 160, 16),
    };
    let components = [
        "storage",
        "security",
        "recall",
        "integration",
        "release",
        "terminal",
        "metadata",
        "checkpoint",
    ];
    let decisions = [
        "binary_runtime_storage",
        "marker_genome_indexing",
        "payload_encryption",
        "context_packet_output",
        "mcp_json_rpc_adapter",
        "local_first_workflow",
        "sealed_page_catalog",
        "deterministic_recall",
    ];

    let mut memories = Vec::new();
    let mut queries = Vec::new();
    for project in 0..projects {
        let scope = format!("project_{project}");
        for row in 0..records_per_project {
            let component = components[row % components.len()];
            let decision = decisions[(row + project) % decisions.len()];
            let id = format!("{scope}:{component}:{decision}:{row}");
            let subject = format!("{component} decision {row}");
            let text = format!(
                "In {scope}, the {component} component uses {decision}. \
                 This memory records a project decision, expected behavior, and integration note {row}."
            );
            memories.push(EvalMemory {
                id: id.clone(),
                scope: scope.clone(),
                subject: Some(subject),
                text,
                markers: vec![
                    format!("component:{component}"),
                    format!("decision:{decision}"),
                    format!("project:{scope}"),
                ],
                kind: default_kind(),
                status: default_status(),
                trust: default_trust(),
                sensitivity: default_sensitivity(),
            });

            if row % query_stride == 0 {
                queries.push(EvalQuery {
                    id: format!("q:{scope}:{component}:{row}"),
                    query: format!("{scope} {component} {decision} decision"),
                    scope: Some(scope.clone()),
                    relevant_ids: vec![id],
                    category: "single_fact_recall".to_string(),
                });
            }
        }
    }

    EvalDataset {
        name: format!("generated_{profile:?}").to_ascii_lowercase(),
        source: "generated safe agent-memory-style fixture".to_string(),
        memories,
        queries,
    }
}

fn schema_example() -> EvalDataset {
    EvalDataset {
        name: "example_agent_memory_eval_dataset".to_string(),
        source: "convert LongMemEval/LoCoMo/local traces into this neutral shape".to_string(),
        memories: vec![EvalMemory {
            id: "memory-001".to_string(),
            scope: "project_alpha".to_string(),
            subject: Some("storage decision".to_string()),
            text: "Project Alpha uses binary runtime storage for agent memory.".to_string(),
            markers: vec![
                "component:storage".to_string(),
                "decision:binary_runtime_storage".to_string(),
            ],
            kind: "project_fact".to_string(),
            status: "active".to_string(),
            trust: "user_confirmed".to_string(),
            sensitivity: "private".to_string(),
        }],
        queries: vec![EvalQuery {
            id: "query-001".to_string(),
            query: "What storage format does Project Alpha use?".to_string(),
            scope: Some("project_alpha".to_string()),
            relevant_ids: vec!["memory-001".to_string()],
            category: "single_fact_recall".to_string(),
        }],
    }
}

fn print_text_report(report: &EvalReport) {
    println!("Agent Memory Eval");
    println!("=================");
    println!(
        "dataset: {} ({})",
        report.dataset.name, report.dataset.source
    );
    println!(
        "memories: {} | queries: {} | avg memory bytes: {:.1} | avg markers/memory: {:.1}",
        report.dataset.memories,
        report.dataset.queries,
        report.dataset.avg_memory_bytes,
        report.dataset.avg_markers_per_memory
    );
    println!();
    println!(
        "{:<28} {:<8} {:<18} {:<8} {:>7} {:>7} {:>7} {:>8} {:>8} {:>8} {:>9} {:>8}",
        "system",
        "layer",
        "index",
        "mode",
        "hit@k",
        "rec@k",
        "prec@k",
        "avg_us",
        "p50_us",
        "p95_us",
        "ctx_bytes",
        "items"
    );
    println!("{}", "-".repeat(134));
    for run in &report.runs {
        println!(
            "{:<28} {:<8} {:<18} {:<8} {:>7.3} {:>7.3} {:>7.3} {:>8} {:>8} {:>8} {:>9} {:>8.2}",
            run.system,
            run.layer,
            run.index_kind,
            run.mode,
            run.hit_at_k,
            run.recall_at_k,
            run.precision_at_k,
            run.avg_latency_micros,
            run.p50_latency_micros,
            run.p95_latency_micros,
            run.avg_context_bytes,
            run.avg_returned_items
        );
    }
    println!();
    println!("notes:");
    for note in &report.notes {
        println!("- {note}");
    }
}

fn selected_indexes(index: IndexSelection) -> Vec<IndexKind> {
    match index {
        IndexSelection::Exact => vec![IndexKind::ExactMarkerPage],
        IndexSelection::BinaryFuse => vec![IndexKind::BinaryFusePage],
        IndexSelection::Both => vec![IndexKind::ExactMarkerPage, IndexKind::BinaryFusePage],
    }
}

fn selected_modes(mode: ModeSelection) -> Vec<RecallMode> {
    match mode {
        ModeSelection::Focused => vec![RecallMode::Focused],
        ModeSelection::Broad => vec![RecallMode::Broad],
        ModeSelection::FocusedBroad => vec![RecallMode::Focused, RecallMode::Broad],
    }
}

fn default_store_root() -> PathBuf {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0);
    std::env::temp_dir().join(format!("mge-agent-memory-eval-{now}"))
}

fn tokenize(input: &str) -> BTreeSet<String> {
    input
        .split(|ch: char| !ch.is_ascii_alphanumeric())
        .filter(|part| part.len() > 1)
        .map(|part| part.to_ascii_lowercase())
        .collect()
}

fn percentile(values: &[u64], percentile: f64) -> u64 {
    if values.is_empty() {
        return 0;
    }
    let index = ((values.len() - 1) as f64 * percentile / 100.0).round() as usize;
    values[index.min(values.len() - 1)]
}

fn average_u64(values: &[u64]) -> u64 {
    if values.is_empty() {
        return 0;
    }
    values.iter().sum::<u64>() / values.len() as u64
}

fn elapsed_micros(started: Instant) -> u64 {
    u64::try_from(started.elapsed().as_micros()).unwrap_or(u64::MAX)
}

fn default_scope() -> String {
    "global".to_string()
}

fn default_kind() -> String {
    "project_fact".to_string()
}

fn default_status() -> String {
    "active".to_string()
}

fn default_trust() -> String {
    "user_confirmed".to_string()
}

fn default_sensitivity() -> String {
    "private".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_dataset_is_valid() {
        let dataset = generate_dataset(GeneratedProfile::Tiny);
        validate_dataset(&dataset).unwrap();
        assert!(!dataset.memories.is_empty());
        assert!(!dataset.queries.is_empty());
    }

    #[test]
    fn schema_example_is_valid() {
        validate_dataset(&schema_example()).unwrap();
    }

    #[test]
    fn scan_baseline_finds_generated_relevant_memory() {
        let dataset = generate_dataset(GeneratedProfile::Tiny);
        let query = &dataset.queries[0];
        let returned = scan_baseline(&dataset, query, 5, RecallMode::Focused);
        assert!(returned.contains(&query.relevant_ids[0]));
    }

    #[test]
    fn run_summary_metrics_are_bounded() {
        let queries = vec![EvalQuery {
            id: "q1".to_string(),
            query: "alpha".to_string(),
            scope: None,
            relevant_ids: vec!["m1".to_string()],
            category: String::new(),
        }];
        let results = vec![QueryResult {
            returned_ids: vec!["m1".to_string(), "m2".to_string()],
            latency_micros: 10,
            context_bytes: 100,
        }];
        let summary = summarize_run(
            "test".to_string(),
            "test".to_string(),
            "none".to_string(),
            "focused".to_string(),
            5,
            1,
            &queries,
            &results,
        );
        assert_eq!(summary.hit_at_k, 1.0);
        assert_eq!(summary.recall_at_k, 1.0);
        assert_eq!(summary.precision_at_k, 0.5);
    }
}
