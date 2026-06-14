declare const process: any;

import { copyFileSync, mkdirSync } from "node:fs";
import { dirname, join } from "node:path";
import { spawnSync } from "node:child_process";

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

  recall(query = "", options: RecallOptions = {}): any {
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

  seal(): any {
    return this.runJson(["seal"]);
  }

  checkpoint(): any {
    return this.runJson(["checkpoint", "--json"]);
  }

  stats(): any {
    return this.runJson(["stats", "--json"]);
  }

  validate(options: { deep?: boolean } = {}): any {
    const args = ["validate", "--json"];
    if (options.deep) {
      args.splice(1, 0, "--deep");
    }
    return this.runJson(args, true);
  }

  rebuildIndexes(): any {
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
