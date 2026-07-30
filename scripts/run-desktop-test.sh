#!/usr/bin/env bash
export DISPLAY=:99
export RUST_LOG=info
exec "$(dirname "$0")/../target/release/gullbur-desktop" "$@"