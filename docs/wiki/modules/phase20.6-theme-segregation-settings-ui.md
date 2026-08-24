# Phase 20.6 Theme Package Segregation and Settings UI

## Source

- `packages/theme-modus-operandi/{package.json,dist/load.js,docs/index.md}`
- `packages/theme-modus-vivendi/{package.json,dist/load.js,docs/index.md}`
- `packages/settings/{package.json,dist/load.js,docs/index.md}`
- `src/protocol/mod.rs` (`Appearance`, `ResolvedAppearance`)
- `src/server/ops/theme.rs` (`op_clay_theme_set_appearance`, `apply_theme`, `apply_appearance`, `canonical_default_specifier`, `resolve_canonical_default_theme`, `build_active_theme_from_record`)
- `src/server/ops/typography.rs` (`apply_typography`)
- `src/server/ops/mod.rs` (`ClayOpState` appearance/explicit-theme-active fields)
- `src/server/configuration.rs` (`PersistedPreferences`, `load_preferences`, `persist_preference`, `clear_preferences`)
- `src/server/js_runtime/mod.rs` (`apply_persisted_preferences`, canonical-default harvest injection)
- `src/server/connection/mod.rs` (`persist_settings_change`, `PersistOutcome`, settings command dispatch, `sdui_command_request` argument forwarding)
- `src/server/command_execution.rs` (`is_settings_command`, `execute_settings`)
- `src/packages/bundled.rs` (Modus + `@clay/settings` registration, FNV-1a-64 fingerprints)
- `runtime/js/theme.js`, `runtime/js/theme.d.ts` (`setTheme`, `setAppearance`, and `setTypography` facades)
- `docs/reference/clay-js-api/theme/set-appearance.md`
- `docs/reference/clay-js-api/configuration.md` (Phase 20.6 precedence section)
- `docs/reference/packages/creating-packages.md` (canonical defaults + override APIs subsections)
- `docs/reference/clay-js-api/api-inventory.toml` (`theme.setAppearance` entry)
- `docs/generated/clay-js-api-registry.json`
- Tests: `tests/theme_packages.rs`, `src/server/js_runtime/mod.rs`, `src/server/ops/theme.rs`, `src/server/configuration.rs`, `src/server/command_execution.rs`, `src/server/connection/mod.rs`, `tests/clay_js_doc_registry.rs`

## Overview

Phase 20.6 (plan `plans/067-Phase20.6-Theme-Package-Segregation-and-User-Theme-Font-UI.md`) segregates the canonical default themes into dedicated first-party packages, adds a system/manual light-dark appearance preference that resolves those canonical defaults, ships a `@clay/settings` SDUI panel for theme/appearance/font/size-hierarchy selection, and persists UI session choices through `preferences.json` with a documented precedence model. No new `ComponentKind`, token, style variable, or Clay JS configuration key was introduced — the surface is pure composition of existing catalog components, existing `clay:theme` facades, and the existing reload→fanout apply path.

The authoritative public API docs live in `docs/reference/clay-js-api/` (`set-theme.md`, `set-appearance.md`, `set-typography.md`, `configuration.md`); this page explains the implementation behind them.

## Responsibilities

- Ship Modus Operandi and Modus Vivendi as inert first-party theme packages (`@clay/theme-modus-operandi`, `@clay/theme-modus-vivendi`) alongside the existing Gruvbox packages, with faithful upstream palettes and GPL-3.0-or-later attribution.
- Make Modus Operandi the canonical light-mode default and Modus Vivendi the canonical dark-mode default, resolved from the `appearance` preference without any `loadPackage` call in `init.js`.
- Expose a bounded `light` | `dark` | `system` appearance preference through a new registry-public `theme.setAppearance` API; `system` follows the OS color-scheme signal with a dark fallback.
- Ensure an explicit `setTheme` always wins over the appearance-derived canonical default.
- Ship `@clay/settings`, a first-party package that registers a right-slot settings panel built only from catalog components (`panel`, `scroll`, `flex`, `collapse`, `dropdown`, `textInput`, `label`, `button`) emitting inert `settings.*` command intents.
- Validate settings commands server-side (`setTheme` requires a bundled first-party `@clay/theme-*` specifier; `setAppearance` requires the bounded enum), persist `setTheme`/`setAppearance`/`reset` to `~/.config/clay/preferences.json`, and trigger a runtime reload so changes apply live without restart.
- Enforce a single documented configuration precedence: canonical/package default < `init.js` < `ui-session` (persisted preferences).
- Keep the `clay:configuration` module closed; appearance is a `clay:theme` API, not a new configuration key. Hidden/ad-hoc keys remain rejected.

