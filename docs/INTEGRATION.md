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

## ContextPacket Contract

`ContextPacket` is the integration result for recall. It contains:

- `query`: the request text.
- `relevant_memory`: returned memory items with kind, content, trust, status, scope, sensitivity, and marker strings.
- `constraints`: retrieval constraints the agent should obey.
- `warnings`: safety or recall warnings.
- `debug`: recall mode, index kind, candidate/page/cell counters, cache counters, and timing breakdown.

The packet is task-relevant and size-controlled, not necessarily tiny. `focused` is narrow, `broad` is wider, and `full_scope` requires an explicit scope.

## Integration Boundaries

Use the lowest layer that fits the host:

- Rust: use `mge-core::MemoryEngine` directly.
- CLI: use the `mge` binary for scripts and shell-based agents.
- MCP-style: use `mge-mcp-server` for JSON-RPC over stdin/stdout.
- Python: use the thin wrapper in `sdk/python`.
- TypeScript: use the thin wrapper in `sdk/typescript`.

Rust remains the core. Python and TypeScript wrappers delegate to the Rust CLI and do not duplicate memory logic.

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
- Encryption, vector DB, UI, and remote service hosting are outside Mandate 2 foundation work.
