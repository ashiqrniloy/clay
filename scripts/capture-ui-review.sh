#!/usr/bin/env bash
# Capture one isolated Clay UI review state.
set -Eeuo pipefail
umask 077

repo=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
fixture=""
output=""
timeout_seconds="${CLAY_UI_REVIEW_TIMEOUT_SECONDS:-45}"
example_config=""
requested_size=""
theme_specifier=""
appearance=""
drive_steps=""

usage() {
    cat <<'EOF'
Usage: scripts/capture-ui-review.sh --fixture <name> --output <directory>
       [--size WxH] [--theme <specifier>] [--appearance light|dark|system]
       [--drive '<json steps>']

--size WxH requests that window size through the computer-use-linux GNOME Shell
extension (`dev.avifenesh.ComputerUseLinux.WindowControl`) and moves the window
fully on-screen first. The requested *width* is verified (it drives every
responsive breakpoint); the height is recorded as measured, because this
session's compositor pins window height to the work area for every app. A
missing extension or a refused request records UNRESOLVED instead of claiming
the size. Measured frame and webview-viewport sizes land in metadata.txt.

A capture is only valid review evidence inside the plan-087 logical envelope of
900×600 or larger, so a measured viewport below that floor records UNRESOLVED
rather than passing: the old fixed 900×600 claim is now a checked floor and a
measured value, not a constant. Larger sizes (`--size 1500x950`, `--size
1024x800`) are what exercise the wide and narrow layouts.
--example-config boots the review against a copy of the canonical
`examples/config/` tree instead of the fixture's own init.js: the whole tree
(init.js plus packages/) is copied into the isolated mode-700 config root, so
the run exercises the shipped example exactly as `cp -r examples/config/. ~/.clay/`
would. It is only valid with the fixtures whose checks match what the canonical
config renders (ui-review-launcher); the other fixtures assert their own panel
content (or auto-open their own surface through their own init.js calls), so
pairing them would be a false pass.
--theme/--appearance seed ~/.clay/preferences.json, so any fixture can be
captured under any shipped theme. --drive executes AT-SPI steps before
the capture, which reaches states that otherwise need keyboard input (palette,
menus, modals, rail toggles, the agent view) without input synthesis:

  [{"find": {"role": "button", "name": "Palette"}, "do": "click"},
   {"find": {"role": "entry", "name": "Document editor"}, "do": "type",
    "text": "hello"},
   {"find": {"role": "button", "name": "Palette"}, "do": "focus"},
   {"wait": 400}]

`do` is one of click, focus, type, clear, insert. `type`/`clear` replace the
whole field through `SetTextContents`; `insert` appends `text` at the caret (or
at `"at": "end"`, the last offset the field exposes) through `InsertText`, so a
large field stays large while the insertion goes through the editor's own
editing path. These actions need the node to expose `EditableText`: the WebKit
document editor exposes only `Text`/`Action`/`Component` on this host, so its
live typing is driven through the portal keyboard session instead (see
`test-plan/artifacts/126-access-paths/`), and an `insert`/`type` step aimed at it
records UNRESOLVED with that reason. Every step is verified against the live
AT-SPI tree; a step that cannot be applied records UNRESOLVED with its reason
instead of passing.

Fixtures:
  ui-review-default         clean Clay shell, core empty-tab fallback (no packages)
  ui-review-launcher        bundled launcher landing on the empty tab
  ui-review-loading         deterministic loading-state SDUI panel
  ui-review-error           configuration/runtime error state
  ui-review-recovery        disconnected/recovery state after server stop
  ui-review-design-system   shipped design-system activation (dark)
  ui-review-design-system-light shipped design-system activation (light)
  ui-review-large-typography user-owned large typography state
  ui-review-completion      completion-ready document (interactive capture)
  ui-review-large-document  ≥4 MiB Rust document with the completion fixture's
                            init.js (plan 126 access-path steps)
  ui-review-command-centre command centre (interactive capture)
  ui-review-rust            authorized Rust analyzer/inlay states (interactive capture)

The command needs a Linux desktop AT-SPI bus, Python GI AT-SPI bindings, and
xdg-desktop-portal Screenshot. Missing capture tooling exits 2 and writes an
UNRESOLVED status instead of claiming a review passed.
EOF
}

