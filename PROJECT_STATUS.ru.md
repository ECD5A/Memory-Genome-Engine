# Memory Genome Engine - Статус Проекта

[English version](PROJECT_STATUS.md)

Этот файл - рабочий журнал репозитория. Его нужно держать актуальным, чтобы не повторять уже сделанную работу и не возвращаться к закрытым решениям без причины.

## Source Of Truth

Цель проекта задаётся исходным промптом и твоими уточнениями: сделать компактный, быстрый, локальный Rust-first memory engine для LLM agents, где память хранится как typed cells + marker genomes + sealed pages + candidate filters, а агент получает только `ContextPacket`.

Главная логика:

```text
Memory = Cells + Markers + Pages + Filters + Context Packets
Hot memory = mutable
Sealed memory = static indexed pages
ExactMarkerPageIndex = default
BinaryFusePageIndex = opt-in probabilistic candidate page filter
Agent receives ContextPacket, not raw memory store
```

Непереговорные правила:

- Не превращать проект в chatbot, UI, cloud service, Markdown memory или vector DB.
- Не хранить raw credentials/secrets; только `SecretReference` placeholders.
- Не делать fake encryption или fake Binary Fuse.
- Не ломать defaults ради экспериментов; быстрые/новые режимы сначала идут как opt-in.
- Не раздувать проект: маленькие модули, понятные traits, тесты, отдельные коммиты.

JSON policy:

- JSON/JSONL не являются internal runtime storage, defaults или частью storage architecture.
- JSON разрешён только как explicit debug output, CLI `--json` output или API-level structured input parsing.
- Runtime storage уже использует compact binary files: MessagePack-based `.mgm`, `.mgd`, `.mgl`, `.mgp` и `.mgi`.
- `MemoryValue::Structured(serde_json::Value)` является API-level structured value, а не обязательством хранить память как JSON.

## Roadmap Snapshot

| Stage | Status | Notes |
| --- | --- | --- |
| v0.1 core/CLI | Done / hardening | Rust core, CLI, cells, markers, hot memory, sealed pages, exact index, recall, context packets работают. |
| v0.2 storage/index foundation | Done / hardening | Binary runtime storage layout, MessagePack, zstd, config, clustering, score debug, Binary Fuse opt-in и validation hardening сделаны. |
| v0.2 remaining | In progress | Benchmark-driven reranking и compact storage/index hardening. |
| v0.3 SDK/MCP | Not started | Python/TypeScript/MCP только после стабилизации Rust core API. |
| v0.4 security | Foundation only | Interfaces/policy есть; real encryption/session unlock/blind markers ещё не начаты. |
| v0.5 safety/search | Partial foundation | Policy/capabilities есть; poisoning/conflict/vector reranking ещё не начаты. |

## Текущий Фокус

- Довести v0.1/v0.2 core/storage/index foundation до состояния, где runtime path быстрый и компактный.
- Держать JSON вне internal runtime storage; использовать его только для explicit debug output и structured input parsing.
- Держать реализацию детерминированной, локальной, компактной, marker/page based и готовой к будущим encryption, SDK и MCP.

## Сделано

- Git-репозиторий инициализирован.
- Rust toolchain подтвержден: `cargo 1.95.0`, `rustc 1.95.0`.
- Создан Rust workspace.
- Реализован `mge-core`:
  - typed memory models;
  - marker canonicalization и persistent dictionary;
  - deterministic marker extraction;
  - deterministic shallow marker extraction для structured JSON keys и коротких scalar values;
  - append-only binary hot log;
  - page model и MessagePack page codec;
  - exact marker-to-page candidate index;
  - recall, reranking, filtering и context packet output;
  - extension traits для store, page codec, compression, index, retrieval и security.
- Реализован `mge-cli`:
  - `init`
  - `remember`
  - `recall`
  - `seal`
  - `inspect`
  - `validate`
  - `stats`
  - `export` / `export --format markdown`
  - `export --format json` как explicit debug output
- CLI `remember` поддерживает structured values через `--json-value`, сохраняемые как `MemoryValue::Structured`.
- CLI `remember` поддерживает typed reference и timestamp values через `--reference-value` и `--timestamp-value`.
- CLI `remember` поддерживает provenance и graph hints через `--source-type`, `--source-ref` и повторяемый `--link`.
- Sealing сохраняет cell `source` metadata и `links` в sealed pages.
- CLI `stats` поддерживает `--json`, сохраняя human output default.
- Добавлена документация:
  - `README.md`
  - `README.ru.md`
  - `docs/ARCHITECTURE.md`
  - `docs/ARCHITECTURE.ru.md`
  - `docs/ROADMAP.md`
  - `docs/ROADMAP.ru.md`
  - `examples/basic_usage.md`
  - `examples/basic_usage.ru.md`
