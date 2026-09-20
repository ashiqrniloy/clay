#!/usr/bin/env bash
# Plan 128 manual steps: isolated live launches for the LSP incremental index.
#
#   launch-live.sh start lsp [bytes]     # tiny Cargo fixture (LSP works end to end)
#   launch-live.sh start mid [bytes]     # 250 KiB src/probe.rs (inside the LSP analysis cap)
#   launch-live.sh start large [bytes]   # layout opens the ≥1 MiB review.rs
#   launch-live.sh start large [bytes]   # layout opens the ≥1 MiB review.rs
#   launch-live.sh stop                  # also flushes the perf report
#   launch-live.sh status
#
# Isolation mirrors the plan 126/127 harnesses (private mode-700 root for
# HOME/XDG/TMPDIR, private socket, fixture workspace, tree-kill teardown). The
# isolated HOME gets read-only symlinks to the host ~/.rustup and ~/.cargo so
# the authorized `rustup run stable rust-analyzer` executable resolves.
set -Eeuo pipefail
umask 077

here=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
repo=$(cd "$here/../../.." && pwd)
root=${CLAY_LIVE_ROOT:-/tmp/clay-plan128-live}
mode=${2:-lsp}
size=${3:-1258291}

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
    # Graceful TERM lets the profiled server dump its perf report.
    for name in client server; do
        if [[ -f "$root/$name.pid" ]]; then
            kill -TERM "$(cat "$root/$name.pid")" 2>/dev/null || true
        fi
    done
    sleep 1
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
    status)
        [[ -S "$root/clay.sock" ]] && echo "socket live" || echo "no socket"
        for name in server client; do
            [[ -f "$root/$name.pid" ]] && echo "$name pid $(cat "$root/$name.pid")" || true
        done
        ls "$root/report" 2>/dev/null || true
        exit 0
        ;;
    start) ;;
    *)
        echo "usage: launch-live.sh start small|large [bytes] | stop | status" >&2
        exit 2
        ;;
esac

stop_all
rm -rf "$root"
mkdir -p "$root/config/clay" "$root/data" "$root/home/.clay" "$root/home/.config" \
    "$root/workspace" "$root/tmp" "$root/report"
chmod 700 "$root" "$root/config" "$root/config/clay" "$root/data" "$root/home" \
    "$root/home/.config" "$root/home/.clay" "$root/workspace" "$root/tmp" "$root/report"
ln -s "$HOME/.rustup" "$root/home/.rustup"
ln -s "$HOME/.cargo" "$root/home/.cargo"

cp "$here/init-lsp.js" "$root/home/.clay/init.js"

python3 - "$root/workspace" "$size" "$mode" <<'PY'
import os
import sys

workspace, large_bytes, mode = sys.argv[1], int(sys.argv[2]), sys.argv[3]
header = '''pub struct ProbeTarget {
    pub value: usize,
}

impl ProbeTarget {
    pub fn scaled(&self, factor: usize) -> usize {
        self.value * factor
    }
}

pub fn probe_helper(value: usize) -> usize {
    let target = ProbeTarget { value };
    target.scaled(2) + value
}
'''


def write_rust(path, target_bytes):
    line = "pub fn helper_{index:06}(value: usize) -> usize {{ value + {index} }}\n"
    with open(path, "w", encoding="utf-8") as handle:
        handle.write(header)
        written = len(header)
        index = 0
        while True:
            text = line.format(index=index)
            if written + len(text) > target_bytes:
                break
            handle.write(text)
            written += len(text)
            index += 1
        handle.write("\npub fn tail_probe() -> usize { 0 }\n")


# A real Cargo crate so rust-analyzer loads a project instead of a loose file.
os.makedirs(f"{workspace}/src", exist_ok=True)
with open(f"{workspace}/Cargo.toml", "w", encoding="utf-8") as handle:
    handle.write('[package]\nname = "clay-live-probe"\nversion = "0.1.0"\nedition = "2024"\n')
# Tiny fixture crate: same shape as tests/fixtures/lsp/rust/src/main.rs, plus
# an in-file item that only rust-analyzer can complete (probe_helper).
main = '''mod helper_items;

use helper_items::probe_helper;

fn answer() -> u32 {
    42
}

// rust-analyzer-only diagnostic: type mismatch (no syntax layer reports this).
fn broken() -> u32 {
    let value: u32 = "text";
    value
}

fn main() {
    let value = answer();
    let other = probe_helper(value as usize);
    println!("{value} {other}");
}
'''
with open(f"{workspace}/src/main.rs", "w", encoding="utf-8") as handle:
    handle.write(main)
with open(f"{workspace}/src/helper_items.rs", "w", encoding="utf-8") as handle:
    handle.write(
        "pub struct ProbeTarget {\n    pub value: usize,\n}\n\n"
        "impl ProbeTarget {\n    pub fn scaled(&self, factor: usize) -> usize {\n        self.value * factor\n    }\n}\n\n"
        "pub fn probe_helper(value: usize) -> usize {\n    let target = ProbeTarget { value };\n    target.scaled(2) + value\n}\n"
    )

if mode == "mid":
    with open(f"{workspace}/src/main.rs", "a", encoding="utf-8") as handle:
        handle.write("mod probe;\n")
    write_rust(f"{workspace}/src/probe.rs", 250 * 1024)
write_rust(f"{workspace}/review.rs", large_bytes)
PY

open_file="src/main.rs"
[[ "$mode" == "mid" ]] && open_file="src/probe.rs"
[[ "$mode" == "large" ]] && open_file="review.rs"

python3 - "$root/config/clay/layout.json" "$root/workspace" "$open_file" <<'PY'
import json
import sys

layout_path, workspace, open_file = sys.argv[1:]
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
                    "panes": {"1": open_file},
                }
            ],
        },
        output,
    )
PY

(
    cd "$root/workspace"
    exec env HOME="$root/home" XDG_CONFIG_HOME="$root/config" XDG_DATA_HOME="$root/data" \
        TMPDIR="$root/tmp" CLAY_PERF_PROFILE=1 CLAY_PERF_REPORT_DIR="$root/report" \
        "$repo/target/debug/clay" server "$root/clay.sock"
) > "$root/server.log" 2>&1 &
echo $! > "$root/server.pid"

for _ in $(seq 1 300); do
    [[ -S "$root/clay.sock" ]] && break
    kill -0 "$(cat "$root/server.pid")" 2>/dev/null || { echo "server exited" >&2; exit 1; }
    sleep 0.1
done
[[ -S "$root/clay.sock" ]] || { echo "no socket" >&2; exit 1; }

(
    cd "$repo"
    exec env HOME="$root/home" XDG_CONFIG_HOME="$root/config" XDG_DATA_HOME="$root/data" \
        TMPDIR="$root/tmp" "$repo/target/debug/clay" client "$root/clay.sock"
) > "$root/client.log" 2>&1 &
echo $! > "$root/client.pid"

sleep 2
echo "live root=$root mode=$mode file=$open_file server=$(cat "$root/server.pid") client=$(cat "$root/client.pid")"