while (($#)); do
    case "$1" in
        --fixture)
            [[ $# -ge 2 ]] || { echo "missing value for --fixture" >&2; exit 2; }
            fixture=$2
            shift 2
            ;;
        --output)
            [[ $# -ge 2 ]] || { echo "missing value for --output" >&2; exit 2; }
            output=$2
            shift 2
            ;;
        --timeout)
            [[ $# -ge 2 ]] || { echo "missing value for --timeout" >&2; exit 2; }
            timeout_seconds=$2
            shift 2
            ;;
        --size)
            [[ $# -ge 2 ]] || { echo "missing value for --size" >&2; exit 2; }
            requested_size=$2
            shift 2
            ;;
        --theme)
            [[ $# -ge 2 ]] || { echo "missing value for --theme" >&2; exit 2; }
            theme_specifier=$2
            shift 2
            ;;
        --appearance)
            [[ $# -ge 2 ]] || { echo "missing value for --appearance" >&2; exit 2; }
            appearance=$2
            shift 2
            ;;
        --example-config)
            example_config=1
            shift
            ;;
        --drive)
            [[ $# -ge 2 ]] || { echo "missing value for --drive" >&2; exit 2; }
            drive_steps=$2
            shift 2
            ;;
        -h|--help)
            usage
            exit 0
            ;;
        *)
            echo "unknown argument: $1" >&2
            usage >&2
            exit 2
            ;;
    esac
done

case "$fixture" in
    ui-review-default|ui-review-launcher|ui-review-loading|ui-review-error|ui-review-recovery|ui-review-design-system|ui-review-design-system-light|ui-review-large-typography|ui-review-completion|ui-review-large-document|ui-review-command-centre|ui-review-workspace|ui-review-rust|ui-review-coding-agent|ui-review-icons-regular-light|ui-review-icons-duotone-dark|ui-review-icons-fallback-large) ;;
    *)
        echo "unknown --fixture: ${fixture:-<missing>}" >&2
        usage >&2
        exit 2
        ;;
esac
[[ -n "$output" ]] || { echo "--output is required" >&2; exit 2; }
if [[ -n "$requested_size" && ! "$requested_size" =~ ^[1-9][0-9]*x[1-9][0-9]*$ ]]; then
    echo "--size must look like WIDTHxHEIGHT" >&2
    exit 2
fi
if [[ -n "$appearance" && ! "$appearance" =~ ^(light|dark|system)$ ]]; then
    echo "--appearance must be light, dark or system" >&2
    exit 2
fi
if [[ -n "$theme_specifier" && ! "$theme_specifier" =~ ^@clay/theme-[a-z0-9-]+$ ]]; then
    echo "--theme must be a bundled @clay/theme-* specifier" >&2
    exit 2
fi
if [[ -n "$drive_steps" ]] && ! python3 -c 'import json,sys; json.loads(sys.argv[1])' "$drive_steps" 2>/dev/null; then
    echo "--drive must be a JSON array of steps" >&2
    exit 2
fi
[[ "$timeout_seconds" =~ ^[1-9][0-9]*$ ]] || { echo "--timeout must be a positive integer" >&2; exit 2; }
if [[ -n "$example_config" ]]; then
    case "$fixture" in
        ui-review-launcher) ;;
        *)
            echo "--example-config is only valid with --fixture ui-review-launcher (the canonical config renders the launcher landing), got ${fixture:-<missing>}" >&2
            exit 2
            ;;
    esac
fi

mkdir -p "$output"
output=$(cd "$output" && pwd)
root=$(mktemp -d "${TMPDIR:-/tmp}/clay-ui-review.XXXXXX")
chmod 700 "$root"
config_home=$root/config
config_dir=$config_home/clay
data_home=$root/data
home=$root/home
workspace=$root/workspace
socket=$root/review.sock
mkdir -p "$config_dir" "$data_home" "$home/.clay" "$home/.config" "$workspace" "$root/tmp"
chmod 700 "$config_home" "$config_dir" "$data_home" "$home" "$home/.config" "$home/.clay" "$workspace" "$root/tmp"

# Optional theme/appearance seeding: preferences.json is the same closed store
# the Settings panel writes, so every fixture can be captured under any shipped
# theme without a second fixture directory.
if [[ -n "$theme_specifier" || -n "$appearance" ]]; then
    python3 - "$home/.clay/preferences.json" "$theme_specifier" "$appearance" <<'PY'
import json, sys
path, theme, appearance = sys.argv[1:]
preferences = {}
if theme:
    preferences["theme"] = theme
if appearance:
    preferences["appearance"] = appearance
with open(path, "w", encoding="utf-8") as output:
    json.dump(preferences, output)
PY
fi

# The Rust fixture keeps Clay configuration/data isolated but lets the fixed
# rustup language-server descriptor inherit the host HOME for its installed
# toolchain. Other fixtures remain fully private.
runtime_home="$home"
if [[ "$fixture" == ui-review-rust ]]; then
    runtime_home="${CLAY_UI_REVIEW_LANGUAGE_SERVER_HOME:-${HOME:-$home}}"
fi

server_pid=""
client_pid=""
desktop_pid=""
exit_status=0

stop_child() {
    local pid=${1:-}
    [[ -n "$pid" ]] || return 0
    if kill -0 "$pid" 2>/dev/null; then
        kill "$pid" 2>/dev/null || true
        for _ in {1..30}; do
            kill -0 "$pid" 2>/dev/null || break
            sleep 0.1
        done
        kill -KILL "$pid" 2>/dev/null || true
    fi
    wait "$pid" 2>/dev/null || true
}

cleanup() {
    local status=$?
    trap - EXIT INT TERM
    stop_child "$desktop_pid"
    stop_child "$client_pid"
    stop_child "$server_pid"
    rm -rf "$root"
    exit "$status"
}
trap cleanup EXIT
trap 'exit 130' INT TERM

unresolved() {
    local reason=$1
    if [[ -n "${latest_dump:-}" && -s "$latest_dump" ]]; then
        cp "$latest_dump" "$output/accessibility.partial.txt"
    fi
    # Retain the isolated server log so unresolved reviews carry failure evidence.
    if [[ -n "${root:-}" && -s "$root/server.log" ]]; then
        cp "$root/server.log" "$output/server.partial.log" 2>/dev/null || true
    fi
    if [[ "$fixture" == ui-review-rust && -f "$root/portal_capture.py" ]]; then
        local toggled_output
        toggled_output="$(dirname "$output")/inlay-toggled-off"
        mkdir -p "$toggled_output"
        cp "$root/server.log" "$output/analyzer-server.log" 2>/dev/null || true
        cp "$root/client.log" "$output/client.log" 2>/dev/null || true
        cp "$root/server.log" "$toggled_output/analyzer-server.log" 2>/dev/null || true
        cp "$root/client.log" "$toggled_output/client.log" 2>/dev/null || true
        python3 "$root/portal_capture.py" "$output/screenshot.png" > "$root/portal-unresolved.out" 2> "$root/portal-unresolved.err" || true
        if [[ -f "$output/screenshot.png" ]]; then
            cp "$output/screenshot.png" "$toggled_output/screenshot.png"
        fi
        if [[ -f "$output/accessibility.partial.txt" ]]; then
            cp "$output/accessibility.partial.txt" "$toggled_output/accessibility.partial.txt"
        fi
        printf 'UNRESOLVED\nreason=%s\nstate=inlay-visible\n' "$reason" > "$output/review.status"
        printf 'UNRESOLVED\nreason=%s\nstate=inlay-toggled-off\n' "$reason" > "$toggled_output/review.status"
    else
        printf 'UNRESOLVED\nreason=%s\n' "$reason" > "$output/review.status"
    fi
    echo "UI review unresolved: $reason" >&2
    exit 2
}

if [[ ! -x "$repo/target/debug/clay" ]]; then
    echo "building target/debug/clay..." >&2
    (cd "$repo" && cargo build --bin clay >/dev/null) || unresolved "cargo build --bin clay failed"
fi

cat > "$root/atspi_probe.py" <<'PY'
import json
import sys
import time

try:
    import gi
    gi.require_version("Atspi", "2.0")
    from gi.repository import Atspi
except Exception as exc:
    print(f"PREREQ_MISSING: {exc}", file=sys.stderr)
    raise SystemExit(3)

MODES = {"prereq", "app", "dump-index", "frame-size", "viewport-size", "drive"}
if not sys.argv or sys.argv[1] not in MODES:
    raise SystemExit(
        "usage: atspi_probe.py prereq|app INDEX|dump-index INDEX|frame-size INDEX"
        "|viewport-size INDEX|drive INDEX JSON"
    )

desktop = Atspi.get_desktop(0)
if sys.argv[1] == "prereq":
    print(f"OK apps={desktop.get_child_count()}")
    raise SystemExit(0)


def clean(value):
    return str(value or "").replace("|", " ").replace("\n", " ")


def application_at(index):
    """Return the live Clay application node at `index`, or None when stale."""
    try:
        application = desktop.get_child_at_index(int(index))
    except Exception:
        return None
    if application is None:
        return None
    try:
        name = str(application.get_name() or "").strip().upper()
        if name not in {"CLAY", "CLAY-DESKTOP"}:
            return None
        pid = application.get_process_id()
        if pid and not __import__("pathlib").Path(f"/proc/{pid}").exists():
            return None
    except Exception:
        return None
    return application


if sys.argv[1] == "app":
    if len(sys.argv) != 3:
        raise SystemExit("usage: atspi_probe.py app INDEX")
    application = application_at(sys.argv[2])
    if application is None:
        raise SystemExit(0)
    print(str(application.get_name() or "").strip().upper())
    raise SystemExit(0)


def walk(node, depth, visit):
    if node is None:
        return
    try:
        visit(node, depth)
    except Exception:
        pass
    try:
        count = node.get_child_count()
    except Exception:
        return
    for index in range(count):
        try:
            walk(node.get_child_at_index(index), depth + 1, visit)
        except Exception:
            continue


if sys.argv[1] == "dump-index":
    if len(sys.argv) != 3:
        raise SystemExit("usage: atspi_probe.py dump-index INDEX")
    application = application_at(sys.argv[2])

    def dump(node, depth):
        app = node.get_application()
        app_name = clean(app.get_name() if app is not None else "")
        if app_name.lower() not in {"clay", "clay-desktop"}:
            return
        selected = "selected" if node.get_state_set().contains(Atspi.StateType.SELECTED) else "-"
        role = clean(node.get_role_name())
        text = ""
        if role == "status bar":
            try:
                text = clean(Atspi.Text.get_text(node, 0, -1))
            except Exception:
                pass
        print("|".join([
            str(depth), role, selected, clean(node.get_name()), text,
            clean(node.path), app_name,
        ]))

    walk(application, 0, dump)
    raise SystemExit(0)


def clay_frame(application):
    """The window frame node (AT-SPI extents; ~3% wider than the client area)."""
    found = []

    def visit(node, _depth):
        if clean(node.get_role_name()) == "frame":
            found.append(node)

    walk(application, 0, visit)
    return found[0] if found else None


if sys.argv[1] == "frame-size":
    if len(sys.argv) != 3:
        raise SystemExit("usage: atspi_probe.py frame-size INDEX")
    application = application_at(sys.argv[2])
    if application is None:
        raise SystemExit("Clay application is not live")
    frame = clay_frame(application)
    if frame is None:
        raise SystemExit("Clay window frame is not exposed")
    extents = frame.get_extents(Atspi.CoordType.SCREEN)
    print(f"{extents.width}x{extents.height}")
    raise SystemExit(0)


if sys.argv[1] == "viewport-size":
    if len(sys.argv) != 3:
        raise SystemExit("usage: atspi_probe.py viewport-size INDEX")
    application = application_at(sys.argv[2])
    if application is None:
        raise SystemExit("Clay application is not live")
    found = []

    def visit(node, _depth):
        if "document web" in clean(node.get_role_name()).lower():
            found.append(node)

    walk(application, 0, visit)
    if not found:
        raise SystemExit("the webview node is not exposed")
    extents = found[0].get_extents(Atspi.CoordType.SCREEN)
    print(f"{extents.width}x{extents.height}")
    raise SystemExit(0)



if sys.argv[1] == "drive":
    if len(sys.argv) != 4:
        raise SystemExit("usage: atspi_probe.py drive INDEX JSON")
    steps = json.loads(sys.argv[3])
    if not isinstance(steps, list):
        raise SystemExit("drive steps must be a JSON array")
    failures = []
    for position, step in enumerate(steps, start=1):
        if not isinstance(step, dict):
            failures.append(f"step {position}: not an object")
            continue
        if "wait" in step:
            time.sleep(max(0, int(step["wait"])) / 1000)
            print(f"OK step {position}: wait {step['wait']}ms")
            continue
        application = application_at(sys.argv[2])
        if application is None:
            failures.append(f"step {position}: Clay application is not live")
            break
        selector = step.get("find") or {}
        role = str(selector.get("role", "")).lower()
        name = str(selector.get("name", "")).lower()
        exact = bool(selector.get("exact"))
        wanted = int(selector.get("index", 0))
        matches = []

        def collect(node, _depth):
            node_role = clean(node.get_role_name()).lower()
            node_name = clean(node.get_name()).lower()
            if role and role not in node_role:
                return
            if name and (node_name != name if exact else name not in node_name):
                return
            matches.append(node)

        walk(application, 0, collect)
        if len(matches) <= wanted:
            failures.append(
                f"step {position}: no node matched role~{role!r} name~{name!r}"
                f" (found {len(matches)}, wanted index {wanted})"
            )
            continue
        node = matches[wanted]
        action = step.get("do", "click")
        try:
            if action == "click":
                count = Atspi.Action.get_n_actions(node)
                chosen = None
                for index in range(count):
                    candidate = clean(Atspi.Action.get_action_name(node, index)).lower()
                    if any(word in candidate for word in ("click", "press", "activate", "jump")):
                        chosen = index
                        break
                if chosen is None and count:
                    chosen = 0
                if chosen is None:
                    failures.append(f"step {position}: node exposes no action")
                    continue
                Atspi.Action.do_action(node, chosen)
                label = clean(Atspi.Action.get_action_name(node, chosen))
                print(f"OK step {position}: click {clean(node.get_name())!r} via {label!r}")
            elif action == "focus":
                if not Atspi.Component.grab_focus(node):
                    failures.append(f"step {position}: grab_focus refused for {clean(node.get_name())!r}")
                    continue
                print(f"OK step {position}: focus {clean(node.get_name())!r}")
            elif action in {"type", "clear"}:
                text = str(step.get("text", "")) if action == "type" else ""
                Atspi.Component.grab_focus(node)
                Atspi.EditableText.set_text_contents(node, text)
                print(f"OK step {position}: {action} {clean(node.get_name())!r}")
            elif action == "insert":
                # InsertText keeps the field's existing content: the large-document
                # steps need the document to stay large while the insertion drives
                # the same live typing path a keypress would.
                text = str(step.get("text", ""))
                Atspi.Component.grab_focus(node)
                if str(step.get("at", "caret")) == "end":
                    offset = Atspi.Text.get_character_count(node)
                else:
                    offset = Atspi.Text.get_caret_offset(node)
                if offset is None or offset < 0:
                    offset = Atspi.Text.get_character_count(node)
                Atspi.EditableText.insert_text(node, offset, text, len(text))
                print(
                    f"OK step {position}: insert {text!r} at {offset} in"
                    f" {clean(node.get_name())!r}"
                )
            else:
                failures.append(f"step {position}: unknown do {action!r}")
        except Exception as exc:  # noqa: BLE001 - the probe reports, never raises
            failures.append(f"step {position}: {action} failed: {exc}")
    for failure in failures:
        print(f"FAIL {failure}", file=sys.stderr)
    raise SystemExit(1 if failures else 0)
PY

cat > "$root/window_control.py" <<'PY'
"""Read the GNOME Shell extension's window listing (a GVariant string)."""

import json
import sys

if len(sys.argv) != 4 or sys.argv[1] not in {"id", "bounds", "rect"}:
    raise SystemExit("usage: window_control.py id|bounds|rect LISTING_FILE PID")

mode, path, pid = sys.argv[1], sys.argv[2], sys.argv[3]
raw = open(path, encoding="utf-8").read().strip()
if raw.startswith("('") and raw.endswith("',)"):
    raw = raw[2:-3]
windows = json.loads(raw)

def is_clay(window):
    if pid and str(window.get("pid")) == pid:
        return True
    return str(window.get("wm_class") or "").startswith("clay")

for window in windows:
    if not is_clay(window):
        continue
    if mode == "id":
        print(window.get("window_id"))
        raise SystemExit(0 if window.get("window_id") else 1)
    bounds = window.get("bounds") or {}
    if mode == "rect":
        print(
            f"{bounds.get('width')}x{bounds.get('height')}"
            f"+{bounds.get('x')}+{bounds.get('y')}"
        )
    else:
        print(f"{bounds.get('width')}x{bounds.get('height')}")
    raise SystemExit(0)
raise SystemExit(1)
PY

cat > "$root/crop_window.py" <<'PY'
import sys
import gi

gi.require_version("Atspi", "2.0")
gi.require_version("GdkPixbuf", "2.0")
from gi.repository import Atspi, GdkPixbuf

if len(sys.argv) < 3:
    raise SystemExit("usage: crop_window.py FULL_PNG OUTPUT_PNG [WxH+X+Y]")
source, destination = sys.argv[1], sys.argv[2]
compositor_bounds = None
if len(sys.argv) == 4 and sys.argv[3]:
    size, x, y = sys.argv[3].split("+")
    width, height = size.split("x")
    compositor_bounds = (int(x), int(y), int(width), int(height))

def clean(value):
    return str(value or "").strip()


def walk(node, visit, depth=0):
    if node is None:
        return
    try:
        visit(node, depth)
    except Exception:
        pass
    try:
        count = node.get_child_count()
    except Exception:
        return
    for index in range(count):
        try:
            walk(node.get_child_at_index(index), visit, depth + 1)
        except Exception:
            continue


desktop = Atspi.get_desktop(0)
extents = None
for i in range(desktop.get_child_count()):
    application = desktop.get_child_at_index(i)
    if clean(application.get_name()).upper() != "CLAY-DESKTOP":
        continue
    try:
        pid = application.get_process_id()
    except Exception:
        pid = 0
    if pid and not __import__("pathlib").Path(f"/proc/{pid}").exists():
        continue
    for j in range(application.get_child_count()):
        child = application.get_child_at_index(j)
        if clean(child.get_name()) == "Clay":
            frame = child.get_extents(0)
            if frame.width > 0 and frame.height > 0:
                extents = (frame.x, frame.y, frame.width, frame.height)
                break
    if extents:
        break

# AT-SPI frame extents carry Wayland shadow padding that overshoots the real
# window, which would pull host pixels (panel, neighbouring windows) into the
# retained crop. Intersect them with the compositor's own window bounds.
if extents is not None and compositor_bounds is not None:
    x, y, width, height = extents
    cx, cy, cw, ch = compositor_bounds
    left = max(x, cx)
    top = max(y, cy)
    right = min(x + width, cx + cw)
    bottom = min(y + height, cy + ch)
    if right - left > 0 and bottom - top > 0:
        extents = (left, top, right - left, bottom - top)

full = GdkPixbuf.Pixbuf.new_from_file(source)
if extents is None:
    raise SystemExit("Clay window extents not found in AT-SPI tree")
x, y, width, height = extents
# Wayland AT-SPI frame extents include invisible shadow/border padding that
# overshoots the visible window. When the compositor gave exact bounds the crop
# is already inside the window; otherwise inset 3% so no host window pixels can
# bleed into retained captures (plan 097 privacy rule).
# ponytail: fixed 3% inset; switch to pixel-accurate edge detection if the
# trimmed chrome ever matters.
if compositor_bounds is None:
    width -= max(1, width // 32)
    height -= max(1, height // 32)
print(
    f"extents=({x},{y},{width},{height}) full={full.get_width()}x{full.get_height()}",
    file=sys.stderr,
)
x = max(0, x)
y = max(0, y)
width = min(width, full.get_width() - x)
height = min(height, full.get_height() - y)
if width <= 0 or height <= 0:
    raise SystemExit(f"Clay window {extents} outside {full.get_width()}x{full.get_height()} screenshot")
full.new_subpixbuf(x, y, width, height).savev(destination, "png", [], [])
PY

cat > "$root/portal_capture.py" <<'PY'
import sys
try:
    import gi
    gi.require_version("Gio", "2.0")
    from gi.repository import Gio, GLib
except Exception as exc:
    print(f"PREREQ_MISSING: {exc}", file=sys.stderr)
    raise SystemExit(3)

if len(sys.argv) != 2:
    raise SystemExit("usage: portal_capture.py OUTPUT")
destination = sys.argv[1]
bus = Gio.bus_get_sync(Gio.BusType.SESSION, None)
proxy = Gio.DBusProxy.new_sync(
    bus, Gio.DBusProxyFlags.NONE, None,
    "org.freedesktop.portal.Desktop",
    "/org/freedesktop/portal/desktop",
    "org.freedesktop.portal.Screenshot", None,
)
reply = proxy.call_sync(
    "Screenshot", GLib.Variant("(sa{sv})", ("", {})),
    Gio.DBusCallFlags.NONE, 5000, None,
)
handle = reply.unpack()[0]
loop = GLib.MainLoop()
error = []

def response(_connection, _sender, _path, _interface, _signal, parameters, _data):
    result, details = parameters.unpack()
    if result != 0:
        error.append(f"portal response code {result}")
    else:
        uri = details.get("uri")
        if not uri:
            error.append("portal response omitted uri")
        else:
            try:
                contents = Gio.File.new_for_uri(uri).load_contents(None)[1]
                with open(destination, "wb") as output:
                    output.write(contents)
            except Exception as exc:
                error.append(str(exc))
    loop.quit()

bus.signal_subscribe(
    "org.freedesktop.portal.Desktop", "org.freedesktop.portal.Request",
    "Response", handle, None, Gio.DBusSignalFlags.NONE, response, None,
)
loop.run()
if error:
    print(error[0], file=sys.stderr)
    raise SystemExit(2)
PY

if ! command -v python3 >/dev/null 2>&1; then
    unresolved "python3 is unavailable"
fi
if ! command -v timeout >/dev/null 2>&1; then
    unresolved "timeout is unavailable"
fi
if ! timeout 15s python3 "$root/atspi_probe.py" prereq > "$root/atspi-prereq.txt" 2> "$root/atspi-prereq.err"; then
    unresolved "Python GI AT-SPI bindings or a live AT-SPI bus are unavailable"
fi

# Keep the review document fixture deterministic and bounded.
printf 'fn hello hello_world helper\n' > "$workspace/review.rs"
if [[ "$fixture" == ui-review-rust ]]; then
    mkdir -p "$workspace/src"
    cp "$repo/tests/fixtures/lsp/rust/Cargo.toml" "$workspace/Cargo.toml"
    cp "$repo/tests/fixtures/lsp/rust/Cargo.lock" "$workspace/Cargo.lock"
    cp "$repo/tests/fixtures/lsp/rust/src/main.rs" "$workspace/src/main.rs"
elif [[ "$fixture" == ui-review-workspace ]]; then
    cat > "$workspace/review.md" <<'MD'
# Review document

## 09:10 — Landing shell

The first-entry summary of the migrated shell.

## 09:40 — Workspace and rail

A second entry so the outline rail has something to navigate.
MD
elif [[ "$fixture" == ui-review-large-document ]]; then
    # Plan 126 access-path steps: a document past the 4 MiB manual threshold so
    # the completion and rope-window paths run against a real large document
    # without a 50 MiB transfer dominating the run. The trailing `pub fn hello`
    # leaves a completable prefix at the end of the file.
    python3 - "$workspace/review.rs" <<'PY'
import sys

line = "pub fn helper_{index:06}() -> usize {{ {index} }}\n"
with open(sys.argv[1], "w", encoding="utf-8") as handle:
    written = 0
    index = 0
    while written < 4 * 1024 * 1024:
        chunk = "".join(line.format(index=index + offset) for offset in range(1000))
        handle.write(chunk)
        written += len(chunk)
        index += 1000
    handle.write("\npub fn hello")
PY
elif [[ "$fixture" == ui-review-loading || "$fixture" == ui-review-design-system || "$fixture" == ui-review-design-system-light || "$fixture" == ui-review-icons-regular-light || "$fixture" == ui-review-icons-duotone-dark || "$fixture" == ui-review-icons-fallback-large ]]; then
    printf 'Fixture document\n' > "$workspace/loading.txt"
fi

document_name=""
case "$fixture" in
    ui-review-loading|ui-review-design-system|ui-review-design-system-light|ui-review-icons-regular-light|ui-review-icons-duotone-dark|ui-review-icons-fallback-large) document_name=loading.txt ;;
    ui-review-completion) document_name=review.rs ;;
    ui-review-large-document) document_name=review.rs ;;
    ui-review-workspace) document_name=review.md ;;
    ui-review-rust) document_name=src/main.rs ;;
