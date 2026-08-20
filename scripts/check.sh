#!/usr/bin/env bash
# Clay supported check wrapper (Linux host).
#   scripts/check.sh quick  — non-release quick feedback: fmt + library unit tests
#   scripts/check.sh full   — serial release gate under one repo-local lock
#   scripts/check.sh report — advisory target-size/executable report
set -eu

repo="$(cd "$(dirname "$0")/.." && pwd)"
cd "$repo"

CURRENT_STAGE="startup"
trap 'if [ "$?" -ne 0 ]; then echo "FAILED at stage: $CURRENT_STAGE" >&2; fi' EXIT

run_stage() {
    CURRENT_STAGE="$1"
    shift
    echo "== [$CURRENT_STAGE] $*"
    "$@"
}

report_size() {
    label="$1"
    path="$2"
    if [ -L "$path" ]; then
        printf '%s: symlink (not traversed)\n' "$label"
    elif [ ! -e "$path" ]; then
        printf '%s: missing\n' "$label"
    else
        size="$(du -sh -- "$path" 2>/dev/null | cut -f1)"
        if [ -n "$size" ]; then
            printf '%s: %s\n' "$label" "$size"
        else
            printf '%s: unavailable\n' "$label"
        fi
    fi
}

report_artifacts() {
    echo "build artifact report (advisory; no cleanup performed)"
    report_size target target
    report_size debug-deps target/debug/deps
    report_size debug-incremental target/debug/incremental
    if [ -d target/debug/deps ] && [ ! -L target/debug/deps ]; then
        executable_count="$(find target/debug/deps -maxdepth 1 -type f -executable -print 2>/dev/null | wc -l | tr -d ' ')"
    else
        executable_count=0
    fi
    printf 'executable files (target/debug/deps): %s\n' "$executable_count"
}

case "${1:-}" in
    quick)
        echo "quick check (non-release): cargo fmt --check && cargo test --lib --quiet"
        run_stage fmt cargo fmt --check
        run_stage lib-test cargo test --lib --quiet
        echo "quick check PASSED"
        ;;
    full)
        if [ -L target ]; then
            echo "refusing: target/ is a symlink; the full-check lock must stay inside the repo" >&2
            exit 1
        fi
        mkdir -p target
        lock="target/.clay-full-check.lock"
        exec 9>"$lock"
        echo "full check: waiting for serial lock $lock (concurrent full runs queue here)"
        flock 9
        echo "full check: acquired $lock"
        run_stage audit cargo audit
        run_stage fmt cargo fmt --check
        run_stage check cargo check --all-targets
        run_stage clippy cargo clippy --all-targets -- -D warnings
        run_stage test cargo test --all-targets --quiet
        run_stage bench-compile cargo bench --no-run
        echo "full check PASSED"
        ;;
    report)
        report_artifacts
        ;;
    *)
        trap - EXIT
        echo "usage: $0 quick|full|report" >&2
        exit 2
        ;;
esac
