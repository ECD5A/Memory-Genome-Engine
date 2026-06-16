<h1 align="center">Memory Genome Engine</h1>

<p align="center">
  <a href="https://www.rust-lang.org/"><img alt="Rust 1.95+" src="https://img.shields.io/badge/Rust-1.95%2B-b45309?style=flat-square&logo=rust&logoColor=white"></a>
  <a href="LICENSE"><img alt="Apache 2.0" src="https://img.shields.io/badge/license-Apache--2.0-15803d?style=flat-square"></a>
  <a href="docs/ARCHITECTURE.md"><img alt="Local-first memory" src="https://img.shields.io/badge/local--first-memory-0e7490?style=flat-square"></a>
  <a href="docs/ARCHITECTURE.md"><img alt="Binary storage" src="https://img.shields.io/badge/binary-storage-6d28d9?style=flat-square"></a>
  <a href="docs/INTEGRATION.md"><img alt="CLI TUI MCP" src="https://img.shields.io/badge/CLI%20.%20TUI%20.%20MCP-ready-0369a1?style=flat-square"></a>
  <a href="docs/SECURITY.md"><img alt="Encrypted stores" src="https://img.shields.io/badge/encrypted-stores-15803d?style=flat-square"></a>
  <a href="docs/INTEGRATION.md"><img alt="Python TypeScript SDK" src="https://img.shields.io/badge/Python%20.%20TypeScript-SDK-2563eb?style=flat-square"></a>
  <br>
  <sub><a href="https://github.com/ECD5A/Memory-Genome-Engine/blob/main/README.ru.md">Русская версия</a></sub>
</p>

Memory Genome Engine is a local-first structured memory engine for AI agents. It stores typed `MemoryCell` records, describes them with `MarkerGenome`, moves cold memory into sealed binary pages, and returns task-relevant `ContextPacket` output for agent workflows.

<p align="center">
  <img src="assets/mge-console-demo-en.gif" alt="Memory Genome Engine terminal dashboard" width="100%">
</p>

## What It Does

- Remembers facts, decisions, preferences, notes, and agent observations.
- Keeps recent memory in fast L1 Hot RAM with durable binary persistence.
- Seals older memory into immutable binary pages with candidate indexes.
- Supports focused, broad, and full-scope recall.
- Provides CLI, TUI, JSON-RPC/MCP-ready adapter, Python SDK, and TypeScript SDK.
- Supports opt-in encrypted stores for hot payloads, snapshots, and sealed page payloads.
- Uses binary runtime storage; JSON is protocol/debug report output only.

## Quick Start

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

Payload encryption protects hot records, snapshots, and sealed page payloads. Metadata such as marker dictionary, indexes, catalog summaries, Markdown export, and process memory while unlocked remains plaintext by design. See [Security](docs/SECURITY.md).

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

## Documentation

- [Quickstart](QUICKSTART.md)
- [Architecture](docs/ARCHITECTURE.md)
- [Security model](docs/SECURITY.md)
- [Integration / MCP / SDK](docs/INTEGRATION.md)
- [Release checks](docs/RELEASE.md)

## Community

- [License](LICENSE)
- [Notice](NOTICE)
- [Security policy](SECURITY.md)
- [Contributing](CONTRIBUTING.md)
- [Code of conduct](CODE_OF_CONDUCT.md)

## Donate

If Memory Genome Engine is useful to your work, you can support the project here:

- Bitcoin (BTC): `1ECDSA1b4d5TcZHtqNpcxmY8pBH1GgHntN`
- USDT (TRC20): `TUF4vPdB6QkjCvZq18rBL4Qj4dK5ihCN75`

## Contact

For commercial integration, support, collaboration, and partnership inquiries:

<p>
  <a href="mailto:stelmak159@gmail.com" aria-label="Email"><img alt="Email" height="24" src="https://cdn.simpleicons.org/gmail/EA4335"></a>
  &nbsp;
  <a href="https://t.me/ECDS4" aria-label="Telegram"><img alt="Telegram" height="24" src="https://cdn.simpleicons.org/telegram/26A5E4"></a>
  &nbsp;
  <a href="https://github.com/ECD5A/Memory-Genome-Engine" aria-label="GitHub repository"><picture><source media="(prefers-color-scheme: dark)" srcset="https://cdn.simpleicons.org/github/FFFFFF"><img alt="GitHub repository" height="24" src="https://cdn.simpleicons.org/github/181717"></picture></a>
</p>

## License

Apache License, Version 2.0. Copyright (c) 2026 ECD5A.
