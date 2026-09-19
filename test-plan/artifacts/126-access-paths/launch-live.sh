#!/usr/bin/env bash
# Plan 126 access-path manual steps: isolated live launch that stays up.
#
# Mirrors scripts/capture-ui-review.sh's isolation (private mode-700
# config/data/socket root, fixture-only documents, canonical completion
# fixture init.js) but does not tear the app down, so a probe can inspect the
# live tree repeatedly (wait for ready, dump the editor interfaces, insert at
# the end of a ≥4 MiB document).
#
#   test-plan/artifacts/126-access-paths/launch-live.sh start [bytes]
#   test-plan/artifacts/126-access-paths/launch-live.sh stop
set -Eeuo pipefail
umask 077

here=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
repo=$(cd "$here/../../.." && pwd)
root=${CLAY_LIVE_ROOT:-/tmp/clay-plan126-live}
bytes=${2:-4194304}

# `clay client` spawns the `clay-desktop` webview as a child, so stopping the
# launcher alone leaves a stale disconnected window behind (and a second launch
# then answers "Reconnect session" instead of connecting). Kill the whole tree.
kill_tree() {
    local pid=$1
    local child
    for child in $(pgrep -P "$pid" 2>/dev/null || true); do
        kill_tree "$child"
    done
    kill "$pid" 2>/dev/null || true
    for _ in {1..30}; do
        kill -0 "$pid" 2>/dev/null || return 0
        sleep 0.1
    done
    kill -KILL "$pid" 2>/dev/null || true
}

stop_all() {
    for name in client server; do
        if [[ -f "$root/$name.pid" ]]; then
            kill_tree "$(cat "$root/$name.pid")"
            rm -f "$root/$name.pid"
        fi
    done
}

case "${1:-}" in
    stop)
        stop_all
        echo "stopped ($root)"
        exit 0
        ;;
    start) ;;
    *)
        echo "usage: launch-live.sh start [bytes] | stop" >&2
        exit 2
        ;;
esac

stop_all
rm -rf "$root"
mkdir -p "$root/config/clay" "$root/data" "$root/home/.clay" "$root/home/.config" \
    "$root/workspace" "$root/tmp"
chmod 700 "$root" "$root/config" "$root/config/clay" "$root/data" "$root/home" \
    "$root/home/.config" "$root/home/.clay" "$root/workspace" "$root/tmp"

if [[ -n "${CLAY_LIVE_INIT:-}" ]]; then
    # Alternate fixture script (e.g. the package-op budget probe).
    cp "$CLAY_LIVE_INIT" "$root/home/.clay/init.js"
else
    cp "$repo/tests/fixtures/configuration/ui-review-completion/init.js" "$root/home/.clay/init.js"
fi

python3 - "$root/workspace/review.rs" "$bytes" <<'PY'
import sys

path, target = sys.argv[1], int(sys.argv[2])
line = "pub fn helper_{index:06}() -> usize {{ {index} }}\n"
with open(path, "w", encoding="utf-8") as handle:
    written = 0
    index = 0
    while written < target:
        chunk = "".join(line.format(index=index + offset) for offset in range(1000))
        handle.write(chunk)
        written += len(chunk)
        index += 1000
    handle.write("\npub fn hello")
print(f"review.rs bytes={written + len(chr(10) + 'pub fn hello')}")
PY

python3 - "$root/config/clay/layout.json" "$root/workspace" <<'PY'
import json
import sys

layout_path, workspace = sys.argv[1:]
with open(layout_path, "w", encoding="utf-8") as output:
    json.dump(
        {
            "version": 2,
            "activeTab": 0,
            "tabs": [
                {
                    "workspaceRoot": workspace,
                    "activePane": 1,
                    "splitTree": {"leaf": {"paneId": 1}},
                    "slots": [],
                    "panes": {"1": "review.rs"},
                }
            ],
        },
        output,
    )
PY

(
    cd "$root/workspace"
    exec env HOME="$root/home" XDG_CONFIG_HOME="$root/config" XDG_DATA_HOME="$root/data" \
        TMPDIR="$root/tmp" "$repo/target/debug/clay" server "$root/clay.sock"
) > "$root/server.log" 2>&1 &
echo $! > "$root/server.pid"

for _ in $(seq 1 300); do
    [[ -S "$root/clay.sock" ]] && break
    kill -0 "$(cat "$root/server.pid")" 2>/dev/null || { echo "server exited" >&2; exit 1; }
    sleep 0.1
done
[[ -S "$root/clay.sock" ]] || { echo "no socket" >&2; exit 1; }

(
    cd "$repo"
    exec env HOME="$root/home" XDG_CONFIG_HOME="$root/config" XDG_DATA_HOME="$root/data" \
        TMPDIR="$root/tmp" "$repo/target/debug/clay" client "$root/clay.sock"
) > "$root/client.log" 2>&1 &
echo $! > "$root/client.pid"

echo "live root=$root server=$(cat "$root/server.pid") client=$(cat "$root/client.pid")"
