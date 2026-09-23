# Plan 133 task 7 — split `src/shell/theme.rs` into parse/resolve/validate

Pure move refactor, no behavior change. Baseline HEAD `40022f6` ("WIP 133"),
clean tree.

## Result

| file | lines |
|---|---|
| `src/shell/theme.rs` (before) | 2,938 |
| `src/shell/theme.rs` (hub after) | 26 |
| `src/shell/theme/parse.rs` (new) | 739 |
| `src/shell/theme/resolve.rs` (new) | 599 |
| `src/shell/theme/validate.rs` (new) | 249 |
| `src/shell/theme/tests.rs` (new) | 1,167 |
| `src/shell/theme/theme_snapshot_tests.rs` (new) | 203 |
| total after | 2,983 |

Every file is under the plan's `< ~1,600` line target; the largest is the moved
`tests.rs`. The 45 added lines are module docs, `use` lines, module
declarations, the hub's three `pub use …::*` re-exports, and two `#[cfg(test)]
mod …;` declarations.

## Ownership

- `theme.rs` — re-export facade (`mod parse/resolve/validate`, `pub use` each,
  `#[cfg(test)] mod tests; mod theme_snapshot_tests;`), so every existing
  `crate::shell::theme::X` path (and `super::theme::{…}` in `components.rs`,
  `package_ui.rs`, `design_system.rs`) keeps working with **zero call-site
  edits**.
- `theme/parse.rs` — token/level enums (`ThemeTokenType`, `ResolvedThemeValue`,
  `ElevationLevel`, `ZLevel`, `DensityLevel`, `MotionDuration`), dimension
  bounds, panel/sidebar geometry constants, `PackageThemeToken` /
  `ResolvedThemeToken` / `ThemeTokenResolver`, the `CoreThemeValue` catalog
  (`core_theme_value`, `core_token_type`, `core_fallback_matches_type`), and
  `CORE_TOKEN_NAMES`.
- `theme/validate.rs` — `DesignTokenError`,
  `validate_design_token_override`, and the contrast floors
  (`TEXT_CONTRAST_MIN`, `UI_CONTRAST_MIN`, `HAIRLINE_VISIBILITY_MIN`,
  `ContrastFailure`, `REQUIRED_CONTRAST_PAIRS`, `REQUIRED_FILL_PAIRS`,
  `theme_meets_contrast`, `validate_active_theme_contrast`) — moved verbatim,
  floors included.
- `theme/resolve.rs` — `SduiThemeStyle` (+ `resolve_*` helpers), `PanelDefaults`,
  `ResolvedUiTheme`, `ThemeTokenValueDto`, `color_to_css`,
  `ResolvedThemeValue::into_dto`, `resolve_theme_token_snapshot`,
  `density_spacing_scale`.
- `theme/tests.rs`, `theme/theme_snapshot_tests.rs` — the two inline test
  modules moved to sibling files with their names and module paths unchanged
  (`shell::theme::tests::…`, `shell::theme::theme_snapshot_tests::…`); 22 + 7 =
  29 tests, identical count to the baseline.

Split by concern, not by accessibility: `resolve.rs` and `validate.rs` cross
reference through the hub re-exports (`use super::*;`), the same pattern
`src/protocol/*` and `src/packages/record/*` use.

## Visibility plumbing (the only production edits)

Six lines gained `pub(crate)` so the split keeps the access the single module
had; no item was added, removed, or renamed:

- `CoreThemeValue` + its `token_type`/`value` fields and `core_theme_value`
  (now `pub(crate)` in `parse.rs`);
- `resolve_f64` and `ResolvedUiTheme::resolved` (called by the moved tests).

`DesignTokenError`, `validate_design_token_override`, `theme_meets_contrast`,
and the level resolvers were already `pub(crate)`; `ContrastFailure` and
`validate_active_theme_contrast` stay `pub` and are re-exported from the hub,
so `src/editor/theme.rs`'s `pub use` and `src/server/ops/theme.rs` are
untouched.

