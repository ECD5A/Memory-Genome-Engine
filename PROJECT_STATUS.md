# Memory Genome Engine - Project Status

[Русская версия](PROJECT_STATUS.ru.md)

This file is the working ledger for this repository. Keep it current so we do not repeat completed work or reopen decisions without a reason.

## Source Of Truth

The project goal is defined by the original prompt and user corrections: build a compact, fast, local Rust-first memory engine for LLM agents, where memory is stored as typed cells + marker genomes + sealed pages + candidate filters, and the agent receives only a `ContextPacket`.

Core logic:

```text
Memory = Cells + Markers + Pages + Filters + Context Packets
Hot memory = mutable
Sealed memory = static indexed pages
ExactMarkerPageIndex = default
BinaryFusePageIndex = opt-in probabilistic candidate page filter
Agent receives ContextPacket, not raw memory store
```

Non-negotiable rules:

- Do not turn the project into a chatbot, UI, cloud service, Markdown memory, or vector DB.
- Do not store raw credentials/secrets; use only `SecretReference` placeholders.
- Do not add fake encryption or fake Binary Fuse.
- Do not break defaults for experiments; new fast/experimental modes start opt-in.
- Do not bloat the project: small modules, clear traits, tests, separate commits.

JSON policy:

- JSON/JSONL are not internal runtime storage, not defaults, and not part of the storage architecture.
- JSON is allowed only as explicit debug output, CLI `--json` output, or API-level structured input parsing.
- Runtime storage uses compact binary files now: MessagePack-based `.mgm`, `.mgd`, `.mgl`, `.mgp`, and `.mgi`.
- `MemoryValue::Structured(serde_json::Value)` is an API-level structured value, not a promise to store memory as JSON.

## Roadmap Snapshot

| Stage | Status | Notes |
| --- | --- | --- |
| v0.1 core/CLI | Done / hardening | Rust core, CLI, cells, markers, hot memory, sealed pages, exact index, recall, and context packets work. |
| v0.2 storage/index foundation | Done / hardening | Binary runtime storage layout, MessagePack, zstd, config, clustering, score debug, Binary Fuse opt-in, and validation hardening are done. |
| v0.2 remaining | In progress | Benchmark-driven reranking and compact storage/index hardening. |
| v0.3 SDK/MCP | Not started | Python/TypeScript/MCP only after Rust core API stabilizes. |
| v0.4 security | Foundation only | Interfaces/policy exist; real encryption/session unlock/blind markers are not started. |
| v0.5 safety/search | Partial foundation | Policy/capabilities exist; poisoning/conflict/vector reranking are not started. |

## Current Focus

- Push the v0.1/v0.2 core/storage/index foundation toward a fast compact runtime path.
- Keep JSON out of internal runtime storage; use it only for explicit debug output and structured input parsing.
- Keep the implementation deterministic, local, compact, marker/page based, and ready for later encryption, SDKs, and MCP.

## Done

- Repository initialized with git.
- Rust toolchain confirmed: `cargo 1.95.0`, `rustc 1.95.0`.
- Initial workspace structure selected:
  - `crates/mge-core`
  - `crates/mge-cli`
  - `docs`
  - `examples`
  - `tests`
- Rust workspace created.
- `mge-core` implemented with:
  - typed memory models;
  - marker canonicalization and persistent dictionary;
  - deterministic marker extraction;
  - deterministic shallow marker extraction for structured JSON keys and short scalar values;
  - append-only binary hot log;
  - page model and MessagePack page codec;
  - exact marker-to-page candidate index;
  - recall, reranking, filtering, and context packet output;
  - extension traits for store, page codec, compression, index, retrieval, and security.
- `mge-cli` implemented with:
  - `init`
  - `remember`
  - `recall`
  - `seal`
  - `inspect`
  - `validate`
  - `stats`
  - `export` / `export --format markdown`
  - `export --format json` as explicit debug output
- CLI `remember` supports structured values through `--json-value`, stored as `MemoryValue::Structured`.
- CLI `remember` supports typed reference and timestamp values through `--reference-value` and `--timestamp-value`.
- CLI `remember` supports provenance and graph hints through `--source-type`, `--source-ref`, and repeated `--link`.
- Sealing preserves cell `source` metadata and `links` in sealed pages.
- CLI `stats` supports `--json` while keeping the human output as the default.
- Documentation added:
  - `README.md`
  - `docs/ARCHITECTURE.md`
  - `docs/ROADMAP.md`
  - `examples/basic_usage.md`
