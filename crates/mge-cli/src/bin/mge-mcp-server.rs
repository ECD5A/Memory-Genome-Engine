// Memory Genome Engine
// Copyright (c) 2026 ECD5A
// Project: https://github.com/ECD5A/Memory-Genome-Engine
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at:
//
//     https://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.

use std::env;
use std::fs;
use std::io::{self, BufRead, Write};
use std::path::PathBuf;
use std::str::FromStr;

use anyhow::{anyhow, bail, Context, Result};
use mge_core::{
    CellId, ContextPacket, MemoryEngine, MemoryKind, MemorySource, MemoryStatus, MemoryValue,
    RecallMode, RecallRequest, RememberRequest, SensitivityLevel, SessionChunkOptions,
    SessionRememberRequest, SessionTurn, StoreStats, TrustLevel,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};

const JSONRPC_VERSION: &str = "2.0";
const PROTOCOL_VERSION: &str = "mge-jsonrpc-1";
const INTEGRATION_SCHEMA_VERSION: u32 = 2;
const MCP_PROTOCOL_VERSION: &str = "2025-06-18";

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
struct RememberSessionParams {
    store_path: PathBuf,
    turns: Vec<SessionTurn>,
    #[serde(default)]
    session_id: Option<String>,
    #[serde(default = "default_session_kind")]
    kind: String,
    #[serde(default = "default_scope")]
    scope: String,
    #[serde(default)]
    subject: Option<String>,
    #[serde(default)]
    markers: Vec<String>,
    #[serde(default = "default_trust")]
    trust: String,
    #[serde(default = "default_sensitivity")]
    sensitivity: String,
    #[serde(default = "default_status")]
    status: String,
    #[serde(default)]
    source_type: Option<String>,
    #[serde(default)]
    source_ref: Option<String>,
    #[serde(default)]
    links: Vec<CellId>,
    #[serde(default)]
    max_turns: Option<usize>,
    #[serde(default)]
    max_bytes: Option<usize>,
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

#[derive(Debug, Deserialize)]
struct McpCallToolParams {
    name: String,
    #[serde(default)]
    arguments: Value,
}

fn main() -> Result<()> {
    let stdin = io::stdin();
    let mut stdout = io::stdout();

    for line in stdin.lock().lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }

        if let Some(response) = handle_line(&line) {
            serde_json::to_writer(&mut stdout, &response)?;
            stdout.write_all(b"\n")?;
            stdout.flush()?;
        }
    }

    Ok(())
}

fn handle_line(line: &str) -> Option<JsonRpcResponse> {
    let line = line.trim_start_matches('\u{feff}');
    match serde_json::from_str::<JsonRpcRequest>(line) {
        Ok(request) => {
            let id = request.id.clone();
            if request.jsonrpc.as_deref().unwrap_or(JSONRPC_VERSION) != JSONRPC_VERSION {
                return request.id.map(|id| {
                    error_response(
                        ToolError {
                            code: -32600,
                            message: "jsonrpc must be \"2.0\"".to_string(),
                            tool_name: request.method,
                            recoverable: true,
                            details: Some(json!({ "error_kind": "invalid_request" })),
                        },
                        Some(id),
                    )
                });
            }

            if id.is_none() {
                let _ = handle_request(request);
                return None;
            }

            match handle_request(request) {
                Ok(result) => Some(JsonRpcResponse {
                    jsonrpc: JSONRPC_VERSION,
                    id,
                    result: Some(result),
                    error: None,
                }),
                Err(err) => Some(error_response(err, id)),
            }
        }
        Err(err) => Some(error_response(
            ToolError {
                code: -32700,
                message: format!("failed to parse JSON-RPC request: {err}"),
                tool_name: "unknown".to_string(),
                recoverable: true,
                details: Some(json!({ "error_kind": "parse_error" })),
            },
            Some(Value::Null),
        )),
    }
}

