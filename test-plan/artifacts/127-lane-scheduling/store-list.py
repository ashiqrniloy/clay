#!/usr/bin/env python3
"""Print the isolated package store's pnpm dependencies (name -> package root)."""
import json
import sys

for entry in json.load(sys.stdin)[0].get("dependencies", {}).values():
    print("{} -> {}".format(entry.get("from") or entry.get("version"), entry.get("path")))