## How It Works

### 1. Modus theme packages

`@clay/theme-modus-operandi` and `@clay/theme-modus-vivendi` mirror the Gruvbox package structure: inert `clay.contributions.textStyles` (48 entries each) plus no-op `dist/index.js` / `dist/load.js` ESM entry/load files. Palettes are faithful to the upstream Modus themes (Protesilaos Stavrou, v4.6.0, GNU GPL v3.0+): keyword = magenta-cooler/warmer, string = blue-warmer, comment = `fg-dim`, type/namespace/struct = cyan-cooler/cyan, macro/special = red-cooler, function = magenta (not bold, unlike Gruvbox). Base UI keys (`shellBg`, `panelBg`, `text`, `placeholder`, `selection`, `caret`, `scrollbar`, `statusBg`, diagnostic severities) map to upstream `bg-main`/`fg-main`/`bg-dim`/`bg-mode-line-active` and red/yellow/cyan-cooler. `docs/index.md` in each package records provenance, the palette table, and retuning instructions.

Packages are registered in `src/packages/bundled.rs` as `BundledPackageEntry` rows (root, name, version, FNV-1a-64 `fingerprint`) in alphabetical order. Embedding is directory-scan based (`CARGO_MANIFEST_DIR`/`packages/` at runtime) — no `include_str!`/`include_bytes!`; the fingerprint is a declared `&'static str` used for tamper/integrity checks. `@clay/settings` is registered the same way.

### 2. Appearance preference and canonical default resolution

`src/protocol/mod.rs` adds `Appearance { Light, Dark, System }` (`#[derive(Default)]` with `System` as default) and `ResolvedAppearance`. `System` resolves to dark (Modus Vivendi) when no OS signal is available; the winit 0.30 `Window::theme()` getter is the planned OS signal source (read at startup, live change best-effort).

`src/server/ops/theme.rs` adds:

- `canonical_default_specifier(ResolvedAppearance)` → `@clay/theme-modus-operandi` (Light) / `@clay/theme-modus-vivendi` (Dark).
- `resolve_canonical_default_theme(clay_state, appearance)` → `ensure_first_party_record` + `build_active_theme_from_record` to produce an `ActiveTheme` snapshot from the bundled inventory, without any `loadPackage` call.
- `op_clay_theme_set_appearance` → parses the bounded enum (rejects unknown values with `theme.invalid_request`), stores the preference in `ClayOpState`, and returns `{ appearance, resolvedTheme }`.
- `apply_theme` / `apply_appearance` / `apply_typography` — `pub(crate)` shared primitives extracted from the ops so the persisted-preference harvest and the JS ops use one code path.

`ClayOpState` gains an `appearance` preference and an `explicit_theme_active` flag. `op_clay_theme_set_theme` sets `explicit_theme_active = true` so an explicit theme always overrides the appearance-derived default.

### 3. Canonical-default harvest injection

`src/server/js_runtime/mod.rs` injects canonical-default resolution into the evaluation harvest: after `init.js` evaluates, if no explicit theme was set, the harvest resolves the canonical default from the current appearance and installs it as the active theme. Then `apply_persisted_preferences` runs immediately after to overlay `ui-session` preferences, so a persisted UI choice always overrides the equivalent `init.js` call and the canonical default. An entirely absent `init.js` does not silently resolve a canonical default — the existing `load_configuration_from_root` error contract is preserved.

### 4. `@clay/settings` SDUI panel

`@clay/settings` is a first-party package whose `dist/load.js` registers a fixed right-slot `panel` contribution plus `settings.*` command metadata via the `clay:ui` / `clay:commands` facades. The panel tree composes only catalog `ComponentKind`s: `panel` + `scroll` + `flex` container, `collapse` sections, `dropdown` pickers (theme over bundled themes, appearance over light/dark/system), `textInput` for font-family stacks and base sizes/hierarchy ratios (with `validationState`), `label` for row/section titles, and `button` (default/muted) for reset/apply. Controls emit inert `settings.*` command intents (`settings.setTheme`, `settings.setAppearance`, `settings.setTypography`, `settings.reset`, `settings.open`, `settings.close`).

