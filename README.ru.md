<h1 align="center">Memory Genome Engine</h1>

<p align="center">
  <a href="https://www.rust-lang.org/"><img alt="Rust 1.95+" src="https://img.shields.io/badge/Rust-1.95%2B-b45309?style=flat-square&logo=rust&logoColor=white"></a>
  <a href="LICENSE"><img alt="Apache 2.0" src="https://img.shields.io/badge/license-Apache--2.0-15803d?style=flat-square"></a>
  <a href="docs/ARCHITECTURE.ru.md"><img alt="Local-first memory" src="https://img.shields.io/badge/local--first-memory-0e7490?style=flat-square"></a>
  <a href="docs/ARCHITECTURE.ru.md"><img alt="Binary storage" src="https://img.shields.io/badge/binary-storage-6d28d9?style=flat-square"></a>
  <a href="docs/INTEGRATION.ru.md"><img alt="CLI TUI MCP" src="https://img.shields.io/badge/CLI%20.%20TUI%20.%20MCP-ready-0369a1?style=flat-square"></a>
  <a href="docs/SECURITY.ru.md"><img alt="Encrypted stores" src="https://img.shields.io/badge/encrypted-stores-15803d?style=flat-square"></a>
  <a href="docs/INTEGRATION.ru.md"><img alt="Python TypeScript SDK" src="https://img.shields.io/badge/Python%20.%20TypeScript-SDK-2563eb?style=flat-square"></a>
</p>

<p align="center">
  <a href="README.md">English version</a>
</p>

Memory Genome Engine - local-first движок структурированной памяти для AI-агентов. Он хранит типизированные `MemoryCell`, описывает их через `MarkerGenome`, переносит холодную память в sealed binary pages и возвращает task-relevant `ContextPacket` для agent workflows.

<p align="center">
  <img src="assets/mge-console-demo-ru.gif" alt="Терминальная панель Memory Genome Engine" width="100%">
</p>

## Что Он Делает

- Запоминает facts, decisions, preferences, notes и agent observations.
- Держит свежую память в быстром L1 Hot RAM с durable binary persistence.
- Запечатывает старую память в immutable binary pages с candidate indexes.
- Поддерживает focused, broad и full-scope recall.
- Даёт CLI, TUI, JSON-RPC/MCP-ready adapter, Python SDK и TypeScript SDK.
- Поддерживает opt-in encrypted stores для hot payloads, snapshots и sealed page payloads.
- Использует binary runtime storage; JSON только protocol/debug/benchmark output.

## Быстрый Старт

```bash
cargo build
cargo run -p mge-cli -- setup
cargo run -p mge-cli -- remember "User prefers concise technical answers" --kind user_preference --scope global --trust user_confirmed
cargo run -p mge-cli -- recall "How should the agent answer technical questions?"
cargo run -p mge-cli -- seal
cargo run -p mge-cli -- validate --deep
```

Terminal UI:

```bash
cargo run -p mge-cli -- tui
cargo run -p mge-cli -- setup --help
```

## Encrypted Store

```bash
export MGE_PASSPHRASE="use-a-real-secret"
cargo run -p mge-cli -- init --encrypted --passphrase-env MGE_PASSPHRASE
cargo run -p mge-cli -- remember "private memory" --passphrase-env MGE_PASSPHRASE
cargo run -p mge-cli -- recall "private memory" --passphrase-env MGE_PASSPHRASE
cargo run -p mge-cli -- seal --passphrase-env MGE_PASSPHRASE
cargo run -p mge-cli -- validate --deep --passphrase-env MGE_PASSPHRASE
```

Payload encryption защищает hot records, snapshots и sealed page payloads. Metadata вроде marker dictionary, indexes, catalog summaries, Markdown export и process memory while unlocked остаётся plaintext by design. Подробнее: [Security](docs/SECURITY.ru.md).

## Agent Integration

CLI:

```bash
cargo run -p mge-cli -- recall "project context" --mode broad --scope my_project
```

MCP-ready JSON-RPC adapter:

```bash
cargo run -p mge-cli --bin mge-mcp-server
```

SDK examples:

```bash
python examples/python_agent_host.py
node examples/typescript_agent_host.ts
```

## Документация

- [Quickstart](QUICKSTART.ru.md)
- [Архитектура](docs/ARCHITECTURE.ru.md)
- [Security model](docs/SECURITY.ru.md)
- [Интеграция / MCP / SDK](docs/INTEGRATION.ru.md)
- [Release checks](docs/RELEASE.ru.md)

## Community

- [License](LICENSE)
- [Notice](NOTICE)
- [Security policy](SECURITY.md)
- [Contributing](CONTRIBUTING.md)
- [Code of conduct](CODE_OF_CONDUCT.md)

## Donate

Если Memory Genome Engine полезен для вашей работы, проект можно поддержать здесь:

- Bitcoin (BTC): `1ECDSA1b4d5TcZHtqNpcxmY8pBH1GgHntN`
- USDT (TRC20): `TUF4vPdB6QkjCvZq18rBL4Qj4dK5ihCN75`

Открыт к обсуждению коммерческой интеграции, поддержки и партнёрства.

## Лицензия

Apache License, Version 2.0. Copyright (c) 2026 ECD5A.
