#!/usr/bin/env python3
"""Plan 133 task 7 verbatim proof: original `src/shell/theme.rs` vs split tree.

Two checks, both order-preserving:

1. every stripped production line of the original file reappears in
   `theme.rs` + `theme/{parse,resolve,validate}.rs`, in the same order, with the
   six documented visibility edits substituted;
2. every stripped line of the two inline test modules reappears in
   `theme/tests.rs` / `theme/theme_snapshot_tests.rs` (rustfmt reindents, so
   indentation is ignored).

Everything else in the new tree is then printed as the added-line inventory.
"""

import re
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
original = subprocess.run(
    ["git", "show", "HEAD:src/shell/theme.rs"],
    cwd=ROOT,
    capture_output=True,
    text=True,
    check=True,
).stdout.split("\n")


def stripped(lines: list[str]) -> list[str]:
    return [line.strip() for line in lines if line.strip()]


def original_slice(a: int, b: int) -> list[str]:
    return stripped(original[a - 1 : b])


def new_file(path: str) -> list[str]:
    return stripped((ROOT / path).read_text().split("\n"))


EDITS = {
    "struct CoreThemeValue {": "pub(crate) struct CoreThemeValue {",
    "fn core_theme_value(token: &str) -> Option<CoreThemeValue> {": "pub(crate) fn core_theme_value(token: &str) -> Option<CoreThemeValue> {",
    "token_type: ThemeTokenType,": "pub(crate) token_type: ThemeTokenType,",
    "value: ResolvedThemeValue,": "pub(crate) value: ResolvedThemeValue,",
    "fn resolve_f64(resolver: &ThemeTokenResolver, token: &str, token_type: ThemeTokenType) -> f64 {": "pub(crate) fn resolve_f64(resolver: &ThemeTokenResolver, token: &str, token_type: ThemeTokenType) -> f64 {",
    "fn resolved(&self, token: &str) -> Option<ResolvedThemeValue> {": "pub(crate) fn resolved(&self, token: &str) -> Option<ResolvedThemeValue> {",
}


def subsequence_report(name: str, original_lines: list[str], target: list[str]) -> None:
    """Order-preserving content check: rustfmt reindents and may reflow lines
    (adding a trailing comma), so compare whitespace- and comma-free text."""

    def text(lines: list[str]) -> str:
        return re.sub(r"[\s,]", "", "".join(lines))

    wanted = "".join(EDITS.get(line, line) for line in original_lines)
    assert text([wanted]) in text(target), f"{name}: original content not found"


# Per-destination order-preserving checks: each original slice must reappear
# verbatim (module-relative order kept, indentation ignored) in its new file.
CASES: list[tuple[str, list[str], str]] = [
    (
        "parse",
        original_slice(16, 647) + original_slice(2529, 2624),
        "src/shell/theme/parse.rs",
    ),
    (
        "resolve",
        original_slice(649, 811)
        + original_slice(1055, 1353)
        + original_slice(2521, 2527)
        + original_slice(2626, 2734),
        "src/shell/theme/resolve.rs",
    ),
    ("validate", original_slice(813, 1053), "src/shell/theme/validate.rs"),
    ("mod tests", original_slice(1357, 2518), "src/shell/theme/tests.rs"),
    (
        "theme_snapshot_tests",
        original_slice(2738, 2937),
        "src/shell/theme/theme_snapshot_tests.rs",
    ),
]

for name, wanted, target_path in CASES:
    subsequence_report(name, wanted, new_file(target_path))

# Added-line inventory: everything in the new tree that is not in the
# (edited) original multiset.
from collections import Counter

original_all = Counter(
    EDITS.get(line, line)
    for line in stripped(original)
)
new_all = Counter(line for path in (
    "src/shell/theme.rs",
    "src/shell/theme/parse.rs",
    "src/shell/theme/resolve.rs",
    "src/shell/theme/validate.rs",
    "src/shell/theme/tests.rs",
    "src/shell/theme/theme_snapshot_tests.rs",
) for line in new_file(path))
added = new_all - original_all
print("verbatim: OK")
print(f"added lines ({sum(added.values())}):")
for line, count in sorted(added.items()):
    print(f"  {count}x {line}")
