# Integration

This document is the single integration reference for agent hosts, the local JSON-RPC MCP-ready adapter, and the thin Python/TypeScript SDK wrappers.

JSON in this layer is protocol/debug output only. It is not runtime storage.

Core flow stays unchanged:

```text
remember -> L1 Hot RAM + pending persistence -> hot/hot.mgl
seal -> Sealed Pages + Indexes
recall -> ContextPacket
```

## Agent Lifecycle

The host owns orchestration. Memory Genome Engine owns memory.

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

- `focused`: default for a narrow question, next action, or tool decision.
- `broad`: project/module/task planning where the agent needs more related memory.
- `full_scope`: explicit audit/export/review inside a known scope; always pass `scope`.

Local host examples:

- Rust/CLI process host: `examples/agent_host_cli.rs`
- Python SDK host: `examples/python_agent_host.py`
- TypeScript SDK host: `examples/typescript_agent_host.ts`
- MCP JSON-RPC transcript: `examples/mcp_agent_session.jsonl`

## ContextPacket Contract

`ContextPacket` is the recall result. It contains:

- `query`
- `relevant_memory`
- `constraints`
- `warnings`
- `debug`

The packet is task-relevant and size-controlled, not necessarily tiny. MCP/SDK recall responses keep the core `ContextPacket` under `context_packet` and expose an adapter wrapper under `context`:

- `query`
- `mode`
- `relevant_memory`
- `constraints`
- `warnings`
- `score_details`
- `debug`
- `store_stats`

## Integration Surfaces

Use the lowest layer that fits the host:

- Rust: `mge-core::MemoryEngine`.
- CLI: `mge`.
- MCP-ready local adapter: `mge-mcp-server`, JSON-RPC over stdin/stdout.
- Python: thin wrapper in `sdk/python`.
- TypeScript: thin wrapper in `sdk/typescript`.

Rust remains the core. Python and TypeScript wrappers delegate to the Rust CLI and do not duplicate memory logic.

## MCP-Ready JSON-RPC Adapter

The adapter expects an existing initialized Memory Genome store. Create it once with CLI/setup before sending `mge_remember`, `mge_recall`, or other store tools:

```bash
cargo run -p mge-cli -- init --profile fast
```

Run:

```bash
cargo run -p mge-cli --bin mge-mcp-server
```

Contract:

- JSON-RPC version: `2.0`
- `protocol_version`: `mge-jsonrpc-1`
- `integration_schema_version`: `1`

Each input line is one JSON-RPC request:

```json
{"jsonrpc":"2.0","id":1,"method":"mge_stats","params":{"store_path":".memory-genome"}}
```

Each output line is one JSON-RPC response:

```json
{"jsonrpc":"2.0","id":1,"result":{"ok":true,"tool":"mge_stats","protocol_version":"mge-jsonrpc-1","integration_schema_version":1,"stats":{}}}
```

Schema discovery:

```json
{"jsonrpc":"2.0","id":"schema","method":"mge_schema","params":{}}
```

The schema response includes tool schemas, the `ContextPacket` wrapper contract, and the structured error contract. Golden fixtures live under `crates/mge-cli/tests/fixtures/mcp`.

One-line local schema smoke:

```bash
printf '%s\n' '{"jsonrpc":"2.0","id":"schema","method":"mge_schema","params":{}}' | cargo run -p mge-cli --bin mge-mcp-server
```

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
- optional `source_type` and `source_ref`
- optional `links`
- optional `passphrase_env`

`mge_recall` input:

- `store_path`
- `query`
- `mode`: `focused`, `broad`, or `full_scope`
- `scope`, required for `full_scope`
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

All store tools accept `store_path`; encrypted stores also accept `passphrase_env`. `mge_validate` accepts `deep`; `mge_export_markdown` accepts optional `output_path`.

There is no `mge_init` MCP tool in the current contract. This keeps the adapter focused on agent memory operations after the host has chosen or created the local store. If an agent host needs automated first-run setup, run `mge init` / `mge setup` once before starting the MCP session.

### Structured Errors

Errors are stable for SDKs:

