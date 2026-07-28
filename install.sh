#!/usr/bin/env bash
set -euo pipefail

REPO="doggsire/obtuiner"
BINARY="obtuiner"
PLUGIN_BINARY="obtuiner-powermenu"

# ── Architecture detection ─────────────────────────────────────────────────────
ARCH=$(uname -m)
case "$ARCH" in
  x86_64)              TARGET="x86_64-unknown-linux-gnu" ;;
  aarch64 | arm64)     TARGET="aarch64-unknown-linux-gnu" ;;
  *)
    echo "error: unsupported architecture '$ARCH'" >&2
    exit 1
    ;;
esac

# ── Resolve latest release tag ─────────────────────────────────────────────────
if command -v curl >/dev/null 2>&1; then
  FETCH="curl -fsSL"
elif command -v wget >/dev/null 2>&1; then
  FETCH="wget -qO-"
else
  echo "error: curl or wget is required" >&2
  exit 1
fi

echo "Fetching latest release info..."
LATEST=$($FETCH "https://api.github.com/repos/$REPO/releases/latest" \
  | grep '"tag_name"' \
  | sed -E 's/.*"tag_name": *"([^"]+)".*/\1/')

if [ -z "$LATEST" ]; then
  echo "error: could not determine latest release (GitHub API rate limit?)" >&2
  exit 1
fi

ARCHIVE="${BINARY}-${LATEST}-${TARGET}.tar.gz"
PLUGIN_ARCHIVE="${PLUGIN_BINARY}-${LATEST}-${TARGET}.tar.gz"
BASE_URL="https://github.com/$REPO/releases/download/$LATEST"

# ── Download to a temp directory ───────────────────────────────────────────────
TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT

echo "Downloading $ARCHIVE..."
$FETCH "${BASE_URL}/${ARCHIVE}"        > "$TMP/$ARCHIVE"
$FETCH "${BASE_URL}/${ARCHIVE}.sha256" > "$TMP/$ARCHIVE.sha256"

echo "Downloading $PLUGIN_ARCHIVE..."
HAS_PLUGIN_ARCHIVE=true
if ! $FETCH "${BASE_URL}/${PLUGIN_ARCHIVE}" > "$TMP/$PLUGIN_ARCHIVE"; then
  HAS_PLUGIN_ARCHIVE=false
fi
if $HAS_PLUGIN_ARCHIVE; then
  if ! $FETCH "${BASE_URL}/${PLUGIN_ARCHIVE}.sha256" > "$TMP/$PLUGIN_ARCHIVE.sha256"; then
    HAS_PLUGIN_ARCHIVE=false
  fi
fi

# ── Verify checksum ────────────────────────────────────────────────────────────
echo "Verifying checksum..."
(cd "$TMP" && sha256sum -c "$ARCHIVE.sha256" --quiet)
if $HAS_PLUGIN_ARCHIVE; then
  (cd "$TMP" && sha256sum -c "$PLUGIN_ARCHIVE.sha256" --quiet)
fi

# ── Extract ────────────────────────────────────────────────────────────────────
tar -xzf "$TMP/$ARCHIVE" -C "$TMP"
if $HAS_PLUGIN_ARCHIVE; then
  tar -xzf "$TMP/$PLUGIN_ARCHIVE" -C "$TMP"
fi

# ── Choose install location ────────────────────────────────────────────────────
if [ -d /usr/bin ]; then
INSTALL_DIR=/usr/bin
elif [ -d /usr/local/bin ]; then
INSTALL_DIR=/usr/local/bin
elif [ -d "$HOME/.local/bin" ]; then
INSTALL_DIR="$HOME/.local/bin"
else
INSTALL_DIR="$HOME/.local/bin"
mkdir -p "$INSTALL_DIR"
fi

# ── Install ────────────────────────────────────────────────────────────────────
DEST="$INSTALL_DIR/$BINARY"
PLUGIN_DEST="$INSTALL_DIR/$PLUGIN_BINARY"
if [ -w "$INSTALL_DIR" ]; then
  mv "$TMP/$BINARY" "$DEST"
  if $HAS_PLUGIN_ARCHIVE; then
    mv "$TMP/$PLUGIN_BINARY" "$PLUGIN_DEST"
  fi
else
  echo "Need sudo to install to $INSTALL_DIR"
  sudo mv "$TMP/$BINARY" "$DEST"
  if $HAS_PLUGIN_ARCHIVE; then
    sudo mv "$TMP/$PLUGIN_BINARY" "$PLUGIN_DEST"
  fi
fi
chmod +x "$DEST"
if $HAS_PLUGIN_ARCHIVE; then
  chmod +x "$PLUGIN_DEST"
fi

echo "Installed $BINARY $LATEST to $DEST"
if $HAS_PLUGIN_ARCHIVE; then
  echo "Installed $PLUGIN_BINARY $LATEST to $PLUGIN_DEST"
else
  echo "Warning: $PLUGIN_BINARY archive not found in release $LATEST; powermenu plugin was not installed."
fi

# ── Create plugin folder structure ────────────────────────────────────────────
PLUGIN_DIR="${XDG_DATA_HOME:-$HOME/.local/share}/ui/plugins"
mkdir -p "$PLUGIN_DIR"
echo "Plugin directory: $PLUGIN_DIR"

# ── PATH hint ─────────────────────────────────────────────────────────────────
case ":$PATH:" in
  *":$INSTALL_DIR:"*) ;;
  *)
    echo ""
    echo "NOTE: $INSTALL_DIR is not in your PATH."
    echo "Add the following to your shell profile (~/.bashrc or ~/.zshrc):"
    echo "  export PATH=\"$INSTALL_DIR:\$PATH\""
    ;;
esac
