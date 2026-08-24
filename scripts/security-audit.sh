#!/usr/bin/env bash
# Release security audit (Linux). Blocking: cargo audit + Tauri capability/CSP
# guards. npm audit is advisory (frontend lockfile is already CI-gated).
set -eu
repo="$(cd "$(dirname "$0")/.." && pwd)"
cd "$repo"

echo "== cargo audit"
cargo audit

echo "== Tauri capability/CSP/updater guards"
cargo test -p clay-desktop --test config_security -- --test-threads=1 --quiet

if [ -f frontend/package-lock.json ]; then
    echo "== frontend npm audit (advisory)"
    if ! npm audit --prefix frontend --omit=dev; then
        echo "advisory: frontend npm audit reported issues (not a release blocker)"
    fi
fi

echo "security-audit PASSED"
