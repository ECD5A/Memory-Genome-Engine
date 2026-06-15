# MCP Adapter

[English version](MCP.md)

`mge-mcp-server` - текущий MCP-ready adapter для Memory Genome Engine. Это локальный line-oriented JSON-RPC процесс через stdin/stdout. Такой вариант держит dependency surface маленьким, пока public integration contract стабилизируется.

JSON здесь является только protocol output. Это не runtime storage.

Текущий contract:

- JSON-RPC version: `2.0`
- `protocol_version`: `mge-jsonrpc-1`
- `integration_schema_version`: `1`

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
{"jsonrpc":"2.0","id":1,"result":{"ok":true,"tool":"mge_stats","protocol_version":"mge-jsonrpc-1","integration_schema_version":1,"stats":{}}}
```

## Schemas

Adapter отдаёт текущую tool schema через `mge_schema`:

```json
{"jsonrpc":"2.0","id":"schema","method":"mge_schema","params":{}}
```

Response содержит:

- `tools`: input/output schema summaries для каждого public tool.
- `context_packet_contract`: stable recall wrapper shape.
- `error_contract`: structured error shape.

Golden contract tests лежат в `crates/mge-cli/tests/fixtures/mcp`.

## Error Model

Ошибки structured и стабильны для SDK:

```json
{"jsonrpc":"2.0","id":1,"error":{"code":-32602,"message":"invalid params: missing field `content`","tool_name":"mge_remember","recoverable":true,"protocol_version":"mge-jsonrpc-1","integration_schema_version":1,"details":{"error_kind":"invalid_params"}}}
```

Fields:

- `code`: JSON-RPC-compatible numeric code.
- `message`: human-readable error.
- `tool_name`: requested tool или `unknown`.
- `recoverable`: можно ли обычно исправить запрос и повторить.
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

Для SDK stability recall также содержит `result.context`, adapter-level wrapper:

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

## Пример

```json
{"jsonrpc":"2.0","id":1,"method":"mge_remember","params":{"store_path":".memory-genome","content":"Agent should recall memory before editing.","kind":"procedure","scope":"agent","markers":["topic:agent_memory"],"trust":"user_confirmed","sensitivity":"private"}}
{"jsonrpc":"2.0","id":2,"method":"mge_recall","params":{"store_path":".memory-genome","query":"agent memory","mode":"focused","scope":"agent","max_items":5}}
```

## Agent Workflow Smoke

Локальный agent host может проверить полный integration path через один long-lived adapter process:

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

Reusable JSONL transcript лежит здесь:

```text
examples/mcp_agent_session.jsonl
```

Перед отправкой lines в `mge-mcp-server` замените `$STORE_PATH` на созданный store path, а `$EXPORT_PATH` на Markdown output path. Transcript включает schema discovery, remember, focused recall, checkpoint, seal, broad recall, deep validate, rebuild indexes, Markdown export и structured invalid-mode error.

## Safety

- Adapter открывает только explicit `store_path`.
- Markdown export пишет в default store export path или explicit `output_path`.
- Он не исполняет files, не скачивает data, не устанавливает dependencies, не читает unrelated directories и не меняет storage layout.
- `full_scope` recall требует `scope`.

## Troubleshooting

- Malformed JSON возвращает JSON-RPC `-32700` с `details.error_kind = "parse_error"`.
- Unknown tools возвращают `-32601` с `details.error_kind = "unknown_method"`.
- Missing или invalid arguments возвращают `-32602` с `details.error_kind = "invalid_params"`.
- Missing или invalid store path возвращает `-32000` с `details.error_kind = "store_open_failed"`.
- Encrypted-mode store без session unlock возвращает `-32000` с `details.error_kind = "store_locked"`. Payload encryption/unlock пока не реализованы, поэтому это безопасный locked-store foundation, а не usable encrypted recall.
- `full_scope` без `scope` возвращает `-32000` с `details.error_kind = "invalid_request"`.
- Invalid recall modes вроде `sideways` являются parameter errors, а не core runtime failures.
- `output_path` у `mge_export_markdown` явный; без него export идёт в default `exports/memory.md` внутри store.

## Compatibility

`integration_schema_version` меняется только при изменении adapter contract. Он не меняет Memory Genome storage version. Добавлять optional fields можно в той же major schema, если старые fields сохраняют смысл.
