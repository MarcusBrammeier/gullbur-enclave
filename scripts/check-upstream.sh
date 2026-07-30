#!/usr/bin/env bash
# check-upstream.sh — Check for upstream dependency updates in the workspace.
#
# Runs `cargo update --dry-run`, parses the output to identify available
# updates, flags major version bumps as BREAKING, and writes a timestamped
# summary to stdout.
#
# Exit codes:
#   0 — No breaking changes detected (or no updates at all)
#   1 — At least one breaking (major-version-bump) update found
#
# Dependencies: bash, cargo, grep, awk, sort, cut, date, cat, head
# No external tools beyond standard Unix utilities and the Rust toolchain.

set -euo pipefail

# ── Config ──────────────────────────────────────────────────────────
WORKSPACE_DIR="$(cd "$(dirname "$0")/.." && pwd)"
TIMESTAMP="$(date -u '+%Y-%m-%dT%H:%M:%SZ')"
BREAKING=0
HAS_UPDATES=0

# ── Helpers ─────────────────────────────────────────────────────────

# Determine if a version change is a breaking (major) bump.
# Semantics: if old and new differ in the MAJOR segment of semver.
# For 0.x.y pre-1.0, the *minor* segment is treated as the major version
# (e.g. 0.22 → 0.23 is breaking,  0.22 → 0.22.1 is not).
# For 1.x.y → 2.x.y, the major bump is breaking.
is_breaking() {
    local old_ver="$1"
    local new_ver="$2"

    # Strip leading 'v' if present
    old_ver="${old_ver#v}"
    new_ver="${new_ver#v}"

    # Split into segments
    local old_major="${old_ver%%.*}"
    local rest="${old_ver#*.}"
    local old_minor="${rest%%.*}"

    local new_major="${new_ver%%.*}"
    local rest2="${new_ver#*.}"
    local new_minor="${rest2%%.*}"

    # For pre-1.0 (major == 0), a minor-segment change = breaking
    if [ "$old_major" = "0" ]; then
        if [ "$old_minor" != "$new_minor" ]; then
            return 0  # breaking
        fi
        return 1  # not breaking
    fi

    # For >=1.0, a major-segment change = breaking
    if [ "$old_major" != "$new_major" ]; then
        return 0  # breaking
    fi
    return 1  # not breaking
}

# ── Header ──────────────────────────────────────────────────────────
echo "═══════════════════════════════════════════════════════════════"
echo "  Upstream Dependency Check"
echo "  Timestamp:  $TIMESTAMP"
echo "  Workspace:  $WORKSPACE_DIR"
echo "═══════════════════════════════════════════════════════════════"
echo ""

# ── Run cargo update --dry-run ──────────────────────────────────────
# Redirect stderr to a temp file so we can capture both stdout and stderr
DRY_RUN_OUT="$WORKSPACE_DIR/.cargo-dry-run-output.tmp"
trap 'rm -f "$DRY_RUN_OUT"' EXIT

echo "→ Running: cargo update --dry-run ..."
echo ""

if ! cd "$WORKSPACE_DIR" 2>/dev/null; then
    echo "ERROR: Cannot change to workspace directory: $WORKSPACE_DIR"
    exit 1
fi

# Run cargo update --dry-run; capture stdout (update lines) and stderr (logging)
# We run with pipefail off for this step so we can capture partial output
set +o pipefail
cargo update --dry-run 2>&1 | tee "$DRY_RUN_OUT" || true
set -o pipefail

echo ""

# ── Parse update lines ─────────────────────────────────────────────
# Expected lines look like:
#     Updating foo v1.2.3 -> v1.3.0
#     Updating bar v0.5.0 -> v0.6.0
#     Updating baz v2.0.0 -> v3.0.0

UPDATES=$(grep -E '^ +Updating ' "$DRY_RUN_OUT" || true)

if [ -z "$UPDATES" ]; then
    echo "✓ No upstream updates available — all dependencies are current."
    echo ""
    echo "═══════════════════════════════════════════════════════════════"
    echo "  Result: NO UPDATES  |  Breaking: 0  |  Exit: 0"
    echo "═══════════════════════════════════════════════════════════════"
    exit 0
fi

HAS_UPDATES=1

# ── Build per-crate summary ─────────────────────────────────────────
echo "───────────────────────────────────────────────────────────────"
echo "  UPDATES AVAILABLE"
echo "───────────────────────────────────────────────────────────────"
echo ""
printf "%-40s %-20s %-20s %s\n" "CRATE" "CURRENT" "AVAILABLE" "STATUS"
printf "%-40s %-20s %-20s %s\n" "─────" "───────" "─────────" "──────"

while IFS= read -r line; do
    # Parse: "    Updating <crate> v<old> -> v<new>"
    # Use awk for robust field splitting
    crate=$(echo "$line" | awk '{print $2}')
    old_ver=$(echo "$line" | awk '{print $3}' | sed 's/^v//')
    new_ver=$(echo "$line" | awk '{print $5}' | sed 's/^v//')

    if [ -z "$crate" ] || [ -z "$old_ver" ] || [ -z "$new_ver" ]; then
        continue
    fi

    status="✓ non-breaking"
    if is_breaking "$old_ver" "$new_ver"; then
        status="⚠ BREAKING"
        BREAKING=1
    fi

    printf "%-40s %-20s %-20s %s\n" "$crate" "v$old_ver" "v$new_ver" "$status"
done <<< "$UPDATES"

echo ""
echo "───────────────────────────────────────────────────────────────"
echo "  SUMMARY"
echo "───────────────────────────────────────────────────────────────"
echo ""

TOTAL=$(echo "$UPDATES" | grep -c . || true)
echo "  Total updates available : $TOTAL"
echo "  Breaking changes        : $BREAKING"

if [ "$BREAKING" -eq 1 ]; then
    echo ""
    echo "  ⚠ BREAKING CHANGES DETECTED — review before updating."
fi

echo ""
echo "═══════════════════════════════════════════════════════════════"
echo "  Result: $( [ "$BREAKING" -eq 1 ] && echo 'BREAKING' || echo 'SAFE' )  |  Breaking: $BREAKING  |  Exit: $([ "$BREAKING" -eq 1 ] && echo '1' || echo '0')"
echo "═══════════════════════════════════════════════════════════════"

exit "$BREAKING"