# Architecture

[Русская версия](ARCHITECTURE.ru.md)

Memory Genome Engine is a Rust-first structured memory layer for LLM agents.

## Pipeline

```text
Agent / CLI / SDK
    -> Memory Engine API
    -> Marker Extractor
    -> Genome Encoder
    -> Hot Memory Layer
    -> Sealed Page Store
    -> Candidate Page Index
    -> Page Reader
    -> Reranker
    -> Context Packet Builder
    -> Agent receives minimal relevant memory
```

## Data Model

The atomic unit is `MemoryCell`. A cell stores a typed value, metadata, trust/status/sensitivity, marker IDs, optional source metadata, and links to other cells.

Values are not assumed to be raw text. v0.1 supports:

- text
- symbol
- number
- boolean
- timestamp
- reference
- structured JSON

Structured JSON marker extraction is deterministic and shallow: object keys and short scalar values become `tag:*` markers, capped to a small marker budget. The core does not use LLM-based extraction.

## Marker Genome

Each cell receives deterministic marker strings, canonicalized into marker IDs by `MarkerDictionary`.

Examples:

```text
kind:user_preference
scope:global
status:active
trust:user_confirmed
sensitivity:private
tag:technical
```

The dictionary persists stable integer IDs in `.memory-genome/dictionary/markers.mgd`.

## Storage Layers

Hot memory is mutable and append-only:

```text
.memory-genome/hot/hot.mgl
.memory-genome/hot/snapshot.mgs   # optional checkpoint snapshot
```

Sealed memory is semi-static and page based:

```text
.memory-genome/manifest.mgm
.memory-genome/dictionary/markers.mgd
.memory-genome/hot/hot.mgl
.memory-genome/hot/snapshot.mgs
.memory-genome/pages/000001.mgp
.memory-genome/pages/000002.mgp
.memory-genome/indexes/page_index.mgi
.memory-genome/indexes/marker_index.mgi
.memory-genome/indexes/fuse_index.mgi
.memory-genome/exports/memory.md
```

## Storage Format Direction

Internal runtime storage is binary from the beginning. JSON is only an optional debug output format; it is not storage, not the default export, and not part of the storage architecture.

Current direction:

- manifest: MessagePack binary `.mgm`;
- marker dictionary: MessagePack binary `.mgd`;
- hot memory: RAM-first L1 layer plus length-prefixed MessagePack append-only `.mgl`;
- optional hot checkpoint: MessagePack binary `.mgs`;
- sealed pages: MessagePack binary `.mgp`, with a future custom binary page codec boundary;
- page catalog and candidate indexes: MessagePack binary `.mgi`;
- compression: zstd is available now;
- fast profile: `mge init --profile fast` uses MessagePack + zstd while keeping `ExactMarkerPageIndex` as the default index;
- human-readable export: Markdown at `.memory-genome/exports/memory.md`;
- debug output: JSON may be emitted explicitly by CLI flags such as `--json` or `export --format json`.

Binary runtime files carry fixed headers with magic bytes, file kind, format version, codec identifier, payload length, and a SHA-256 payload checksum. This applies to `manifest.mgm`, `dictionary/markers.mgd`, `hot/hot.mgl` frames, `pages/*.mgp`, and `indexes/*.mgi`.

Full-file storage writes use a temporary file, flush/sync, and same-directory rename where practical. Hot memory remains a binary log format: `hot/hot.mgl` has a `hot_log` frame followed by `hot_record` frames.

Hot memory is RAM-first. `remember` updates `HotMemoryLayer` immediately and queues the cell for the persistence path; `recall` reads L1 RAM and does not wait for disk. `seal`, `checkpoint`, and normal engine drop flush pending hot events before crossing a durability boundary.

Durability policy is stored in the manifest and exposed through config:

```bash
mge config set durability fast
mge config set durability balanced
mge config set durability safe
mge checkpoint
```

- `fast`: queued hot persistence writes without forcing fsync except explicit checkpoint/seal boundaries.
- `balanced`: the default; queued writes are fsynced at checkpoint/seal/drop boundaries.
- `safe`: RAM-first API is preserved, but flushed queued records use per-record sync semantics during the persistence flush path.

