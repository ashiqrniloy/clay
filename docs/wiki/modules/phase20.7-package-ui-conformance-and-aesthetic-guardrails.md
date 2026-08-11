# Phase 20.7 Package UI Conformance and Aesthetic Guardrails

## Source

- `src/shell/theme.rs` (`validate_active_theme_contrast`, `ContrastFailure`, `TEXT_CONTRAST_MIN`, `UI_CONTRAST_MIN`, `core_fallback_matches_type`, `validate_design_token_override`, `core_theme_value`)
- `src/shell/components.rs` (`ComponentKind::as_str`, `applicable_states`, `ComponentCatalogError::reject`, `sanitize_rejected`, `json_value_kind`, `reject_raw_style_token`, `validate_component_kind`, `validate_style_variables`, `validate_style_variable`, `validate_enum_style_variable`)
- `src/server/ops/theme.rs` (`enforce_contrast`, `format_contrast_failure`, `apply_theme`, `resolve_canonical_default_theme`)
- `src/packages/record.rs` (`parse_design_token_contributions`, local `json_value_kind`/`sanitize_rejected`)
- `src/server/ui.rs` (`register_component` `ComponentCatalogError` → `UiContributionRule` mapping)
- `src/masonry_sdui.rs` (`component_state_palette`, `SduiThemeStyle::from_ui_theme`, `each_component_kind_renders_all_five_states`)
- `src/editor/theme.rs` (`pub use crate::shell::theme::{ContrastFailure, validate_active_theme_contrast}`)
- `tests/package_ui_conformance.rs` (conformance suite: 10 tests)
- `tests/ui_primitive_conformance.rs` (chrome-paint source-scan suite)
- `tests/theme_packages.rs` (bundled-theme contrast + inert-data)
- `tests/suites/editor.rs` (wires `package_ui_conformance`)
- Authoring docs: `docs/reference/packages/creating-packages.md` (§ "Phase 20.7 authoring contract: UI conformance guardrails")
- Skill references: `.agents/skills/clay-ui/references/components.md` (conformance contract), `.agents/skills/clay-ui/references/tokens.md` (Rules 7–8)
- Plan: `plans/068-Phase20.7-Package-UI-Conformance-and-Aesthetic-Guardrails.md`

## Overview

Phase 20.7 (plan `plans/068-…`) hardens the host-authority validation boundary for package UI so a package cannot destroy Clay's established aesthetics or usability. It adds: an active-theme contrast/legibility floor (WCAG-AA 4.5 text / 3.0 non-text), a per-`ComponentKind` state-completeness contract (`applicable_states`), enriched author diagnostics that name the rejected value + expected type + field, four code-vs-catalog drift guards, three package-runtime trust-domain invariant tests, and a `catalog × state × theme` conformance matrix. **No new `ComponentKind`, typed style variable, token, Clay JS op, JS facade, or configuration key was introduced.** Every conformance helper is `pub(crate)` or a test-re-export; conformance is host authority, not package-facing — no `ui.validate*` op or `clay:*` facade exists for it.

The authoritative public API docs (`docs/reference/clay-js-api/`, `docs/reference/packages/creating-packages.md`) and the clay-ui skill references (`components.md`, `tokens.md`) document the authoring contract; this page explains the implementation behind the guardrails.

## Responsibilities

- Reject an active theme whose status-chrome token pairs fall below `TEXT_CONTRAST_MIN` (4.5) or `UI_CONTRAST_MIN` (3.0); a below-AA theme is not activated and records a `theme.contrast` diagnostic. Startup stays safe: a sub-contrast canonical default falls back to the Clay core default.
- Pin the per-`ComponentKind` interaction-state contract (`applicable_states`) and tie it to the SDUI paint path (`component_state_palette`) so a catalog↔paint drift fails CI.
- Enrich every component-catalog and design-token rejection to name the rejected value, expected type, and field via a single stable diagnostic shape, sanitized so an author string cannot break the message.
- Lint code-vs-catalog drift in four directions: `ComponentKind` enum ↔ `component_state_palette` match arms ↔ `components.md` `Package-Facing Component Kinds` table; typed-style-variable match arms ↔ `components.md` `Typed Style Variables` table; `core_theme_value` match arms ↔ `tokens.md` Core Tokens tables.
- Assert the two package-runtime trust domains stay intact under conformance: third-party raw values and oversized payloads are rejected at the adopted `assemble_package_record` boundary without reaching the trusted runtime, and no conformance helper is exposed as a `deno_core` op or JS facade.
- Keep conformance non-user-disableable and non-package-facing: validation runs inside the Rust host validator at parse/install/theme-apply time only; no new configuration API, no new public programmatic API.

