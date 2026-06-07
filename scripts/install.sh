#!/bin/bash
set -euo pipefail

REPO="undivisible/poke-around"
BIN="${POKE_AROUND_BIN:-/usr/local/bin/poke-around}"
VERSION="${1:-latest}"

fetch_release_json() {
  local version="$1"
  local api_url
  if [[ "$version" == "latest" ]]; then
    api_url="https://api.github.com/repos/$REPO/releases/latest"
  else
    api_url="https://api.github.com/repos/$REPO/releases/tags/$version"
  fi
  curl -fsSL -H "Accept: application/vnd.github+json" "$api_url"
}

json_tag_name() {
  sed -n 's/^[[:space:]]*"tag_name":[[:space:]]*"\([^"]*\)".*/\1/p' | head -n 1
}

json_asset_digest() {
  local asset="$1"
  ASSET_NAME="$asset" perl -0ne '
    if (/"name"\s*:\s*"\Q$ENV{ASSET_NAME}\E"(?:(?!"browser_download_url").)*"digest"\s*:\s*"sha256:([0-9a-f]{64})"/s) {
      print "$1\n";
      exit 0;
    }
  '
}

case "$VERSION" in
  latest) ;;
  v*) ;;
  *) VERSION="v$VERSION" ;;
esac

file_sha256() {
  if command -v shasum >/dev/null 2>&1; then
    shasum -a 256 "$1" | awk '{print $1}'
  elif command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$1" | awk '{print $1}'
  else
    echo "shasum or sha256sum is required" >&2
    exit 1
  fi
}

echo " Installing poke-around..."

OS="$(uname -s)"
ARCH="$(uname -m)"

case "$OS" in
  Darwin) OS="macos" ;;
  Linux) OS="linux" ;;
  *) echo "Unsupported OS: $OS"; exit 1 ;;
esac

case "$ARCH" in
  x86_64|amd64) ARCH="x86_64" ;;
  arm64|aarch64) ARCH="aarch64" ;;
  *) echo "Unsupported arch: $ARCH"; exit 1 ;;
esac

ASSET="poke-around-$OS-$ARCH.tar.gz"
RELEASE_JSON="$(fetch_release_json "$VERSION")"
RESOLVED_VERSION="$(printf '%s\n' "$RELEASE_JSON" | json_tag_name)"
if [[ -z "$RESOLVED_VERSION" ]]; then
  echo "Could not resolve release tag for $VERSION" >&2
  exit 1
fi
VERSION="$RESOLVED_VERSION"
EXPECTED_SHA256="$(printf '%s\n' "$RELEASE_JSON" | json_asset_digest "$ASSET")"
if [[ -z "$EXPECTED_SHA256" ]]; then
  echo "No checksum digest for $VERSION/$ASSET" >&2
  exit 1
fi

URL="https://github.com/$REPO/releases/download/$VERSION/$ASSET"
echo " Downloading $URL..."
TMP_ARCHIVE="$(mktemp "${TMPDIR:-/tmp}/poke-around.XXXXXX")"
TMP_DIR="$(mktemp -d "${TMPDIR:-/tmp}/poke-around.XXXXXX")"
TMP_INSTALL="$(dirname "$BIN")/.poke-around.$$.tmp"
trap 'rm -f "$TMP_ARCHIVE" "$TMP_INSTALL"; rm -rf "$TMP_DIR"' EXIT
curl -fsSL -o "$TMP_ARCHIVE" "$URL"
ACTUAL_SHA256="$(file_sha256 "$TMP_ARCHIVE")"
if [[ "$ACTUAL_SHA256" != "$EXPECTED_SHA256" ]]; then
  echo "Checksum mismatch for $ASSET" >&2
  echo "expected: $EXPECTED_SHA256" >&2
  echo "actual:   $ACTUAL_SHA256" >&2
  exit 1
fi
tar -xzf "$TMP_ARCHIVE" -C "$TMP_DIR"

if [[ ! -x "$TMP_DIR/poke-around" ]]; then
  echo "Archive did not contain executable poke-around" >&2
  exit 1
fi

INSTALL_DIR="$(dirname "$BIN")"

if [[ -w "$INSTALL_DIR" && ( ! -e "$BIN" || -w "$BIN" ) ]]; then
  install -m 755 "$TMP_DIR/poke-around" "$TMP_INSTALL"
  mv -f "$TMP_INSTALL" "$BIN"
else
  sudo install -m 755 "$TMP_DIR/poke-around" "$TMP_INSTALL"
  sudo mv -f "$TMP_INSTALL" "$BIN"
fi

echo " Installed to $BIN"
echo " Run: poke-around --help"