- Roadmap обновлён: completed v0.1/v0.2 foundation work и deferred experiments отмечены явно.
- Добавлены Rust tests для marker canonicalization, dictionary IDs, cell creation, marker generation, hot recall, sealing, sealed recall, index lookup, filtering, context packet text и stats output.
- Добавлен CLI milestone integration test против реального binary `mge`.
- Добавлена MIT license от ECD5A.
- README оформлен бейджами, EN/RU навигацией, Donate-блоком и license section.
- Добавлен `MessagePackPageCodec` за существующим trait `PageCodec` как первый v0.2 codec step.
- Добавлен `ZstdCompression` за существующим trait `Compressor`.
- Store manifest теперь хранит default `page_codec` и `compression` для новых sealed pages.
- Page catalog entries теперь хранят per-page codec/compression для mixed-store и backward-compatible reads.
- Internal store files теперь используют final binary layout:
  - `manifest.mgm`
  - `dictionary/markers.mgd`
  - `hot/hot.mgl`
  - `pages/*.mgp`
  - `indexes/page_index.mgi`
  - `indexes/marker_index.mgi`
  - `indexes/fuse_index.mgi`
  - `exports/memory.md` для human-readable Markdown export
- Hot memory теперь использует length-prefixed MessagePack records вместо JSONL.
- Marker dictionary, manifest, page catalog и candidate indexes теперь persist как MessagePack binary files.
- Новые sealed pages теперь хранят codec-independent SHA-256 content checksums.
- Canonical bytes для page checksum и logical page-size estimates теперь используют MessagePack вместо JSON.
- CLI `init` теперь использует binary runtime storage by default; JSON page codec отклоняется для runtime store initialization/config.
- CLI `init --profile fast` добавлен как opt-in compact storage profile: MessagePack + zstd + exact index.
- CLI `export` теперь по умолчанию пишет Markdown в `.memory-genome/exports/memory.md`; JSON export является explicit debug output.
- Добавлены CLI `config show` и `config set` для существующих stores.
- Storage config updates меняют только defaults для будущих seals; существующие pages остаются нетронутыми и читаются через catalog metadata.
- Добавлены tests для zstd roundtrip, init options, MessagePack+zstd sealed recall, binary storage layout, Markdown export и binary catalog defaults.
- Добавлен trait `PageClusterer`.
- `ScopeKindClusterer` сохранен как default seal clustering strategy.
- Добавлен `MarkerOverlapClusterer` как deterministic no-ML extension strategy.
- Добавлен `PageBuildOptions` с defaults: 64 KiB target page bytes и 512 max cells.
- Page builder теперь соблюдает logical page limits.
- Добавлен `ContextDebugInfo.score_details` для transparent reranking в JSON/debug output.
- Reranking теперь записывает marker, subject, value overlap, exact value match, trust, status и sensitivity score components.
- Context packet building теперь deduplicate ranked cells по `cell_id` перед возвратом memory агентам.
- Prompt text output остается компактным и не раскрывает score internals.
- Добавлен `IndexKind` с реализованным kind `exact_marker_page`.
- Manifest, page catalog, stats и exact index files теперь несут index kind metadata.
- `CandidatePageIndex` теперь раскрывает `kind()` и query statistics для static index implementations.
- Добавлен `BinaryFusePageIndex` как opt-in `binary_fuse_page`, при этом `ExactMarkerPageIndex` остается default.
- Binary Fuse page filters - реальные `xorf::BinaryFuse16` filters, построенные per sealed page по `marker_summary`; fake Binary Fuse implementation не добавлялся.
- CLI `init` и `config set` теперь поддерживают `--index-kind exact_marker_page|binary_fuse_page`.
- При смене `index_kind` пересобирается только candidate index по существующим sealed pages; sealed page files не переписываются.
- Recall debug теперь показывает index kind, page filters scanned, candidate pages returned, loaded pages, sealed cells scanned и post-load false-positive candidate pages.
- Tests теперь проверяют `exact_candidates ⊆ binary_fuse_candidates` на тех же sealed pages и проверяют смену index kind без rewrite page files.
- Добавлен synthetic benchmark tool: `cargo run -p mge-cli --bin mge-synthetic-bench`.
- Synthetic benchmark сравнивает `exact_marker_page` и opt-in `binary_fuse_page` на одинаковых generated stores и проверяет `exact_candidates ⊆ binary_fuse_candidates`.
- Hot log archiving теперь использует уникальные archive names, если несколько seals попадают в одно timestamp window.
- Добавлены `ValidationReport` и CLI `validate` как read-only consistency checks для manifest, catalog, pages, page checksums, marker references и candidate index coverage.
- Store validation теперь проверяет cell links на unknown targets и self-links.
- Store validation теперь предупреждает об orphan page files и unknown unmanaged index files.
- Store validation теперь проверяет marker dictionary forward/reverse consistency, canonical markers и `next_id`.
- Добавлен `RecallPolicy` как центральная recall filtering policy.
- Добавлен `AgentCapabilities` для explicit future access grants.
- CLI recall теперь имеет opt-in flags `--include-deprecated` и `--include-secret-references`.
- Добавлены `AuditLogger` interface и `NoopAuditLogger` recall hook.
- Добавлен `PageClustererKind` в manifest/config.
- CLI `init` и `config set` теперь поддерживают `--page-clusterer scope_kind|marker_overlap`.
- Seal path теперь использует configured page clusterer, default остается `scope_kind`.

