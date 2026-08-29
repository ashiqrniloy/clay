#!/usr/bin/env bash
# Rebuild the production renderer and debug Clay binaries.
#   scripts/build.sh              # frontend/dist + debug clay and clay-desktop
#   scripts/build.sh run          # same, then cargo run (GUI)
#   scripts/build.sh run -- client
set -eu

repo="$(cd "$(dirname "$0")/.." && pwd)"
cd "$repo"

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
