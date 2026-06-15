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
| v0.2 remaining | Closed | Benchmark foundation is ready; further core cleanup is benchmark-gated. |
| v0.3 SDK/MCP | In progress | Mandate 2 integration foundation is active: MCP-ready adapter and thin Python/TypeScript wrappers are present. |
| v0.4 security | In progress | Mandate 3 has threat model plus real session unlock and authenticated hot log/snapshot/sealed-page payload encryption; encrypted indexes/blind markers remain future work. |
| v0.5 safety/search | Partial foundation | Policy/capabilities exist; poisoning/conflict/vector reranking are not started. |

## Current Focus

- Build Mandate 3 security/encryption while keeping Mandate 1 core and Mandate 2 integration stable.
- Keep JSON out of internal runtime storage; use it only for explicit debug output and structured input parsing.
- Keep the implementation deterministic, local, compact, marker/page based, and ready for later security work.

## Mandate 1 Closure Status

Mandate 1 is closed on commit `d441ca1`.

Ready:

- Binary runtime storage layout is implemented and validated.
- L1 Hot RAM indexes and RAM-first durability/recovery are implemented.
- `MarkerGenome` is explicit while `MemoryCell.markers` remains the flattened compatibility/index view.
- Sealed pages, page catalog metadata, metadata pruning, decoded page cache, and runtime scoring cache are implemented.
- `ExactMarkerPageIndex` remains the default; `BinaryFusePageIndex` remains optional and benchmark-gated.
- `validate --deep` and `rebuild-indexes` work without rewriting sealed page payloads.
- CLI workflow and Rust core API are usable for init/open, remember, recall, seal, checkpoint, validate, and rebuild.
- `mge-synthetic-bench` and `mge-corpus-bench` are ready for generated and local real-workload measurements.

Latest verification:

- `cargo fmt`: passed.
- `cargo test`: passed, 115 tests total.
- CLI smoke: passed.
- Synthetic benchmark smoke: passed with exact/BinaryFuse subset check true.
- Generated corpus profiles `small`, `medium`, `code-heavy`, `docs-heavy`, and `mixed`: passed.
- Repo-local corpus: passed on 38 files, 622884 bytes, 794 chunks, subset check true.

Benchmark decision summary:

- L1 Hot RAM is not a bottleneck.
- Sealed repeated recall is stable enough for developer-ready core.
- BinaryFuse sometimes helps, but not consistently enough to replace exact.
- Page decode share does not justify custom codec work now.
- Scoring/filtering is the only recurring bottleneck signal, but it needs a larger user-provided corpus before further cleanup.

Closure benchmark summary, exact baseline:

- Generated profiles passed: `small`, `medium`, `code-heavy`, `docs-heavy`, `mixed`.
- Generated repeated focused recall range: 1596-2713 us; hot focused range: 261-420 us.
- Generated page decode share range: 8-14%; scoring/filtering share range: 15-31%; ContextPacket share range: 4-6%.
- Repo-local corpus: 38 files, 622884 bytes, 794 chunks, repeated focused 9647 us, repeated broad 15867 us, repeated full-scope 7804 us.
- Repo-local shares: page decode 13%, scoring/filtering 23%, ContextPacket 2%; repeated locality benefit 39%.
- Repo-local BinaryFuse repeated focused: 8778 us vs exact 9647 us, but BinaryFuse remains optional because profile results are mixed.

Remaining closure gaps:

- Run a larger user-provided corpus if available.
- Keep `docs/BENCHMARKS.md` and `docs/BENCHMARKS.ru.md` current as benchmark output evolves.
- Keep minimal Rust API examples in `examples/basic_usage.rs`.
- Optional scoring/filtering cleanup only after a larger real corpus confirms the same bottleneck.

## Mandate 2 Progress

Active mandate: Agent Integration / MCP / SDK.

Done in Mandate 2 foundation:

