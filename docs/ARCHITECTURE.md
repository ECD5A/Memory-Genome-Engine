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

The dictionary persists stable integer IDs in `.memory-genome/markers.json`.

## Storage Layers

Hot memory is mutable and append-only:

```text
.memory-genome/hot/hot_cells.jsonl
```

Sealed memory is semi-static and page based:

```text
.memory-genome/pages/000001.mgp
.memory-genome/indexes/page_catalog.json
.memory-genome/indexes/marker_to_pages.json
```

Page files use codecs hidden behind the `PageCodec` trait:

- `JsonPageCodec`
- `MessagePackPageCodec`

Compression is hidden behind the `Compressor` trait:

- `NoCompression`
- `ZstdCompression`

The manifest stores the default codec/compression for newly sealed pages. Each `PageCatalogEntry` stores the actual codec/compression for that page, so the engine can read mixed stores and old JSON/no-compression pages.

`mge config set` updates only manifest defaults for future seals. It does not rewrite existing page files or mutate existing page catalog entries.

## Page Clustering

Page building is hidden behind the `PageClusterer` trait.

Current default:

- `ScopeKindClusterer`: groups cells by `scope + kind`.

Available deterministic extension:

- `MarkerOverlapClusterer`: groups cells inside the same `scope + kind` base group by marker overlap. It does not use ML or embeddings.

`PageBuildOptions` adds logical page limits:

- target page bytes, default 64 KiB;
- max cells per page, default 512.

The default seal path still uses `ScopeKindClusterer`; marker-overlap clustering is an extension point until store-level clustering config is added.

## Candidate Page Index

v0.1 implements `ExactMarkerPageIndex`:

```text
MarkerId -> Vec<PageId>
```

The public index abstraction is `CandidatePageIndex`. Later versions can replace the exact map with XOR/Binary Fuse/Ribbon-style static filters without changing the retrieval API.

Hot memory uses mutable scanning/indexing. Sealed pages use static candidate indexes.

`IndexKind` records the current index implementation in manifest/catalog/index metadata. The only implemented kind is `exact_marker_page`. Binary Fuse/XOR/Ribbon are intentionally not faked.

## Retrieval

Recall does the following:

1. Extract deterministic query markers.
2. Map known markers to marker IDs.
3. Scan hot memory.
4. Query sealed page candidates.
5. Load only candidate pages.
6. Rerank cells by marker overlap, subject/value matches, trust, status, and sensitivity.
7. Filter deprecated/rejected and `SecretReference` cells by default.
8. Emit a `ContextPacket`.

Reranking is transparent in JSON/debug output. `ContextDebugInfo.score_details` reports the score components for returned items:

- marker overlap score;
- exact subject match score;
- value overlap score;
- trust bonus;
- status bonus;
- sensitivity penalty.

The prompt text output intentionally stays compact and does not include scores.

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
- `SecretReference` cells are excluded.

`AgentCapabilities` provides an API boundary for explicit future access grants such as `ReadSecretReferences`. CLI recall exposes opt-in flags for compatibility and testing:

```bash
mge recall "api key" --include-secret-references
mge recall "old decision" --include-deprecated
```

`AuditLogger` is defined as an interface and wired through a `NoopAuditLogger` hook in recall. Durable audit storage is intentionally left for a later security package.
