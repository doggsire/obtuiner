#!/usr/bin/env bash
set -euo pipefail

REPO="doggsire/obtuiner"
BINARY="obtuiner"

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
BASE_URL="https://github.com/$REPO/releases/download/$LATEST"

# ── Download to a temp directory ───────────────────────────────────────────────
TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT

echo "Downloading $ARCHIVE..."
$FETCH "${BASE_URL}/${ARCHIVE}"        > "$TMP/$ARCHIVE"
$FETCH "${BASE_URL}/${ARCHIVE}.sha256" > "$TMP/$ARCHIVE.sha256"

# ── Verify checksum ────────────────────────────────────────────────────────────
echo "Verifying checksum..."
(cd "$TMP" && sha256sum -c "$ARCHIVE.sha256" --quiet)

# ── Extract ────────────────────────────────────────────────────────────────────
tar -xzf "$TMP/$ARCHIVE" -C "$TMP"

# ── Choose install location ────────────────────────────────────────────────────
if [ -w /usr/bin ]; then
INSTALL_DIR=/usr/bin
elif [ -w /usr/local/bin ]; then
INSTALL_DIR=/usr/local/bin
elif [ -d "$HOME/.local/bin" ]; then
INSTALL_DIR="$HOME/.local/bin"
else
INSTALL_DIR="$HOME/.local/bin"
mkdir -p "$INSTALL_DIR"
fi

# ── Install ────────────────────────────────────────────────────────────────────
DEST="$INSTALL_DIR/$BINARY"
if [ -w "$INSTALL_DIR" ]; then
  mv "$TMP/$BINARY" "$DEST"
else
  echo "Need sudo to install to $INSTALL_DIR"
  sudo mv "$TMP/$BINARY" "$DEST"
fi
chmod +x "$DEST"

echo "Installed $BINARY $LATEST to $DEST"

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