Recovery reads `hot/snapshot.mgs` when usable, then replays `hot/hot.mgl` after the snapshot offset. If the final hot record frame is truncated or corrupted, recovery keeps all valid earlier frames, truncates the bad tail, and rebuilds `HotMemoryLayer` from valid cells.

Page files use codecs hidden behind the `PageCodec` trait:

- `MessagePackPageCodec` for runtime page storage;
- `JsonPageCodec` exists only as an optional debug/legacy codec and is rejected for runtime store initialization/config.

New sealed pages carry a SHA-256 content checksum computed over a canonical MessagePack page representation with the checksum field cleared. The checksum is codec-independent and is verified by `mge validate`.

Compression is hidden behind the `Compressor` trait:

- `NoCompression`
- `ZstdCompression`

The manifest stores the default codec/compression for newly sealed pages. Each `PageCatalogEntry` stores the actual codec/compression for that page, so the engine can read mixed binary stores without rewriting existing pages.

`mge config set` updates manifest defaults and lightweight derived indexes. It does not rewrite existing page files or mutate existing page catalog entries. Changing `--index-kind` rebuilds only the candidate index from existing sealed pages.

`mge validate` is a read-only storage consistency check. It verifies manifest/catalog/index kind alignment, page file readability, binary headers, payload checksums, page metadata, marker summaries, page checksums, marker dictionary consistency and references, cell links, candidate-index coverage, and orphan storage files. It reports wrong magic, wrong file kind, unsupported version, truncated payload, and corrupted payload errors. It does not repair or rewrite store data.

## Page Clustering

Page building is hidden behind the `PageClusterer` trait.

Current default:

- `ScopeKindClusterer`: groups cells by `scope + kind`.

Available deterministic extension:

- `MarkerOverlapClusterer`: groups cells inside the same `scope + kind` base group by marker overlap. It does not use ML or embeddings.

`PageBuildOptions` adds logical page limits:

- target page bytes, default 64 KiB;
- max cells per page, default 512.

The default seal path still uses `ScopeKindClusterer`. `MarkerOverlapClusterer` is available as explicit opt-in store config:

```bash
mge config set --page-clusterer marker_overlap
```

## Candidate Page Index

The default index is `ExactMarkerPageIndex`:

```text
MarkerId -> Vec<PageId>
```

It remains the default because it is stable and easy to debug.

`BinaryFusePageIndex` is available as an opt-in index kind:

```bash
mge init --index-kind binary_fuse_page
mge config set --index-kind binary_fuse_page
```

This is a real per-page static filter implementation backed by the `xorf` crate's `xorf::BinaryFuse16`. For every sealed page, the engine builds one Binary Fuse filter from that page's `marker_summary`. A query scans the page filters and returns candidate page IDs.

`BinaryFusePageIndex` is a probabilistic candidate page filter, not an inverted `marker -> pages` index. False positives are allowed and simply load extra pages. False negatives are not expected when the filter is built correctly. The default Binary Fuse query uses union candidate semantics so it does not return fewer candidates than the exact index for the same sealed pages; tests assert `exact_candidates ⊆ binary_fuse_candidates`.

Hot memory uses mutable scanning/indexing. Sealed pages use static candidate indexes.

`IndexKind` records the current index implementation in manifest/catalog/index metadata. Implemented kinds are `exact_marker_page` and `binary_fuse_page`. Switching the kind does not rewrite sealed page files; it rebuilds only the candidate index from existing sealed pages.

## Index And Filter Minimalism

The project deliberately avoids a filter zoo.

L1 Hot RAM uses only exact mutable RAM indexes:

- `CellId -> MemoryCell`
- `MarkerId -> Vec<CellId>`
- canonical scope -> cells
- kind -> cells
- status -> cells

Bloom, Counting Bloom, Cuckoo, XOR, Ribbon, or other filter families are not used in L1 Hot RAM.

L2 Sealed Pages use:

