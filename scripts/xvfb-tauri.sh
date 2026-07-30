#!/usr/bin/env bash
# xvfb-tauri.sh — Run Tauri dev/build/package under a virtual framebuffer
# Usage:
#   ./xvfb-tauri.sh dev        # Start Tauri dev with virtual display
#   ./xvfb-tauri.sh build      # Build Tauri + relay binaries
#   ./xvfb-tauri.sh package    # Build all binaries + package extension
#   ./xvfb-tauri.sh relay     # Build relay binary only

set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
DESKTOP_DIR="$SCRIPT_DIR/../apps/desktop"

case "${1:-dev}" in
  dev)
    echo "[xvfb] Starting Tauri dev on virtual display..."
    cd "$DESKTOP_DIR"
    xvfb-run --auto-servernum --server-args="-screen 0 1280x720x24" \
      npx tauri dev
    ;;
  relay)
    echo "[xvfb] Building foss-wallet-relay..."
    cd "$PROJECT_DIR"
    cargo build --bin foss-wallet-relay --release
    ls -lh target/release/foss-wallet-relay
    ;;
  build)
    cd "$PROJECT_DIR"
    echo "[xvfb] Building relay binary..."
    cargo build --bin foss-wallet-relay --release
    echo "[xvfb] Building Tauri desktop..."
    cd "$DESKTOP_DIR"
    npx tauri build
    ;;
  package)
    cd "$PROJECT_DIR"
    echo "[xvfb] Building all binaries..."
    cargo build --bin foss-wallet-relay --release
    cd "$DESKTOP_DIR"
    npx tauri build
    echo "[xvfb] Packaging browser extension..."
    cd "$PROJECT_DIR"
    bash scripts/package-extension.sh
    echo "[xvfb] All builds complete. Artifacts in:"
    ls -lh target/release/foss-wallet-relay
    ls -lh target/release/bundle/
    ls -lh dist/
    ;;
  *)
    echo "Usage: $0 {dev|build|package|relay}"
    exit 1
    ;;
esac
