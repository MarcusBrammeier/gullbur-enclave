#!/usr/bin/env bash
# install-nm-host.sh — Install native messaging host for browser extension
#
# Installs the gullbur-relay binary and registers it with Chrome/Firefox
# as a native messaging host. Defaults to per-user install (no sudo).
#
# Usage:
#   ./scripts/install-nm-host.sh                    # per-user (default)
#   ./scripts/install-nm-host.sh --system            # system-wide (sudo)
#   ./scripts/install-nm-host.sh --bin /path/to/relay # custom binary path
#
# Per-user paths:
#   Chrome:  ~/.config/google-chrome/NativeMessagingHosts/
#   Chromium: ~/.config/chromium/NativeMessagingHosts/
#   Brave:    ~/.config/BraveSoftware/Brave-Browser/NativeMessagingHosts/
#   Firefox:  ~/.mozilla/native-messaging-hosts/
#
# System-wide paths:
#   Chrome:  /etc/opt/chrome/native-messaging-hosts/
#   Chromium: /etc/chromium/native-messaging-hosts/
#   Firefox:  /usr/lib/mozilla/native-messaging-hosts/

set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
MANIFESTS_DIR="$SCRIPT_DIR/manifests"

INSTALL_MODE="user"
BINARY_PATH=""

# ── Parse args ─────────────────────────────────────────────────────────────
while [[ $# -gt 0 ]]; do
  case "$1" in
    --system) INSTALL_MODE="system"; shift ;;
    --bin)    BINARY_PATH="$2"; shift 2 ;;
    --help)   head -30 "$0"; exit 0 ;;
    *)        echo "Unknown: $1"; exit 1 ;;
  esac
done

# ── Resolve binary path ────────────────────────────────────────────────────
if [ -z "$BINARY_PATH" ]; then
  RELAY_TARGET="$PROJECT_DIR/target/release/gullbur-relay"
  if [ ! -f "$RELAY_TARGET" ]; then
    echo "==> Building gullbur-relay..."
    cargo build --bin gullbur-relay --release --manifest-path "$PROJECT_DIR/Cargo.toml"
  fi
  BINARY_PATH="$RELAY_TARGET"
fi

if [ ! -f "$BINARY_PATH" ]; then
  echo "ERROR: Binary not found at $BINARY_PATH"
  echo "  Build it: cargo build --bin gullbur-relay --release"
  exit 1
fi

# Resolve to absolute path
BINARY_PATH="$(cd "$(dirname "$BINARY_PATH")" && pwd)/$(basename "$BINARY_PATH")"
chmod +x "$BINARY_PATH"
echo "==> Relay binary: $BINARY_PATH ($(du -h "$BINARY_PATH" | cut -f1))"

# ── Set install paths ──────────────────────────────────────────────────────
if [ "$INSTALL_MODE" = "system" ]; then
  CHROME_DIR="/etc/opt/chrome/native-messaging-hosts"
  CHROMIUM_DIR="/etc/chromium/native-messaging-hosts"
  BRAVE_DIR="/etc/opt/chrome/native-messaging-hosts"  # Chrome-compatible
  FIREFOX_DIR="/usr/lib/mozilla/native-messaging-hosts"
  INSTALL_CMD="install -m 644"
  echo "==> Mode: system-wide"
else
  CHROME_DIR="$HOME/.config/google-chrome/NativeMessagingHosts"
  CHROMIUM_DIR="$HOME/.config/chromium/NativeMessagingHosts"
  BRAVE_DIR="$HOME/.config/BraveSoftware/Brave-Browser/NativeMessagingHosts"
  FIREFOX_DIR="$HOME/.mozilla/native-messaging-hosts"
  INSTALL_CMD="install -m 644"
  echo "==> Mode: per-user"
fi

# ── Install manifests ──────────────────────────────────────────────────────
install_manifest() {
  local src="$1"
  local dst="$2"
  local label="$3"

  mkdir -p "$(dirname "$dst")"
  sed "s|<BINARY_PATH>|$BINARY_PATH|g" "$src" > "$dst"
  chmod 644 "$dst"
  echo "    $label: $dst"
}

install_manifest \
  "$MANIFESTS_DIR/com.gullbur.wallet.relay.chrome.json" \
  "$CHROME_DIR/com.gullbur.wallet.relay.json" \
  "Chrome"

install_manifest \
  "$MANIFESTS_DIR/com.gullbur.wallet.relay.chrome.json" \
  "$CHROMIUM_DIR/com.gullbur.wallet.relay.json" \
  "Chromium"

install_manifest \
  "$MANIFESTS_DIR/com.gullbur.wallet.relay.chrome.json" \
  "$BRAVE_DIR/com.gullbur.wallet.relay.json" \
  "Brave"

install_manifest \
  "$MANIFESTS_DIR/com.gullbur.wallet.relay.firefox.json" \
  "$FIREFOX_DIR/com.gullbur.wallet.relay.json" \
  "Firefox"

echo ""
echo "✅  gullbur-relay native messaging host installed ($INSTALL_MODE mode)."
echo ""
echo "    To register the Chrome extension ID:"
echo "      Edit $CHROME_DIR/com.gullbur.wallet.relay.json"
echo "      Replace __CHROME_EXTENSION_ID__ with your extension ID"
echo "      (found at chrome://extensions → Developer mode → extension ID)"
echo ""
echo "    Firefox uses wallet@gullbur.io — already set in the manifest."
echo "    Restart the browser to activate."