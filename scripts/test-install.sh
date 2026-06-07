#!/bin/bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
INSTALL_SH="$SCRIPT_DIR/install.sh"

require_literal() {
  local text="$1"
  if ! grep -Fq "$text" "$INSTALL_SH"; then
    echo "missing: $text" >&2
    exit 1
  fi
}

reject_literal() {
  local text="$1"
  if grep -Fq "$text" "$INSTALL_SH"; then
    echo "unexpected: $text" >&2
    exit 1
  fi
}

require_regex() {
  local pattern="$1"
  if ! grep -Eq "$pattern" "$INSTALL_SH"; then
    echo "missing pattern: $pattern" >&2
    exit 1
  fi
}

require_literal 'install_from_repo()'
require_literal 'bun run build:bridge && cargo build --workspace --release'
require_literal 'POKE_AROUND_USE_RELEASE'
require_literal 'fetch_release_json()'
require_literal 'json_asset_digest()'
require_literal 'No checksum digest for $VERSION/$ASSET'
require_literal 'URL="https://github.com/$REPO/releases/download/$VERSION/$ASSET"'
require_literal 'TMP_ARCHIVE="$(mktemp'
require_literal 'TMP_DIR="$(mktemp -d'
require_literal 'trap '\''rm -f "$TMP_ARCHIVE" "$TMP_INSTALL"; rm -rf "$TMP_DIR"'\'' EXIT'
require_regex 'curl -fsSL -o "\$TMP_ARCHIVE" "\$URL"'
require_regex 'tar -xzf "\$TMP_ARCHIVE" -C "\$TMP_DIR"'
require_regex 'install -m 755 "\$TMP_DIR/poke-around" "\$TMP_INSTALL"'
require_regex 'sudo install -m 755 "\$TMP_DIR/poke-around" "\$TMP_INSTALL"'
require_regex 'mv -f "\$TMP_INSTALL" "\$BIN"'
require_regex 'sudo mv -f "\$TMP_INSTALL" "\$BIN"'
reject_literal 'sha256_for_asset()'
reject_literal 'curl -fsSL -o "$BIN" "$URL"'
reject_literal 'releases/$VERSION/download'

WORK_DIR="$(mktemp -d "${TMPDIR:-/tmp}/poke-around-install-test.XXXXXX")"
trap 'rm -rf "$WORK_DIR"' EXIT
MOCK_BIN="$WORK_DIR/bin"
mkdir -p "$MOCK_BIN"

cat > "$MOCK_BIN/bun" <<'MOCK'
#!/bin/bash
set -euo pipefail
mkdir -p bridge/dist/traybin
printf 'local bridge\n' > bridge/dist/poke-around-bridge.js
printf 'tray\n' > bridge/dist/traybin/tray
MOCK
chmod +x "$MOCK_BIN/bun"

cat > "$MOCK_BIN/cargo" <<'MOCK'
#!/bin/bash
set -euo pipefail
mkdir -p target/release
printf '#!/bin/sh\necho local\n' > target/release/poke-around
chmod +x target/release/poke-around
MOCK
chmod +x "$MOCK_BIN/cargo"

LOCAL_TARGET="$WORK_DIR/local/poke-around"
mkdir -p "$(dirname "$LOCAL_TARGET")"
PATH="$MOCK_BIN:$PATH" POKE_AROUND_BIN="$LOCAL_TARGET" "$INSTALL_SH"

if [[ "$("$LOCAL_TARGET")" != "local" ]]; then
  echo "local installed binary did not execute" >&2
  exit 1
fi

if [[ "$(cat "$(dirname "$LOCAL_TARGET")/poke-around-bridge.js")" != "local bridge" ]]; then
  echo "local bridge was not installed" >&2
  exit 1
fi

if [[ "$(cat "$(dirname "$LOCAL_TARGET")/traybin/tray")" != "tray" ]]; then
  echo "local traybin was not installed" >&2
  exit 1
fi

rm -f "$MOCK_BIN/bun" "$MOCK_BIN/cargo"

cat > "$MOCK_BIN/uname" <<'MOCK'
#!/bin/bash
set -euo pipefail
case "$1" in
  -s) printf '%s\n' Darwin ;;
  -m) printf '%s\n' arm64 ;;
  *) exit 1 ;;
esac
MOCK
chmod +x "$MOCK_BIN/uname"

cat > "$MOCK_BIN/curl" <<'MOCK'
#!/bin/bash
set -euo pipefail
out=""
url=""
while [[ $# -gt 0 ]]; do
  case "$1" in
    -o)
      out="$2"
      shift 2
      ;;
    -H)
      shift 2
      ;;
    -*)
      shift
      ;;
    *)
      url="$1"
      shift
      ;;
  esac
done
if [[ -z "$out" ]]; then
  printf '{\n  "tag_name": "v9.9.9",\n  "assets": [\n    {\n      "name": "poke-around-macos-aarch64.tar.gz",\n      "digest": "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"\n    }\n  ]\n}\n'
  exit 0
fi
case "$url" in
  *v9.9.9/poke-around-macos-aarch64.tar.gz) ;;
  *) echo "unexpected url: $url" >&2; exit 1 ;;
esac
tmp="$(mktemp -d "${TMPDIR:-/tmp}/poke-around-curl.XXXXXX")"
trap 'rm -rf "$tmp"' EXIT
printf '#!/bin/sh\necho release\n' > "$tmp/poke-around"
printf 'release bridge\n' > "$tmp/poke-around-bridge.js"
mkdir -p "$tmp/traybin"
printf 'release tray\n' > "$tmp/traybin/tray"
chmod +x "$tmp/poke-around"
tar -czf "$out" -C "$tmp" poke-around poke-around-bridge.js traybin
MOCK
chmod +x "$MOCK_BIN/curl"

cat > "$MOCK_BIN/shasum" <<'MOCK'
#!/bin/bash
set -euo pipefail
printf '%s  %s\n' "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa" "${@: -1}"
MOCK
chmod +x "$MOCK_BIN/shasum"

RELEASE_TARGET="$WORK_DIR/release/poke-around"
mkdir -p "$(dirname "$RELEASE_TARGET")"
PATH="$MOCK_BIN:$PATH" POKE_AROUND_USE_RELEASE=1 POKE_AROUND_BIN="$RELEASE_TARGET" "$INSTALL_SH"

if [[ "$("$RELEASE_TARGET")" != "release" ]]; then
  echo "release installed binary did not execute" >&2
  exit 1
fi

if [[ "$(cat "$(dirname "$RELEASE_TARGET")/poke-around-bridge.js")" != "release bridge" ]]; then
  echo "release bridge was not installed" >&2
  exit 1
fi

if [[ "$(cat "$(dirname "$RELEASE_TARGET")/traybin/tray")" != "release tray" ]]; then
  echo "release traybin was not installed" >&2
  exit 1
fi

if compgen -G "$(dirname "$RELEASE_TARGET")/.poke-around.*.tmp" > /dev/null; then
  echo "staged install file was not cleaned up" >&2
  exit 1
fi
