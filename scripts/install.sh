#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

install_dir="${HOME}/.local/bin"
no_build=0
include_dev_tools=0

usage() {
  cat <<'EOF'
Usage: scripts/install.sh [--install-dir DIR] [--no-build] [--include-dev-tools]

Builds local release binaries and copies them to a user-writable bin directory.
No packages are published and root privileges are not required.
By default this installs product binaries only: mge and mge-mcp-server.
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --install-dir)
      install_dir="${2:?missing --install-dir value}"
      shift 2
      ;;
    --no-build)
      no_build=1
      shift
      ;;
    --include-dev-tools)
      include_dev_tools=1
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "unknown argument: $1" >&2
      usage >&2
      exit 2
      ;;
  esac
done

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

if [[ "$no_build" != "1" ]]; then
  require_command cargo
  if [[ "$include_dev_tools" == "1" ]]; then
    echo "Building product and development tool release binaries..."
    cargo build --locked -p mge-cli --bins --release
  else
    echo "Building product release binaries..."
    cargo build --locked -p mge-cli --bin mge --bin mge-mcp-server --release
  fi
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

mkdir -p "$install_dir"

for name in "${product_bins[@]}"; do
  src="$(find_bin "$name")"
  cp -f "$src" "$install_dir/$(basename "$src")"
  chmod 755 "$install_dir/$(basename "$src")"
done

if [[ "$include_dev_tools" == "1" ]]; then
  for name in "${dev_tool_bins[@]}"; do
    src="$(find_bin "$name")"
    cp -f "$src" "$install_dir/$(basename "$src")"
    chmod 755 "$install_dir/$(basename "$src")"
  done
  echo "Installed development benchmark tools."
fi

"$install_dir/$(basename "$(find_bin mge)")" --version

echo "Installed release binaries to: $install_dir"
echo "Add this directory to PATH if it is not already available."
