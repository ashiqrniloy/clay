#!/usr/bin/env bash
# Rebuild the production renderer and debug Clay binaries.
#   scripts/build.sh              # frontend/dist + debug clay and clay-desktop
#   scripts/build.sh run          # same, then cargo run (GUI)
#   scripts/build.sh run -- client
set -eu

repo="$(cd "$(dirname "$0")/.." && pwd)"
cd "$repo"

# A running Clay keeps serving the OLD build: the desktop supervises its own
# server and the daemon is that server's child, so a rebuild alone is not what
# you then see (and a live server still owns the endpoint socket). Stop them
# first. The pattern is anchored to this checkout's target/ so cargo/rustc
# (whose argv mentions target/debug/deps) and unrelated processes never match.
clay_procs="^$repo/target/(debug|release)/clay(-server|-desktop)?( |$)|node [^ ]*clay-agent/dist/main\.js( |$)"
if pgrep -f "$clay_procs" >/dev/null 2>&1; then
  echo "== stopping running clay: $(pgrep -f "$clay_procs" | tr '\n' ' ')"
  pkill -f "$clay_procs" 2>/dev/null || true
  for _ in $(seq 1 40); do
    pgrep -f "$clay_procs" >/dev/null 2>&1 || break
    sleep 0.1
  done
  pkill -9 -f "$clay_procs" 2>/dev/null || true
  sleep 0.2
fi

echo "== frontend"
(cd frontend && npm run build)

if [ "${1:-}" = "run" ]; then
  shift
  if [ "${1:-}" = "--" ]; then shift; fi
  echo "== cargo"
  cargo build -p clay -p clay-desktop
  echo "== run"
  exec "$repo/target/debug/clay" "$@"
fi
if [ $# -ne 0 ]; then
  echo "usage: $0 [run [-- clay-args...]]" >&2
  exit 2
fi

echo "== cargo"
cargo build -p clay -p clay-desktop
echo "built frontend/dist + target/debug/clay + target/debug/clay-desktop"
