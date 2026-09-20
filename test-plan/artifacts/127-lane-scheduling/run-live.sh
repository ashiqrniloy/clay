#!/usr/bin/env bash
# Plan 127 lane-scheduling manual steps: isolated live launches.
#
#   run-live.sh start fixture            # plan 127 fixture package (grant-gate check)
#   run-live.sh start completion         # bundled @clay/rust: typing + completion
#   run-live.sh start markdown [bytes]   # bundled @clay/markdown: big document, parse handler
#   run-live.sh stop
#   run-live.sh status                   # store contents + fixture adoption state
#
# Isolation mirrors scripts/capture-ui-review.sh and the plan 126 harness: a
# private mode-700 root for HOME/XDG/TMPDIR, a private server socket, a fixture
# workspace, and teardown that kills the whole client/server process tree.
#
# Fixture installation note (`fixture` mode): Clay v1 `clay install` accepts only
# `npm:` specs (PackageSpec::parse rejects every other source family), so the
# store is seeded with the same command shape that backend runs — `pnpm add
# <path>` in `~/.clay/packages` — and the package is then taken through Clay's
# own authority verbs (`clay package inspect`/`adopt`/`enable`). The
# `loadPackage` line mirrors the self-removing block `clay install` appends.
set -Eeuo pipefail
umask 077

here=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
repo=$(cd "$here/../../.." && pwd)
root=${CLAY_LIVE_ROOT:-/tmp/clay-plan127-live}
mode=${2:-completion}
size=${3:-1048576}
store="$root/home/.clay/packages"
# The host's corepack cache is shared so the isolated HOME never fetches pnpm.
host_corepack="${COREPACK_HOME:-$HOME/.cache/node/corepack}"

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
    for name in client server; do
        if [[ -f "$root/$name.pid" ]]; then
            kill_tree "$(cat "$root/$name.pid")"
            rm -f "$root/$name.pid"
        fi
    done
}

isolated() {
    env HOME="$root/home" XDG_CONFIG_HOME="$root/config" XDG_DATA_HOME="$root/data" \
        TMPDIR="$root/tmp" PATH="$root/bin:$PATH" COREPACK_HOME="$host_corepack" \
        COREPACK_ENABLE_DOWNLOAD_PROMPT=0 "$@"
}

case "${1:-}" in
    stop)
        stop_all
        echo "stopped ($root)"
        exit 0
        ;;
    status)
        isolated bash -c "cd '$store' && corepack pnpm list --json" 2>/dev/null \
            | python3 "$here/store-list.py" || true
        for name in "@fixture/lane" "@fixture/laneblocked"; do
            isolated "$repo/target/debug/clay" package inspect "$name" 2>&1 \
                | grep -E "^(Package|Status|Adoption)" || true
        done
        exit 0
        ;;
    start) ;;
    *)
        echo "usage: run-live.sh start fixture|completion|markdown [bytes] | stop | status" >&2
        exit 2
        ;;
esac

# The store survives a restart so the fixture install/adopt evidence is kept.
if [[ "$mode" == "fixture" ]]; then
    : "${CLAY_LIVE_KEEP_ROOT:=}"
else
    stop_all
    rm -rf "$root"
fi
mkdir -p "$root/bin" "$root/config/clay" "$root/data" "$root/home/.clay" \
    "$root/home/.config" "$root/workspace" "$root/tmp" "$store" "$root/perf"
chmod 700 "$root" "$root/bin" "$root/config" "$root/config/clay" "$root/data" \
    "$root/home" "$root/home/.config" "$root/home/.clay" "$root/workspace" \
    "$root/tmp" "$store"

# `corepack pnpm` is the offline pnpm on this host; Clay's backend spawns `pnpm`.
cat > "$root/bin/pnpm" <<'SH'
#!/bin/sh
exec corepack pnpm "$@"
SH
chmod 700 "$root/bin/pnpm"

