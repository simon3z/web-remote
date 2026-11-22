#!/usr/bin/env bash
# CI quality gate for web-remote.
# Usage: ./ci.sh
set -euo pipefail
cd "$(dirname "$0")"

cargo fmt --check
cargo clippy --all-targets -- -D warnings
RUSTDOCFLAGS="-D warnings" cargo doc --no-deps
cargo package --allow-dirty
cargo test --benches

# UI structure + contract checks (the UI is a single hand-written HTML
# file with no build step): the `ui::tests` module asserts the invariants
# directly — buttons match the Rust Key enum, no hardcoded secret, no
# external assets, stable document shell and button count.

echo "✓ All checks passed."
