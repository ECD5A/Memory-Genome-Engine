#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

require_command() {
  local name="$1"
  if ! command -v "$name" >/dev/null 2>&1; then
    echo "missing required command: $name" >&2
    return 127
  fi
}

require_command cargo
require_command grep
require_command mktemp
require_command tee

echo "Building release binaries for smoke..."
cargo build -p mge-cli --bins --release

target_root="${CARGO_TARGET_DIR:-$repo_root/target}"
bin_dir="$target_root/release"

find_bin() {
  local name="$1"
  if [[ -x "$bin_dir/$name" ]]; then
    printf '%s\n' "$bin_dir/$name"
  elif [[ -x "$bin_dir/$name.exe" ]]; then
    printf '%s\n' "$bin_dir/$name.exe"
  else
    echo "missing release binary: $name" >&2
    return 1
  fi
}

mge_bin="$(find_bin mge)"
mcp_bin="$(find_bin mge-mcp-server)"
find_bin mge-synthetic-bench >/dev/null
find_bin mge-corpus-bench >/dev/null
echo "Development benchmark tools are build-checked but not installed by default."

"$mge_bin" --version >/dev/null
"$mge_bin" tui --help >/dev/null
"$mge_bin" setup --help >/dev/null

tmp_root="$(mktemp -d "${TMPDIR:-/tmp}/mge-release-smoke.XXXXXX")"
cleanup() {
  if [[ "${KEEP_MGE_SMOKE:-0}" != "1" ]]; then
    rm -rf "$tmp_root"
  else
    echo "Keeping smoke directory: $tmp_root"
  fi
}
trap cleanup EXIT

plain_store="$tmp_root/plain/.memory-genome"
encrypted_store="$tmp_root/encrypted/.memory-genome"

echo "CLI smoke..."
"$mge_bin" --store "$plain_store" init --profile fast
"$mge_bin" --store "$plain_store" remember "release smoke memory" --kind project_fact --scope release --trust tool_observed
"$mge_bin" --store "$plain_store" recall "release smoke" >/dev/null
"$mge_bin" --store "$plain_store" checkpoint >/dev/null
"$mge_bin" --store "$plain_store" seal >/dev/null
"$mge_bin" doctor --store "$plain_store" --deep >/dev/null
"$mge_bin" --store "$plain_store" validate --deep >/dev/null

echo "Encrypted smoke..."
export MGE_RELEASE_SMOKE_PASSPHRASE="${MGE_RELEASE_SMOKE_PASSPHRASE:-local-release-smoke-passphrase}"
"$mge_bin" --store "$encrypted_store" init --encrypted --passphrase-env MGE_RELEASE_SMOKE_PASSPHRASE
"$mge_bin" --store "$encrypted_store" remember "private release smoke" --passphrase-env MGE_RELEASE_SMOKE_PASSPHRASE >/dev/null
"$mge_bin" --store "$encrypted_store" checkpoint --passphrase-env MGE_RELEASE_SMOKE_PASSPHRASE >/dev/null
"$mge_bin" --store "$encrypted_store" seal --passphrase-env MGE_RELEASE_SMOKE_PASSPHRASE >/dev/null
"$mge_bin" --store "$encrypted_store" recall "private release smoke" --passphrase-env MGE_RELEASE_SMOKE_PASSPHRASE >/dev/null
"$mge_bin" doctor --store "$encrypted_store" --deep --passphrase-env MGE_RELEASE_SMOKE_PASSPHRASE >/dev/null
"$mge_bin" --store "$encrypted_store" validate --deep --passphrase-env MGE_RELEASE_SMOKE_PASSPHRASE >/dev/null

echo "MCP smoke..."
printf '{"jsonrpc":"2.0","id":1,"method":"mge_schema","params":{}}\n{"jsonrpc":"2.0","id":2,"method":"mge_stats","params":{"store_path":"%s"}}\n' "$plain_store" \
  | "$mcp_bin" \
  | tee "$tmp_root/mcp-response.jsonl" >/dev/null
grep -q '"protocol_version":"mge-jsonrpc-1"' "$tmp_root/mcp-response.jsonl"
grep -q '"tool":"mge_stats"' "$tmp_root/mcp-response.jsonl"

if command -v python >/dev/null 2>&1; then
  echo "Python SDK smoke..."
  MGE_BIN="$mge_bin" python examples/python_basic_usage.py >/dev/null
else
  echo "Python not found; skipping optional Python SDK smoke"
fi

if command -v node >/dev/null 2>&1; then
  echo "TypeScript SDK smoke..."
  if ! MGE_BIN="$mge_bin" node examples/typescript_basic_usage.ts >/dev/null; then
    echo "Node runtime could not run TypeScript example; skipping optional TypeScript SDK smoke"
  fi
else
  echo "Node not found; skipping optional TypeScript SDK smoke"
fi

if command -v rustc >/dev/null 2>&1; then
  echo "Rust CLI host example smoke..."
  rustc examples/agent_host_cli.rs -o "$tmp_root/agent_host_cli"
  MGE_BIN="$mge_bin" "$tmp_root/agent_host_cli" >/dev/null
else
  echo "rustc not found; skipping optional Rust example smoke"
fi

echo "Release smoke ok"
