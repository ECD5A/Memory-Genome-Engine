# SDK

[English version](SDK.md)

Мандат 2 начинается с thin SDK wrappers. Это integration helpers, а не альтернативные engines. Storage, recall, indexing, validation и sealing logic остаются в Rust.

## Python

Где находится:

```text
sdk/python/mge_sdk/__init__.py
examples/python_basic_usage.py
```

Запуск примера из корня репозитория:

```bash
python examples/python_basic_usage.py
```

Базовая форма:

```python
from mge_sdk import MemoryGenomeClient

client = MemoryGenomeClient(".memory-genome")
client.init(profile="fast")
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
examples/typescript_basic_usage.ts
```

Запуск примера на Node version с TypeScript stripping:

```bash
node examples/typescript_basic_usage.ts
```

Базовая форма:

```typescript
import { MemoryGenomeClient } from "./sdk/typescript/src/mge.ts";

const client = new MemoryGenomeClient(".memory-genome");
client.init("fast");
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
- remember
- focused/broad/full-scope recall
- seal
- checkpoint
- stats
- validate / validate deep
- rebuild indexes
- export Markdown

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

## Policy

- Rust остаётся core.
- SDK не дублируют memory logic.
- SDK JSON - это protocol/debug output от CLI, а не runtime storage.
- SDK не добавляют network, dependency install, UI, encryption, vector DB, custom codec или новые filters.
