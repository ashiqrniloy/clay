# Plan 133 task 5 evidence — table-driven closed-choice validation in `src/server/ui.rs`

Task: "Table-driven validation in `src/server/ui.rs` (C4)" — the 21 hand-written
string-set checks become one declarative table consulted by one validator.

## Result

`src/server/ui.rs`: **3,320 → 2,164 lines**; the 1,112-line inline test module moved
to **`src/server/ui/tests.rs`** (task-3/4 convention, `#[cfg(test)] mod tests;`),
which also carries the two new table tests.

The 21 checks (147 lines of `if !VALID_*.contains(..) { return Err(context.error(
rule, Some(id), "…")) }`) are now one-line calls:

```rust
validate_choice("slot", slot, &id, &context)?;
```

## The table

```rust
type ChoiceRow = (&'static str, &'static [&'static str], UiContributionRule, &'static str);
// (key, allowed, rule, label)
const CHOICE_FIELDS: &[ChoiceRow] = &[
    ("slot", VALID_SLOTS, Rule::InvalidSlot, "panel slot must be one of"),
    ("state.scope", VALID_UI_STATE_SCOPES, Rule::InvalidStateScope, "UI state scope must be"),
    … 21 rows, one per validated field …
];
```

- `validate_choice(key, value, id, context)` is the single validator: allowed
  values, diagnostic rule, and message all come from the row.
- The message is *derived* (`choice_message`: label + `a, b, or c` built from the
  allowed slice), so adding a value is one line in the `VALID_*` slice and the
  diagnostic follows. `label` keeps the two "must be one of …" phrasings and the
  `, or` Oxford comma.
- Rows are keyed by `<family>.<field>`; layout-override rows validate an override
  *value* (`layoutOverride.slot`, `…visibility`, `…fallback`) and keep the
  existing id = offending value, while the object-backed rows keep id = the
  contribution id — so diagnostic (rule, id, message) triples are unchanged.
- Reads and defaults stay at the call sites (`required_str` / `optional_str`
  `unwrap_or(..)`), so missing-field, blank-string, and wrong-type behaviour is
  untouched; only the membership check moved into the table.

## Security acceptance (coverage strictly non-decreasing)

Two new unit tests in `src/server/ui/tests.rs`, generated from the pre-change
file so they cannot drift into agreeing with a regression:

- `choice_fields_pin_every_allowed_set` — the table has exactly 21 rows, every
  `VALID_*` slice still has ≥1 row, keys are unique, and each row's allowed set
  **equals the set that shipped before** (`assert_eq!` per field, literal lists).
- `choice_messages_match_the_shipped_diagnostics` — each row's derived message
  equals the exact published message (21 literals) — this caught the missing
  Oxford comma on the first run, which is why the phrasing is preserved verbatim.

## Performance

The lookup is a ≤21-entry linear scan per closed-choice field, executed when a
package registers contributions or a layout override is validated — not on the
SDUI snapshot/update hot path. No allocation happens on the accepted path (the
message is built only for a rejected value). Payload-budget and SDUI conformance
tests are green.

## Compromises

- The plan's "ui.rs shrinks (target: < ~2,000 lines)" is **not fully met**: 2,164
  lines remain (from 3,320). The 21 checks were 147 lines and the table +
  validator + docs cost 111, so the validation refactor itself saves ~36 lines;
  the file is otherwise the registry and the per-contribution validators, which C4
  does not cover. Going below ~2,000 needs a family split of `ui.rs` (e.g.
  `src/server/ui/{panels,overlays,inputs,state}.rs`), which is a separate task.
- Files touched beyond the task's declared `src/server/ui.rs`: the extracted
  `src/server/ui/tests.rs` (same deviation pattern plan 133 task 3 documented for
  `src/server/mod.rs`).

## Gates

- `scripts/check.sh full` — `full check PASSED`; **1998 passed / 0 failed / 1
  ignored** (task-1 baseline 1996 + the 2 new table tests), clippy
  `--all-targets -- -D warnings` clean, `cargo fmt --check` clean, desktop and
  bindings stages green (`task5-check-full.log`, `task5-exit-codes.txt`).
- First full-gate attempt hung in
  `manual_smoke_docs::plan118_ui_review_harness_captures_the_shipped_system_and_rejects_removed_states`
  (the known desktop-capture flake: passes standalone in 50 s, and passed in the
  rerun); 1488 tests had passed with 0 failures before the hang. Not related to
  this change — `ui.rs` is not exercised by that harness.
- Unit/guard spot checks: `cargo test --lib server::ui::` 24 passed;
  `presentation` suite `package_ui_conformance` 31 passed; `protocol` suite
  `performance_budgets` + `documentation_coverage` + `clay_js_doc_registry` 99
  passed; `security` suite visibility guards 6 passed.

## Artifacts

- `task5-table-validation.py` — codemod that replaced the 21 sites (each located
  by its unique diagnostic message, brace-matched) and inserted the table; the
  21 rows are also its data source.
- `task5-check-full.log`, `task5-exit-codes.txt`.