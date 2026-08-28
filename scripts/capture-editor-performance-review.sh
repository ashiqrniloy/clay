#!/usr/bin/env bash
# Plan 099 editor visual/performance review capture (real Tauri/WebKit run).
#
# Launches an isolated profiled server+desktop pair with a synthetic fixture
# workspace and one deterministic editor state per --state value. While the
# window is open, the operator captures screenshots (computer-use) at the
# staged moments; closing the window finalizes the artifact directory with the
# per-process performance reports.
#
# States: editor-light, editor-dark-four-pane, editor-large-loading,
#         editor-diagnostics, editor-binary-error, editor-budget-error,
#         editor-large-typography
#
# Artifacts under --output: metadata.txt, layout.json copy, init.js copy,
# perf reports (frontend snapshot + server summary), review.status.
set -Eeuo pipefail
umask 077

repo="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
state=""
output=""
timeout_seconds="${CLAY_REVIEW_TIMEOUT:-120}"

usage() {
    grep '^# ' - <<'EOF'
# Usage: scripts/capture-editor-performance-review.sh --state <name> --output <dir> [--timeout <s>]
EOF
    exit 2
}

while (($#)); do
    case "$1" in
        --state) state=$2; shift 2 ;;
        --output) output=$2; shift 2 ;;
        --timeout) timeout_seconds=$2; shift 2 ;;
        -h|--help) usage ;;
        *) echo "unknown argument: $1" >&2; exit 2 ;;
    esac
done
[[ -n "$state" && -n "$output" ]] || usage

case "$state" in
    editor-light | editor-dark-four-pane | editor-large-loading | \
        editor-diagnostics | editor-binary-error | editor-budget-error | \
        editor-large-typography) ;;
    *) echo "unknown state: $state" >&2; exit 2 ;;
esac

bin="$repo/target/debug/clay"
[[ -x "$bin" ]] || { echo "build first: cargo build --bin clay" >&2; exit 1; }
mkdir -p "$output"
# Absolute: the server/desktop subshells cd elsewhere before inheriting it.
output="$(cd "$output" && pwd)"

root="$(mktemp -d "${TMPDIR:-/tmp}/clay-editor-review.XXXXXX")"
home="$root/home"
config_home="$root/config"
data_home="$root/data"
mkdir -p "$home" "$config_home/clay" "$data_home"
workspace="$root/workspace"
mkdir -p "$workspace"
socket="$root/review.sock"

cleanup() {
    local status=$?
    trap - EXIT INT TERM
    [[ -n "${desktop_pid:-}" ]] && {
        pkill -TERM -P "$desktop_pid" 2>/dev/null || true
        kill "$desktop_pid" 2>/dev/null || true
    }
    if [[ -n "${server_pid:-}" ]] && kill -0 "$server_pid" 2>/dev/null; then
        kill -TERM "$server_pid" 2>/dev/null || true
        for _ in $(seq 1 30); do
            kill -0 "$server_pid" 2>/dev/null || break
            sleep 0.1
        done
    fi
    for log in server.log desktop.log; do
        [[ -f "$root/$log" ]] && cp "$root/$log" "${output:-/tmp}/$log" 2>/dev/null || true
    done
    rm -rf "$root"
    exit "$status"
}
trap cleanup EXIT
trap 'exit 130' INT TERM

# ---- synthetic fixture documents (never user content) -----------------------
cat >"$workspace/notes.md" <<'MD'
# Review notes

Introductory paragraph for the visual review pass.

## Section one

- alpha
- bravo
- charlie

```rust
fn reviewed() -> u32 { 7 }
```

## Section two

Closing text with enough body to fill a viewport and exercise wrapping.
MD

cat >"$workspace/review.rs" <<'RS'
//! Fixture module for syntax and fold review.
pub struct Frame {
    pub id: u64,
}

impl Frame {
    pub fn new(id: u64) -> Self {
        Self { id }
    }

