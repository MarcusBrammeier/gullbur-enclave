#!/usr/bin/env bash
# Build the browser extension (Chrome/Firefox) for the wallet relay
set -euo pipefail

cd "$(dirname "$0")/../apps/browser-extension"

EXT_DIR="dist"

rm -rf "$EXT_DIR"
mkdir -p "$EXT_DIR"/icons

# Copy files
cp manifest.json "$EXT_DIR"/
cp background.js "$EXT_DIR"/
cp content.js "$EXT_DIR"/
cp icons/*.png "$EXT_DIR"/icons/

# Update native host path to relative
cp native-host.json "$EXT_DIR"/
echo "✅ Extension built at apps/browser-extension/$EXT_DIR/"
echo "   Load unpacked in Chrome: chrome://extensions"
echo "   Load unpacked in Firefox: about:debugging#/runtime/this-firefox"