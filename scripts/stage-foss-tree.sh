#!/usr/bin/env bash
# ──────────────────────────────────────────────────────────────────────
# Gullbúr Enclave — Clean FOSS Tree Staging
#
# Prepares the public v0.1.0-beta.1 FOSS staging repository by cloning the
# private dev repo and stripping everything internal per docs/FOSS_BOUNDARY.md.
#
# The public tree contains ONLY the core FOSS surface:
#   - crate engine   (crates/)
#   - desktop GUI    (apps/desktop + browser extension)
#   - fuzz targets, integration tests
#   - public docs (README, LICENSE, ARCHITECTURE, CONTRIBUTING, SECURITY,
#     DONATIONS — scrubbed of internal notes)
#
# EXCLUDED from public (kept private / Pro-Enterprise):
#   - apps/cli                 (Pro/Enterprise headless CLI + test harness)
#   - .hermes/, PLANS/, docs/plans/   (internal plans/roadmap)
#   - STATE.md, docs/FOSS_BOUNDARY.md (internal state; STALE brand refs)
#   - scripts/ internal tooling       (sweeps, downloaders — internal)
#   - debug logs, .env, test wallets, signing keystore
#
# Exit 0 = staging succeeded.
# ──────────────────────────────────────────────────────────────────────
set -euo pipefail

DEV_REPO="/root/fosscryptocore-new"
STAGE_DIR="${STAGE_DIR:-/root/gullbur-foss-staging}"
STAGE_REPO="$STAGE_DIR/gullbur-enclave"
# Public stable snapshot tag + bundled version (internal develops 0.1.x freely).
TAG="${TAG:-v0.1.0-beta}"
BUNDLED_VER="${BUNDLED_VER:-0.1.0-beta}"

cd "$DEV_REPO"
HEAD=$(git rev-parse --short HEAD)

echo "Staging FOSS tree from $DEV_REPO @ $HEAD → $STAGE_REPO"
rm -rf "$STAGE_REPO"
mkdir -p "$STAGE_DIR"

# Shallow clone the private repo (no internal history leaked).
git clone --quiet --no-local "$DEV_REPO" "$STAGE_REPO"
cd "$STAGE_REPO"

echo "┌─ Excluding internal/Pro paths ─"
# Build the exclusion list (git pathspec). Each excludes keeps the file out of
# the staged tree AND out of index.
EXCLUDES=(
  "apps/cli"                    # Pro/Enterprise headless CLI
  "apps/desktop/src-tauri/gen"  # build artifacts (APK/AAB/etc)
  ".hermes"                     # internal plans
  "PLANS"
  "docs/plans"
  "STATE.md"                    # internal state doc
  "docs/FOSS_BOUNDARY.md"       # internal boundary doc
  "scripts"                     # internal test/download tooling
  "test.Dockerfile"
  ".env" ".env.*"
  "*.log"
  "docs/CODE_SWEEP.md"
)

# Remove from working tree + index.
for pat in "${EXCLUDES[@]}"; do
  if git ls-files -- "$pat" | grep -q .; then
    git rm -rq --ignore-unmatch "$pat" 2>/dev/null || true
    echo "  ✂ removed: $pat"
  fi
done

echo "┌─ Verify no stale internal paths remain ─"
STALE=$(git ls-files | grep -E '^(apps/cli|\.hermes|PLANS|docs/plans|STATE\.md|scripts/|docs/FOSS_BOUNDARY\.md)/' || true)
if [ -n "$STALE" ]; then
  echo "  ✗ Stale internal files still tracked:"; echo "$STALE"
  exit 1
fi

echo "┌─ Prune workspace members the public tree cannot build (apps/cli) ─"
# apps/cli (Pro/Enterprise headless CLI) is stripped from the public tree, so it
# must also be dropped from [workspace] members or `cargo build --workspace`
# fails on a public clone. cli-integration + fuzz only depend on the shipped
# crates, so they stay.
if [ -f Cargo.toml ]; then
  sed -i '/^[[:space:]]*"apps\/cli",[[:space:]]*$/d' Cargo.toml
  echo "  ✂ removed \"apps/cli\" from workspace members"
fi

echo "┌─ Align bundled version to the public snapshot ($BUNDLED_VER) ─"
# Set Cargo package version, package.json, and tauri.conf to the public snapshot
# so the shipped artifact and tag are self-consistent (internal stays on 0.x).
sed -i "s/^version = \"[^\"]*\"/version = \"$BUNDLED_VER\"/" Cargo.toml 2>/dev/null || true
if [ -f apps/desktop/package.json ]; then
  sed -i "s/\"version\": \"[^\"]*\"/\"version\": \"$BUNDLED_VER\"/" apps/desktop/package.json
fi
if [ -f apps/desktop/src-tauri/tauri.conf.json ]; then
  sed -i "s/\"version\": \"[^\"]*\"/\"version\": \"$BUNDLED_VER\"/" apps/desktop/src-tauri/tauri.conf.json
fi
echo "  (Cargo.toml + package.json + tauri.conf  → $BUNDLED_VER)"

echo "┌─ Strip internal-only sections from public docs ─"
# Remove any trailing "internal doc" lines from CODE_SWEEP-style content, if present.
for f in docs/*.md README.md; do [ -f "$f" ] && sed -i '/\*Internal doc.*scrub before public repo release.*\*/d' "$f" 2>/dev/null || true; done

echo "┌─ Drop inherited dev tags (must not ship to public) ─"
# The shallow clone inherits all dev repo tags (v0.0.x, beta.x). A clean
# public FOSS repo must start with only the intended release tag.
for t in $(git tag); do
  git tag -d "$t" >/dev/null 2>&1 || true
done
echo "  dropped $(git tag | wc -l) leftover tags → ${TAG} only"

echo "┌─ Commit clean FOSS tree ─"
git add -A
if [ -n "$(git diff --cached --stat)" ]; then
  git -c user.name="Gullbur Release" -c user.email="release@gullbur.local" \
    commit -q -m "chore: stage clean FOSS tree ($TAG) from dev HEAD $HEAD"
fi

echo "┌─ Tag ─"
git -c user.name="Gullbur Release" -c user.email="release@gullbur.local" \
  tag -a "$TAG" -m "$TAG — clean public FOSS staging"

echo ""
echo "═══════════════════════════════════════════════"
echo "  FOSS staging ready: $STAGE_REPO @ $TAG"
echo "  Contents:"
git ls-files | sed 's/^/    /' | head -40
echo "  (… $(git ls-files | wc -l) files total)"
echo "═══════════════════════════════════════════════"