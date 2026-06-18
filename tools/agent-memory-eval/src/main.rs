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
    if dataset
        .queries
        .iter()
        .all(|query| query.relevant_ids.is_empty())
    {
        return Err(anyhow!(
            "dataset has no queries with relevant_ids; retrieval metrics need evidence labels"
        ));
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
                if turn
                    .get("has_answer")
                    .and_then(Value::as_bool)
                    .unwrap_or(false)
                {
                    relevant_ids.push(id.clone());
                }
                session_turn_ids
                    .entry(session_id.clone())
                    .or_default()
                    .push(id.clone());
                memories.push(EvalMemory {
                    id,
                    scope: scope.clone(),
                    subject: Some(format!("{question_type} {role} {session_id}")),
                    text: content,
                    markers: vec![
                        format!("benchmark:longmemeval"),
                        format!("question_type:{}", safe_marker_value(&question_type)),
                        format!("role:{}", safe_marker_value(&role)),
                        format!("session:{}", safe_marker_value(&session_id)),
                    ],
                    kind: "project_fact".to_string(),
                    status: "active".to_string(),
                    trust: "user_confirmed".to_string(),
                    sensitivity: "private".to_string(),
                });
            }
        }

        if relevant_ids.is_empty() {
            for answer_session_id in &answer_session_ids {
                if let Some(turn_ids) = session_turn_ids.get(answer_session_id) {
                    relevant_ids.extend(turn_ids.iter().cloned());
                }
            }
        }
        relevant_ids.sort();
        relevant_ids.dedup();

        if relevant_ids.is_empty() {
            skipped_without_evidence += 1;
            continue;
        }

        queries.push(EvalQuery {
            id: question_id,
            query: question,
            scope: Some(scope),
            relevant_ids,
            category: question_type,
        });
        if max_queries.is_some_and(|max| queries.len() >= max) {
            break;
        }
        if max_memories.is_some_and(|max| memories.len() >= max) {
            break;
        }
    }

    Ok(EvalDataset {
        name: "longmemeval_converted".to_string(),
        source: format!("{source}; skipped queries without evidence: {skipped_without_evidence}"),
        memories,
        queries,
    })
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
            "LongMemEval and best-effort LoCoMo-style JSON adapters are local-only; no datasets are committed or downloaded by the tool.".to_string(),
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
            if expected.is_empty() {
                hit_sum += if returned.is_empty() { 1.0 } else { 0.0 };
                recall_sum += if returned.is_empty() { 1.0 } else { 0.0 };
            } else {
                hit_sum += if relevant_returned > 0 { 1.0 } else { 0.0 };
                recall_sum += relevant_returned as f64 / expected.len() as f64;
            }
            precision_sum += if returned.is_empty() {
                if expected.is_empty() {
                    1.0
                } else {
                    0.0
                }
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
    ));
    output.push('\n');
    output.push_str(&"-".repeat(134));
    output.push('\n');
    for run in &report.runs {
        output.push_str(&format!(
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
        ));
        output.push('\n');
    }
    output.push_str("\nnotes:\n");
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
        let dataset = convert_longmemeval(&value, "test".to_string(), None, None).expect("convert");
        validate_dataset(&dataset).unwrap();
        assert_eq!(dataset.memories.len(), 2);
        assert_eq!(dataset.queries.len(), 1);
        assert_eq!(dataset.queries[0].relevant_ids.len(), 1);
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
