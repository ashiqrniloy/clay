#!/usr/bin/env python3
"""Plan 133 task 7: split src/shell/theme.rs into parse/resolve/validate modules.

Mechanical slice by top-level item ranges. Production ranges are moved
verbatim (the only edits are four added `pub(crate)` keywords so sibling
modules and the moved tests keep access they had inside one file); the two
test modules move verbatim into sibling files with unchanged module names.

Run from anywhere: `python3 test-plan/artifacts/133-file-decomposition/task7-split-theme.py`.
"""

from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
SRC = ROOT / "src/shell/theme.rs"

lines = SRC.read_text().split("\n")
assert len(lines) == 2938 or lines[-1] == "", f"unexpected line count {len(lines)}"


def seg(a: int, b: int) -> str:
    """1-indexed inclusive slice, no trailing newline."""
    return "\n".join(lines[a - 1 : b])


def one(text: str, old: str, new: str) -> str:
    assert text.count(old) == 1, f"expected exactly one {old!r}, got {text.count(old)}"
    return text.replace(old, new)


# --- parse.rs: token/level enums, core catalog, package-token resolver ---
parse_body = seg(16, 647)
parse_body = one(
    parse_body, "struct CoreThemeValue {", "pub(crate) struct CoreThemeValue {"
)
parse_body = one(
    parse_body,
    "fn core_theme_value(token: &str)",
    "pub(crate) fn core_theme_value(token: &str)",
)
parse_body = one(
    parse_body,
    "    token_type: ThemeTokenType,\n    value: ResolvedThemeValue,\n}",
    "    pub(crate) token_type: ThemeTokenType,\n    pub(crate) value: ResolvedThemeValue,\n}",
)
PARSE = f"""//! Typed theme-token parsing: token/level enums, the core token catalog, and
//! the package-token resolver (plan 133 task 7 split of `src/shell/theme.rs`).

use crate::str_enum::string_enum_impl;

use std::collections::BTreeMap;

use crate::color::Color;

use crate::editor::typography::UiTextVariant;

{parse_body}

{seg(2529, 2624)}
"""

# --- resolve.rs: SduiThemeStyle, PanelDefaults, ResolvedUiTheme, snapshot ---
resolve_body = seg(649, 811) + "\n\n" + seg(1055, 1353)
resolve_body = one(resolve_body, "fn resolve_f64(", "pub(crate) fn resolve_f64(")
resolve_body = one(
    resolve_body,
    "    fn resolved(&self, token: &str)",
    "    pub(crate) fn resolved(&self, token: &str)",
)
resolve_tail = seg(2521, 2527) + "\n\n" + seg(2626, 2734)
RESOLVE = f"""//! Resolution and compositing of validated theme tokens into paint/layout
//! values: the SDUI style projection, the cached registry, panel geometry, and
//! the flat token snapshot the React client installs (plan 133 task 7 split of
//! `src/shell/theme.rs`).

use std::collections::BTreeMap;

use crate::color::Color;

use crate::editor::typography::UiTextVariant;
use crate::shell::layout::FixedSlotId;

use super::*;

{resolve_body}

{resolve_tail}
"""

# --- validate.rs: design-token override validation + WCAG contrast floors ---
VALIDATE = f"""//! Validation of active-theme design-token overrides and the WCAG contrast
//! floors every installable theme must clear (plan 133 task 7 split of
//! `src/shell/theme.rs`).

use crate::color::Color;

use super::*;

{seg(813, 1053)}
"""

# --- tests.rs: the former inline `mod tests`, same module name ---
TESTS = f"""//! Unit tests for the shell theme module (plan 133 task 7: moved verbatim
//! from `src/shell/theme.rs`; module path `shell::theme::tests` unchanged).

use crate::color::Color;
use crate::editor::typography::UiTextVariant;

{seg(1357, 2518)}
"""

# --- theme_snapshot_tests.rs: the former inline snapshot suite ---
SNAPSHOT_TESTS = f"""//! Snapshot/DTO/contrast unit tests for the resolved theme token surface
//! (plan 133 task 7: moved verbatim from `src/shell/theme.rs`; module path
//! `shell::theme::theme_snapshot_tests` unchanged).

{seg(2738, 2937)}
"""

# --- hub: pure re-export facade; the catalog lives in parse.rs ---
HUB = """//! Clay-owned typed theme tokens for package UI and SDUI rendering.
//!
//! Package declarations may name semantic, package-prefixed tokens, but they do
//! not provide raw colors, CSS, renderer callbacks, or native style handles.
//! Clay resolves every package token through a same-typed core fallback token
//! before Masonry paint/layout reads cached native values.
//!
//! Split by concern (plan 133 task 7): `parse` owns the token/level enums,
//! the core catalog, and the package-token resolver; `validate` owns
//! design-token override validation and the WCAG contrast floors; `resolve`
//! owns the cached resolved registry, panel geometry, and the flat snapshot.
//! All three are re-exported here, so `crate::shell::theme::X` stays the
//! call-site path.

mod parse;
mod resolve;
mod validate;

pub use parse::*;
pub use resolve::*;
pub use validate::*;

#[cfg(test)]
mod tests;
#[cfg(test)]
mod theme_snapshot_tests;
"""

targets = {
    ROOT / "src/shell/theme/parse.rs": PARSE,
    ROOT / "src/shell/theme/resolve.rs": RESOLVE,
    ROOT / "src/shell/theme/validate.rs": VALIDATE,
    ROOT / "src/shell/theme/tests.rs": TESTS,
    ROOT / "src/shell/theme/theme_snapshot_tests.rs": SNAPSHOT_TESTS,
    SRC: HUB,
}

# Verbatim checks: every original production line (outside the two test
# modules) must reappear as a (stripped) line in exactly one destination,
# except the four lines whose only change is the added `pub(crate)` keyword.
EDITED_LINES = {
    "struct CoreThemeValue {": "pub(crate) struct CoreThemeValue {",
    "fn core_theme_value(token: &str) -> Option<CoreThemeValue> {": "pub(crate) fn core_theme_value(token: &str) -> Option<CoreThemeValue> {",
    "token_type: ThemeTokenType,": "pub(crate) token_type: ThemeTokenType,",
    "value: ResolvedThemeValue,": "pub(crate) value: ResolvedThemeValue,",
    "fn resolve_f64(resolver: &ThemeTokenResolver, token: &str, token_type: ThemeTokenType) -> f64 {": "pub(crate) fn resolve_f64(resolver: &ThemeTokenResolver, token: &str, token_type: ThemeTokenType) -> f64 {",
    "fn resolved(&self, token: &str) -> Option<ResolvedThemeValue> {": "pub(crate) fn resolved(&self, token: &str) -> Option<ResolvedThemeValue> {",
}


def stripped(text: str) -> list[str]:
    return [line.strip() for line in text.split("\n") if line.strip()]


production_original = stripped(seg(1, 1354) + "\n" + seg(2520, 2735))
new_production = stripped(PARSE + "\n" + RESOLVE + "\n" + VALIDATE + "\n" + HUB)
from collections import Counter

for old, new in EDITED_LINES.items():
    assert production_original.count(old) == 1, f"expected one original {old!r}"
    assert new_production.count(new) >= 1, f"expected edited {new!r}"
    production_original.remove(old)

missing = Counter(production_original) - Counter(new_production)
assert not missing, f"production lines missing after split: {missing}"

for path, content in targets.items():
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(content)

print("wrote:")
for path, content in targets.items():
    print(f"  {path.relative_to(ROOT)}: {content.count(chr(10))} lines")
