# Memory Genome Engine

[![Rust](https://img.shields.io/badge/Rust-1.95%2B-f74c00?logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![License: MIT](https://img.shields.io/badge/License-MIT-green.svg)](LICENSE)
[![Status](https://img.shields.io/badge/status-Mandate%204%20in%20progress-blue)](PROJECT_STATUS.md)
[![Interface](https://img.shields.io/badge/interface-TUI%20%7C%20CLI%20%7C%20Core%20API%20%7C%20MCP-informational)](crates/)
[![Storage](https://img.shields.io/badge/storage-binary%20cells%20%2B%20pages-6f42c1)](docs/ARCHITECTURE.md)

[Russian version](README.ru.md)

Memory Genome Engine is a Rust-first structured memory engine for LLM agents. It stores memory as typed `MemoryCell` records, encodes each cell with a `MarkerGenome`, seals cold memory into binary pages, and returns task-relevant `ContextPacket` output through marker-based recall.

```text
Memory = Cells + Markers + Pages + Filters + Context Packets
```

The engine is local-first. Agents recall relevant memory from the engine instead of reading raw vault files or owning the whole memory store.

## Current Capabilities

- Rust core library: `mge-core`.
- CLI and local JSON-RPC adapter: `mge`, `mge-mcp-server`.
- Human terminal interface: `mge tui`.
- Thin Python and TypeScript SDK wrappers over the Rust CLI.
- L1 Hot RAM layer with durable binary hot log.
- Sealed binary page layer with page catalog and candidate indexes.
- `ExactMarkerPageIndex` as the default candidate index.
- Optional `BinaryFusePageIndex` backed by `xorf::BinaryFuse16`.
- Focused, broad, and full-scope recall modes.
- Deep validation and safe index/catalog rebuild.
- Encrypted mode for hot log, checkpoint snapshot, and sealed page payloads.
- Read-only `mge doctor` diagnostics and local release smoke scripts.

## Quick Start

```bash
cargo build
cargo run -p mge-cli -- init --profile fast
cargo run -p mge-cli -- remember "User prefers concise technical explanations" --kind user_preference --scope global --trust user_confirmed
cargo run -p mge-cli -- recall "How should the agent answer technical questions?"
cargo run -p mge-cli -- seal
cargo run -p mge-cli -- validate --deep
cargo run -p mge-cli -- tui
```

Full setup and usage: [Quickstart](QUICKSTART.md).

## Documentation

- [Quickstart](QUICKSTART.md)
- [Architecture](docs/ARCHITECTURE.md)
- [Security](docs/SECURITY.md)
- [Integration / MCP / SDK](docs/INTEGRATION.md)
- [Release / Benchmarks](docs/RELEASE.md)
- [Project Status](PROJECT_STATUS.md)

## Runtime Storage Policy

Runtime storage is binary:

```text
.memory-genome/
  manifest.mgm
  dictionary/markers.mgd
  hot/hot.mgl
  hot/snapshot.mgs
  pages/*.mgp
  indexes/*.mgi
  exports/memory.md
```

JSON is protocol/debug/benchmark output only. It is not runtime storage.

## Security Summary

Encrypted stores are opt-in:

```bash
export MGE_PASSPHRASE="use-a-real-secret"
mge init --encrypted --passphrase-env MGE_PASSPHRASE
```

Encrypted mode protects:

- `hot/hot.mgl` hot record payloads;
- `hot/snapshot.mgs` checkpoint payloads;
- `pages/*.mgp` sealed page payloads.

Metadata remains plaintext by design: marker dictionary, index files, page catalog summaries, safe manifest metadata, Markdown export, and process memory while unlocked. Missing unlock returns `store_locked`; wrong keys return `auth_failed` / authentication failure. Details: [Security](docs/SECURITY.md).

## Examples

- [Rust API example](examples/basic_usage.rs)
- [Rust CLI host example](examples/agent_host_cli.rs)
- [Python SDK example](examples/python_basic_usage.py)
- [Python agent host example](examples/python_agent_host.py)
- [TypeScript SDK example](examples/typescript_basic_usage.ts)
- [TypeScript agent host example](examples/typescript_agent_host.ts)
- [MCP JSON-RPC session](examples/mcp_agent_session.jsonl)

## Current Limits

- No web/desktop GUI; the human interface is terminal-first through `mge tui`.
- No vector database.
- No encrypted indexes or blind marker metadata yet.
- No encrypted Markdown export yet.
- No external MCP SDK dependency; the current adapter is local JSON-RPC stdin/stdout.

## Donate

If Memory Genome Engine is useful to your work, you can support the project here:

- Bitcoin (BTC): `1ECDSA1b4d5TcZHtqNpcxmY8pBH1GgHntN`
- USDT (TRC20): `TUF4vPdB6QkjCvZq18rBL4Qj4dK5ihCN75`

## License

MIT License. Copyright (c) 2026 ECD5A.