## Method and verbatim proof

- Extraction script (provenance): `task7-split-theme.py`. It slices top-level
  item ranges, writes the six files, and refuses unless every production line
  reappears exactly once (stripped-line multiset) with only the six documented
  `pub(crate)` substitutions.
- Independent verification: `task7-verify.py` (stripped, whitespace/comma-free
  order-preserving content check against `git show HEAD:src/shell/theme.rs` for
  each destination) — `verbatim: OK`; its added-line inventory contains only
  module docs, `use` lines, `mod`/`pub use` declarations, the six edited
  signatures, and two rustfmt reflows (`expected_muted` condition, one
  `resolve_theme_token_snapshot` call that now fits on one line).
- `cargo fmt --all` was the only reflow step; test module bodies were written
  with the original indentation and let rustfmt dedent (no manual whitespace
  editing).

## Forced guard updates (source paths, not behavior)

- `tests/package_ui_conformance.rs::core_token_catalog_matches_tokens_md` now
  reads `src/shell/theme/parse.rs` for `core_theme_value`.
- `tests/primitives_docs.rs` (`phase20_1_token_catalog_…`,
  `no_component_kind_or_token_renamed`) reads `src/shell/theme/parse.rs`.
- `tests/rust_visibility_api_mapping.rs` pins `HAIRLINE_VISIBILITY_MIN` /
  `REQUIRED_FILL_PAIRS` in `src/shell/theme/validate.rs`.
- The `src/shell/theme.rs` path itself still exists (hub), so docs/wiki
  references need no path fix in this task.

## Gates (2026-09-21)

`scripts/check.sh full` — audit, fmt, check, clippy (`-D warnings`, zero
warnings), test, and bench-compile all **PASS**; root test totals **1944 passed
/ 0 failed / 1 ignored**, identical to the task-6 run. Full log and per-stage
codes: `task7-check-full.log`, `task7-exit-codes.txt`.

**Environment limitation (not a change regression):** the gate aborts at
`desktop-clippy` because this host has no WebKitGTK dev packages
(`javascriptcoregtk-4.1` / `webkit2gtk-4.1` are absent, and there is no
passwordless sudo to install them); `pkg-config --exists javascriptcoregtk-4.1`
fails at HEAD too. Supplementary desktop verification with stubbed `.pc`
files and a stub `frontend/dist/index.html` (type-check only, no link/run)
passed:

```
cargo check  -p clay-desktop --all-targets            # Finished
cargo clippy -p clay-desktop --all-targets -- -D warnings  # Finished, zero warnings
```

That covers `src-tauri/src/bridge/dto.rs` and the desktop test targets, which
consume `clay::shell::theme::{ThemeTokenValueDto, density_spacing_scale,
resolve_theme_token_snapshot}` — all preserved by the hub re-exports. The
`bindings` stage (`cargo test -p clay-desktop … export_webview_contract_bindings`)
needs a real link, so it could not run here; the DTO has the same type name and
ts-rs export, and the freshness check is part of the next green full gate on a
host with WebKitGTK.

Targeted checks on this host: `cargo test --lib shell::theme` 29/29 pass (same
names), `cargo test --test presentation` (theme packages + package UI
conformance, 62 pass), `cargo test --test protocol primitives_docs` (35 pass),
`cargo test --test security rust_visibility` (6 pass).

## Security

Contrast-floor logic and the `#[cfg(test)]` floor assertions moved verbatim
(`HAIRLINE_VISIBILITY_MIN` 1.2, `REQUIRED_FILL_PAIRS`, `theme_meets_contrast`,
`validate_active_theme_contrast` in `validate.rs`); the core-catalog contrast
test (`core_catalog_meets_every_required_contrast_pair`) is unchanged and green.
No wire format, no allowed set, and no validation branch changed.
