#!/usr/bin/env bash
# Plan 099: real-device editor performance matrix.
#
# Builds an instrumented frontend (VITE_CLAY_PERF_PROFILE=1) and desktop,
# generates synthetic fixtures under an approved temp root, starts a Clay
# server + Tauri/WebKit desktop with per-process performance recorders, and
# collects source-free perf reports when the window closes:
#
#   <report>/frontend-frontend-perf-snapshot.json   browser/CodeMirror stages
#   <report>/clay-desktop-perf-summary.json          bridge/client stages
#   <report>/clay-server-perf-summary.json           server/syntax stages
#   <report>/summary.json                            merged verdict + p95 table
#
# Automated invariants (always enforced):
#   - bounded retention: frontend retained events <= 4096
#   - stage presence: open/ready/patch delivery stages appear (matrix ran)
# Timing invariants (only with --enforce; approved targets block after three
# stable designated-device runs — see docs/development/performance.md):
#   - zero long tasks > 50 ms during the captured traces
#
# The open/type/scroll/fold/save/reload flows themselves are operator-driven
# (keyboard automation is unavailable on unprivileged hosts); the checklist is
# printed while the desktop window is open. CI-blocking deterministic
# counterparts live in tests/editor_performance.rs and
# frontend/src/editor/extensions/performance.test.ts.
set -Eeuo pipefail
umask 077

repo="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo"

sizes="1,10"
kinds="mixed-unicode,many-short-lines,long-lines,newline-heavy"
label=""
enforce=0
report_root="target/perf/editor-performance"

usage() {
    cat <<'EOF'
Usage: scripts/editor-performance-smoke.sh [--sizes <MiB list>] [--kinds <list>]
                                           [--label <name>] [--enforce] [--keep]

  --sizes   comma list of fixture sizes in MiB (default: 1,10; try 1,10,50)
  --kinds   comma list of perf fixture kinds
            (mixed-unicode, many-short-lines, long-lines, newline-heavy)
  --label   report directory name (default: <timestamp>)
  --enforce fail the run on the approved long-task/retention targets
  --keep    keep the generated workspace (default: removed on exit)
EOF
}

while (($#)); do
    case "$1" in
        --sizes) sizes=$2; shift 2 ;;
        --kinds) kinds=$2; shift 2 ;;
        --label) label=$2; shift 2 ;;
        --enforce) enforce=1; shift ;;
        --keep) keep_workspace=1; shift ;;
        -h|--help) usage; exit 0 ;;
        *) echo "unknown argument: $1" >&2; usage >&2; exit 2 ;;
    esac
done

command -v python3 >/dev/null || { echo "python3 is required for report analysis" >&2; exit 2; }

echo "== building instrumented frontend (VITE_CLAY_PERF_PROFILE=1)"
( cd frontend && VITE_CLAY_PERF_PROFILE=1 npm run build >/dev/null )

echo "== building clay binaries"
cargo build --bin clay --quiet
cargo build -p clay-desktop --bin clay-desktop --quiet

bin="$repo/target/debug/clay"
run_id="${label:-$(date +%Y%m%d-%H%M%S)}"
report="$repo/$report_root/$run_id"
workspace="$(mktemp -d "${TMPDIR:-/tmp}/clay-editor-perf.XXXXXX")"
socket="$workspace/perf.sock"
mkdir -p "$report"

keep_workspace=${keep_workspace:-0}
server_pid=""
desktop_pid=""
cleanup() {
    local status=$?
    trap - EXIT INT TERM
    if [ -n "$desktop_pid" ]; then
        pkill -TERM -P "$desktop_pid" 2>/dev/null || true
        kill "$desktop_pid" 2>/dev/null || true
    fi
    if [ -n "$server_pid" ] && kill -0 "$server_pid" 2>/dev/null; then
        # Graceful stop so the server dumps its perf report.
        kill -TERM "$server_pid" 2>/dev/null || true
        for _ in $(seq 1 30); do
            kill -0 "$server_pid" 2>/dev/null || break
            sleep 0.1
        done
    fi
    [ "$keep_workspace" = 1 ] || rm -rf "$workspace"
    exit "$status"
}
trap cleanup EXIT
trap 'exit 130' INT TERM

echo "== generating fixtures in $workspace (approved temp root)"
IFS=',' read -r -a size_list <<<"$sizes"
IFS=',' read -r -a kind_list <<<"$kinds"
for size in "${size_list[@]}"; do
    [[ "$size" =~ ^[1-9][0-9]*$ ]] || { echo "invalid size: $size" >&2; exit 2; }
    for kind in "${kind_list[@]}"; do
        case "$kind" in
            mixed-unicode|many-short-lines|long-lines|newline-heavy) ;;
            *) echo "invalid kind: $kind" >&2; exit 2 ;;
        esac
        base="$workspace/perf-${size}mib-${kind}"
        generated="$repo/target/perf-fixtures/perf-${size}mib-${kind}.txt"
        # The generator only writes under approved roots; copy from there.
        "$bin" perf-fixture --kind "$kind" --size-mib "$size" --seed 9001 \
            --output "$generated" >/dev/null
        cp "$generated" "$base.txt"
        # Language variants: classification is path-driven, so the same
        # generated shape exercises every first-party mode.
        for ext in md rs ts tsx js; do
            cp "$generated" "$base.$ext"
        done
    done
