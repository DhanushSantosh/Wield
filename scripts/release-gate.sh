#!/usr/bin/env bash
set -euo pipefail

tag="${1:?Usage: bash scripts/release-gate.sh vX.Y.Z[-beta.N]}"
bundle_dir="${CARGO_TARGET_DIR:-target}/release/bundle/appimage"
node --test scripts/release-check.test.mjs
node scripts/release-check.mjs manifest "$tag"
npm ci
npm audit --audit-level=high
cargo audit
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --locked -- --test-threads=1
npm run check
mkdir -p "$bundle_dir"
find "$bundle_dir" -maxdepth 1 -type f -name '*.AppImage' -delete
NO_STRIP=1 APPIMAGE_EXTRACT_AND_RUN=1 npm run build -w apps/wield -- --bundles appimage
node scripts/release-check.mjs artifact "$tag" "$bundle_dir"
cd "$bundle_dir"
sha256sum --check SHA256SUMS