## How It Works

### 1. Contrast / legibility floor

`src/shell/theme.rs` defines the thresholds:

```rust
pub(crate) const TEXT_CONTRAST_MIN: f64 = 4.5;   // WCAG AA text (SC 1.4.3)
pub(crate) const UI_CONTRAST_MIN: f64 = 3.0;     // WCAG AA non-text (SC 1.4.11)
```

`validate_active_theme_contrast(active_theme)` is the public test-facing entry point (`pub fn`, re-exported via `clay::editor::theme` for integration tests). It walks the resolved status-chrome token pairs (foreground over background), computes the WCAG relative-luminance contrast ratio, and returns `Ok(())` or `Err(ContrastFailure { pair, threshold, ratio, .. })`. `ContrastFailure` is `pub` (re-exported the same way) so tests can assert pair/threshold/ratio.

The enforcement helper is `pub(crate)` and lives where it runs:

```rust
// src/server/ops/theme.rs
fn enforce_contrast(…, specifier) -> Result<(), ContrastFailure>;
fn format_contrast_failure(specifier, failure) -> String;
```

`apply_theme` calls `enforce_contrast` **before** `set_active_theme`/`set_explicit_theme_active`: on failure it records a `theme.contrast` diagnostic and returns a `JsErrorBox` **without mutating the active theme** — the prior valid theme stays active. `resolve_canonical_default_theme` calls `validate_active_theme_contrast` and on failure records the diagnostic and returns `None`, so a sub-contrast canonical default at startup falls back to the Clay core default rather than bricking startup.

The contrast check reads resolved token pairs from the active theme; it runs at theme-apply time only (configuration/reload), never in a paint/layout/pointer/scroll/keypress hot path.

### 2. State-completeness contract

`src/shell/components.rs::applicable_states(kind: ComponentKind) -> &'static [InteractionState]` is the per-kind interaction-state contract, grounded in `components.md` per-kind notes:

| Category | Kinds | Applicable states |
|----------|-------|-------------------|
| interactive triggers | `button`, `list`, `dropdown`, `collapse`, `textInput` | `Rest`/`Hover`/`Active`/`Focus`/`Disabled` |
| chrome containers | `panel`, `overlay`, `modal` | `Rest` only |
| text-no-fill | `label`, `statusItem` | `Rest`/`Focus`/`Disabled` |
| scrollbar-bearing | `editorView`, `scroll` | `Rest`/`Hover`/`Active` |
| layout containers | `flex`, `stack`, `portal` | `Rest` only |

`InteractionState` is derived from client-local pointer/focus hit-testing (`SduiNativeState::interaction_state`), **not** from package descriptors — packages declare no interaction states, so there is no per-declaration state-completeness surface to validate. `applicable_states` is a conformance primitive (test-consumed), marked `#[allow(dead_code)]` to match the `ResolvedUiTheme` precedent.

`ComponentKind::as_str(self) -> &'static str` is the inverse of `ComponentKind::parse` (also `pub(crate)`, `#[allow(dead_code)]`), so a single `ComponentKind` value drives both `applicable_states(kind)` and `component_state_palette(kind.as_str(), state)` in the conformance matrix — a round-trip ground truth.

### 3. Author diagnostics

Component-catalog rejections use a single stable shape via a centralized constructor:

```rust
// src/shell/components.rs
pub(crate) fn reject(field, rejected_value, expected, reason) -> Self {
    // "{field} = `{value}` rejected: expected {expected}; {reason}"
}
```

Two private helpers sanitize the author-supplied value:

```rust
fn sanitize_rejected(value: &str) -> String;  // trim, strip backticks → apostrophes, bound 80 chars
fn json_value_kind(value: &Value) -> String;  // "string `x`", "number 42", "array", "object", "null"
```

Every `ComponentCatalogError` site that omitted the rejected value was enriched: non-object `style`, non-string/empty token, token-resolution failure, raw-color/CSS (`reject_raw_style_token` now takes `expected: ThemeTokenType`), and `fontRole`/`variant`/`validationState` enum rejections. Example shipped message:

```text
style.background = `#ff00aa` rejected: expected color-role token; raw colors or raw CSS are not allowed; reference a Clay token (e.g. surface.main)
```

`src/packages/record.rs::parse_design_token_contributions` does its own per-type validation and builds `PackageRecordError` directly (it never reaches `DesignTokenError`, which is `pub(crate)` and only used by the client revalidation path). The `json_value_kind`/`sanitize_rejected` pair is **duplicated locally** in `record.rs` (ponytail: an 8-line pair duplicated across two validation modules rather than promoting a shared module; marked `// ponytail:` to fold into a shared diagnostic module if a third author-JSON validator appears). Each design-token `value`-rejection message appends `; got {actual}` (e.g. `got number 12`, `got \`#zz\``), naming the rejected value kind while preserving every existing substring assertion.

### 4. Runtime-path mapping (preserved contract)

`src/server/ui.rs::register_component` maps `ComponentCatalogError` → `UiContributionRule`:

```rust
let rule = if error.field == "style" || error.message.contains("raw CSS") {
    UiContributionRule::ProhibitedAuthority
} else {
    UiContributionRule::InvalidThemeToken
};
```

The `reject_raw_style_token` reason retains the substring `raw CSS` ("raw colors or raw CSS are not allowed") so raw-color/CSS rejections still classify as `ProhibitedAuthority` (the pre-existing `theme_token_registry_rejects_raw_css_raw_colors_and_type_mismatches` test asserts this). The substring-sniff mapping is pre-existing and fragile; replacing it with a discriminant on `ComponentCatalogError` is out of scope (would add struct churn with no other consumer) and is a follow-up candidate.

### 5. Code-vs-catalog drift guards

`tests/package_ui_conformance.rs` parses source and docs with section-scoped parsers (each `split`s on its `##` heading and stops at the next `## ` heading), so prose in later sections never leaks into a guard's set:

- `catalog_is_drift_free_across_doc_enum_and_paint_path` — `ComponentKind::parse` variants ↔ `component_state_palette` match arms ↔ `components.md` `Package-Facing Component Kinds` table (ASCII-alphanumeric filter on the kind column; the reserved `table` variant lives in a separate `DeferredComponentKind::parse` block the parser scopes past).
- `style_variable_catalog_matches_components_md` — typed-style-variable + enum-style-variable match arms (`src/shell/components.rs`) ↔ `components.md` `Typed Style Variables` table (past-header flag skips the table header row).
- `core_token_catalog_matches_tokens_md` — `core_theme_value` match arms (`src/shell/theme.rs`) ↔ `tokens.md` `Core Tokens (implemented)` tables (filters token names containing `.`).

The in-crate `masonry_sdui::tests::applicable_states_match_component_state_palette` ties `applicable_states` (task 4) to the `component_state_palette` paint path: for each kind category it asserts the documented states match and the palette renders token-driven output for the applicable states.

### 6. Trust-domain invariants

`tests/package_ui_conformance.rs` pins the two-domain contract:

- `third_party_raw_color_rejected_at_adopted_boundary_trusted_runtime_unchanged` — a non-`@clay/*` package with `style.background = "#ff00aa"` is rejected at `assemble_package_record` (`InvalidContributionDescriptor`, contribution_id `style.background`); the `Err` yields no `PackageRecord`, so no descriptor is installed (trusted runtime unchanged).
- `third_party_oversized_sdui_payload_rejected_without_reaching_client` — `estimatedSnapshotBytes: 4097` → `PayloadBudgetExceeded` naming `SDUI_SNAPSHOT_PAYLOAD_BUDGET_BYTES`; no record → no payload reaches the client.
- `no_conformance_helper_exposed_as_op_or_facade` — walks `src/**/*.rs` for `fn op_clay_<name>` and asserts none is `op_clay_ui_validate*` or carries `conform`/`contrast`/`legibility`; scans `src/server/facades.rs` specifiers and asserts none carries conformance intent. Conformance is host authority, never package-facing.

### 7. Conformance matrix

`bundled_theme_conformance_matrix` (parametric over the 4 bundled themes — Gruvbox dark/light, Modus Operandi/Vivendi) asserts inert data (no permissions/modes), zero `designTokens` (all bundled themes contribute only `textStyles`), and AA contrast. The behavioral `catalog × state` render matrix lives in-crate (`masonry_sdui::tests::each_component_kind_renders_all_five_states`, 15 kinds × 5 states) because the `shell` module is `pub(crate)` — `ResolvedUiTheme`, `component_state_palette`, and `SduiNativeState` are unreachable from integration tests. Parametric-over-themes for the SDUI render matrix is a no-op (all bundled themes contribute zero `designTokens`, so all resolve to the core fallback catalog).

