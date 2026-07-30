#!/usr/bin/env bash
# package-extension.sh — Bundle the browser extension for distribution
#
# Produces:
#   dist/browser-extension-chrome.zip   — Chrome/Chromium
#   dist/browser-extension-firefox.zip  — Firefox
#
# Usage:
#   ./scripts/package-extension.sh [--release]

set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
EXT_DIR="$PROJECT_DIR/apps/browser-extension"
DIST_DIR="$PROJECT_DIR/dist"
EXTENSION_VERSION="0.1.0"

mkdir -p "$DIST_DIR"

# ── 1. Chrome — full manifest already at apps/browser-extension ────
echo "==> Packaging Chrome extension v$EXTENSION_VERSION..."
(cd "$EXT_DIR" && zip -qr "$DIST_DIR/browser-extension-chrome-$EXTENSION_VERSION.zip" \
  manifest.json background.js content.js icons/ \
  --exclude "*.gitkeep" \
)
echo "    Created: $DIST_DIR/browser-extension-chrome-$EXTENSION_VERSION.zip"

# ── 2. Firefox — same files, different browser_specific_settings is embedded ──
echo "==> Packaging Firefox extension v$EXTENSION_VERSION..."
cp "$DIST_DIR/browser-extension-chrome-$EXTENSION_VERSION.zip" \
   "$DIST_DIR/browser-extension-firefox-$EXTENSION_VERSION.zip"
echo "    Created: $DIST_DIR/browser-extension-firefox-$EXTENSION_VERSION.zip"

echo ""
echo "✅  Extension bundles ready at $DIST_DIR/"
ls -lh "$DIST_DIR"/browser-extension-*.zip