```json
{"jsonrpc":"2.0","id":1,"error":{"code":-32602,"message":"invalid params: missing field `content`","tool_name":"mge_remember","recoverable":true,"protocol_version":"mge-jsonrpc-1","integration_schema_version":1,"details":{"error_kind":"invalid_params"}}}
```

Important `details.error_kind` values:

- `parse_error`
- `unknown_method`
- `invalid_params`
- `store_open_failed`
- `store_locked`
- `auth_failed`
- `invalid_request`

Malformed JSON returns `-32700`; unknown tools return `-32601`; missing or invalid params return `-32602`; store/runtime failures use `-32000`.

### MCP Workflow Example

Bootstrap the store first:

```bash
cargo run -p mge-cli -- init --profile fast
```

Then send JSON-RPC requests to `mge-mcp-server`:

```json
{"jsonrpc":"2.0","id":1,"method":"mge_remember","params":{"store_path":".memory-genome","content":"Agent should recall memory before editing.","kind":"procedure","scope":"agent","markers":["topic:agent_memory"],"trust":"user_confirmed","sensitivity":"private"}}
{"jsonrpc":"2.0","id":2,"method":"mge_recall","params":{"store_path":".memory-genome","query":"agent memory","mode":"focused","scope":"agent","max_items":5}}
{"jsonrpc":"2.0","id":3,"method":"mge_checkpoint","params":{"store_path":".memory-genome"}}
{"jsonrpc":"2.0","id":4,"method":"mge_seal","params":{"store_path":".memory-genome"}}
{"jsonrpc":"2.0","id":5,"method":"mge_validate","params":{"store_path":".memory-genome","deep":true}}
```

A reusable session transcript is in `examples/mcp_agent_session.jsonl`.

Run the transcript against the local adapter:

```bash
cargo run -p mge-cli --bin mge-mcp-server < examples/mcp_agent_session.jsonl
```

## Python SDK

Location:

```text
sdk/python/mge_sdk/__init__.py
sdk/python/pyproject.toml
sdk/python/README.md
examples/python_basic_usage.py
examples/python_agent_host.py
```

Run:

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

Typed surface includes `RecallMode`, `RememberOptions`, `ContextPacket`, `StoreStats`, `ValidationReport`, `McpError`, `MgeCommandError`, `MgeProtocolError`, and `result_or_raise_mcp_error(response)`.

## TypeScript SDK

Location:

```text
sdk/typescript/src/mge.ts
sdk/typescript/package.json
sdk/typescript/tsconfig.json
sdk/typescript/README.md
examples/typescript_basic_usage.ts
examples/typescript_agent_host.ts
```

Run:

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

Typed surface includes `RecallMode`, `RememberOptions`, `RecallOptions`, `ContextPacket`, `StoreStats`, `ValidationReport`, `McpStructuredError`, `MemoryGenomeCommandError`, `MemoryGenomeProtocolError`, and `resultOrThrowMcpError(response)`.

## Encrypted Store Unlock

Encrypted stores use the same integration paths. The host passes only the environment variable name that contains the passphrase:

- CLI: `--passphrase-env MGE_PASSPHRASE`
- MCP params: `"passphrase_env": "MGE_PASSPHRASE"`
- Python: `MemoryGenomeClient(..., passphrase_env="MGE_PASSPHRASE")`
- TypeScript: `new MemoryGenomeClient(..., { passphraseEnv: "MGE_PASSPHRASE" })`

The passphrase value must stay outside protocol payloads and logs. Encrypted mode protects `hot/hot.mgl`, `hot/snapshot.mgs`, and sealed page payloads in `pages/*.mgp`. Indexes, marker dictionary, catalog summaries, and Markdown export remain plaintext by design.

## Local-First Safety

- No internet access is required.
- Corpus files are not executed.
- Stores are opened only from explicit paths.
- Markdown export writes to `exports/memory.md` by default or an explicit output path.
- `full_scope` recall requires `scope`.
- The adapter and SDKs do not change storage layout, codec, filters, recall semantics, or encryption behavior.

## Current Limits

- `mge-mcp-server` is MCP-ready local JSON-RPC, not a full external MCP SDK implementation.
- SDKs are thin local wrappers around `mge`; package publishing is not done yet.
- Encrypted indexes/blind marker metadata, vector DB, UI, and remote service hosting are outside the current integration layer.
