# Memory Genome Engine

[![Rust](https://img.shields.io/badge/Rust-1.95%2B-f74c00?logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![License: MIT](https://img.shields.io/badge/License-MIT-green.svg)](LICENSE)
[![Status](https://img.shields.io/badge/status-v0.1%20prototype-blue)](PROJECT_STATUS.md)
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

The v0.1 implementation is Rust-first:

- `mge-core`: reusable memory engine library.
- `mge-cli`: first command-line interface, binary name `mge`.
- `.memory-genome/`: local store with manifest, marker dictionary, hot JSONL log, page files, and JSON indexes.
- Page storage supports JSON or MessagePack codecs and none/zstd compression through stable traits.

More detail:

- [Architecture](docs/ARCHITECTURE.md)
- [Roadmap](docs/ROADMAP.md)
- [Basic usage](examples/basic_usage.md)
- [Project status](PROJECT_STATUS.md)

## Why Not Markdown

Markdown is good for export and human reading, but bad as the internal high-speed memory format. This engine stores typed cells and marker IDs internally, then emits human-readable context packets or JSON exports when needed.

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
cargo run -p mge-cli -- init --page-codec messagepack --compression zstd
```

## CLI

```bash
mge init
mge init --page-codec messagepack --compression zstd
mge remember "..." --kind user_preference --scope global --trust user_confirmed
mge recall "technical answer style"
mge seal
mge config show
mge config set --page-codec messagepack --compression zstd
mge inspect
mge stats
mge export --format json
```

Use `--marker` on recall for explicit marker search:

```bash
mge recall "technical answer style" --marker kind:user_preference --marker scope:global
```

`mge config set` changes only the defaults for future sealed pages. Existing page files are not rewritten; each catalog entry keeps the codec/compression needed to read that page.

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

- No GUI.
- No chatbot.
- No vector database.
- No fake Binary Fuse implementation.
- No fake encryption.
- No credential storage.

## Donate

If Memory Genome Engine is useful to your work, you can support the project here:

- Bitcoin (BTC): `1ECDSA1b4d5TcZHtqNpcxmY8pBH1GgHntN`
- USDT (TRC20): `TUF4vPdB6QkjCvZq18rBL4Qj4dK5ihCN75`

## License

MIT License. Copyright (c) 2026 ECD5A.