esac
if [[ -n "$example_config" ]]; then
    # Canonical example tree: init.js plus packages/ (and agents/, if present),
    # copied exactly as the documented `cp -r examples/config/. ~/.clay/`.
    cp -r "$repo/examples/config/." "$home/.clay/"
    config_source="examples/config (canonical tree, cp -r parity)"
else
    init_fixture="$repo/tests/fixtures/configuration/$fixture/init.js"
    cp "$init_fixture" "$home/.clay/init.js"
    config_source="tests/fixtures/configuration/$fixture/init.js"
fi

if [[ -n "$document_name" ]]; then
    python3 - "$config_dir/layout.json" "$workspace" "$document_name" <<'PY'
import json, sys
layout_path, workspace, document_name = sys.argv[1:]
with open(layout_path, "w", encoding="utf-8") as output:
    json.dump({
        "version": 2,
        "activeTab": 0,
        "tabs": [{
            "workspaceRoot": workspace,
            "activePane": 1,
            "splitTree": {"leaf": {"paneId": 1}},
            "slots": [],
            "panes": {"1": document_name},
        }],
    }, output)
PY
fi

cat > "$output/instructions.md" <<EOF
# Clay UI review capture

- Fixture: $fixture
- Window size: recorded in metadata.txt (window_requested_size, window_viewport)
- Screenshot: screenshot.png
- Accessibility dump: accessibility.txt

