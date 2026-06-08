#!/bin/bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
INSTALL_SH="$SCRIPT_DIR/install.sh"
INSTALL_PS1="$SCRIPT_DIR/install.ps1"
GITATTRIBUTES="$SCRIPT_DIR/../.gitattributes"

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
require_literal 'cargo build --workspace --release'
require_literal 'POKE_AROUND_USE_RELEASE'
require_literal 'TMP_ARCHIVE="$(mktemp'
require_literal 'TMP_DIR="$(mktemp -d'
require_literal 'trap '\''rm -f "$TMP_ARCHIVE" "$TMP_INSTALL"; rm -rf "$TMP_DIR"'\'' EXIT'
require_regex 'curl -fsSL -o "\$TMP_ARCHIVE" "\$URL"'
require_literal 'fetch_release_json()'
require_literal 'json_tag_name()'
require_literal 'json_asset_digest()'
require_literal 'api_url="https://api.github.com/repos/$REPO/releases/latest"'
require_literal 'api_url="https://api.github.com/repos/$REPO/releases/tags/$version"'
require_literal 'file_sha256()'
require_literal 'Checksum mismatch for $ASSET'
require_literal 'URL="https://github.com/$REPO/releases/download/$VERSION/$ASSET"'
require_regex 'tar -xzf "\$TMP_ARCHIVE" -C "\$TMP_DIR"'
require_regex 'install -m 755 "\$TMP_DIR/poke-around" "\$TMP_INSTALL"'
require_regex 'sudo install -m 755 "\$TMP_DIR/poke-around" "\$TMP_INSTALL"'
require_regex 'mv -f "\$TMP_INSTALL" "\$BIN"'
require_regex 'sudo mv -f "\$TMP_INSTALL" "\$BIN"'
reject_literal 'curl -fsSL -o "$BIN" "$URL"'
reject_literal 'releases/$VERSION/download'

if [[ ! -f "$INSTALL_PS1" ]]; then
  echo "missing: $INSTALL_PS1" >&2
  exit 1
fi

require_windows_literal() {
  local text="$1"
  if ! grep -Fq "$text" "$INSTALL_PS1"; then
    echo "missing: $text" >&2
    exit 1
  fi
}

require_windows_literal '$Asset = "poke-around-windows-x86_64.zip"'
require_windows_literal '$DefaultInstallDir = Join-Path $env:LOCALAPPDATA "Programs\poke-around"'
require_windows_literal 'Invoke-WebRequest -Uri $Url -OutFile $ArchivePath'
require_windows_literal 'Expand-Archive -Path $ArchivePath -DestinationPath $ExtractDir -Force'
require_windows_literal 'Copy-Item -LiteralPath (Join-Path $ExtractDir "poke-around.exe") -Destination (Join-Path $InstallDir "poke-around.exe") -Force'
require_windows_literal 'Write-Host "Run: $(Join-Path $InstallDir "poke-around.exe") --help"'

if grep -Fq '*.ps1 linguist-vendored' "$GITATTRIBUTES"; then
  echo "unexpected: *.ps1 linguist-vendored" >&2
  exit 1
fi

WORK_DIR="$(mktemp -d "${TMPDIR:-/tmp}/poke-around-install-test.XXXXXX")"
trap 'rm -rf "$WORK_DIR"' EXIT
MOCK_BIN="$WORK_DIR/bin"
mkdir -p "$MOCK_BIN"

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

rm -f "$MOCK_BIN/cargo"

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
    *)
      url="$1"
      shift
      ;;
  esac
done
if [[ "$url" == "https://api.github.com/repos/undivisible/poke-around/releases/latest" ]]; then
  cat <<'JSON'
{
  "tag_name": "v0.3.20",
  "assets": [
    {
      "name": "poke-around-macos-aarch64.tar.gz",
      "digest": "sha256:1111111111111111111111111111111111111111111111111111111111111111"
    }
  ],
  "tarball_url": "https://api.github.com/repos/undivisible/poke-around/tarball/v0.3.20"
}
JSON
  exit 0
fi
tmp="$(mktemp -d "${TMPDIR:-/tmp}/poke-around-curl.XXXXXX")"
trap 'rm -rf "$tmp"' EXIT
printf '#!/bin/sh\necho installed\n' > "$tmp/poke-around"
chmod +x "$tmp/poke-around"
tar -czf "$out" -C "$tmp" poke-around
MOCK
chmod +x "$MOCK_BIN/curl"

cat > "$MOCK_BIN/shasum" <<'MOCK'
#!/bin/bash
set -euo pipefail
printf '%s  %s\n' "1111111111111111111111111111111111111111111111111111111111111111" "${@: -1}"
MOCK
chmod +x "$MOCK_BIN/shasum"

TARGET="$WORK_DIR/install/poke-around"
mkdir -p "$(dirname "$TARGET")"
PATH="$MOCK_BIN:$PATH" POKE_AROUND_USE_RELEASE=1 POKE_AROUND_BIN="$TARGET" "$INSTALL_SH"

if [[ "$("$TARGET")" != "installed" ]]; then
  echo "installed binary did not execute" >&2
  exit 1
fi

if compgen -G "$(dirname "$TARGET")/.poke-around.*.tmp" > /dev/null; then
  echo "staged install file was not cleaned up" >&2
  exit 1
fi
