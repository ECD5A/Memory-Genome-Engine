# Дорожная Карта

[English version](ROADMAP.md)

## v0.1

- Rust core.
- Rust CLI.
- `MemoryCell`.
- `MarkerDictionary`.
- Hot memory.
- Recall from hot memory.
- Sealed pages.
- Simple marker-to-page index.
- Context packets.

## v0.2

- MessagePack page codec. Сделано как foundation.
- zstd compression. Сделано как foundation.
- Store config show/set для future page defaults. Сделано.
- Page clustering trait и logical page limits. Сделано как foundation.
- Reranking transparency через debug score details. Сделано.
- `IndexKind` metadata и exact-index extension boundary. Сделано.
- Store-level clustering config и marker-overlap seal mode.
- Более сильный reranking.
- Реальная XOR/Binary Fuse index implementation за существующим index trait.

## v0.3

- Python SDK через PyO3/maturin.
- TypeScript SDK или REST wrapper.
- MCP server.

## v0.4

- Page-level encryption.
- Session unlock.
- Encrypted indexes.
- Blind marker tokens.
- Audit log.

## v0.5

- Policy-gated retrieval.
- Agent capabilities.
- Memory poisoning detection.
- Conflict detection.
- Optional vector reranking внутри candidate pages.
