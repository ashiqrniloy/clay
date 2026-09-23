#!/usr/bin/env bash
# Focus the isolated Clay window (Hyprland 0.56 Lua dispatcher form) and the
# editor node, then verify the compositor really holds Clay before any input.
set -Eeuo pipefail
hyprctl dispatch 'hl.dsp.focus({window="class:clay-desktop"})' >/dev/null
sleep 0.4
python3 "$(dirname "$0")/probe.py" focus >/dev/null
active=$(hyprctl activewindow -j | python3 -c 'import json,sys; print(json.load(sys.stdin)["class"])')
if [[ "$active" != "clay-desktop" ]]; then
    echo "refusing input: active window is $active" >&2
    exit 3
fi
echo "clay-desktop focused"
