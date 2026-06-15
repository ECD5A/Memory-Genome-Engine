# Интеграция

[English version](INTEGRATION.md)

Этот документ - единая справка по agent host lifecycle, локальному JSON-RPC MCP-ready adapter и thin Python/TypeScript SDK wrappers.

JSON в этом слое является только protocol/debug output. Это не runtime storage.

Core flow не меняется:

```text
remember -> L1 Hot RAM + pending persistence -> hot/hot.mgl
seal -> Sealed Pages + Indexes
recall -> ContextPacket
```

## Agent Lifecycle

Host управляет orchestration. Memory Genome Engine управляет памятью.

```text
host starts task
-> recall focused or broad
-> host works using ContextPacket
-> remember useful result
-> checkpoint for hot durability
-> recall again if the task continues
-> seal at a stable task/session boundary
-> validate --deep in smoke or maintenance flows
```

Recall modes:

- `focused`: default для узкого вопроса, следующего действия или tool decision.
- `broad`: project/module/task planning, когда агенту нужно больше связанной памяти.
- `full_scope`: явный audit/export/review внутри известного scope; всегда передавать `scope`.

Local host examples:

- Rust/CLI process host: `examples/agent_host_cli.rs`
- Python SDK host: `examples/python_agent_host.py`
- TypeScript SDK host: `examples/typescript_agent_host.ts`
- MCP JSON-RPC transcript: `examples/mcp_agent_session.jsonl`

## ContextPacket Contract

`ContextPacket` - результат recall. Он содержит:

- `query`
- `relevant_memory`
- `constraints`
- `warnings`
- `debug`

Packet является task-relevant и size-controlled, но не обязан быть маленьким. MCP/SDK recall responses держат core `ContextPacket` в `context_packet` и adapter wrapper в `context`:

- `query`
- `mode`
- `relevant_memory`
- `constraints`
- `warnings`
- `score_details`
- `debug`
- `store_stats`

## Integration Surfaces

Используй самый низкий слой, который подходит host-у:

- Rust: `mge-core::MemoryEngine`.
- CLI: `mge`.
- MCP-ready local adapter: `mge-mcp-server`, JSON-RPC через stdin/stdout.
- Python: thin wrapper в `sdk/python`.
- TypeScript: thin wrapper в `sdk/typescript`.

Rust остается core. Python и TypeScript wrappers вызывают Rust CLI и не дублируют memory logic.

## MCP-Ready JSON-RPC Adapter

Запуск:

```bash
cargo run -p mge-cli --bin mge-mcp-server
```

Contract:

- JSON-RPC version: `2.0`
- `protocol_version`: `mge-jsonrpc-1`
- `integration_schema_version`: `1`

Каждая input line - один JSON-RPC request:

```json
{"jsonrpc":"2.0","id":1,"method":"mge_stats","params":{"store_path":".memory-genome"}}
```

Каждая output line - один JSON-RPC response:

```json
{"jsonrpc":"2.0","id":1,"result":{"ok":true,"tool":"mge_stats","protocol_version":"mge-jsonrpc-1","integration_schema_version":1,"stats":{}}}
```

Schema discovery:

```json
{"jsonrpc":"2.0","id":"schema","method":"mge_schema","params":{}}
```

Schema response включает tool schemas, `ContextPacket` wrapper contract и structured error contract. Golden fixtures лежат в `crates/mge-cli/tests/fixtures/mcp`.

### MCP Tools

`mge_remember` input:

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
- optional `passphrase_env`

`mge_recall` input:

- `store_path`
- `query`
- `mode`: `focused`, `broad` или `full_scope`
- `scope`, required для `full_scope`
- optional `markers`
- optional `max_items`
- optional `kind`
- optional `include_deprecated`
- optional `include_secret_references`
- optional `passphrase_env`

Store tools:

- `mge_seal`
- `mge_checkpoint`
- `mge_stats`
- `mge_validate`
- `mge_rebuild_indexes`
- `mge_export_markdown`

Все store tools принимают `store_path`; encrypted stores также принимают `passphrase_env`. `mge_validate` принимает `deep`; `mge_export_markdown` принимает optional `output_path`.

### Structured Errors

Ошибки стабильны для SDK:

```json
{"jsonrpc":"2.0","id":1,"error":{"code":-32602,"message":"invalid params: missing field `content`","tool_name":"mge_remember","recoverable":true,"protocol_version":"mge-jsonrpc-1","integration_schema_version":1,"details":{"error_kind":"invalid_params"}}}
```

Важные `details.error_kind`:

- `parse_error`
- `unknown_method`
- `invalid_params`
- `store_open_failed`
- `store_locked`
- `auth_failed`
- `invalid_request`

Malformed JSON возвращает `-32700`; unknown tools возвращают `-32601`; missing/invalid params возвращают `-32602`; store/runtime failures используют `-32000`.

### MCP Workflow Example

