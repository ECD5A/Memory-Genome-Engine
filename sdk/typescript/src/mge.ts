declare const process: any;

import { copyFileSync, mkdirSync } from "node:fs";
import { dirname, join } from "node:path";
import { spawnSync } from "node:child_process";

export const PROTOCOL_VERSION = "mge-jsonrpc-1";
export const INTEGRATION_SCHEMA_VERSION = 1;

export type RecallMode = "focused" | "broad" | "full_scope" | "full-scope";

export interface MemoryGenomeClientOptions {
  command?: string[];
  cwd?: string;
}

export interface RememberOptions {
  kind?: string;
  scope?: string;
  markers?: string[];
  trust?: string;
  sensitivity?: string;
  status?: string;
  subject?: string;
}

export interface RecallOptions {
  mode?: RecallMode;
  scope?: string;
  markers?: string[];
  maxItems?: number;
  kind?: string;
}

export interface ContextMemoryItem {
  kind: string;
  content: string;
  trust: string;
  status: string;
  scope: string;
  sensitivity: string;
  markers: string[];
}

export interface ContextPacketDebug {
  recall_mode?: string;
  max_items?: number;
  index_kind?: string;
  returned_items?: number;
  total_recall_micros?: number;
  score_details?: unknown[];
  [key: string]: unknown;
}

export interface ContextPacket {
  query: string;
  relevant_memory: ContextMemoryItem[];
  constraints: string[];
  warnings: string[];
  debug: ContextPacketDebug;
}

export interface StoreStats {
  hot_cells: number;
  sealed_pages: number;
  sealed_cells: number;
  marker_count: number;
  current_index_kind: string;
  store_size_bytes: number;
  [key: string]: unknown;
}

export interface ValidationReport {
  ok: boolean;
  index_kind: string;
  checked_hot_cells: number;
  checked_sealed_pages: number;
  checked_sealed_cells: number;
  errors: string[];
  warnings: string[];
}

export interface McpStructuredError {
  code: number;
  message: string;
  tool_name: string;
  recoverable: boolean;
  protocol_version: string;
  integration_schema_version: number;
  details?: Record<string, unknown>;
}

export interface McpJsonRpcResponse<T = unknown> {
  jsonrpc: "2.0";
  id?: unknown;
  result?: T;
  error?: McpStructuredError;
}

export class MemoryGenomeCommandError extends Error {
  command: string[];
  status: number | null;
  stdout: string;
  stderr: string;

  constructor(command: string[], status: number | null, stdout: string, stderr: string) {
    super(`Memory Genome command failed with exit code ${status}: ${command.join(" ")}\n${stderr}`);
    this.command = command;
    this.status = status;
    this.stdout = stdout;
    this.stderr = stderr;
  }
}

export class MemoryGenomeProtocolError extends Error {
  code: number;
  toolName: string;
  recoverable: boolean;
  details?: Record<string, unknown>;

  constructor(error: McpStructuredError) {
    super(`${error.tool_name}: ${error.message}`);
    this.code = error.code;
    this.toolName = error.tool_name;
    this.recoverable = error.recoverable;
    this.details = error.details;
  }
}

export class MemoryGenomeClient {
  private storePath: string;
  private command: string[];
  private cwd?: string;

  constructor(storePath: string, options: MemoryGenomeClientOptions = {}) {
    this.storePath = storePath;
    this.command = options.command ?? commandFromEnv();
    this.cwd = options.cwd;
  }

  init(profile = "fast"): string {
    return this.runText(["init", "--profile", profile]);
  }

  remember(content: string, options: RememberOptions = {}): number {
    const args = [
      "remember",
      content,
      "--kind",
      options.kind ?? "temporary_note",
      "--scope",
      options.scope ?? "global",
      "--trust",
      options.trust ?? "agent_inferred",
      "--sensitivity",
      options.sensitivity ?? "private",
      "--status",
      options.status ?? "active",
    ];
    if (options.subject) {
      args.push("--subject", options.subject);
    }
    for (const marker of options.markers ?? []) {
      args.push("--marker", marker);
    }

    const output = this.runText(args);
    const match = output.match(/Remembered cell (\d+)/);
    if (!match) {
      throw new Error(`could not parse remembered cell id from: ${output}`);
    }
    return Number(match[1]);
  }

  recall(query = "", options: RecallOptions = {}): ContextPacket {
    const args = ["recall"];
    if (query.length > 0) {
      args.push(query);
    }
    args.push(
      "--mode",
      options.mode ?? "focused",
      "--max-items",
      String(options.maxItems ?? 5),
      "--json",
    );
    if (options.scope) {
      args.push("--scope", options.scope);
    }
    if (options.kind) {
      args.push("--kind", options.kind);
    }
    for (const marker of options.markers ?? []) {
      args.push("--marker", marker);
    }
    return this.runJson(args);
  }

  seal(): unknown {
    return this.runJson(["seal"]);
  }

  checkpoint(): unknown {
    return this.runJson(["checkpoint", "--json"]);
  }

  stats(): StoreStats {
    return this.runJson(["stats", "--json"]);
  }

  validate(options: { deep?: boolean } = {}): ValidationReport {
    const args = ["validate", "--json"];
    if (options.deep) {
      args.splice(1, 0, "--deep");
    }
    return this.runJson(args, true);
  }

  rebuildIndexes(): unknown {
    return this.runJson(["rebuild-indexes", "--json"]);
  }

  exportMarkdown(outputPath?: string): string {
    this.runText(["export", "--format", "markdown"]);
    const defaultPath = join(this.storePath, "exports", "memory.md");
    if (!outputPath) {
      return defaultPath;
    }
    mkdirSync(dirname(outputPath), { recursive: true });
    copyFileSync(defaultPath, outputPath);
    return outputPath;
  }

  private runJson(args: string[], allowFailure = false): any {
    return JSON.parse(this.runText(args, allowFailure));
  }

  private runText(args: string[], allowFailure = false): string {
    const command = [...this.command, "--store", this.storePath, ...args];
    const completed = spawnSync(command[0], command.slice(1), {
      cwd: this.cwd,
      encoding: "utf8",
    });
    if (completed.error) {
      throw completed.error;
    }
    if (completed.status !== 0 && !allowFailure) {
      throw new MemoryGenomeCommandError(
        command,
        completed.status,
        completed.stdout,
        completed.stderr,
      );
    }
    return completed.stdout;
  }
}

function commandFromEnv(): string[] {
  const configured = typeof process !== "undefined" ? process.env.MGE_CLI : undefined;
  if (configured && configured.trim().length > 0) {
    return configured.trim().split(/\s+/);
  }
  return ["mge"];
}

export function resultOrThrowMcpError<T>(response: McpJsonRpcResponse<T>): T {
  if (response.error) {
    throw new MemoryGenomeProtocolError(response.error);
  }
  if (response.result !== undefined) {
    return response.result;
  }
  throw new MemoryGenomeProtocolError({
    code: -32603,
    message: "JSON-RPC response has no result or structured error",
    tool_name: "unknown",
    recoverable: false,
    protocol_version: PROTOCOL_VERSION,
    integration_schema_version: INTEGRATION_SCHEMA_VERSION,
  });
}
