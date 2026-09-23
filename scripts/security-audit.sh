#!/usr/bin/env bash
# Release security audit (Linux). Blocking: cargo audit + Tauri capability/CSP
# guards. bun audit is advisory (frontend lockfile is already CI-gated) and
# covers devDependencies too (bun 1.4.2 has no --omit=dev equivalent).
set -eu
repo="$(cd "$(dirname "$0")/.." && pwd)"
cd "$repo"

echo "== cargo audit"
cargo audit

echo "== Tauri capability/CSP/updater guards"
cargo test -p clay-desktop --test config_security -- --test-threads=1 --quiet

if [ -f frontend/bun.lock ]; then
    echo "== frontend bun audit (advisory)"
    if ! (cd frontend && bun audit); then
        echo "advisory: frontend bun audit reported issues (not a release blocker)"
    fi
fi

echo "security-audit PASSED"
