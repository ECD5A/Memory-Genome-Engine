# Roadmap

[Русская версия](ROADMAP.ru.md)

## v0.1

- Rust core. Done.
- Rust CLI. Done.
- `MemoryCell`. Done.
- `MarkerDictionary`. Done.
- Hot memory. Done.
- Recall from hot memory. Done.
- Sealed pages. Done.
- Simple marker-to-page index. Done as `ExactMarkerPageIndex`.
- Context packets. Done.

## v0.2

- MessagePack page codec. Done as foundation.
- zstd compression. Done as foundation.
- Store config show/set for future page defaults. Done.
- JSON runtime path reduction. In progress: JSON remains for debug/export/config compatibility, runtime storage moves toward MessagePack/binary.
- Fast storage profile. Done as opt-in `mge init --profile fast`.
- Page clustering trait and logical page limits. Done as foundation.
- Reranking transparency through debug score details. Done.
- Exact value match and context-packet dedupe. Done as reranking/output hardening.
- Structured JSON marker extraction. Done as deterministic shallow extraction.
- `IndexKind` metadata and exact-index extension boundary. Done.
- Recall policy, agent capabilities, and audit hook foundation. Done.
- Store-level clustering config and marker-overlap seal mode. Done.
- Real Binary Fuse page candidate filter behind the existing index trait. Done with `xorf::BinaryFuse16`.
- Store validation hardening for checksums, links, marker dictionary consistency, orphan page files, and unknown index files. Done.
- CLI milestone integration test against the real `mge` binary. Done.
- Better reranking. Foundation done; future tuning should be benchmark-driven.
- Additional XOR/Ribbon-style index experiments. Deferred until Binary Fuse shows practical benefit on larger stores.

## v0.3

- Rust API freeze for SDK boundary.
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
