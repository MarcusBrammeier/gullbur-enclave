#!/bin/bash
set -e
echo "=== EVM Plugin Test ==="
CLI="/app/target/release/gullbur-cli"

$CLI launch --port 19877 &
VAULT_PID=$!
sleep 2

$CLI --port 19877 init "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about"
echo "INIT OK"

ACCT=$($CLI --port 19877 create-account ethereum 0)
echo "$ACCT" | grep -q "0x" && echo "ETH ADDRESS OK" || (echo "BAD ADDRESS"; exit 1)

$CLI --port 19877 list-networks | grep -q "ethereum" && echo "NETWORKS OK"
$CLI --port 19877 validate-address ethereum 0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045

kill $VAULT_PID 2>/dev/null
echo "=== EVM DONE ==="