- `ExactMarkerPageIndex` as the reliable default baseline;
- `BinaryFusePageIndex` as the only current optional static probabilistic candidate filter backend.

New index/filter backends may be added only when a benchmark shows a real bottleneck, the backend improves a real scenario, correctness is preserved, `CandidatePageIndex` remains stable, and the public API does not become a collection of experimental filters.

## Retrieval

Recall does the following:

1. Extract deterministic query markers.
2. Map known markers to marker IDs.
3. Scan hot memory.
4. Query sealed page candidates.
5. Load only candidate pages.
6. Rerank cells by marker overlap, subject/value matches, trust, status, and sensitivity.
7. Filter deprecated/rejected and `SecretReference` cells by default.
8. Deduplicate ranked cells by `cell_id`.
9. Emit a `ContextPacket`.

Recall modes are explicit:

- `focused` is the default point-query mode. It uses normal scoring/reranking and `max_items`.
- `broad` is for wider tasks, projects, modules, or themes. It expands candidate page selection and raises the effective result limit when the caller did not request a larger one.
- `full_scope` is for an explicit request for all active memory inside a scope. It requires a scope, does not require text relevance, and still excludes deprecated/rejected/superseded memory by default.

The `ContextPacket` is task-relevant and size-controlled. It is not treated as inherently tiny; `max_items`, recall mode, and scope define the practical size boundary.

Reranking is transparent in JSON/debug output. `ContextDebugInfo.score_details` reports the score components for returned items:

- marker overlap score;
- exact subject match score;
- value overlap score;
- exact value match score;
- trust bonus;
- status bonus;
- sensitivity penalty.

The prompt text output intentionally stays compact and does not include scores.

Recall debug/statistics output also exposes the candidate-index path:

- recall mode;
- effective max items;
- index kind;
- pages considered;
- pruned candidate pages;
- hot/sealed/total cells scanned;
- cells decoded;
- cells filtered;
- cells ranked;
- page filters scanned;
- candidate pages returned;
- loaded pages;
- sealed cells scanned;
- false-positive candidate pages after page load;
- returned items;
- whether full-scope was used.

Detailed recall timing is also reported for performance diagnosis:

- query marker extraction;
- hot memory lookup;
- candidate page index lookup;
- page file read/load;
- page decode;
- cell filtering;
- reranking;
- ContextPacket build;
- total recall time.

Recall may prune candidate pages before decode only when catalog `marker_summary` makes the decision deterministic: missing required scope/kind markers, impossible query marker overlap, or a page summary containing only policy-disallowed status/sensitivity markers. This uses existing page metadata and does not change storage layout.

## Extension Traits

The code has explicit interfaces for future changes:

- `Store`
- `PageCodec`
- `Compressor`
- `CandidatePageIndex`
- `Retriever`
- `SecurityProvider`

## Future Security

The page pipeline is structured for:

```text
encode page -> compress page -> encrypt page -> write page
```

Current storage uses `NoSecurity` as an honest pass-through implementation. It does not claim to encrypt.

Future security layers:

- page-level encryption
- session-bound unlock
- page keys
- encrypted indexes
- blind marker tokens
- HMAC marker indexes
- policy-gated retrieval
- agent capabilities
- audit log

Future blind marker direction:

```text
blind_marker = HMAC(index_key, canonical_marker)
```

## Policy Foundation

Recall uses `RecallPolicy` as the central filtering policy. Defaults are restrictive for risky memory:

- deprecated memories are excluded;
- rejected memories are excluded;
- superseded memories are excluded;
- `SecretReference` cells are excluded.

`AgentCapabilities` provides an API boundary for explicit future access grants such as `ReadSecretReferences`. CLI recall exposes opt-in flags for compatibility and testing:

```bash
mge recall "api key" --include-secret-references
mge recall "old decision" --include-deprecated
mge recall "module work" --mode broad
mge recall --mode full-scope --scope project-alpha
```

`AuditLogger` is defined as an interface and wired through a `NoopAuditLogger` hook in recall. Durable audit storage is intentionally left for a later security package.