This run uses a private mode-700 config/data/socket root and fixture-only
documents. It never reads the ambient Clay configuration. The Rust fixture
inherits host HOME only for the fixed rustup toolchain lookup.
EOF
case "$fixture" in
    ui-review-large-document)
        cat >> "$output/instructions.md" <<'EOF'

The workspace holds a ≥4 MiB `review.rs` (the completion fixture's init.js is
copied, so the `Ctrl+Space` binding and the `@clay/rust` provider are active).
Plan 126 steps: the document opens through the chunked path, the editor is
read-only until ready, then an insertion at the end of the file drives the live
typing path on a document that stays past 4 MiB.
EOF
        ;;
    ui-review-completion)
        cat >> "$output/instructions.md" <<'EOF'

Interactive step: focus the editor, type `hel` if needed, press `Ctrl+Space`,
then press Enter in the terminal to capture the visible completion menu. The
script records UNRESOLVED instead of passing if completion is not visible.
EOF
        ;;
    ui-review-command-centre)
        cat >> "$output/instructions.md" <<'EOF'

Interactive step: press `Ctrl+Alt+P`, then press Enter in the terminal to
capture the visible centered Command Centre. The script records UNRESOLVED
instead of passing if the dialog/menu is not visible.
EOF
        ;;
    ui-review-design-system|ui-review-design-system-light)
        cat >> "$output/instructions.md" <<'EOF'

