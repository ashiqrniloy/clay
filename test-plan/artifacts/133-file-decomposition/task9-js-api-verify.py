#!/usr/bin/env python3
"""Plan 133 task 9 (verify-only): no Clay JS API surface changed.

Checks, all against the pre-plan baseline `6ee3b4c`:

1. the JS API artifact paths (`runtime/js`, `docs/reference/clay-js-api`,
   `docs/generated`, `docs/index.md`, `frontend/src/bridge`,
   `src-tauri/bindings`) are byte-identical (committed and working tree);
2. every `deno_core` op wrapper path is byte-identical except the two task-6
   `match_same_arms` merges (`src/server/ops/{modes,packages}.rs`), which are
   printed for review and are structurally identical arms;
3. `UiContributionRule` (the JS-visible error-kind enum) is unchanged;
4. every closed-choice validation message is unchanged: the baseline's literal
   messages are recomposed from the current table (`CHOICE_FIELDS` + `VALID_*`
   + `choice_message`) and the sets must match exactly.
"""

import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
BASELINE = "6ee3b4c"
failures: list[str] = []


def baseline(path: str) -> str:
    return subprocess.run(
        ["git", "show", f"{BASELINE}:{path}"],
        cwd=ROOT,
        capture_output=True,
        text=True,
        check=True,
    ).stdout


def changed(paths: list[str]) -> list[str]:
    committed = subprocess.run(
        ["git", "diff", "--name-only", f"{BASELINE}..HEAD", "--", *paths],
        cwd=ROOT,
        capture_output=True,
        text=True,
        check=True,
    ).stdout.split()
    working = subprocess.run(
        ["git", "status", "--porcelain", "--", *paths],
        cwd=ROOT,
        capture_output=True,
        text=True,
        check=True,
    ).stdout.splitlines()
    return committed + working


JS_API_PATHS = [
    "runtime/js",
    "docs/reference/clay-js-api",
    "docs/generated",
    "docs/index.md",
    "frontend/src/bridge",
    "src-tauri/bindings",
]
touched = changed(JS_API_PATHS)
if touched:
    failures.append(f"JS API artifacts changed: {touched}")
else:
    print(f"OK JS API artifacts unchanged vs {BASELINE}: {', '.join(JS_API_PATHS)}")

ops = changed(["src/server/ops"])
expected_ops = {"src/server/ops/modes.rs", "src/server/ops/packages.rs"}
if set(ops) - expected_ops:
    failures.append(f"unexpected op wrapper changes: {sorted(set(ops) - expected_ops)}")
else:
    print(f"OK op wrappers: only task-6 arm merges changed ({', '.join(sorted(ops))})")

if baseline("src/server/ops/ui.rs") != (ROOT / "src/server/ops/ui.rs").read_text():
    failures.append("src/server/ops/ui.rs changed vs baseline")
else:
    print("OK src/server/ops/ui.rs byte-identical (ui.* op wiring untouched)")


def rule_variants(text: str) -> list[str]:
    body = re.search(r"enum UiContributionRule \{(.*?)\n\}", text, re.S).group(1)
    return [line.strip().rstrip(",") for line in body.splitlines() if line.strip()]


current_ui = (ROOT / "src/server/ui.rs").read_text()
if rule_variants(baseline("src/server/ui.rs")) != rule_variants(current_ui):
    failures.append("UiContributionRule variants changed")
else:
    print(f"OK UiContributionRule unchanged ({len(rule_variants(current_ui))} variants)")


def base_messages(text: str) -> set[str]:
    return set(re.findall(r'"([a-zA-Z][^"]*must be[^"]*)"', text))


baseline_messages = base_messages(baseline("src/server/ui.rs"))

arrays = {
    name: re.findall(r'"([^"]+)"', body)
    for name, body in re.findall(
        r"const (VALID_[A-Z_]+): &\[&str\] =\s*&\[(.*?)\];", current_ui, re.S
    )
}
rows = re.findall(
    r'^\s*\("([^"]+)", (VALID_[A-Z_]+), Rule::\w+, "([^"]*)"\),$', current_ui, re.M
)


def choice_message(allowed: list[str], label: str) -> str:
    if not allowed:
        return label
    head, last = allowed[:-1], allowed[-1]
    if not head:
        return f"{label} {last}"
    if len(head) == 1:
        return f"{label} {head[0]} or {last}"
    return f"{label} {', '.join(head)}, or {last}"


composed = {choice_message(arrays[name], label) for _, name, label in rows}
extra = composed - baseline_messages
# Baseline messages that are not closed-choice rows must still exist verbatim
# in the current file (only the 21 table-driven messages are recomposed).
still_literal = base_messages(current_ui)
unaccounted = baseline_messages - composed - still_literal
if extra or unaccounted:
    failures.append(f"message drift: extra={sorted(extra)} unaccounted={sorted(unaccounted)}")
else:
    print(
        f"OK all {len(composed)} closed-choice messages recompose byte-for-byte "
        f"from CHOICE_FIELDS/VALID_*; the other "
        f"{len(baseline_messages) - len(composed)} baseline diagnostics are unchanged literals"
    )

# The moved Rust tests still assert the full message text.
tests_text = (ROOT / "src/server/ui/tests.rs").read_text()
asserted = base_messages(tests_text)
uncovered = composed - asserted
if uncovered:
    failures.append(f"messages not asserted by the moved tests: {sorted(uncovered)}")
else:
    print(f"OK moved tests assert all {len(composed)} full message strings")

if failures:
    print("\nFAILURES:")
    for failure in failures:
        print(f"- {failure}")
    sys.exit(1)
print("\nall JS API surface checks passed")