```json
{"jsonrpc":"2.0","id":1,"method":"mge_remember","params":{"store_path":".memory-genome","content":"Agent should recall memory before editing.","kind":"procedure","scope":"agent","markers":["topic:agent_memory"],"trust":"user_confirmed","sensitivity":"private"}}
{"jsonrpc":"2.0","id":2,"method":"mge_recall","params":{"store_path":".memory-genome","query":"agent memory","mode":"focused","scope":"agent","max_items":5}}
{"jsonrpc":"2.0","id":3,"method":"mge_checkpoint","params":{"store_path":".memory-genome"}}
{"jsonrpc":"2.0","id":4,"method":"mge_seal","params":{"store_path":".memory-genome"}}
{"jsonrpc":"2.0","id":5,"method":"mge_validate","params":{"store_path":".memory-genome","deep":true}}
```

Reusable session transcript: `examples/mcp_agent_session.jsonl`.

## Python SDK

Где находится:

```text
sdk/python/mge_sdk/__init__.py
sdk/python/pyproject.toml
sdk/python/README.md
examples/python_basic_usage.py
examples/python_agent_host.py
```

Запуск:

```bash
python examples/python_basic_usage.py
python examples/python_agent_host.py
```

Optional editable install:

```bash
python -m pip install -e sdk/python
python -c "import mge_sdk; print(mge_sdk.MemoryGenomeClient)"
```

Basic shape:

```python
from mge_sdk import MemoryGenomeClient

client = MemoryGenomeClient(".memory-genome", passphrase_env="MGE_PASSPHRASE")
client.init(profile="fast", encrypted=True)
client.remember("Agent should use recalled context.", kind="procedure", scope="agent")
packet = client.recall("agent context", mode="focused", scope="agent")
client.checkpoint()
client.seal()
client.validate(deep=True)
client.rebuild_indexes()
client.export_markdown()
```

Typed surface включает `RecallMode`, `RememberOptions`, `ContextPacket`, `StoreStats`, `ValidationReport`, `McpError`, `MgeCommandError`, `MgeProtocolError` и `result_or_raise_mcp_error(response)`.

## TypeScript SDK

Где находится:

```text
sdk/typescript/src/mge.ts
sdk/typescript/package.json
sdk/typescript/tsconfig.json
sdk/typescript/README.md
examples/typescript_basic_usage.ts
examples/typescript_agent_host.ts
```

Запуск:

```bash
node examples/typescript_basic_usage.ts
node examples/typescript_agent_host.ts
```

Optional package smoke:

```bash
cd sdk/typescript
npm run smoke
npm run check # if tsc is available
```

Basic shape:

```typescript
import { MemoryGenomeClient } from "./sdk/typescript/src/mge.ts";

const client = new MemoryGenomeClient(".memory-genome", {
  passphraseEnv: "MGE_PASSPHRASE",
});
client.init("fast", { encrypted: true });
client.remember("Agent should use recalled context.", {
  kind: "procedure",
  scope: "agent",
});
const packet = client.recall("agent context", {
  mode: "focused",
  scope: "agent",
});
client.checkpoint();
client.seal();
client.validate({ deep: true });
client.rebuildIndexes();
client.exportMarkdown();
```

Typed surface включает `RecallMode`, `RememberOptions`, `RecallOptions`, `ContextPacket`, `StoreStats`, `ValidationReport`, `McpStructuredError`, `MemoryGenomeCommandError`, `MemoryGenomeProtocolError` и `resultOrThrowMcpError(response)`.

## Encrypted Store Unlock

Encrypted stores используют те же integration paths. Host передает только имя environment variable, где лежит passphrase:

- CLI: `--passphrase-env MGE_PASSPHRASE`
- MCP params: `"passphrase_env": "MGE_PASSPHRASE"`
- Python: `MemoryGenomeClient(..., passphrase_env="MGE_PASSPHRASE")`
- TypeScript: `new MemoryGenomeClient(..., { passphraseEnv: "MGE_PASSPHRASE" })`

Passphrase value должен оставаться вне protocol payloads и logs. Encrypted mode защищает `hot/hot.mgl`, `hot/snapshot.mgs` и sealed page payloads в `pages/*.mgp`. Indexes, marker dictionary, catalog summaries и Markdown export остаются plaintext by design.

## Local-First Safety

- Internet access не требуется.
- Corpus files не исполняются.
- Stores открываются только из explicit paths.
- Markdown export пишет в `exports/memory.md` по умолчанию или в explicit output path.
- `full_scope` recall требует `scope`.
- Adapter и SDK не меняют storage layout, codec, filters, recall semantics или encryption behavior.

## Current Limits

- `mge-mcp-server` - MCP-ready local JSON-RPC, а не full external MCP SDK implementation.
- SDK - thin local wrappers вокруг `mge`; package publishing пока нет.
- Encrypted indexes/blind marker metadata, vector DB, UI и remote service hosting вне текущего integration layer.
