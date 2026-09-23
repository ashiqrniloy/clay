#!/usr/bin/env bash
# Plan 098 manual large-document desktop smoke.
#
# Generates synthetic fixtures (never committed), starts a Clay server whose
# workspace is the fixture directory, opens the Tauri/React desktop, and prints
# the manual checklist. Automated counterpart: `cargo test --test runtime
# large_document::`.
set -eu
repo="$(cd "$(dirname "$0")/.." && pwd)"
cd "$repo"

echo "== building clay binary"
cargo build --bin clay --quiet

ws="$(mktemp -d /tmp/clay-large-doc-smoke.XXXXXX)"
socket="$ws/clay.sock"
bin="$repo/target/debug/clay"

desktop_pid=""
server_pid=""
cleanup() {
    if [ -n "$desktop_pid" ]; then
        pkill -TERM -P "$desktop_pid" 2>/dev/null || true
        kill "$desktop_pid" 2>/dev/null || true
    fi
    if [ -n "$server_pid" ]; then kill "$server_pid" 2>/dev/null || true; fi
    rm -rf "$ws"
}
trap cleanup EXIT

echo "== generating fixtures in $ws"
"$bin" perf-fixture --kind mixed-unicode --size-mib 50 \
    --output target/perf-fixtures/large-50m.txt
cp target/perf-fixtures/large-50m.txt "$ws/large.md"
truncate -s 257M "$ws/oversize.txt"   # sparse; exceeds the 256 MiB resident budget
printf 'hello binary\0content after a NUL byte' > "$ws/binary.dat"

echo "== starting server (workspace: $ws, socket: $socket)"
(cd "$ws" && exec "$bin" server "$socket") &
server_pid=$!
for _ in $(seq 1 50); do
    [ -S "$socket" ] && break
    kill -0 "$server_pid" 2>/dev/null || exit 1
    sleep 0.1
done
[ -S "$socket" ] || { echo "server socket did not appear: $socket" >&2; exit 1; }

echo "== launching desktop client (close its window to finish)"
"$bin" client "$socket" &
desktop_pid=$!

cat <<'CHECKLIST'

Manual checklist (Plan 098):
  1. Open file -> large.md: pane shows "Loading full document..." status,
     text paints immediately, full document ready within seconds.
  2. Type at the top of large.md after ready: edits apply locally, Save and
     Reload round-trip without diagnostics.
  3. Open oversize.txt: visible refusal naming the resident document budget
     (status bar + empty pane); app stays responsive.
  4. Open binary.dat: visible refusal saying the file appears binary.
  5. Close the desktop window; this script stops the server and removes
     the fixtures.

CHECKLIST

wait "$desktop_pid"
kill "$server_pid" 2>/dev/null || true
