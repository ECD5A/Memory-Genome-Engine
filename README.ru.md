# Memory Genome Engine

[![Rust](https://img.shields.io/badge/Rust-1.95%2B-f74c00?logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![License: MIT](https://img.shields.io/badge/License-MIT-green.svg)](LICENSE)
[![Status](https://img.shields.io/badge/status-Mandate%204%20in%20progress-blue)](PROJECT_STATUS.ru.md)
[![Interface](https://img.shields.io/badge/interface-TUI%20%7C%20CLI%20%7C%20Core%20API%20%7C%20MCP-informational)](crates/)
[![Storage](https://img.shields.io/badge/storage-binary%20cells%20%2B%20pages-6f42c1)](docs/ARCHITECTURE.ru.md)

[English version](README.md)

Memory Genome Engine - Rust-first движок структурированной памяти для LLM-агентов. Он хранит память как типизированные `MemoryCell`, описывает каждую запись через `MarkerGenome`, переносит холодную память в binary sealed pages и возвращает task-relevant `ContextPacket` через marker-based recall.

```text
Memory = Cells + Markers + Pages + Filters + Context Packets
```

Движок local-first: агент запрашивает релевантную память у MGE, а не читает raw vault files и не владеет всем memory store.

## Текущие Возможности

- Rust core library: `mge-core`.
- Human terminal interface: `mge tui`.
- First-run setup helper: `mge setup`.
- CLI и local JSON-RPC adapter: `mge`, `mge-mcp-server`.
- Thin Python и TypeScript SDK wrappers поверх Rust CLI.
- L1 Hot RAM layer с durable binary hot log.
- Sealed binary page layer с page catalog и candidate indexes.
- `ExactMarkerPageIndex` как default candidate index.
- Optional `BinaryFusePageIndex` на `xorf::BinaryFuse16`.
- Recall modes: focused, broad, full-scope.
- Deep validation и safe index/catalog rebuild.
- Encrypted mode для hot log, checkpoint snapshot и sealed page payloads.
- Read-only `mge doctor` diagnostics и local release smoke scripts.

## Быстрый Старт

```bash
cargo build
cargo run -p mge-cli -- setup
cargo run -p mge-cli -- remember "User prefers concise technical explanations" --kind user_preference --scope global --trust user_confirmed
cargo run -p mge-cli -- recall "How should the agent answer technical questions?"
cargo run -p mge-cli -- seal
cargo run -p mge-cli -- validate --deep
cargo run -p mge-cli -- tui
```

Полный старт и базовое использование: [Quickstart](QUICKSTART.ru.md).

## Документация

- [Quickstart](QUICKSTART.ru.md)
- [Архитектура](docs/ARCHITECTURE.ru.md)
- [Security](docs/SECURITY.ru.md)
- [Интеграция / MCP / SDK](docs/INTEGRATION.ru.md)
- [Release / Бенчмарки](docs/RELEASE.ru.md)
- [Статус проекта](PROJECT_STATUS.ru.md)

## Runtime Storage Policy

Runtime storage бинарный:

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

JSON используется только как protocol/debug/benchmark output. Это не runtime storage.

## Security Summary

Encrypted stores включаются явно:

```bash
export MGE_PASSPHRASE="use-a-real-secret"
mge init --encrypted --passphrase-env MGE_PASSPHRASE
```

Encrypted mode защищает:

- `hot/hot.mgl` hot record payloads;
- `hot/snapshot.mgs` checkpoint payloads;
- `pages/*.mgp` sealed page payloads.

Metadata остаётся plaintext by design: marker dictionary, index files, page catalog summaries, safe manifest metadata, Markdown export и process memory while unlocked. Missing unlock возвращает `store_locked`; wrong key возвращает `auth_failed` / authentication failure. Подробнее: [Security](docs/SECURITY.ru.md).

## Примеры

- [Rust API example](examples/basic_usage.rs)
- [Rust CLI host example](examples/agent_host_cli.rs)
- [Python SDK example](examples/python_basic_usage.py)
- [Python agent host example](examples/python_agent_host.py)
- [TypeScript SDK example](examples/typescript_basic_usage.ts)
- [TypeScript agent host example](examples/typescript_agent_host.ts)
- [MCP JSON-RPC session](examples/mcp_agent_session.jsonl)

## Текущие Ограничения

- Web/desktop GUI нет; human interface terminal-first через `mge tui`.
- Vector database нет.
- Encrypted indexes и blind marker metadata пока нет.
- Encrypted Markdown export пока нет.
- External MCP SDK dependency нет; текущий adapter - local JSON-RPC stdin/stdout.

## Donate

If Memory Genome Engine is useful to your work, you can support the project here:

- Bitcoin (BTC): `1ECDSA1b4d5TcZHtqNpcxmY8pBH1GgHntN`
- USDT (TRC20): `TUF4vPdB6QkjCvZq18rBL4Qj4dK5ihCN75`

## Лицензия

MIT License. Copyright (c) 2026 ECD5A.
