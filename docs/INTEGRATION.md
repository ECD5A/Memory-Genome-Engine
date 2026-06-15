# Integration

[Russian version](INTEGRATION.ru.md)

Memory Genome Engine is integrated as a local-first memory service for agents. The agent does not read store files directly. It calls an integration boundary, gets a `ContextPacket`, works with that context, then stores useful results back through `remember`.

Core flow stays unchanged:

```text
remember -> L1 Hot RAM + pending persistence -> hot/hot.mgl
seal -> Sealed Pages + Indexes
recall -> ContextPacket
```

## Agent Lifecycle

1. Agent receives a task.
2. Agent calls `recall` with `focused`, `broad`, or explicit `full_scope`.
3. Memory Genome Engine returns a `ContextPacket`.
4. Agent uses the packet as task-relevant memory.
5. Agent stores durable lessons, decisions, preferences, or task state through `remember`.
6. Agent may call `checkpoint` for hot-memory durability or `seal` to move hot cells into sealed pages.

## Agent Host Pattern

The host owns orchestration. Memory Genome Engine owns memory:

```text
host starts task
-> recall focused or broad
-> host does local work using ContextPacket
-> remember useful result
-> checkpoint for hot durability
-> recall again if the task continues
-> seal when the task/session boundary is stable
-> validate --deep in smoke or maintenance flows
```

Use recall modes conservatively:

- `focused`: default for a narrow question, next action, or small tool decision.
- `broad`: project/module/task planning where the agent needs more related memory.
- `full_scope`: explicit audit/export/review inside a known scope; always pass `scope`.

Local host examples:

- Rust/CLI process host: `examples/agent_host_cli.rs`
- Python SDK host: `examples/python_agent_host.py`
- TypeScript SDK host: `examples/typescript_agent_host.ts`
- MCP JSON-RPC transcript: `examples/mcp_agent_session.jsonl`

## ContextPacket Contract

`ContextPacket` is the integration result for recall. It contains:

- `query`: the request text.
- `relevant_memory`: returned memory items with kind, content, trust, status, scope, sensitivity, and marker strings.
- `constraints`: retrieval constraints the agent should obey.
- `warnings`: safety or recall warnings.
- `debug`: recall mode, index kind, candidate/page/cell counters, cache counters, and timing breakdown.

The packet is task-relevant and size-controlled, not necessarily tiny. `focused` is narrow, `broad` is wider, and `full_scope` requires an explicit scope.

MCP/SDK recall responses keep the core `ContextPacket` under `context_packet` and also expose a stable adapter wrapper under `context`:

- `query`
- `mode`
- `relevant_memory`
- `constraints`
- `warnings`
- `score_details`
- `debug`
- `store_stats`

## Integration Boundaries

Use the lowest layer that fits the host:

- Rust: use `mge-core::MemoryEngine` directly.
- CLI: use the `mge` binary for scripts and shell-based agents.
- MCP-style: use `mge-mcp-server` for JSON-RPC over stdin/stdout.
- Python: use the thin wrapper in `sdk/python`.
- TypeScript: use the thin wrapper in `sdk/typescript`.

Rust remains the core. Python and TypeScript wrappers delegate to the Rust CLI and do not duplicate memory logic.

## Encrypted Store Unlock

Encrypted stores use the same integration paths. The host passes only the environment variable name that contains the passphrase:

- CLI: `--passphrase-env MGE_PASSPHRASE`
- MCP JSON-RPC params: `"passphrase_env": "MGE_PASSPHRASE"`
- Python SDK: `MemoryGenomeClient(..., passphrase_env="MGE_PASSPHRASE")`
- TypeScript SDK: `new MemoryGenomeClient(..., { passphraseEnv: "MGE_PASSPHRASE" })`

The passphrase value must stay outside protocol payloads and logs. Current encryption covers `hot/hot.mgl`, `hot/snapshot.mgs`, and sealed page payloads in `pages/*.mgp`. Indexes, marker dictionary, catalog summaries, and Markdown export remain plaintext by design.

## Local Developer Setup

Build the Rust tools first:

```bash
cargo build
```

Run the MCP-ready adapter:

```bash
cargo run -p mge-cli --bin mge-mcp-server
```

Run SDK smokes from the repository root:

```bash
python examples/python_basic_usage.py
node examples/typescript_basic_usage.ts
```

Optional local packaging checks:

```bash
python -m pip install -e sdk/python
cd sdk/typescript
npm run smoke
npm run check # if tsc is available
```

The current decision is to keep the versioned JSON-RPC stdin/stdout adapter as the main local MCP-ready surface. A full external MCP SDK dependency is intentionally deferred until the contract needs host-specific transport features.

## Versioning

The current integration contract uses:

- `protocol_version`: `mge-jsonrpc-1`
- `integration_schema_version`: `1`

These fields version only the MCP/SDK protocol contract. They do not change the binary storage format, page codec, filter strategy, or recall semantics.

## Local-First Safety

- No internet access is required.
- Corpus files are not executed.
- Stores are opened only from explicit paths.
- Markdown export writes to `exports/memory.md` by default or an explicit output path in the adapter.
- JSON is used for CLI/MCP/SDK protocol output and debug reports only. It is not runtime storage.
- Runtime storage remains binary: `manifest.mgm`, `dictionary/markers.mgd`, `hot/hot.mgl`, `pages/*.mgp`, and `indexes/*.mgi`.

## Current Limits

- `mge-mcp-server` is an MCP-ready local JSON-RPC adapter, not a full external MCP SDK implementation.
- SDKs are thin local wrappers around `mge`; package publishing is not done yet.
- Encrypted indexes/blind marker metadata, vector DB, UI, and remote service hosting are outside the current integration layer.

## Troubleshooting

- If a wrapper cannot find `mge`, pass the repository cargo command explicitly.
- If an MCP request fails, check the structured `error.details.error_kind` before parsing the human-readable message.
- If `full_scope` recall fails, provide `scope`; this is required to avoid accidental broad memory exposure.
- If Markdown export writes somewhere unexpected, pass `output_path` explicitly or use the default store `exports/memory.md`.
- JSON-RPC/CLI JSON is protocol or debug output only. Runtime storage remains binary.
