# Дорожная Карта

[English version](ROADMAP.md)

## v0.1

- Rust core. Сделано.
- Rust CLI. Сделано.
- `MemoryCell`. Сделано.
- `MarkerDictionary`. Сделано.
- Hot memory. Сделано.
- Recall from hot memory. Сделано.
- Sealed pages. Сделано.
- Simple marker-to-page index. Сделано как `ExactMarkerPageIndex`.
- Context packets. Сделано.

## v0.2

- MessagePack page codec. Сделано как foundation.
- zstd compression. Сделано как foundation.
- Store config show/set для future page defaults. Сделано.
- JSON runtime path reduction. В работе: JSON остаётся для debug/export/config compatibility, runtime storage уходит в MessagePack/binary.
- Fast storage profile. Следующий opt-in шаг.
- Page clustering trait и logical page limits. Сделано как foundation.
- Reranking transparency через debug score details. Сделано.
- Exact value match и context-packet dedupe. Сделано как reranking/output hardening.
- Structured JSON marker extraction. Сделано как deterministic shallow extraction.
- `IndexKind` metadata и exact-index extension boundary. Сделано.
- Recall policy, agent capabilities и audit hook foundation. Сделано.
- Store-level clustering config и marker-overlap seal mode. Сделано.
- Реальный Binary Fuse page candidate filter за существующим index trait. Сделано на `xorf::BinaryFuse16`.
- Store validation hardening для checksums, links, marker dictionary consistency, orphan page files и unknown index files. Сделано.
- CLI milestone integration test против реального binary `mge`. Сделано.
- Более сильный reranking. Foundation сделан; дальнейший tuning лучше делать через benchmark.
- Дополнительные XOR/Ribbon-style index experiments. Отложено до понятной пользы Binary Fuse на больших stores.

## v0.3

- Rust API freeze для SDK boundary.
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
