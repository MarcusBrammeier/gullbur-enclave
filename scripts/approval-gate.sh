#!/usr/bin/env bash
# approval-gate.sh — Pre-merge HITL gate script.
#
# Can be used as a git hook (pre-push or pre-commit) or CI step.
#   - Runs approval-check.sh on the current HEAD.
#   - If all checks pass, creates a signed tag  approved-<short-sha>.
#   - If any check fails, exits 1 with a summary of failures.
#
# Usage:
#   ./scripts/approval-gate.sh                # check current HEAD
#   ./scripts/approval-gate.sh <sha|branch>    # check target

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
APPROVAL_CHECK="${ROOT_DIR}/scripts/approval-check.sh"
TARGET="${1:-HEAD}"

# Colours
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
NC='\033[0m' # No Color

if [ ! -x "$APPROVAL_CHECK" ]; then
    echo -e "${RED}Error: approval-check.sh not found or not executable at:${NC}"
    echo "  $APPROVAL_CHECK"
    exit 1
fi

echo -e "${CYAN}========================================${NC}"
echo -e "${CYAN}  HITL Pre-Merge Gate${NC}"
echo -e "${CYAN}========================================${NC}"
echo "Target:    $TARGET"
echo ""

# Run the approval check
if ! "$APPROVAL_CHECK" "$TARGET"; then
    echo -e ""
    echo -e "${RED}========================================${NC}"
    echo -e "${RED}  ❌ GATE FAILED${NC}"
    echo -e "${RED}========================================${NC}"
    echo ""
    echo "One or more approval checks failed."
    echo "Fix the issues above and re-run the gate."
    echo ""
    echo "To see the full report:"
    echo "  ls -lt ${ROOT_DIR}/scripts/approval-reports/"
    exit 1
fi

# All checks passed — create a signed tag
SHORT_SHA="$(git rev-parse --short "$TARGET" 2>/dev/null || echo "unknown")"
TAG_NAME="approved-${SHORT_SHA}"

echo -e ""
echo -e "${GREEN}========================================${NC}"
echo -e "${GREEN}  ✅ All Checks Passed${NC}"
echo -e "${GREEN}========================================${NC}"
echo ""

# Check if the tag already exists
if git tag -l "$TAG_NAME" | grep -q "^${TAG_NAME}$"; then
    echo -e "${YELLOW}Tag ${TAG_NAME} already exists. Removing and re-creating.${NC}"
    git tag -d "$TAG_NAME"
fi

# Create signed tag
echo "Creating signed tag: $TAG_NAME"
if git tag -s "$TAG_NAME" -m "Approved commit ${SHORT_SHA} — all HITL checks passed $(date -u +%Y-%m-%dT%H:%M:%SZ)"; then
    echo -e "${GREEN}✅ Signed tag created: ${TAG_NAME}${NC}"
    echo ""
    echo "Tag details:"
    git tag -v "$TAG_NAME" 2>&1 || echo -e "${YELLOW}(Tag verification requires the signing key to be available)${NC}"
else
    echo -e "${YELLOW}⚠ Signed tag creation failed (GPG key may not be configured).${NC}"
    echo "Creating unsigned tag as fallback."
    git tag -a "$TAG_NAME" -m "Approved commit ${SHORT_SHA} — all HITL checks passed $(date -u +%Y-%m-%dT%H:%M:%SZ)"
    echo -e "${YELLOW}⚠ Unsigned tag created: ${TAG_NAME} (consider configuring GPG signing)${NC}"
fi

echo ""
echo -e "${GREEN}Gate passed. Tag ${TAG_NAME} is ready.${NC}"
exit 0