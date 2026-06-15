# Интеграция

[English version](INTEGRATION.md)

Memory Genome Engine подключается как local-first сервис памяти для агентов. Агент не читает storage files напрямую. Он вызывает integration boundary, получает `ContextPacket`, работает с этим контекстом и сохраняет полезный результат обратно через `remember`.

Core flow не меняется:

```text
remember -> L1 Hot RAM + pending persistence -> hot/hot.mgl
seal -> Sealed Pages + Indexes
recall -> ContextPacket
```

## Agent Lifecycle

1. Агент получает задачу.
2. Агент вызывает `recall` в режиме `focused`, `broad` или явном `full_scope`.
3. Memory Genome Engine возвращает `ContextPacket`.
4. Агент использует packet как task-relevant memory.
5. Агент сохраняет важные выводы, решения, preferences или task state через `remember`.
6. Агент может вызвать `checkpoint` для durable hot memory или `seal`, чтобы перенести hot cells в sealed pages.

## Agent Host Pattern

Host отвечает за orchestration. Memory Genome Engine отвечает за память:

```text
host starts task
-> recall focused или broad
-> host делает local work с ContextPacket
-> remember useful result
-> checkpoint для hot durability
-> recall again, если task продолжается
-> seal, когда task/session boundary стабилен
-> validate --deep в smoke или maintenance flows
```

Recall modes используйте консервативно:

- `focused`: default для узкого вопроса, next action или small tool decision.
- `broad`: project/module/task planning, когда агенту нужно больше related memory.
- `full_scope`: explicit audit/export/review внутри известного scope; всегда передавайте `scope`.

Local host examples:

- Rust/CLI process host: `examples/agent_host_cli.rs`
- Python SDK host: `examples/python_agent_host.py`
- TypeScript SDK host: `examples/typescript_agent_host.ts`
- MCP JSON-RPC transcript: `examples/mcp_agent_session.jsonl`

## ContextPacket Contract

`ContextPacket` - главный результат recall для integration layer. В нём есть:

- `query`: текст запроса.
- `relevant_memory`: memory items с kind, content, trust, status, scope, sensitivity и marker strings.
- `constraints`: ограничения, которые агент должен учитывать.
- `warnings`: предупреждения безопасности или recall.
- `debug`: recall mode, index kind, page/cell counters, cache counters и timing breakdown.

Packet должен быть task-relevant и size-controlled, но не обязательно маленьким. `focused` узкий, `broad` шире, `full_scope` требует явный scope.

MCP/SDK recall responses сохраняют core `ContextPacket` в `context_packet` и дополнительно дают stable adapter wrapper в `context`:

- `query`
- `mode`
- `relevant_memory`
- `constraints`
- `warnings`
- `score_details`
- `debug`
- `store_stats`

## Integration Boundaries

Используйте самый простой подход для host:

- Rust: напрямую `mge-core::MemoryEngine`.
- CLI: binary `mge` для scripts и shell-based agents.
- MCP-style: `mge-mcp-server` для JSON-RPC over stdin/stdout.
- Python: thin wrapper в `sdk/python`.
- TypeScript: thin wrapper в `sdk/typescript`.

Rust остаётся core. Python и TypeScript wrappers вызывают Rust CLI и не дублируют memory logic.

## Unlock Для Encrypted Store

Encrypted stores используют те же integration paths. Host передаёт только имя environment variable, где лежит passphrase:

- CLI: `--passphrase-env MGE_PASSPHRASE`
- MCP JSON-RPC params: `"passphrase_env": "MGE_PASSPHRASE"`
- Python SDK: `MemoryGenomeClient(..., passphrase_env="MGE_PASSPHRASE")`
- TypeScript SDK: `new MemoryGenomeClient(..., { passphraseEnv: "MGE_PASSPHRASE" })`

Значение passphrase не должно попадать в protocol payloads и logs. Сейчас encryption покрывает `hot/hot.mgl` и `hot/snapshot.mgs`; sealed pages и indexes будут отдельным security package.

## Local Developer Setup

Сначала соберите Rust tools:

```bash
cargo build
```

Запуск MCP-ready adapter:

```bash
cargo run -p mge-cli --bin mge-mcp-server
```

SDK smoke из корня репозитория:

```bash
python examples/python_basic_usage.py
node examples/typescript_basic_usage.ts
```

Опциональные локальные packaging checks:

```bash
python -m pip install -e sdk/python
cd sdk/typescript
npm run smoke
npm run check # если tsc доступен
```

Текущее решение: оставить versioned JSON-RPC stdin/stdout adapter основной local MCP-ready поверхностью. Полная external MCP SDK dependency отложена до момента, когда contract реально потребует host-specific transport features.

## Versioning

Текущий integration contract использует:

- `protocol_version`: `mge-jsonrpc-1`
- `integration_schema_version`: `1`

Эти fields version только MCP/SDK protocol contract. Они не меняют binary storage format, page codec, filter strategy или recall semantics.

## Local-First Safety

- Интернет не нужен.
- Corpus files не исполняются.
- Store открывается только по explicit path.
- Markdown export пишет в `exports/memory.md` или в explicit output path adapter-а.
- JSON используется только как CLI/MCP/SDK protocol output и debug report. Это не runtime storage.
- Runtime storage остаётся binary: `manifest.mgm`, `dictionary/markers.mgd`, `hot/hot.mgl`, `pages/*.mgp`, `indexes/*.mgi`.

## Current Limits

- `mge-mcp-server` сейчас MCP-ready local JSON-RPC adapter, а не полная внешняя MCP SDK реализация.
- SDK пока thin local wrappers вокруг `mge`; package publishing ещё не делался.
- Sealed page encryption, vector DB, UI и remote service hosting не входят в текущий integration layer.

## Troubleshooting

- Если wrapper не находит `mge`, передайте cargo command из репозитория явно.
- Если MCP request падает, сначала смотрите structured `error.details.error_kind`, а не только human-readable message.
- Если `full_scope` recall падает, передайте `scope`; это требование защищает от случайной выдачи слишком широкой памяти.
- Если Markdown export пишет не туда, передайте `output_path` явно или используйте default store `exports/memory.md`.
- JSON-RPC/CLI JSON - это protocol/debug output. Runtime storage остаётся binary.
