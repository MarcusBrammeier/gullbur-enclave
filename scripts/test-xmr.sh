#!/bin/bash
set -e
echo "=== XMR Plugin Test ==="
CLI="/app/target/release/gullbur-cli"

$CLI launch --port 19878 &
VAULT_PID=$!
sleep 2

$CLI --port 19878 init "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about"
echo "INIT OK"

ACCT=$($CLI --port 19878 create-account monero 0)
echo "Account: $ACCT"
echo "$ACCT" | grep -q "4" && echo "XMR ADDRESS OK" || (echo "BAD ADDRESS"; exit 1)

$CLI --port 19878 list-networks | grep -q "monero" && echo "NETWORKS OK"

kill $VAULT_PID 2>/dev/null
echo "=== XMR DONE ==="