- Stable Rust integration boundary reviewed; `MemoryEngine` already covers init/open, remember, recall, seal, checkpoint, stats, validate, validate_deep, rebuild indexes, and Markdown export.
- Added `mge-mcp-server` as a local JSON-RPC stdin/stdout adapter with tools for remember, recall, seal, checkpoint, stats, validate, rebuild indexes, and Markdown export.
- Stabilized MCP tool contracts with `protocol_version = mge-jsonrpc-1`, `integration_schema_version = 1`, `mge_schema`, structured errors, adapter-level recall `context`, and golden JSON contract fixtures.
- Added thin Python SDK wrapper in `sdk/python`.
- Added thin TypeScript SDK wrapper in `sdk/typescript`.
- Added typed Python and TypeScript SDK contract surfaces plus structured protocol error helpers.
- Added runnable Python and TypeScript basic examples.
- Added agent workflow examples for CLI, MCP-style JSON-RPC, Python, and TypeScript.
- Added integration docs: `docs/INTEGRATION.md`, `docs/MCP.md`, and `docs/SDK.md`, plus Russian mirrors.
- Added local developer packaging metadata for the thin SDK wrappers: `sdk/python/pyproject.toml`, `sdk/typescript/package.json`, and `sdk/typescript/tsconfig.json`.
- Hardened MCP adapter tests for malformed JSON, unknown tool names, missing required args, invalid recall modes, `full_scope` without scope, invalid store paths, and explicit Markdown export paths.
- Documented local SDK installation/use, MCP adapter commands, JSON-RPC workflow examples, troubleshooting, and schema versioning notes.
- Added runnable local agent host examples for Rust/CLI, Python SDK, TypeScript SDK, and MCP JSON-RPC transcript replay.
- Added compatibility smoke coverage for a single-process MCP multi-call session, relative and absolute store paths, Python agent host workflow, TypeScript agent host workflow where supported, and Rust CLI host workflow.

Mandate 2 constraints preserved:

- Rust remains the core.
- Python and TypeScript do not duplicate memory logic; they delegate to the Rust CLI.
- JSON is protocol/debug output only, not runtime storage.
- Storage layout, filters, page codec, recall modes, `MarkerGenome`, and `MemoryCell.markers` are unchanged.

Next Mandate 2 step:

- Keep the current versioned JSON-RPC stdin/stdout adapter as the main local MCP-ready surface; defer any full external MCP SDK dependency until a concrete host integration needs it.
- Next useful package: run the JSON-RPC adapter against a real local agent runner/host harness or add package-level release checks for the thin SDKs, without changing the core.

## Mandate 2 Closure Status

Mandate 2 is ready to close as a local agent integration-ready layer.

Ready:

- MCP adapter: `mge-mcp-server` is the primary local MCP-ready surface using JSON-RPC over stdin/stdout.
- Protocol contract: `protocol_version = mge-jsonrpc-1`, `integration_schema_version = 1`.
- Tool schema: `mge_schema` exposes remember, recall, seal, checkpoint, stats, validate, rebuild indexes, and Markdown export contracts.
- Error model: structured JSON-RPC errors include `code`, `message`, `tool_name`, `recoverable`, protocol/schema versions, and `details.error_kind`.
- Python SDK: thin wrapper over the Rust CLI with typed request/response surfaces, `py.typed`, local README, import smoke, agent-host smoke, and wheel metadata check passing without publishing.
- TypeScript SDK: thin wrapper over the Rust CLI with typed interfaces, `types`/typed export metadata, local README, runtime smoke, and agent-host smoke. `tsc` remains optional and was not available in the current environment.
- Examples: CLI/Rust host, Python host, TypeScript host, MCP JSONL session transcript, and agent workflow docs are present.
- Docs: README, Integration, MCP, SDK, and project status documents describe the local host pattern, lifecycle, recall modes, schema versioning, and limitations.

Latest Mandate 2 verification:

- `cargo fmt`: passed.
- `cargo test`: passed, 124 tests total.
- CLI smoke: passed.
- MCP workflow smoke: passed.
- Python SDK smoke: passed.
- TypeScript SDK runtime smoke: passed.
- Rust example smoke: passed.
- Python wheel metadata check: passed with `python -m pip wheel --no-deps --no-build-isolation sdk/python`.
- TypeScript compile check: skipped because `tsc` was not installed.

Intentionally not implemented:

- No external MCP SDK dependency.
- No package publishing.
- No PyO3/maturin Python native binding.
- No UI, encryption, vector DB, new filters, storage layout changes, codec changes, or recall semantic changes.

Known limitations:

- The MCP adapter is MCP-ready JSON-RPC stdin/stdout, not a full external MCP SDK transport/server implementation.
- Python and TypeScript SDKs shell out to the Rust CLI; they are integration wrappers, not alternate engines.
- TypeScript source execution requires a Node runtime that supports TypeScript stripping, or a local TypeScript toolchain chosen by the host.

Recommended next mandate:

- Mandate 3 should be an explicit product decision: either real host integration with a chosen agent runner/MCP host, or security/session work. Do not restart core optimization without a benchmark or integration blocker.

## Mandate 3 Security Status

Active mandate: Security / Encryption.

Done in Mandate 3 foundation:

- Added `docs/SECURITY.md` and `docs/SECURITY.ru.md`.
- Documented the threat model: protected assets, in-scope threats, out-of-scope threats, metadata policy, session model, validation/rebuild behavior, and implementation gates.
- Documented the encryption design direction before implementation.
- Reconfirmed in the foundation phase that `NoSecurity` is pass-through, not fake encryption.
- Reconfirmed that JSON/JSONL remain protocol/debug/benchmark output only, not runtime storage.
- Added manifest-level `SecurityMode` and `SecurityConfig`.
- Added `mge init --encrypted` as an opt-in encrypted-mode store marker.
- Added `mge config security` for manifest-level security status without opening payloads.
- Added locked-store errors for encrypted-mode payload operations without session unlock.
- Added MCP `store_locked` structured error classification.

Done in Mandate 3 hot-encryption package:

- Added crypto dependencies: `chacha20poly1305`, `argon2`, `rand`, and `zeroize`.
- Added session unlock through passphrase environment variables: `--passphrase-env MGE_PASSPHRASE`.
- Added manifest security metadata for KDF params, AEAD scheme/version, salt, and encrypted key-check block without storing passphrases or raw keys.
- Added runtime-only `SessionKey` with zeroization on drop.
- Encrypted/authenticated hot record payloads in `hot/hot.mgl` for stores initialized with key metadata.
- Encrypted/authenticated checkpoint payloads in `hot/snapshot.mgs`.
- Preserved hot recovery behavior: valid encrypted records replay after snapshot offset, and a corrupted/truncated final encrypted frame is discarded without destroying earlier valid hot memory.
- Added CLI passphrase-env support for init, remember, recall, seal, checkpoint, inspect, validate, rebuild-indexes, stats, and export.
- Added MCP optional `passphrase_env` support and `auth_failed` structured error classification.
- Added Python SDK `passphrase_env` and TypeScript SDK `passphraseEnv` pass-through.

Done in Mandate 3 sealed-page encryption package:

- Encrypted/authenticated sealed page payloads in `pages/*.mgp` for stores initialized with key metadata.
- Preserved readable page frame headers for file kind/version/codec/payload length/checksum handling.
- Added encrypted page codec ids for MessagePack and MessagePack+zstd page payloads without changing page storage layout or the `CandidatePageIndex` API.
- Kept marker dictionary, candidate indexes, page catalog summaries, and Markdown export plaintext by design in this package.
- Sealed recall decrypts page payloads only after session unlock; missing unlock returns `store_locked`, wrong key or corrupted AEAD payload returns `auth_failed`.
- `validate --deep` and `rebuild-indexes` read encrypted pages with the same session unlock path and do not silently skip encrypted pages.
- MCP, Python SDK, and TypeScript SDK encrypted sealed recall smokes are covered through passphrase environment variables.

Done in Mandate 3 metadata/index risk memo:

