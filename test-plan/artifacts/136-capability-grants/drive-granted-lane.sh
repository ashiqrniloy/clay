#!/usr/bin/env bash
# Plan 136 task 6 live driver: type into the open `demo.lane` document so the
# fixture package's 500 ms parse handler holds the general lane, trigger
# completion while it runs, and capture the popup tree + window screenshots.
#
#   run-live.sh start granted-lane      # server + client + granted fixture
#   drive-granted-lane.sh               # focus, type, trigger, capture
#   run-live.sh stop                    # writes perf/clay-server-perf-summary.json
#
# Focus: this host runs the mango (dwl) compositor, so the Clay window is
# focused through the compositor's own IPC (`mmsg dispatch focusstack,next`) and
# verified with `mmsg get focusing-client` before any input is synthesized. The
# guard below refuses to type unless the compositor reports `clay-desktop` as the
# focused client, so keystrokes can never land in another application.
#
# Screenshots use the xdg-desktop-portal capture path (no GNOME Shell extension
# on mango); only the cropped Clay window is retained, the full desktop capture
# is deleted.
set -Eeuo pipefail

here=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
probe="$here/../127-lane-scheduling/probe.py"
shot="$here/../127-lane-scheduling/portal-shot.py"
root=${CLAY_LIVE_ROOT:-/tmp/clay-plan136-live}
out=${CLAY_LIVE_EVIDENCE:-$here/live-granted-lane}
mkdir -p "$out"

clients=$(mmsg get all-clients)
client_pid=$(cat "$root/client.pid")
if ! printf '%s' "$clients" | jq -e '.clients[] | select(.appid == "clay-desktop")' >/dev/null; then
    echo "no clay-desktop window is exposed by the compositor" >&2
    exit 1
fi
geometry=$(printf '%s' "$clients" | jq -r '.clients[] | select(.appid == "clay-desktop") | "\(.width)x\(.height)+\(.x)+\(.y)"')
exposed_client_pid=$(printf '%s' "$clients" | jq -r '.clients[] | select(.appid == "clay-desktop") | .pid')
[[ "$exposed_client_pid" == "$client_pid" ]] \
    || echo "warning: clay-desktop pid=$exposed_client_pid harness client pid=$client_pid" >&2

# Focus the Clay window: select its tag (which re-establishes a focused client
# when a layer surface held focus), then cycle the stack until Clay owns it.
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

python3 "$shot" "$out/window-before.png" "$geometry" && rm -f /tmp/plan126-full.png
python3 "$probe" focus >/dev/null
python3 "$probe" editor | tail -1 > "$out/editor-before.txt"

# Edit → schedules the fixture package's parse handler (500 ms general-lane hold).
wtype " lane"
sleep 0.15
# The fixture's manifest-declared `.` trigger fires the completion request while
# that handler still holds the general lane.
wtype "."
sleep 0.4
python3 "$shot" "$out/window-popup.png" "$geometry" && rm -f /tmp/plan126-full.png

python3 "$probe" completion 20 > "$out/tree-popup.txt" 2>&1 || true
python3 "$shot" "$out/window-after.png" "$geometry" && rm -f /tmp/plan126-full.png
python3 "$probe" editor | tail -1 > "$out/editor-after.txt"
echo "captured $out"
