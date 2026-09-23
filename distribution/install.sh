#!/usr/bin/env sh
# Clay curl installer (the "curl" install channel, plan 115 task 8).
#
# Installs the native `clay` binary into --bindir (default ~/.local/bin)
# after verifying its sha256 digest, then writes the channel.json marker
# that `clay update` (self) consumes (src/packages/self_update.rs):
#   {"version":1,"channel":"curl","argv":[...]}
# The marker is owner-only (0600) next to the installed binary.
#
# Local/fixture mode (used by scripts/package-smoke.sh):
#   sh distribution/install.sh --version 0.1.0 --artifact-dir dist/
# Remote mode (docs-only until artifacts are hosted):
#   curl -fsSL https://clay.dev/install.sh | sh -s -- --version 0.1.0
set -eu

version=""
artifact_dir=""
bindir="${CLAY_INSTALL_BINDIR:-$HOME/.local/bin}"
base_url="https://clay.dev/dist"

usage() {
    echo "usage: install.sh --version <v> [--artifact-dir <dir>] [--bindir <dir>] [--base-url <url>]" >&2
    exit 2
}

while [ $# -gt 0 ]; do
    case "$1" in
        --version) version="${2:?}"; shift 2 ;;
        --artifact-dir) artifact_dir="${2:?}"; shift 2 ;;
        --bindir) bindir="${2:?}"; shift 2 ;;
        --base-url) base_url="${2:?}"; shift 2 ;;
        --help) usage ;;
        *) echo "install.sh: unknown argument: $1" >&2; usage ;;
    esac
done

[ -n "$version" ] || { echo "install.sh: --version is required" >&2; usage; }

case "$(uname -s)-$(uname -m)" in
    Linux-x86_64) target="linux-x64" ;;
    Linux-aarch64 | Linux-arm64) target="linux-arm64" ;;
    *) echo "install.sh: unsupported platform $(uname -s)-$(uname -m); use the npm wrapper or build from source" >&2; exit 1 ;;
esac

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

artifact="clay-$version-$target"

if [ -n "$artifact_dir" ]; then
    src="$artifact_dir/$artifact"
    [ -f "$src" ] || { echo "install.sh: missing artifact $src" >&2; exit 1; }
    cp "$src" "$work/clay"
    cp "$src.sha256" "$work/clay.sha256"
else
    command -v curl >/dev/null 2>&1 || { echo "install.sh: curl not found" >&2; exit 1; }
    curl -fsSL "$base_url/$artifact" -o "$work/clay"
    curl -fsSL "$base_url/$artifact.sha256" -o "$work/clay.sha256"
fi

# Digest verification happens BEFORE anything is installed or executed.
cd "$work"
expected="$(cut -d' ' -f1 clay.sha256)"
actual="$(sha256sum clay | cut -d' ' -f1)"
if [ "$expected" != "$actual" ]; then
    echo "install.sh: digest mismatch for $artifact (expected $expected, got $actual)" >&2
    exit 1
fi

mkdir -p "$bindir"
install -m 0755 "$work/clay" "$bindir/clay"

# Record the channel so `clay update` (self) can re-run this exact install.
if [ -n "$artifact_dir" ]; then
    install_dir_abs="$(cd "$artifact_dir" && pwd)"
    self_abs="$0"
    argv="[\"sh\",\"$self_abs\",\"--version\",\"$version\",\"--bindir\",\"$bindir\",\"--artifact-dir\",\"$install_dir_abs\"]"
else
    argv="[\"sh\",\"-c\",\"curl -fsSL $base_url/install.sh | sh -s -- --version $version --bindir $bindir\"]"
fi

marker="$bindir/channel.json"
if command -v python3 >/dev/null 2>&1; then
    CHANNEL_VERSION=1 CHANNEL_KIND=curl CHANNEL_ARGV="$argv" MARKER="$marker" python3 - <<'PY'
import json, os
argv = json.loads(os.environ["CHANNEL_ARGV"])
marker = {
    "version": int(os.environ["CHANNEL_VERSION"]),
    "channel": os.environ["CHANNEL_KIND"],
    "argv": argv,
}
with open(os.environ["MARKER"], "w") as fh:
    json.dump(marker, fh)
    fh.write("\n")
PY
else
    printf '{"version":1,"channel":"curl","argv":%s}\n' "$argv" > "$marker"
fi
chmod 0600 "$marker"

echo "Installed clay $version to $bindir/clay"
echo "Channel marker: $marker (self-update: clay update)"
echo "Clay packages run with full system access - review before installing."
echo "clay install never executes package code; clay package adopt is the reviewable gate."