    pub fn render(&self) -> String {
        format!("frame-{}", self.id)
    }
}

pub fn summarize(frames: &[Frame]) -> String {
    frames
        .iter()
        .map(Frame::render)
        .collect::<Vec<_>>()
        .join(",")
}
RS

cat >"$workspace/module.ts" <<'TS'
export interface PaneState {
  readonly paneId: number;
  documentPath: string | null;
}

export function describe(pane: PaneState): string {
  return pane.documentPath ?? `pane-${pane.paneId}`;
}
TS

printf 'plain text line %06d: the quick brown fox jumps over the lazy dog\n' \
    "$(seq 1 16384 | tr '\n' ' ')" >/dev/null 2>&1 || true
python3 - "$workspace/medium.txt" <<'PY'
import sys
with open(sys.argv[1], "w") as out:
    for i in range(16384):
        out.write(f"plain text line {i:06d}: the quick brown fox jumps over the lazy dog\n")
PY

large="$repo/target/perf-fixtures/perf-50mib-newline-heavy.txt"
[[ -f "$large" ]] || "$bin" perf-fixture --kind newline-heavy --size-mib 50 \
    --seed 9001 --output "$large" >/dev/null
cp "$large" "$workspace/large.txt"

cat >"$workspace/broken.md" <<'MD'
# Roadmap

Marksman reports the missing link here: [[missing-doc]].
MD

printf 'CLAY\x00binary\x01fixture' >"$workspace/sample.bin"
python3 - "$workspace/huge.txt" <<'PY'
import os, sys
with open(sys.argv[1], "wb") as out:
    out.truncate(300 * 1024 * 1024)  # sparse; exceeds the 256 MiB budget
PY

# ---- per-state config + layout ---------------------------------------------
panes_python='{"version": 2, "activeTab": 0, "tabs": [{"workspaceRoot": ARGV[1], "activePane": 1, "splitTree": json.loads(ARGV[2]), "slots": [], "panes": json.loads(ARGV[3])}]}'
write_layout() {
    python3 - "$config_home/clay/layout.json" "$workspace" "$1" "$2" <<'PY'
import json, sys
path, workspace, tree, panes = sys.argv[1:]
with open(path, "w", encoding="utf-8") as out:
    json.dump({
        "version": 2,
        "activeTab": 0,
        "tabs": [{
            "workspaceRoot": workspace,
            "activePane": 1,
            "splitTree": json.loads(tree),
            "slots": [],
            "panes": json.loads(panes),
        }],
    }, out)
PY
}

single='{"leaf": {"paneId": 1}}'
four='{"split": {"orientation": "horizontal", "ratio": 0.5, "first": {"split": {"orientation": "vertical", "ratio": 0.5, "first": {"leaf": {"paneId": 1}}, "second": {"leaf": {"paneId": 2}}}}, "second": {"split": {"orientation": "vertical", "ratio": 0.5, "first": {"leaf": {"paneId": 3}}, "second": {"leaf": {"paneId": 4}}}}}}'

# The server config runtime resolves init.js from $HOME/.config/clay while
# the desktop layout reader honors XDG_CONFIG_HOME; provide both.
mkdir -p "$home/.config/clay"
init_target="$config_home/clay/init.js"
init_home_copy="$home/.config/clay/init.js"
case "$state" in
    editor-light)
        cat >"$init_target" <<'JS'
import { setTheme } from "clay:theme";
import { loadPackage } from "clay:packages";
setTheme("@clay/theme-modus-operandi");
await loadPackage("@clay/markdown");
JS
        write_layout "$single" '{"1": "notes.md"}'
        ;;
    editor-dark-four-pane)
        cat >"$init_target" <<'JS'
