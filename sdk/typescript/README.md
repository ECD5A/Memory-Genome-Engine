# Memory Genome Engine TypeScript SDK

This package is a thin local wrapper around the Rust `mge` CLI. It does not implement storage, recall, indexing, sealing, or validation logic in TypeScript.

JSON returned by the CLI is protocol/debug output only. Runtime storage remains binary.

## Local Use From The Repository

Run the example from the repository root with a Node version that supports TypeScript stripping:

```bash
node examples/typescript_basic_usage.ts
```

Use the checked-out Rust CLI during development:

```typescript
import { MemoryGenomeClient } from "./sdk/typescript/src/mge.ts";

const client = new MemoryGenomeClient(".memory-genome", {
  command: ["cargo", "run", "-q", "-p", "mge-cli", "--bin", "mge", "--"],
  cwd: process.cwd(),
});
```

## Optional Type Check

If `tsc` is available locally:

```bash
cd sdk/typescript
npm run check
```

No package has been published and no dependency install is required for the example.

## Smoke

```bash
npm run smoke
```

## Errors

- `MemoryGenomeCommandError`: local CLI process failed.
- `MemoryGenomeProtocolError`: structured JSON-RPC/MCP adapter error.

Use `resultOrThrowMcpError(response)` when talking directly to `mge-mcp-server`.
