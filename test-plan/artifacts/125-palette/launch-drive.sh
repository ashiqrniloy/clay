#!/usr/bin/env bash
# Plan 125 launch test — canonical examples/config tree, isolated root,
# one clean launch, five states, AT-SPI-driven (no host input synthesis).
#
#   bash launch-drive.sh [root]
#
# Evidence lands next to this script: screenshots/NN-*.png (+ .ax.txt).
set -Eeuo pipefail
repo=/home/arn/Projects/clay
out="$repo/test-plan/artifacts/125-palette/screenshots"
root=${1:-/tmp/plan125-launchtest}

# --- isolated root: canonical config, unmodified ---------------------------
for f in "$root"/*.pid; do [[ -e "$f" ]] && kill "$(cat "$f")" 2>/dev/null || true; done
pkill -f "$repo/target/debug/clay server" 2>/dev/null || true
pkill -x clay-desktop 2>/dev/null || true
sleep 1
rm -rf "$root"
mkdir -p "$root/home/.clay" "$root/config" "$root/data" "$root/tmp" "$root/workspace"
chmod 700 "$root" "$root/home" "$root/home/.clay" "$root/config" "$root/data" "$root/tmp" "$root/workspace"
cp -r "$repo/examples/config/." "$root/home/.clay/"
grep -q 'setTheme("@clay/theme-gruvbox-material-dark")' "$root/home/.clay/init.js" ||
    { echo "canonical config must keep its shipped setTheme"; exit 1; }
printf '# Plan 125 launch-test workspace\n\ncanonical example config; isolated root.\n' \
    > "$root/workspace/notes.md"
(
    cd "$root/workspace"
    exec env HOME="$root/home" XDG_CONFIG_HOME="$root/config" XDG_DATA_HOME="$root/data" TMPDIR="$root/tmp" \
        "$repo/target/debug/clay" server "$root/review.sock"
) > "$root/server.log" 2>&1 &
echo $! > "$root/server.pid"
for _ in $(seq 1 200); do [[ -S "$root/review.sock" ]] && break; sleep 0.1; done
[[ -S "$root/review.sock" ]] || { echo "server socket never appeared"; exit 1; }

# --- window visibility time -------------------------------------------------
start_ms=$(date +%s%3N)
env HOME="$root/home" XDG_CONFIG_HOME="$root/config" XDG_DATA_HOME="$root/data" TMPDIR="$root/tmp" \
    "$repo/target/debug/clay" client "$root/review.sock" > "$root/client.log" 2>&1 &
echo $! > "$root/client.pid"
for _ in $(seq 1 1200); do
    python3 /tmp/clay-window-check.py 2>/dev/null && break
    sleep 0.05
done
end_ms=$(date +%s%3N)
python3 /tmp/clay-window-check.py --json > "$root/window.json"
echo "window_visible_after_ms=$((end_ms - start_ms))"
bash /tmp/prepare-window.sh 1500 940 > /dev/null 2>&1
sleep 1
python3 /tmp/clay-window-check.py --json

# --- five states -----------------------------------------------------------
mkdir -p "$out"
bash /tmp/capture-state.sh "$out" 01-rest 2>&1 | tail -1
python3 /tmp/atspi_action.py click button "Control Center" > /dev/null 2>&1
sleep 2
bash /tmp/capture-state.sh "$out" 02-palette-titlebar 2>&1 | tail -1
python3 /tmp/atspi_action.py click button "palette" > /dev/null 2>&1
sleep 2
bash /tmp/capture-state.sh "$out" 03-palette-via-hint 2>&1 | tail -1
python3 /tmp/atspi_action.py click button "hide lane" > /dev/null 2>&1
sleep 2
bash /tmp/capture-state.sh "$out" 04-lane-hidden-palette-gone 2>&1 | tail -1
python3 /tmp/atspi_action.py click button "lane" > /dev/null 2>&1
sleep 2
bash /tmp/capture-state.sh "$out" 05-lane-restored-palette 2>&1 | tail -1
echo "states captured into $out"
