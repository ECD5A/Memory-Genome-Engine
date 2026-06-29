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

Store a short agent session with deterministic production chunking:

```typescript
client.rememberSession(
  [
    { role: "user", content: "Keep release rollback steps." },
    { role: "assistant", content: "Validate before publishing." },
  ],
  { scope: "release", sessionId: "release-review", maxTurns: 4 },
);
const packet = client.recall("release rollback", { scope: "release", maxItems: 5 });
```

Four-turn chunks are the measured compact option; the SDK default remains the quality-first eight turns.

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
node ../../examples/typescript_agent_host.ts
```

## Errors

- `MemoryGenomeCommandError`: local CLI process failed.
- `MemoryGenomeProtocolError`: structured JSON-RPC/MCP adapter error.

Use `resultOrThrowMcpError(response)` when talking directly to `mge-mcp-server`.

`client.recall(..., { minScore: 20 })` can be used by agent hosts that prefer no memory over weak matches. The score floor is opt-in.

## Feedback

Use the [integration report](https://github.com/ECD5A/Memory-Genome-Engine/issues/new?template=integration_report.yml) for a tested host workflow, the [general feedback form](https://github.com/ECD5A/Memory-Genome-Engine/issues/new?template=general_feedback.yml) for short usability notes, or [Q&A](https://github.com/ECD5A/Memory-Genome-Engine/discussions/categories/q-a) for setup help.
