#!/bin/bash
set -euo pipefail

REPO="undivisible/poke-around"
BIN="${POKE_AROUND_BIN:-/usr/local/bin/poke-around}"
VERSION="${1:-v0.3.12}"

case "$VERSION" in
  0.3.12|latest) VERSION="v0.3.12" ;;
  0.3.11) VERSION="v0.3.11" ;;
  0.3.10) VERSION="v0.3.10" ;;
  0.3.8) VERSION="v0.3.8" ;;
  0.3.7) VERSION="v0.3.7" ;;
  0.3.6) VERSION="v0.3.6" ;;
  0.3.5) VERSION="v0.3.5" ;;
  0.3.4) VERSION="v0.3.4" ;;
  0.3.2) VERSION="v0.3.2" ;;
esac

sha256_for_asset() {
  case "$1:$2" in
    v0.3.12:poke-around-macos-aarch64.tar.gz) printf '%s\n' "6b8112d882e9d8c9920766a6e88aec4b1b50edc6c7777d1fe0c70f3bf32715ff" ;;
    v0.3.12:poke-around-macos-x86_64.tar.gz) printf '%s\n' "75ad09bda8fc6a8afe5581cd503cf2bb8c08e23da2d5c6c81f41f3590982a5b3" ;;
    v0.3.12:poke-around-linux-x86_64.tar.gz) printf '%s\n' "6e37b0670a07ee153661d03d4bab9097c3973a4924f4170c33115a3c5d05ef49" ;;
    v0.3.12:poke-around-linux-aarch64.tar.gz) printf '%s\n' "19c597b48fa6357280f904dc3257e03c2b59f0333b9d7a0188527d75905ad6aa" ;;
    v0.3.11:poke-around-macos-aarch64.tar.gz) printf '%s\n' "c1379e85f28da20260182dbe92fbb8aba01e9375bcdbba95b98c97643ebe0cd8" ;;
    v0.3.11:poke-around-macos-x86_64.tar.gz) printf '%s\n' "2eb25a545a49f63e4698dd299c5c5e2dc97de702854266d6e128439f5d5afb9d" ;;
    v0.3.11:poke-around-linux-x86_64.tar.gz) printf '%s\n' "c0c64a06b4ae42623d407f7c25fb335930e5a8f0e3becc1c9accf04a023da580" ;;
    v0.3.11:poke-around-linux-aarch64.tar.gz) printf '%s\n' "975b4ddeacb48484c5c2639b0e7ce632e4eb73ae66a6e13422239a2bc9719a1d" ;;
    v0.3.10:poke-around-macos-aarch64.tar.gz) printf '%s\n' "72f103d8f5572ff7a3c740afe6c28a704d9f5e19dba929f663706c90e901b7f9" ;;
    v0.3.10:poke-around-macos-x86_64.tar.gz) printf '%s\n' "4ca4d1f2580b0e6657016f66e24fb4626b41da804bfb938aa74324fb74d986dd" ;;
    v0.3.10:poke-around-linux-x86_64.tar.gz) printf '%s\n' "d0384d5f98fe739a98892a60ceec14deb57b5979e3269c8ae725f78667c7ca4b" ;;
    v0.3.10:poke-around-linux-aarch64.tar.gz) printf '%s\n' "86c553f941075c8cf7c56ded1e37dea03f9c938997e66fbe60d07871ffa7fade" ;;
    v0.3.8:poke-around-macos-aarch64.tar.gz) printf '%s\n' "22ad537012c933824608d315f507ab83f66a5c0eaa71f6b3de6ca7662795af62" ;;
    v0.3.8:poke-around-macos-x86_64.tar.gz) printf '%s\n' "f7c95be9549e6efd1dbaf712ba122932c988ded010ef5a535e1f03c996a80be7" ;;
    v0.3.8:poke-around-linux-x86_64.tar.gz) printf '%s\n' "104c9fd5175cdcf26a8477f97484361a7b399cf7eb6581673933f342e52a3a0d" ;;
    v0.3.8:poke-around-linux-aarch64.tar.gz) printf '%s\n' "a428bb54d3530e0cff54ff266afe6f03449bdd76e3be12cf116ae95cc5ba0ce3" ;;
    v0.3.7:poke-around-macos-aarch64.tar.gz) printf '%s\n' "bd654ee2099c10b3c14bdfa28b20e5073a551d830054e2c1de219b8773bb79fe" ;;
    v0.3.7:poke-around-macos-x86_64.tar.gz) printf '%s\n' "8ce41bdcc5304922d27576f805c891bbde2c9def09289adbc43f3d79e5471eb5" ;;
    v0.3.7:poke-around-linux-x86_64.tar.gz) printf '%s\n' "6263a560e6f03ddbc702bf05e86d231cb7dc7129236c22fc3d9355aeaefa2674" ;;
    v0.3.7:poke-around-linux-aarch64.tar.gz) printf '%s\n' "35fa12a6edbacf72fb4a3e8e1dcc6972b182459406a17136124b18db5db26fbd" ;;
    v0.3.6:poke-around-macos-aarch64.tar.gz) printf '%s\n' "6d73ad299294cc4e0a97b30aaeb61b7602ca4f7c25f97f20ae12d52ff10bbe25" ;;
    v0.3.6:poke-around-macos-x86_64.tar.gz) printf '%s\n' "f5c8ef13f6df040829c3e827feb97af7be3e0c74006d8c5c15a6fdd55075db16" ;;
    v0.3.6:poke-around-linux-x86_64.tar.gz) printf '%s\n' "41b322f3ec8d6290ffe0cd979ac25795e9b27f6a5a8c14afbbb9fe56c8e97c58" ;;
    v0.3.6:poke-around-linux-aarch64.tar.gz) printf '%s\n' "32c43de820083098474d1bfacc0f27c3b595b78a720d1f9d6ed2eb773c645823" ;;
    v0.3.5:poke-around-macos-aarch64.tar.gz) printf '%s\n' "95525a554f37132c9e75fb5b6e31b90241f94faacbb8b773251ba50ac907ea0a" ;;
    v0.3.5:poke-around-macos-x86_64.tar.gz) printf '%s\n' "f45ce156ebe2b8a7090b140834c6804780a43c3eadb103cb54903d396bbfa5ba" ;;
    v0.3.5:poke-around-linux-x86_64.tar.gz) printf '%s\n' "97377e11dd5c3d839bb7b08ca727cca51d555c3a879304facdd56bc1f52acf90" ;;
    v0.3.5:poke-around-linux-aarch64.tar.gz) printf '%s\n' "5a6734c6a7dcd5c299d853220adc6f66cd950449832014e5a9d71e2b09f84f40" ;;
    v0.3.4:poke-around-macos-aarch64.tar.gz) printf '%s\n' "a6f02a62e533842fced008fcf670f0672b553fa073caf7874b1aa173ca78c640" ;;
    v0.3.4:poke-around-macos-x86_64.tar.gz) printf '%s\n' "4e16e3c51fa9f16c2d869e8b4067a8c5ad5b904d44e037acebb91fd31250bf04" ;;
    v0.3.4:poke-around-linux-x86_64.tar.gz) printf '%s\n' "cf669060ca3c7539b646d741d71e3a5a7e5b9527a97e4c2fd567f046958b5ee5" ;;
    v0.3.4:poke-around-linux-aarch64.tar.gz) printf '%s\n' "1ae2f02d0b14b80766a92e352d4d225a2747f095dd658249f1a4133810e6e4d5" ;;
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
