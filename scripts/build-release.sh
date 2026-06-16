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

product_bins=(
  mge
  mge-mcp-server
)
dev_tool_bins=(
  mge-synthetic-bench
  mge-corpus-bench
)

require_command cargo
require_command uname

if [[ "${MGE_INCLUDE_DEV_TOOLS:-0}" == "1" ]]; then
  echo "Building product and development tool release binaries..."
  cargo build -p mge-cli --bins --release
else
  echo "Building product release binaries..."
  cargo build -p mge-cli --bin mge --bin mge-mcp-server --release
fi

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
for name in "${product_bins[@]}"; do
  find_bin "$name" >/dev/null
done

platform="$(uname -s | tr '[:upper:]' '[:lower:]')-$(uname -m | tr '[:upper:]' '[:lower:]')"
layout_dir="$target_root/mge-release/$platform"
layout_bin_dir="$layout_dir/bin"
layout_docs_dir="$layout_dir/docs"
layout_dev_tools_dir="$layout_dir/dev-tools"
rm -rf "$layout_dir"
mkdir -p "$layout_bin_dir" "$layout_docs_dir"

for name in "${product_bins[@]}"; do
  src="$(find_bin "$name")"
  cp -f "$src" "$layout_bin_dir/$(basename "$src")"
done

if [[ "${MGE_INCLUDE_DEV_TOOLS:-0}" == "1" ]]; then
  mkdir -p "$layout_dev_tools_dir"
  for name in "${dev_tool_bins[@]}"; do
    src="$(find_bin "$name")"
    cp -f "$src" "$layout_dev_tools_dir/$(basename "$src")"
  done
  echo "Development benchmark tools copied to: $layout_dev_tools_dir"
fi

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
