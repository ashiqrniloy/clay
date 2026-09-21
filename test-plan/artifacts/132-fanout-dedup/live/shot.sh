#!/usr/bin/env bash
# Capture the isolated Clay window at its CURRENT geometry. Refuses when any
# other window holds compositor focus (a stray dialog once landed in a capture).
set -Eeuo pipefail
out=$1
hyprctl dispatch 'hl.dsp.focus({window="class:clay-desktop"})' >/dev/null
sleep 0.4
active=$(hyprctl activewindow -j | python3 -c 'import json,sys; c=json.load(sys.stdin); print(c["class"])')
if [[ "$active" != "clay-desktop" ]]; then
    echo "refusing capture: active window is $active" >&2
    exit 3
fi
geom=$(hyprctl activewindow -j | python3 -c 'import json,sys; c=json.load(sys.stdin); x,y=c["at"]; w,h=c["size"]; print(f"{x},{y} {w}x{h}")')
grim -g "$geom" "$out"
echo "$out ($geom)"