- Added an encrypted metadata and index risk model to `docs/SECURITY.md` and `docs/SECURITY.ru.md`.
- Documented exactly what remains plaintext: manifest safe metadata, marker dictionary, catalog summaries, candidate indexes, encoded sizes, marker/scope/kind/status/sensitivity/trust summaries, and explicit Markdown export.
- Compared current plaintext metadata mode, hashed marker dictionary, blind marker indexes, encrypted dictionary with plaintext derived index IDs, and fully encrypted metadata.
- Recommended keeping payload-encrypted mode as the default encrypted mode and deferring blind marker mode until a prototype proves correctness, rebuild behavior, and benchmark impact.
- Reconfirmed that `CandidatePageIndex`, `MarkerGenome`, recall modes, storage layout, Exact/BinaryFuse strategy, and filter minimalism must remain stable.

Security design decisions:

- Protect payload bytes first: `hot/hot.mgl`, `hot/snapshot.mgs`, and `pages/*.mgp`.
- Keep binary file headers readable for file kind/version/codec/payload length/integrity handling.
- Allow selected catalog/index metadata to remain plaintext initially for deterministic recall and validation, with risks documented.
- Do not silently fallback from encrypted mode to plaintext.
- Existing unencrypted stores must continue to work.
- Encrypted store conversion must be an explicit future operation, not a silent config flip.
- Use well-known Rust crypto crates when implementation starts; do not write custom crypto.

Crypto dependencies in use:

- AEAD: `chacha20poly1305` with XChaCha20-Poly1305.
- KDF: `argon2`.
- Random salt/nonce generation: `rand`.
- Memory hygiene: `zeroize`.

Current limitations:

- Hot storage payloads and sealed page payloads are encrypted for encrypted stores with key metadata: `hot/hot.mgl`, `hot/snapshot.mgs`, and `pages/*.mgp`.
- No blind marker indexes or encrypted indexes yet.
- Markdown export remains plaintext by design.
- `mge init --encrypted` without `--passphrase-env` still creates a locked encrypted-mode marker/config state, but payload operations remain locked because no key metadata exists.

Latest Mandate 3 verification:

- `cargo fmt`: passed.
- `cargo test`: passed, 135 tests total.
- CLI unencrypted smoke: passed.
- CLI encrypted smoke: passed, including hot log/snapshot/page plaintext absence and wrong-key failure.
- Encrypted reopen sealed recall smoke: passed.
- Encrypted validate/rebuild smoke: passed.
- MCP encrypted sealed recall and wrong-key smoke: passed.
- Python SDK encrypted sealed recall smoke: passed.
- TypeScript SDK encrypted sealed recall smoke: passed.
- Rust example smoke: passed.

Next Mandate 3 step:

- If metadata privacy becomes a hard requirement, prototype Phase 1 keyed marker fingerprints for encrypted stores. Do not implement blind indexes directly without benchmark and migration evidence.

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
- Keyword tokenization now has an ASCII fast path for common text/code corpus data, reducing temporary string allocation while preserving the same normalized token output.
- Marker value canonicalization now has an ASCII fast path and avoids an extra trim/copy pass for common marker/query/value text.
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
- Decoded sealed page scoring data is now lazy per cell: page cache stores runtime cache slots and builds token/canonical scoring data only for cells that pass cheap metadata/filter checks and actually need text scoring.
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
- `HotMemoryLayer` also keeps runtime-only derived scoring data per hot cell for focused/broad hot recall. This cache is rebuilt from hot log/snapshot recovery, updated on `remember`, and cleared on `seal`; it is not written to `hot/hot.mgl`.
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
- Corpus benchmark tool added as `cargo run -p mge-cli --bin mge-corpus-bench`.
- Corpus benchmark imports only local text/code corpus files with explicit max-files/max-bytes limits, skips symlinks and common generated directories, never executes corpus files, writes stores only under `--store-root`, compares exact vs Binary Fuse, and reports cold vs repeated focused/broad/full-scope recall.
- Corpus benchmark comparison output now includes direct exact-vs-Binary-Fuse summaries for hot/cold/repeated focused/broad/full-scope recall, repeated timing breakdowns, ContextPacket build time, storage size, average encoded page size, and average cells per page.
- Corpus benchmark comparison output now also includes repeated recall locality summaries and top timing bottlenecks for hot focused, sealed cold focused, sealed repeated focused, and sealed repeated broad workloads.
- Recall debug and corpus benchmark output now report sealed cells skipped before token scoring and sealed cells that actually reached token scoring.
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
- `cargo test`: passed, 115 tests total (13 CLI unit tests + 9 CLI integration tests + 2 core unit tests + 91 core integration tests).
- Validation/rebuild tests: passed for clean deep validation, corrupted/mismatched catalog summaries, missing exact index restore, active Binary Fuse index restore, recall after rebuild, hot memory untouched, and no JSON/JSONL runtime storage regression.
- Recall modes tests: passed for focused top result, broad expanded output, full-scope scoped output, full-scope missing-scope error, default status filtering, and no JSON/JSONL runtime storage regression.
- Recall modes CLI smoke command: passed for `--mode broad`, `--mode full-scope --scope`, and full-scope missing-scope failure.
- Benchmark harness integration smoke test: passed for exact + Binary Fuse modes and required metrics.
- Corpus benchmark integration smoke test: passed for local corpus import, exact + Binary Fuse modes, cold/repeated recall metrics, validation/rebuild checks, subset check, and no JSON/JSONL runtime storage regression.
- Latest corpus benchmark smoke command: passed on 24 local files, 246015 imported bytes, 225 chunks.
  - exact_marker_page: cold focused avg 33769 us, repeated focused avg 20853 us, repeated broad avg 3761 us, page decode avg 2481 us, scoring cache build avg 7652 us, cell filtering avg 7943 us.
  - binary_fuse_page: repeated focused avg 20497 us, repeated broad avg 3483 us, page decode avg 2476 us, scoring cache build avg 7648 us, cell filtering avg 7919 us.
  - subset check: focused exact candidates subset of binary_fuse candidates passed.
