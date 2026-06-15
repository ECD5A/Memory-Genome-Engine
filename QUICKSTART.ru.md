# Quickstart

[English version](QUICKSTART.md)

Этот guide показывает короткий путь от clean checkout до working local Memory Genome store.

## Build

```bash
cargo build
```

Главный CLI binary - `mge`:

```bash
cargo run -p mge-cli -- --help
```

## Создать Store

Default store:

```bash
cargo run -p mge-cli -- init
```

Fast profile с compact sealed pages:

```bash
cargo run -p mge-cli -- init --profile fast
```

Runtime store бинарный. JSON не используется как runtime storage.

## Remember

```bash
cargo run -p mge-cli -- remember "User prefers concise technical explanations" \
  --kind user_preference \
  --scope global \
  --trust user_confirmed
```

Structured value:

```bash
cargo run -p mge-cli -- remember \
  --kind user_preference \
  --subject answer_style \
  --json-value '{"style":"concise","max_examples":2}'
```

Reference value для sensitive placeholders:

```bash
cargo run -p mge-cli -- remember \
  --kind project_fact \
  --reference-value vault://references/api-key \
  --sensitivity secret_reference
```

Provenance и links:

```bash
cargo run -p mge-cli -- remember "Decision recorded" \
  --kind decision \
  --source-type issue \
  --source-ref MGE-1 \
  --link 1
```

## Recall

```bash
cargo run -p mge-cli -- recall "How should the agent answer technical questions?"
```

Recall modes:

```bash
cargo run -p mge-cli -- recall "technical answer style" --mode focused
cargo run -p mge-cli -- recall "project context" --mode broad
cargo run -p mge-cli -- recall --mode full-scope --scope global
```

Explicit markers для deterministic recall:

```bash
cargo run -p mge-cli -- recall "answer style" --marker kind:user_preference --marker scope:global
```

## Seal, Validate, Export

```bash
cargo run -p mge-cli -- seal
cargo run -p mge-cli -- recall "How should the agent answer technical questions?"
cargo run -p mge-cli -- stats
cargo run -p mge-cli -- validate --deep
cargo run -p mge-cli -- rebuild-indexes
cargo run -p mge-cli -- export
```

Markdown export human-readable и plaintext by design.

## Encrypted Store

Encrypted mode включается явно:

```bash
export MGE_PASSPHRASE="use-a-real-secret"
cargo run -p mge-cli -- init --encrypted --passphrase-env MGE_PASSPHRASE
cargo run -p mge-cli -- remember "private memory" --passphrase-env MGE_PASSPHRASE
cargo run -p mge-cli -- checkpoint --passphrase-env MGE_PASSPHRASE
cargo run -p mge-cli -- seal --passphrase-env MGE_PASSPHRASE
cargo run -p mge-cli -- recall "private memory" --passphrase-env MGE_PASSPHRASE
cargo run -p mge-cli -- validate --deep --passphrase-env MGE_PASSPHRASE
```

Encrypted mode защищает hot log payloads, checkpoint payloads и sealed page payloads. Marker dictionary, indexes, catalog summaries, Markdown export и process memory остаются plaintext by design. Подробно: [Security](docs/SECURITY.ru.md).

## Storage Defaults

Compact page storage для нового store:

```bash
cargo run -p mge-cli -- init --page-codec messagepack --compression zstd
```

Изменить defaults для future sealed pages в existing store:

```bash
cargo run -p mge-cli -- config show
cargo run -p mge-cli -- config set --page-codec messagepack --compression zstd
cargo run -p mge-cli -- config set --page-clusterer marker_overlap
```

Existing sealed pages не переписываются при смене config defaults.

## Optional Binary Fuse Index

Exact marker page index является default. Binary Fuse включается явно:

```bash
cargo run -p mge-cli -- init --index-kind binary_fuse_page
cargo run -p mge-cli -- config set --index-kind binary_fuse_page
```

Смена index kind rebuild-ит candidate indexes по existing sealed pages; page payloads не переписываются.

## Local Integration

Запустить JSON-RPC adapter:

```bash
cargo run -p mge-cli --bin mge-mcp-server
```

Запустить SDK examples:

```bash
python examples/python_basic_usage.py
node examples/typescript_basic_usage.ts
```

Подробнее:

- [Интеграция / MCP / SDK](docs/INTEGRATION.ru.md)