The surface is a fixed panel rather than a transient `modal` overlay because the component tree (~3.4 KiB) exceeds `SDUI_UPDATE_PAYLOAD_BUDGET_BYTES` (1024). No `multi-select` is used — the three font profiles are fixed and edited via `textInput`.

`src/server/connection/runtime.rs::sdui_command_request` forwards `SduiActionSource` (`ListItem.item_id` for dropdowns/lists, the node id for buttons) as command arguments, enabling value carriage from SDUI actions to the server handler.

### 5. Settings command validation and persistence

`src/server/command_execution.rs::execute_settings` validates:

- `settings.setTheme` — requires a bundled first-party `@clay/theme-*` specifier from `BUNDLED_PACKAGES`.
- `settings.setAppearance` — requires `light` | `dark` | `system`.
- `settings.setTypography` — the Tauri/React trusted settings module sends one complete three-profile/seven-ratio JSON transaction; the shared typography parser validates it before atomic persistence and again during reload.

`src/server/connection/runtime.rs::persist_settings_change` (`PersistOutcome`) merges `settings.setTheme` / `settings.setAppearance` / `settings.reset` into `~/.config/clay/preferences.json` (atomic tmp + rename) and triggers `reload_runtime_generation()` so the change applies live through the canonical apply path: persist → reload → `init.js` re-eval + `apply_persisted_preferences` → `RuntimeStateSnapshot` fanout. No restart required. Live theme apply rides the existing `RuntimeStateSnapshot` fanout (which already ships `ActiveTheme`); no separate `ThemeUpdates` broadcast primitive was needed.

### 6. `preferences.json` and precedence

`src/server/configuration.rs::PersistedPreferences` owns the closed store: at most three keys (`theme`, `appearance`, `typography`), bounded to `PREFERENCES_PAYLOAD_BUDGET_BYTES` (8 KiB), validated at load and persist time, and authority-rejecting (no raw ops, CSS, callbacks, client JS, or state values). `load_preferences` skips `null` fields so a partial preference file does not produce false diagnostics, and drops corrupted/oversized/manual-edit fields field-by-field with a diagnostic so startup never breaks. The `setPackageOption` source taxonomy is extended with `ui-session` to label these persisted values.

Precedence (highest wins):

| Rank | Source | Location | Overrides |
|------|--------|----------|-----------|
| 1 | `ui-session` | `~/.config/clay/preferences.json` (written by `settings.setTheme`/`setAppearance`) | everything below |
| 2 | `init-js` | `~/.config/clay/init.js` `setTheme`/`setAppearance`/`setTypography` | package/canonical defaults |
| 3 | canonical/package default | appearance-derived Modus default (`System`→dark→Vivendi; `Light`→Operandi), or Clay core default | — |

### 7. `setAppearance` Clay JS API

`theme.setAppearance` is a registry-public `clay:theme` facade (id `theme.setAppearance`, phase Phase 20.6, owner `server`, `custom_properties: [appearance:enum=required]`). It is NOT a `clay:configuration` API — `clay:configuration` stays closed (`setPackageOption` + `loadConfigurationModule` + `getConfigurationState` only). Docs: `docs/reference/clay-js-api/theme/set-appearance.md`; inventory entry in `docs/reference/clay-js-api/api-inventory.toml`; generated registry in `docs/generated/clay-js-api-registry.json` (regenerated via `cargo run --bin update-doc-registry`).

### 8. Plan 088 Clay JS API verification

Plan 088 adds no new public programmatic surface. The existing theme trio is the complete user-facing API for this modernization:

| Stable ID | Facade export | Deno op | Rust owner | Reference |
|---|---|---|---|---|
| `theme.setTheme` | `clay:theme` / `setTheme` | `op_clay_theme_set_theme` | `src/server/ops/theme.rs::apply_theme` | [`set-theme.md`](../../reference/clay-js-api/theme/set-theme.md) |
| `theme.setAppearance` | `clay:theme` / `setAppearance` | `op_clay_theme_set_appearance` | `src/server/ops/theme.rs::apply_appearance` | [`set-appearance.md`](../../reference/clay-js-api/theme/set-appearance.md) |
| `theme.setTypography` | `clay:theme` / `setTypography` | `op_clay_theme_set_typography` | `src/server/ops/typography.rs::apply_typography` | [`set-typography.md`](../../reference/clay-js-api/theme/set-typography.md) |