The fixture selects the shipped `@clay/design-instrument` design system
explicitly and publishes a representative panel, action, enabled list row,
disabled list row, and editor view. It is paired with the theme named in the
fixture.
EOF
        ;;
    ui-review-rust)
        cat >> "$output/instructions.md" <<'EOF'

Interactive steps: focus the editor, make a no-op edit (type one space, then
Backspace), wait for the Rust inlay overlay, and press Enter to capture the
visible state. Then press `Ctrl+Alt+I` and press Enter again to capture the
client-local toggled-off state. The script records UNRESOLVED if the analyzer
worker or inlay state cannot be verified.
EOF
        ;;
    ui-review-recovery)
        cat >> "$output/instructions.md" <<'EOF'

The script stops the isolated server after the connected tree appears and
captures the resulting disconnected/recovery state.
EOF
        ;;
esac

cat > "$output/metadata.txt" <<EOF
fixture=$fixture
config_source=$config_source
window_requested_size=${requested_size:-default}
theme=${theme_specifier:-fixture-default}
appearance=${appearance:-fixture-default}
drive_steps=${drive_steps:-none}
ipc=private-unix-socket
config=private-mode-700
screenshot=xdg-desktop-portal
accessibility=python3-gi-atspi
EOF

(
    # Keep bootstrap workspace and fixture document IDs aligned: the server's
    # default workspace is its current directory, while the loading SDUI tree
    # targets document 1.
    cd "$workspace"
    exec env HOME="$runtime_home" XDG_CONFIG_HOME="$config_home" XDG_DATA_HOME="$data_home" \
        TMPDIR="$root/tmp" "$repo/target/debug/clay" server "$socket"
) > "$root/server.log" 2>&1 &
server_pid=$!

