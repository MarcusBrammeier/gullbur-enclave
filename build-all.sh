#!/usr/bin/env bash
# ──────────────────────────────────────────────────────────────────────
# Gullbúr Enclave — build-all.sh
#
# Builds EVERY shipping release variant from current HEAD and stages them
# for the test server. This is the canonical "did my change break the
# delivery path?" gate — run after any code change.
#
# Variants built:
#   1. gullbur-desktop binary (debug)         target/debug/gullbur-desktop
#   2. gullbur-desktop binary (release)       target/release/gullbur-desktop
#   3. headless CLI (release, --features headless) — Internal test harness
#   4. Linux .deb package
#   5. Linux AppImage
#   (Android APK/AAB are built via CI on tag push — see release.yml)
#
# After the build, artifacts are copied to ./apk-output/ (served by the
# test webserver) OR to $OUT_DIR if set.
#
# Exit 0 = all variants built and staged.
# ──────────────────────────────────────────────────────────────────────
set -euo pipefail

ROOT="$(cd "$(dirname "$0")" && pwd)"
cd "$ROOT"

OUT_DIR="${OUT_DIR:-$ROOT/apk-output}"
mkdir -p "$OUT_DIR"

PASS=0; FAIL=0
ok()  { echo "  ${GREEN}✓${NC} $1"; ((PASS++)); }
bad() { echo "  ${RED}✗${NC} $1"; ((FAIL++)); }
GREEN='\033[0;32m'; RED='\033[0;31m'; NC='\033[0m'

echo "═══════════════════════════════════════════════════════════"
echo "  Gullbúr Enclave — build-all.sh"
echo "  HEAD: $(git rev-parse --short HEAD)  $(git log -1 --pretty=%s | cut -c1-60)"
echo "═══════════════════════════════════════════════════════════"

echo "▸ [1/5] Frontend (dist/)"
( cd apps/desktop && npm run build ) && ok "vendored dist/" || bad "frontend build"

echo "▸ [2/5] Desktop binary (debug + release)"
cargo build -p gullbur-desktop 2>&1 | tail -1 || bad "debug build"
[ -x target/debug/gullbur-desktop ] && ok "debug binary" || bad "debug binary missing"

cargo build --release -p gullbur-desktop 2>&1 | tail -1 || bad "release build"
[ -x target/release/gullbur-desktop ] && ok "release binary ($(du -h target/release/gullbur-desktop | cut -f1))" || bad "release binary missing"

echo "▸ [3/5] Headless CLI (release, --features headless — internal)"
cargo build --release -p gullbur-cli --features headless 2>&1 | tail -1 || bad "headless CLI build"
[ -x target/release/gullbur-cli ] && ok "headless CLI" || bad "headless CLI missing"

echo "▸ [4/5] Full test sweep (10 layers)"
bash scripts/full-test-sweep.sh && ok "full test sweep" || bad "full test sweep"

echo "▸ [5/5] Stage .deb + AppImage to $OUT_DIR"
DEB="$(find target/release/bundle/deb -name '*.deb' 2>/dev/null | head -1)"
APPIMAGE="$(find target/release/bundle/appimage -name '*.AppImage' 2>/dev/null | head -1)"
if [ -n "$DEB" ]; then cp -f "$DEB" "$OUT_DIR/"; ok "staged .deb: $(basename "$DEB")"; else bad "no .deb found"; fi
if [ -n "$APPIMAGE" ]; then cp -f "$APPIMAGE" "$OUT_DIR/"; ok "staged AppImage: $(basename "$APPIMAGE")"; else bad "no AppImage found"; fi

echo ""
echo "  Result: $PASS/$((PASS+FAIL)) steps OK  →  artifacts in $OUT_DIR"
echo "  Serve with:  cd $OUT_DIR && python3 -m http.server 8080"
echo "═══════════════════════════════════════════════════════════"
exit $(( FAIL > 0 ? 1 : 0 ))