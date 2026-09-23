# Plan 134 task 2 — D4 strict float comparisons replaced (2026-09-22)

Task: "Replace strict float comparisons in production shell/theme/layout code
(D4)". Baseline inventory: `baseline-clippy-float-cmp.txt` (task 1, clippy
0.1.98, `-W clippy::pedantic`).

## Result

`clippy::float_cmp`: **81 unique sites → 0**. Pedantic run exit 0; the only lint
code removed versus the baseline set is `clippy::float_cmp`; no `#[allow]` was
added or existed. No allocation on any compare path: bit tests and
`(a - b).abs() < EPSILON` only.

## Classification and conversions

### Runtime production (identity → exact bit comparison)

`src/shell/icons.rs` 369, 375, 564 — SVG arc large-arc/sweep flags must be the
literals 0/1. A tolerance here would be a validation hole, so all three routes
go through one new private helper (`is_arc_flag`, `src/shell/icons.rs:283`):
`value.abs().to_bits() == 0.0f32.to_bits() || ... == 1.0f32.to_bits()`.
`abs()` preserves the previous acceptance set exactly — `0.0`, `-0.0`, `1.0`
accepted; `0.5`, `0.999_999_9`, `1.0 + f32::EPSILON`, `2.0`, NaN rejected.
Regression net: `arc_flag_identity_accepts_only_exact_zero_and_one`
(`src/shell/icons.rs` tests; +1 test over the task-1 baseline).

### Test modules inside production files (19)

All were `assert_eq!`/`assert_ne!` on golden values; converted to
`(actual - expected).abs() < f32::EPSILON` / `f64::EPSILON` (and the
`assert_ne!` pair to `>= EPSILON`), which is the house tolerance pattern
(`src/shell/layout/mod.rs:698`, `src/editor/theme.rs:1130` twice over):

- `src/editor/theme.rs` 785, 790 (opaque-alpha identity, f32)
- `src/server/ops/modes.rs` 744, 754 (caret width, f32)
- `src/server/ops/theme.rs` 836, 849 (recipe geometry, f64)
- `src/shell/design_system.rs` 2127, 2130, 2142, 2143 (recipe geometry, f64)
- `src/shell/icons.rs` 997 ×2 (arc flags → `is_arc_flag`, identity)
- `src/shell/package_ui.rs` 1263, 1303, 1304, 1305, 1359, 1390, 1392 (layout, f64)

### Test files (59)

Same tolerance conversion, except `tests/performance_budgets.rs:596`
(`COMPLETION_MAX_WIDTH_PX` vs its literal) which uses
`assert_eq!(COMPLETION_MAX_WIDTH_PX.to_bits(), 480.0_f64.to_bits())` — a
constant identity, not a measurement.

- `src/shell/theme/tests.rs` 61, 529, 1042–1046, 1055–1057, 1075, 1076, 1130, 1154, 1155
- `src/shell/layout/tests.rs` 580, 582, 584, 598, 601, 604, 613, 666, 686, 1083
  (file-level `EPSILON: f64 = 0.000_001` hoisted from `assert_rect_eq` and reused)
- `src/shell/theme/theme_snapshot_tests.rs` 184
- `src/server/runtime_generation_tests.rs` 617, 618, 619, 838 (typography f32)
- `src/server/connection/tests/mod.rs` 711, 833 (typography f32)
- `src/server/connection/tests/protocol_and_bootstrap.rs` 287 (typography f32)
- `src/server/js_runtime/tests/{editor_layout_and_design_system,facades_and_modes,load_package_markdown_defaults,runtime_themes_and_typography}.rs`
  (recipe f64 / caret f32 / typography f32)
- `tests/package_ui_conformance.rs` 914, 918, 1546–1550, 2602, 2674, 2730, 2754, 2758
  (the two at 2602/2674 are test-file runtime logic: opacity `== 1.0` → tolerance)
- `tests/theme_packages.rs` 482, 786, 879, 943 (contrast thresholds, f64)
- `tests/example_config_control_center_chord.rs` 289, 290, 291 (recipe geometry, f64)

(Baseline line numbers; some shifted a few lines after the edits.)

### Strict comparisons clippy's `float_cmp` does not flag (5, fixed anyway)

Found by a post-pass grep for float-literal equality; these sit inside
`assert!`/iterator closures where clippy 0.1.98's `float_cmp` does not fire, and
were converted for consistency with D4:

- `src/shell/layout/tests.rs` 859 (`anchor.x0 == 0.0`)
- `tests/package_ui_conformance.rs` 1749, 2463, 2560, 2737 (border-width/zero-offset checks)

## Deviation from the plan's wording

The plan expected named domain epsilons (e.g. `CONTRAST_EPSILON`). None of the
sites was a contrast-ratio or layout-ratio *tolerance* comparison: contrast code
already compares floors with `>=`, and every flagged site is an exact
golden/identity comparison. The epsilons used are the std per-type constants
(`f32::EPSILON` / `f64::EPSILON`) inline — the established house pattern — plus
the one hoisted `EPSILON` constant in `src/shell/layout/tests.rs`. Adding a named
constant equal to `f64::EPSILON` would add a concept without changing behavior.

## Verification

- `cargo clippy --all-targets -- -W clippy::pedantic`: exit 0, 0 `float_cmp`.
- `scripts/check.sh full`: exit 0, 2001 passed / 0 failed / 1 ignored
  (`task2-check-full.log`, `task2-exit-codes.txt`).
- Targeted runs before the gate: icons 12, shell::theme 29, shell::layout 84,
  shell::package_ui 12, ops::theme 17, ops::modes 8, editor::theme 18,
  design_system 6, runtime_generation 42, runtime suite 75 — all pass.
