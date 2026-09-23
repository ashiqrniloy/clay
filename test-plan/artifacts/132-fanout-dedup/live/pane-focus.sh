#!/usr/bin/env bash
# Report which pane's editor holds focus (pane 1 = left half, pane 2 = right).
set -Eeuo pipefail
python3 "$(dirname "$0")/probe.py" dump | python3 -c '
import sys
for line in sys.stdin:
    parts = line.rstrip("\n").split("|")
    if len(parts) > 6 and parts[1] == "entry" and parts[3] == "Document editor":
        x = int(parts[5].split(",")[0])
        pane = 1 if x < 900 else 2
        focused = "focused" in parts[2]
        print("pane %d editor focused=%s" % (pane, str(focused).lower()))
'