- Latest canonicalization benchmark smoke command: passed on 24 local files, 250192 imported bytes, 229 chunks.
  - exact_marker_page: cold focused avg 30434 us, repeated focused avg 18186 us, repeated broad avg 3729 us, page decode avg 2518 us, scoring cache build avg 6149 us, cell filtering avg 6770 us.
  - binary_fuse_page: repeated focused avg 17851 us, repeated broad avg 3395 us, page decode avg 2529 us, scoring cache build avg 6040 us, cell filtering avg 6839 us.
  - subset check: focused exact candidates subset of binary_fuse candidates passed.
- Safe generated corpus benchmark command: passed on 36 generated files, 748536 imported bytes, 960 chunks, avg chunk 774 bytes, avg markers per cell 4, 6 scopes, 6 extensions.
  - storage: exact 2567669 bytes, binary_fuse 2584948 bytes, avg encoded page 54594 bytes, avg cells/page 43.64.
  - hot recall avg: focused exact 44978 us / binary 44729 us, broad exact 45374 us / binary 45176 us, full-scope exact 4218 us / binary 4085 us.
  - sealed cold avg: focused exact 69505 us / binary 69090 us, broad exact 70348 us / binary 69116 us, full-scope exact 21184 us / binary 21303 us.
  - sealed repeated avg: focused exact 20894 us / binary 20718 us, broad exact 6439 us / binary 6507 us, full-scope exact 6655 us / binary 6627 us.
  - repeated focused exact timing: total 20894 us, page decode 2953 us, scoring cache build 6125 us, cell filtering 7715 us, ContextPacket build 530 us.
  - repeated broad exact timing: total 6439 us, page decode 0 us, scoring cache build 0 us, cell filtering 2475 us, ContextPacket build 505 us.
  - subset check: focused exact candidates subset of binary_fuse candidates passed.
- Hot scoring cache benchmark command: passed on the same generated corpus shape, 36 files, 748536 imported bytes, 960 chunks.
  - hot focused exact: 44978 us -> 4082 us.
  - hot broad exact: 45374 us -> 4718 us.
  - hot full-scope exact: 4218 us -> 4047 us.
  - sealed repeated focused exact stayed comparable: 20894 us -> 21144 us.
  - sealed repeated broad exact stayed comparable: 6439 us -> 6669 us.
  - subset check: focused exact candidates subset of binary_fuse candidates passed.
