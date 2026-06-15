#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

cargo build -p mge-cli --bin mge >/dev/null

bin_dir="$repo_root/target/debug"
if [[ -x "$bin_dir/mge" ]]; then
  mge_bin="$bin_dir/mge"
elif [[ -x "$bin_dir/mge.exe" ]]; then
  mge_bin="$bin_dir/mge.exe"
else
  echo "missing mge debug binary" >&2
  exit 1
fi

tmp_root="$(mktemp -d "${TMPDIR:-/tmp}/mge-demo-local.XXXXXX")"
store="$tmp_root/.memory-genome"
export MGE_DEMO_PASSPHRASE="${MGE_DEMO_PASSPHRASE:-local-demo-passphrase}"

echo "Creating encrypted local demo store at $store"
"$mge_bin" --store "$store" init --profile fast --encrypted --passphrase-env MGE_DEMO_PASSPHRASE
"$mge_bin" --store "$store" remember "Agent should recall project context before local work." --kind procedure --scope demo --trust user_confirmed --marker topic:demo --passphrase-env MGE_DEMO_PASSPHRASE
"$mge_bin" --store "$store" recall "project context" --mode focused --scope demo --passphrase-env MGE_DEMO_PASSPHRASE
"$mge_bin" --store "$store" remember "Fake local agent work result: demo workflow completed." --kind tool_result --scope demo --trust tool_observed --marker topic:demo --passphrase-env MGE_DEMO_PASSPHRASE
"$mge_bin" --store "$store" checkpoint --passphrase-env MGE_DEMO_PASSPHRASE
"$mge_bin" --store "$store" seal --passphrase-env MGE_DEMO_PASSPHRASE >/dev/null
"$mge_bin" --store "$store" recall "demo workflow completed" --mode broad --scope demo --passphrase-env MGE_DEMO_PASSPHRASE
"$mge_bin" doctor --store "$store" --deep --passphrase-env MGE_DEMO_PASSPHRASE
"$mge_bin" --store "$store" validate --deep --passphrase-env MGE_DEMO_PASSPHRASE
"$mge_bin" --store "$store" export --passphrase-env MGE_DEMO_PASSPHRASE

echo "Markdown export is plaintext by design: $store/exports/memory.md"
echo "Demo store: $store"
