#!/usr/bin/env bash
set -euo pipefail

version="latest"
install_dir="${HOME}/.local/bin"
repository="ECD5A/Memory-Genome-Engine"
base_url=""
source_dir=""

usage() {
  cat <<'EOF'
Usage: scripts/install-release.sh [--version VERSION] [--install-dir DIR]
       [--repository OWNER/REPO] [--base-url URL] [--source-dir DIR]

Downloads and verifies a GitHub release, then installs mge and
mge-mcp-server into a user-writable directory. Root is not required.

Examples:
  ./scripts/install-release.sh
  ./scripts/install-release.sh --version v0.1.1
  ./scripts/install-release.sh --source-dir target/mge-release/archives
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --version) version="${2:?missing --version value}"; shift 2 ;;
    --install-dir) install_dir="${2:?missing --install-dir value}"; shift 2 ;;
    --repository) repository="${2:?missing --repository value}"; shift 2 ;;
    --base-url) base_url="${2:?missing --base-url value}"; shift 2 ;;
    --source-dir) source_dir="${2:?missing --source-dir value}"; shift 2 ;;
    -h|--help) usage; exit 0 ;;
    *) echo "unknown argument: $1" >&2; usage >&2; exit 2 ;;
  esac
done

if [[ -n "$base_url" && -n "$source_dir" ]]; then
  echo "--base-url and --source-dir are mutually exclusive" >&2
  exit 2
fi

case "$(uname -s)" in
  Linux*) os="linux" ;;
  Darwin*) os="macos" ;;
  *) echo "unsupported release operating system: $(uname -s)" >&2; exit 1 ;;
esac
case "$(uname -m)" in
  x86_64|amd64) arch="x86_64" ;;
  arm64|aarch64) arch="aarch64" ;;
  *) echo "unsupported release architecture: $(uname -m)" >&2; exit 1 ;;
esac
if [[ "$os" == "linux" && "$arch" != "x86_64" ]]; then
  echo "no Linux release archive is published for architecture: $arch" >&2
  exit 1
fi

asset="mge-${os}-${arch}.tar.gz"
temp_root="$(mktemp -d "${TMPDIR:-/tmp}/mge-install.XXXXXX")"
trap 'rm -rf "$temp_root"' EXIT
archive="$temp_root/$asset"
checksums="$temp_root/SHA256SUMS"
extracted="$temp_root/extracted"
mkdir -p "$extracted"

receive_file() {
  local name="$1"
  local destination="$2"
  if [[ -n "$source_dir" ]]; then
    local source
    source="$(cd "$source_dir" && pwd)/$name"
    if [[ ! -f "$source" ]]; then
      echo "release file is missing: $source" >&2
      return 1
    fi
    cp -f "$source" "$destination"
    return
  fi

  local root
  if [[ -n "$base_url" ]]; then
    root="${base_url%/}"
  elif [[ "$version" == "latest" ]]; then
    root="https://github.com/$repository/releases/latest/download"
  else
    root="https://github.com/$repository/releases/download/$version"
  fi
  command -v curl >/dev/null 2>&1 || { echo "missing required command: curl" >&2; return 127; }
  curl --fail --location --silent --show-error "$root/$name" --output "$destination"
}

receive_file "$asset" "$archive"
receive_file SHA256SUMS "$checksums"
expected="$(awk -v name="$asset" '$2 == name || $2 == "*" name { print tolower($1); exit }' "$checksums")"
if [[ ! "$expected" =~ ^[0-9a-f]{64}$ ]]; then
  echo "SHA256SUMS does not contain an exact checksum for $asset" >&2
  exit 1
fi
if command -v sha256sum >/dev/null 2>&1; then
  actual="$(sha256sum "$archive" | awk '{print tolower($1)}')"
elif command -v shasum >/dev/null 2>&1; then
  actual="$(shasum -a 256 "$archive" | awk '{print tolower($1)}')"
else
  echo "missing sha256sum or shasum" >&2
  exit 127
fi
if [[ "$actual" != "$expected" ]]; then
  echo "checksum mismatch for $asset (expected $expected, got $actual)" >&2
  exit 1
fi

tar -xzf "$archive" -C "$extracted"
mge="$(find "$extracted" -type f -path '*/bin/mge' -print -quit)"
mcp="$(find "$extracted" -type f -path '*/bin/mge-mcp-server' -print -quit)"
if [[ -z "$mge" || -z "$mcp" ]]; then
  echo "verified archive does not contain both product binaries" >&2
  exit 1
fi

mkdir -p "$install_dir"
install -m 0755 "$mge" "$install_dir/mge"
install -m 0755 "$mcp" "$install_dir/mge-mcp-server"
"$install_dir/mge" --version
echo "Verified $asset and installed product binaries to: $install_dir"
