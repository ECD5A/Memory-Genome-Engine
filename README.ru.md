# Memory Genome Engine

[![Rust](https://img.shields.io/badge/Rust-1.95%2B-f74c00?logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![License: MIT](https://img.shields.io/badge/License-MIT-green.svg)](LICENSE)
[![Status](https://img.shields.io/badge/status-v0.1%20prototype-blue)](PROJECT_STATUS.ru.md)
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
- `mge-cli`: первый CLI-интерфейс, бинарник `mge`.
- `.memory-genome/`: локальное хранилище с manifest, marker dictionary, hot JSONL log, page files и JSON indexes.
- Page storage поддерживает JSON или MessagePack codecs и none/zstd compression через стабильные traits.
- Candidate page search по умолчанию использует `ExactMarkerPageIndex`; `BinaryFusePageIndex` доступен как opt-in probabilistic page filter на реальном `xorf::BinaryFuse16`.

Подробнее:

- [Архитектура](docs/ARCHITECTURE.ru.md)
- [Дорожная карта](docs/ROADMAP.ru.md)
- [Базовое использование](examples/basic_usage.ru.md)
- [Статус проекта](PROJECT_STATUS.ru.md)

## Почему Не Markdown

Markdown удобен для экспорта и чтения человеком, но плох как внутренний высокоскоростной формат памяти. Этот движок хранит типизированные cells и marker IDs, а наружу отдает readable context packets или JSON export.

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
mge init --page-codec messagepack --compression zstd
mge init --index-kind binary_fuse_page
mge config set --page-clusterer marker_overlap
mge remember "..." --kind user_preference --scope global --trust user_confirmed
mge recall "technical answer style"
mge recall "api key" --include-secret-references
mge seal
mge config show
mge config set --page-codec messagepack --compression zstd
mge config set --index-kind binary_fuse_page
mge inspect
mge stats
mge export --format json
```

Для явного marker search используйте `--marker`:

```bash
mge recall "technical answer style" --marker kind:user_preference --marker scope:global
```

`mge config set` меняет defaults и легкие derived indexes. Существующие page files не переписываются; каждая catalog entry хранит codec/compression, нужные для чтения этой страницы. При смене `--index-kind` пересобирается только candidate page index по существующим sealed pages.

`BinaryFusePageIndex` - probabilistic candidate page filter, а не inverted `marker -> pages` map. Он строит один реальный `xorf::BinaryFuse16` static filter на каждую sealed page по ее `marker_summary`, сканирует page filters при query и может вернуть extra candidate pages. `ExactMarkerPageIndex` остается default для стабильного дебага.

Deprecated/rejected memories и `SecretReference` cells фильтруются по умолчанию. Recall opt-in flags стоит использовать только если у caller есть явная причина и capability.

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
