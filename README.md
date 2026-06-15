# Memory Genome Engine

[![Rust](https://img.shields.io/badge/Rust-1.95%2B-f74c00?logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![License: MIT](https://img.shields.io/badge/License-MIT-green.svg)](LICENSE)
[![Status](https://img.shields.io/badge/status-Mandate%202%20integration-blue)](PROJECT_STATUS.md)
[![Interface](https://img.shields.io/badge/interface-CLI%20%7C%20Core%20API-informational)](crates/)
[![Storage](https://img.shields.io/badge/storage-cells%20%2B%20markers%20%2B%20pages-6f42c1)](docs/ARCHITECTURE.md)

[Русская версия](README.ru.md)

A fast structured memory engine for LLM agents. It stores memory as typed cells, encodes them with marker genomes, groups them into pages, and retrieves task-relevant context packets through marker-based candidate page search.

## Why This Exists

Most agent memory systems store raw text chunks, Markdown notes, or vector database entries. Memory Genome Engine stores typed memory cells with marker genomes, so agents can retrieve structured memory faster and more precisely.

The core idea is:

```text
Memory = Cells + Markers + Pages + Filters + Context Packets
```

Agents should lease memory from the engine, not own the whole memory store. They ask for relevant context and receive a compact context packet instead of reading vault files directly.

## Architecture

```text
Cells -> Marker Genome -> Hot Memory -> Sealed Pages -> Candidate Page Index -> Context Packet
```

The current implementation is Rust-first:

- `mge-core`: reusable memory engine library.
- `mge-cli`: command-line interface, binary name `mge`, plus `mge-mcp-server` for local JSON-RPC integration.
- `sdk/python` and `sdk/typescript`: thin wrappers around the Rust CLI.
- `.memory-genome/`: local binary store with `manifest.mgm`, `dictionary/markers.mgd`, `hot/hot.mgl`, `pages/*.mgp`, and `indexes/*.mgi`.
- `MarkerGenome`: structured marker DNA of a `MemoryCell`; flattened marker IDs remain the runtime/index view.
- Runtime storage uses MessagePack-oriented binary files; zstd compression is available for sealed pages.
- Candidate page search uses `ExactMarkerPageIndex` by default; `BinaryFusePageIndex` is available as an opt-in probabilistic page filter backed by `xorf::BinaryFuse16`.

More detail:

- [Architecture](docs/ARCHITECTURE.md)
- [Roadmap](docs/ROADMAP.md)
- [Benchmarks](docs/BENCHMARKS.md)
- [Integration](docs/INTEGRATION.md)
- [MCP adapter](docs/MCP.md)
- [SDKs](docs/SDK.md)
- [Basic usage](examples/basic_usage.md)
- [Agent workflow](examples/agent_workflow.md)
- [Rust API example](examples/basic_usage.rs)
- [Python SDK example](examples/python_basic_usage.py)
- [TypeScript SDK example](examples/typescript_basic_usage.ts)
- [Project status](PROJECT_STATUS.md)

## Why Not Markdown

Markdown is good for export and human reading, but bad as the internal high-speed memory format. This engine stores typed cells and marker IDs internally, then emits human-readable context packets, Markdown exports, or explicit debug JSON when needed.

## Why Not Vector-Only RAG

Vector search can be added later as a reranker inside already selected candidate pages. The core system stays marker/page based so retrieval can be deterministic, compact, and policy-aware.

## Future Security

The storage layer is designed for future page-level encryption, session keys, blind marker indexes, and policy-gated access.

Future page write flow:

```text
encode page -> compress page -> encrypt page -> store page
```

Current non-encrypted flow:

```text
encode page -> optional compression -> no encryption -> store page
```

The agent should not own the key, read vault files directly, or receive more memory than a task requires.

## Quick Start

```bash
cargo build

cargo run -p mge-cli -- init

cargo run -p mge-cli -- remember "User prefers concise technical explanations" \
  --kind user_preference \
  --scope global \
  --trust user_confirmed

cargo run -p mge-cli -- recall "How should the agent answer technical questions?"

cargo run -p mge-cli -- seal

cargo run -p mge-cli -- recall "How should the agent answer technical questions?"

cargo run -p mge-cli -- stats
```

For compact sealed pages, initialize a new store with MessagePack and zstd:

```bash
cargo run -p mge-cli -- init --profile fast
cargo run -p mge-cli -- init --page-codec messagepack --compression zstd
```

For opt-in Binary Fuse candidate filtering:

```bash
cargo run -p mge-cli -- init --index-kind binary_fuse_page
cargo run -p mge-cli -- config set --index-kind binary_fuse_page
```

## CLI

```bash
mge init
mge init --profile fast
mge init --page-codec messagepack --compression zstd
mge init --index-kind binary_fuse_page
mge config set --page-clusterer marker_overlap
mge remember "..." --kind user_preference --scope global --trust user_confirmed
mge remember --kind user_preference --subject answer_style --json-value '{"style":"concise","max_examples":2}'
mge remember --kind project_fact --reference-value vault://references/api-key --sensitivity secret_reference
mge remember "Decision recorded" --kind decision --source-type issue --source-ref MGE-1 --link 1
mge recall "technical answer style"
mge recall "technical answer style" --mode broad
mge recall --mode full-scope --scope global
mge recall "api key" --include-secret-references
mge seal
mge config show
mge config set --page-codec messagepack --compression zstd
mge config set --index-kind binary_fuse_page
mge inspect
mge validate
mge stats
mge stats --json
mge export
mge export --format json # explicit debug export
```

Core benchmark/smoke harness:

```bash
cargo run -p mge-cli --bin mge-synthetic-bench -- --cells 1200 --pages 120 --scopes 16 --markers-per-cell 5 --marker-groups 12 --targeted-queries 6 --noise-queries 3 --repeats 5 --seed 1
cargo run -p mge-cli --bin mge-corpus-bench -- --corpus . --store-root ../mge-corpus-bench-store --profile mixed --max-files 200 --max-bytes 8388608 --chunk-lines 24 --repeats 3 --seed 1
cargo run -p mge-cli --bin mge-corpus-bench -- --generated --profile small --store-root ../mge-generated-bench-store --repeats 2 --seed 7
```

`mge-corpus-bench` reads only local text/code corpus files, skips symlinks and unsupported binary extensions, writes only under `--store-root`, compares exact vs Binary Fuse, and emits JSON benchmark/debug output with a human-readable recommendation section.

Benchmark reports are explained in [Benchmarks](docs/BENCHMARKS.md). JSON benchmark output is report/debug output only, not runtime storage.

## Agent Integration

Mandate 2 adds local integration boundaries without changing the core storage architecture:

- `mge-mcp-server`: MCP-ready JSON-RPC stdin/stdout adapter.
- Python SDK: thin wrapper in `sdk/python`.
- TypeScript SDK: thin wrapper in `sdk/typescript`.
- Agent examples: [Agent workflow](examples/agent_workflow.md).

The integration layer returns `ContextPacket` from recall and uses JSON only as protocol/debug output, not runtime storage.

Local integration smoke:

```bash
cargo run -p mge-cli --bin mge-mcp-server
python examples/python_basic_usage.py
node examples/typescript_basic_usage.ts
```

SDKs include repository-local packaging metadata for development:

```bash
python -m pip install -e sdk/python
cd sdk/typescript && npm run smoke
```

The current MCP-ready surface is the versioned JSON-RPC stdin/stdout adapter with `protocol_version = mge-jsonrpc-1` and `integration_schema_version = 1`. A full external MCP SDK dependency is intentionally not added yet.

Use `--marker` on recall for explicit marker search:

```bash
mge recall "technical answer style" --marker kind:user_preference --marker scope:global
```

Recall modes are explicit: `focused` is the default, `broad` returns a wider task-relevant packet, and `full-scope` requires `--scope` to return all default-allowed memory inside that scope.

Use `--json-value` on remember when the value should be stored as `MemoryValue::Structured` instead of text or a scalar:

```bash
mge remember --kind user_preference --subject answer_style --json-value '{"style":"concise","max_examples":2}'
```

In PowerShell, pass escaped quotes or assign the JSON string to a variable before invoking `mge`.

Use `--reference-value` for references/placeholders and `--timestamp-value` for Unix timestamp seconds. Do not pass raw credentials or secret material.

```bash
mge remember --kind project_fact --reference-value vault://references/api-key --sensitivity secret_reference
mge remember --kind task_state --timestamp-value 1760000000
```

Use `--source-type` and `--source-ref` together to record provenance. Use repeated `--link` values to link a new cell to existing cell IDs.

`mge config set` changes defaults and lightweight derived indexes only. Existing page files are not rewritten; each catalog entry keeps the codec/compression needed to read that page. Changing `--index-kind` rebuilds only the candidate page index from existing sealed pages.

`mge validate` is a read-only consistency check for manifest, page catalog, page files, page checksums, marker dictionary consistency/references, cell links, candidate index coverage, and orphan storage files.

`BinaryFusePageIndex` is a probabilistic candidate page filter, not an inverted `marker -> pages` map. It builds one real `xorf::BinaryFuse16` static filter per sealed page from that page's `marker_summary`, scans page filters on query, and may return extra candidate pages. `ExactMarkerPageIndex` remains the default for stable debugging.

Deprecated/rejected/superseded memories and `SecretReference` cells are filtered by default. Use recall opt-in flags only when the caller has an explicit reason and capability.

## Repository Layout

```text
crates/
  mge-core/
  mge-cli/
docs/
examples/
tests/
```

## Current Limits

- Mandate 1 core is closed: storage, L1 Hot RAM, sealed pages, indexes, validation/rebuild, CLI, and benchmark foundation are ready.
- Mandate 2 integration foundation is active: MCP-ready adapter and thin Python/TypeScript SDK wrappers are present.
- A larger user-provided corpus is still useful before any further scoring/filtering cleanup.
- No GUI.
- No chatbot.
- No vector database.
- No fake Binary Fuse implementation; the opt-in Binary Fuse path uses the real `xorf` crate.
- No fake encryption.
- No credential storage.

## Donate

If Memory Genome Engine is useful to your work, you can support the project here:

- Bitcoin (BTC): `1ECDSA1b4d5TcZHtqNpcxmY8pBH1GgHntN`
- USDT (TRC20): `TUF4vPdB6QkjCvZq18rBL4Qj4dK5ihCN75`

## License

MIT License. Copyright (c) 2026 ECD5A.
