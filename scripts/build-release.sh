#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

echo "Building release binaries..."
cargo build -p mge-cli --bins --release

bin_dir="$repo_root/target/release"

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
find_bin mge-mcp-server >/dev/null
find_bin mge-synthetic-bench >/dev/null
find_bin mge-corpus-bench >/dev/null

"$mge_bin" --version

echo "Release build ok: $bin_dir"
