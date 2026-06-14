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
- Do not add Bloom, Counting Bloom, Cuckoo, XOR, Ribbon, or new filter families without benchmark proof and a stable `CandidatePageIndex` boundary.

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
  - explicit `MarkerGenome` model for structured marker DNA;
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
- `MemoryCell` now carries explicit `MarkerGenome` plus a flattened `markers: Vec<MarkerId>` runtime/index view for backward compatibility.
- `MarkerGenome` separates scope, kind, status, trust, sensitivity, subject, value/domain, and custom marker IDs.
- `MarkerGenome` exposes all marker IDs, system marker IDs, custom marker IDs, key system markers, page-summary markers, and a deterministic fingerprint.
- L1 Hot RAM indexes, page grouping, page summaries, recall filtering/scoring, markdown export, and validation now use genome-compatible marker access while preserving old vec-style records.
- Recall hot path now uses borrowed `MemoryValue` text where possible, static stopword lookup for tokenization, cheaper scoped filtering, and single-pass ContextPacket assembly to reduce allocations and temporary rebuilds.
- Engine recall ranking now uses lightweight ranked cell handles for hot/sealed candidates, so `MemoryCell` is not cloned for every scored candidate before reranking.
- ContextPacket output still allocates only the returned items; marker string vectors and content strings are built after final ranking and dedupe.
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
- Page catalog entries now also record lightweight pre-decode summaries: scope marker summary, kind marker summary, direct status summary, direct sensitivity summary, trust summary, and encoded page size.
- Sealed recall now has a small bounded decoded-page cache for immutable sealed pages; validation and rebuild paths intentionally bypass it and read page files directly.
- Decoded sealed pages can now keep runtime-only scoring data for repeated recall: per-cell value tokens, canonical value text, and subject tokens are cached in RAM after a decoded page cache hit. This does not change `.mgp` storage, ContextPacket output, validation, or rebuild behavior.
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
- `HotMemoryLayer` added as the L1 RAM layer for mutable hot memory.
- `HotMemoryLayer` keeps exact in-memory indexes:
  - `cells_by_id: CellId -> MemoryCell`
  - `marker_to_cells: MarkerId -> Vec<CellId>`
  - `scope_to_cells: ScopeId -> Vec<CellId>`
  - `kind_to_cells: KindId -> Vec<CellId>`
  - `status_to_cells: Status -> Vec<CellId>`
