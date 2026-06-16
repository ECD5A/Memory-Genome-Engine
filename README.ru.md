# Memory Genome Engine

[![Rust](https://img.shields.io/badge/Rust-1.95%2B-f74c00?logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![License: Apache--2.0](https://img.shields.io/badge/License-Apache--2.0-green.svg)](LICENSE)
[![Interface](https://img.shields.io/badge/interface-CLI%20%7C%20TUI%20%7C%20MCP%20%7C%20SDK-informational)](docs/INTEGRATION.ru.md)
[![Storage](https://img.shields.io/badge/storage-binary%20local--first-6f42c1)](docs/ARCHITECTURE.ru.md)

[English version](README.md)

Memory Genome Engine - local-first движок структурированной памяти для AI-агентов. Он хранит типизированные `MemoryCell`, описывает их через `MarkerGenome`, переносит холодную память в sealed binary pages и возвращает task-relevant `ContextPacket` для agent workflows.

Demo GIF placeholder: `assets/mge-console-demo.gif`

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
- [Release и benchmark checks](docs/RELEASE.ru.md)

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
