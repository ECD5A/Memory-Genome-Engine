declare const process: {
  env: Record<string, string | undefined>;
  cwd(): string;
};

declare module "node:fs" {
  export function copyFileSync(from: string, to: string): void;
  export function mkdirSync(path: string, options?: { recursive?: boolean }): void;
}

declare module "node:path" {
  export function dirname(path: string): string;
  export function join(...parts: string[]): string;
}

declare module "node:child_process" {
  export interface SpawnSyncReturns {
    status: number | null;
    stdout: string;
    stderr: string;
    error?: Error;
  }

  export function spawnSync(
    command: string,
    args?: string[],
    options?: { cwd?: string; encoding?: string },
  ): SpawnSyncReturns;
}
