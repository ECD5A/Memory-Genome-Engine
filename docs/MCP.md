# MCP Adapter

[Russian version](MCP.ru.md)

`mge-mcp-server` is the current Memory Genome Engine MCP-ready adapter. It is a local line-oriented JSON-RPC process over stdin/stdout. This keeps the dependency surface small while the public integration contract stabilizes.

JSON here is protocol output only. It is not runtime storage.

Current contract:

- JSON-RPC version: `2.0`
- `protocol_version`: `mge-jsonrpc-1`
- `integration_schema_version`: `1`

## Run

```bash
cargo run -p mge-cli --bin mge-mcp-server
```

Each input line must be one JSON-RPC request:

```json
{"jsonrpc":"2.0","id":1,"method":"mge_stats","params":{"store_path":".memory-genome"}}
```

Each output line is one JSON-RPC response:

```json
{"jsonrpc":"2.0","id":1,"result":{"ok":true,"tool":"mge_stats","protocol_version":"mge-jsonrpc-1","integration_schema_version":1,"stats":{}}}
```

## Schemas

The adapter exposes the current tool schema with `mge_schema`:

```json
{"jsonrpc":"2.0","id":"schema","method":"mge_schema","params":{}}
```

The response contains:

- `tools`: input/output schema summaries for every public tool.
- `context_packet_contract`: stable recall wrapper shape.
- `error_contract`: structured error shape.

Golden contract tests live under `crates/mge-cli/tests/fixtures/mcp`.

## Error Model

Errors are structured and stable for SDKs:

```json
{"jsonrpc":"2.0","id":1,"error":{"code":-32602,"message":"invalid params: missing field `content`","tool_name":"mge_remember","recoverable":true,"protocol_version":"mge-jsonrpc-1","integration_schema_version":1,"details":{"error_kind":"invalid_params"}}}
```

Fields:

- `code`: JSON-RPC-compatible numeric code.
- `message`: human-readable error.
- `tool_name`: requested tool or `unknown`.
- `recoverable`: whether the caller can usually fix and retry.
- `details.error_kind`: stable machine-readable category.

## Tools

### `mge_remember`

Input:

- `store_path`
- `content`
- `kind`, default `temporary_note`
- `scope`, default `global`
- `markers`, default `[]`
- `trust`, default `agent_inferred`
- `sensitivity`, default `private`
- `status`, default `active`
- optional `subject`
- optional `source_type` and `source_ref`
- optional `links`
- optional `passphrase_env` for encrypted stores

### `mge_recall`

Input:

- `store_path`
- `query`
- `mode`: `focused`, `broad`, or `full_scope`
- `scope`, required for `full_scope`
- optional `markers`
- optional `max_items`
- optional `kind`
- optional `include_deprecated`
- optional `include_secret_references`
- optional `passphrase_env` for encrypted stores

Output contains the `ContextPacket` under `result.context_packet`.

For SDK stability, recall also includes `result.context`, an adapter-level wrapper:

- `query`
- `mode`
- `relevant_memory`
- `constraints`
- `warnings`
- `score_details`
- `debug`
- `store_stats`

### Store Operations

- `mge_seal`: `{ "store_path": "..." }`
- `mge_checkpoint`: `{ "store_path": "..." }`
- `mge_stats`: `{ "store_path": "..." }`
- `mge_validate`: `{ "store_path": "...", "deep": true }`
- `mge_rebuild_indexes`: `{ "store_path": "..." }`
- `mge_export_markdown`: `{ "store_path": "...", "output_path": "optional/path.md" }`

All store operations also accept optional `passphrase_env` for encrypted stores. The value is an environment variable name, not a passphrase. Encrypted sealed recall, deep validation, and rebuild-indexes use the same unlock path; the adapter never accepts raw passphrase values.

## Example

```json
{"jsonrpc":"2.0","id":1,"method":"mge_remember","params":{"store_path":".memory-genome","content":"Agent should recall memory before editing.","kind":"procedure","scope":"agent","markers":["topic:agent_memory"],"trust":"user_confirmed","sensitivity":"private"}}
{"jsonrpc":"2.0","id":2,"method":"mge_recall","params":{"store_path":".memory-genome","query":"agent memory","mode":"focused","scope":"agent","max_items":5}}
```

## Agent Workflow Smoke

A local agent host can exercise the complete integration path with one long-lived adapter process:

```json
{"jsonrpc":"2.0","id":1,"method":"mge_recall","params":{"store_path":".memory-genome","query":"current task context","mode":"focused","scope":"agent"}}
{"jsonrpc":"2.0","id":2,"method":"mge_remember","params":{"store_path":".memory-genome","content":"Agent completed the integration smoke workflow.","kind":"tool_result","scope":"agent","markers":["topic:integration_smoke"],"trust":"tool_observed"}}
{"jsonrpc":"2.0","id":3,"method":"mge_checkpoint","params":{"store_path":".memory-genome"}}
{"jsonrpc":"2.0","id":4,"method":"mge_seal","params":{"store_path":".memory-genome"}}
{"jsonrpc":"2.0","id":5,"method":"mge_recall","params":{"store_path":".memory-genome","query":"integration smoke workflow","mode":"broad","scope":"agent"}}
{"jsonrpc":"2.0","id":6,"method":"mge_validate","params":{"store_path":".memory-genome","deep":true}}
{"jsonrpc":"2.0","id":7,"method":"mge_rebuild_indexes","params":{"store_path":".memory-genome"}}
{"jsonrpc":"2.0","id":8,"method":"mge_export_markdown","params":{"store_path":".memory-genome"}}
```

A reusable JSONL transcript lives at:

```text
examples/mcp_agent_session.jsonl
```

Replace `$STORE_PATH` with a created store path and `$EXPORT_PATH` with a Markdown output path before sending the lines to `mge-mcp-server`. The transcript includes schema discovery, remember, focused recall, checkpoint, seal, broad recall, deep validate, rebuild indexes, Markdown export, and a structured invalid-mode error.

## Safety

- The adapter opens only explicit `store_path` values.
- Markdown export writes to the default store export path or an explicit `output_path`.
- It does not execute files, download data, install dependencies, read unrelated directories, or change the storage layout.
- `full_scope` recall requires `scope`.

## Troubleshooting

- Malformed JSON returns JSON-RPC `-32700` with `details.error_kind = "parse_error"`.
- Unknown tools return `-32601` with `details.error_kind = "unknown_method"`.
- Missing or invalid arguments return `-32602` with `details.error_kind = "invalid_params"`.
- A missing or invalid store path returns `-32000` with `details.error_kind = "store_open_failed"`.
- An encrypted-mode store opened without session unlock returns `-32000` with `details.error_kind = "store_locked"`.
- A wrong passphrase or authenticated decryption failure returns `-32000` with `details.error_kind = "auth_failed"`.
- Encrypted sealed page payloads require unlock for `mge_recall`, `mge_validate` with `deep: true`, and `mge_rebuild_indexes`.
- `full_scope` without `scope` returns `-32000` with `details.error_kind = "invalid_request"`.
- Invalid recall modes such as `sideways` are parameter errors, not core runtime failures.
- `output_path` on `mge_export_markdown` is explicit; otherwise export goes to the store default `exports/memory.md`.

## Compatibility

`integration_schema_version` changes only when the adapter contract changes. It does not change the Memory Genome storage version. Adding optional fields is allowed within the same major schema when existing fields keep their meaning.
