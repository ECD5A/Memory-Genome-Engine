#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

if [[ -n "${MGE_BIN:-}" ]]; then
  mge_bin="$MGE_BIN"
else
  cargo build --locked -p mge-cli --bin mge --bin mge-mcp-server >/dev/null
  bin_dir="$repo_root/target/debug"
  if [[ -x "$bin_dir/mge" ]]; then
    mge_bin="$bin_dir/mge"
  elif [[ -x "$bin_dir/mge.exe" ]]; then
    mge_bin="$bin_dir/mge.exe"
  else
    echo "missing mge debug binary" >&2
    exit 1
  fi
fi

tmp_root="$(mktemp -d "${TMPDIR:-/tmp}/mge-demo-local.XXXXXX")"
store="$tmp_root/.memory-genome"
export MGE_DEMO_PASSPHRASE="${MGE_DEMO_PASSPHRASE:-local-demo-passphrase}"
cleanup() {
  if [[ "${KEEP_MGE_DEMO:-0}" != "1" ]]; then
    rm -rf "$tmp_root"
  fi
}
trap cleanup EXIT

echo "Session 1: record and seal durable project memory"
"$mge_bin" --store "$store" init --profile fast --encrypted --passphrase-env MGE_DEMO_PASSPHRASE
"$mge_bin" --store "$store" remember-session --session-id demo-planning --scope demo --turn "user=Prepare the release plan" --turn "assistant=Use a staged rollout" --turn "user=Keep a tested rollback path" --passphrase-env MGE_DEMO_PASSPHRASE
"$mge_bin" --store "$store" checkpoint --passphrase-env MGE_DEMO_PASSPHRASE
"$mge_bin" --store "$store" seal --passphrase-env MGE_DEMO_PASSPHRASE >/dev/null

echo "Session 2: reopen, recall the decision, and store the result"
"$mge_bin" --store "$store" recall "What release and rollback approach was chosen?" --mode focused --scope demo --passphrase-env MGE_DEMO_PASSPHRASE
"$mge_bin" --store "$store" remember "Release candidate passed the local verification gate." --kind tool_result --scope demo --trust tool_observed --marker topic:release --passphrase-env MGE_DEMO_PASSPHRASE
"$mge_bin" --store "$store" checkpoint --passphrase-env MGE_DEMO_PASSPHRASE
"$mge_bin" doctor --store "$store" --deep --passphrase-env MGE_DEMO_PASSPHRASE
"$mge_bin" --store "$store" validate --deep --passphrase-env MGE_DEMO_PASSPHRASE

echo "Two-session local memory demo passed."
if [[ "${KEEP_MGE_DEMO:-0}" == "1" ]]; then
  echo "Keeping demo store: $store"
fi
