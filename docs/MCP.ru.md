# MCP Adapter

[English version](MCP.md)

`mge-mcp-server` - текущий MCP-ready adapter для Memory Genome Engine. Это локальный line-oriented JSON-RPC процесс через stdin/stdout. Такой вариант держит dependency surface маленьким, пока public integration contract стабилизируется.

JSON здесь является только protocol output. Это не runtime storage.

## Запуск

```bash
cargo run -p mge-cli --bin mge-mcp-server
```

Каждая входная строка - один JSON-RPC request:

```json
{"jsonrpc":"2.0","id":1,"method":"mge_stats","params":{"store_path":".memory-genome"}}
```

Каждая выходная строка - один JSON-RPC response:

```json
{"jsonrpc":"2.0","id":1,"result":{"ok":true,"stats":{}}}
```

Ошибки structured:

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
- optional `source_type` и `source_ref`
- optional `links`

### `mge_recall`

Input:

- `store_path`
- `query`
- `mode`: `focused`, `broad` или `full_scope`
- `scope`, required для `full_scope`
- optional `markers`
- optional `max_items`
- optional `kind`
- optional `include_deprecated`
- optional `include_secret_references`

Output содержит `ContextPacket` в `result.context_packet`.

### Store Operations

- `mge_seal`: `{ "store_path": "..." }`
- `mge_checkpoint`: `{ "store_path": "..." }`
- `mge_stats`: `{ "store_path": "..." }`
- `mge_validate`: `{ "store_path": "...", "deep": true }`
- `mge_rebuild_indexes`: `{ "store_path": "..." }`
- `mge_export_markdown`: `{ "store_path": "...", "output_path": "optional/path.md" }`

## Пример

```json
{"jsonrpc":"2.0","id":1,"method":"mge_remember","params":{"store_path":".memory-genome","content":"Agent should recall memory before editing.","kind":"procedure","scope":"agent","markers":["topic:agent_memory"],"trust":"user_confirmed","sensitivity":"private"}}
{"jsonrpc":"2.0","id":2,"method":"mge_recall","params":{"store_path":".memory-genome","query":"agent memory","mode":"focused","scope":"agent","max_items":5}}
```

## Safety

- Adapter открывает только explicit `store_path`.
- Markdown export пишет в default store export path или explicit `output_path`.
- Он не исполняет files, не скачивает data, не устанавливает dependencies, не читает unrelated directories и не меняет storage layout.
- `full_scope` recall требует `scope`.
