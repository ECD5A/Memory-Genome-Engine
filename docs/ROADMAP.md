# Roadmap

[Русская версия](ROADMAP.ru.md)

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

- MessagePack page codec. Done as foundation.
- zstd compression. Done as foundation.
- Store config show/set for future page defaults. Done.
- Page clustering trait and logical page limits. Done as foundation.
- Reranking transparency through debug score details. Done.
- `IndexKind` metadata and exact-index extension boundary. Done.
- Recall policy, agent capabilities, and audit hook foundation. Done.
- Store-level clustering config and marker-overlap seal mode. Done.
- Better reranking.
- Real XOR/Binary Fuse index implementation behind the existing index trait.

## v0.3

- Python SDK through PyO3/maturin.
- TypeScript SDK or REST wrapper.
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
- Optional vector reranking inside candidate pages.
