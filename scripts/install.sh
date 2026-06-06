#!/bin/bash
set -euo pipefail

REPO="undivisible/poke-around"
BIN="${POKE_AROUND_BIN:-/usr/local/bin/poke-around}"
VERSION="${1:-v0.3.2}"

case "$VERSION" in
  0.3.2|latest) VERSION="v0.3.2" ;;
esac

sha256_for_asset() {
  case "$1:$2" in
    v0.3.2:poke-around-macos-aarch64.tar.gz) printf '%s\n' "4c60e61338b3023fba3d12bcf9ad85d8f694161f0faadb626c4f22b042127397" ;;
    v0.3.2:poke-around-macos-x86_64.tar.gz) printf '%s\n' "273fe8fd3e45431287c2aca7f0b1e076dfdaea9d323443361c3037f412e08aaf" ;;
    v0.3.2:poke-around-linux-x86_64.tar.gz) printf '%s\n' "3def9ea22e80ce7c4e5afa0adc9682aa402081746d55f57ba43041ab84f0efed" ;;
    *) return 1 ;;
  esac
}

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
if ! EXPECTED_SHA256="$(sha256_for_asset "$VERSION" "$ASSET")"; then
  echo "No checksum for $VERSION/$ASSET" >&2
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
BRIDGE="$INSTALL_DIR/poke-around-bridge.js"

if [[ -w "$INSTALL_DIR" && ( ! -e "$BIN" || -w "$BIN" ) ]]; then
  install -m 755 "$TMP_DIR/poke-around" "$TMP_INSTALL"
  mv -f "$TMP_INSTALL" "$BIN"
  install -m 644 "$TMP_DIR/poke-around-bridge.js" "$BRIDGE"
else
  sudo install -m 755 "$TMP_DIR/poke-around" "$TMP_INSTALL"
  sudo mv -f "$TMP_INSTALL" "$BIN"
  sudo install -m 644 "$TMP_DIR/poke-around-bridge.js" "$BRIDGE"
fi

echo " Installed to $BIN"
echo " Run: poke-around --help"