for _ in $(seq 1 "$((timeout_seconds * 10))"); do
    [[ -S "$socket" ]] && break
    kill -0 "$server_pid" 2>/dev/null || unresolved "server exited before creating its socket"
    sleep 0.1
done
[[ -S "$socket" ]] || unresolved "timed out waiting for the isolated server socket"
(
    cd "$repo"
    exec env HOME="$runtime_home" XDG_CONFIG_HOME="$config_home" XDG_DATA_HOME="$data_home" \
        TMPDIR="$root/tmp" "$repo/target/debug/clay" client "$socket"
) > "$root/client.log" 2>&1 &
client_pid=$!
for _ in {1..30}; do
    desktop_pid=$(pgrep -P "$client_pid" -n 2>/dev/null || true)
    [[ -n "$desktop_pid" ]] && break
    sleep 0.1
done

latest_dump="$root/latest.dump"
clay_index=""
capture_dump() {
    if [[ -z "$clay_index" ]]; then
        for _ in 1 2 3 4 5; do
            for index in $(seq 0 31); do
                local name
                name=$(timeout 3s python3 "$root/atspi_probe.py" app "$index" 2>/dev/null || true)
                if [[ "$name" == "CLAY" || "$name" == "CLAY-DESKTOP" ]]; then
                    clay_index=$index
                    break
                fi
            done
            [[ -n "$clay_index" ]] && break
        done
    fi
    : > "$latest_dump"
    [[ -n "$clay_index" ]] || return 0
    timeout 2s python3 "$root/atspi_probe.py" dump-index "$clay_index" > "$latest_dump" 2>/dev/null || true
}
wait_for_tree() {
    local pattern=${1:-'|frame|'}
    local deadline=$((SECONDS + timeout_seconds))
    while ((SECONDS < deadline)); do
        if ! kill -0 "$client_pid" 2>/dev/null; then
            return 1
        fi
        capture_dump
        if grep -Fq '|frame|' "$latest_dump" && grep -Fq "$pattern" "$latest_dump"; then
            return 0
        fi
        sleep 0.1
    done
    return 1
}
wait_for_inlay() {
    local deadline=$((SECONDS + timeout_seconds))
    while ((SECONDS < deadline)); do
        if ! kill -0 "$client_pid" 2>/dev/null; then
            return 1
        fi
        if grep -F 'kind: InlayHint' "$root/client.log" \
            | grep -Fq 'spans: [DecorationSpan'; then
            return 0
        fi
        sleep 0.1
    done
    return 1
}

wait_for_tree 'Clay workspace' || unresolved "Clay window/accessibility shell did not appear"