## В Работе

- Нет активного implementation item на этот момент.

## Дальше

- Продолжать core hardening через validation, storage и index tests без изменения defaults.
- Durable audit log storage добавлять только в более позднем security package.
- Conflict/poisoning detection рассматривать только после стабилизации текущего storage/index core.
- SDK wrappers добавлять только после стабилизации Rust core API.

## Откаты / Не Повторять

- Не начинать с UI, chatbot, cloud service, vector DB, fake encryption, fake Binary Fuse или Markdown как внутреннего storage format.
- Не хранить реальные credentials или secrets. Sensitive values должны быть представлены metadata/placeholders через `SecretReference`.
- Не заменять marker/page API на vector-only retrieval flow.

## Команды Проверки

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
cargo run -p mge-cli -- seal
cargo run -p mge-cli -- recall "How should the agent answer technical questions?"
cargo run -p mge-cli -- stats
cargo run -p mge-cli -- stats --json
cargo run -p mge-cli -- validate
cargo run -p mge-cli -- export
cargo run -p mge-cli -- init --index-kind binary_fuse_page
cargo run -p mge-cli -- config set --index-kind binary_fuse_page
cargo run -p mge-cli --bin mge-synthetic-bench -- --cells 1200 --pages 120 --marker-groups 12 --targeted-queries 6 --noise-queries 3
```

## Статус Проверки

- `cargo fmt`: passed.
- `cargo test`: passed, 66 tests total (12 CLI unit tests + 2 CLI integration tests + 1 core unit test + 51 core integration tests).
- Milestone smoke commands: passed.
- MessagePack+zstd smoke commands: passed.
- Config show/set mixed-store smoke commands: passed.
- Default clustering smoke commands: passed.
- Recall JSON score debug smoke command: passed.
- Index kind stats/config smoke command: passed.
- Binary Fuse init/recall/stats smoke command: passed.
- Exact-to-Binary-Fuse config switch smoke command: passed; sealed page hash unchanged.
- Binary storage layout CLI smoke command: passed; required `.mgm/.mgd/.mgl/.mgp/.mgi` files существуют, старые JSON/JSONL storage files отсутствуют, Markdown export size 621 bytes.
- JSON runtime page codec reject smoke command: passed; `mge init --page-codec json` завершается с `invalid input`.
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
- Validate CLI smoke commands: passed для `exact_marker_page` и `binary_fuse_page`.
- Page checksum smoke command: passed для MessagePack+zstd sealed page, checksum length 64, `mge validate --json` ok.
- Structured JSON remember smoke command: passed, exported value type `structured`.
- Typed reference/timestamp remember smoke command: passed, exported value types `reference` и `timestamp`.
- Source/link remember smoke command: passed, exported source и links retained.
- Source/link seal persistence test: passed.
- Link validation smoke command: passed для valid link и failed as expected для unknown link.
- Orphan storage validation tests: passed для orphan page files и unknown unmanaged index files.
- Context packet dedupe test: passed для duplicate ranked cells с одинаковым `cell_id`.
- Structured JSON marker extraction tests: passed для marker generation и hot recall.
- Structured JSON marker extraction CLI smoke command: passed, recall matched `tag:style` и `tag:concise`.
- CLI milestone integration test: passed для init, remember, recall JSON, seal, stats JSON и validate JSON.
- Fast profile CLI integration test: passed для `mge init --profile fast`.
- Binary storage layout tests: passed для `.mgm/.mgd/.mgl/.mgp/.mgi` files и отсутствия старых JSON/JSONL storage files.
- Markdown export test: passed для `.memory-genome/exports/memory.md`.
- Marker dictionary consistency validation test: passed.
- Stats JSON smoke command: passed, `sealed_pages` и `current_index_kind` exported.
- Recall policy secret-reference opt-in smoke command: passed.
- Marker-overlap clusterer seal smoke command: passed.
- Smoke result после sealing:
  - hot cells: 0
  - sealed pages: 1-2 depending on smoke scenario
  - sealed cells: 1-2 depending on smoke scenario
  - index type: `exact_marker_page` или `binary_fuse_page` depending on smoke scenario
  - current index kind: `exact_marker_page` или `binary_fuse_page` depending on smoke scenario
  - current page codec: `messagepack`
  - current compression: `none` или `zstd` depending on smoke scenario
