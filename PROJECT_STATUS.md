# Memory Genome Engine - Project Status

[Русская версия](PROJECT_STATUS.ru.md)

This file is the working ledger for this repository. Keep it current so we do not repeat completed work or reopen decisions without a reason.

## Current Focus

- Build v0.1 Rust-first core and CLI in this folder.
- Keep the first implementation deterministic, local, marker/page based, and ready for later compression, encryption, SDKs, and MCP.

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
  - append-only hot JSONL store;
  - page model and JSON page codec;
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
  - `export --format json`
- CLI `remember` supports structured values through `--json-value`, stored as `MemoryValue::Structured`.
- CLI `remember` supports typed reference and timestamp values through `--reference-value` and `--timestamp-value`.
- CLI `remember` supports provenance and graph hints through `--source-type`, `--source-ref`, and repeated `--link`.
- CLI `stats` supports `--json` while keeping the human output as the default.
- Documentation added:
  - `README.md`
  - `docs/ARCHITECTURE.md`
  - `docs/ROADMAP.md`
  - `examples/basic_usage.md`
- Rust tests added for marker canonicalization, dictionary IDs, cell creation, marker generation, hot recall, sealing, sealed recall, index lookup, filtering, context packet text, and stats output.
- MIT license added for ECD5A.
- README polished with badges, navigation, Donate block, and license section.
- Russian mirrors added for important Markdown files.
- `MessagePackPageCodec` added behind the existing `PageCodec` trait as the first v0.2 codec step.
- `ZstdCompression` added behind the existing `Compressor` trait.
- Store manifest now records default `page_codec` and `compression` for newly sealed pages.
- Page catalog entries now record per-page codec/compression for mixed-store and backward-compatible reads.
- New sealed pages now store codec-independent SHA-256 content checksums.
- CLI `init` now supports `--page-codec json|messagepack` and `--compression none|zstd`.
- CLI `config show` and `config set` added for existing stores.
- Storage config updates change only future seal defaults; existing pages remain untouched and readable through catalog metadata.
- Tests added for zstd roundtrip, init options, MessagePack+zstd sealed recall, and legacy catalog defaults.
- `PageClusterer` trait added.
- `ScopeKindClusterer` kept as the default seal clustering strategy.
- `MarkerOverlapClusterer` added as a deterministic no-ML extension strategy.
- `PageBuildOptions` added with 64 KiB target page bytes and 512 max cells defaults.
- Page builder now enforces logical page limits.
- `ContextDebugInfo.score_details` added for transparent reranking in JSON/debug output.
- Reranking now records marker, subject, value overlap, exact value match, trust, status, and sensitivity score components.
- Prompt text output remains compact and does not expose score internals.
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
- Hot log archiving now uses unique archive names when multiple seals happen within the same timestamp window.
- `ValidationReport` and CLI `validate` added as read-only consistency checks for manifest, catalog, pages, page checksums, marker references, and candidate index coverage.
- Store validation now checks cell links for unknown targets and self-links.
- `RecallPolicy` added as the central recall filtering policy.
- `AgentCapabilities` added for explicit future access grants.
- CLI recall now has opt-in flags `--include-deprecated` and `--include-secret-references`.
- `AuditLogger` interface and `NoopAuditLogger` recall hook added.
- `PageClustererKind` added to manifest/config.
- CLI `init` and `config set` now support `--page-clusterer scope_kind|marker_overlap`.
- Seal path now uses configured page clusterer, with `scope_kind` as the default.

## In Progress

- No active implementation item at this moment.

## Next

- Benchmark `exact_marker_page` vs opt-in `binary_fuse_page` on larger stores before changing any defaults.
- Add durable audit log storage in a later security package.
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
cargo run -p mge-cli -- remember "User prefers concise technical explanations" --kind user_preference --scope global --trust user_confirmed
cargo run -p mge-cli -- remember --kind user_preference --subject answer_style --json-value '{"style":"concise","max_examples":2}'
cargo run -p mge-cli -- remember --kind project_fact --reference-value vault://references/api-key --sensitivity secret_reference
cargo run -p mge-cli -- remember --kind task_state --timestamp-value 1760000000
cargo run -p mge-cli -- remember "Decision recorded" --kind decision --source-type issue --source-ref MGE-1 --link 1
cargo run -p mge-cli -- recall "How should the agent answer technical questions?"
cargo run -p mge-cli -- seal
cargo run -p mge-cli -- recall "How should the agent answer technical questions?"
cargo run -p mge-cli -- stats
cargo run -p mge-cli -- stats --json
cargo run -p mge-cli -- validate
cargo run -p mge-cli -- init --index-kind binary_fuse_page
cargo run -p mge-cli -- config set --index-kind binary_fuse_page
cargo run -p mge-cli --bin mge-synthetic-bench -- --cells 1200 --pages 120 --marker-groups 12 --targeted-queries 6 --noise-queries 3
```

## Verification Status

- `cargo fmt`: passed.
- `cargo test`: passed, 50 tests total (9 CLI unit tests + 1 core unit test + 40 integration tests).
- Milestone smoke commands: passed.
- MessagePack+zstd smoke commands: passed.
- Config show/set mixed-store smoke commands: passed.
- Default clustering smoke commands: passed.
- Recall JSON score debug smoke command: passed.
- Index kind stats/config smoke command: passed.
- Binary Fuse init/recall/stats smoke command: passed.
- Exact-to-Binary-Fuse config switch smoke command: passed; sealed page hash unchanged.
- Synthetic exact-vs-Binary-Fuse benchmark smoke command: passed.
  - config: 1200 cells, 120 sealed pages, 12 marker groups, 6 targeted queries, 3 noise queries.
  - exact: avg recall latency 11545 us, total candidate pages 60, loaded pages 60, sealed cells scanned 600, result count 120.
  - binary_fuse_page: avg recall latency 13426 us, total candidate pages 60, loaded pages 60, sealed cells scanned 600, result count 120, post-load false-positive pages 0.
  - subset check: `exact_candidates ⊆ binary_fuse_candidates` passed.
- Validate CLI smoke commands: passed for `exact_marker_page` and `binary_fuse_page`.
- Page checksum smoke command: passed for MessagePack+zstd sealed page, checksum length 64, `mge validate --json` ok.
- Structured JSON remember smoke command: passed, exported value type `structured`.
- Typed reference/timestamp remember smoke command: passed, exported value types `reference` and `timestamp`.
- Source/link remember smoke command: passed, exported source and links retained.
- Link validation smoke command: passed for valid link and failed as expected for unknown link.
- Stats JSON smoke command: passed, `sealed_pages` and `current_index_kind` exported.
- Recall policy secret-reference opt-in smoke command: passed.
- Marker-overlap clusterer seal smoke command: passed.
- Smoke result after sealing:
  - hot cells: 0
  - sealed pages: 1-2 depending on smoke scenario
  - sealed cells: 1-2 depending on smoke scenario
  - index type: `exact_marker_page` or `binary_fuse_page` depending on smoke scenario
  - current index kind: `exact_marker_page` or `binary_fuse_page` depending on smoke scenario
  - current page codec: `json` or `messagepack` depending on smoke scenario
  - current compression: `none` or `zstd` depending on smoke scenario
