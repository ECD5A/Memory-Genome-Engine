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

## Policy

- Rust остаётся core.
- SDK не дублируют memory logic.
- SDK JSON - это protocol/debug output от CLI, а не runtime storage.
- SDK не добавляют network, dependency install, UI, encryption, vector DB, custom codec или новые filters.
