# SDKs

[Russian version](SDK.ru.md)

Mandate 2 starts with thin SDK wrappers. They are integration helpers, not alternate engines. All storage, recall, indexing, validation, and sealing logic remains in Rust.

## Python

Location:

```text
sdk/python/mge_sdk/__init__.py
sdk/python/pyproject.toml
examples/python_basic_usage.py
```

Run the example from the repository root:

```bash
python examples/python_basic_usage.py
python examples/python_agent_host.py
```

Optional editable install from the repository:

```bash
python -m pip install -e sdk/python
python -c "import mge_sdk; print(mge_sdk.MemoryGenomeClient)"
```

Basic shape:

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
sdk/typescript/package.json
sdk/typescript/tsconfig.json
examples/typescript_basic_usage.ts
```

Run the example with a Node version that supports TypeScript stripping:

```bash
node examples/typescript_basic_usage.ts
node examples/typescript_agent_host.ts
```

Optional package smoke from the repository:

```bash
cd sdk/typescript
npm run smoke
npm run check # only if tsc is installed locally
```

Basic shape:

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
- opt-in encrypted store init through passphrase env
- encrypted hot log/snapshot/sealed-page unlock pass-through
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

The host examples simulate a local agent without calling external APIs:

- Python: `examples/python_agent_host.py`
- TypeScript: `examples/typescript_agent_host.ts`

Each example follows:

```text
init/open store -> recall focused -> fake local work -> remember -> checkpoint -> recall broad -> seal -> recall sealed -> validate deep
```

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

## Troubleshooting

- If the SDK cannot find `mge`, pass `command=["cargo", "run", "-q", "-p", "mge-cli", "--bin", "mge", "--"]` and `cwd=repo_root`.
- If TypeScript type checking is unavailable, run the runtime smoke with `node examples/typescript_basic_usage.ts`; `tsc` is optional for this repository-local wrapper.
- If a JSON-RPC call fails, inspect `error.details.error_kind`; invalid parameters use `invalid_params`, missing stores use `store_open_failed`, encrypted stores without session unlock use `store_locked`, wrong passphrases use `auth_failed`, missing full-scope scope uses `invalid_request`, and malformed JSON uses `parse_error`.
- Security note: SDKs pass only the passphrase environment variable name. They must not log passphrases or raw key material. Crypto and storage remain in the Rust core/CLI. Encrypted sealed recall, `validate(deep=True)`, and `rebuild_indexes()` require the same passphrase environment path as hot-memory operations.
- Do not inspect or modify `.memory-genome` files from the SDKs. The SDKs are process wrappers around the Rust engine.

## Policy

- Rust remains the core.
- SDKs do not duplicate memory logic.
- SDK JSON is protocol/debug output from the CLI, not runtime storage.
- No network, dependency install, UI, encryption, vector DB, custom codec, or new filters are introduced by these SDKs.
