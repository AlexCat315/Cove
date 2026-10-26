#!/bin/sh
set -eu
COVE_VERSION=${COVE_VERSION:-latest}
COVE_ARCHIVE=
COVE_CHECKSUM=
while [ "$#" -gt 0 ]; do
  case "$1" in
    --version) COVE_VERSION=$2; shift 2 ;;
    --archive) COVE_ARCHIVE=$2; shift 2 ;;
    --checksum) COVE_CHECKSUM=$2; shift 2 ;;
    *) echo "Usage: install.sh [--version VERSION] [--archive FILE --checksum SHA256]" >&2; exit 1 ;;
  esac
done
case "$(uname -s)" in Darwin) COVE_SYSTEM=macos ;; Linux) COVE_SYSTEM=linux ;; *) echo 'Use install.ps1 on Windows.' >&2; exit 1 ;; esac
case "$(uname -m)" in arm64|aarch64) COVE_ARCH=arm64 ;; x86_64|amd64) COVE_ARCH=x86_64 ;; *) echo 'Unsupported CPU architecture.' >&2; exit 1 ;; esac
COVE_TEMP=$(mktemp -d)
trap 'rm -rf "$COVE_TEMP"' EXIT HUP INT TERM
COVE_NAME="cove-$COVE_SYSTEM-$COVE_ARCH.tar.gz"
if [ -z "$COVE_ARCHIVE" ]; then
  if [ "$COVE_VERSION" = latest ]; then
    COVE_URL=https://github.com/AlexCat315/Cove/releases/latest/download
  else
    case "$COVE_VERSION" in *[!0-9A-Za-z.-]*|'') echo 'Invalid version.' >&2; exit 1 ;; esac
    COVE_URL="https://github.com/AlexCat315/Cove/releases/download/v$COVE_VERSION"
  fi
  curl -fLsS --proto '=https' "$COVE_URL/$COVE_NAME" -o "$COVE_TEMP/$COVE_NAME"
  curl -fLsS --proto '=https' "$COVE_URL/SHA256SUMS" -o "$COVE_TEMP/SHA256SUMS"
  COVE_CHECKSUM=$(awk -v name="$COVE_NAME" '$2 == name { print $1 }' "$COVE_TEMP/SHA256SUMS")
  COVE_ARCHIVE="$COVE_TEMP/$COVE_NAME"
fi
if [ "${#COVE_CHECKSUM}" -ne 64 ]; then echo 'A unique SHA-256 checksum is required.' >&2; exit 1; fi
if command -v sha256sum >/dev/null 2>&1; then
  COVE_ACTUAL=$(sha256sum "$COVE_ARCHIVE" | awk '{print $1}')
else
  COVE_ACTUAL=$(shasum -a 256 "$COVE_ARCHIVE" | awk '{print $1}')
fi
if [ "$COVE_ACTUAL" != "$COVE_CHECKSUM" ]; then echo 'CLI checksum mismatch.' >&2; exit 1; fi
mkdir "$COVE_TEMP/payload"
tar -xzf "$COVE_ARCHIVE" -C "$COVE_TEMP/payload"
if [ "$COVE_SYSTEM" = linux ]; then
  COVE_DATA=${COVE_HOME:-"${XDG_DATA_HOME:-$HOME/.local/share}/cove"}
else
  COVE_DATA=${COVE_HOME:-"$HOME/.cove"}
fi
COVE_BIN=${COVE_BIN_DIR:-"$HOME/.local/bin"}
"$COVE_TEMP/payload/cove/cove" internal install-cli "$COVE_DATA" --bin-directory "$COVE_BIN"
case ":$PATH:" in
  *":$COVE_BIN:"*) ;;
  *)
    COVE_ESCAPED=$(printf '%s' "$COVE_BIN" | sed "s/'/'\\\\''/g")
    case "${SHELL:-/bin/sh}" in
      */zsh) COVE_PROFILE="$HOME/.zshrc" ;;
      */bash) COVE_PROFILE="$HOME/.bashrc" ;;
      */fish)
        COVE_PROFILE="${XDG_CONFIG_HOME:-$HOME/.config}/fish/conf.d/cove.fish"
        mkdir -p "$(dirname "$COVE_PROFILE")"
        printf "fish_add_path --prepend '%s'\n" "$COVE_ESCAPED" > "$COVE_PROFILE"
        COVE_PROFILE=
        ;;
      *) COVE_PROFILE="$HOME/.profile" ;;
    esac
    if [ -n "$COVE_PROFILE" ] && ! { [ -f "$COVE_PROFILE" ] && grep -F '# Cove CLI' "$COVE_PROFILE" >/dev/null; }; then
      printf "\n# Cove CLI\nexport PATH='%s':\"\044PATH\"\n" "$COVE_ESCAPED" >> "$COVE_PROFILE"
    fi
    echo 'Open a new terminal to use cove.'
    ;;
esac
