# MCP Adapter

[Russian version](MCP.ru.md)

`mge-mcp-server` is the current Memory Genome Engine MCP-ready adapter. It is a local line-oriented JSON-RPC process over stdin/stdout. This keeps the dependency surface small while the public integration contract stabilizes.

JSON here is protocol output only. It is not runtime storage.

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
{"jsonrpc":"2.0","id":1,"result":{"ok":true,"stats":{}}}
```

Errors are structured:

```json
{"jsonrpc":"2.0","id":1,"error":{"code":-32000,"message":"failed to open store .memory-genome"}}
```

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

Output contains the `ContextPacket` under `result.context_packet`.

### Store Operations

- `mge_seal`: `{ "store_path": "..." }`
- `mge_checkpoint`: `{ "store_path": "..." }`
- `mge_stats`: `{ "store_path": "..." }`
- `mge_validate`: `{ "store_path": "...", "deep": true }`
- `mge_rebuild_indexes`: `{ "store_path": "..." }`
- `mge_export_markdown`: `{ "store_path": "...", "output_path": "optional/path.md" }`

## Example

```json
{"jsonrpc":"2.0","id":1,"method":"mge_remember","params":{"store_path":".memory-genome","content":"Agent should recall memory before editing.","kind":"procedure","scope":"agent","markers":["topic:agent_memory"],"trust":"user_confirmed","sensitivity":"private"}}
{"jsonrpc":"2.0","id":2,"method":"mge_recall","params":{"store_path":".memory-genome","query":"agent memory","mode":"focused","scope":"agent","max_items":5}}
```

## Safety

- The adapter opens only explicit `store_path` values.
- Markdown export writes to the default store export path or an explicit `output_path`.
- It does not execute files, download data, install dependencies, read unrelated directories, or change the storage layout.
- `full_scope` recall requires `scope`.