## Code Examples

Below-AA theme is not activated (server path):

```rust
// src/server/ops/theme.rs::apply_theme
enforce_contrast(&clay_state, specifier, &active_theme)
    .map_err(|f| {
        record_diagnostic(&clay_state, "theme.contrast", format_contrast_failure(specifier, &f));
        JsErrorBox::from(format_contrast_failure(specifier, &f))
    })?;
set_active_theme(&clay_state, active_theme);  // reached only on Ok
```

Stable rejection shape (component catalog):

```text
style.background = `#ff00aa` rejected: expected color-role token; raw colors or raw CSS are not allowed; reference a Clay token (e.g. surface.main)
color-role design token `value` must be a #rgb, #rrggbb, or #rrggbbaa hex string; got number 12
```

Trust-domain gate (conformance is not package-facing):

```rust
// tests/package_ui_conformance.rs::no_conformance_helper_exposed_as_op_or_facade
let conformance_ops: Vec<String> = clay_op_names().into_iter()
    .filter(|n| n.starts_with("op_clay_ui_validate") || n.contains("conform") || ...)
    .collect();
assert!(conformance_ops.is_empty(), "no conformance helper may be exposed as a deno_core op; found: {conformance_ops:?}");
```

## Primitive Coverage

- **Contrast / legibility floor:** owning module `src/shell/theme.rs` (`validate_active_theme_contrast`, `ContrastFailure`, `TEXT_CONTRAST_MIN`/`UI_CONTRAST_MIN`) + `src/server/ops/theme.rs` (`enforce_contrast`/`format_contrast_failure`). No JS facade/op. Permission: none. Hot path: theme-apply (configuration/reload) only. Validation: WCAG relative-luminance ratio over resolved status-chrome pairs. Reference docs: `creating-packages.md` § Phase 20.7, `tokens.md` Rule 7.
- **State-completeness contract:** owning module `src/shell/components.rs` (`applicable_states`, `ComponentKind::as_str`). No JS facade/op. Conformance primitive, test-consumed. Reference docs: `components.md` `Package-Facing Component Kinds` notes, `creating-packages.md` § Phase 20.7.
- **Author diagnostics:** owning module `src/shell/components.rs` (`ComponentCatalogError::reject`, `sanitize_rejected`, `json_value_kind`) + `src/packages/record.rs` (local duplicate pair + `; got {actual}` appends). Surfaced through `PackageRecordError` (parse path) and `UiContributionDiagnostic` (runtime path). Reference docs: `creating-packages.md` § Phase 20.7 (diagnostic message format + example).
- **Drift lint:** owning test `tests/package_ui_conformance.rs` (3 guards) + `src/masonry_sdui.rs::tests` (`applicable_states_match_component_state_palette`). Source-scan, no runtime work. Reference docs: `components.md` conformance contract, `tokens.md` Rule 8.
- **Trust-domain invariants:** owning test `tests/package_ui_conformance.rs` (3 tests). Source-scan + boundary rejection. Reference docs: `creating-packages.md` § Phase 20.7 (authority boundaries), `components.md` conformance contract.
- **Reuse rule:** future guardrails extend `ComponentCatalogError::reject` or add a drift guard to `tests/package_ui_conformance.rs` rather than introducing a package-facing validation API. Conformance diagnostics surface through the existing server runtime diagnostics channel — no new broadcast primitive.

## Invariants and Constraints

- Conformance is host authority, not package-facing: no `ui.validate*` op, no `clay:*` facade, no new `configuration.*` key. `validate_active_theme_contrast`/`ContrastFailure` are `pub` only as a test-facing re-export via `clay::editor::theme`; they are not wired to any op or facade (no trust path).
- A below-AA theme is never activated; the prior valid theme stays active on `apply_theme` failure, and a sub-contrast canonical default falls back to the Clay core default at startup (`resolve_canonical_default_theme` returns `None`).
- Guardrails are non-user-disableable (no config toggle to relax contrast thresholds).
- `applicable_states` is the per-kind state contract; `InteractionState` is derived from pointer/focus hit-testing, never from package descriptors — packages declare no interaction states.
- The rejected value in a diagnostic is sanitized (trimmed, backticks stripped, ≤80 chars) so an author string cannot break the message shape or inject markdown.
- The `json_value_kind`/`sanitize_rejected` pair is duplicated across `src/shell/components.rs` and `src/packages/record.rs` (`ponytail:` comment); fold into a shared diagnostic module if a third author-JSON validator appears.
- The `ui.rs` `error.message.contains("raw CSS")` substring-sniff maps raw-color rejections to `ProhibitedAuthority`; it is pre-existing and fragile — a discriminant on `ComponentCatalogError` is the upgrade path (follow-up candidate).
- Pre-existing test baselines unrelated to this phase: `--test protocol` 132/4 and `--test security` 121/1 both verified identical on a clean tree (all Phase 20.7 changes stashed).

## Tests

- `tests/package_ui_conformance.rs` (10 tests, wired via `tests/suites/editor.rs`):
  - `bundled_theme_conformance_matrix` — 4 bundled themes: inert, zero `designTokens`, AA contrast.
  - `catalog_is_drift_free_across_doc_enum_and_paint_path` — `ComponentKind` ↔ paint ↔ doc tri-directional drift.
  - `style_variable_catalog_matches_components_md` — style-variable match arms ↔ doc table.
  - `core_token_catalog_matches_tokens_md` — `core_theme_value` ↔ tokens.md Core Tokens.
  - `style_variable_rejection_names_value_expected_type_and_field` — raw `#ff00aa` rejection.
  - `design_token_type_mismatch_names_token_expected_and_actual` — `surface.hover` given number 12.
  - `reserved_component_kind_names_kind_and_reserved` — reserved `table`.
  - `third_party_raw_color_rejected_at_adopted_boundary_trusted_runtime_unchanged` — trust domain.
  - `third_party_oversized_sdui_payload_rejected_without_reaching_client` — trust domain.
  - `no_conformance_helper_exposed_as_op_or_facade` — op/facade absence.
