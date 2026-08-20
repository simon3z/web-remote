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

# HTML/CSS/JS UI structural check (the UI is a single hand-written file with
# no build step, so we assert invariants directly rather than run a formatter
# that would just churn it). The deep contract checks (buttons match the
# Rust Key enum, no hardcoded secret, no external assets) live in `cargo
# test` (ui::tests); this only guards the overall structure.
python3 - <<'PY'
from lxml import html
doc = open("src/ui/index.html").read()
tree = html.fromstring(doc)
assert tree is not None and tree.tag == "html", "top-level must be <html>"
assert tree.find(".//body") is not None, "missing <body>"
buttons = tree.findall(".//button")
assert len(buttons) == 11, f"expected 11 buttons, got {len(buttons)}"
grid = tree.find('.//div[@class="grid"]')
assert grid is not None, "missing .grid container"
assert tree.find('.//div[@id="hint"]') is not None, "missing #hint"
for b in buttons:
    assert b.get("data-key"), "button missing data-key"
    assert b.get("data-icon"), "button missing data-icon"
    assert b.get("aria-label"), "button missing aria-label"
print("UI HTML structure OK")
PY

# Cognitive complexity threshold (arborist).
# Most complex function: qr_to_ansi_text = 9. Threshold 20 leaves generous
# headroom while still catching a function that grows into a tangle.
arborist src/ --threshold 20 --exceeds-only

echo "✓ All checks passed."
