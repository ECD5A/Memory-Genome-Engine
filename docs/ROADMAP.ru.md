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
- Binary runtime storage layout. Сделано: `manifest.mgm`, `dictionary/markers.mgd`, `hot/hot.mgl`, `pages/*.mgp` и `indexes/*.mgi`.
- Binary file headers, payload checksums и atomic writes. Сделано.
- L1 Hot RAM layer. Сделано с exact RAM indexes и queued binary hot-log persistence.
- Hot durability policies и checkpoint snapshot. Сделано: `fast`, `balanced` default, `safe`, плюс optional `hot/snapshot.mgs`.
- JSON runtime storage removal. Сделано для текущего storage path; JSON остаётся только как явный debug output/API parsing.
- Fast storage profile. Сделано как opt-in `mge init --profile fast`.
- Markdown human-readable export. Сделано как `.memory-genome/exports/memory.md`.
- Canonical page checksum и logical page sizing убраны с JSON и переведены на MessagePack bytes. Сделано.
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
- Минимализм index/filter. Сделано как project rule: не добавлять Bloom, Counting Bloom, Cuckoo, XOR, Ribbon или новую filter family без benchmark-доказательства реальной пользы и без дестабилизации `CandidatePageIndex`.

## v0.3

- Rust API freeze для SDK boundary. Foundation сделан через `MemoryEngine` integration review.
- MCP server. Foundation сделан как `mge-mcp-server`, local JSON-RPC stdin/stdout MCP-ready adapter.
- Python SDK. Foundation сделан как thin CLI wrapper в `sdk/python`; PyO3/maturin остаётся optional future packaging work.
- TypeScript SDK. Foundation сделан как thin Node wrapper в `sdk/typescript`.
- Agent integration examples. Foundation сделан.

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
