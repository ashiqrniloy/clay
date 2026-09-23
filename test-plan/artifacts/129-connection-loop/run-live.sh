#!/usr/bin/env bash
# Plan 129 connection-loop-decomposition manual steps: isolated live launches.
#
#   run-live.sh start editing [bytes]  # bundled @clay/rust: typing, indent, undo, completion
#   run-live.sh start cursors          # repeated-word document for the Ctrl+D family
#   run-live.sh start cursorstyle      # typography + caret-style fixture (ligature sample)
#   run-live.sh start commands         # fresh profile (no init.js): Control Center defaults
#   run-live.sh start chords           # Ctrl+Q Ctrl+W sequence chord fixture
#   run-live.sh stop
#
# Isolation mirrors scripts/capture-ui-review.sh and the plan 126/127 harnesses:
# a private mode-700 root for HOME/XDG/TMPDIR, a private server socket, a
# fixture workspace, and teardown that kills the whole client/server tree.
# Keyboard input is driven through the computer-use-linux MCP (portal keyboard
# session) after probe.py focus; verify every step against the live AT-SPI tree.
set -Eeuo pipefail
umask 077

here=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
repo=$(cd "$here/../../.." && pwd)
root=${CLAY_LIVE_ROOT:-/tmp/clay-plan129-live}
mode=${2:-editing}
size=${3:-65536}

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
        echo "usage: run-live.sh start editing|cursors|cursorstyle|commands|chords [bytes] | stop" >&2
        exit 2
        ;;
esac

stop_all
rm -rf "$root"
mkdir -p "$root/config/clay" "$root/data" "$root/home/.clay" "$root/home/.config" \
    "$root/workspace" "$root/tmp"
chmod 700 "$root" "$root/config" "$root/config/clay" "$root/data" "$root/home" \
    "$root/home/.config" "$root/home/.clay" "$root/workspace" "$root/tmp"

document=demo.txt
case "$mode" in
    editing)
        cp "$repo/tests/fixtures/configuration/ui-review-completion/init.js" "$root/home/.clay/init.js"
        python3 - "$root/workspace/review.rs" "$size" <<'PY'
import sys

path, target = sys.argv[1], int(sys.argv[2])
line = "pub fn helper_{index:06}() -> usize {{ {index} }}\n"
with open(path, "w", encoding="utf-8") as handle:
    written, index = 0, 0
    while written < target:
        chunk = "".join(line.format(index=index + offset) for offset in range(1000))
        handle.write(chunk)
        written += len(chunk)
        index += 1000
    handle.write("\nfn indented_sample() {\n    let x = 1;\n    // caret-here comment line\n}\n")
PY
        document=review.rs
        ;;
    cursors)
        cp "$repo/tests/fixtures/configuration/ui-review-completion/init.js" "$root/home/.clay/init.js"
        cat > "$root/workspace/cursors.rs" <<'EOF'
fn camelCaseCounter(camelCaseCounter: usize) -> usize {
    let camelCaseCounter = camelCaseCounter + 1;
    let camelCaseCounter = camelCaseCounter + 1;
    let camelCaseCounter = camelCaseCounter + 1;
    camelCaseCounter
}

fn column_box() {
    alpha beta gamma
    alpha beta gamma
    alpha beta gamma
}
EOF
        document=cursors.rs
        ;;
    cursorstyle)
        cp "$here/init-cursor-style.js" "$root/home/.clay/init.js"
        cat > "$root/workspace/caret.rs" <<'EOF'
// Ligature sample: =>  !=  ==  ->  ::  ||  >=
// Zero sample: 0O 0O 0O
fn main() {
    let sample = 0;
}
EOF
        document=caret.rs
        ;;
    commands)
        cat > "$root/workspace/commands.rs" <<'EOF'
fn commands_fixture() {
    let marker = 1;
}
EOF
        document=commands.rs
        ;;
    chords)
        cp "$here/init-chords.js" "$root/home/.clay/init.js"
        cat > "$root/workspace/commands.rs" <<'EOF'
fn commands_fixture() {
    let marker = 1;
}
EOF
        document=commands.rs
        ;;
    *)
        echo "unknown mode $mode" >&2
        exit 2
        ;;
esac
if [[ -n "${CLAY_LIVE_INIT:-}" ]]; then
    cp "$CLAY_LIVE_INIT" "$root/home/.clay/init.js"
fi

python3 - "$root/config/clay/layout.json" "$root/workspace" "$document" <<'PY'
import json, sys

layout_path, workspace, document = sys.argv[1:]
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
                    "panes": {"1": document},
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
        TMPDIR="$root/tmp" "$repo/target/debug/clay" client "$root/clay.sock"
) > "$root/client.log" 2>&1 &
echo $! > "$root/client.pid"

echo "live root=$root mode=$mode document=$document server=$(cat "$root/server.pid") client=$(cat "$root/client.pid")"