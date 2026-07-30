#!/usr/bin/env bash
# git-protect.sh — Branch protection rules for the foss-wallet repo.
#
# Run this on a bare remote to enforce receive-side protection:
#   ssh server "bash -s" < scripts/git-protect.sh /srv/git/fosswallet.git
#
# When run with a <repo-path> argument, configures that bare repo.
# When run without arguments in the repo, configures the local repo
# as if it were a remote (applies to receive-pack).

set -euo pipefail

REPO="${1:-.}"

if [ ! -d "$REPO" ]; then
    echo "Error: not a directory: $REPO"
    echo "Usage: $0 [path-to-.git-or-bare-repo]"
    exit 1
fi

cd "$REPO"

echo "=== Applying branch protection rules to: $REPO ==="

# --- Deny non-fast-forward pushes (requires --force) ---
git config receive.denyNonFastForwards true

# --- Deny deletion of branches ---
git config receive.denyDeletes true

# --- Require GPG/SSH signature on tags (uncomment when ready) ---
# git config receive.denyNonFastForwards true
# git config tag.gpgSign true

# --- Restrict refs that can be pushed ---
# Uncomment to allow pushes only to main (creates a whitelist):
# git config receive.denyNonFastForwards false   # not needed with hook
# The following uses a receive hook to whitelist branches.
# For now we rely on GitHub/GitLab per-branch settings for this.

echo ""
echo "Current receive-side config:"
git config --get-regexp '^receive\.' || echo "(none beyond defaults)"
echo "=== Done ==="
echo ""
echo "Recommended next steps:"
echo "  1. Set up a remote:   git remote add origin <url>"
echo "  2. Push:              git push origin main --tags"
echo "  3. On GitHub/GitLab:  enable 'Require linear history' and"
echo "                        'Restrict pushes that delete tags' for main."