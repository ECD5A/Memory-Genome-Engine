use std::fs;
use std::io::{self, BufRead, Write};
use std::path::PathBuf;
use std::str::FromStr;

use anyhow::{anyhow, bail, Context, Result};
use mge_core::{
    CellId, MemoryEngine, MemoryKind, MemorySource, MemoryStatus, MemoryValue, RecallMode,
    RecallRequest, RememberRequest, SensitivityLevel, TrustLevel,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

const JSONRPC_VERSION: &str = "2.0";

#[derive(Debug, Deserialize)]
struct JsonRpcRequest {
    #[serde(default)]
    jsonrpc: Option<String>,
    #[serde(default)]
    id: Option<Value>,
    method: String,
    #[serde(default)]
    params: Value,
}

#[derive(Debug, Serialize)]
struct JsonRpcResponse {
    jsonrpc: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<JsonRpcError>,
}

#[derive(Debug, Serialize)]
struct JsonRpcError {
    code: i64,
    message: String,
}

#[derive(Debug, Deserialize)]
struct StoreParams {
    store_path: PathBuf,
}

#[derive(Debug, Deserialize)]
struct RememberParams {
    store_path: PathBuf,
    content: String,
    #[serde(default = "default_kind")]
    kind: String,
    #[serde(default = "default_scope")]
    scope: String,
    #[serde(default)]
    markers: Vec<String>,
    #[serde(default = "default_trust")]
    trust: String,
    #[serde(default = "default_sensitivity")]
    sensitivity: String,
    #[serde(default = "default_status")]
    status: String,
    #[serde(default)]
    subject: Option<String>,
    #[serde(default)]
    source_type: Option<String>,
    #[serde(default)]
    source_ref: Option<String>,
    #[serde(default)]
    links: Vec<CellId>,
}

#[derive(Debug, Deserialize)]
struct RecallParams {
    store_path: PathBuf,
    #[serde(default)]
    query: String,
    #[serde(default = "default_recall_mode")]
    mode: String,
    #[serde(default)]
    scope: Option<String>,
    #[serde(default)]
    markers: Vec<String>,
    #[serde(default)]
    max_items: Option<usize>,
    #[serde(default)]
    kind: Option<String>,
    #[serde(default)]
    include_deprecated: bool,
    #[serde(default)]
    include_secret_references: bool,
}

#[derive(Debug, Deserialize)]
struct ValidateParams {
    store_path: PathBuf,
    #[serde(default)]
    deep: bool,
}

#[derive(Debug, Deserialize)]
struct ExportMarkdownParams {
    store_path: PathBuf,
    #[serde(default)]
    output_path: Option<PathBuf>,
}

fn main() -> Result<()> {
    let stdin = io::stdin();
    let mut stdout = io::stdout();

    for line in stdin.lock().lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }

        let response = handle_line(&line);
        serde_json::to_writer(&mut stdout, &response)?;
        stdout.write_all(b"\n")?;
        stdout.flush()?;
    }

    Ok(())
}

fn handle_line(line: &str) -> JsonRpcResponse {
    match serde_json::from_str::<JsonRpcRequest>(line) {
        Ok(request) => {
            let id = request.id.clone();
            if request.jsonrpc.as_deref().unwrap_or(JSONRPC_VERSION) != JSONRPC_VERSION {
                return error_response(id, -32600, "jsonrpc must be \"2.0\"");
            }
            match handle_request(request) {
                Ok(result) => JsonRpcResponse {
                    jsonrpc: JSONRPC_VERSION,
                    id,
                    result: Some(result),
                    error: None,
                },
                Err(err) => error_response(id, -32000, err.to_string()),
            }
        }
        Err(err) => error_response(
            None,
            -32700,
            format!("failed to parse JSON-RPC request: {err}"),
        ),
    }
}

fn handle_request(request: JsonRpcRequest) -> Result<Value> {
    match request.method.as_str() {
        "mge_remember" => mge_remember(request.params),
        "mge_recall" => mge_recall(request.params),
        "mge_seal" => mge_seal(request.params),
        "mge_checkpoint" => mge_checkpoint(request.params),
        "mge_stats" => mge_stats(request.params),
        "mge_validate" => mge_validate(request.params),
        "mge_rebuild_indexes" => mge_rebuild_indexes(request.params),
        "mge_export_markdown" => mge_export_markdown(request.params),
        other => bail!("unknown method: {other}"),
    }
}

fn mge_remember(params: Value) -> Result<Value> {
    let params: RememberParams = parse_params(params)?;
    let mut engine = open_engine(&params.store_path)?;
    let mut request = RememberRequest::new(
        MemoryKind::from_str(&params.kind)?,
        MemoryValue::Text(params.content),
    );
    request.scope = params.scope;
    request.markers = params.markers;
    request.trust = TrustLevel::from_str(&params.trust)?;
    request.sensitivity = SensitivityLevel::from_str(&params.sensitivity)?;
    request.status = MemoryStatus::from_str(&params.status)?;
    request.subject = params.subject;
    request.source = parse_memory_source(params.source_type, params.source_ref)?;
    request.links = params.links;

    let cell = engine.remember(request)?;
    Ok(json!({
        "ok": true,
        "cell_id": cell.id,
        "scope": cell.scope,
        "kind": cell.kind,
        "status": cell.status,
        "json_runtime_storage": false
    }))
}

