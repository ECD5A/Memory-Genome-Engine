# Memory Genome Engine

[![Rust](https://img.shields.io/badge/Rust-1.95%2B-f74c00?logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![License: MIT](https://img.shields.io/badge/License-MIT-green.svg)](LICENSE)
[![Status](https://img.shields.io/badge/status-Mandate%202%20integration-blue)](PROJECT_STATUS.ru.md)
[![Interface](https://img.shields.io/badge/interface-CLI%20%7C%20Core%20API-informational)](crates/)
[![Storage](https://img.shields.io/badge/storage-cells%20%2B%20markers%20%2B%20pages-6f42c1)](docs/ARCHITECTURE.ru.md)

[English version](README.md)

Memory Genome Engine - быстрый структурированный движок памяти для LLM-агентов. Он хранит память как типизированные ячейки, кодирует их маркерными геномами, группирует в страницы и возвращает агентам компактные контекстные пакеты через marker-based candidate page search.

## Зачем Это Нужно

Большинство систем памяти для агентов хранят сырые текстовые чанки, Markdown-заметки или записи в векторной базе. Memory Genome Engine хранит типизированные memory cells с marker genomes, чтобы агент мог быстрее и точнее получать структурированную память.

Главная формула:

```text
Memory = Cells + Markers + Pages + Filters + Context Packets
```

Агент должен арендовать память у движка, а не владеть всем хранилищем. Он запрашивает релевантный контекст и получает небольшой context packet, не читая vault-файлы напрямую.

## Архитектура

```text
Cells -> Marker Genome -> Hot Memory -> Sealed Pages -> Candidate Page Index -> Context Packet
```

Реализация v0.1 сделана Rust-first:

- `mge-core`: переиспользуемая библиотека движка памяти.
- `mge-cli`: CLI interface, binary `mge`, плюс `mge-mcp-server` для local JSON-RPC integration.
- `sdk/python` и `sdk/typescript`: thin wrappers вокруг Rust CLI.
- `.memory-genome/`: локальное binary-хранилище с `manifest.mgm`, `dictionary/markers.mgd`, `hot/hot.mgl`, `pages/*.mgp` и `indexes/*.mgi`.
- `MarkerGenome`: structured marker DNA каждой `MemoryCell`; flattened marker IDs остаются runtime/index view.
- Runtime storage использует MessagePack-oriented binary files; zstd compression доступен для sealed pages.
- Candidate page search по умолчанию использует `ExactMarkerPageIndex`; `BinaryFusePageIndex` доступен как opt-in probabilistic page filter на реальном `xorf::BinaryFuse16`.

Подробнее:

- [Архитектура](docs/ARCHITECTURE.ru.md)
- [Дорожная карта](docs/ROADMAP.ru.md)
- [Бенчмарки](docs/BENCHMARKS.ru.md)
- [Интеграция](docs/INTEGRATION.ru.md)
- [MCP adapter](docs/MCP.ru.md)
- [SDK](docs/SDK.ru.md)
- [Базовое использование](examples/basic_usage.ru.md)
- [Agent workflow](examples/agent_workflow.ru.md)
- [Rust API example](examples/basic_usage.rs)
- [Rust CLI host example](examples/agent_host_cli.rs)
- [Python SDK example](examples/python_basic_usage.py)
- [Python agent host example](examples/python_agent_host.py)
- [TypeScript SDK example](examples/typescript_basic_usage.ts)
- [TypeScript agent host example](examples/typescript_agent_host.ts)
- [MCP JSON-RPC session](examples/mcp_agent_session.jsonl)
- [Статус проекта](PROJECT_STATUS.ru.md)

## Почему Не Markdown

Markdown удобен для экспорта и чтения человеком, но плох как внутренний высокоскоростной формат памяти. Этот движок хранит типизированные cells и marker IDs, а наружу отдает readable context packets, Markdown exports или явный debug JSON.

## Почему Не Vector-Only RAG

Векторный поиск можно добавить позже как reranker внутри уже выбранных candidate pages. Ядро остается marker/page based, чтобы retrieval был детерминированным, компактным и готовым к policy-gated доступу.

## Будущая Безопасность

Storage layer спроектирован так, чтобы позже добавить page-level encryption, session keys, blind marker indexes и policy-gated access.

Будущий pipeline записи страницы:

```text
encode page -> compress page -> encrypt page -> store page
```

Текущий non-encrypted pipeline:

```text
encode page -> optional compression -> no encryption -> store page
```

Агент не должен владеть ключом, читать vault-файлы напрямую или получать больше памяти, чем нужно для задачи.

## Быстрый Старт

```bash
cargo build

cargo run -p mge-cli -- init

cargo run -p mge-cli -- remember "User prefers concise technical explanations" \
  --kind user_preference \
  --scope global \
  --trust user_confirmed

cargo run -p mge-cli -- recall "How should the agent answer technical questions?"

cargo run -p mge-cli -- seal

cargo run -p mge-cli -- recall "How should the agent answer technical questions?"

cargo run -p mge-cli -- stats
```

Для компактных sealed pages можно инициализировать новое хранилище с MessagePack и zstd:

```bash
cargo run -p mge-cli -- init --profile fast
cargo run -p mge-cli -- init --page-codec messagepack --compression zstd
```

Для opt-in Binary Fuse candidate filtering:

```bash
cargo run -p mge-cli -- init --index-kind binary_fuse_page
cargo run -p mge-cli -- config set --index-kind binary_fuse_page
```

## CLI

```bash
mge init
mge init --profile fast
mge init --page-codec messagepack --compression zstd
mge init --index-kind binary_fuse_page
mge config set --page-clusterer marker_overlap
mge remember "..." --kind user_preference --scope global --trust user_confirmed
mge remember --kind user_preference --subject answer_style --json-value '{"style":"concise","max_examples":2}'
mge remember --kind project_fact --reference-value vault://references/api-key --sensitivity secret_reference
mge remember "Decision recorded" --kind decision --source-type issue --source-ref MGE-1 --link 1
mge recall "technical answer style"
mge recall "technical answer style" --mode broad
mge recall --mode full-scope --scope global
mge recall "api key" --include-secret-references
mge seal
mge config show
mge config set --page-codec messagepack --compression zstd
mge config set --index-kind binary_fuse_page
mge inspect
mge validate
mge stats
mge stats --json
mge export
mge export --format json # explicit debug export
```

Core benchmark/smoke harness:

```bash
cargo run -p mge-cli --bin mge-synthetic-bench -- --cells 1200 --pages 120 --scopes 16 --markers-per-cell 5 --marker-groups 12 --targeted-queries 6 --noise-queries 3 --repeats 5 --seed 1
cargo run -p mge-cli --bin mge-corpus-bench -- --corpus . --store-root ../mge-corpus-bench-store --profile mixed --max-files 200 --max-bytes 8388608 --chunk-lines 24 --repeats 3 --seed 1
cargo run -p mge-cli --bin mge-corpus-bench -- --generated --profile small --store-root ../mge-generated-bench-store --repeats 2 --seed 7
```

`mge-corpus-bench` читает только локальные text/code corpus files, пропускает symlinks и неподдерживаемые binary extensions, пишет только в `--store-root`, сравнивает exact vs Binary Fuse и выводит JSON benchmark/debug output с human-readable recommendation section.

Benchmark reports объяснены в [Бенчмарках](docs/BENCHMARKS.ru.md). JSON benchmark output - это report/debug output, а не runtime storage.

## Agent Integration

Мандат 2 добавляет local integration boundaries без изменения core storage architecture:

- `mge-mcp-server`: MCP-ready JSON-RPC stdin/stdout adapter.
- Python SDK: thin wrapper в `sdk/python`.
- TypeScript SDK: thin wrapper в `sdk/typescript`.
- Agent examples: [Agent workflow](examples/agent_workflow.ru.md).

Integration layer возвращает `ContextPacket` из recall и использует JSON только как protocol/debug output, а не runtime storage.

Local integration smoke:

```bash
cargo run -p mge-cli --bin mge-mcp-server
python examples/python_basic_usage.py
node examples/typescript_basic_usage.ts
```

SDK содержат локальную packaging metadata для разработки:

```bash
python -m pip install -e sdk/python
cd sdk/typescript && npm run smoke
```

Текущая MCP-ready поверхность - versioned JSON-RPC stdin/stdout adapter с `protocol_version = mge-jsonrpc-1` и `integration_schema_version = 1`. Полная external MCP SDK dependency пока намеренно не добавляется.

Рекомендуемый lifecycle для agent host:

```text
recall -> local work -> remember -> checkpoint -> recall again -> seal -> validate
```

Используйте `focused` для узкого next-step context, `broad` для более широкого project/task context, а `full-scope` только с явным `--scope` для scoped review/export flows.

Для явного marker search используйте `--marker`:

```bash
mge recall "technical answer style" --marker kind:user_preference --marker scope:global
```

Recall modes заданы явно: `focused` используется по умолчанию, `broad` возвращает более широкий task-relevant packet, а `full-scope` требует `--scope`, чтобы вернуть всю default-allowed память внутри этого scope.

Используйте `--json-value` в `remember`, если значение нужно сохранить как `MemoryValue::Structured`, а не как текст или scalar:

```bash
mge remember --kind user_preference --subject answer_style --json-value '{"style":"concise","max_examples":2}'
```

В PowerShell передавайте JSON с escaped quotes или сначала сохраните JSON-строку в переменную.

Используйте `--reference-value` для ссылок/placeholders и `--timestamp-value` для Unix timestamp seconds. Не передавайте raw credentials или secret material.

```bash
mge remember --kind project_fact --reference-value vault://references/api-key --sensitivity secret_reference
mge remember --kind task_state --timestamp-value 1760000000
```

Используйте `--source-type` и `--source-ref` вместе, чтобы сохранить provenance. Повторяемый `--link` связывает новую cell с существующими cell IDs.

`mge config set` меняет defaults и легкие derived indexes. Существующие page files не переписываются; каждая catalog entry хранит codec/compression, нужные для чтения этой страницы. При смене `--index-kind` пересобирается только candidate page index по существующим sealed pages.

`mge validate` - read-only consistency check для manifest, page catalog, page files, page checksums, marker dictionary consistency/references, cell links, candidate index coverage и orphan storage files.

`BinaryFusePageIndex` - probabilistic candidate page filter, а не inverted `marker -> pages` map. Он строит один реальный `xorf::BinaryFuse16` static filter на каждую sealed page по ее `marker_summary`, сканирует page filters при query и может вернуть extra candidate pages. `ExactMarkerPageIndex` остается default для стабильного дебага.

Deprecated/rejected/superseded memories и `SecretReference` cells фильтруются по умолчанию. Recall opt-in flags стоит использовать только если у caller есть явная причина и capability.

## Структура Репозитория

```text
crates/
  mge-core/
  mge-cli/
docs/
examples/
tests/
```

## Текущие Ограничения

- Mandate 1 core закрыт: storage, L1 Hot RAM, sealed pages, indexes, validation/rebuild, CLI и benchmark foundation готовы.
- Mandate 2 integration foundation активен: MCP-ready adapter и thin Python/TypeScript SDK wrappers уже есть.
- Большой user-provided corpus всё ещё полезен перед любым дальнейшим scoring/filtering cleanup.
- Нет GUI.
- Нет chatbot.
- Нет vector database.
- Нет fake Binary Fuse implementation; opt-in Binary Fuse path использует реальный crate `xorf`.
- Нет fake encryption.
- Нет credential storage.

## Donate

If Memory Genome Engine is useful to your work, you can support the project here:

- Bitcoin (BTC): `1ECDSA1b4d5TcZHtqNpcxmY8pBH1GgHntN`
- USDT (TRC20): `TUF4vPdB6QkjCvZq18rBL4Qj4dK5ihCN75`

## Лицензия

MIT License. Copyright (c) 2026 ECD5A.
