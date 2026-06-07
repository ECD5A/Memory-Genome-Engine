# Архитектура

[English version](ARCHITECTURE.md)

Memory Genome Engine - Rust-first слой структурированной памяти для LLM-агентов.

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

## Модель Данных

Атомарная единица - `MemoryCell`. Cell хранит типизированное значение, metadata, trust/status/sensitivity, marker IDs, optional source metadata и links на другие cells.

Значения не считаются просто сырым текстом. v0.1 поддерживает:

- text
- symbol
- number
- boolean
- timestamp
- reference
- structured JSON

## Marker Genome

Каждая cell получает детерминированные marker strings, которые canonicalize в marker IDs через `MarkerDictionary`.

Примеры:

```text
kind:user_preference
scope:global
status:active
trust:user_confirmed
sensitivity:private
tag:technical
```

Dictionary сохраняет стабильные integer IDs в `.memory-genome/markers.json`.

## Storage Layers

Hot memory изменяемая и append-only:

```text
.memory-genome/hot/hot_cells.jsonl
```

Sealed memory полустатическая и page based:

```text
.memory-genome/pages/000001.mgp
.memory-genome/indexes/page_catalog.json
.memory-genome/indexes/marker_to_pages.json
.memory-genome/indexes/binary_fuse_pages.json
```

Page files используют codecs, скрытые за trait `PageCodec`:

- `JsonPageCodec`
- `MessagePackPageCodec`

Compression скрыт за trait `Compressor`:

- `NoCompression`
- `ZstdCompression`

Manifest хранит default codec/compression для новых sealed pages. Каждая `PageCatalogEntry` хранит фактический codec/compression конкретной страницы, поэтому движок может читать mixed stores и старые JSON/no-compression pages.

`mge config set` обновляет manifest defaults и легкие derived indexes. Он не переписывает существующие page files и не мутирует существующие page catalog entries. При смене `--index-kind` пересобирается только candidate index по существующим sealed pages.

## Page Clustering

Page building скрыт за trait `PageClusterer`.

Текущий default:

- `ScopeKindClusterer`: группирует cells по `scope + kind`.

Доступное deterministic extension:

- `MarkerOverlapClusterer`: группирует cells внутри базовой группы `scope + kind` по marker overlap. Он не использует ML или embeddings.

`PageBuildOptions` добавляет logical page limits:

- target page bytes, default 64 KiB;
- max cells per page, default 512.

Default seal path пока использует `ScopeKindClusterer`. `MarkerOverlapClusterer` доступен как explicit opt-in store config:

```bash
mge config set --page-clusterer marker_overlap
```

## Candidate Page Index

Default index - `ExactMarkerPageIndex`:

```text
MarkerId -> Vec<PageId>
```

Он остается default, потому что стабилен и удобен для дебага.

`BinaryFusePageIndex` доступен как opt-in index kind:

```bash
mge init --index-kind binary_fuse_page
mge config set --index-kind binary_fuse_page
```

Это реальная per-page static filter implementation на crate `xorf` и типе `xorf::BinaryFuse16`. Для каждой sealed page движок строит один Binary Fuse filter по `marker_summary` этой страницы. Query сканирует page filters и возвращает candidate page IDs.

`BinaryFusePageIndex` - probabilistic candidate page filter, а не inverted `marker -> pages` index. False positives допустимы и просто приводят к загрузке extra pages. False negatives не ожидаются, если filter корректно построен. Default Binary Fuse query использует union candidate semantics, поэтому не возвращает меньше candidates, чем exact index на тех же sealed pages; тесты проверяют `exact_candidates ⊆ binary_fuse_candidates`.

Hot memory использует mutable scanning/indexing. Sealed pages используют static candidate indexes.

`IndexKind` фиксирует текущую index implementation в manifest/catalog/index metadata. Реализованные kinds: `exact_marker_page` и `binary_fuse_page`. Смена kind не переписывает sealed page files; пересобирается только candidate index по существующим sealed pages.

## Retrieval

Recall делает следующее:

1. Извлекает детерминированные query markers.
2. Мапит известные markers в marker IDs.
3. Сканирует hot memory.
4. Запрашивает candidate sealed pages.
5. Загружает только candidate pages.
6. Rerank cells по marker overlap, subject/value matches, trust, status и sensitivity.
7. По умолчанию фильтрует deprecated/rejected и `SecretReference` cells.
8. Возвращает `ContextPacket`.

Reranking прозрачен в JSON/debug output. `ContextDebugInfo.score_details` показывает score components для возвращенных items:

- marker overlap score;
- exact subject match score;
- value overlap score;
- trust bonus;
- status bonus;
- sensitivity penalty.

Prompt text output намеренно остается компактным и не включает scores.

Recall debug/statistics output также показывает candidate-index path:

- index kind;
- page filters scanned;
- candidate pages returned;
- loaded pages;
- sealed cells scanned;
- false-positive candidate pages after page load.

## Extension Traits

В коде есть явные interfaces для будущих изменений:

- `Store`
- `PageCodec`
- `Compressor`
- `CandidatePageIndex`
- `Retriever`
- `SecurityProvider`

## Future Security

Page pipeline подготовлен под:

```text
encode page -> compress page -> encrypt page -> write page
```

Текущее storage использует честную pass-through реализацию `NoSecurity`. Он не делает вид, что шифрует данные.

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

Recall использует `RecallPolicy` как центральную filtering policy. Defaults ограничивают рискованную память:

- deprecated memories исключены;
- rejected memories исключены;
- `SecretReference` cells исключены.

`AgentCapabilities` задает API boundary для будущих explicit access grants, например `ReadSecretReferences`. CLI recall дает opt-in flags для compatibility и testing:

```bash
mge recall "api key" --include-secret-references
mge recall "old decision" --include-deprecated
```

`AuditLogger` определен как interface и подключен через `NoopAuditLogger` hook в recall. Durable audit storage намеренно оставлен для следующего security package.