done

echo "== starting server (profiled, report dir: $report)"
(
    cd "$workspace" && CLAY_PERF_PROFILE=1 CLAY_PERF_REPORT_DIR="$report" \
        exec "$bin" server "$socket" --config-fixture clay-performance-matrix
) &
server_pid=$!
for _ in $(seq 1 100); do
    [ -S "$socket" ] && break
    kill -0 "$server_pid" 2>/dev/null || { echo "server failed to start" >&2; exit 1; }
    sleep 0.1
done
[ -S "$socket" ] || { echo "server socket did not appear" >&2; exit 1; }

echo "== launching profiled desktop (close its window to finish the run)"
CLAY_PERF_PROFILE=1 CLAY_PERF_REPORT_DIR="$report" \
    "$bin" client "$socket" &
desktop_pid=$!

cat <<CHECKLIST

Manual matrix checklist (Plan 099) — fixtures live in $workspace:
  1. Open each perf-*.md / .rs / .ts / .tsx / .js / .txt fixture (Ctrl+B file
     browser). Text must paint immediately and complete without a blank wait.
  2. Type a burst at the top; scroll end-to-end; toggle a fold (Ctrl+Shift+F).
  3. Save (Ctrl+S) and reload each document; no diagnostics expected.
  4. Split panes (Ctrl+\\ / Ctrl+-) to four panes on a small fixture and
     repeat a short type/scroll pass.
  5. Close the desktop window: reports land in $report and this script
     prints the invariant verdict and p95 table.

CHECKLIST

set +e
wait "$desktop_pid"
desktop_status=$?
set -e
desktop_pid=""

# Graceful server stop dumps the server-side summary.
kill -TERM "$server_pid" 2>/dev/null || true
for _ in $(seq 1 30); do
    kill -0 "$server_pid" 2>/dev/null || break
    sleep 0.1
done
server_pid=""

echo "== analyzing reports in $report"
analysis="$report/analyze.py"
cat >"$analysis" <<'PYEOF'
import json, pathlib, sys

report = pathlib.Path(sys.argv[1])
enforce = sys.argv[2] == "--enforce"

def load(name):
    path = report / name
    if not path.is_file():
        return None
    return json.loads(path.read_text())

frontend = load("frontend-frontend-perf-snapshot.json")
desktop = load("clay-desktop-perf-summary.json")
server = load("clay-server-perf-summary.json")

failures = []
warnings = []

if frontend is None:
    failures.append("frontend snapshot missing (window close did not fire the report)")
else:
    retained = frontend.get("retainedEvents", 0)
    if retained > 4096:
        failures.append(f"frontend retention unbounded: {retained} > 4096")
    stages = {e.get("stage") for e in frontend.get("events", [])}
    for required in ("editor.open", "editor.ready", "bridge.patch_delivery"):
        if required not in stages:
            warnings.append(f"stage {required} absent: was the matrix checklist driven?")

def p95_of(metric):
    # Frontend summaries report milliseconds; Rust summaries report nanos.
    if metric is None:
        return None
    if "p95Ms" in metric:
        return metric["p95Ms"]
    if "p95Nanos" in metric:
        return round(metric["p95Nanos"] / 1_000_000, 3)
    return None

table = {}
for label, doc in (("frontend", frontend), ("desktop", desktop), ("server", server)):
    if doc:
        for stage, metric in doc.get("metrics", {}).items():
            value = p95_of(metric)
            if value is not None:
                table[f"{label}:{stage}"] = value

long_tasks = []
if frontend:
    for event in frontend.get("events", []):
        if event.get("stage") == "editor.long_task":
            duration = event.get("durationMs") or 0
            if duration > 50:
                long_tasks.append(duration)
if long_tasks:
    message = f"long tasks > 50ms in trace: {sorted(long_tasks, reverse=True)}"
    (failures if enforce else warnings).append(message)

summary = {
    "verdict": "fail" if failures else "pass",
    "enforced": enforce,
    "longTasksOver50ms": sorted(long_tasks, reverse=True),
    "warnings": warnings,
    "failures": failures,
    "p95Ms": dict(sorted(table.items())),
}
(report / "summary.json").write_text(json.dumps(summary, indent=2) + "\n")

print(f"verdict: {summary['verdict']} (enforce={enforce})")
for warning in warnings:
    print(f"warning: {warning}")
for failure in failures:
    print(f"failure: {failure}")
print("p95 table (ms):")
for stage, value in summary["p95Ms"].items():
    print(f"  {stage}: {value}")
sys.exit(1 if failures else 0)
PYEOF

set +e
python3 "$analysis" "$report" "$( [ "$enforce" = 1 ] && echo --enforce || echo --record )"
analysis_status=$?
set -e
exit "$analysis_status"
