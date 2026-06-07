# Memory Genome Engine - Статус Проекта

[English version](PROJECT_STATUS.md)

Этот файл - рабочий журнал репозитория. Его нужно держать актуальным, чтобы не повторять уже сделанную работу и не возвращаться к закрытым решениям без причины.

## Текущий Фокус

- Собрать v0.1 Rust-first core и CLI в этой папке.
- Держать первую реализацию детерминированной, локальной, marker/page based и готовой к будущим compression, encryption, SDK и MCP.

## Сделано

- Git-репозиторий инициализирован.
- Rust toolchain подтвержден: `cargo 1.95.0`, `rustc 1.95.0`.
- Создан Rust workspace.
- Реализован `mge-core`:
  - typed memory models;
  - marker canonicalization и persistent dictionary;
  - deterministic marker extraction;
  - append-only hot JSONL store;
  - page model и JSON page codec;
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
  - `export --format json`
- Добавлена документация:
  - `README.md`
  - `README.ru.md`
  - `docs/ARCHITECTURE.md`
  - `docs/ARCHITECTURE.ru.md`
  - `docs/ROADMAP.md`
  - `docs/ROADMAP.ru.md`
  - `examples/basic_usage.md`
  - `examples/basic_usage.ru.md`
- Добавлены Rust tests для marker canonicalization, dictionary IDs, cell creation, marker generation, hot recall, sealing, sealed recall, index lookup, filtering, context packet text и stats output.
- Добавлена MIT license от ECD5A.
- README оформлен бейджами, EN/RU навигацией, Donate-блоком и license section.
- Добавлен `MessagePackPageCodec` за существующим trait `PageCodec` как первый v0.2 codec step.
- Добавлен `ZstdCompression` за существующим trait `Compressor`.
- Store manifest теперь хранит default `page_codec` и `compression` для новых sealed pages.
- Page catalog entries теперь хранят per-page codec/compression для mixed-store и backward-compatible reads.
- CLI `init` теперь поддерживает `--page-codec json|messagepack` и `--compression none|zstd`.
- Добавлены CLI `config show` и `config set` для существующих stores.
- Storage config updates меняют только defaults для будущих seals; существующие pages остаются нетронутыми и читаются через catalog metadata.
- Добавлены tests для zstd roundtrip, init options, MessagePack+zstd sealed recall и legacy catalog defaults.
- Добавлен trait `PageClusterer`.
- `ScopeKindClusterer` сохранен как default seal clustering strategy.
- Добавлен `MarkerOverlapClusterer` как deterministic no-ML extension strategy.
- Добавлен `PageBuildOptions` с defaults: 64 KiB target page bytes и 512 max cells.
- Page builder теперь соблюдает logical page limits.
- Добавлен `ContextDebugInfo.score_details` для transparent reranking в JSON/debug output.
- Reranking теперь записывает marker, subject, value overlap, exact value match, trust, status и sensitivity score components.
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
- Добавлены `ValidationReport` и CLI `validate` как read-only consistency checks для manifest, catalog, pages, marker references и candidate index coverage.
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

- Benchmark `exact_marker_page` vs opt-in `binary_fuse_page` на больших stores перед любым изменением defaults.
- Добавить durable audit log storage в следующем security package.
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
cargo run -p mge-cli -- remember "User prefers concise technical explanations" --kind user_preference --scope global --trust user_confirmed
cargo run -p mge-cli -- recall "How should the agent answer technical questions?"
cargo run -p mge-cli -- seal
cargo run -p mge-cli -- recall "How should the agent answer technical questions?"
cargo run -p mge-cli -- stats
cargo run -p mge-cli -- validate
cargo run -p mge-cli -- init --index-kind binary_fuse_page
cargo run -p mge-cli -- config set --index-kind binary_fuse_page
cargo run -p mge-cli --bin mge-synthetic-bench -- --cells 1200 --pages 120 --marker-groups 12 --targeted-queries 6 --noise-queries 3
```

## Статус Проверки

- `cargo fmt`: passed.
- `cargo test`: passed, 38 tests total (1 core unit test + 37 integration tests).
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
- Validate CLI smoke commands: passed для `exact_marker_page` и `binary_fuse_page`.
- Recall policy secret-reference opt-in smoke command: passed.
- Marker-overlap clusterer seal smoke command: passed.
- Smoke result после sealing:
  - hot cells: 0
  - sealed pages: 1-2 depending on smoke scenario
  - sealed cells: 1-2 depending on smoke scenario
  - index type: `exact_marker_page` или `binary_fuse_page` depending on smoke scenario
  - current index kind: `exact_marker_page` или `binary_fuse_page` depending on smoke scenario
  - current page codec: `json` или `messagepack` depending on smoke scenario
  - current compression: `none` или `zstd` depending on smoke scenario