- `MemoryEngine::open_at` / `init_at` now load `hot/hot.mgl` once and rebuild the L1 RAM layer from the durable binary log.
- Hot memory now follows a RAM-first model: `remember` updates `HotMemoryLayer` immediately and queues the cell for hot-log persistence; `recall` does not wait for disk.
- Pending hot events flush through the queued persistence path at `checkpoint`, `seal`, and normal engine drop boundaries.
- Durability policy is configurable as `fast`, `balanced` default, or `safe`.
- `mge checkpoint` writes optional binary `hot/snapshot.mgs` after flushing pending hot events.
- Recovery can load `hot/snapshot.mgs`, replay `hot/hot.mgl` after the snapshot offset, and truncate a corrupted final hot record without losing earlier valid frames.
- Hot recall now gets candidates from `HotMemoryLayer` using marker/scope/kind/status indexes before the existing filtering/scoring path.
- `seal` now flushes pending hot events, uses current hot cells from `HotMemoryLayer`, archives/clears `hot/hot.mgl`, removes stale `hot/snapshot.mgs`, and clears RAM indexes after a successful seal.
- `stats` and exports use the current RAM hot view where safe; `validate` still reads durable hot storage to check recovery/integrity.
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
- `ContextDebugInfo` now includes detailed recall timing breakdown: query marker extraction, hot memory lookup, candidate page index lookup, page file read/load, page decode, cell filtering, reranking, ContextPacket build, and total recall.
- `IndexKind` added with the implemented `exact_marker_page` kind.
- Manifest, page catalog, stats, and exact index files now carry index kind metadata.
- `CandidatePageIndex` now exposes `kind()` and query statistics for static index implementations.
- `BinaryFusePageIndex` added as opt-in `binary_fuse_page` while `ExactMarkerPageIndex` remains the default.
- Binary Fuse page filters are real `xorf::BinaryFuse16` filters built per sealed page from `marker_summary`; no fake Binary Fuse implementation was added.
- CLI `init` and `config set` now support `--index-kind exact_marker_page|binary_fuse_page`.
- Changing `index_kind` rebuilds only the candidate index from existing sealed pages; sealed page files are not rewritten.
- Recall debug now reports index kind, page filters scanned, candidate pages returned, loaded pages, sealed cells scanned, and post-load false-positive candidate pages.
- Recall debug now also reports pages considered, pruned candidate pages, pages pruned by metadata, cells decoded, cells filtered, and cells ranked.
- Tests now assert `exact_candidates ⊆ binary_fuse_candidates` for the same sealed pages and verify index-kind switching without page rewrites.
- Synthetic benchmark tool added as `cargo run -p mge-cli --bin mge-synthetic-bench`.
- Synthetic benchmark compares `exact_marker_page` and opt-in `binary_fuse_page` on identical generated stores and checks `exact_candidates ⊆ binary_fuse_candidates`.
- Synthetic benchmark harness now reports remember, seal, hot focused/broad/full-scope recall before seal, sealed focused/broad/full-scope recall after seal, index lookup, page decode, ContextPacket build, candidate pages, pages pruned by metadata, hot total/candidate/scanned cells, cells scanned, returned items, storage size, seal hot-clear correctness, and p50/p95/avg metrics where practical.
- Index/filter minimalism is documented: L1 Hot RAM uses exact mutable indexes only; L2 uses `ExactMarkerPageIndex` by default and `BinaryFusePageIndex` as the only optional static probabilistic filter backend.
- Hot log archiving now uses unique archive names when multiple seals happen within the same timestamp window.
- `ValidationReport` and CLI `validate` added as read-only consistency checks for manifest, catalog, pages, page checksums, marker references, and candidate index coverage.
- `validate_deep()` and CLI `validate --deep` added for stricter sealed page/catalog/index checks.
- Store validation now checks cell links for unknown targets and self-links.
- Store validation now warns about orphan page files and unknown unmanaged index files.
- Deep validation treats orphan `pages/*.mgp`, missing page catalog, and missing active candidate index files as errors.
- Store validation now checks marker dictionary forward/reverse consistency, canonical markers, and `next_id`.
- `rebuild_catalog_and_indexes()` added as safe rebuild tooling for L2 sealed memory metadata.
- CLI `rebuild-indexes` rebuilds `indexes/page_index.mgi`, `indexes/marker_index.mgi`, and active `indexes/fuse_index.mgi` when `binary_fuse_page` is configured.
- Catalog/index rebuild reads existing `pages/*.mgp` as source of truth, decodes binary page frames through their header codec, atomically writes rebuilt `.mgi` files, and does not rewrite sealed page payloads, memory cells, or hot memory.
- Seal/config index rebuild paths now keep `ExactMarkerPageIndex` as the reliable baseline while `BinaryFusePageIndex` remains opt-in.
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
- Do not expand the project into a filter zoo. New filter/index families require benchmark evidence, correctness proof, and no public API sprawl.

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
cargo run -p mge-cli -- validate --deep
cargo run -p mge-cli -- rebuild-indexes
cargo run -p mge-cli -- export
cargo run -p mge-cli -- config set durability safe
cargo run -p mge-cli -- checkpoint
cargo run -p mge-cli -- init --index-kind binary_fuse_page
cargo run -p mge-cli -- config set --index-kind binary_fuse_page
cargo run -p mge-cli --bin mge-synthetic-bench -- --cells 1200 --pages 120 --scopes 16 --markers-per-cell 5 --marker-groups 12 --targeted-queries 6 --noise-queries 3 --repeats 5 --seed 1
```

## Verification Status

- `cargo fmt`: passed.
- `cargo test`: passed, 109 tests total (13 CLI unit tests + 5 CLI integration tests + 1 core unit test + 90 core integration tests).
- Validation/rebuild tests: passed for clean deep validation, corrupted/mismatched catalog summaries, missing exact index restore, active Binary Fuse index restore, recall after rebuild, hot memory untouched, and no JSON/JSONL runtime storage regression.
- Recall modes tests: passed for focused top result, broad expanded output, full-scope scoped output, full-scope missing-scope error, default status filtering, and no JSON/JSONL runtime storage regression.
- Recall modes CLI smoke command: passed for `--mode broad`, `--mode full-scope --scope`, and full-scope missing-scope failure.
- Benchmark harness integration smoke test: passed for exact + Binary Fuse modes and required metrics.
- Benchmark harness CLI smoke command: passed.
  - config: 120 cells, 12 sealed pages, 4 logical scopes, 5 markers per cell, 4 marker groups, 4 targeted queries, 2 noise queries, 3 repeats, seed 7.
  - exact_marker_page: remember avg 8367 us, seal avg 61834 us, focused recall avg 5270 us, broad recall avg 12575 us, full-scope recall avg 1764 us, index lookup avg 1 us, page decode avg 391 us, ContextPacket build avg 944 us, storage 108585 bytes.
  - binary_fuse_page: remember avg 8040 us, seal avg 54785 us, focused recall avg 5312 us, broad recall avg 12805 us, full-scope recall avg 1871 us, index lookup avg 1 us, page decode avg 395 us, ContextPacket build avg 952 us, storage 112749 bytes.
  - subset check: focused exact candidates subset of binary_fuse candidates passed.
- Recall detailed breakdown package: passed.
  - Safe hot-path optimization: scoring now reuses precomputed query marker/token sets, canonical query/scope values, and effective recall policy instead of rebuilding them per cell.
  - Safe page pruning: candidate pages whose catalog `marker_summary` proves no query marker match are skipped before page decode.
  - Benchmark before/after on the same smoke config:
    - before exact focused/broad/full-scope avg: 5856 / 13152 / 1991 us.
    - after exact focused/broad/full-scope avg: 5102 / 12722 / 2019 us.
    - before binary_fuse focused/broad/full-scope avg: 5327 / 13753 / 1913 us.
    - after binary_fuse focused/broad/full-scope avg: 5132 / 12333 / 2100 us.
  - Current broad bottlenecks: cell filtering and page decode dominate; index lookup is not the main cost on this dataset.
- Recall hot-path optimization package: passed.
  - Page-level prefiltering now uses catalog `marker_summary` for required scope/kind markers, query marker impossibility, status summary, and sensitivity summary before page decode when the summary is conclusive.
  - Cell filtering now rejects explicit-marker misses and scope-marker misses before scoring/token work.
  - Benchmark before/after on the same smoke config:
    - before exact focused/broad/full-scope avg: 5091 / 12503 / 1750 us.
    - after exact focused/broad/full-scope avg: 5835 / 7746 / 1788 us.
    - before binary_fuse focused/broad/full-scope avg: 5089 / 12617 / 1946 us.
    - after binary_fuse focused/broad/full-scope avg: 5290 / 7814 / 1970 us.
  - Broad cell filtering improved on the benchmark: exact 7094 -> 2427 us, binary_fuse 6908 -> 2407 us; broad ranked cells dropped from 90 to 30 while returned items stayed 20.
  - Page pruning smoke command: passed with pages considered 2, loaded 1, pruned 1, returned 1.
  - Remaining broad bottleneck: page decode is now the largest stable cost on this dataset; index lookup remains small.
- Sealed page metadata/catalog pruning package: passed.
  - Page catalog now stores lightweight pre-decode summaries for scope markers, kind markers, status, sensitivity, trust, and encoded page size.
  - Recall now prunes candidate pages by metadata before full page read/decode when the decision is deterministic: missing required scope/kind markers, missing explicit query markers, only disallowed statuses, or only disallowed sensitivities.
  - CLI smoke command passed with pages considered 2, loaded 1, pages pruned by metadata 1, returned 1.
  - Benchmark before/after on 240 cells, 24 sealed pages, 6 marker groups, 6 targeted + 2 noise queries, 3 repeats, seed 11:
    - exact broad avg: 17077 -> 12919 us; broad pages loaded avg: 21 -> 11; broad cells decoded avg: 210 -> 117; broad page decode avg: 7689 -> 4264 us.
    - binary_fuse broad avg: 16559 -> 12976 us; broad pages loaded avg: 21 -> 11; broad cells decoded avg: 210 -> 117; broad page decode avg: 7508 -> 4229 us.
    - focused exact remained page-limited at 11 loaded pages; full-scope remains scope-limited and correctness-preserving.
  - New tests cover explicit-marker metadata pruning, status-summary pruning, sensitivity-summary pruning, catalog metadata summaries, no false negatives for broad pruning, full-scope correctness, default status exclusion, and no JSON/JSONL runtime storage regression.
  - Remaining broad bottleneck: page decode and cell filtering still dominate when the candidate set genuinely overlaps; index lookup is still small.
- MarkerGenome package: passed.
  - Added explicit `MarkerGenome` core type without changing storage layout.
  - `remember` now builds marker IDs through `MarkerDictionary`, then stores structured `MarkerGenome` plus flattened marker IDs in `MemoryCell`.
  - Old named MessagePack `MemoryCell` records without `marker_genome` still deserialize and remain indexable through flattened `markers`.
  - Tests cover genome construction, system/custom separation, old vec-style compatibility, and existing hot/sealed/full-scope recall paths.
  - Benchmark smoke config: 120 cells, 12 pages, 4 scopes, 4 marker groups, 4 targeted queries, 2 noise queries, 3 repeats, seed 7.
  - exact_marker_page: hot focused avg 3933 us, hot lookup avg 166 us, sealed focused avg 6672 us, broad avg 6955 us, broad pages loaded avg 3, broad pages pruned by metadata avg 6.
  - binary_fuse_page: hot focused avg 3741 us, hot lookup avg 139 us, sealed focused avg 6746 us, broad avg 6785 us, broad pages loaded avg 3, broad pages pruned by metadata avg 6.
  - Benchmark subset check passed.
- Marker access allocation reduction package: passed.
  - Added borrowed/iterator marker accessors: `iter_all_marker_ids`, `iter_system_marker_ids`, `iter_custom_marker_ids`, `*_marker_id`, `flattened_marker_ids`, `iter_flattened_marker_ids`, `for_each_marker_id_for_indexing`, and `marker_overlap_count`.
  - Core hot paths no longer call `marker_ids_for_indexing()`; that method remains only as a compatibility helper.
  - Removed intermediate marker Vec rebuilds from L1 Hot RAM indexing, page grouping, page summaries, sealed recall scoring, ContextPacket marker export, validation, and Markdown export.
  - Benchmark before/after on the same 120 cells / 12 pages / seed 7 smoke config:
    - exact_marker_page: hot focused 3933 -> 2955 us; hot lookup 166 -> 145 us; sealed focused 6672 -> 5762 us; broad 6955 -> 5895 us.
    - binary_fuse_page: hot focused 3741 -> 2880 us; hot lookup 139 -> 139 us; sealed focused 6746 -> 5846 us; broad 6785 -> 5800 us.
    - after exact timing: focused filter 2214 us, broad filter 2263 us, focused context build 378 us, broad context build 379 us, focused decode 1448 us, broad decode 1487 us.
  - Benchmark subset check passed.
- Sealed scoring cache package: passed.
  - Added runtime-only `CachedCellScoringData` for decoded sealed pages: value tokens, canonical value text, and subject tokens are derived once for cached pages and reused by focused/broad scoring.
  - Cold page decode does not build this cache; cache data is attached only after a decoded page cache hit, so first-read latency and `.mgp` layout stay unchanged.
  - `score_cell_debug_with_cached_context()` avoids per-cell value/subject tokenization for cached sealed pages; fallback scoring is used only when no cached scoring entry exists.
  - Validate/deep-validate and rebuild-indexes still bypass decoded/scoring caches and read sealed page files directly.
  - Benchmark before/after on 1200 cells / 120 pages / 16 scopes / seed 23:
    - before exact focused/broad/full-scope avg: 34428 / 34707 / 11463 us.
    - after exact focused/broad/full-scope avg: 34090 / 34371 / 11335 us.
    - before binary_fuse focused/broad/full-scope avg: 35556 / 35917 / 12793 us.
    - after binary_fuse focused/broad/full-scope avg: 35174 / 36421 / 12875 us.
    - exact broad cell filtering improved 9119 -> 5825 us; exact broad page decode accounting rose 11634 -> 14710 us because scoring-cache construction is counted there on cache hits.
  - Benchmark smoke with 120 cells / 12 pages / seed 7 passed; subset check remained true.
  - Remaining bottleneck: MessagePack full-page decode/cache-miss cost and ContextPacket build on large returned sets; Binary Fuse is still not the dominant cost on this dataset.
- Sealed recall timing cleanup package: passed.
  - `page_decode_micros` now reports only page frame decode/decompress/page decode work.
  - `scoring_cache_build_micros` reports runtime scoring-cache construction separately.
  - Debug output now includes `decoded_page_cache_hits`, `decoded_page_cache_misses`, `scoring_cache_hits`, and `scoring_cache_misses`.
  - Synthetic benchmark JSON now exposes separate page decode, scoring cache build, cell filtering, reranking, ContextPacket build, and total recall timings.
  - Benchmark after cleanup on 1200 cells / 120 pages / 16 scopes / seed 23:
    - exact focused total 34199 us, page decode 11628 us, scoring cache build 3115 us, cell filtering 5851 us.
    - exact broad total 34257 us, page decode 11576 us, scoring cache build 3090 us, cell filtering 5848 us.
    - binary focused total 35181 us, page decode 11557 us, scoring cache build 3104 us, cell filtering 5798 us.
    - binary broad total 35393 us, page decode 11526 us, scoring cache build 3132 us, cell filtering 5777 us.
  - Total latency stayed materially the same; only accounting was cleaned up.
- L1 Hot RAM layer package: passed.
  - `HotMemoryLayer` indexes hot cells in RAM by cell id, marker id, canonical scope, kind, and status.
  - Correctness tests passed for immediate recall after remember, reopen recovery from `hot/hot.mgl`, hot clearing after seal, sealed recall after seal, full-scope hot+sealed recall, and default status exclusion before scoring.
  - Hot-only broad recall smoke before/after on 80 hot cells:
    - before: total 3970 us, hot lookup 2568 us, hot scanned 80.
    - after: total 1345 us, hot lookup 189 us, hot scanned 80, sealed index lookup 0 us.
  - Latest benchmark smoke config: 120 cells, 12 pages, 4 scopes, 4 markers per cell, 4 marker groups, 4 targeted queries, 2 noise queries, 3 repeats, seed 7.
  - exact_marker_page: hot focused avg 2890 us, hot lookup avg 144 us, hot candidates avg 30, sealed focused avg 11256 us, sealed page decode avg 4103 us, broad avg 11155 us, full-scope avg 1791 us, post-seal hot cells 0.
  - binary_fuse_page: hot focused avg 2888 us, hot lookup avg 140 us, hot candidates avg 30, sealed focused avg 11461 us, sealed page decode avg 4153 us, broad avg 11227 us, full-scope avg 1927 us, post-seal hot cells 0.
  - Benchmark subset check: focused exact candidates subset of binary_fuse candidates passed.
- RAM-first hot durability package: passed.
  - `remember` is RAM-first and queues hot persistence without waiting for `hot/hot.mgl`.
  - `checkpoint` and `seal` flush pending hot events first.
  - `mge config set durability fast|balanced|safe` and `mge checkpoint` are implemented.
  - `hot/snapshot.mgs` is optional binary checkpoint storage, not a new memory layer.
  - Crash recovery keeps valid hot frames and truncates only a corrupted final frame.
  - Tests passed for immediate RAM recall before log flush, checkpoint/reopen recovery, corrupted final frame recovery, safe/balanced flush paths, seal hot-log/snapshot clearing, checkpoint snapshot + replay, and no JSON runtime storage regression.
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