- Roadmap refreshed to mark completed v0.1/v0.2 foundation work and deferred experiments clearly.
- Rust tests added for marker canonicalization, dictionary IDs, cell creation, marker generation, hot recall, sealing, sealed recall, index lookup, filtering, context packet text, and stats output.
- CLI milestone integration test added against the real `mge` binary.
- MIT license added for ECD5A.
- README polished with badges, navigation, Donate block, and license section.
- Russian mirrors added for important Markdown files.
- `MessagePackPageCodec` added behind the existing `PageCodec` trait as the first v0.2 codec step.
- `ZstdCompression` added behind the existing `Compressor` trait.
- Store manifest now records default `page_codec` and `compression` for newly sealed pages.
- Page catalog entries now record per-page codec/compression for mixed-store and backward-compatible reads.
- Internal store files now use the final binary layout:
  - `manifest.mgm`
  - `dictionary/markers.mgd`
  - `hot/hot.mgl`
  - `pages/*.mgp`
  - `indexes/page_index.mgi`
  - `indexes/marker_index.mgi`
  - `indexes/fuse_index.mgi`
  - `exports/memory.md` for human-readable Markdown export
- Hot memory now uses length-prefixed MessagePack records instead of JSONL.
- Marker dictionary, manifest, page catalog, and candidate indexes now persist as MessagePack binary files.
- Binary runtime files now carry fixed headers with magic bytes, file kind, format version, codec identifier, payload length, and SHA-256 payload checksum.
- Full-file storage writes now use temp-file writes, flush/sync, and same-directory rename where practical.
- Hot memory now stores a `hot_log` frame followed by `hot_record` frames.
- New sealed pages now store codec-independent SHA-256 content checksums.
- Page checksum canonical bytes and logical page-size estimates now use MessagePack instead of JSON.
- CLI `init` now supports binary runtime storage by default; JSON page codec is rejected for runtime store initialization/config.
- CLI `init --profile fast` added as opt-in compact storage profile: MessagePack + zstd + exact index.
- CLI `export` now writes Markdown to `.memory-genome/exports/memory.md` by default; JSON export is explicit debug output.
- CLI `config show` and `config set` added for existing stores.
- Storage config updates change only future seal defaults; existing pages remain untouched and readable through catalog metadata.
- Tests added for zstd roundtrip, init options, MessagePack+zstd sealed recall, binary storage layout, Markdown export, and binary catalog defaults.
- `PageClusterer` trait added.
- `ScopeKindClusterer` kept as the default seal clustering strategy.
- `MarkerOverlapClusterer` added as a deterministic no-ML extension strategy.
- `PageBuildOptions` added with 64 KiB target page bytes and 512 max cells defaults.
- Page builder now enforces logical page limits.
- `ContextDebugInfo.score_details` added for transparent reranking in JSON/debug output.
- Reranking now records marker, subject, value overlap, exact value match, trust, status, and sensitivity score components.
- Context packet building now deduplicates ranked cells by `cell_id` before returning memory to agents.
- Prompt text output remains compact and does not expose score internals.
- Explicit recall modes added: `focused` default, `broad`, and `full_scope`.
- `ContextPacket` is task-relevant and size-controlled, not assumed to be artificially small.
- `ContextDebugInfo` now reports recall mode, effective max items, scanned cells, returned items, and whether full-scope was used.
- `IndexKind` added with the implemented `exact_marker_page` kind.
- Manifest, page catalog, stats, and exact index files now carry index kind metadata.
- `CandidatePageIndex` now exposes `kind()` and query statistics for static index implementations.
- `BinaryFusePageIndex` added as opt-in `binary_fuse_page` while `ExactMarkerPageIndex` remains the default.
- Binary Fuse page filters are real `xorf::BinaryFuse16` filters built per sealed page from `marker_summary`; no fake Binary Fuse implementation was added.
- CLI `init` and `config set` now support `--index-kind exact_marker_page|binary_fuse_page`.
- Changing `index_kind` rebuilds only the candidate index from existing sealed pages; sealed page files are not rewritten.
- Recall debug now reports index kind, page filters scanned, candidate pages returned, loaded pages, sealed cells scanned, and post-load false-positive candidate pages.
- Tests now assert `exact_candidates ⊆ binary_fuse_candidates` for the same sealed pages and verify index-kind switching without page rewrites.
- Synthetic benchmark tool added as `cargo run -p mge-cli --bin mge-synthetic-bench`.
- Synthetic benchmark compares `exact_marker_page` and opt-in `binary_fuse_page` on identical generated stores and checks `exact_candidates ⊆ binary_fuse_candidates`.
- Synthetic benchmark harness now reports remember, seal, focused recall, broad recall, full-scope recall, index lookup, page decode, ContextPacket build, candidate pages, cells scanned, returned items, storage size, and p50/p95/avg metrics where practical.
- Hot log archiving now uses unique archive names when multiple seals happen within the same timestamp window.
- `ValidationReport` and CLI `validate` added as read-only consistency checks for manifest, catalog, pages, page checksums, marker references, and candidate index coverage.
- Store validation now checks cell links for unknown targets and self-links.
- Store validation now warns about orphan page files and unknown unmanaged index files.
- Store validation now checks marker dictionary forward/reverse consistency, canonical markers, and `next_id`.
- `RecallPolicy` added as the central recall filtering policy.
- `AgentCapabilities` added for explicit future access grants.
- CLI recall now has `--mode focused|broad|full-scope`, plus opt-in flags `--include-deprecated` and `--include-secret-references`.
- Full-scope recall requires an explicit `--scope`; deprecated/rejected/superseded memories are filtered by default.
- `AuditLogger` interface and `NoopAuditLogger` recall hook added.
- `PageClustererKind` added to manifest/config.
- CLI `init` and `config set` now support `--page-clusterer scope_kind|marker_overlap`.
- Seal path now uses configured page clusterer, with `scope_kind` as the default.

