declare const process: any;

import { mkdtempSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { MemoryGenomeClient } from "../sdk/typescript/src/mge.ts";

const root = process.cwd();
const storePath = join(mkdtempSync(join(tmpdir(), "mge-typescript-example-")), ".memory-genome");
const command = process.env.MGE_BIN
  ? [process.env.MGE_BIN]
  : ["cargo", "run", "-q", "-p", "mge-cli", "--bin", "mge", "--"];

const client = new MemoryGenomeClient(storePath, {
  command,
  cwd: root,
});

client.init("fast");
const cellId = client.remember(
  "Agent should recall ContextPacket memory before editing the project.",
  {
    kind: "procedure",
    scope: "mandate_2",
    markers: ["topic:agent_integration"],
    trust: "user_confirmed",
    sensitivity: "private",
  },
);

const hotPacket = client.recall("agent integration context packet", {
  mode: "focused",
  scope: "mandate_2",
  maxItems: 3,
});
if (hotPacket.relevant_memory.length === 0) {
  throw new Error("expected hot recall result");
}

const checkpoint = client.checkpoint();
if (checkpoint.hot_cells !== 1) {
  throw new Error("expected one hot cell at checkpoint");
}

const seal = client.seal();
if (seal.hot_cells_sealed !== 1) {
  throw new Error("expected one sealed cell");
}

const sealedPacket = client.recall("agent integration context packet", {
  mode: "broad",
  scope: "mandate_2",
  maxItems: 5,
});
if (sealedPacket.relevant_memory.length === 0) {
  throw new Error("expected sealed recall result");
}

const validation = client.validate({ deep: true });
if (validation.ok !== true) {
  throw new Error("expected valid store");
}

const rebuild = client.rebuildIndexes();
if (rebuild.pages_unchanged !== true) {
  throw new Error("expected pages to remain unchanged");
}

const markdownPath = client.exportMarkdown();

console.log(
  `typescript sdk example ok: cell=${cellId}, items=${sealedPacket.relevant_memory.length}, markdown=${markdownPath}`,
);