- Lazy sealed scoring cache benchmark command: passed on the same generated corpus shape, 36 files, 748536 imported bytes, 960 chunks.
  - sealed repeated focused exact: 21252 us -> 15859 us.
  - sealed repeated focused binary_fuse: 21774 us -> 15546 us.
  - sealed repeated broad exact: 6642 us -> 6405 us.
  - repeated focused exact timing after: total 15859 us, page decode 2976 us, scoring cache build 6221 us, cell filtering 8806 us, ContextPacket build 518 us.
  - subset check: focused exact candidates subset of binary_fuse candidates passed.
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
- Corpus benchmark package: passed.
  - Added `mge-corpus-bench` for local real-workload measurement before any custom codec or partial-decode decision.
  - Supported corpus extensions: `.txt`, `.md`, `.rs`, `.toml`, `.json` as text import only, `.py`, `.ts`, `.js`.
  - Safety: does not download, does not execute corpus files, skips symlinks, skips common generated dirs, respects max-files/max-bytes/max-file-bytes, and requires a fresh `--store-root` outside the corpus root.
  - Metrics include files/bytes/chunks, avg chunk bytes, avg markers per cell, scopes/extensions, remember/seal/storage/page size, hot recall, sealed cold recall, sealed repeated recall, cache hits/misses, page read/decode, scoring cache build, filtering, reranking, ContextPacket build, cells decoded/filtered/ranked, returned items, validation/rebuild status, and exact-vs-Binary-Fuse subset check.
  - Local repo corpus smoke: 24 files, 239826 bytes, 220 chunks, avg chunk 1089 bytes, 6 scopes, 3 extensions.
  - exact repeated focused: total 25033 us, page decode 2528 us, scoring cache build 9862 us, cell filtering 9786 us, ContextPacket build 322 us.
  - exact cold focused: total 40544 us, page decode 9084 us, cell filtering 28046 us, ContextPacket build 323 us.
  - binary repeated focused: total 24266 us, page decode 2442 us, scoring cache build 9741 us, cell filtering 9653 us.
  - repeated broad loaded about 3 pages / 78 cells on this limited repo corpus; metadata pruning kept broad recall small.
  - Current real-ish bottleneck: cell filtering/scoring and scoring-cache construction dominate repeated focused recall; cold focused recall is dominated by filtering plus page decode.
- ASCII tokenizer hot-path package: passed.
  - `tokenize_keywords` now uses a byte-level ASCII fast path for common text/code corpus data and keeps the Unicode fallback compatible with the previous implementation.
  - Temporary token strings are allocated only after stopword/singularization/dedup checks where possible.
  - Corpus before/after on comparable 24-file repo corpus smoke:
    - before exact repeated focused: total 26480 us, scoring cache build 10116 us, cell filtering 10619 us.
    - after exact repeated focused: total 20853 us, scoring cache build 7652 us, cell filtering 7943 us.
    - after binary repeated focused: total 20497 us, scoring cache build 7648 us, cell filtering 7919 us.
  - Recall/storage architecture unchanged; no new filters, codec, storage layout, SDK, UI, vector DB, or JSON runtime storage added.
- ASCII canonicalization hot-path package: passed.
  - `canonicalize_marker_value` now uses byte-level ASCII canonicalization for common marker/query/value text and keeps the Unicode fallback compatible.
  - The function avoids the previous trailing `trim_matches('_').to_string()` copy by skipping leading separators and popping a single trailing separator.
  - Corpus after tokenizer vs after canonicalization on comparable 24-file repo corpus smoke:
    - exact repeated focused: 20853 us -> 18186 us.
    - exact scoring cache build: 7652 us -> 6149 us.
    - exact cell filtering: 7943 us -> 6770 us.
    - binary repeated focused: 20497 us -> 17851 us.
  - Recall/storage architecture unchanged; no new filters, codec, storage layout, SDK, UI, vector DB, or JSON runtime storage added.
- Corpus benchmark summary package: passed.
  - Added a decision-ready `comparison` summary for exact vs BinaryFuse across hot/cold/repeated recall and focused/broad/full-scope modes.
  - Repeated sealed recall summary now exposes total recall, query marker extraction, hot lookup, index lookup, page read/load, page decode, scoring cache build, cell filtering, reranking, and ContextPacket build by recall mode.
  - Existing detailed `modes` output remains unchanged; this is report shape only, not a retrieval/storage behavior change.
