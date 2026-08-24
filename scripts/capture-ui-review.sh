#!/usr/bin/env bash
# Capture one isolated Clay UI review state.
set -Eeuo pipefail
umask 077

repo=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
fixture=""
output=""
timeout_seconds="${CLAY_UI_REVIEW_TIMEOUT_SECONDS:-45}"

usage() {
    cat <<'EOF'
Usage: scripts/capture-ui-review.sh --fixture <name> --output <directory>

Fixtures:
  ui-review-default         clean Clay shell/welcome state
  ui-review-loading         deterministic loading-state SDUI panel
  ui-review-error           configuration/runtime error state
  ui-review-recovery        disconnected/recovery state after server stop
  ui-review-large-typography user-owned large typography state
  ui-review-completion      completion-ready document (interactive capture)
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
    ui-review-default|ui-review-loading|ui-review-error|ui-review-recovery|ui-review-large-typography|ui-review-completion|ui-review-command-centre|ui-review-rust) ;;
    *)
        echo "unknown --fixture: ${fixture:-<missing>}" >&2
        usage >&2
        exit 2
        ;;
esac
[[ -n "$output" ]] || { echo "--output is required" >&2; exit 2; }
[[ "$timeout_seconds" =~ ^[1-9][0-9]*$ ]] || { echo "--timeout must be a positive integer" >&2; exit 2; }

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
mkdir -p "$config_dir" "$data_home" "$home/.config/clay" "$workspace" "$root/tmp"
chmod 700 "$config_home" "$config_dir" "$data_home" "$home" "$home/.config" "$home/.config/clay" "$workspace" "$root/tmp"

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
import sys
try:
    import gi
    gi.require_version("Atspi", "2.0")
    from gi.repository import Atspi
except Exception as exc:
    print(f"PREREQ_MISSING: {exc}", file=sys.stderr)
    raise SystemExit(3)

if not sys.argv or sys.argv[1] not in {"prereq", "app", "dump-index"}:
    raise SystemExit("usage: atspi_probe.py prereq|app INDEX|dump-index INDEX")

desktop = Atspi.get_desktop(0)
if sys.argv[1] == "prereq":
    print(f"OK apps={desktop.get_child_count()}")
    raise SystemExit(0)
if len(sys.argv) != 3:
    raise SystemExit("usage: atspi_probe.py app INDEX|dump-index INDEX")
application = desktop.get_child_at_index(int(sys.argv[2]))
if sys.argv[1] == "app":
    print(str(application.get_name() or "").strip().upper())
    raise SystemExit(0)

def clean(value):
    return str(value or "").replace("|", " ").replace("\n", " ")

def walk(node, depth):
    if node is None:
        return
    try:
        app = node.get_application()
        app_name = clean(app.get_name() if app is not None else "")
        if app_name.lower() in {"clay", "clay-desktop"}:
            selected = "selected" if node.get_state_set().contains(Atspi.StateType.SELECTED) else "-"
            print("|".join([
                str(depth), clean(node.get_role_name()), selected,
                clean(node.get_name()), clean(node.path), app_name,
            ]))
    except Exception:
        return
    try:
        count = node.get_child_count()
    except Exception:
        return
    for index in range(count):
        try:
            walk(node.get_child_at_index(index), depth + 1)
        except Exception:
            continue

walk(application, 0)
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
elif [[ "$fixture" == ui-review-loading ]]; then
    printf 'Fixture document\n' > "$workspace/loading.txt"
else
    printf 'Loading workspace…\n' > "$workspace/loading.txt"
fi

document_name=""
case "$fixture" in
    ui-review-loading) document_name=loading.txt ;;
    ui-review-completion) document_name=review.rs ;;
    ui-review-rust) document_name=src/main.rs ;;
esac
init_fixture="$repo/tests/fixtures/configuration/$fixture/init.js"
cp "$init_fixture" "$home/.config/clay/init.js"

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
- Logical window: 900×600
- Screenshot: screenshot.png
- Accessibility dump: accessibility.txt

This run uses a private mode-700 config/data/socket root and fixture-only
documents. It never reads the ambient Clay configuration. The Rust fixture
inherits host HOME only for the fixed rustup toolchain lookup.
EOF
case "$fixture" in
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
window_logical_size=900x600
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
wait_for_runtime_loading_tree() {
    local deadline=$((SECONDS + timeout_seconds))
    while ((SECONDS < deadline)); do
        if ! kill -0 "$client_pid" 2>/dev/null; then
            return 1
        fi
        if grep -Fq 'title: "Loading review"' "$root/client.log" \
            && grep -Fq 'text: "Loading workspace…"' "$root/client.log"; then
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
# Force one watcher-driven reload only after the client has completed its
# initial handshake, so runtime fixtures are delivered through the live
# RuntimeStateSnapshot path instead of racing startup bootstrap.
sleep 0.2
touch "$home/.config/clay/init.js"

case "$fixture" in
    ui-review-error)
        wait_for_tree 'Runtime' || unresolved "runtime error diagnostic did not appear"
        ;;
    ui-review-loading)
        wait_for_runtime_loading_tree || unresolved "loading SDUI tree did not appear"
        cat > "$output/runtime-tree.txt" <<'EOF'
RuntimeStateSnapshot=PASS
sdui_panel=Loading review
sdui_label=Loading workspace…
EOF
        printf '\nRuntime evidence: `runtime-tree.txt` records the delivered SDUI snapshot.\n' >> "$output/instructions.md"
        ;;
    ui-review-recovery)
        stop_child "$server_pid"
        server_pid=""
        wait_for_tree 'Disconnected' || wait_for_tree 'Recovery:' || unresolved "disconnected/recovery state did not appear"
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

cp "$latest_dump" "$output/accessibility.txt"
if ! python3 "$root/portal_capture.py" "$output/screenshot.png" > "$root/portal.out" 2> "$root/portal.err"; then
    printf 'UNRESOLVED\nreason=xdg-desktop-portal Screenshot is unavailable\n' > "$output/review.status"
    echo "UI review unresolved: xdg-desktop-portal Screenshot is unavailable" >&2
    exit_status=2
else
    printf 'PASS\nfixture=%s\n' "$fixture" > "$output/review.status"
    echo "UI review captured: $output" >&2
fi
exit "$exit_status"