fn mge_recall(params: Value) -> Result<Value> {
    let params: RecallParams = parse_params(params)?;
    let engine = open_engine(&params.store_path)?;
    let mode = RecallMode::from_str(&params.mode)?;
    if mode == RecallMode::FullScope && params.scope.is_none() {
        bail!("full_scope recall requires scope");
    }
    if matches!(mode, RecallMode::Focused | RecallMode::Broad) && params.query.trim().is_empty() {
        bail!("focused and broad recall require query");
    }

    let mut request = RecallRequest::new(params.query);
    request.mode = mode;
    request.markers = params.markers;
    request.scope = params.scope;
    request.include_deprecated = params.include_deprecated;
    request.include_secret_references = params.include_secret_references;
    if let Some(max_items) = params.max_items {
        request.max_items = max_items;
    }
    request.kind = params
        .kind
        .as_deref()
        .map(MemoryKind::from_str)
        .transpose()?;

    let packet = engine.recall(request)?;
    Ok(json!({
        "ok": true,
        "context_packet": packet,
        "json_runtime_storage": false
    }))
}

fn mge_seal(params: Value) -> Result<Value> {
    let params: StoreParams = parse_params(params)?;
    let mut engine = open_engine(&params.store_path)?;
    let report = engine.seal()?;
    Ok(json!({ "ok": true, "seal": report }))
}

fn mge_checkpoint(params: Value) -> Result<Value> {
    let params: StoreParams = parse_params(params)?;
    let mut engine = open_engine(&params.store_path)?;
    let report = engine.checkpoint()?;
    Ok(json!({ "ok": true, "checkpoint": report }))
}

fn mge_stats(params: Value) -> Result<Value> {
    let params: StoreParams = parse_params(params)?;
    let engine = open_engine(&params.store_path)?;
    let stats = engine.stats()?;
    Ok(json!({ "ok": true, "stats": stats }))
}

fn mge_validate(params: Value) -> Result<Value> {
    let params: ValidateParams = parse_params(params)?;
    let engine = open_engine(&params.store_path)?;
    let report = if params.deep {
        engine.validate_deep()?
    } else {
        engine.validate()?
    };
    Ok(json!({ "ok": report.ok, "validation": report }))
}

fn mge_rebuild_indexes(params: Value) -> Result<Value> {
    let params: StoreParams = parse_params(params)?;
    let engine = open_engine(&params.store_path)?;
    let report = engine.rebuild_catalog_and_indexes()?;
    Ok(json!({ "ok": true, "rebuild": report }))
}

fn mge_export_markdown(params: Value) -> Result<Value> {
    let params: ExportMarkdownParams = parse_params(params)?;
    let engine = open_engine(&params.store_path)?;
    let path = if let Some(path) = params.output_path {
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent)?;
            }
        }
        fs::write(&path, engine.export_markdown()?)?;
        path
    } else {
        engine.export_markdown_to_default_path()?
    };
    Ok(json!({
        "ok": true,
        "output_path": path,
        "format": "markdown",
        "json_runtime_storage": false
    }))
}

fn parse_params<T: for<'de> Deserialize<'de>>(params: Value) -> Result<T> {
    serde_json::from_value(params).map_err(|err| anyhow!("invalid params: {err}"))
}

fn open_engine(store_path: &PathBuf) -> Result<MemoryEngine> {
    MemoryEngine::open_at(store_path)
        .with_context(|| format!("failed to open store {}", store_path.display()))
}

fn parse_memory_source(
    source_type: Option<String>,
    source_ref: Option<String>,
) -> Result<Option<MemorySource>> {
    match (source_type, source_ref) {
        (Some(source_type), Some(reference)) => Ok(Some(MemorySource {
            source_type,
            reference,
        })),
        (None, None) => Ok(None),
        _ => bail!("source requires both source_type and source_ref"),
    }
}

fn error_response(id: Option<Value>, code: i64, message: impl Into<String>) -> JsonRpcResponse {
    JsonRpcResponse {
        jsonrpc: JSONRPC_VERSION,
        id,
        result: None,
        error: Some(JsonRpcError {
            code,
            message: message.into(),
        }),
    }
}

fn default_kind() -> String {
    "temporary_note".to_string()
}

fn default_scope() -> String {
    "global".to_string()
}

fn default_trust() -> String {
    "agent_inferred".to_string()
}

fn default_sensitivity() -> String {
    "private".to_string()
}

fn default_status() -> String {
    "active".to_string()
}

fn default_recall_mode() -> String {
    "focused".to_string()
}
