use std::env;
use std::fs;
use std::io::{self, BufRead, Write};
use std::path::PathBuf;
use std::str::FromStr;

use anyhow::{anyhow, bail, Context, Result};
use mge_core::{
    CellId, ContextPacket, MemoryEngine, MemoryKind, MemorySource, MemoryStatus, MemoryValue,
    RecallMode, RecallRequest, RememberRequest, SensitivityLevel, StoreStats, TrustLevel,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};

const JSONRPC_VERSION: &str = "2.0";
const PROTOCOL_VERSION: &str = "mge-jsonrpc-1";
const INTEGRATION_SCHEMA_VERSION: u32 = 1;

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
    tool_name: String,
    recoverable: bool,
    protocol_version: &'static str,
    integration_schema_version: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    details: Option<Value>,
}

#[derive(Debug)]
struct ToolError {
    code: i64,
    message: String,
    tool_name: String,
    recoverable: bool,
    details: Option<Value>,
}

#[derive(Debug, Deserialize)]
struct StoreParams {
    store_path: PathBuf,
    #[serde(default)]
    passphrase_env: Option<String>,
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
    #[serde(default)]
    passphrase_env: Option<String>,
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
    #[serde(default)]
    passphrase_env: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ValidateParams {
    store_path: PathBuf,
    #[serde(default)]
    deep: bool,
    #[serde(default)]
    passphrase_env: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ExportMarkdownParams {
    store_path: PathBuf,
    #[serde(default)]
    output_path: Option<PathBuf>,
    #[serde(default)]
    passphrase_env: Option<String>,
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
    let line = line.trim_start_matches('\u{feff}');
    match serde_json::from_str::<JsonRpcRequest>(line) {
        Ok(request) => {
            let id = request.id.clone();
            if request.jsonrpc.as_deref().unwrap_or(JSONRPC_VERSION) != JSONRPC_VERSION {
                return error_response(
                    ToolError {
                        code: -32600,
                        message: "jsonrpc must be \"2.0\"".to_string(),
                        tool_name: request.method,
                        recoverable: true,
                        details: Some(json!({ "error_kind": "invalid_request" })),
                    },
                    id,
                );
            }

            match handle_request(request) {
                Ok(result) => JsonRpcResponse {
                    jsonrpc: JSONRPC_VERSION,
                    id,
                    result: Some(result),
                    error: None,
                },
                Err(err) => error_response(err, id),
            }
        }
        Err(err) => error_response(
            ToolError {
                code: -32700,
                message: format!("failed to parse JSON-RPC request: {err}"),
                tool_name: "unknown".to_string(),
                recoverable: true,
                details: Some(json!({ "error_kind": "parse_error" })),
            },
            None,
        ),
    }
}

fn handle_request(request: JsonRpcRequest) -> std::result::Result<Value, ToolError> {
    let tool = request.method.as_str();
    match tool {
        "mge_schema" => Ok(mge_schema()),
        "mge_remember" => with_tool(tool, mge_remember(request.params)),
        "mge_recall" => with_tool(tool, mge_recall(request.params)),
        "mge_seal" => with_tool(tool, mge_seal(request.params)),
        "mge_checkpoint" => with_tool(tool, mge_checkpoint(request.params)),
        "mge_stats" => with_tool(tool, mge_stats(request.params)),
        "mge_validate" => with_tool(tool, mge_validate(request.params)),
        "mge_rebuild_indexes" => with_tool(tool, mge_rebuild_indexes(request.params)),
        "mge_export_markdown" => with_tool(tool, mge_export_markdown(request.params)),
        other => Err(ToolError {
            code: -32601,
            message: format!("unknown method: {other}"),
            tool_name: other.to_string(),
            recoverable: false,
            details: Some(json!({ "error_kind": "unknown_method" })),
        }),
    }
}

fn with_tool(tool: &str, result: Result<Value>) -> std::result::Result<Value, ToolError> {
    result.map_err(|err| {
        let error_kind = classify_error_kind_from_error(&err);
        let message = if error_kind == "store_locked" {
            message_with_cause(&err, "store is locked")
        } else if error_kind == "auth_failed" {
            message_with_cause(&err, "authentication failed")
        } else {
            err.to_string()
        };
        ToolError {
            code: if error_kind == "invalid_params" {
                -32602
            } else {
                -32000
            },
            message,
            tool_name: tool.to_string(),
            recoverable: true,
            details: Some(json!({ "error_kind": error_kind })),
        }
    })
}

fn classify_error_kind_from_error(err: &anyhow::Error) -> &'static str {
    for cause in err.chain() {
        let message = cause.to_string();
        if message.contains("store is locked") {
            return "store_locked";
        }
        if message.contains("authentication failed") {
            return "auth_failed";
        }
    }
    classify_error_kind(&err.to_string())
}

fn message_with_cause(err: &anyhow::Error, pattern: &str) -> String {
    let outer = err.to_string();
    for cause in err.chain().skip(1) {
        let cause_message = cause.to_string();
        if cause_message.contains(pattern) {
            return format!("{outer}: {cause_message}");
        }
    }
    outer
}

fn mge_schema() -> Value {
    tool_result(
        "mge_schema",
        true,
        json!({
            "tools": tool_schemas(),
            "context_packet_contract": context_packet_contract_schema(),
            "error_contract": error_contract_schema()
        }),
    )
}

fn mge_remember(params: Value) -> Result<Value> {
    let params: RememberParams = parse_params(params)?;
    let mut engine = open_engine(&params.store_path, params.passphrase_env.as_deref())?;
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
    Ok(tool_result(
        "mge_remember",
        true,
        json!({
            "cell_id": cell.id,
            "scope": cell.scope,
            "kind": cell.kind,
            "status": cell.status,
            "json_runtime_storage": false
        }),
    ))
}

fn mge_recall(params: Value) -> Result<Value> {
    let params: RecallParams = parse_params(params)?;
    let engine = open_engine(&params.store_path, params.passphrase_env.as_deref())?;
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
    let stats = engine.stats()?;
    let context = context_contract(&packet, &stats);
    Ok(tool_result(
        "mge_recall",
        true,
        json!({
            "context_packet": packet,
            "context": context,
            "json_runtime_storage": false
        }),
    ))
}

fn mge_seal(params: Value) -> Result<Value> {
    let params: StoreParams = parse_params(params)?;
    let mut engine = open_engine(&params.store_path, params.passphrase_env.as_deref())?;
    let report = engine.seal()?;
    Ok(tool_result("mge_seal", true, json!({ "seal": report })))
}

fn mge_checkpoint(params: Value) -> Result<Value> {
    let params: StoreParams = parse_params(params)?;
    let mut engine = open_engine(&params.store_path, params.passphrase_env.as_deref())?;
    let report = engine.checkpoint()?;
    Ok(tool_result(
        "mge_checkpoint",
        true,
        json!({ "checkpoint": report }),
    ))
}

fn mge_stats(params: Value) -> Result<Value> {
    let params: StoreParams = parse_params(params)?;
    let engine = open_engine(&params.store_path, params.passphrase_env.as_deref())?;
    let stats = engine.stats()?;
    Ok(tool_result("mge_stats", true, json!({ "stats": stats })))
}

fn mge_validate(params: Value) -> Result<Value> {
    let params: ValidateParams = parse_params(params)?;
    let engine = open_engine(&params.store_path, params.passphrase_env.as_deref())?;
    let report = if params.deep {
        engine.validate_deep()?
    } else {
        engine.validate()?
    };
    Ok(tool_result(
        "mge_validate",
        report.ok,
        json!({ "validation": report }),
    ))
}

fn mge_rebuild_indexes(params: Value) -> Result<Value> {
    let params: StoreParams = parse_params(params)?;
    let engine = open_engine(&params.store_path, params.passphrase_env.as_deref())?;
    let report = engine.rebuild_catalog_and_indexes()?;
    Ok(tool_result(
        "mge_rebuild_indexes",
        true,
        json!({ "rebuild": report }),
    ))
}

fn mge_export_markdown(params: Value) -> Result<Value> {
    let params: ExportMarkdownParams = parse_params(params)?;
    let engine = open_engine(&params.store_path, params.passphrase_env.as_deref())?;
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
    Ok(tool_result(
        "mge_export_markdown",
        true,
        json!({
            "output_path": path,
            "format": "markdown",
            "json_runtime_storage": false
        }),
    ))
}

fn tool_result(tool: &str, ok: bool, body: Value) -> Value {
    let mut object = match body {
        Value::Object(object) => object,
        other => {
            let mut object = Map::new();
            object.insert("value".to_string(), other);
            object
        }
    };
    object.insert("ok".to_string(), Value::Bool(ok));
    object.insert("tool".to_string(), Value::String(tool.to_string()));
    object.insert(
        "protocol_version".to_string(),
        Value::String(PROTOCOL_VERSION.to_string()),
    );
    object.insert(
        "integration_schema_version".to_string(),
        Value::Number(INTEGRATION_SCHEMA_VERSION.into()),
    );
    Value::Object(object)
}

fn context_contract(packet: &ContextPacket, stats: &StoreStats) -> Value {
    json!({
        "query": packet.query,
        "mode": packet.debug.recall_mode,
        "relevant_memory": packet.relevant_memory,
        "constraints": packet.constraints,
        "warnings": packet.warnings,
        "score_details": packet.debug.score_details,
        "debug": packet.debug,
        "store_stats": stats
    })
}

fn parse_params<T: for<'de> Deserialize<'de>>(params: Value) -> Result<T> {
    serde_json::from_value(params).map_err(|err| anyhow!("invalid params: {err}"))
}

fn open_engine(store_path: &PathBuf, passphrase_env: Option<&str>) -> Result<MemoryEngine> {
    let passphrase = passphrase_from_env(passphrase_env)?;
    MemoryEngine::open_at_with_passphrase(store_path, passphrase.as_deref())
        .with_context(|| format!("failed to open store {}", store_path.display()))
}

fn passphrase_from_env(passphrase_env: Option<&str>) -> Result<Option<String>> {
    let Some(name) = passphrase_env else {
        return Ok(None);
    };
    let value = env::var(name).with_context(|| format!("passphrase env var {name} is not set"))?;
    if value.is_empty() {
        bail!("passphrase env var {name} is empty");
    }
    Ok(Some(value))
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

fn error_response(err: ToolError, id: Option<Value>) -> JsonRpcResponse {
    JsonRpcResponse {
        jsonrpc: JSONRPC_VERSION,
        id,
        result: None,
        error: Some(JsonRpcError {
            code: err.code,
            message: err.message,
            tool_name: err.tool_name,
            recoverable: err.recoverable,
            protocol_version: PROTOCOL_VERSION,
            integration_schema_version: INTEGRATION_SCHEMA_VERSION,
            details: err.details,
        }),
    }
}

fn classify_error_kind(message: &str) -> &'static str {
    if message.contains("invalid params") || is_invalid_enum_value(message) {
        "invalid_params"
    } else if message.contains("store is locked") {
        "store_locked"
    } else if message.contains("authentication failed") {
        "auth_failed"
    } else if message.contains("failed to open store") {
        "store_open_failed"
    } else if message.contains("requires scope") || message.contains("requires query") {
        "invalid_request"
    } else {
        "tool_error"
    }
}

fn is_invalid_enum_value(message: &str) -> bool {
    message.contains("unknown MemoryKind")
        || message.contains("unknown TrustLevel")
        || message.contains("unknown SensitivityLevel")
        || message.contains("unknown MemoryStatus")
        || message.contains("unknown RecallMode")
        || message.contains("unknown index kind")
        || message.contains("unknown durability policy")
}

fn tool_schemas() -> Value {
    json!({
        "mge_remember": {
            "input": {
                "required": ["store_path", "content"],
                "properties": {
                    "store_path": "string path to existing Memory Genome store",
                    "content": "string memory content",
                    "kind": "memory kind string, default temporary_note",
                    "scope": "scope string, default global",
                    "markers": "array of marker strings",
                    "trust": "trust level string, default agent_inferred",
                    "sensitivity": "sensitivity level string, default private",
                    "status": "status string, default active",
                    "subject": "optional subject string",
                    "source_type": "optional source type; requires source_ref",
                    "source_ref": "optional source reference; requires source_type",
                    "links": "array of linked CellId numbers",
                    "passphrase_env": "optional environment variable name used to unlock encrypted stores"
                }
            },
            "output": ["ok", "tool", "protocol_version", "integration_schema_version", "cell_id", "scope", "kind", "status", "json_runtime_storage"]
        },
        "mge_recall": {
            "input": {
                "required": ["store_path"],
                "properties": {
                    "store_path": "string path to existing Memory Genome store",
                    "query": "required for focused/broad",
                    "mode": "focused | broad | full_scope",
                    "scope": "required for full_scope",
                    "markers": "array of marker strings",
                    "max_items": "optional positive integer",
                    "kind": "optional memory kind string",
                    "include_deprecated": "boolean",
                    "include_secret_references": "boolean",
                    "passphrase_env": "optional environment variable name used to unlock encrypted stores"
                }
            },
            "output": ["ok", "tool", "protocol_version", "integration_schema_version", "context_packet", "context", "json_runtime_storage"]
        },
        "mge_seal": store_tool_schema("seal"),
        "mge_checkpoint": store_tool_schema("checkpoint"),
        "mge_stats": store_tool_schema("stats"),
        "mge_validate": {
            "input": {
                "required": ["store_path"],
                "properties": {
                    "store_path": "string path to existing Memory Genome store",
                    "deep": "boolean, default false",
                    "passphrase_env": "optional environment variable name used to unlock encrypted stores"
                }
            },
            "output": ["ok", "tool", "protocol_version", "integration_schema_version", "validation"]
        },
        "mge_rebuild_indexes": store_tool_schema("rebuild"),
        "mge_export_markdown": {
            "input": {
                "required": ["store_path"],
                "properties": {
                    "store_path": "string path to existing Memory Genome store",
                    "output_path": "optional markdown output path",
                    "passphrase_env": "optional environment variable name used to unlock encrypted stores"
                }
            },
            "output": ["ok", "tool", "protocol_version", "integration_schema_version", "output_path", "format", "json_runtime_storage"]
        }
    })
}

fn store_tool_schema(output_field: &str) -> Value {
    json!({
        "input": {
            "required": ["store_path"],
            "properties": {
                "store_path": "string path to existing Memory Genome store",
                "passphrase_env": "optional environment variable name used to unlock encrypted stores"
            }
        },
        "output": ["ok", "tool", "protocol_version", "integration_schema_version", output_field]
    })
}

fn context_packet_contract_schema() -> Value {
    json!({
        "context_packet": "core ContextPacket, unchanged",
        "context": {
            "query": "string",
            "mode": "focused | broad | full_scope",
            "relevant_memory": "array of ContextMemoryItem",
            "constraints": "array of strings",
            "warnings": "array of strings",
            "score_details": "array of score debug entries",
            "debug": "ContextDebugInfo",
            "store_stats": "StoreStats snapshot after recall"
        }
    })
}

fn error_contract_schema() -> Value {
    json!({
        "jsonrpc": "2.0",
        "error": {
            "code": "number",
            "message": "string",
            "tool_name": "string",
            "recoverable": "boolean",
            "protocol_version": PROTOCOL_VERSION,
            "integration_schema_version": INTEGRATION_SCHEMA_VERSION,
            "details": "optional object"
        }
    })
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
