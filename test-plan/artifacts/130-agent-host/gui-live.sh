#!/usr/bin/env bash
# Plan 130 agent-host decomposition — isolated live GUI launch for the
# agent-surface manual steps (modules 16/17) on a real Linux build.
#
#   gui-live.sh start   # server + desktop client, isolated root, canonical example config
#   gui-live.sh stop
#
# Isolation mirrors scripts/capture-ui-review.sh and the plan 124/126/127/129
# harnesses: mode-700 root for HOME/XDG/TMPDIR, private server socket, scratch
# workspace, canonical `examples/config` as `~/.clay`. No input synthesis: every
# interactive step is driven through AT-SPI actions (probe.py) or recorded
# UNRESOLVED with this host's ceiling.
set -Eeuo pipefail
umask 077

here=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
repo=$(cd "$here/../../.." && pwd)
root=${CLAY_LIVE_ROOT:-/tmp/clay-plan130-gui}

kill_tree() {
    local pid=$1 child
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
        echo "usage: gui-live.sh start | stop" >&2
        exit 2
        ;;
esac

stop_all
rm -rf "$root"
mkdir -p "$root/config/clay" "$root/data" "$root/home/.config" "$root/home/.clay" \
    "$root/tmp" "$root/workspace" "$root/shots"
chmod 700 "$root" "$root/config" "$root/config/clay" "$root/data" "$root/home" \
    "$root/home/.config" "$root/home/.clay" "$root/tmp" "$root/workspace"

cp -r "$repo/examples/config/." "$root/home/.clay/"
cat > "$root/workspace/notes.md" <<'EOF'
# Plan 130 scratch workspace

Agent-surface manual pass; nothing here is written by the probe.
EOF

python3 - "$root/config/clay/layout.json" "$root/workspace" <<'PY'
import json, sys

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
                    "panes": {"1": "notes.md"},
                }
            ],
        },
        output,
    )
PY

(
    cd "$root/workspace"
    exec env HOME="$root/home" XDG_CONFIG_HOME="$root/config" XDG_DATA_HOME="$root/data" \
        TMPDIR="$root/tmp" CLAY_AGENT_MOCK=1 \
        "$repo/target/debug/clay" server "$root/clay.sock"
) > "$root/server.log" 2>&1 &
echo $! > "$root/server.pid"

for _ in $(seq 1 300); do
    [[ -S "$root/clay.sock" ]] && break
    kill -0 "$(cat "$root/server.pid")" 2>/dev/null || {
        echo "server exited" >&2
        tail -20 "$root/server.log" >&2
        exit 1
    }
    sleep 0.1
done
[[ -S "$root/clay.sock" ]] || { echo "no socket" >&2; exit 1; }

(
    cd "$repo"
    exec env HOME="$root/home" XDG_CONFIG_HOME="$root/config" XDG_DATA_HOME="$root/data" \
        TMPDIR="$root/tmp" CLAY_AGENT_MOCK=1 \
        "$repo/target/debug/clay" client "$root/clay.sock"
) > "$root/client.log" 2>&1 &
echo $! > "$root/client.pid"

for _ in $(seq 1 300); do
    if python3 "$here/probe.py" dump >/dev/null 2>&1; then
        break
    fi
    sleep 0.2
done

# The isolated client can open on a different GNOME workspace/monitor than the
# portal screenshot surface, which would make a retained crop show whatever is
# on the visible workspace instead of Clay. Activate the window first (the
# accessibility bus knows where it is; the compositor then brings it forward).
window_id=$(gdbus call --session --dest dev.avifenesh.ComputerUseLinux.WindowControl \
    --object-path /dev/avifenesh/ComputerUseLinux/WindowControl \
    --method dev.avifenesh.ComputerUseLinux.WindowControl.ListWindows 2>/dev/null |
    python3 -c 'import json,sys
text = sys.stdin.read()
windows = json.loads(text[text.find("["):text.rfind("]") + 1])
print(next((str(w["window_id"]) for w in windows if str(w.get("wm_class", "")) == "clay-desktop"), ""))' 2>/dev/null || true)
if [[ -n "${window_id:-}" ]]; then
    gdbus call --session --dest dev.avifenesh.ComputerUseLinux.WindowControl \
        --object-path /dev/avifenesh/ComputerUseLinux/WindowControl \
        --method dev.avifenesh.ComputerUseLinux.WindowControl.ActivateWindow "$window_id" >/dev/null 2>&1 || true
    sleep 1
fi

echo "live root=$root server=$(cat "$root/server.pid") client=$(cat "$root/client.pid") window=${window_id:-unknown}"
