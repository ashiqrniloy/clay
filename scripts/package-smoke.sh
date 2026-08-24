#!/usr/bin/env bash
# Structural packaged-release smoke. Does not produce a .deb unless
# CLAY_TAURI_BUNDLE=1 (needs icons + sidecars + Tauri CLI).
set -eu
repo="$(cd "$(dirname "$0")/.." && pwd)"
cd "$repo"

version_toml() {
    # First package.version in a Cargo.toml (skip dependency versions).
    awk '/^\[package\]/{p=1} p&&/^version = /{gsub(/"/,"",$3); print $3; exit}' "$1"
}

desktop="$(version_toml src-tauri/Cargo.toml)"
root="$(version_toml Cargo.toml)"
tauri="$(python3 -c "import json; print(json.load(open('src-tauri/tauri.conf.json'))['version'])")"
frontend="$(python3 -c "import json; print(json.load(open('frontend/package.json'))['version'])")"
agent="$(python3 -c "import json; print(json.load(open('clay-agent/package.json'))['version'])")"

echo "versions: crate=$root desktop=$desktop tauri=$tauri frontend=$frontend agent=$agent"
if [ "$desktop" != "$root" ] || [ "$desktop" != "$tauri" ] || [ "$desktop" != "$frontend" ] || [ "$desktop" != "$agent" ]; then
    echo "version mismatch across release artifacts" >&2
    exit 1
fi

test -f src-tauri/icons/icon.png
test -x scripts/security-audit.sh

if command -v node >/dev/null 2>&1; then
    echo "== canonical example configuration syntax"
    node --check examples/init.js
    node --check examples/packages/first-party.js
    node --check examples/packages/third-party.js
else
    echo "node not found; skipping canonical example node --check"
fi

echo "== release policy + missing-artifact tests"
cargo test -p clay-desktop release -- --test-threads=1 --quiet
cargo test -p clay-desktop missing_server_binary -- --test-threads=1 --quiet

if [ -d frontend/dist/assets ]; then
    echo "== frontend bundle budget"
    npm run check:budget --prefix frontend
fi

if [ -d clay-agent/node_modules ]; then
    echo "== clay-agent tests"
    npm test --prefix clay-agent
fi

if [ "${CLAY_TAURI_BUNDLE:-}" = "1" ]; then
    echo "== tauri bundle (opt-in)"
    command -v cargo-tauri >/dev/null || { echo "cargo-tauri missing" >&2; exit 1; }
    cargo tauri build --bundles deb
fi

echo "package-smoke PASSED"
echo "install/uninstall: use the host package manager on the produced deb/rpm;"
echo "the desktop shell has no in-app updater (unsigned payloads cannot apply)."