document=demo.lane
case "$mode" in
    fixture)
        fixture="$here/fixture-package"
        isolated bash -c "cd '$store' && corepack pnpm add '$fixture' --ignore-scripts" >/dev/null
        isolated "$repo/target/debug/clay" package adopt "@fixture/lane" >/dev/null 2>&1 || true
        # `enable` is expected to fail closed here: no user-facing capability
        # grant surface exists for parse-document/completion-provider on a
        # non-bundled package (recorded in the artifact README).
        isolated "$repo/target/debug/clay" package enable "@fixture/lane" \
            > "$root/enable.log" 2>&1 || true
        cp "$here/init-fixture.js" "$root/home/.clay/init.js"
        cat > "$root/workspace/$document" <<'EOF'
lane fixture document
hello
EOF
        ;;
    completion)
        cp "$repo/tests/fixtures/configuration/ui-review-completion/init.js" \
            "$root/home/.clay/init.js"
        python3 - "$root/workspace/review.rs" 65536 <<'PY'
import sys

path, target = sys.argv[1], int(sys.argv[2])
line = "pub fn helper_{index:06}() -> usize {{ {index} }}\n"
with open(path, "w", encoding="utf-8") as handle:
    written, index = 0, 0
    while written < target:
        chunk = "".join(line.format(index=index + offset) for offset in range(1000))
        handle.write(chunk)
        written += len(chunk)
        index += 1000
    handle.write("\npub fn hello")
PY
        document=review.rs
        ;;
    markdown)
        cp "$here/init-markdown.js" "$root/home/.clay/init.js"
        python3 - "$root/workspace/notes.md" "$size" <<'PY'
import sys

path, target = sys.argv[1], int(sys.argv[2])
with open(path, "w", encoding="utf-8") as handle:
    written, index = 0, 0
    while written < target:
        chunk = "".join(
            "# heading {index}\n\n- item {index}\n- [link](https://example.com/{index})\n\n".format(
                index=index + offset
            )
            for offset in range(200)
        )
        handle.write(chunk)
        written += len(chunk)
        index += 200
    handle.write("\nplain tail line\n")
print(f"notes.md bytes={written}")
PY
        document=notes.md
        ;;
    *)
        echo "unknown mode $mode" >&2
        exit 2
        ;;
esac

python3 - "$root/config/clay/layout.json" "$root/workspace" "$document" <<'PY'
import json, sys

layout_path, workspace, document = sys.argv[1:]
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
                    "panes": {"1": document},
                }
            ],
        },
        output,
    )
PY

(
    cd "$root/workspace"
    exec env HOME="$root/home" XDG_CONFIG_HOME="$root/config" XDG_DATA_HOME="$root/data" \
        TMPDIR="$root/tmp" PATH="$root/bin:$PATH" COREPACK_HOME="$host_corepack" \
        CLAY_PERF_REPORT_DIR="$root/perf" CLAY_PERF_PROFILE="${CLAY_LIVE_PERF:-1}" \
        "$repo/target/debug/clay" server "$root/clay.sock"
) > "$root/server.log" 2>&1 &
echo $! > "$root/server.pid"

for _ in $(seq 1 300); do
    [[ -S "$root/clay.sock" ]] && break
    kill -0 "$(cat "$root/server.pid")" 2>/dev/null || {
        echo "server exited" >&2
        tail -20 "$root/server.log" >&2
        exit 1
    }
    sleep 0.1
done
[[ -S "$root/clay.sock" ]] || { echo "no socket" >&2; exit 1; }

(
    cd "$repo"
    exec env HOME="$root/home" XDG_CONFIG_HOME="$root/config" XDG_DATA_HOME="$root/data" \
        TMPDIR="$root/tmp" PATH="$root/bin:$PATH" COREPACK_HOME="$host_corepack" \
        "$repo/target/debug/clay" client "$root/clay.sock"
) > "$root/client.log" 2>&1 &
echo $! > "$root/client.pid"

echo "live root=$root mode=$mode document=$document server=$(cat "$root/server.pid") client=$(cat "$root/client.pid")"