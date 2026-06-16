# Memory Genome Engine

[![Rust](https://img.shields.io/badge/Rust-1.95%2B-f74c00?logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![License: Apache--2.0](https://img.shields.io/badge/License-Apache--2.0-green.svg)](LICENSE)
[![Interface](https://img.shields.io/badge/interface-CLI%20%7C%20TUI%20%7C%20MCP%20%7C%20SDK-informational)](docs/INTEGRATION.md)
[![Storage](https://img.shields.io/badge/storage-binary%20local--first-6f42c1)](docs/ARCHITECTURE.md)

[Russian version](README.ru.md)

Memory Genome Engine is a local-first structured memory engine for AI agents. It stores typed `MemoryCell` records, describes them with `MarkerGenome`, moves cold memory into sealed binary pages, and returns task-relevant `ContextPacket` output for agent workflows.

Demo GIF placeholder: `assets/mge-console-demo.gif`

## What It Does

- Remembers facts, decisions, preferences, notes, and agent observations.
- Keeps recent memory in fast L1 Hot RAM with durable binary persistence.
- Seals older memory into immutable binary pages with candidate indexes.
- Supports focused, broad, and full-scope recall.
- Provides CLI, TUI, JSON-RPC/MCP-ready adapter, Python SDK, and TypeScript SDK.
- Supports opt-in encrypted stores for hot payloads, snapshots, and sealed page payloads.
- Uses binary runtime storage; JSON is protocol/debug/benchmark output only.

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
- [Release and benchmark checks](docs/RELEASE.md)

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

## License

Apache License, Version 2.0. Copyright (c) 2026 ECD5A.