Each row is present in `api-inventory.toml`, `docs/index.md`, and the generated registry, with user-facing name, empty default key bindings, behavior-changing `custom_properties`, security notes, lookup tags, facade path, op path, and backing Rust path. `cargo run --bin update-doc-registry` is the repair command when authoritative Markdown changes; the registry was already current for this verification.

The shared `apply_theme`, `apply_appearance`, and `apply_typography` functions are `pub(crate)` server primitives, not additional JS APIs. Plan 088's client-only `ClayShellWidget::set_active_typography`, `PackageModalDismiss`, and benchmark/layout helpers likewise have no op, facade, inventory entry, or configuration authority. The `settings.*` identifiers belong to the `@clay/settings` package's `apiPrefix`, so they remain inert server-first package command intents rather than reserved core IDs or duplicate Clay JS exports.

Verification is split across `tests/clay_js_api_inventory.rs` (inventory/docs/index/generated-registry/schema and Rust/op/facade paths), `tests/clay_js_doc_registry.rs` (theme metadata, custom properties, lookup, precedence, authority denial), `tests/clay_js_facade_layout.rs` (JS and declaration exports plus facade inclusion), and `tests/rust_visibility_api_mapping.rs` (visual-only Rust visibility/hidden bridge allowlist). The facade test explicitly covers all three theme exports.

## Code Examples

Canonical default in a silent `init.js`:

```js
// ~/.config/clay/init.js — no loadPackage call needed
import { setAppearance } from "clay:theme";
setAppearance("system"); // System → OS signal, dark fallback → @clay/theme-modus-vivendi
```

Opt-in Gruvbox (explicit wins over canonical default):

```js
import { setTheme } from "clay:theme";
setTheme("@clay/theme-gruvbox-material-light");
```

Settings command intent (emitted by the `@clay/settings` panel, validated by `execute_settings`):

```text
settings.setTheme  { specifier: "@clay/theme-modus-operandi" }
settings.setAppearance { appearance: "light" }
settings.reset
```

## Primitive Coverage

- **Appearance preference + canonical default resolution:** owning module `src/server/ops/theme.rs`; JS facade `theme.setAppearance` (`runtime/js/theme.js`); Deno op `op_clay_theme_set_appearance`; protocol `Appearance`/`ResolvedAppearance` (`src/protocol/mod.rs`); `pub(crate)` apply primitives (`apply_theme`/`apply_appearance`/`apply_typography`) reused by the harvest. Permissions: none beyond resolving bundled first-party themes. Hot path: configuration/reload only. Validation: bounded enum, first-party specifier, manifest payload budget.
- **Persisted user preferences:** owning module `src/server/configuration.rs::PersistedPreferences`; closed three-key store, 8 KiB budget, atomic tmp+rename, field-by-field corruption drop. No JS facade — written by the Rust settings command executor, not callable from `init.js`.
- **Settings SDUI surface:** owning package `@clay/settings`; catalog-only composition (no new `ComponentKind`/token/style variable). Command intents validated by `execute_settings`; live apply via reload→fanout.
- **Reuse rule:** future themes ship as inert `textStyles` packages and become canonical defaults only by editing `canonical_default_specifier`; future settings controls compose existing catalog kinds and emit `settings.*` intents — no per-surface Rust branches.

## Invariants and Constraints

- An explicit `setTheme` always wins over the appearance-derived canonical default.
- `System` appearance falls back to dark (Modus Vivendi) when no OS signal is present.
- The `clay:configuration` module stays closed; appearance is a `clay:theme` API. No undocumented configuration keys.
- `preferences.json` is authority-rejecting and closed-key; corrupted/oversized fields are dropped, not fatal.
- Theme packages are inert data — no renderer code, native widgets, raw ops, file/network/shell access, or workspace mutation. Non-`@clay/*` specifiers are denied by `setTheme`/`settings.setTheme`.
- `settings.setTypography` persists only a complete validated transaction. Invalid families, sizes, hierarchy ratios, unknown fields, prohibited authority, or oversized payloads preserve the previous preference and runtime generation.
- Settings live-apply rides the existing reload→`RuntimeStateSnapshot` fanout; no separate theme broadcast primitive was added (contrast/payload guardrails deferred to Phase 20.7).

