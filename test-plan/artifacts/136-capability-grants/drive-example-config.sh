#!/usr/bin/env bash
# Plan 136 task 12 live driver: launch the canonical example configuration and
# exercise the shell.
#
#   run-live.sh start example-config     # server + client on a copy of examples/config
#   drive-example-config.sh              # focus, menu, command, pane, file browser
#   run-live.sh stop                     # writes perf/clay-server-perf-summary.json
#
# Input paths (all real user affordances, none invented for the test):
#   * the header "Control Center" button (AT-SPI press; the same button shows
#     the `Ctrl+X Ctrl+O` hint) opens the command palette;
#   * typing filters the palette rows;
#   * the palette row's own chord `Ctrl+Shift+\` runs `shell.clientAddEqualPane`
#     (the palette names it), which opens a pane;
#   * `Ctrl+B` is the example config's active `workspace.toggleFileBrowser`
#     binding, so it proves the configuration's keymap committed.
#
# Focus/screenshot rules mirror drive-granted-lane.sh: this host runs the mango
# (dwl) compositor, so the Clay window is focused through the compositor IPC and
# verified before any input is synthesized (keystrokes can never land in another
# application); screenshots use the xdg-desktop-portal capture path and only the
# cropped Clay window is retained.
set -Eeuo pipefail

here=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
probe="$here/../127-lane-scheduling/probe.py"
shot="$here/../127-lane-scheduling/portal-shot.py"
root=${CLAY_LIVE_ROOT:-/tmp/clay-plan136-example}
out=${CLAY_LIVE_EVIDENCE:-$here/live-example-config}
mkdir -p "$out"

clients=$(mmsg get all-clients)
if ! printf '%s' "$clients" | jq -e '.clients[] | select(.appid == "clay-desktop")' >/dev/null; then
    echo "no clay-desktop window is exposed by the compositor" >&2
    exit 1
fi
geometry=$(printf '%s' "$clients" | jq -r '.clients[] | select(.appid == "clay-desktop") | "\(.width)x\(.height)+\(.x)+\(.y)"')
clay_tag=$(printf '%s' "$clients" | jq -r '.clients[] | select(.appid == "clay-desktop") | .tags[0]')
mmsg dispatch "view,$clay_tag" >/dev/null || true
sleep 0.3
for _ in $(seq 1 8); do
    [[ $(mmsg get focusing-client | jq -r '.appid // ""') == clay-desktop ]] && break
    mmsg dispatch focusstack,next >/dev/null
    sleep 0.3
done
mmsg get focusing-client > "$out/focus.json"
if ! grep -q '"appid":"clay-desktop"' "$out/focus.json"; then
    echo "refusing to synthesize input: clay-desktop does not own keyboard focus" >&2
    exit 1
fi
echo "compositor focus: $(cat "$out/focus.json")" > "$out/focus.txt"
echo "bounds=$geometry" >> "$out/focus.txt"
cat "$out/focus.txt"

shot_window() {
    python3 "$shot" "$out/$1" "$geometry" && rm -f /tmp/plan126-full.png
}

# The client is up when the editor node exists (the AT-SPI readiness path the
# plan 126 harness uses; `wait-ready` never reports ready on this host).
for _ in $(seq 1 100); do
    python3 "$probe" editor 2>/dev/null | grep -q "Document editor" && break
    sleep 0.2
done
python3 "$probe" focus >/dev/null
shot_window window-initial.png
python3 "$probe" editor | tail -1 > "$out/editor-initial.txt"
python3 "$probe" dump > "$out/tree-initial.txt"

# 1. Open the menu: the header Control Center button (AT-SPI press).
python3 "$probe" click "Control Center" > "$out/click-control-center.txt" 2>&1
sleep 1.0
shot_window window-menu.png
python3 "$probe" dump > "$out/tree-menu.txt"
grep -E "\|dialog\||\|list box\|" "$out/tree-menu.txt" > "$out/tree-menu-hits.txt" || true

# 2. Filter the palette (typing is the documented query path).
wtype "equal"
sleep 0.8
shot_window window-menu-filtered.png
python3 "$probe" dump > "$out/tree-menu-filtered.txt"
grep "|list item|" "$out/tree-menu-filtered.txt" | head -3 > "$out/tree-menu-filtered-hits.txt" || true
wtype -k Escape
sleep 0.8

# 3. Run a command: the row's own chord (`Ctrl+Shift+\` → Add Equal Pane).
wtype -M ctrl -M shift -s 40 -k backslash -s 40 -m shift -m ctrl
sleep 1.2
shot_window window-pane.png
python3 "$probe" dump > "$out/tree-after-pane.txt"

# 4. The example config's own binding: `Ctrl+B` → workspace.toggleFileBrowser.
wtype -M ctrl -k b -m ctrl
sleep 1.0
shot_window window-file-browser.png
python3 "$probe" dump > "$out/tree-after-files.txt"
python3 "$probe" editor | tail -1 > "$out/editor-after.txt"

{
    echo "editors: initial=$(grep -c 'Document editor' "$out/tree-initial.txt") after=$(grep -c 'Document editor' "$out/tree-after-files.txt")"
    echo "palette open (Commands nodes): $(grep -c 'Commands' "$out/tree-menu.txt")"
    echo "palette rows after filter: $(grep -c '|list item|' "$out/tree-menu-filtered.txt")"
    echo "file-browser nodes: initial=$(grep -c 'Filter files' "$out/tree-initial.txt") after=$(grep -c 'Filter files' "$out/tree-after-files.txt")"
    echo "editor chars: initial=$(sed -n 's/.*chars=\([0-9]*\).*/\1/p' "$out/editor-initial.txt") after=$(sed -n 's/.*chars=\([0-9]*\).*/\1/p' "$out/editor-after.txt")"
} > "$out/interaction-summary.txt"
cat "$out/interaction-summary.txt"

grep -icE "configuration failed|configuration\.module_failed|load_failed|MissingCapabilityGrant|panicked" \
    "$root/server.log" > "$out/config-failure-count.txt" || true
echo "config-failure lines: $(cat "$out/config-failure-count.txt")"
echo "captured $out"
