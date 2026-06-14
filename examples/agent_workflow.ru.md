# Agent Memory Workflow

[English version](agent_workflow.md)

Этот пример показывает ожидаемый agent loop:

1. Получить задачу.
2. Запросить task-relevant memory.
3. Работать, используя `ContextPacket`.
4. Сохранить полезный результат.
5. При необходимости вызвать checkpoint или seal.

## CLI Agent Flow

```bash
mge init --profile fast

mge recall "finish Mandate 2 integration docs" \
  --mode focused \
  --scope mandate_2 \
  --json

# Агент работает, используя returned ContextPacket.

mge remember "Mandate 2 integration docs were updated with MCP and SDK usage." \
  --kind task_state \
  --scope mandate_2 \
  --trust tool_observed \
  --marker topic:agent_integration

mge checkpoint
mge seal
```

## MCP-Style Flow

Отправляйте по одному JSON-RPC request на строку в `mge-mcp-server`:

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

# Агент работает с packet["relevant_memory"].

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

// Агент работает с packet.relevant_memory.

client.remember("Mandate 2 integration docs were updated with MCP and SDK usage.", {
  kind: "task_state",
  scope: "mandate_2",
  markers: ["topic:agent_integration"],
  trust: "tool_observed",
});
client.checkpoint();
```

JSON в этих примерах является только protocol/debug output, а не runtime storage.
