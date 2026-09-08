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
npm_wrapper="$(python3 -c "import json; print(json.load(open('distribution/npm/package.json'))['version'])")"

echo "versions: crate=$root desktop=$desktop tauri=$tauri frontend=$frontend agent=$agent npm_wrapper=$npm_wrapper"
if [ "$desktop" != "$root" ] || [ "$desktop" != "$tauri" ] || [ "$desktop" != "$frontend" ] || [ "$desktop" != "$agent" ] || [ "$desktop" != "$npm_wrapper" ]; then
    echo "version mismatch across release artifacts (includes @arnilo/clay wrapper)" >&2
    exit 1
fi

test -f src-tauri/icons/icon.png
test -x scripts/security-audit.sh

if command -v node >/dev/null 2>&1; then
    echo "== canonical example configuration syntax"
    node --check examples/config/init.js
    node --check examples/config/packages/first-party.js
    node --check examples/config/packages/third-party.js
else
    echo "node not found; skipping canonical example node --check"
fi

echo "== distribution checks (plan 115 task 8)"
python3 - <<'PY'
import json
pkg = json.load(open('distribution/npm/package.json'))
assert pkg['name'] == '@arnilo/clay', pkg['name']
assert pkg.get('scripts', {}) == {}, f"lifecycle scripts present: {pkg.get('scripts')}"
assert 'postinstall' not in pkg and 'preinstall' not in pkg
PY
if command -v npm >/dev/null 2>&1; then
    echo "== npm pack dry-run (@arnilo/clay wrapper)"
    (cd distribution/npm && npm pack --dry-run --pack-destination "$(mktemp -d)" >/dev/null)
else
    echo "npm not found; skipping npm pack dry-run"
fi

dist_tmp="$(mktemp -d)"
trap 'rm -rf "$dist_tmp"' EXIT
printf '#!/bin/sh\necho clay-check\n' > "$dist_tmp/clay-0.0.0-smoke-linux-x64"
chmod +x "$dist_tmp/clay-0.0.0-smoke-linux-x64"
sha256sum "$dist_tmp/clay-0.0.0-smoke-linux-x64" > "$dist_tmp/clay-0.0.0-smoke-linux-x64.sha256"
sh distribution/install.sh --version 0.0.0-smoke --artifact-dir "$dist_tmp" --bindir "$dist_tmp/bin" >/dev/null
python3 - "$dist_tmp/bin/channel.json" <<'PY'
import json, sys
marker = json.load(open(sys.argv[1]))
assert marker["version"] == 1 and marker["channel"] == "curl", marker
assert isinstance(marker["argv"], list) and marker["argv"], marker
assert all(isinstance(a, str) and a for a in marker["argv"]), marker
assert any(a.endswith("install.sh") for a in marker["argv"]), marker
PY
# Tampered artifact must be rejected before install.
printf 'x' >> "$dist_tmp/clay-0.0.0-smoke-linux-x64"
if sh distribution/install.sh --version 0.0.0-smoke --artifact-dir "$dist_tmp" --bindir "$dist_tmp/bin-tampered" >/dev/null 2>&1; then
    echo "install.sh accepted a tampered artifact" >&2
    exit 1
fi
echo "distribution checks PASSED"

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