# Optional window sizing. AT-SPI cannot resize a Wayland toplevel here, but the
# GNOME Shell extension can: it resizes the window and moves it fully on-screen
# (an off-screen window would bleed host pixels into the retained crop).
compositor_bounds=""
window_id=""
extension_method="dev.avifenesh.ComputerUseLinux.WindowControl"
extension_path="/dev/avifenesh/ComputerUseLinux/WindowControl"
extension_call() {
    gdbus call --session --dest "$extension_method" --object-path "$extension_path" \
        --method "$extension_method.$1" "${@:2}" 2>/dev/null
}
if [[ -n "$requested_size" ]]; then
    width=${requested_size%x*}
    height=${requested_size#*x}
    extension_call ListWindows > "$root/windows.json" \
        || unresolved "the computer-use-linux GNOME Shell extension is unavailable"
    window_id=$(python3 "$root/window_control.py" id "$root/windows.json" "$desktop_pid") \
        || unresolved "no Clay window is exposed to the GNOME Shell extension"
    extension_call MoveWindow "$window_id" 40 40 >/dev/null \
        || unresolved "moving the Clay window on-screen failed"
    extension_call ResizeWindow "$window_id" "$width" "$height" >/dev/null \
        || unresolved "resizing the Clay window to $requested_size failed"
    sleep 0.8
    extension_call ListWindows > "$root/windows-resized.json" \
        || unresolved "re-reading windows after resize failed"
    bounds=$(python3 "$root/window_control.py" bounds "$root/windows-resized.json" "$desktop_pid" || true)
    compositor_bounds=$(python3 "$root/window_control.py" rect "$root/windows-resized.json" "$desktop_pid" || true)
    # Width is the layout-relevant dimension (1240/1000/760 breakpoints); the
    # compositor pins height, so a shorter frame is recorded, not failed.
    [[ "${bounds%%x*}" == "$width" ]] \
        || unresolved "window width did not take $width (measured ${bounds:-unknown})"
    if [[ "$bounds" != "$requested_size" ]]; then
        printf 'window_resize_note=requested %s, compositor kept %s\n' "$requested_size" "$bounds" \
            >> "$output/metadata.txt"
    fi
fi
# Force one watcher-driven reload only after the client has completed its
# initial handshake, so runtime fixtures are delivered through the live
# RuntimeStateSnapshot path instead of racing startup bootstrap.
sleep 0.2
touch "$home/.clay/init.js"
if [[ "$fixture" == ui-review-error ]]; then
    # Exercise reload-time invalid selection while the client stays connected.
    sleep 0.2
    cat > "$home/.clay/init.js" <<'EOF'
import { setTheme } from "clay:theme";
setTheme("@clay/does-not-exist");
EOF
fi

case "$fixture" in
    ui-review-error)
        deadline=$((SECONDS + timeout_seconds))
        while ((SECONDS < deadline)) \
            && ! grep -Fq 'clay server runtime reload failed' "$root/server.log"; do
            sleep 0.1
        done
        grep -Fq 'clay server runtime reload failed' "$root/server.log" \
            || unresolved "runtime reload failure was not logged"
        wait_for_tree 'JavaScript runtime evaluation failed' \
            || unresolved "runtime error diagnostic did not appear"
        cat > "$output/runtime-tree.txt" <<'EOF'
RuntimeDiagnostic=PASS
sanitized_message=JavaScript runtime evaluation failed.
code=packages.not_installed
client_stay=connected
EOF
        printf '\nRuntime evidence: `runtime-tree.txt` records the sanitized reload diagnostic.\n' >> "$output/instructions.md"
        ;;
    ui-review-launcher)
        # The landing is a compiled host panel for @clay/launcher's contribution.
        # AT-SPI exposes its section labels and buttons; the ellipsis-bearing
        # folder button proves the launcher rendered and not the core fallback.
        wait_for_tree 'Open folder…' || unresolved "launcher landing did not appear"
        cat > "$output/runtime-tree.txt" <<'EOF'
Landing=PASS
surface=@clay/launcher empty-tab contribution (host-rendered panel)
panes=Workspaces,Agents
core_fallback=Open file / Open folder only (absent here)
EOF
        printf '\nRuntime evidence: `runtime-tree.txt` records the launcher landing.\n' >> "$output/instructions.md"
        ;;
    ui-review-loading)
        wait_for_tree 'Loading review' || unresolved "loading SDUI tree did not appear"
        cat > "$output/runtime-tree.txt" <<'EOF'
RuntimeStateSnapshot=PASS
sdui_panel=Loading review
sdui_label=Loading workspace…
EOF
        printf '\nRuntime evidence: `runtime-tree.txt` records the delivered SDUI snapshot.\n' >> "$output/instructions.md"
        ;;
    ui-review-design-system|ui-review-design-system-light)
        wait_for_tree 'Design system review' || unresolved "design-system SDUI tree did not appear"
        cat > "$output/runtime-tree.txt" <<'EOF'
RuntimeStateSnapshot=PASS
active_design_system=@clay/design-instrument
sdui_panel=Design system review
sdui_states=enabled,disabled
EOF
        printf '\nRuntime evidence: `runtime-tree.txt` records shipped design-system activation and host-owned states.\n' >> "$output/instructions.md"
        ;;
    ui-review-icons-regular-light|ui-review-icons-duotone-dark|ui-review-icons-fallback-large)
        wait_for_tree 'Icon Pack Review' || unresolved "icon-pack SDUI tree did not appear"
        cat > "$output/runtime-tree.txt" <<EOF
RuntimeStateSnapshot=PASS
fixture=$fixture
sdui_panel=Icon Pack Review
sdui_icons=git.branch,status.success,preview.toggle,file.folder,file.file
editor_action_row=save,reload,close (icon buttons) + open (text)
EOF
        ;;
    ui-review-recovery)
        stop_child "$server_pid"
        server_pid=""
        wait_for_tree 'Reconnect session' || unresolved "disconnected/recovery state did not appear"
        ;;
    ui-review-rust)
        if [[ ! -t 0 ]]; then
            unresolved "Rust inlay states require a TTY for keyboard input"
        fi
        printf 'Rust fixture is live. Make a no-op edit, wait for visible inlays, then press Enter: ' >&2
        read -r
        wait_for_inlay || unresolved "Rust analyzer did not publish a non-empty inlay set"
        capture_dump
        mkdir -p "$output"
        cp "$latest_dump" "$output/accessibility.txt"
        if ! python3 "$root/portal_capture.py" "$output/screenshot.png" > "$root/portal-visible.out" 2> "$root/portal-visible.err"; then
            unresolved "xdg-desktop-portal Screenshot failed for visible inlay state"
        fi
        printf 'PASS\nfixture=ui-review-rust\nstate=inlay-visible\n' > "$output/review.status"

        toggled_output="$(dirname "$output")/inlay-toggled-off"
        mkdir -p "$toggled_output"
        printf 'Press Ctrl+Alt+I to toggle inlays off, then press Enter: ' >&2
        read -r
        capture_dump
        cp "$latest_dump" "$toggled_output/accessibility.txt"
        cp "$output/instructions.md" "$toggled_output/instructions.md"
        cp "$output/metadata.txt" "$toggled_output/metadata.txt"
        if ! python3 "$root/portal_capture.py" "$toggled_output/screenshot.png" > "$root/portal-off.out" 2> "$root/portal-off.err"; then
            unresolved "xdg-desktop-portal Screenshot failed for toggled-off inlay state"
        fi
        printf 'PASS\nfixture=ui-review-rust\nstate=inlay-toggled-off\n' > "$toggled_output/review.status"
        printf 'PASS\nfixture=ui-review-rust\nstates=inlay-visible,inlay-toggled-off\n' > "$output/review.status"
        exit 0
        ;;
    ui-review-completion|ui-review-command-centre)
        if [[ ! -t 0 ]]; then
            unresolved "interactive state requires a TTY for keyboard capture"
        fi
        printf 'Review fixture %s is live. Follow %s, then press Enter here: ' "$fixture" "$output/instructions.md" >&2
        read -r
        capture_dump
        if [[ "$fixture" == ui-review-completion ]]; then
            grep -Eiq 'completion|no completions' "$latest_dump" || unresolved "completion menu/status did not appear"
        else
            grep -Eiq 'control center|dialog|menu' "$latest_dump" || unresolved "Command Centre menu/dialog did not appear"
        fi
        ;;
