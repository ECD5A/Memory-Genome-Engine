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
- Reranking теперь записывает marker, subject, value, trust, status и sensitivity score components.
- Prompt text output остается компактным и не раскрывает score internals.
- Добавлен `IndexKind` с реализованным kind `exact_marker_page`.
- Manifest, page catalog, stats и exact index files теперь несут index kind metadata.
- `CandidatePageIndex` теперь раскрывает `kind()` для будущих static index implementations.
- Binary Fuse/XOR/Ribbon indexes остаются roadmap items; fake implementation не добавлялся.
- Добавлен `RecallPolicy` как центральная recall filtering policy.
- Добавлен `AgentCapabilities` для explicit future access grants.
- CLI recall теперь имеет opt-in flags `--include-deprecated` и `--include-secret-references`.
- Добавлены `AuditLogger` interface и `NoopAuditLogger` recall hook.

## В Работе

- Нет активного implementation item на этот момент.

## Дальше

- Добавить store-level clustering config и optional marker-overlap seal mode.
- Добавить реальный static filter index за `CandidatePageIndex`.
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
```

## Статус Проверки

- `cargo fmt`: passed.
- `cargo test`: passed, 27 tests.
- Milestone smoke commands: passed.
- MessagePack+zstd smoke commands: passed.
- Config show/set mixed-store smoke commands: passed.
- Default clustering smoke commands: passed.
- Recall JSON score debug smoke command: passed.
- Index kind stats/config smoke command: passed.
- Recall policy secret-reference opt-in smoke command: passed.
- Smoke result после sealing:
  - hot cells: 0
  - sealed pages: 1-2 depending on smoke scenario
  - sealed cells: 1-2 depending on smoke scenario
  - index type: `exact_marker_page`
  - current index kind: `exact_marker_page`
  - current page codec: `messagepack`
  - current compression: `zstd`
