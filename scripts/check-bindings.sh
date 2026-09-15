#!/usr/bin/env bash
# Webview contract staleness guard (plan 119 SC-1).
#
# `frontend/src/bridge/generated/` is written from the Rust DTO layer by a
# feature-gated codegen test. Regenerate it here and fail when the working tree
# no longer matches, so a contract change can never land with stale bindings.
set -eu

repo="$(cd "$(dirname "$0")/.." && pwd)"
cd "$repo"

cargo test -p clay-desktop --features ts-bindings --lib --quiet \
    export_webview_contract_bindings

changes="$(git status --porcelain -- frontend/src/bridge/generated)"
if [ -n "$changes" ]; then
    printf '%s\n' "$changes" >&2
    echo "stale webview bindings: frontend/src/bridge/generated is not what the DTO layer produces." >&2
    echo "Commit the regenerated output above (scripts/check-bindings.sh regenerates it)." >&2
    exit 1
fi

echo "webview bindings up to date"