## Tests

- `tests/theme_packages.rs`: Gruvbox + Modus packages validate as inert full 48-entry mappings (`assert_full_theme_mapping` parameterized by `keyword_bold`: Gruvbox true, Modus false), distinct palettes (`assert_distinct_theme_palettes`), and AA-contrast status chrome across all four themes. Command: `cargo test --test editor theme_packages::`.
- `src/server/js_runtime/mod.rs`: `set_theme_resolves_first_party_gruvbox_theme` (both Gruvbox variants), `canonical_default_is_modus_not_gruvbox`, `explicit_set_theme_wins_over_canonical_default`, `absent_init_js_loads_no_runtime_theme`, `set_appearance_light_resolves_canonical_modus_operandi`, `explicit_set_theme_wins_over_appearance`, `set_appearance_rejects_unknown_value`, and the `preferences_*` precedence matrix (`preferences_override_init_js_theme_on_reload`, `no_preferences_lets_init_js_win`, `preferences_appearance_applies_when_init_js_is_silent`, `preferences_theme_beats_appearance_canonical_default`, `preferences_typography_round_trips_through_reload`). Command: `cargo test --lib js_runtime::`.
- `src/server/ops/theme.rs`: `appearance_system_falls_back_to_dark_without_os_signal`, `appearance_parse_rejects_unknown_values`.
- `src/server/configuration.rs`: `PersistedPreferences` round-trip, first-party theme validation, bounded appearance enum, unknown-key rejection, corrupted JSON, payload budget.
- `src/server/command_execution.rs`: `is_settings_command` prefix + `settings.setTheme`/`setAppearance` validation (6 tests).
- `src/server/connection/mod.rs`: `sdui_command_request` `item_id`/`node_id` argument forwarding.
- `tests/clay_js_doc_registry.rs`: `configuration_api_documents_phase20_6_appearance_and_precedence` (setAppearance registry-public, `appearance` custom_property, authority denial, configuration.md precedence/ui-session/preferences.json docs, closed `clay:configuration`).
- `src/packages/bundled.rs`: `bundled_extension_points_match_real_contributions`, `inventory_matches_source_tree`, integrity/provenance tests include the Modus + settings entries.
- Commands: `cargo test --lib packages::bundled`, `cargo test --lib theme`, `cargo test --test editor`, `cargo test --test protocol configuration`.

## Plan 097 Phase 9 Tauri/React settings projection

The target client compiles `frontend/src/settings/SettingsPanel.tsx` as the
trusted presentation module for the exact bundled `@clay/settings` panel.
`settings.open` and `settings.close` remain server-validated package commands;
the server returns a narrow `ShellClientCommandRequest` only after validation,
and the per-tab React controller toggles the panel. Theme/appearance actions
keep the existing item-id path. Typography is now one complete bounded JSON
argument built from the current Rust-resolved typography snapshot, validated by
`validate_typography_request`, atomically written to `preferences.json`, and
applied through the existing reload transaction. Third-party package surfaces
cannot select this compiled module or call Tauri.

## Related

- [Editor Theme Registry](editor-theme-registry.md) — `StyleRegistry`, `setTheme`, `ActiveTheme`, Phase 20.1 typed design tokens.
- [Configuration Runtime](configuration-runtime.md) — `init.js` loading, closed `clay:configuration` module, `setPackageOption`.
- [Typography Registry and Font Roles](typography-registry-and-font-roles.md) — `setTypography`, font profiles, hierarchy.
- [Slot-Aware Package UI](slot-aware-package-ui.md) — `clay:ui` contribution registry, catalog kinds, fixed panel composition.
- [Persistent Runtime Hot Reload](persistent-runtime-hot-reload.md) — `RuntimeStateSnapshot` fanout, reload lifecycle.
- [Package Loading](package-loading.md) — bundled package registration, FNV-1a-64 fingerprints.
- [`theme.setAppearance`](../../reference/clay-js-api/theme/set-appearance.md)
- [`theme.setTheme`](../../reference/clay-js-api/theme/set-theme.md)
- [Clay Configuration System](../../reference/clay-js-api/configuration.md)
- [Canonical defaults vs opt-in themes (authoring guide)](../../reference/packages/creating-packages.md)
- `plans/067-Phase20.6-Theme-Package-Segregation-and-User-Theme-Font-UI.md`