#!/usr/bin/env bash
# Run YouDown from the source tree (debug-friendly).
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

if [[ ! -x "$ROOT/target/release/youdown" ]]; then
    cargo build --release --manifest-path "$ROOT/Cargo.toml"
fi

exec "$ROOT/target/release/youdown" "$@"
