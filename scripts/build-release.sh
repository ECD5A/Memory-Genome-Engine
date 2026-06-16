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

required_bins=(
  mge
  mge-mcp-server
  mge-synthetic-bench
  mge-corpus-bench
)

require_command cargo
require_command uname

echo "Building release binaries..."
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
for name in "${required_bins[@]}"; do
  find_bin "$name" >/dev/null
done

platform="$(uname -s | tr '[:upper:]' '[:lower:]')-$(uname -m | tr '[:upper:]' '[:lower:]')"
layout_dir="$target_root/mge-release/$platform"
layout_bin_dir="$layout_dir/bin"
layout_docs_dir="$layout_dir/docs"
mkdir -p "$layout_bin_dir" "$layout_docs_dir"

for name in "${required_bins[@]}"; do
  src="$(find_bin "$name")"
  cp -f "$src" "$layout_bin_dir/$(basename "$src")"
done

for path in LICENSE README.md README.ru.md QUICKSTART.md QUICKSTART.ru.md SECURITY.md CONTRIBUTING.md CODE_OF_CONDUCT.md; do
  if [[ -f "$path" ]]; then
    cp -f "$path" "$layout_dir/$(basename "$path")"
  fi
done

for path in docs/RELEASE.md docs/RELEASE.ru.md docs/SECURITY.md docs/SECURITY.ru.md docs/INTEGRATION.md docs/INTEGRATION.ru.md; do
  if [[ -f "$path" ]]; then
    cp -f "$path" "$layout_docs_dir/$(basename "$path")"
  fi
done

"$mge_bin" --version

echo "Release build ok: $bin_dir"
echo "Release layout ok: $layout_dir"
