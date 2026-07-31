#!/usr/bin/env bash
set -euo pipefail

# Audit script for Gullbúr Enclave.
# Suppresses known transitive unmaintained warnings that are brought in
# by Tauri / system deps and cannot be resolved by this project alone.
# When a transitive dep becomes unmaintained, add its crate name here
# so the weekly CI audit keeps clean output while still catching real
# vulnerabilities and direct dep issues.

cargo audit \
    --ignore RUSTSEC-2024-0344 \
    --ignore RUSTSEC-2024-0373 \
    --ignore RUSTSEC-2025-0098 \
    "$@"