- Corpus benchmark bottleneck summary package: passed.
  - Added `sealed_repeated_locality` with decoded page cache hits/misses, scoring cache hits/misses, pages loaded/pruned, cells decoded/ranked, and returned items.
  - Added `top_bottlenecks_avg_micros` for the main hot/cold/repeated workloads, sorted by average component time.
  - This is report shape only; recall, storage, indexes, filters, and ContextPacket output are unchanged.
- Sealed token-scoring counter package: passed.
  - `ContextDebugInfo` now includes `sealed_cells_skipped_before_token_scoring` and `sealed_cells_token_scored`.
  - `mge-corpus-bench` records those counters in per-mode output and exact-vs-BinaryFuse locality summaries.
  - This is debug/reporting only; storage layout, recall results, validation, rebuild-indexes, and caches are unchanged.
- Sealed token overlap cache package: passed.
  - `CachedCellScoringData` now builds a runtime-only token set for longer value token lists and scoring checks the shorter query side first.
  - The cache is still in RAM only; `.mgp` files, MessagePack page codec, storage layout, indexes, validation, rebuild-indexes, and ContextPacket output are unchanged.
  - Generated safe corpus before/after on the same 18 imported files / 1980 chunks smoke shape:
    - exact repeated focused: 22513 -> 20201 us.
    - exact repeated broad: 12280 -> 7970 us.
    - binary_fuse repeated focused: 24127 -> 18740 us.
    - binary_fuse repeated broad: 14459 -> 7151 us.
    - exact focused cell filtering: 12212 -> 9134 us; scoring cache build rose 6742 -> 8031 us because the per-cell cached token set is built once.
  - `sealed_cells_skipped_before_token_scoring` showed only 43 cells skipped vs 484 token-scored on this corpus, so extra metadata/filter pruning was not the right next optimization.
- Real-workload corpus benchmark readiness package: passed.
  - `mge-corpus-bench` now accepts the real-workload command shape with visible `--corpus` alias, `--profile small|medium|code-heavy|docs-heavy|mixed`, `--chunk-lines`, and `--seed` while preserving existing `--corpus-root` and `--chunk-bytes`.
  - Added `--generated` diverse local corpus mode with markdown notes, Rust/Python/TypeScript/JS/config/long-text/fragment/noise files under `--store-root/generated-corpus`.
  - Added recommendation JSON with machine-readable bottleneck signals and human-readable summary lines: hot bottleneck, sealed cold/repeated bottleneck, BinaryFuse usefulness, page decode/scoring/filtering/ContextPacket shares, repeated locality benefit, and suggested next core step.
  - Safety remains local-only: no download, no corpus execution, symlinks skipped, unsupported binary extensions skipped before read, corpus files not modified, stores written under `--store-root`.
  - Tests cover real local directory mode through `--corpus`, generated small and medium profiles, binary extension skip, optional symlink skip, recommendation output, exact subset BinaryFuse check, and rejection of nested `--store-root` inside corpus.
- L1 Hot RAM scoring cache package: passed.
  - Hot focused/broad recall now reuses `CachedCellScoringData` built at `remember`/hot recovery time instead of tokenizing hot cell value/subject on every recall.
  - Runtime scoring data is cleared with `HotMemoryLayer::clear()` during seal and is not persisted into `hot/hot.mgl` or snapshots as a separate storage format.
  - Correctness tests passed for cache build/clear, hot recall, reopen recovery, seal clearing, full-scope behavior, and no JSON runtime storage regression.
- Lazy sealed page scoring cache package: passed.
  - `PageScoringCache` now stores lazy per-cell `OnceLock<CachedCellScoringData>` slots instead of tokenizing every cell on the page when the page enters scoring cache.
  - Sealed focused/broad recall now runs cheap filter checks before building token/canonical scoring data and uses prechecked scoring to avoid duplicate filter work.
  - Runtime cache remains in RAM only; `.mgp` files, page codec, storage layout, validation, and rebuild-indexes are unchanged.
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
