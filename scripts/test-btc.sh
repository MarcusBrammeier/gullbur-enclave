#!/bin/bash
set -e
echo "=== BTC Plugin Test ==="
CLI="/app/target/release/gullbur-cli"

$CLI launch --port 19876 &
VAULT_PID=$!
sleep 2

$CLI --port 19876 init "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about"
echo "INIT OK"

ACCT=$($CLI --port 19876 create-account bitcoin 0)
echo "Account: $ACCT"
echo "$ACCT" | grep -q "bc1" && echo "BTC ADDRESS OK" || (echo "BAD ADDRESS"; exit 1)

$CLI --port 19876 validate-address bitcoin bc1qw508d6qejxtdg4y5r3zarvary0c5xw7kv8f3t4
$CLI --port 19876 validate-address bitcoin not-an-address

kill $VAULT_PID 2>/dev/null
echo "=== BTC DONE ==="