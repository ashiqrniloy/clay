#!/usr/bin/env bash
# Plan 132 fanout/DTO-dedup manual steps: isolated live launches.
#
#   run-live.sh start caret           # module 07: typography pin + hollow block caret (ligature sample)
#   run-live.sh start caret-invalid   # module 07 negative: deny-by-default caret shape
#   run-live.sh start wrap            # module 07 T22/T23: column wrap override + long line
#   run-live.sh start editing         # module 04/10: editor-command lane (init.js + reload)
#   run-live.sh start panes           # module 13 S14/S17: cursor pane-focus policy, two panes
#   run-live.sh start panes-click     # module 13 S13 A-side: default click policy, two panes
#   run-live.sh stop
#
# Any mode's init.js can be replaced with CLAY_LIVE_INIT=<path> (used for the
# wrap `none` and the invalid-value A/B legs).
#
# Isolation mirrors scripts/capture-ui-review.sh and the plan 126/127/129
# harnesses: private mode-700 root for HOME/XDG/TMPDIR, private server socket,
# fixture workspace, tree-kill teardown. Typing is driven with wtype on this
# Hyprland host (plan 129's portal keyboard ceiling does not apply here);
# pointer moves for the pane-focus policy with `hyprctl dispatch movecursor`.
set -Eeuo pipefail
umask 077

here=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
repo=$(cd "$here/../../../.." && pwd)
root=${CLAY_LIVE_ROOT:-/tmp/clay-plan132-live}
mode=${2:-caret}

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
        echo "usage: run-live.sh start caret|caret-invalid|wrap|editing|panes|panes-click | stop" >&2
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
tree='{"leaf": {"paneId": 1}}'
panes_json=""
long_line=$(python3 -c 'print("let tail = \"" + "x" * 180 + "\";")')

case "$mode" in
    caret)
        cp "$here/init-caret.js" "$root/home/.clay/init.js"
        cat > "$root/workspace/caret.rs" <<'EOF'
// Ligature sample: =>  !=  ==  ->  ::  ||  >=
// Zero sample: 0O 0O 0O
fn main() {
    let sample = 0;
}
EOF
        document=caret.rs
        ;;
    caret-invalid)
        cp "$here/init-invalid-caret.js" "$root/home/.clay/init.js"
        cp "$here/init-caret.js" "$root/home/.clay/init-valid.js"
        cat > "$root/workspace/caret.rs" <<'EOF'
fn main() {
    let sample = 0;
}
EOF
        document=caret.rs
        ;;
    wrap)
        cp "$here/init-wrap-column.js" "$root/home/.clay/init.js"
        {
            echo "// Long-line wrap fixture (module 07 T22/T23/T24)."
            echo "fn main() {"
            echo "    $long_line"
            echo "}"
        } > "$root/workspace/wrap.rs"
        document=wrap.rs
        ;;
    editing)
        cp "$here/init-editor-command.js" "$root/home/.clay/init.js"
        cat > "$root/workspace/commands.rs" <<'EOF'
fn commands_fixture() {
    let alpha = 1;
    let beta = 2;
}
EOF
        document=commands.rs
        ;;
    review)
        # Empty pane: the fixture's SDUI tree lands on the empty-tab surface.
        panes_json='{}'
        document=""
        ;;
    panes|panes-click)
        if [[ "$mode" == "panes" ]]; then
            cp "$here/init-pane-focus.js" "$root/home/.clay/init.js"
        fi
        printf 'fn one() {\n    let marker = 1;\n}\n' > "$root/workspace/one.rs"
        printf 'fn two() {\n    let marker = 2;\n}\n' > "$root/workspace/two.rs"
        document=one.rs
        tree='{"split": {"orientation": "horizontal", "ratio": 0.5, "first": {"leaf": {"paneId": 1}}, "second": {"leaf": {"paneId": 2}}}}'
        panes_json='{"1": "one.rs", "2": "two.rs"}'
        ;;
    *)
        echo "unknown mode $mode" >&2
        exit 2
        ;;
esac
if [[ -z "$panes_json" ]]; then
    panes_json=$(python3 -c 'import json,sys; print(json.dumps({"1": sys.argv[1]}))' "$document")
fi
if [[ -z "$document" ]]; then
    document=welcome
fi
if [[ -n "${CLAY_LIVE_INIT:-}" ]]; then
    cp "$CLAY_LIVE_INIT" "$root/home/.clay/init.js"
fi

python3 - "$root/config/clay/layout.json" "$root/workspace" "$tree" "$panes_json" <<'PY'
import json, sys

layout_path, workspace, tree, panes = sys.argv[1:]
with open(layout_path, "w", encoding="utf-8") as output:
    json.dump(
        {
            "version": 2,
            "activeTab": 0,
            "tabs": [
                {
                    "workspaceRoot": workspace,
                    "activePane": 1,
                    "splitTree": json.loads(tree),
                    "slots": [],
                    "panes": json.loads(panes),
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
