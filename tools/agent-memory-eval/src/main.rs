// Memory Genome Engine
// Copyright (c) 2026 ECD5A
// Project: https://github.com/ECD5A/Memory-Genome-Engine
//
// Licensed under the Apache License, Version 2.0.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fs;
use std::path::PathBuf;
use std::str::FromStr;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use anyhow::{anyhow, Context, Result};
use clap::{Parser, ValueEnum};
use mge_core::{
    chunk_session_turns, CompressionKind, DurabilityPolicy, IndexKind, InitOptions, MemoryEngine,
    MemoryKind, MemorySource, MemoryStatus, MemoryValue, PageClustererKind, PageCodecKind,
    RecallMode, RecallRequest, RememberRequest, SecurityMode, SensitivityLevel,
    SessionChunkOptions, SessionTurn, TrustLevel,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Parser)]
#[command(
    name = "mge-agent-memory-eval",
    about = "Developer-only agent memory evaluation harness for Memory Genome Engine"
)]
struct Args {
    /// Optional neutral JSON dataset. If omitted, a deterministic generated dataset is used.
    #[arg(long)]
    input: Option<PathBuf>,

    /// Input dataset format.
    #[arg(long, value_enum, default_value_t = InputFormat::Auto)]
    input_format: InputFormat,

    /// How conversation datasets are converted into memory records.
    #[arg(long, value_enum, default_value_t = IngestMode::RawTurn)]
    ingest_mode: IngestMode,

    /// Generated dataset profile.
    #[arg(long, value_enum, default_value_t = GeneratedProfile::Small)]
    profile: GeneratedProfile,

    /// Query difficulty for the deterministic generated dataset.
    #[arg(long, value_enum, default_value_t = GeneratedQueryProfile::Lexical)]
    query_profile: GeneratedQueryProfile,

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

    /// Which non-MGE retrieval baselines to run.
    #[arg(long, value_enum, default_value_t = BaselineSelection::Both)]
    baselines: BaselineSelection,

    /// Output format.
    #[arg(long, value_enum, default_value_t = OutputFormat::Text)]
    output: OutputFormat,

    /// Keep the generated/evaluation store after the run.
    #[arg(long)]
    keep_store: bool,

    /// Print the neutral EvalDataset JSON shape and exit.
    #[arg(long)]
    print_schema: bool,

    /// Limit the number of converted queries for local smoke runs.
    #[arg(long)]
    max_queries: Option<usize>,

    /// Limit the number of converted memories for local smoke runs.
    #[arg(long)]
    max_memories: Option<usize>,