fn handle_request(request: JsonRpcRequest) -> std::result::Result<Value, ToolError> {
    let tool = request.method.as_str();
    match tool {
        "initialize" => Ok(mcp_initialize(&request.params)),
        "ping" => Ok(json!({})),
        "tools/list" => Ok(json!({ "tools": mcp_tools() })),
        "tools/call" => mcp_call_tool(request.params),
        "notifications/initialized" | "notifications/cancelled" => Ok(json!({})),
        "mge_schema" => Ok(mge_schema()),
        "mge_remember" => with_tool(tool, mge_remember(request.params)),
        "mge_remember_session" => with_tool(tool, mge_remember_session(request.params)),
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

fn mcp_initialize(params: &Value) -> Value {
    let requested = params
        .get("protocolVersion")
        .and_then(Value::as_str)
        .unwrap_or(MCP_PROTOCOL_VERSION);
    let protocol_version = match requested {
        "2024-11-05" | MCP_PROTOCOL_VERSION => requested,
        _ => MCP_PROTOCOL_VERSION,
    };
    json!({
        "protocolVersion": protocol_version,
        "capabilities": {
            "tools": { "listChanged": false }
        },
        "serverInfo": {
            "name": "memory-genome-engine",
            "title": "Memory Genome Engine",
            "version": env!("CARGO_PKG_VERSION")
        },
        "instructions": "Initialize a store with the mge CLI, then use mge_recall before work and mge_remember or mge_remember_session after useful work."
    })
}

fn mcp_call_tool(params: Value) -> std::result::Result<Value, ToolError> {
    let call: McpCallToolParams = serde_json::from_value(params).map_err(|err| ToolError {
        code: -32602,
        message: format!("invalid tools/call params: {err}"),
        tool_name: "tools/call".to_string(),
        recoverable: true,
        details: Some(json!({ "error_kind": "invalid_params" })),
    })?;
    match call_named_tool(&call.name, call.arguments) {
        Ok(result) => Ok(mcp_tool_result(result, false)),
        Err(err) => {
            let error = json!({
                "code": err.code,
                "message": err.message,
                "tool_name": err.tool_name,
                "recoverable": err.recoverable,
                "protocol_version": PROTOCOL_VERSION,
                "integration_schema_version": INTEGRATION_SCHEMA_VERSION,
                "details": err.details
            });
            Ok(mcp_tool_result(error, true))
        }
    }
}

fn call_named_tool(name: &str, params: Value) -> std::result::Result<Value, ToolError> {
    match name {
        "mge_schema" => Ok(mge_schema()),
        "mge_remember" => with_tool(name, mge_remember(params)),
        "mge_remember_session" => with_tool(name, mge_remember_session(params)),
        "mge_recall" => with_tool(name, mge_recall(params)),
        "mge_seal" => with_tool(name, mge_seal(params)),
        "mge_checkpoint" => with_tool(name, mge_checkpoint(params)),
        "mge_stats" => with_tool(name, mge_stats(params)),
        "mge_validate" => with_tool(name, mge_validate(params)),
        "mge_rebuild_indexes" => with_tool(name, mge_rebuild_indexes(params)),
        "mge_export_markdown" => with_tool(name, mge_export_markdown(params)),
        other => Err(ToolError {
            code: -32602,
            message: format!("unknown tool: {other}"),
            tool_name: other.to_string(),
            recoverable: false,
            details: Some(json!({ "error_kind": "unknown_tool" })),
        }),
    }
}

fn mcp_tool_result(structured: Value, is_error: bool) -> Value {
    let text = serde_json::to_string_pretty(&structured)
        .unwrap_or_else(|_| "failed to serialize tool result".to_string());
    json!({
        "content": [{ "type": "text", "text": text }],
        "structuredContent": structured,
        "isError": is_error
    })
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
        if message.contains("store is busy") {
            return "store_busy";
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

fn mge_remember_session(params: Value) -> Result<Value> {
    let params: RememberSessionParams = parse_params(params)?;
    let mut engine = open_engine(&params.store_path, params.passphrase_env.as_deref())?;
    let mut request = SessionRememberRequest::new(params.turns);
    request.session_id = params.session_id;
    request.kind = MemoryKind::from_str(&params.kind)?;
    request.scope = params.scope;
    request.subject = params.subject;
    request.markers = params.markers;
    request.trust = TrustLevel::from_str(&params.trust)?;
    request.sensitivity = SensitivityLevel::from_str(&params.sensitivity)?;
    request.status = MemoryStatus::from_str(&params.status)?;
    request.source = parse_memory_source(params.source_type, params.source_ref)?;
    request.links = params.links;
    let defaults = SessionChunkOptions::default();
    request.chunk_options = SessionChunkOptions {
        max_turns: params.max_turns.unwrap_or(defaults.max_turns),
        max_bytes: params.max_bytes.unwrap_or(defaults.max_bytes),
    };
    let report = engine.remember_session(request)?;
    let cell_ids = report.cells.iter().map(|cell| cell.id).collect::<Vec<_>>();
    Ok(tool_result(
        "mge_remember_session",
        true,
        json!({
            "turns": report.turns,
            "chunks": report.chunks,
            "cell_ids": cell_ids,
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
    } else if message.contains("store is busy") {
        "store_busy"
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
        "mge_remember_session": {
            "input": {
                "required": ["store_path", "turns"],
                "properties": {
                    "store_path": "string path to existing Memory Genome store",
                    "turns": "array of {role, content} session turns",
                    "session_id": "optional stable session identifier",
                    "kind": "memory kind string, default project_fact",
                    "scope": "scope string, default global",
                    "subject": "optional chunk subject prefix",
                    "markers": "array of marker strings applied to every chunk",
                    "trust": "trust level string, default tool_observed",
                    "sensitivity": "sensitivity level string, default private",
                    "status": "status string, default active",
                    "max_turns": "optional positive chunk turn limit, default 8",
                    "max_bytes": "optional positive chunk byte target, default 4096",
                    "passphrase_env": "optional environment variable name used to unlock encrypted stores"
                }
            },
            "output": ["ok", "tool", "protocol_version", "integration_schema_version", "turns", "chunks", "cell_ids", "json_runtime_storage"]
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

fn mcp_tools() -> Vec<Value> {
    vec![
        mcp_tool(
            "mge_schema",
            "Return the versioned Memory Genome Engine integration contract.",
            json!({ "type": "object", "properties": {} }),
        ),
        mcp_tool(
            "mge_remember",
            "Store one typed memory cell in L1 hot memory.",
            json!({
                "type": "object",
                "required": ["store_path", "content"],
                "properties": {
                    "store_path": { "type": "string" },
                    "content": { "type": "string", "minLength": 1 },
                    "kind": { "type": "string", "default": "temporary_note" },
                    "scope": { "type": "string", "default": "global" },
                    "markers": { "type": "array", "items": { "type": "string" } },
                    "trust": { "type": "string", "default": "agent_inferred" },
                    "sensitivity": { "type": "string", "default": "private" },
                    "status": { "type": "string", "default": "active" },
                    "subject": { "type": "string" },
                    "source_type": { "type": "string" },
                    "source_ref": { "type": "string" },
                    "links": { "type": "array", "items": { "type": "integer", "minimum": 1 } },
                    "passphrase_env": { "type": "string" }
                }
            }),
        ),
        mcp_tool(
            "mge_remember_session",
            "Deterministically chunk agent turns and store each chunk as a memory cell.",
            json!({
                "type": "object",
                "required": ["store_path", "turns"],
                "properties": {
                    "store_path": { "type": "string" },
                    "turns": {
                        "type": "array",
                        "minItems": 1,
                        "items": {
                            "type": "object",
                            "required": ["role", "content"],
                            "properties": {
                                "role": { "type": "string", "minLength": 1 },
                                "content": { "type": "string", "minLength": 1 }
                            }
                        }
                    },
                    "session_id": { "type": "string" },
                    "kind": { "type": "string", "default": "project_fact" },
                    "scope": { "type": "string", "default": "global" },
                    "subject": { "type": "string" },
                    "markers": { "type": "array", "items": { "type": "string" } },
                    "trust": { "type": "string", "default": "agent_inferred" },
                    "sensitivity": { "type": "string", "default": "private" },
                    "status": { "type": "string", "default": "active" },
                    "max_turns": { "type": "integer", "minimum": 1, "default": 8 },
                    "max_bytes": { "type": "integer", "minimum": 1, "default": 4096 },
                    "passphrase_env": { "type": "string" }
                }
            }),
        ),
        mcp_tool(
            "mge_recall",
            "Recall task-relevant memory as a ContextPacket.",
            json!({
                "type": "object",
                "required": ["store_path"],
                "properties": {
                    "store_path": { "type": "string" },
                    "query": { "type": "string" },
                    "mode": { "type": "string", "enum": ["focused", "broad", "full_scope"], "default": "focused" },
                    "scope": { "type": "string" },
                    "markers": { "type": "array", "items": { "type": "string" } },
                    "max_items": { "type": "integer", "minimum": 1 },
                    "kind": { "type": "string" },
                    "include_deprecated": { "type": "boolean", "default": false },
                    "include_secret_references": { "type": "boolean", "default": false },
                    "passphrase_env": { "type": "string" }
                }
            }),
        ),
        mcp_store_tool("mge_seal", "Seal current hot memory into immutable pages."),
        mcp_store_tool(
            "mge_checkpoint",
            "Flush pending hot records and write a recovery snapshot.",
        ),
        mcp_store_tool("mge_stats", "Return current store statistics."),
        mcp_tool(
            "mge_validate",
            "Validate store structure and optionally decode all sealed pages.",
            json!({
                "type": "object",
                "required": ["store_path"],
                "properties": {
                    "store_path": { "type": "string" },
                    "deep": { "type": "boolean", "default": false },
                    "passphrase_env": { "type": "string" }
                }
            }),
        ),
        mcp_store_tool(
            "mge_rebuild_indexes",
            "Rebuild catalog and candidate indexes from sealed pages.",
        ),
        mcp_tool(
            "mge_export_markdown",
            "Explicitly export memory to a plaintext Markdown file.",
            json!({
                "type": "object",
                "required": ["store_path"],
                "properties": {
                    "store_path": { "type": "string" },
                    "output_path": { "type": "string" },
                    "passphrase_env": { "type": "string" }
                }
            }),
        ),
    ]
}

fn mcp_store_tool(name: &str, description: &str) -> Value {
    mcp_tool(
        name,
        description,
        json!({
            "type": "object",
            "required": ["store_path"],
            "properties": {
                "store_path": { "type": "string" },
                "passphrase_env": { "type": "string" }
            }
        }),
    )
}

fn mcp_tool(name: &str, description: &str, input_schema: Value) -> Value {
    json!({
        "name": name,
        "description": description,
        "inputSchema": input_schema
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

fn default_session_kind() -> String {
    "project_fact".to_string()
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
