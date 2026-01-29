#!/usr/bin/env bash
set -euo pipefail

if [[ ! -f "Cargo.toml" ]]; then
  echo "ERROR: Cargo.toml not found. Run this script from the repo root." >&2
  exit 1
fi

echo "Running cargo fmt..."
cargo fmt --all

echo "Running cargo test..."
cargo test --all

OUT=/tmp/stbl-out
rm -rf "$OUT"

echo "Building pagination fixture..."
cargo run -p stbl_cli -- build \
  -s crates/stbl_core/tests/fixtures/site-pagination \
  --out "$OUT"

if [[ ! -f "$OUT/index.html" ]]; then
  echo "ERROR: $OUT/index.html not found" >&2
  exit 1
fi

PAGE_MATCH=$(find "$OUT" -type f \( -path "*/page/2/*" -o -name "*page*2*.html" \) | head -n 1)
if [[ -z "$PAGE_MATCH" ]]; then
  echo "ERROR: No paginated page found (page 2 or higher)" >&2
  exit 1
fi

if grep -R '<span class="meta"></span>' "$OUT" >/dev/null; then
  echo "ERROR: Found empty meta spans" >&2
  exit 1
fi

if ! grep -E '\b20[0-9]{2}-[0-9]{2}-[0-9]{2}\b' "$OUT/index.html" >/dev/null; then
  echo "ERROR: No YYYY-MM-DD date found in index.html" >&2
  exit 1
fi

TAG_MATCH=$(find "$OUT" -type f -path "*/tags/*" | head -n 1)
if [[ -z "$TAG_MATCH" ]]; then
  echo "ERROR: No tag page found under $OUT" >&2
  exit 1
fi

echo "Validation OK"
