# SDKs

[Russian version](SDK.ru.md)

Mandate 2 starts with thin SDK wrappers. They are integration helpers, not alternate engines. All storage, recall, indexing, validation, and sealing logic remains in Rust.

## Python

Location:

```text
sdk/python/mge_sdk/__init__.py
examples/python_basic_usage.py
```

Run the example from the repository root:

```bash
python examples/python_basic_usage.py
```

Basic shape:

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

By default the wrapper runs `mge` from `PATH`. During local development pass a command explicitly:

```python
MemoryGenomeClient(
    ".memory-genome",
    command=["cargo", "run", "-q", "-p", "mge-cli", "--bin", "mge", "--"],
    cwd=repo_root,
)
```

## TypeScript

Location:

```text
sdk/typescript/src/mge.ts
examples/typescript_basic_usage.ts
```

Run the example with a Node version that supports TypeScript stripping:

```bash
node examples/typescript_basic_usage.ts
```

Basic shape:

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

The TypeScript wrapper uses Node built-ins only and shells out to the Rust CLI.

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

Both wrappers cover:

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

The CLI wrappers raise command errors for failed local process execution. If a host talks directly to `mge-mcp-server`, both SDKs include helpers that map the structured JSON-RPC error into a typed protocol exception.

Error fields:

- `code`
- `message`
- `tool_name`
- `recoverable`
- `protocol_version`
- `integration_schema_version`
- optional `details`

## Versioning

SDKs target `protocol_version = mge-jsonrpc-1` and `integration_schema_version = 1`. These are integration contract versions and do not change the binary storage layout or storage version.

## Policy

- Rust remains the core.
- SDKs do not duplicate memory logic.
- SDK JSON is protocol/debug output from the CLI, not runtime storage.
- No network, dependency install, UI, encryption, vector DB, custom codec, or new filters are introduced by these SDKs.