    /// Optional path for the report. Uses --output format.
    #[arg(long)]
    report: Option<PathBuf>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum InputFormat {
    Auto,
    Neutral,
    LongMemEval,
    Locomo,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum IngestMode {
    #[value(name = "raw-turn")]
    RawTurn,
    #[value(name = "session-doc")]
    SessionDoc,
    #[value(name = "session-chunk")]
    SessionChunk,
    #[value(name = "session-plus-turn")]
    SessionPlusTurn,
    #[value(name = "key-fact")]
    KeyFact,
}

impl IngestMode {
    fn as_str(self) -> &'static str {
        match self {
            IngestMode::RawTurn => "raw-turn",
            IngestMode::SessionDoc => "session-doc",
            IngestMode::SessionChunk => "session-chunk",
            IngestMode::SessionPlusTurn => "session-plus-turn",
            IngestMode::KeyFact => "key-fact",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum GeneratedProfile {
    Tiny,
    Small,
    Medium,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum GeneratedQueryProfile {
    Lexical,
    Paraphrase,
    #[value(name = "hard-negative")]
    HardNegative,
    Mixed,
}

impl GeneratedQueryProfile {
    fn as_str(self) -> &'static str {
        match self {
            Self::Lexical => "lexical",
            Self::Paraphrase => "paraphrase",
            Self::HardNegative => "hard-negative",
            Self::Mixed => "mixed",
        }
    }
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
enum BaselineSelection {
    Keyword,
    Bm25,
    #[value(name = "text-index")]
    TextIndex,
    #[value(name = "page-token")]
    PageToken,
    All,
    Both,
    None,
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
    positive_queries: usize,
    top_k: usize,
    hit_at_k: f64,
    recall_at_k: f64,
    precision_at_k: f64,
    mrr_at_k: f64,
    ndcg_at_k: f64,
    negative_queries: usize,
    no_answer_accuracy: Option<f64>,
    abstention_analysis: Option<AbstentionAnalysis>,
    category_breakdown: Vec<CategorySummary>,
    avg_latency_micros: u64,
    avg_end_to_end_micros: u64,
    p50_latency_micros: u64,
    p95_latency_micros: u64,
    avg_context_bytes: usize,
    avg_context_tokens_estimate: usize,
    avg_returned_items: f64,
    avg_items_at_k: f64,
    avg_hot_candidates: f64,
    avg_pages_loaded: f64,
    avg_pages_pruned: f64,
    avg_cells_scanned: f64,
    avg_store_open_micros: u64,
    avg_page_read_micros: u64,
    avg_page_decode_micros: u64,
    avg_scoring_cache_build_micros: u64,
    avg_cell_filtering_micros: u64,
    avg_reranking_micros: u64,
    avg_context_packet_build_micros: u64,
}

#[derive(Clone, Debug, Serialize)]
struct CategorySummary {
    category: String,
    queries: usize,
    positive_queries: usize,
    negative_queries: usize,
    hit_at_k: f64,
    recall_at_k: f64,
    precision_at_k: f64,
    mrr_at_k: f64,
    ndcg_at_k: f64,
    no_answer_accuracy: Option<f64>,
}

#[derive(Clone, Debug, Default)]
struct RetrievalMetricAccumulator {
    samples: usize,
    positive_samples: usize,
    negative_samples: usize,
    hit_sum: f64,
    recall_sum: f64,
    precision_sum: f64,
    mrr_sum: f64,
    ndcg_sum: f64,
    no_answer_sum: f64,
}

#[derive(Clone, Debug, Serialize)]
struct AbstentionAnalysis {
    threshold_score: i64,
    balanced_accuracy: f64,
    positive_hit_accept_rate: f64,
    negative_reject_rate: f64,
    positive_samples: usize,
    negative_samples: usize,
}

#[derive(Clone, Debug, Default)]
struct QueryResult {
    returned_ids: Vec<String>,
    top_score: Option<i64>,
    latency_micros: u64,
    store_open_micros: u64,
    context_bytes: usize,
    hot_candidates: usize,
    pages_loaded: usize,
    pages_pruned: usize,
    cells_scanned: usize,
    page_read_micros: u64,
    page_decode_micros: u64,
    scoring_cache_build_micros: u64,
    cell_filtering_micros: u64,
    reranking_micros: u64,
    context_packet_build_micros: u64,
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
            let text = serde_json::to_string_pretty(&report)?;
            write_or_print_report(&args, &text)?;
        }
        OutputFormat::Text => {
            let text = text_report(&report);
            write_or_print_report(&args, &text)?;
        }
    }

    Ok(())
}

fn load_dataset(args: &Args) -> Result<EvalDataset> {
    if let Some(input) = &args.input {
        let bytes = fs::read(input)
            .with_context(|| format!("failed to read dataset {}", input.display()))?;
        let format = match args.input_format {
            InputFormat::Auto => detect_input_format(&bytes)?,
            explicit => explicit,
        };
        let mut dataset = match format {
            InputFormat::Auto => unreachable!("auto input format should be resolved"),
            InputFormat::Neutral => serde_json::from_slice(&bytes).with_context(|| {
                format!(
                    "failed to parse dataset {}; expected neutral EvalDataset JSON",
                    input.display()
                )
            })?,
            InputFormat::LongMemEval => convert_longmemeval(
                &serde_json::from_slice(&bytes)?,
                input.display().to_string(),
                args.max_memories,
                args.max_queries,
                args.ingest_mode,
            )?,
            InputFormat::Locomo => convert_locomo(
                &serde_json::from_slice(&bytes)?,
                input.display().to_string(),
                args.max_memories,
                args.max_queries,
            )?,
        };
        apply_limits(&mut dataset, args.max_memories, args.max_queries);
        return Ok(dataset);
    }

    Ok(generate_dataset_with_queries(
        args.profile,
        args.query_profile,
    ))
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

fn detect_input_format(bytes: &[u8]) -> Result<InputFormat> {
    let value: Value = serde_json::from_slice(bytes)?;
    if value.get("memories").is_some() && value.get("queries").is_some() {
        return Ok(InputFormat::Neutral);
    }
    let first = value
        .as_array()
        .and_then(|items| items.first())
        .unwrap_or(&value);
    if first.get("haystack_sessions").is_some() || first.get("question_id").is_some() {
        return Ok(InputFormat::LongMemEval);
    }
    if first.get("qa").is_some()
        || first.get("conversation").is_some()
        || first
            .as_object()
            .is_some_and(|object| object.keys().any(|key| key.starts_with("session")))
    {
        return Ok(InputFormat::Locomo);
    }
    Err(anyhow!(
        "could not detect dataset format; pass --input-format neutral|long-mem-eval|locomo"
    ))
}

fn apply_limits(
    dataset: &mut EvalDataset,
    max_memories: Option<usize>,
    max_queries: Option<usize>,
) {
    if let Some(max) = max_memories {
        dataset.memories.truncate(max);
        let memory_ids = dataset
            .memories
            .iter()
            .map(|memory| memory.id.as_str())
            .collect::<HashSet<_>>();
        dataset.queries.retain(|query| {
            !query.relevant_ids.is_empty()
                && query
                    .relevant_ids
                    .iter()
                    .all(|id| memory_ids.contains(id.as_str()))
        });
    }
    if let Some(max) = max_queries {
        dataset.queries.truncate(max);
    }
}

fn convert_longmemeval(
    value: &Value,
    source: String,
    max_memories: Option<usize>,
    max_queries: Option<usize>,
    ingest_mode: IngestMode,
) -> Result<EvalDataset> {
    let instances = value
        .as_array()
        .ok_or_else(|| anyhow!("LongMemEval input must be a JSON array"))?;
    let mut memories = Vec::new();
    let mut queries = Vec::new();
    let mut skipped_without_evidence = 0usize;

    for (instance_index, instance) in instances.iter().enumerate() {
        let question_id = string_field(instance, &["question_id", "id"])
            .unwrap_or_else(|| format!("longmemeval-{instance_index}"));
        let question_type = string_field(instance, &["question_type", "category"])
            .unwrap_or_else(|| "unknown".to_string());
        let question = string_field(instance, &["question"])
            .ok_or_else(|| anyhow!("LongMemEval instance {question_id} has no question"))?;

        let haystack_sessions = instance
            .get("haystack_sessions")
            .and_then(Value::as_array)
            .ok_or_else(|| {
                anyhow!("LongMemEval instance {question_id} has no haystack_sessions")
            })?;
        let session_ids = instance
            .get("haystack_session_ids")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let answer_session_ids = instance
            .get("answer_session_ids")
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(value_to_compact_string)
                    .collect::<HashSet<_>>()
            })
            .unwrap_or_default();

        let mut session_turn_ids: HashMap<String, Vec<String>> = HashMap::new();
        let mut session_memory_ids: HashMap<String, String> = HashMap::new();
        let mut session_chunk_ids: HashMap<String, Vec<String>> = HashMap::new();
        let mut session_fact_ids: HashMap<String, Vec<String>> = HashMap::new();
        let mut relevant_ids = Vec::new();
        let scope = format!("longmemeval:{question_id}");

        for (session_index, session) in haystack_sessions.iter().enumerate() {
            let session_id = session_ids
                .get(session_index)
                .and_then(value_to_compact_string)
                .unwrap_or_else(|| format!("session-{session_index}"));
            let Some(turns) = session.as_array() else {
                continue;
            };
            let session_memory_id = format!("{question_id}:s{session_index}:{session_id}:session");
            let mut session_lines = Vec::new();
            let mut session_has_answer = answer_session_ids.contains(&session_id);
            let mut converted_turns = Vec::new();

            for (turn_index, turn) in turns.iter().enumerate() {
                let content = string_field(turn, &["content", "text", "message"])
                    .or_else(|| turn.as_str().map(str::to_string));
                let Some(content) = content else {
                    continue;
                };
                if !is_safe_memory_text(&content) {
                    continue;
                }
                let role =
                    string_field(turn, &["role", "speaker"]).unwrap_or_else(|| "turn".to_string());
                let id = format!("{question_id}:s{session_index}:{session_id}:turn-{turn_index}");
                let has_answer = turn
                    .get("has_answer")
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                session_has_answer |= has_answer;
                session_lines.push(format!("{role}: {content}"));
                converted_turns.push(LongMemEvalTurn {
                    id: id.clone(),
                    role: role.clone(),
                    content: content.clone(),
                    has_answer,
                });
                session_turn_ids
                    .entry(session_id.clone())
                    .or_default()
                    .push(id);
            }

            if converted_turns.is_empty() {
                continue;
            }

            if matches!(
                ingest_mode,
                IngestMode::SessionDoc | IngestMode::SessionPlusTurn
            ) {
                session_memory_ids.insert(session_id.clone(), session_memory_id.clone());
                if session_has_answer {
                    relevant_ids.push(session_memory_id.clone());
                }
                memories.push(EvalMemory {
                    id: session_memory_id,
                    scope: scope.clone(),
                    subject: Some(format!("{question_type} session {session_id}")),
                    text: session_lines.join("\n"),
                    markers: vec![
                        format!("benchmark:longmemeval"),
                        format!("question_type:{}", safe_marker_value(&question_type)),
                        "memory_granularity:session".to_string(),
                        format!("session:{}", safe_marker_value(&session_id)),
                    ],
                    kind: "project_fact".to_string(),
                    status: "active".to_string(),
                    trust: "user_confirmed".to_string(),
                    sensitivity: "private".to_string(),
                });
            }

            if ingest_mode == IngestMode::SessionChunk {
                for (chunk_index, chunk) in chunk_longmemeval_session(&converted_turns)?
                    .into_iter()
                    .enumerate()
                {
                    let id =
                        format!("{question_id}:s{session_index}:{session_id}:chunk-{chunk_index}");
                    if chunk.has_answer {
                        relevant_ids.push(id.clone());
                    }
                    session_chunk_ids
                        .entry(session_id.clone())
                        .or_default()
                        .push(id.clone());
                    memories.push(EvalMemory {
                        id,
                        scope: scope.clone(),
                        subject: Some(format!("{question_type} context block {session_id}")),
                        text: chunk.text,
                        markers: vec![
                            "benchmark:longmemeval".to_string(),
                            format!("question_type:{}", safe_marker_value(&question_type)),
                            "memory_granularity:session_chunk".to_string(),
                            format!("session:{}", safe_marker_value(&session_id)),
                        ],
                        kind: "project_fact".to_string(),
                        status: "active".to_string(),
                        trust: "user_confirmed".to_string(),
                        sensitivity: "private".to_string(),
                    });
                }
            }

            if ingest_mode == IngestMode::KeyFact {
                for converted in &converted_turns {
                    for (fact_index, fact) in
                        split_key_facts(&converted.content).into_iter().enumerate()
                    {
                        let id = format!("{}:fact-{fact_index}", converted.id);
                        if converted.has_answer {
                            relevant_ids.push(id.clone());
                        }
                        session_fact_ids
                            .entry(session_id.clone())
                            .or_default()
                            .push(id.clone());
                        memories.push(EvalMemory {
                            id,
                            scope: scope.clone(),
                            subject: Some(format!(
                                "{question_type} key fact {} {session_id}",
                                converted.role
                            )),
                            text: fact,
                            markers: vec![
                                format!("benchmark:longmemeval"),
                                format!("question_type:{}", safe_marker_value(&question_type)),
                                "memory_granularity:key_fact".to_string(),
                                format!("role:{}", safe_marker_value(&converted.role)),
                                format!("session:{}", safe_marker_value(&session_id)),
                            ],
                            kind: "project_fact".to_string(),
                            status: "active".to_string(),
                            trust: "user_confirmed".to_string(),
                            sensitivity: "private".to_string(),
                        });
                    }
                }
            }

            if matches!(
                ingest_mode,
                IngestMode::RawTurn | IngestMode::SessionPlusTurn
            ) {
                for converted in converted_turns {
                    if converted.has_answer {
                        relevant_ids.push(converted.id.clone());
                    }
                    memories.push(EvalMemory {
                        id: converted.id,
                        scope: scope.clone(),
                        subject: Some(format!("{question_type} {} {session_id}", converted.role)),
                        text: converted.content,
                        markers: vec![
                            format!("benchmark:longmemeval"),
                            format!("question_type:{}", safe_marker_value(&question_type)),
                            "memory_granularity:turn".to_string(),
                            format!("role:{}", safe_marker_value(&converted.role)),
                            format!("session:{}", safe_marker_value(&session_id)),
                        ],
                        kind: "project_fact".to_string(),
                        status: "active".to_string(),
                        trust: "user_confirmed".to_string(),
                        sensitivity: "private".to_string(),
                    });
                }
            }
        }

        if relevant_ids.is_empty() {
            for answer_session_id in &answer_session_ids {
                if matches!(
                    ingest_mode,
                    IngestMode::SessionDoc | IngestMode::SessionPlusTurn
                ) {
                    if let Some(session_memory_id) = session_memory_ids.get(answer_session_id) {
                        relevant_ids.push(session_memory_id.clone());
                    }
                }
                if matches!(
                    ingest_mode,
                    IngestMode::RawTurn | IngestMode::SessionPlusTurn
                ) {
                    if let Some(turn_ids) = session_turn_ids.get(answer_session_id) {
                        relevant_ids.extend(turn_ids.iter().cloned());
                    }
                }
                if ingest_mode == IngestMode::KeyFact {
                    if let Some(fact_ids) = session_fact_ids.get(answer_session_id) {
                        relevant_ids.extend(fact_ids.iter().cloned());
                    }
                }
                if ingest_mode == IngestMode::SessionChunk {
                    if let Some(chunk_ids) = session_chunk_ids.get(answer_session_id) {
                        relevant_ids.extend(chunk_ids.iter().cloned());
                    }
                }
            }
        }
        relevant_ids.sort();
        relevant_ids.dedup();

        let is_abstention = question_id.ends_with("_abs");
        if is_abstention {
            relevant_ids.clear();
        }
        if relevant_ids.is_empty() && !is_abstention {
            skipped_without_evidence += 1;
            continue;
        }

        queries.push(EvalQuery {
            id: question_id,
            query: question,
            scope: Some(scope),
            relevant_ids,
            category: if is_abstention {
                "abstention".to_string()
            } else {
                question_type
            },
        });
        if max_queries.is_some_and(|max| queries.len() >= max) {
            break;
        }
        if max_memories.is_some_and(|max| memories.len() >= max) {
            break;
        }
    }

    Ok(EvalDataset {
        name: format!("longmemeval_{}", ingest_mode.as_str().replace('-', "_")),
        source: format!(
            "{source}; ingest_mode: {}; skipped queries without evidence: {skipped_without_evidence}",
            ingest_mode.as_str()
        ),
        memories,
        queries,
    })
}

#[derive(Clone, Debug)]
struct LongMemEvalTurn {
    id: String,
    role: String,
    content: String,
    has_answer: bool,
}

#[derive(Clone, Debug)]
struct LongMemEvalChunk {
    text: String,
    has_answer: bool,
}

fn chunk_longmemeval_session(turns: &[LongMemEvalTurn]) -> Result<Vec<LongMemEvalChunk>> {
    let production_turns = turns
        .iter()
        .map(|turn| SessionTurn::new(&turn.role, &turn.content))
        .collect::<Vec<_>>();
    chunk_session_turns(&production_turns, SessionChunkOptions::default())?
        .into_iter()
        .map(|chunk| {
            let has_answer = turns[chunk.start_turn..chunk.end_turn]
                .iter()
                .any(|turn| turn.has_answer);
            Ok(LongMemEvalChunk {
                text: chunk.text,
                has_answer,
            })
        })
        .collect()
}

fn convert_locomo(
    value: &Value,
    source: String,
    max_memories: Option<usize>,
    max_queries: Option<usize>,
) -> Result<EvalDataset> {
    let records = value
        .as_array()
        .ok_or_else(|| anyhow!("LoCoMo input must be a JSON array"))?;
    let mut memories = Vec::new();
    let mut queries = Vec::new();
    let mut skipped_without_evidence = 0usize;

    for (record_index, record) in records.iter().enumerate() {
        let conversation_id = string_field(
            record,
            &["conversation_id", "sample_id", "id", "dialogue_id"],
        )
        .unwrap_or_else(|| format!("locomo-{record_index}"));
        let scope = format!("locomo:{conversation_id}");
        let before = memories.len();
        let mut dialog_to_memory = HashMap::new();
        collect_locomo_memories(
            record,
            &scope,
            &conversation_id,
            "root",
            &mut memories,
            &mut dialog_to_memory,
        );
        if memories.len() == before {
            continue;
        }

        if let Some(qa_items) = record.get("qa").and_then(Value::as_array) {
            for (qa_index, qa) in qa_items.iter().enumerate() {
                let Some(question) = string_field(qa, &["question", "query"]) else {
                    continue;
                };
                let mut relevant_ids = Vec::new();
                if let Some(evidence) = qa.get("evidence") {
                    collect_locomo_relevant_ids(
                        evidence,
                        &dialog_to_memory,
                        &memories,
                        &mut relevant_ids,
                    );
                }
                relevant_ids.sort();
                relevant_ids.dedup();
                if relevant_ids.is_empty() {
                    skipped_without_evidence += 1;
                    continue;
                }
                let category = value_to_compact_string(qa.get("category").unwrap_or(&Value::Null))
                    .unwrap_or_else(|| "qa".to_string());
                queries.push(EvalQuery {
                    id: format!("{conversation_id}:qa-{qa_index}"),
                    query: question,
                    scope: Some(scope.clone()),
                    relevant_ids,
                    category,
                });
                if max_queries.is_some_and(|max| queries.len() >= max) {
                    break;
                }
            }
        }
        if max_queries.is_some_and(|max| queries.len() >= max) {
            break;
        }
        if max_memories.is_some_and(|max| memories.len() >= max) {
            break;
        }
    }

    Ok(EvalDataset {
        name: "locomo_converted".to_string(),
        source: format!("{source}; skipped queries without evidence: {skipped_without_evidence}"),
        memories,
        queries,
    })
}

fn collect_locomo_memories(
    value: &Value,
    scope: &str,
    conversation_id: &str,
    path: &str,
    memories: &mut Vec<EvalMemory>,
    dialog_to_memory: &mut HashMap<String, String>,
) {
    match value {
        Value::Array(items) => {
            for (index, item) in items.iter().enumerate() {
                collect_locomo_memories(
                    item,
                    scope,
                    conversation_id,
                    &format!("{path}.{index}"),
                    memories,
                    dialog_to_memory,
                );
            }
        }
        Value::Object(object) => {
            if let Some(text) = string_field(value, &["text", "content", "message", "utterance"]) {
                if is_safe_memory_text(&text) {
                    let speaker = string_field(value, &["speaker", "role", "name"])
                        .unwrap_or_else(|| "speaker".to_string());
                    let dialog_id = string_field(value, &["dialog_id", "dia_id", "id", "turn_id"])
                        .unwrap_or_else(|| path.to_string());
                    let id = format!("{conversation_id}:{dialog_id}");
                    dialog_to_memory.insert(dialog_id.clone(), id.clone());
                    memories.push(EvalMemory {
                        id,
                        scope: scope.to_string(),
                        subject: Some(format!("locomo {speaker} {dialog_id}")),
                        text,
                        markers: vec![
                            "benchmark:locomo".to_string(),
                            format!("speaker:{}", safe_marker_value(&speaker)),
                            format!("dialog:{}", safe_marker_value(&dialog_id)),
                        ],
                        kind: "project_fact".to_string(),
                        status: "active".to_string(),
                        trust: "user_confirmed".to_string(),
                        sensitivity: "private".to_string(),
                    });
                    return;
                }
            }

            for (key, nested) in object {
                if matches!(
                    key.as_str(),
                    "qa" | "qa_pairs" | "event_summary" | "summary"
                ) {
                    continue;
                }
                collect_locomo_memories(
                    nested,
                    scope,
                    conversation_id,
                    &format!("{path}.{key}"),
                    memories,
                    dialog_to_memory,
                );
            }
        }
        _ => {}
    }
}

fn collect_locomo_relevant_ids(
    evidence: &Value,
    dialog_to_memory: &HashMap<String, String>,
    memories: &[EvalMemory],
    output: &mut Vec<String>,
) {
    match evidence {
        Value::Array(items) => {
            for item in items {
                collect_locomo_relevant_ids(item, dialog_to_memory, memories, output);
            }
        }
        Value::Object(object) => {
            for key in ["dialog_id", "dia_id", "id", "turn_id"] {
                if let Some(id) = object
                    .get(key)
                    .and_then(value_to_compact_string)
                    .and_then(|dialog_id| dialog_to_memory.get(&dialog_id).cloned())
                {
                    output.push(id);
                }
            }
            for nested in object.values() {
                collect_locomo_relevant_ids(nested, dialog_to_memory, memories, output);
            }
        }
        Value::String(text) => {
            if let Some(id) = dialog_to_memory.get(text) {
                output.push(id.clone());
                return;
            }
            let needle = text.trim();
            if needle.len() < 8 {
                return;
            }
            for memory in memories {
                if memory.text.contains(needle) || needle.contains(memory.text.as_str()) {
                    output.push(memory.id.clone());
                }
            }
        }
        Value::Number(_) | Value::Bool(_) => {
            if let Some(key) = value_to_compact_string(evidence) {
                if let Some(id) = dialog_to_memory.get(&key) {
                    output.push(id.clone());
                }
            }
        }
        Value::Null => {}
    }
}

fn string_field(value: &Value, keys: &[&str]) -> Option<String> {
    for key in keys {
        if let Some(text) = value.get(*key).and_then(Value::as_str) {
            return Some(text.to_string());
        }
    }
    None
}

fn value_to_compact_string(value: &Value) -> Option<String> {
    match value {
        Value::String(text) => Some(text.clone()),
        Value::Number(number) => Some(number.to_string()),
        Value::Bool(value) => Some(value.to_string()),
        _ => None,
    }
}

fn safe_marker_value(input: &str) -> String {
    let mut output = String::new();
    let mut last_was_separator = false;
    for ch in input.chars() {
        if ch.is_ascii_alphanumeric() {
            output.push(ch.to_ascii_lowercase());
            last_was_separator = false;
        } else if !last_was_separator {
            output.push('_');
            last_was_separator = true;
        }
    }
    let output = output.trim_matches('_').to_string();
    if output.is_empty() {
        "unknown".to_string()
    } else {
        output
    }
}

fn is_safe_memory_text(input: &str) -> bool {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return false;
    }
    if trimmed.len() <= 64 && trimmed.chars().all(|ch| !ch.is_ascii_alphanumeric()) {
        return false;
    }
    true
}

fn split_key_facts(input: &str) -> Vec<String> {
    let mut facts = Vec::new();
    for part in input.split(['\n', '.', '!', '?', ';']) {
        if facts.len() >= 3 {
            break;
        }
        let fact = part.trim();
        if fact.len() < 16 || !is_safe_memory_text(fact) {
            continue;
        }
        let fact = if fact.len() > 360 {
            let boundary = fact
                .char_indices()
                .take_while(|(index, _)| *index <= 360)
                .last()
                .map(|(index, ch)| index + ch.len_utf8())
                .unwrap_or(360);
            fact[..boundary].trim().to_string()
        } else {
            fact.to_string()
        };
        if !facts.iter().any(|existing| existing == &fact) {
            facts.push(fact);
        }
    }
    if facts.is_empty() && is_safe_memory_text(input) {
        facts.push(input.trim().to_string());
    }
    facts
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
                "sealed_after_seal",
                index_kind,
            )?);
        }

        drop(engine);
        for mode in &modes {
            runs.push(evaluate_mge_cold(
                &store_root,
                &dataset,
                &cell_to_eval_id,
                *mode,
                args.top_k,
                args.repeats,
                index_kind,
            )?);
        }

        let reopened = MemoryEngine::open_at(&store_root)?;
        for mode in &modes {
            warm_mge_queries(&reopened, &dataset, *mode, args.top_k)?;
            runs.push(evaluate_mge(
                &reopened,
                &dataset,
                &cell_to_eval_id,
                *mode,
                args.top_k,
                args.repeats,
                "sealed_repeated",
                index_kind,
            )?);
        }
        drop(reopened);

        if !args.keep_store {
            let _ = fs::remove_dir_all(&store_root);
        }
    }

    for mode in modes {
        for baseline in selected_baselines(args.baselines) {
            runs.push(match baseline {
                BaselineKind::Keyword => {
                    evaluate_keyword_baseline(&dataset, mode, args.top_k, args.repeats)
                }
                BaselineKind::Bm25 => {
                    evaluate_bm25_baseline(&dataset, mode, args.top_k, args.repeats)
                }
                BaselineKind::TextIndex => {
                    evaluate_text_index_baseline(&dataset, mode, args.top_k, args.repeats)
                }
                BaselineKind::PageToken => {
                    evaluate_page_token_baseline(&dataset, mode, args.top_k, args.repeats)
                }
            });
        }
    }

    if !args.keep_store && args.store_root.is_none() {
        let _ = fs::remove_dir_all(&base_store);
    }

    let mut notes = vec![
        "Developer-only harness; datasets are not bundled and JSON is eval input/output only."
            .to_string(),
        "LongMemEval and best-effort LoCoMo-style JSON adapters are local-only; no datasets are committed or downloaded by the tool.".to_string(),
        "This measures retrieval behavior, not final LLM answer quality.".to_string(),
        "scoring cache build time is measured inside cell filtering time; those columns are not additive.".to_string(),
    ];
    if args.input.is_none() {
        notes.push(format!(
            "Generated query profile: {}; paraphrase queries retain scope/component anchors, while hard negatives deliberately share a scope with unrelated memories.",
            args.query_profile.as_str()
        ));
    }

    Ok(EvalReport {
        dataset: summary,
        runs,
        notes,
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

#[allow(clippy::too_many_arguments)]
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
            results.push(evaluate_mge_query(
                engine,
                query,
                cell_to_eval_id,
                mode,
                top_k,
            )?);
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

fn evaluate_mge_cold(
    store_root: &std::path::Path,
    dataset: &EvalDataset,
    cell_to_eval_id: &HashMap<u64, String>,
    mode: RecallMode,
    top_k: usize,
    repeats: usize,
    index_kind: IndexKind,
) -> Result<RunSummary> {
    let mut results = Vec::new();
    for query in &dataset.queries {
        for _ in 0..repeats {
            let open_started = Instant::now();
            let engine = MemoryEngine::open_at(store_root)?;
            let store_open_micros = elapsed_micros(open_started);
            let mut result = evaluate_mge_query(&engine, query, cell_to_eval_id, mode, top_k)?;
            result.store_open_micros = store_open_micros;
            results.push(result);
        }
    }

    Ok(summarize_run(
        format!("mge_{}", index_kind.as_str()),
        "sealed_cold".to_string(),
        index_kind.as_str().to_string(),
        mode.as_str().to_string(),
        top_k,
        repeats,
        &dataset.queries,
        &results,
    ))
}

fn warm_mge_queries(
    engine: &MemoryEngine,
    dataset: &EvalDataset,
    mode: RecallMode,
    top_k: usize,
) -> Result<()> {
    for query in &dataset.queries {
        let mut request = RecallRequest::new(query.query.clone());
        request.mode = mode;
        request.scope = query.scope.clone();
        request.max_items = top_k;
        engine.recall(request)?;
    }
    Ok(())
}

fn evaluate_mge_query(
    engine: &MemoryEngine,
    query: &EvalQuery,
    cell_to_eval_id: &HashMap<u64, String>,
    mode: RecallMode,
    top_k: usize,
) -> Result<QueryResult> {
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
    Ok(QueryResult {
        returned_ids,
        top_score: packet
            .debug
            .score_details
            .first()
            .map(|detail| detail.score),
        latency_micros,
        store_open_micros: 0,
        context_bytes: packet.to_prompt_text().len(),
        hot_candidates: packet.debug.hot_candidate_cells,
        pages_loaded: packet.debug.loaded_pages,
        pages_pruned: packet.debug.pages_pruned_by_metadata,
        cells_scanned: packet.debug.cells_scanned,
        page_read_micros: packet.debug.page_file_read_load_micros,
        page_decode_micros: packet.debug.page_decode_micros,
        scoring_cache_build_micros: packet.debug.scoring_cache_build_micros,
        cell_filtering_micros: packet.debug.cell_filtering_micros,
        reranking_micros: packet.debug.reranking_micros,
        context_packet_build_micros: packet.debug.context_packet_build_micros,
    })
}

fn evaluate_keyword_baseline(
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
                hot_candidates: 0,
                pages_loaded: 0,
                pages_pruned: 0,
                cells_scanned: dataset.memories.len(),
                ..QueryResult::default()
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

fn evaluate_bm25_baseline(
    dataset: &EvalDataset,
    mode: RecallMode,
    top_k: usize,
    repeats: usize,
) -> RunSummary {
    let index = Bm25Index::build(dataset);
    let mut results = Vec::new();
    for query in &dataset.queries {
        for _ in 0..repeats {
            let started = Instant::now();
            let returned_ids = index.search(query, top_k, mode);
            let context_bytes = returned_ids
                .iter()
                .filter_map(|id| dataset.memories.iter().find(|memory| memory.id == *id))
                .map(|memory| memory.text.len())
                .sum::<usize>();
            results.push(QueryResult {
                returned_ids,
                latency_micros: elapsed_micros(started),
                context_bytes,
                hot_candidates: 0,
                pages_loaded: 0,
                pages_pruned: 0,
                cells_scanned: dataset.memories.len(),
                ..QueryResult::default()
            });
        }
    }

    summarize_run(
        "bm25_baseline".to_string(),
        "inverted_index".to_string(),
        "none".to_string(),
        mode.as_str().to_string(),
        top_k,
        repeats,
        &dataset.queries,
        &results,
    )
}

fn evaluate_text_index_baseline(
    dataset: &EvalDataset,
    mode: RecallMode,
    top_k: usize,
    repeats: usize,
) -> RunSummary {
    let index = TextCandidateIndex::build(dataset);
    let mut results = Vec::new();
    for query in &dataset.queries {
        for _ in 0..repeats {
            let started = Instant::now();
            let returned_ids = index.search(query, top_k, mode);
            let context_bytes = returned_ids
                .iter()
                .filter_map(|id| dataset.memories.iter().find(|memory| memory.id == *id))
                .map(|memory| memory.text.len())
                .sum::<usize>();
            results.push(QueryResult {
                returned_ids,
                latency_micros: elapsed_micros(started),
                context_bytes,
                hot_candidates: 0,
                pages_loaded: 0,
                pages_pruned: 0,
                cells_scanned: 0,
                ..QueryResult::default()
            });
        }
    }

    summarize_run(
        "text_candidate_index".to_string(),
        "inverted_index".to_string(),
        "none".to_string(),
        mode.as_str().to_string(),
        top_k,
        repeats,
        &dataset.queries,
        &results,
    )
}

fn evaluate_page_token_baseline(
    dataset: &EvalDataset,
    mode: RecallMode,
    top_k: usize,
    repeats: usize,
) -> RunSummary {
    let index = PageTokenIndex::build(dataset, 64);
    let mut results = Vec::new();
    for query in &dataset.queries {
        for _ in 0..repeats {
            let started = Instant::now();
            let returned_ids = index.search(query, top_k, mode);
            let context_bytes = returned_ids
                .iter()
                .filter_map(|id| dataset.memories.iter().find(|memory| memory.id == *id))
                .map(|memory| memory.text.len())
                .sum::<usize>();
            results.push(QueryResult {
                returned_ids,
                latency_micros: elapsed_micros(started),
                context_bytes,
                hot_candidates: 0,
                pages_loaded: 0,
                pages_pruned: 0,
                cells_scanned: 0,
                ..QueryResult::default()
            });
        }
    }

    summarize_run(
        "page_token_summary".to_string(),
        "page_prefilter".to_string(),
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum BaselineKind {
    Keyword,
    Bm25,
    TextIndex,
    PageToken,
}

fn selected_baselines(selection: BaselineSelection) -> Vec<BaselineKind> {
    match selection {
        BaselineSelection::Keyword => vec![BaselineKind::Keyword],
        BaselineSelection::Bm25 => vec![BaselineKind::Bm25],
        BaselineSelection::TextIndex => vec![BaselineKind::TextIndex],
        BaselineSelection::PageToken => vec![BaselineKind::PageToken],
        BaselineSelection::Both => vec![BaselineKind::Keyword, BaselineKind::Bm25],
        BaselineSelection::All => vec![
            BaselineKind::Keyword,
            BaselineKind::Bm25,
            BaselineKind::TextIndex,
            BaselineKind::PageToken,
        ],
        BaselineSelection::None => Vec::new(),
    }
}

#[derive(Clone, Debug)]
struct Bm25Index {
    docs: Vec<Bm25Doc>,
    document_frequency: HashMap<String, usize>,
    avg_doc_len: f64,
}

#[derive(Clone, Debug)]
struct Bm25Doc {
    id: String,
    scope: String,
    length: usize,
    term_frequency: HashMap<String, usize>,
}

impl Bm25Index {
    fn build(dataset: &EvalDataset) -> Self {
        let mut docs = Vec::with_capacity(dataset.memories.len());
        let mut document_frequency = HashMap::<String, usize>::new();
        let mut total_doc_len = 0usize;

        for memory in &dataset.memories {
            let tokens = memory_tokens(memory);
            let mut term_frequency = HashMap::<String, usize>::new();
            for token in tokens {
                *term_frequency.entry(token).or_insert(0) += 1;
            }
            for token in term_frequency.keys() {
                *document_frequency.entry(token.clone()).or_insert(0) += 1;
            }
            let length = term_frequency.values().sum::<usize>();
            total_doc_len += length;
            docs.push(Bm25Doc {
                id: memory.id.clone(),
                scope: memory.scope.clone(),
                length,
                term_frequency,
            });
        }

        let avg_doc_len = if docs.is_empty() {
            0.0
        } else {
            total_doc_len as f64 / docs.len() as f64
        };
        Self {
            docs,
            document_frequency,
            avg_doc_len,
        }
    }

    fn search(&self, query: &EvalQuery, top_k: usize, mode: RecallMode) -> Vec<String> {
        let query_terms = tokenize(&query.query);
        let mut scored = Vec::new();
        for doc in &self.docs {
            if let Some(scope) = &query.scope {
                if doc.scope != *scope {
                    continue;
                }
            }
            let score = self.score_doc(doc, &query_terms);
            if score > 0.0 || matches!(mode, RecallMode::Broad) {
                scored.push((doc.id.clone(), score, doc.length));
            }
        }
        scored.sort_by(|a, b| {
            b.1.partial_cmp(&a.1)
                .unwrap_or(std::cmp::Ordering::Equal)
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

    fn score_doc(&self, doc: &Bm25Doc, query_terms: &BTreeSet<String>) -> f64 {
        let doc_count = self.docs.len() as f64;
        let mut score = 0.0;
        for term in query_terms {
            let Some(tf) = doc.term_frequency.get(term).copied() else {
                continue;
            };
            let df = self.document_frequency.get(term).copied().unwrap_or(0) as f64;
            score += bm25_term_score(doc_count, df, tf, doc.length, self.avg_doc_len);
        }
        score
    }
}

#[derive(Clone, Debug)]
struct TextCandidateIndex {
    docs: Vec<Bm25Doc>,
    postings: HashMap<String, Vec<(usize, usize)>>,
    document_frequency: HashMap<String, usize>,
    avg_doc_len: f64,
}

impl TextCandidateIndex {
    fn build(dataset: &EvalDataset) -> Self {
        let mut docs = Vec::with_capacity(dataset.memories.len());
        let mut postings = HashMap::<String, Vec<(usize, usize)>>::new();
        let mut document_frequency = HashMap::<String, usize>::new();
        let mut total_doc_len = 0usize;

        for memory in &dataset.memories {
            let mut term_frequency = HashMap::<String, usize>::new();
            for token in memory_tokens(memory) {
                *term_frequency.entry(token).or_insert(0) += 1;
            }
            let doc_index = docs.len();
            for (token, frequency) in &term_frequency {
                postings
                    .entry(token.clone())
                    .or_default()
                    .push((doc_index, *frequency));
                *document_frequency.entry(token.clone()).or_insert(0) += 1;
            }
            let length = term_frequency.values().sum::<usize>().max(1);
            total_doc_len += length;
            docs.push(Bm25Doc {
                id: memory.id.clone(),
                scope: memory.scope.clone(),
                length,
                term_frequency,
            });
        }

        let avg_doc_len = if docs.is_empty() {
            0.0
        } else {
            total_doc_len as f64 / docs.len() as f64
        };
        Self {
            docs,
            postings,
            document_frequency,
            avg_doc_len,
        }
    }

    fn search(&self, query: &EvalQuery, top_k: usize, mode: RecallMode) -> Vec<String> {
        let query_terms = tokenize(&query.query);
        let mut scores = HashMap::<usize, f64>::new();
        let doc_count = self.docs.len() as f64;
        for term in &query_terms {
            let Some(postings) = self.postings.get(term) else {
                continue;
            };
            let df = self.document_frequency.get(term).copied().unwrap_or(0) as f64;
            for (doc_index, tf) in postings {
                let doc = &self.docs[*doc_index];
                if let Some(scope) = &query.scope {
                    if doc.scope != *scope {
                        continue;
                    }
                }
                *scores.entry(*doc_index).or_insert(0.0) +=
                    bm25_term_score(doc_count, df, *tf, doc.length, self.avg_doc_len);
            }
        }

        let mut scored = scores
            .into_iter()
            .filter_map(|(doc_index, score)| {
                let doc = self.docs.get(doc_index)?;
                Some((doc.id.clone(), score, doc.length))
            })
            .collect::<Vec<_>>();
        scored.sort_by(|a, b| {
            b.1.partial_cmp(&a.1)
                .unwrap_or(std::cmp::Ordering::Equal)
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

    fn score_doc(&self, doc: &Bm25Doc, query_terms: &BTreeSet<String>) -> f64 {
        let doc_count = self.docs.len() as f64;
        let mut score = 0.0;
        for term in query_terms {
            let Some(tf) = doc.term_frequency.get(term).copied() else {
                continue;
            };
            let df = self.document_frequency.get(term).copied().unwrap_or(0) as f64;
            score += bm25_term_score(doc_count, df, tf, doc.length, self.avg_doc_len);
        }
        score
    }
}

#[derive(Clone, Debug)]
struct PageTokenIndex {
    text_index: TextCandidateIndex,
    pages: Vec<PageTokenSummary>,
}

#[derive(Clone, Debug)]
struct PageTokenSummary {
    doc_indexes: Vec<usize>,
    tokens: BTreeSet<String>,
}

impl PageTokenIndex {
    fn build(dataset: &EvalDataset, page_size: usize) -> Self {
        let text_index = TextCandidateIndex::build(dataset);
        let mut pages = Vec::new();
        for chunk in (0..text_index.docs.len()).step_by(page_size.max(1)) {
            let end = (chunk + page_size.max(1)).min(text_index.docs.len());
            let doc_indexes = (chunk..end).collect::<Vec<_>>();
            let mut tokens = BTreeSet::new();
            for doc_index in &doc_indexes {
                tokens.extend(text_index.docs[*doc_index].term_frequency.keys().cloned());
            }
            pages.push(PageTokenSummary {
                doc_indexes,
                tokens,
            });
        }
        Self { text_index, pages }
    }

    fn search(&self, query: &EvalQuery, top_k: usize, mode: RecallMode) -> Vec<String> {
        let query_terms = tokenize(&query.query);
        let mut scored = Vec::new();
        for page in &self.pages {
            if !query_terms.iter().any(|term| page.tokens.contains(term)) {
                continue;
            }
            for doc_index in &page.doc_indexes {
                let doc = &self.text_index.docs[*doc_index];
                if let Some(scope) = &query.scope {
                    if doc.scope != *scope {
                        continue;
                    }
                }
                let score = self.text_index.score_doc(doc, &query_terms);
                if score > 0.0 {
                    scored.push((doc.id.clone(), score, doc.length));
                }
            }
        }
        scored.sort_by(|a, b| {
            b.1.partial_cmp(&a.1)
                .unwrap_or(std::cmp::Ordering::Equal)
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
}

fn memory_tokens(memory: &EvalMemory) -> Vec<String> {
    tokenize_vec(&format!(
        "{} {} {}",
        memory.subject.as_deref().unwrap_or_default(),
        memory.text,
        memory.markers.join(" ")
    ))
}

fn bm25_term_score(
    doc_count: f64,
    document_frequency: f64,
    term_frequency: usize,
    doc_len: usize,
    avg_doc_len: f64,
) -> f64 {
    let k1 = 1.2;
    let b = 0.75;
    let idf = ((doc_count - document_frequency + 0.5) / (document_frequency + 0.5) + 1.0).ln();
    let tf = term_frequency as f64;
    let doc_len = doc_len.max(1) as f64;
    let normalization = tf + k1 * (1.0 - b + b * doc_len / avg_doc_len.max(1.0));
    idf * (tf * (k1 + 1.0)) / normalization
}

impl RetrievalMetricAccumulator {
    fn record(&mut self, query: &EvalQuery, result: &QueryResult, top_k: usize) {
        self.samples += 1;
        if query.relevant_ids.is_empty() {
            self.negative_samples += 1;
            self.no_answer_sum += f64::from(result.returned_ids.is_empty());
            return;
        }

        self.positive_samples += 1;
        let expected = query.relevant_ids.iter().collect::<BTreeSet<_>>();
        let returned_at_k = result.returned_ids.iter().take(top_k).collect::<Vec<_>>();
        let returned_unique = returned_at_k.iter().copied().collect::<BTreeSet<_>>();
        let relevant_returned = returned_unique.intersection(&expected).count();

        self.hit_sum += f64::from(relevant_returned > 0);
        self.recall_sum += relevant_returned as f64 / expected.len() as f64;
        self.precision_sum += if returned_unique.is_empty() {
            0.0
        } else {
            relevant_returned as f64 / returned_unique.len() as f64
        };
        self.mrr_sum += returned_at_k
            .iter()
            .position(|id| expected.contains(*id))
            .map(|index| 1.0 / (index + 1) as f64)
            .unwrap_or(0.0);

        let mut seen = HashSet::new();
        let dcg = returned_at_k
            .iter()
            .enumerate()
            .filter(|(_, id)| expected.contains(**id) && seen.insert(id.as_str()))
            .map(|(index, _)| 1.0 / ((index + 2) as f64).log2())
            .sum::<f64>();
        let ideal = (0..expected.len().min(top_k))
            .map(|index| 1.0 / ((index + 2) as f64).log2())
            .sum::<f64>();
        self.ndcg_sum += if ideal > 0.0 { dcg / ideal } else { 0.0 };
    }

    fn category_summary(&self, category: String, repeats: usize) -> CategorySummary {
        let positive_denominator = self.positive_samples.max(1) as f64;
        CategorySummary {
            category,
            queries: self.samples / repeats,
            positive_queries: self.positive_samples / repeats,
            negative_queries: self.negative_samples / repeats,
            hit_at_k: self.hit_sum / positive_denominator,
            recall_at_k: self.recall_sum / positive_denominator,
            precision_at_k: self.precision_sum / positive_denominator,
            mrr_at_k: self.mrr_sum / positive_denominator,
            ndcg_at_k: self.ndcg_sum / positive_denominator,
            no_answer_accuracy: (self.negative_samples > 0)
                .then_some(self.no_answer_sum / self.negative_samples as f64),
        }
    }
}

#[allow(clippy::too_many_arguments)]
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
    let mut metrics = RetrievalMetricAccumulator::default();
    let mut category_metrics = BTreeMap::<String, RetrievalMetricAccumulator>::new();
    let mut context_bytes_sum = 0usize;
    let mut returned_items_sum = 0usize;
    let mut items_at_k_sum = 0usize;
    let mut hot_candidates_sum = 0usize;
    let mut pages_loaded_sum = 0usize;
    let mut pages_pruned_sum = 0usize;
    let mut cells_scanned_sum = 0usize;
    let mut store_open_micros_sum = 0u64;
    let mut page_read_micros_sum = 0u64;
    let mut page_decode_micros_sum = 0u64;
    let mut scoring_cache_build_micros_sum = 0u64;
    let mut cell_filtering_micros_sum = 0u64;
    let mut reranking_micros_sum = 0u64;
    let mut context_packet_build_micros_sum = 0u64;
    let mut latencies = Vec::with_capacity(results.len());

    for (query_index, query) in queries.iter().enumerate() {
        for repeat_index in 0..repeats {
            let result = &results[query_index * repeats + repeat_index];
            metrics.record(query, result, top_k);
            let category = if query.category.is_empty() {
                "uncategorized"
            } else {
                query.category.as_str()
            };
            category_metrics
                .entry(category.to_string())
                .or_default()
                .record(query, result, top_k);
            context_bytes_sum += result.context_bytes;
            returned_items_sum += result.returned_ids.len();
            items_at_k_sum += result.returned_ids.len().min(top_k);
            hot_candidates_sum += result.hot_candidates;
            pages_loaded_sum += result.pages_loaded;
            pages_pruned_sum += result.pages_pruned;
            cells_scanned_sum += result.cells_scanned;
            store_open_micros_sum = store_open_micros_sum.saturating_add(result.store_open_micros);
            page_read_micros_sum = page_read_micros_sum.saturating_add(result.page_read_micros);
            page_decode_micros_sum =
                page_decode_micros_sum.saturating_add(result.page_decode_micros);
            scoring_cache_build_micros_sum =
                scoring_cache_build_micros_sum.saturating_add(result.scoring_cache_build_micros);
            cell_filtering_micros_sum =
                cell_filtering_micros_sum.saturating_add(result.cell_filtering_micros);
            reranking_micros_sum = reranking_micros_sum.saturating_add(result.reranking_micros);
            context_packet_build_micros_sum =
                context_packet_build_micros_sum.saturating_add(result.context_packet_build_micros);
            latencies.push(result.latency_micros);
        }
    }

    latencies.sort_unstable();
    let samples = results.len().max(1);
    let positive_denominator = metrics.positive_samples.max(1) as f64;
    RunSummary {
        system,
        layer,
        index_kind,
        mode,
        queries: queries.len(),
        positive_queries: queries
            .iter()
            .filter(|query| !query.relevant_ids.is_empty())
            .count(),
        top_k,
        hit_at_k: metrics.hit_sum / positive_denominator,
        recall_at_k: metrics.recall_sum / positive_denominator,
        precision_at_k: metrics.precision_sum / positive_denominator,
        mrr_at_k: metrics.mrr_sum / positive_denominator,
        ndcg_at_k: metrics.ndcg_sum / positive_denominator,
        negative_queries: queries
            .iter()
            .filter(|query| query.relevant_ids.is_empty())
            .count(),
        no_answer_accuracy: (metrics.negative_samples > 0)
            .then_some(metrics.no_answer_sum / metrics.negative_samples as f64),
        abstention_analysis: summarize_abstention(queries, results, repeats, top_k),
        category_breakdown: category_metrics
            .into_iter()
            .map(|(category, metrics)| metrics.category_summary(category, repeats))
            .collect(),
        avg_latency_micros: average_u64(&latencies),
        avg_end_to_end_micros: average_u64(&latencies)
            .saturating_add(store_open_micros_sum / samples as u64),
        p50_latency_micros: percentile(&latencies, 50.0),
        p95_latency_micros: percentile(&latencies, 95.0),
        avg_context_bytes: context_bytes_sum / samples,
        avg_context_tokens_estimate: (context_bytes_sum / samples).div_ceil(4),
        avg_returned_items: returned_items_sum as f64 / samples as f64,
        avg_items_at_k: items_at_k_sum as f64 / samples as f64,
        avg_hot_candidates: hot_candidates_sum as f64 / samples as f64,
        avg_pages_loaded: pages_loaded_sum as f64 / samples as f64,
        avg_pages_pruned: pages_pruned_sum as f64 / samples as f64,
        avg_cells_scanned: cells_scanned_sum as f64 / samples as f64,
        avg_store_open_micros: store_open_micros_sum / samples as u64,
        avg_page_read_micros: page_read_micros_sum / samples as u64,
        avg_page_decode_micros: page_decode_micros_sum / samples as u64,
        avg_scoring_cache_build_micros: scoring_cache_build_micros_sum / samples as u64,
        avg_cell_filtering_micros: cell_filtering_micros_sum / samples as u64,
        avg_reranking_micros: reranking_micros_sum / samples as u64,
        avg_context_packet_build_micros: context_packet_build_micros_sum / samples as u64,
    }
}

fn summarize_abstention(
    queries: &[EvalQuery],
    results: &[QueryResult],
    repeats: usize,
    top_k: usize,
) -> Option<AbstentionAnalysis> {
    let positive_samples = queries
        .iter()
        .filter(|query| !query.relevant_ids.is_empty())
        .count()
        * repeats;
    let negative_samples = queries
        .iter()
        .filter(|query| query.relevant_ids.is_empty())
        .count()
        * repeats;
    if positive_samples == 0 || negative_samples == 0 {
        return None;
    }

    let mut thresholds = results
        .iter()
        .filter_map(|result| result.top_score)
        .collect::<Vec<_>>();
    let highest = thresholds.iter().copied().max()?;
    thresholds.push(highest.saturating_add(1));
    thresholds.sort_unstable();
    thresholds.dedup();

    let mut best: Option<AbstentionAnalysis> = None;
    for threshold in thresholds {
        let mut positive_correct = 0usize;
        let mut negative_correct = 0usize;
        for (query_index, query) in queries.iter().enumerate() {
            let expected = query.relevant_ids.iter().collect::<HashSet<_>>();
            for repeat_index in 0..repeats {
                let result = &results[query_index * repeats + repeat_index];
                let accepted = !result.returned_ids.is_empty()
                    && result.top_score.is_some_and(|score| score >= threshold);
                if expected.is_empty() {
                    negative_correct += usize::from(!accepted);
                } else if accepted
                    && result
                        .returned_ids
                        .iter()
                        .take(top_k)
                        .any(|id| expected.contains(id))
                {
                    positive_correct += 1;
                }
            }
        }

        let positive_hit_accept_rate = positive_correct as f64 / positive_samples as f64;
        let negative_reject_rate = negative_correct as f64 / negative_samples as f64;
        let candidate = AbstentionAnalysis {
            threshold_score: threshold,
            balanced_accuracy: (positive_hit_accept_rate + negative_reject_rate) / 2.0,
            positive_hit_accept_rate,
            negative_reject_rate,
            positive_samples,
            negative_samples,
        };
        let replace = best.as_ref().is_none_or(|current| {
            candidate.balanced_accuracy > current.balanced_accuracy
                || (candidate.balanced_accuracy == current.balanced_accuracy
                    && (candidate.negative_reject_rate > current.negative_reject_rate
                        || (candidate.negative_reject_rate == current.negative_reject_rate
                            && candidate.threshold_score > current.threshold_score)))
        });
        if replace {
            best = Some(candidate);
        }
    }
    best
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

fn generate_dataset_with_queries(
    profile: GeneratedProfile,
    query_profile: GeneratedQueryProfile,
) -> EvalDataset {
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
                if matches!(
                    query_profile,
                    GeneratedQueryProfile::Lexical | GeneratedQueryProfile::Mixed
                ) {
                    queries.push(EvalQuery {
                        id: format!("q:lexical:{scope}:{component}:{row}"),
                        query: format!(
                            "In {scope}, what does integration note {row} record for {component} {decision}?"
                        ),
                        scope: Some(scope.clone()),
                        relevant_ids: vec![id.clone()],
                        category: "lexical_single_fact".to_string(),
                    });
                }
                if matches!(
                    query_profile,
                    GeneratedQueryProfile::Paraphrase | GeneratedQueryProfile::Mixed
                ) {
                    queries.push(EvalQuery {
                        id: format!("q:paraphrase:{scope}:{component}:{row}"),
                        query: format!(
                            "For {scope}, which choice is attached to {component} record number {row}?"
                        ),
                        scope: Some(scope.clone()),
                        relevant_ids: vec![id.clone()],
                        category: "partial_paraphrase".to_string(),
                    });
                }
                if matches!(
                    query_profile,
                    GeneratedQueryProfile::HardNegative | GeneratedQueryProfile::Mixed
                ) {
                    queries.push(EvalQuery {
                        id: format!("q:negative:{scope}:{component}:{row}"),
                        query: format!(
                            "In {scope}, what deployment deadline is recorded for missing item {row}?"
                        ),
                        scope: Some(scope.clone()),
                        relevant_ids: Vec::new(),
                        category: "hard_negative".to_string(),
                    });
                }
            }
        }
    }

    EvalDataset {
        name: format!("generated_{profile:?}_{}", query_profile.as_str()).to_ascii_lowercase(),
        source: format!(
            "generated safe agent-memory-style fixture ({})",
            query_profile.as_str()
        ),
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

fn text_report(report: &EvalReport) -> String {
    let mut output = String::new();
    output.push_str("Agent Memory Eval\n");
    output.push_str("=================\n");
    output.push_str(&format!(
        "dataset: {} ({})",
        report.dataset.name, report.dataset.source
    ));
    output.push('\n');
    output.push_str(&format!(
        "memories: {} | queries: {} | avg memory bytes: {:.1} | avg markers/memory: {:.1}",
        report.dataset.memories,
        report.dataset.queries,
        report.dataset.avg_memory_bytes,
        report.dataset.avg_markers_per_memory
    ));
    output.push_str("\n\n");
    output.push_str(&format!(
        "{:<28} {:<18} {:<18} {:<8} {:>7} {:>7} {:>7} {:>7} {:>7} {:>8} {:>8} {:>8} {:>8} {:>9} {:>8}",
        "system",
        "layer",
        "index",
        "mode",
        "hit@k",
        "rec@k",
        "prec@k",
        "mrr@k",
        "ndcg@k",
        "avg_us",
        "e2e_us",
        "p50_us",
        "p95_us",
        "ctx_bytes",
        "items"
    ));
    output.push('\n');
    output.push_str(&"-".repeat(169));
    output.push('\n');
    for run in &report.runs {
        output.push_str(&format!(
            "{:<28} {:<18} {:<18} {:<8} {:>7.3} {:>7.3} {:>7.3} {:>7.3} {:>7.3} {:>8} {:>8} {:>8} {:>8} {:>9} {:>8.2}",
            run.system,
            run.layer,
            run.index_kind,
            run.mode,
            run.hit_at_k,
            run.recall_at_k,
            run.precision_at_k,
            run.mrr_at_k,
            run.ndcg_at_k,
            run.avg_latency_micros,
            run.avg_end_to_end_micros,
            run.p50_latency_micros,
            run.p95_latency_micros,
            run.avg_context_bytes,
            run.avg_returned_items
        ));
        output.push('\n');
    }
    output.push_str("\ncontext budget (averages; token estimate is UTF-8 bytes / 4):\n");
    output.push_str(&format!(
        "{:<28} {:<18} {:<8} {:>10} {:>10} {:>10} {:>12}\n",
        "system", "layer", "mode", "out_items", "items@k", "ctx_bytes", "est_tokens"
    ));
    output.push_str(&"-".repeat(105));
    output.push('\n');
    for run in &report.runs {
        output.push_str(&format!(
            "{:<28} {:<18} {:<8} {:>10.2} {:>10.2} {:>10} {:>12}\n",
            run.system,
            run.layer,
            run.mode,
            run.avg_returned_items,
            run.avg_items_at_k,
            run.avg_context_bytes,
            run.avg_context_tokens_estimate
        ));
    }
    output.push_str("\ncategory breakdown (positive retrieval metrics use strict top-k):\n");
    output.push_str(&format!(
        "{:<28} {:<18} {:<8} {:<26} {:>7} {:>7} {:>7} {:>7} {:>7}\n",
        "system", "layer", "mode", "category", "hit@k", "rec@k", "mrr@k", "ndcg@k", "no_ans"
    ));
    output.push_str(&"-".repeat(135));
    output.push('\n');
    for run in &report.runs {
        for category in &run.category_breakdown {
            output.push_str(&format!(
                "{:<28} {:<18} {:<8} {:<26} {:>7.3} {:>7.3} {:>7.3} {:>7.3} {:>7}\n",
                run.system,
                run.layer,
                run.mode,
                category.category,
                category.hit_at_k,
                category.recall_at_k,
                category.mrr_at_k,
                category.ndcg_at_k,
                category
                    .no_answer_accuracy
                    .map(|value| format!("{value:.3}"))
                    .unwrap_or_else(|| "-".to_string())
            ));
        }
    }
    if report.runs.iter().any(|run| run.negative_queries > 0) {
        output.push_str("\nhard-negative rejection:\n");
        output.push_str(&format!(
            "{:<28} {:<18} {:<8} {:>10} {:>16}\n",
            "system", "layer", "mode", "queries", "no_answer_acc"
        ));
        output.push_str(&"-".repeat(86));
        output.push('\n');
        for run in report.runs.iter().filter(|run| run.negative_queries > 0) {
            output.push_str(&format!(
                "{:<28} {:<18} {:<8} {:>10} {:>16.3}\n",
                run.system,
                run.layer,
                run.mode,
                run.negative_queries,
                run.no_answer_accuracy.unwrap_or_default()
            ));
        }
    }
    if report
        .runs
        .iter()
        .any(|run| run.abstention_analysis.is_some())
    {
        output.push_str("\neval-only score threshold sweep:\n");
        output.push_str(&format!(
            "{:<28} {:<18} {:<8} {:>10} {:>10} {:>10} {:>10}\n",
            "system", "layer", "mode", "threshold", "balanced", "pos_hit", "neg_reject"
        ));
        output.push_str(&"-".repeat(105));
        output.push('\n');
        for run in report
            .runs
            .iter()
            .filter(|run| run.abstention_analysis.is_some())
        {
            let analysis = run.abstention_analysis.as_ref().unwrap();
            output.push_str(&format!(
                "{:<28} {:<18} {:<8} {:>10} {:>10.3} {:>10.3} {:>10.3}\n",
                run.system,
                run.layer,
                run.mode,
                analysis.threshold_score,
                analysis.balanced_accuracy,
                analysis.positive_hit_accept_rate,
                analysis.negative_reject_rate
            ));
        }
    }
    output.push_str("\nwork counters (averages):\n");
    output.push_str(&format!(
        "{:<28} {:<18} {:<8} {:>10} {:>10} {:>10} {:>12}\n",
        "system", "layer", "mode", "hot_cand", "pages", "pruned", "cells_scan"
    ));
    output.push_str(&"-".repeat(106));
    output.push('\n');
    for run in &report.runs {
        output.push_str(&format!(
            "{:<28} {:<18} {:<8} {:>10.1} {:>10.1} {:>10.1} {:>12.1}\n",
            run.system,
            run.layer,
            run.mode,
            run.avg_hot_candidates,
            run.avg_pages_loaded,
            run.avg_pages_pruned,
            run.avg_cells_scanned
        ));
    }
    output.push_str("\ntiming breakdown (averages, microseconds):\n");
    output.push_str(&format!(
        "{:<28} {:<18} {:<8} {:>8} {:>8} {:>8} {:>10} {:>10} {:>8} {:>10}\n",
        "system",
        "layer",
        "mode",
        "open",
        "read",
        "decode",
        "score_build",
        "filter",
        "rerank",
        "packet"
    ));
    output.push_str(&"-".repeat(125));
    output.push('\n');
    for run in &report.runs {
        output.push_str(&format!(
            "{:<28} {:<18} {:<8} {:>8} {:>8} {:>8} {:>10} {:>10} {:>8} {:>10}\n",
            run.system,
            run.layer,
            run.mode,
            run.avg_store_open_micros,
            run.avg_page_read_micros,
            run.avg_page_decode_micros,
            run.avg_scoring_cache_build_micros,
            run.avg_cell_filtering_micros,
            run.avg_reranking_micros,
            run.avg_context_packet_build_micros
        ));
    }
    output.push_str("\nnotes:\n");
    output.push_str("- avg_us measures recall after engine open; e2e_us adds store_open for sealed_cold runs.\n");
    output.push_str("- retrieval metrics exclude negative/abstention queries and score only the first top_k returned ids.\n");
    output.push_str(
        "- hard-negative queries count as correct only when retrieval returns no items.\n",
    );
    output.push_str(
        "- threshold sweep is diagnostic only and does not change production recall output.\n",
    );
    for note in &report.notes {
        output.push_str(&format!("- {note}\n"));
    }
    output
}

fn write_or_print_report(args: &Args, text: &str) -> Result<()> {
    if let Some(report_path) = &args.report {
        if let Some(parent) = report_path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(report_path, text)
            .with_context(|| format!("failed to write report {}", report_path.display()))?;
        println!("wrote report {}", report_path.display());
    } else {
        println!("{text}");
    }
    Ok(())
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
    tokenize_vec(input).into_iter().collect()
}

fn tokenize_vec(input: &str) -> Vec<String> {
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
        let dataset =
            generate_dataset_with_queries(GeneratedProfile::Tiny, GeneratedQueryProfile::Lexical);
        validate_dataset(&dataset).unwrap();
        assert!(!dataset.memories.is_empty());
        assert!(!dataset.queries.is_empty());
        for query in &dataset.queries {
            let row = query.relevant_ids[0].rsplit(':').next().unwrap();
            assert!(query.query.contains(&format!("note {row}")));
        }
    }

    #[test]
    fn generated_query_profiles_cover_paraphrases_and_hard_negatives() {
        let paraphrase = generate_dataset_with_queries(
            GeneratedProfile::Tiny,
            GeneratedQueryProfile::Paraphrase,
        );
        validate_dataset(&paraphrase).unwrap();
        assert!(paraphrase
            .queries
            .iter()
            .all(|query| query.category == "partial_paraphrase"));
        assert!(paraphrase
            .queries
            .iter()
            .all(|query| !query.relevant_ids.is_empty()));

        let negatives = generate_dataset_with_queries(
            GeneratedProfile::Tiny,
            GeneratedQueryProfile::HardNegative,
        );
        validate_dataset(&negatives).unwrap();
        assert!(negatives
            .queries
            .iter()
            .all(|query| query.relevant_ids.is_empty()));

        let mixed =
            generate_dataset_with_queries(GeneratedProfile::Tiny, GeneratedQueryProfile::Mixed);
        validate_dataset(&mixed).unwrap();
        assert_eq!(mixed.queries.len(), paraphrase.queries.len() * 3);
    }

    #[test]
    fn schema_example_is_valid() {
        validate_dataset(&schema_example()).unwrap();
    }

    #[test]
    fn converts_longmemeval_turn_level_evidence() {
        let value = serde_json::json!([
            {
                "question_id": "q1",
                "question_type": "single-session-user",
                "question": "What color is the notebook?",
                "haystack_session_ids": ["s1"],
                "answer_session_ids": ["s1"],
                "haystack_sessions": [[
                    {"role": "user", "content": "The notebook is blue.", "has_answer": true},
                    {"role": "assistant", "content": "Got it."}
                ]]
            }
        ]);
        let dataset =
            convert_longmemeval(&value, "test".to_string(), None, None, IngestMode::RawTurn)
                .expect("convert");
        validate_dataset(&dataset).unwrap();
        assert_eq!(dataset.memories.len(), 2);
        assert_eq!(dataset.queries.len(), 1);
        assert_eq!(dataset.queries[0].relevant_ids.len(), 1);
    }

    #[test]
    fn converts_longmemeval_session_level_evidence() {
        let value = serde_json::json!([
            {
                "question_id": "q1",
                "question_type": "single-session-user",
                "question": "What color is the notebook?",
                "haystack_session_ids": ["s1", "s2"],
                "answer_session_ids": ["s1"],
                "haystack_sessions": [
                    [
                        {"role": "user", "content": "The notebook is blue."},
                        {"role": "assistant", "content": "Got it."}
                    ],
                    [
                        {"role": "user", "content": "The folder is red."}
                    ]
                ]
            }
        ]);
        let dataset = convert_longmemeval(
            &value,
            "test".to_string(),
            None,
            None,
            IngestMode::SessionDoc,
        )
        .expect("convert");
        validate_dataset(&dataset).unwrap();
        assert_eq!(dataset.memories.len(), 2);
        assert_eq!(dataset.queries[0].relevant_ids.len(), 1);
        assert!(dataset.queries[0].relevant_ids[0].ends_with(":session"));
    }

    #[test]
    fn converts_longmemeval_abstention_as_negative_even_with_answer_sessions() {
        let value = serde_json::json!([{
            "question_id": "q1_abs",
            "question_type": "single-session-user",
            "question": "What event never happened?",
            "haystack_session_ids": ["s1"],
            "answer_session_ids": ["s1"],
            "haystack_sessions": [[
                {"role": "user", "content": "The notebook is blue."}
            ]]
        }]);
        let dataset = convert_longmemeval(
            &value,
            "test".to_string(),
            None,
            None,
            IngestMode::SessionChunk,
        )
        .expect("convert");
        validate_dataset(&dataset).unwrap();
        assert_eq!(dataset.queries.len(), 1);
        assert!(dataset.queries[0].relevant_ids.is_empty());
        assert_eq!(dataset.queries[0].category, "abstention");
    }

    #[test]
    fn converts_longmemeval_session_chunks_with_evidence() {
        let turns = (0..10)
            .map(|index| {
                serde_json::json!({
                    "role": "user",
                    "content": format!("Context statement number {index} with useful detail."),
                    "has_answer": index == 9
                })
            })
            .collect::<Vec<_>>();
        let value = serde_json::json!([{
            "question_id": "q1",
            "question_type": "single-session-user",
            "question": "Which statement has the answer?",
            "haystack_session_ids": ["s1"],
            "answer_session_ids": ["s1"],
            "haystack_sessions": [turns]
        }]);
        let dataset = convert_longmemeval(
            &value,
            "test".to_string(),
            None,
            None,
            IngestMode::SessionChunk,
        )
        .expect("convert");

        validate_dataset(&dataset).unwrap();
        assert_eq!(dataset.memories.len(), 2);
        assert_eq!(dataset.queries[0].relevant_ids.len(), 1);
        assert!(dataset.queries[0].relevant_ids[0].ends_with("chunk-1"));
    }

    #[test]
    fn converts_longmemeval_key_fact_evidence() {
        let value = serde_json::json!([
            {
                "question_id": "q1",
                "question_type": "single-session-user",
                "question": "What color is the notebook?",
                "haystack_session_ids": ["s1"],
                "answer_session_ids": ["s1"],
                "haystack_sessions": [[
                    {"role": "user", "content": "The notebook is blue. The folder is red.", "has_answer": true}
                ]]
            }
        ]);
        let dataset =
            convert_longmemeval(&value, "test".to_string(), None, None, IngestMode::KeyFact)
                .expect("convert");
        validate_dataset(&dataset).unwrap();
        assert_eq!(dataset.memories.len(), 2);
        assert!(dataset.queries[0]
            .relevant_ids
            .iter()
            .all(|id| id.contains(":fact-")));
    }

    #[test]
    fn converts_locomo_evidence_ids() {
        let value = serde_json::json!([
            {
                "conversation_id": "c1",
                "session_1": [
                    {"dialog_id": "d1", "speaker": "Alice", "text": "Alice ordered tea."},
                    {"dialog_id": "d2", "speaker": "Bob", "text": "Bob ordered coffee."}
                ],
                "qa": [
                    {"question": "What did Alice order?", "evidence": ["d1"], "category": 1}
                ]
            }
        ]);
        let dataset = convert_locomo(&value, "test".to_string(), None, None).expect("convert");
        validate_dataset(&dataset).unwrap();
        assert_eq!(dataset.memories.len(), 2);
        assert_eq!(dataset.queries.len(), 1);
        assert_eq!(dataset.queries[0].relevant_ids.len(), 1);
    }

    #[test]
    fn scan_baseline_finds_generated_relevant_memory() {
        let dataset =
            generate_dataset_with_queries(GeneratedProfile::Tiny, GeneratedQueryProfile::Lexical);
        let query = &dataset.queries[0];
        let returned = scan_baseline(&dataset, query, 5, RecallMode::Focused);
        assert!(returned.contains(&query.relevant_ids[0]));
    }

    #[test]
    fn bm25_baseline_finds_generated_relevant_memory() {
        let dataset =
            generate_dataset_with_queries(GeneratedProfile::Tiny, GeneratedQueryProfile::Lexical);
        let query = &dataset.queries[0];
        let index = Bm25Index::build(&dataset);
        let returned = index.search(query, 5, RecallMode::Focused);
        assert!(returned.contains(&query.relevant_ids[0]));
    }

    #[test]
    fn text_candidate_index_finds_generated_relevant_memory() {
        let dataset =
            generate_dataset_with_queries(GeneratedProfile::Tiny, GeneratedQueryProfile::Lexical);
        let query = &dataset.queries[0];
        let index = TextCandidateIndex::build(&dataset);
        let returned = index.search(query, 5, RecallMode::Focused);
        assert!(returned.contains(&query.relevant_ids[0]));
    }

    #[test]
    fn page_token_index_finds_generated_relevant_memory() {
        let dataset =
            generate_dataset_with_queries(GeneratedProfile::Tiny, GeneratedQueryProfile::Lexical);
        let query = &dataset.queries[0];
        let index = PageTokenIndex::build(&dataset, 8);
        let returned = index.search(query, 5, RecallMode::Focused);
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
            hot_candidates: 2,
            pages_loaded: 0,
            pages_pruned: 0,
            cells_scanned: 2,
            ..QueryResult::default()
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
        assert_eq!(summary.mrr_at_k, 1.0);
        assert_eq!(summary.ndcg_at_k, 1.0);
        assert_eq!(summary.no_answer_accuracy, None);
        assert_eq!(summary.positive_queries, 1);
        assert_eq!(summary.avg_items_at_k, 2.0);
        assert_eq!(summary.avg_context_tokens_estimate, 25);
        assert_eq!(summary.category_breakdown.len(), 1);

        let report = EvalReport {
            dataset: summarize_dataset(&generate_dataset_with_queries(
                GeneratedProfile::Tiny,
                GeneratedQueryProfile::Lexical,
            )),
            runs: vec![summary],
            notes: Vec::new(),
        };
        let rendered = text_report(&report);
        assert!(rendered.contains("timing breakdown (averages, microseconds):"));
        assert!(rendered.contains("score_build"));
    }

    #[test]
    fn run_summary_measures_rank_and_negative_rejection() {
        let ranked_query = EvalQuery {
            id: "ranked".to_string(),
            query: "target".to_string(),
            scope: None,
            relevant_ids: vec!["target".to_string()],
            category: "ranked".to_string(),
        };
        let ranked = summarize_run(
            "test".to_string(),
            "test".to_string(),
            "none".to_string(),
            "focused".to_string(),
            5,
            1,
            &[ranked_query],
            &[QueryResult {
                returned_ids: vec!["distractor".to_string(), "target".to_string()],
                ..QueryResult::default()
            }],
        );
        assert_eq!(ranked.mrr_at_k, 0.5);
        assert!((ranked.ndcg_at_k - 0.630_929_753).abs() < 1e-6);

        let negative_query = EvalQuery {
            id: "negative".to_string(),
            query: "missing".to_string(),
            scope: None,
            relevant_ids: Vec::new(),
            category: "hard_negative".to_string(),
        };
        let rejected = summarize_run(
            "test".to_string(),
            "test".to_string(),
            "none".to_string(),
            "focused".to_string(),
            5,
            1,
            &[negative_query],
            &[QueryResult::default()],
        );
        assert_eq!(rejected.negative_queries, 1);
        assert_eq!(rejected.no_answer_accuracy, Some(1.0));
        assert_eq!(rejected.positive_queries, 0);
        assert_eq!(rejected.mrr_at_k, 0.0);
        assert_eq!(rejected.ndcg_at_k, 0.0);
        assert_eq!(rejected.category_breakdown[0].no_answer_accuracy, Some(1.0));
    }

    #[test]
    fn run_summary_applies_strict_top_k_to_broad_output() {
        let query = EvalQuery {
            id: "strict-top-k".to_string(),
            query: "target".to_string(),
            scope: None,
            relevant_ids: vec!["target".to_string()],
            category: "ranking".to_string(),
        };
        let summary = summarize_run(
            "mge".to_string(),
            "sealed_repeated".to_string(),
            "exact".to_string(),
            "broad".to_string(),
            5,
            1,
            &[query],
            &[QueryResult {
                returned_ids: vec![
                    "d1".to_string(),
                    "d2".to_string(),
                    "d3".to_string(),
                    "d4".to_string(),
                    "d5".to_string(),
                    "target".to_string(),
                ],
                ..QueryResult::default()
            }],
        );

        assert_eq!(summary.avg_returned_items, 6.0);
        assert_eq!(summary.avg_items_at_k, 5.0);
        assert_eq!(summary.hit_at_k, 0.0);
        assert_eq!(summary.recall_at_k, 0.0);
        assert_eq!(summary.mrr_at_k, 0.0);
        assert_eq!(summary.ndcg_at_k, 0.0);
    }

    #[test]
    fn run_summary_finds_eval_only_abstention_threshold() {
        let queries = vec![
            EvalQuery {
                id: "positive".to_string(),
                query: "known".to_string(),
                scope: None,
                relevant_ids: vec!["target".to_string()],
                category: "positive".to_string(),
            },
            EvalQuery {
                id: "negative".to_string(),
                query: "unknown".to_string(),
                scope: None,
                relevant_ids: Vec::new(),
                category: "abstention".to_string(),
            },
        ];
        let results = vec![
            QueryResult {
                returned_ids: vec!["target".to_string()],
                top_score: Some(80),
                ..QueryResult::default()
            },
            QueryResult {
                returned_ids: vec!["distractor".to_string()],
                top_score: Some(20),
                ..QueryResult::default()
            },
        ];
        let summary = summarize_run(
            "mge".to_string(),
            "hot".to_string(),
            "exact".to_string(),
            "focused".to_string(),
            5,
            1,
            &queries,
            &results,
        );
        let analysis = summary.abstention_analysis.expect("threshold analysis");
        assert_eq!(analysis.threshold_score, 80);
        assert_eq!(analysis.balanced_accuracy, 1.0);
        assert_eq!(analysis.positive_hit_accept_rate, 1.0);
        assert_eq!(analysis.negative_reject_rate, 1.0);
    }
}
