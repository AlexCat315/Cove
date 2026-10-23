#!/bin/sh
set -eu
GUAVA_VERSION=${GUAVA_VERSION:-latest}
GUAVA_ARCHIVE=
GUAVA_CHECKSUM=
while [ "$#" -gt 0 ]; do
  case "$1" in
    --version) GUAVA_VERSION=$2; shift 2 ;;
    --archive) GUAVA_ARCHIVE=$2; shift 2 ;;
    --checksum) GUAVA_CHECKSUM=$2; shift 2 ;;
    *) echo "Usage: install.sh [--version VERSION] [--archive FILE --checksum SHA256]" >&2; exit 1 ;;
  esac
done
case "$(uname -s)" in Darwin) GUAVA_SYSTEM=macos ;; Linux) GUAVA_SYSTEM=linux ;; *) echo 'Use install.ps1 on Windows.' >&2; exit 1 ;; esac
case "$(uname -m)" in arm64|aarch64) GUAVA_ARCH=arm64 ;; x86_64|amd64) GUAVA_ARCH=x86_64 ;; *) echo 'Unsupported CPU architecture.' >&2; exit 1 ;; esac
GUAVA_TEMP=$(mktemp -d)
trap 'rm -rf "$GUAVA_TEMP"' EXIT HUP INT TERM
GUAVA_NAME="guava-$GUAVA_SYSTEM-$GUAVA_ARCH.tar.gz"
if [ -z "$GUAVA_ARCHIVE" ]; then
  if [ "$GUAVA_VERSION" = latest ]; then
    GUAVA_URL=https://github.com/AlexCat315/GuavaUI/releases/latest/download
  else
    case "$GUAVA_VERSION" in *[!0-9A-Za-z.-]*|'') echo 'Invalid version.' >&2; exit 1 ;; esac
    GUAVA_URL="https://github.com/AlexCat315/GuavaUI/releases/download/v$GUAVA_VERSION"
  fi
  curl -fLsS --proto '=https' "$GUAVA_URL/$GUAVA_NAME" -o "$GUAVA_TEMP/$GUAVA_NAME"
  curl -fLsS --proto '=https' "$GUAVA_URL/SHA256SUMS" -o "$GUAVA_TEMP/SHA256SUMS"
  GUAVA_CHECKSUM=$(awk -v name="$GUAVA_NAME" '$2 == name { print $1 }' "$GUAVA_TEMP/SHA256SUMS")
  GUAVA_ARCHIVE="$GUAVA_TEMP/$GUAVA_NAME"
fi
if [ "${#GUAVA_CHECKSUM}" -ne 64 ]; then echo 'A unique SHA-256 checksum is required.' >&2; exit 1; fi
if command -v sha256sum >/dev/null 2>&1; then
  GUAVA_ACTUAL=$(sha256sum "$GUAVA_ARCHIVE" | awk '{print $1}')
else
  GUAVA_ACTUAL=$(shasum -a 256 "$GUAVA_ARCHIVE" | awk '{print $1}')
fi
if [ "$GUAVA_ACTUAL" != "$GUAVA_CHECKSUM" ]; then echo 'CLI checksum mismatch.' >&2; exit 1; fi
mkdir "$GUAVA_TEMP/payload"
tar -xzf "$GUAVA_ARCHIVE" -C "$GUAVA_TEMP/payload"
if [ "$GUAVA_SYSTEM" = linux ]; then
  GUAVA_DATA=${GUAVA_HOME:-"${XDG_DATA_HOME:-$HOME/.local/share}/guava"}
else
  GUAVA_DATA=${GUAVA_HOME:-"$HOME/.guava"}
fi
GUAVA_BIN=${GUAVA_BIN_DIR:-"$HOME/.local/bin"}
"$GUAVA_TEMP/payload/guava/guava" internal install-cli "$GUAVA_DATA" --bin-directory "$GUAVA_BIN"
case ":$PATH:" in
  *":$GUAVA_BIN:"*) ;;
  *)
    GUAVA_ESCAPED=$(printf '%s' "$GUAVA_BIN" | sed "s/'/'\\\\''/g")
    case "${SHELL:-/bin/sh}" in
      */zsh) GUAVA_PROFILE="$HOME/.zshrc" ;;
      */bash) GUAVA_PROFILE="$HOME/.bashrc" ;;
      */fish)
        GUAVA_PROFILE="${XDG_CONFIG_HOME:-$HOME/.config}/fish/conf.d/guava.fish"
        mkdir -p "$(dirname "$GUAVA_PROFILE")"
        printf "fish_add_path --prepend '%s'\n" "$GUAVA_ESCAPED" > "$GUAVA_PROFILE"
        GUAVA_PROFILE=
        ;;
      *) GUAVA_PROFILE="$HOME/.profile" ;;
    esac
    if [ -n "$GUAVA_PROFILE" ] && ! { [ -f "$GUAVA_PROFILE" ] && grep -F '# Guava CLI' "$GUAVA_PROFILE" >/dev/null; }; then
      printf "\n# Guava CLI\nexport PATH='%s':\"\044PATH\"\n" "$GUAVA_ESCAPED" >> "$GUAVA_PROFILE"
    fi
    echo 'Open a new terminal to use guava.'
    ;;
esac