## In Progress

- No active implementation item at this moment.

## Next

- Continue core hardening through validation, storage, and index tests without changing defaults.
- Add durable audit log storage only in a later security package.
- Consider conflict/poisoning detection only after the current storage/index core remains stable.
- Add SDK wrappers only after the Rust core API stabilizes.

## Rollbacks / Do Not Repeat

- Do not start with UI, chatbot, cloud service, vector DB, fake encryption, fake Binary Fuse, or Markdown as the internal storage format.
- Do not store real credentials or secrets. Sensitive values must be represented with `SecretReference` metadata/placeholders.
- Do not replace the marker/page API with a vector-only retrieval flow.

## Verification Commands

```bash
cargo build
cargo test
cargo run -p mge-cli -- init
cargo run -p mge-cli -- init --profile fast
cargo run -p mge-cli -- remember "User prefers concise technical explanations" --kind user_preference --scope global --trust user_confirmed
cargo run -p mge-cli -- remember --kind user_preference --subject answer_style --json-value '{"style":"concise","max_examples":2}'
cargo run -p mge-cli -- remember --kind project_fact --reference-value vault://references/api-key --sensitivity secret_reference
cargo run -p mge-cli -- remember --kind task_state --timestamp-value 1760000000
cargo run -p mge-cli -- remember "Decision recorded" --kind decision --source-type issue --source-ref MGE-1 --link 1
cargo run -p mge-cli -- recall "How should the agent answer technical questions?"
cargo run -p mge-cli -- recall "How should the agent answer technical questions?" --mode broad
cargo run -p mge-cli -- recall --mode full-scope --scope global
cargo run -p mge-cli -- seal
cargo run -p mge-cli -- recall "How should the agent answer technical questions?"
cargo run -p mge-cli -- stats
cargo run -p mge-cli -- stats --json
cargo run -p mge-cli -- validate
cargo run -p mge-cli -- export
cargo run -p mge-cli -- init --index-kind binary_fuse_page
cargo run -p mge-cli -- config set --index-kind binary_fuse_page
cargo run -p mge-cli --bin mge-synthetic-bench -- --cells 1200 --pages 120 --scopes 16 --markers-per-cell 5 --marker-groups 12 --targeted-queries 6 --noise-queries 3 --repeats 5 --seed 1
```

## Verification Status

- `cargo fmt`: passed.
- `cargo test`: passed, 80 tests total (12 CLI unit tests + 4 CLI integration tests + 1 core unit test + 63 core integration tests).
- Recall modes tests: passed for focused top result, broad expanded output, full-scope scoped output, full-scope missing-scope error, default status filtering, and no JSON/JSONL runtime storage regression.
- Recall modes CLI smoke command: passed for `--mode broad`, `--mode full-scope --scope`, and full-scope missing-scope failure.
- Benchmark harness integration smoke test: passed for exact + Binary Fuse modes and required metrics.
- Benchmark harness CLI smoke command: passed.
  - config: 120 cells, 12 sealed pages, 4 logical scopes, 5 markers per cell, 4 marker groups, 4 targeted queries, 2 noise queries, 3 repeats, seed 7.
  - exact_marker_page: remember avg 8367 us, seal avg 61834 us, focused recall avg 5270 us, broad recall avg 12575 us, full-scope recall avg 1764 us, index lookup avg 1 us, page decode avg 391 us, ContextPacket build avg 944 us, storage 108585 bytes.
  - binary_fuse_page: remember avg 8040 us, seal avg 54785 us, focused recall avg 5312 us, broad recall avg 12805 us, full-scope recall avg 1871 us, index lookup avg 1 us, page decode avg 395 us, ContextPacket build avg 952 us, storage 112749 bytes.
  - subset check: focused exact candidates subset of binary_fuse candidates passed.