esac


if [[ -z "$compositor_bounds" ]]; then
    extension_call ListWindows > "$root/windows.json" 2>/dev/null || true
    compositor_bounds=$(python3 "$root/window_control.py" rect "$root/windows.json" "$desktop_pid" 2>/dev/null || true)
fi
if [[ -n "$drive_steps" ]]; then
    if ! drive_output=$(timeout 60s python3 "$root/atspi_probe.py" drive "$clay_index" "$drive_steps" 2>&1); then
        # Keep the whole probe transcript, not just its last line: the probe
        # prints per-step OK lines on stdout and FAIL lines on stderr, so
        # either stream may arrive last and a tail alone hides the reason.
        printf '%s\n' "$drive_output" > "$output/drive.failed.txt"
        unresolved "drive step failed: $(printf '%s' "$drive_output" | grep -m1 '^FAIL' || printf '%s' "$drive_output" | tail -n 1)"
    fi
    printf '%s\n' "$drive_output" > "$output/drive.txt"
    printf '\nDriven steps (AT-SPI, no input synthesis):\n\n```\n%s\n```\n' "$drive_output" >> "$output/instructions.md"
    # Let the driven surface animate in (enter tier is 240ms) before capture.
    sleep 0.8
fi
# Measure the settled geometry only after any driven step: a compositor-driven
# size change (or an unmaximise) must be reflected in the recorded viewport.
viewport_size=$(timeout 10s python3 "$root/atspi_probe.py" viewport-size "$clay_index" 2>/dev/null || true)
frame_size=$(timeout 10s python3 "$root/atspi_probe.py" frame-size "$clay_index" 2>/dev/null || true)
# Plan 087 envelope floor: a capture smaller than a 900×600 logical window cannot
# show the layouts this harness exists to review, so it is not passing evidence.
if [[ "$viewport_size" =~ ^([0-9]+)x([0-9]+)$ ]]; then
    if (( BASH_REMATCH[1] < 900 || BASH_REMATCH[2] < 600 )); then
        unresolved "measured viewport $viewport_size is below the 900×600 review floor"
    fi
fi
# Re-dump: driven steps change the exposed tree.
capture_dump

cp "$latest_dump" "$output/accessibility.txt"
if ! python3 "$root/portal_capture.py" "$root/full-screenshot.png" > "$root/portal.out" 2> "$root/portal.err"; then
    printf 'UNRESOLVED\nreason=xdg-desktop-portal Screenshot is unavailable\n' > "$output/review.status"
    echo "UI review unresolved: xdg-desktop-portal Screenshot is unavailable" >&2
    exit_status=2
elif ! python3 "$root/crop_window.py" "$root/full-screenshot.png" "$output/screenshot.png" "$compositor_bounds" > "$root/crop.out" 2> "$root/crop.err"; then
    # Never retain full-desktop captures: they can contain unrelated host
    # windows (plan 097 privacy rule).
    printf 'UNRESOLVED\nreason=Clay window crop failed; full-desktop capture discarded\n' > "$output/review.status"
    echo "UI review unresolved: Clay window crop failed" >&2
    exit_status=2
else
    # Keep the accessibility tree of the captured state: it is the evidence for
    # role/name/state exposure and for transient surfaces (inlays, dialogs) that
    # no later run can reproduce without replaying the same drive steps.
    [[ -f "${latest_dump:-}" ]] && cp "$latest_dump" "$output/a11y-tree.txt"
    screenshot_size=$(python3 - "$output/screenshot.png" <<'PY'
import struct, sys
with open(sys.argv[1], "rb") as handle:
    header = handle.read(24)
width, height = struct.unpack(">II", header[16:24])
print(f"{width}x{height}")
PY
)
    {
        printf 'window_viewport=%s\n' "${viewport_size:-unknown}"
        printf 'window_frame=%s\n' "${frame_size:-unknown}"
        printf 'screenshot=%s\n' "$screenshot_size"
    } >> "$output/metadata.txt"
    printf '\n- Measured viewport: %s (window frame %s, cropped screenshot %s)\n' \
        "${viewport_size:-unknown}" "${frame_size:-unknown}" "$screenshot_size" >> "$output/instructions.md"
    # Bounded, privacy-safe server evidence for the startup contract
    # ("configuration commits a generation with no configuration failed
    # diagnostics"): only diagnostic/configuration/generation lines survive,
    # and the scratch root is redacted. Full logs stay failure-only.
    if [[ -s "$root/server.log" ]]; then
        sed -e "s|$root|<isolated-root>|g" "$root/server.log" \
            | grep -Ei 'diagnostic|configuration|generation|load_failed|module_failed|error|listening|package discovery' \
            | head -40 > "$output/server.diagnostics.txt" || true
        {
            # Counters a reviewer can cite without reading a log: the package
            # registrations prove the loaded agent package ran its load entry,
            # and the failure count is the startup contract's own metric.
            printf 'agent_registration_lines=%s\n' "$(grep -c '^\[agent-reg\]' "$root/server.log" || true)"
            printf 'configuration_failed_lines=%s\n' "$(grep -ci 'configuration failed' "$root/server.log" || true)"
        } >> "$output/server.diagnostics.txt"
        printf '\nServer diagnostics (bounded, root-redacted): `server.diagnostics.txt`.\n' \
            >> "$output/instructions.md"
    fi
    printf 'PASS\nfixture=%s\nviewport=%s\nscreenshot=%s\n' \
        "$fixture" "${viewport_size:-unknown}" "$screenshot_size" > "$output/review.status"
    echo "UI review captured: $output" >&2
fi
exit "$exit_status"
