#!/bin/sh
set -eu

repo="${DRIFTLESS_REPO:-akshay5995/driftless}"
version="${DRIFTLESS_VERSION:-latest}"
install_dir="${DRIFTLESS_INSTALL_DIR:-$HOME/.local/bin}"

usage() {
  cat <<'EOF'
Install driftless from GitHub Releases.

Environment:
  DRIFTLESS_VERSION      Version tag to install, for example v0.2.1. Defaults to latest.
  DRIFTLESS_INSTALL_DIR  Install directory. Defaults to ~/.local/bin.
  DRIFTLESS_REPO         GitHub repo. Defaults to akshay5995/driftless.
EOF
}

need() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "error: required command not found: $1" >&2
    exit 1
  fi
}

target() {
  os="$(uname -s)"
  arch="$(uname -m)"
  case "$os:$arch" in
    Linux:x86_64|Linux:amd64) echo "x86_64-unknown-linux-gnu" ;;
    Linux:aarch64|Linux:arm64) echo "aarch64-unknown-linux-gnu" ;;
    Darwin:x86_64) echo "x86_64-apple-darwin" ;;
    Darwin:arm64|Darwin:aarch64) echo "aarch64-apple-darwin" ;;
    MINGW*:x86_64|MSYS*:x86_64|CYGWIN*:x86_64) echo "x86_64-pc-windows-msvc" ;;
    *)
      echo "error: unsupported platform: $os $arch" >&2
      exit 1
      ;;
  esac
}

latest_tag() {
  latest_url="$(curl -fsSLI -o /dev/null -w '%{url_effective}' "https://github.com/$repo/releases/latest")"
  tag="${latest_url##*/}"
  if [ -z "$tag" ] || [ "$tag" = "latest" ]; then
    echo "error: could not resolve latest release tag" >&2
    exit 1
  fi
  echo "$tag"
}

verify_checksum() {
  archive="$1"
  if ! awk -v file="$archive" '$2 == file { print; found = 1 } END { exit found ? 0 : 1 }' SHA256SUMS > SHA256SUMS.selected; then
    echo "error: checksum for $archive not found in SHA256SUMS" >&2
    exit 1
  fi

  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum -c SHA256SUMS.selected
  elif command -v shasum >/dev/null 2>&1; then
    shasum -a 256 -c SHA256SUMS.selected
  else
    echo "error: sha256sum or shasum is required to verify downloads" >&2
    exit 1
  fi
}

case "${1:-}" in
  -h|--help)
    usage
    exit 0
    ;;
  "")
    ;;
  *)
    echo "error: unexpected argument: $1" >&2
    usage >&2
    exit 1
    ;;
esac

need curl
platform="$(target)"

if [ "$version" = "latest" ]; then
  tag="$(latest_tag)"
else
  tag="$version"
fi
release_version="${tag#v}"

case "$platform" in
  *windows*)
    archive="driftless-$release_version-$platform.zip"
    binary="driftless.exe"
    need unzip
    ;;
  *)
    archive="driftless-$release_version-$platform.tar.gz"
    binary="driftless"
    need tar
    ;;
esac

base_url="https://github.com/$repo/releases/download/$tag"
tmpdir="$(mktemp -d)"
trap 'rm -rf "$tmpdir"' EXIT HUP INT TERM

cd "$tmpdir"
echo "downloading $archive from $repo $tag"
curl -fL --proto '=https' --tlsv1.2 -o "$archive" "$base_url/$archive"
curl -fL --proto '=https' --tlsv1.2 -o SHA256SUMS "$base_url/SHA256SUMS"
verify_checksum "$archive"

case "$archive" in
  *.zip) unzip -q "$archive" ;;
  *.tar.gz) tar -xzf "$archive" ;;
esac

mkdir -p "$install_dir"
cp "driftless-$release_version-$platform/$binary" "$install_dir/$binary"
chmod 755 "$install_dir/$binary"

echo "installed $("$install_dir/$binary" --version 2>/dev/null || echo driftless) to $install_dir/$binary"