- Milestone smoke commands: passed.
- MessagePack+zstd smoke commands: passed.
- Config show/set mixed-store smoke commands: passed.
- Default clustering smoke commands: passed.
- Recall JSON score debug smoke command: passed.
- Index kind stats/config smoke command: passed.
- Binary Fuse init/recall/stats smoke command: passed.
- Exact-to-Binary-Fuse config switch smoke command: passed; sealed page hash unchanged.
- Binary storage layout CLI smoke command: passed; required `.mgm/.mgd/.mgl/.mgp/.mgi` files exist, old JSON/JSONL storage files absent, Markdown export size 621 bytes.
- Binary header CLI smoke command: passed; all runtime `.mg*` files had `MGEFILE` magic and corrupted page validation reported `wrong magic for page`.
- JSON runtime page codec reject smoke command: passed; `mge init --page-codec json` exits with `invalid input`.
- Synthetic exact-vs-Binary-Fuse benchmark smoke command: passed.
  - config: 1200 cells, 120 sealed pages, 12 marker groups, 6 targeted queries, 3 noise queries.
  - exact: avg recall latency 11545 us, total candidate pages 60, loaded pages 60, sealed cells scanned 600, result count 120.
  - binary_fuse_page: avg recall latency 13426 us, total candidate pages 60, loaded pages 60, sealed cells scanned 600, result count 120, post-load false-positive pages 0.
  - subset check: `exact_candidates ⊆ binary_fuse_candidates` passed.
- Small post-binary-layout benchmark smoke command: passed.
  - config: 120 cells, 12 sealed pages, 4 marker groups, 3 targeted queries, 1 noise query.
  - exact: avg recall latency 7410 us, total candidate pages 9, loaded pages 9, sealed cells scanned 90, result count 60.
  - binary_fuse_page: avg recall latency 4182 us, total candidate pages 9, loaded pages 9, sealed cells scanned 90, result count 60, post-load false-positive pages 0.
  - subset check: `exact_candidates ⊆ binary_fuse_candidates` passed.
- Validate CLI smoke commands: passed for `exact_marker_page` and `binary_fuse_page`.
- Page checksum smoke command: passed for MessagePack+zstd sealed page, checksum length 64, `mge validate --json` ok.
- Structured JSON remember smoke command: passed, exported value type `structured`.
- Typed reference/timestamp remember smoke command: passed, exported value types `reference` and `timestamp`.
- Source/link remember smoke command: passed, exported source and links retained.
- Source/link seal persistence test: passed.
- Link validation smoke command: passed for valid link and failed as expected for unknown link.
- Orphan storage validation tests: passed for orphan page files and unknown unmanaged index files.
- Context packet dedupe test: passed for duplicate ranked cells with the same `cell_id`.
- Structured JSON marker extraction tests: passed for marker generation and hot recall.
- Structured JSON marker extraction CLI smoke command: passed, recall matched `tag:style` and `tag:concise`.
- CLI milestone integration test: passed for init, remember, recall JSON, seal, stats JSON, and validate JSON.
- Fast profile CLI integration test: passed for `mge init --profile fast`.
- Binary storage layout tests: passed for `.mgm/.mgd/.mgl/.mgp/.mgi` files and absence of old JSON/JSONL storage files.
- Binary storage header validation tests: passed for wrong magic, wrong file kind, unsupported version, truncated payload, corrupted payload, wrong hot log magic, and wrong index magic.
- Markdown export test: passed for `.memory-genome/exports/memory.md`.
- Marker dictionary consistency validation test: passed.
- Stats JSON smoke command: passed, `sealed_pages` and `current_index_kind` exported.
- Recall policy secret-reference opt-in smoke command: passed.
- Marker-overlap clusterer seal smoke command: passed.
- Smoke result after sealing:
  - hot cells: 0
  - sealed pages: 1-2 depending on smoke scenario
  - sealed cells: 1-2 depending on smoke scenario
  - index type: `exact_marker_page` or `binary_fuse_page` depending on smoke scenario
  - current index kind: `exact_marker_page` or `binary_fuse_page` depending on smoke scenario
  - current page codec: `messagepack`
  - current compression: `none` or `zstd` depending on smoke scenario
