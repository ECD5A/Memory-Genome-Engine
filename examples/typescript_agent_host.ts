declare const process: any;

import { mkdtempSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { MemoryGenomeClient } from "../sdk/typescript/src/mge.ts";

const root = process.cwd();
const storePath = join(mkdtempSync(join(tmpdir(), "mge-typescript-agent-host-")), ".memory-genome");
const command = process.env.MGE_BIN
  ? [process.env.MGE_BIN]
  : ["cargo", "run", "-q", "-p", "mge-cli", "--bin", "mge", "--"];

const client = new MemoryGenomeClient(storePath, {
  command,
  cwd: root,
});

client.init("fast");

const session = client.rememberSession(
  [
    { role: "user", content: "Prepare the local integration release" },
    { role: "assistant", content: "Use the existing agent host contract" },
    { role: "user", content: "Keep the verification result for recall" },
  ],
  {
    sessionId: "typescript-agent-host",
    scope: "agent_demo",
    markers: ["topic:agent_host"],
    maxTurns: 2,
  },
);
if (session.chunks !== 2) {
  throw new Error("expected two deterministic session chunks");
}

const task = "prepare local agent host integration smoke";
const focusedPacket = client.recall(task, {
  mode: "focused",
  scope: "agent_demo",
  maxItems: 5,
});
if (focusedPacket.debug.recall_mode !== "focused") {
  throw new Error("expected focused recall mode");
}

// Fake local work. No external LLM/API call is made here.
const workResult = "TypeScript agent host completed a fake integration task using ContextPacket memory.";
const cellId = client.remember(workResult, {
  kind: "tool_result",
  scope: "agent_demo",
  markers: ["topic:agent_host", "lang:typescript"],
  trust: "tool_observed",
  sensitivity: "private",
});

const checkpoint = client.checkpoint() as { hot_cells: number };
if (checkpoint.hot_cells !== 3) {
  throw new Error("expected three hot cells at checkpoint");
}

const broadPacket = client.recall("agent host integration task", {
  mode: "broad",
  scope: "agent_demo",
  maxItems: 10,
});
if (!broadPacket.relevant_memory.some((item) => item.content === workResult)) {
  throw new Error("expected broad recall to include fake work result");
}

const seal = client.seal() as { hot_cells_sealed: number };
if (seal.hot_cells_sealed !== 3) {
  throw new Error("expected three sealed hot cells");
}

const sealedPacket = client.recall("agent host integration task", {
  mode: "focused",
  scope: "agent_demo",
  maxItems: 5,
});
if (!sealedPacket.relevant_memory.some((item) => item.content === workResult)) {
  throw new Error("expected sealed recall to include fake work result");
}

const validation = client.validate({ deep: true });
if (!validation.ok) {
  throw new Error(`expected valid store: ${validation.errors.join("; ")}`);
}

console.log(
  `typescript agent host example ok: cell=${cellId}, sealed_items=${sealedPacket.relevant_memory.length}, store=${storePath}`,
);
