# Agent Memory Workflow

[Russian version](agent_workflow.ru.md)

This example shows the expected agent loop:

1. Receive a task.
2. Recall task-relevant memory.
3. Work using the `ContextPacket`.
4. Store useful output.
5. Optionally checkpoint or seal.

## CLI Agent Flow

```bash
mge init --profile fast

mge recall "finish Mandate 2 integration docs" \
  --mode focused \
  --scope mandate_2 \
  --json

# Agent works using the returned ContextPacket.

mge remember "Mandate 2 integration docs were updated with MCP and SDK usage." \
  --kind task_state \
  --scope mandate_2 \
  --trust tool_observed \
  --marker topic:agent_integration

mge checkpoint
mge seal
```

## MCP-Style Flow

Send one JSON-RPC request per line to `mge-mcp-server`:

```json
{"jsonrpc":"2.0","id":1,"method":"mge_recall","params":{"store_path":".memory-genome","query":"finish Mandate 2 integration docs","mode":"focused","scope":"mandate_2","max_items":5}}
{"jsonrpc":"2.0","id":2,"method":"mge_remember","params":{"store_path":".memory-genome","content":"Mandate 2 integration docs were updated with MCP and SDK usage.","kind":"task_state","scope":"mandate_2","markers":["topic:agent_integration"],"trust":"tool_observed","sensitivity":"private"}}
{"jsonrpc":"2.0","id":3,"method":"mge_checkpoint","params":{"store_path":".memory-genome"}}
```

## Python Agent Flow

```python
packet = client.recall(
    "finish Mandate 2 integration docs",
    mode="focused",
    scope="mandate_2",
)

# Agent works using packet["relevant_memory"].

client.remember(
    "Mandate 2 integration docs were updated with MCP and SDK usage.",
    kind="task_state",
    scope="mandate_2",
    markers=["topic:agent_integration"],
    trust="tool_observed",
)
client.checkpoint()
```

## TypeScript Agent Flow

```typescript
const packet = client.recall("finish Mandate 2 integration docs", {
  mode: "focused",
  scope: "mandate_2",
});

// Agent works using packet.relevant_memory.

client.remember("Mandate 2 integration docs were updated with MCP and SDK usage.", {
  kind: "task_state",
  scope: "mandate_2",
  markers: ["topic:agent_integration"],
  trust: "tool_observed",
});
client.checkpoint();
```

JSON in these examples is protocol/debug output only, not runtime storage.