import { setTheme } from "clay:theme";
import { loadPackage } from "clay:packages";
setTheme("@clay/theme-modus-vivendi");
await loadPackage("@clay/markdown");
await loadPackage("@clay/rust");
await loadPackage("@clay/typescript");
JS
        write_layout "$four" '{"1": "notes.md", "2": "review.rs", "3": "module.ts", "4": "medium.txt"}'
        ;;
    editor-large-loading)
        cat >"$init_target" <<'JS'
import { loadPackage } from "clay:packages";
await loadPackage("@clay/markdown");
JS
        write_layout "$single" '{"1": "large.txt"}'
        ;;
    editor-diagnostics)
        cp "$repo/tests/fixtures/configuration/lsp-markdown/init.js" \
            "$init_target"
        write_layout "$single" '{"1": "broken.md"}'
        ;;
    editor-binary-error)
        cat >"$init_target" <<'JS'
import { loadPackage } from "clay:packages";
await loadPackage("@clay/markdown");
JS
        write_layout "$single" '{"1": "sample.bin"}'
        ;;
    editor-budget-error)
        cat >"$init_target" <<'JS'
import { loadPackage } from "clay:packages";
await loadPackage("@clay/markdown");
JS
        write_layout "$single" '{"1": "huge.txt"}'
        ;;
    editor-large-typography)
        cat >"$init_target" <<'JS'
import { setTypography } from "clay:theme";
import { loadPackage } from "clay:packages";
setTypography({
    monospace: { families: ["monospace"], size: 20 },
    proportional: { families: ["sans-serif"], size: 21 },
    ui: { families: ["system-ui"], size: 24 },
});
await loadPackage("@clay/markdown");
JS
        write_layout "$single" '{"1": "notes.md"}'
        ;;
esac
# Provide the HOME-resolved copy the server config runtime evaluates.
cp "$init_target" "$init_home_copy"
cp "$init_target" "$output/init.js"

# ---- launch profiled server + desktop ---------------------------------------
mkdir -p "$root/tmp"
(
    cd "$workspace" && exec env HOME="$home" XDG_CONFIG_HOME="$config_home" \
        XDG_DATA_HOME="$data_home" TMPDIR="$root/tmp" CLAY_PERF_PROFILE=1 \
        CLAY_PERF_REPORT_DIR="$output" "$bin" server "$socket"
) >"$root/server.log" 2>&1 &
server_pid=$!

for _ in $(seq 1 "$((timeout_seconds * 10))"); do
    [[ -S "$socket" ]] && break
    kill -0 "$server_pid" 2>/dev/null || {
        echo "server exited before creating its socket" >&2
        tail -5 "$root/server.log" >&2
        exit 2
    }
    sleep 0.1
done
[[ -S "$socket" ]] || { echo "timed out waiting for the server socket" >&2; exit 2; }

(
    cd "$repo" && exec env HOME="$home" XDG_CONFIG_HOME="$config_home" \
        XDG_DATA_HOME="$data_home" TMPDIR="$root/tmp" CLAY_PERF_PROFILE=1 \
        CLAY_PERF_REPORT_DIR="$output" "$bin" client "$socket"
) >"$root/desktop.log" 2>&1 &
desktop_pid=$!

cat >"$output/metadata.txt" <<EOF
state=$state
window=tauri-webkit (logical 1280x800 unless a narrow build was configured)
ipc=private-unix-socket
config=private-mode-700
workspace=$workspace (synthetic fixtures only)
perf=CLAY_PERF_PROFILE=1 CLAY_PERF_REPORT_DIR=$output
EOF
cat >"$output/review.status" <<'EOF'
CAPTURED
EOF

# Operator closes the desktop window when the staged screenshots are done;
# the script then stops the server (which dumps its perf report) and exits.
set +e
wait "$desktop_pid"
desktop_pid=""
kill -TERM "$server_pid" 2>/dev/null
for _ in $(seq 1 30); do
    kill -0 "$server_pid" 2>/dev/null || break
    sleep 0.1
done
server_pid=""
set -e

echo "state $state captured into $output"
ls -1 "$output"
