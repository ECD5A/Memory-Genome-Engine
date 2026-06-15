# SDK

[English version](SDK.md)

Мандат 2 начинается с thin SDK wrappers. Это integration helpers, а не альтернативные engines. Storage, recall, indexing, validation и sealing logic остаются в Rust.

## Python

Где находится:

```text
sdk/python/mge_sdk/__init__.py
sdk/python/pyproject.toml
examples/python_basic_usage.py
```

Запуск примера из корня репозитория:

```bash
python examples/python_basic_usage.py
python examples/python_agent_host.py
```

Опциональная editable install из репозитория:

```bash
python -m pip install -e sdk/python
python -c "import mge_sdk; print(mge_sdk.MemoryGenomeClient)"
```

Базовая форма:

```python
from mge_sdk import MemoryGenomeClient

client = MemoryGenomeClient(".memory-genome", passphrase_env="MGE_PASSPHRASE")
client.init(profile="fast", encrypted=True)
security = client.security_config()
client.remember("Agent should use recalled context.", kind="procedure", scope="agent")
packet = client.recall("agent context", mode="focused", scope="agent")
client.checkpoint()
client.seal()
client.validate(deep=True)
client.rebuild_indexes()
client.export_markdown()
```

Typed SDK surface:

- `RecallMode`
- `RememberOptions`
- `ContextMemoryItem`
- `ContextPacket`
- `StoreStats`
- `ValidationReport`
- `McpError`
- `MgeCommandError`
- `MgeProtocolError`
- `result_or_raise_mcp_error(response)`

По умолчанию wrapper запускает `mge` из `PATH`. Для локальной разработки передайте command явно:

```python
MemoryGenomeClient(
    ".memory-genome",
    command=["cargo", "run", "-q", "-p", "mge-cli", "--bin", "mge", "--"],
    cwd=repo_root,
)
```

## TypeScript

Где находится:

```text
sdk/typescript/src/mge.ts
sdk/typescript/package.json
sdk/typescript/tsconfig.json
examples/typescript_basic_usage.ts
```

Запуск примера на Node version с TypeScript stripping:

```bash
node examples/typescript_basic_usage.ts
node examples/typescript_agent_host.ts
```

Опциональный package smoke из репозитория:

```bash
cd sdk/typescript
npm run smoke
npm run check # если локально установлен tsc
```

Базовая форма:

```typescript
import { MemoryGenomeClient } from "./sdk/typescript/src/mge.ts";

const client = new MemoryGenomeClient(".memory-genome", {
  passphraseEnv: "MGE_PASSPHRASE",
});
client.init("fast", { encrypted: true });
const security = client.securityConfig();
client.remember("Agent should use recalled context.", {
  kind: "procedure",
  scope: "agent",
});
const packet = client.recall("agent context", {
  mode: "focused",
  scope: "agent",
});
client.checkpoint();
client.seal();
client.validate({ deep: true });
client.rebuildIndexes();
client.exportMarkdown();
```

TypeScript wrapper использует только Node built-ins и вызывает Rust CLI.

Typed SDK surface:

- `RecallMode`
- `RememberOptions`
- `RecallOptions`
- `ContextMemoryItem`
- `ContextPacket`
- `StoreStats`
- `ValidationReport`
- `McpStructuredError`
- `MemoryGenomeCommandError`
- `MemoryGenomeProtocolError`
- `resultOrThrowMcpError(response)`

## Supported Operations

Оба wrapper-а поддерживают:

- init/open store
- opt-in encrypted store init through passphrase env
- encrypted hot log/snapshot unlock pass-through
- security config readout
- remember
- focused/broad/full-scope recall
- seal
- checkpoint
- stats
- validate / validate deep
- rebuild indexes
- export Markdown

## Agent Loop Examples

Host examples имитируют локального агента без внешних API:

- Python: `examples/python_agent_host.py`
- TypeScript: `examples/typescript_agent_host.ts`

Каждый пример следует flow:

```text
init/open store -> recall focused -> fake local work -> remember -> checkpoint -> recall broad -> seal -> recall sealed -> validate deep
```

## Structured Errors

CLI wrappers выбрасывают command errors при failed local process execution. Если host говорит напрямую с `mge-mcp-server`, оба SDK имеют helpers, которые превращают structured JSON-RPC error в typed protocol exception.

Error fields:

- `code`
- `message`
- `tool_name`
- `recoverable`
- `protocol_version`
- `integration_schema_version`
- optional `details`

## Versioning

SDK targets: `protocol_version = mge-jsonrpc-1` и `integration_schema_version = 1`. Это integration contract versions, они не меняют binary storage layout или storage version.

## Troubleshooting

- Если SDK не находит `mge`, передайте `command=["cargo", "run", "-q", "-p", "mge-cli", "--bin", "mge", "--"]` и `cwd=repo_root`.
- Если TypeScript type checking недоступен, запускайте runtime smoke: `node examples/typescript_basic_usage.ts`; `tsc` для локального wrapper опционален.
- Если JSON-RPC call завершился ошибкой, смотрите `error.details.error_kind`: invalid args дают `invalid_params`, missing store даёт `store_open_failed`, encrypted store без session unlock даёт `store_locked`, wrong passphrase даёт `auth_failed`, `full_scope` без scope даёт `invalid_request`, malformed JSON даёт `parse_error`.
- Security note: SDK передают только имя passphrase environment variable. Они не должны логировать passphrase или raw key material. Crypto и storage остаются в Rust core/CLI. Encrypted sealed recall, `validate(deep=True)` и `rebuild_indexes()` требуют тот же passphrase environment path, что и hot-memory operations.
- Не читайте и не меняйте `.memory-genome` files из SDK. SDK являются process wrappers вокруг Rust engine.

## Policy

- Rust остаётся core.
- SDK не дублируют memory logic.
- SDK JSON - это protocol/debug output от CLI, а не runtime storage.
- SDK не добавляют network, dependency install, UI, encryption, vector DB, custom codec или новые filters.