- `tests/theme_packages.rs`: `theme_package_below_aa_contrast_is_rejected`, `bundled_themes_sdui_pairs_meet_aa_contrast` (parametric 4 themes), `enforce_contrast_accepts_core_palette`, `enforce_contrast_rejects_low_contrast_without_mutating_active_theme`.
- `src/shell/components.rs::tests`: `applicable_states_table_matches_components_md` (+ existing catalog validation tests).
- `src/masonry_sdui.rs::tests`: `applicable_states_match_component_state_palette`, `each_component_kind_renders_all_five_states`, `component_state_colors_are_token_derived`.
- `tests/ui_primitive_conformance.rs`: chrome-paint source-scan (no `Color::from_rgb8` literals / hardcoded sizes outside `primitives.rs`+`theme.rs`).
- Commands: `cargo test --test editor package_ui_conformance`, `cargo test --lib shell::components masonry_sdui`, `cargo test --test editor theme_packages`, `cargo test --test editor ui_primitive_conformance`.

## Related

- [Shell Primitives](shell-primitives.md) — `src/shell/primitives.rs` chrome primitive layer, token-driven paint.
- [Editor Theme Registry](editor-theme-registry.md) — `StyleRegistry`, `ActiveTheme`, Phase 20.1 typed design tokens, `ResolvedUiTheme`.
- [Slot-Aware Package UI](slot-aware-package-ui.md) — `clay:ui` contribution registry, component catalog, `ComponentKind` validation.
- [Server-Driven UI](server-driven-ui.md) — SDUI paint path, `SduiNativeState`, payload budgets.
- [Package Loading](package-loading.md) — `assemble_package_record` adopted boundary, payload budgets.
- [Third-Party Runtime Authority](third-party-runtime-authority.md) — two trust domains, inert-data invariants.
- [Maintenance and Validation](maintenance-validation.md) — drift-guard test patterns.
- [Phase 20.6 Theme Package Segregation](phase20.6-theme-segregation-settings-ui.md) — canonical default resolution (`apply_theme`/`resolve_canonical_default_theme` now contrast-guarded).
- [Phase 20.4 Core Component Uplift](phase20.4-core-component-uplift-primitive-review.md) — `component_state_color`/`list_row_fill_color` token→state mapping the contrast floor protects.
- [UI Chrome Primitives Reference](../../reference/primitives/ui-chrome-primitives.md)
- [Creating Packages — Phase 20.7 conformance guardrails](../../reference/packages/creating-packages.md)
- [Clay UI Component Catalog (skill)](../../../.agents/skills/clay-ui/references/components.md)
- [Clay Theme Tokens (skill)](../../../.agents/skills/clay-ui/references/tokens.md)
- `plans/068-Phase20.7-Package-UI-Conformance-and-Aesthetic-Guardrails.md`