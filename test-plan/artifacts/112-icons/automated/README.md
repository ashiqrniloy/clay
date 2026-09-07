# Plan 112 Task 10 — Cross-Layer Compatibility, Security, and Performance Verification

Date: 2026-09-07 · Host: Linux · All commands run from repo root unless prefixed.

## Command summary (raw logs in this directory)

| Gate | Command | Result | Log |
| --- | --- | --- | --- |
| Format | `cargo fmt --check` | PASS (one line-wrap in new test auto-fixed, then clean) | `cargo-fmt-check.txt` |
| Compile | `cargo check --all-targets` | PASS | `cargo-check.txt` |
| Lint | `cargo clippy --all-targets -- -D warnings` | PASS, 0 findings | `cargo-clippy.txt` |
| Root tests | `cargo test` | 1251 lib + 46 + 46 + 75 runtime + 135 security + 207/208 protocol PASS; 1 protocol failure = pre-existing `documentation_coverage::parity_ledger…` (Plan 109 WIP domain, present at HEAD before icon work) | `cargo-test-root.txt` |
| Tauri compile | `cargo check --manifest-path src-tauri/Cargo.toml --all-targets` | PASS (after adding `icon: None` / `active_icon_pack: None` to 1 dto.rs test fixture — Task 6 field additions had not reached this file) | `tauri-check.txt` |
| Tauri lint | `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings` | PASS, 0 findings | `tauri-clippy.txt` |
| Tauri tests | `cargo test --manifest-path src-tauri/Cargo.toml` | 52 passed / 0 failed (incl. icon-pack DTO round-trip + fail-closed tests) | `tauri-test.txt` |
| Frontend types | `npm --prefix frontend run typecheck` | PASS, 0 errors | `frontend-typecheck.txt` |
| Frontend lint | `npm --prefix frontend run lint` | 12 problems (9 errors, 3 warnings) — unchanged from HEAD baseline, documented pre-existing | `frontend-lint.txt` |
| Frontend tests | `npm --prefix frontend test` | 280 passed / 0 failed (incl. 17 icon tests + 2 new task-10 matrix tests) | `frontend-vitest.txt` |
| Frontend build | `npm --prefix frontend run build` | PASS | `frontend-build.txt` |
| Bundle budget | `npm --prefix frontend run check:budget` | PASS — shell gzip 164.4 kB / 180 kB, total gzip 378.0 kB / 400 kB | `frontend-budget.txt` |

## New tests added by task 10

- `src/server/ops/theme.rs::icon_pack_matrix_coexists_with_theme_and_design_system_selections` —
  Dark appearance + `@clay/theme-gruvbox-material-dark` + `@clay/design-glass` +
  `@clay/icons-phosphor-duotone` all resolve concurrently; pack swap changes geometry
  (duotone shade layer disappears) and leaves theme/design-system/appearance untouched;
  failed pack selection preserves every other resolved state.
- `frontend/src/test/icons.test.tsx` › "embeds no colors in icon output so themes stay the
  sole color authority" — duotone pack with 0.2-opacity layer renders `fill="currentColor"`
  only; zero per-path `fill`/`stroke`/`style`/`class`; opacity survives as geometry, not color.
- `frontend/src/test/icons.test.tsx` › "keeps action and label semantics identical across the
  pack matrix" — fallback → Regular → Duotone: same accessible name ("Save"), same press
  semantics, icon-only glyph swaps underneath.

## Compatibility matrix evidence

| Dimension | Values exercised | Where proven |
| --- | --- | --- |
| Icon pack | Regular, Duotone, partial third-party (`@vendor/partial-icons`), fallback/none | Task 5 op tests (`apply_icon_pack_selects_bundled_packs…`, `…requires_prior_enable…`), Task 4 `tests/icon_packages.rs`, frontend icon-store tests, task-10 semantics test |
| Design system | Neobrutal core fallback, `@clay/design-neobrutal`, `@clay/design-glass` | `apply_design_system_*` tests + new coexistence test |
| Theme / appearance | Light/Dark appearance, explicit `@clay/theme-gruvbox-material-dark` | `apply_theme`/`apply_appearance` tests + new coexistence test |
| Startup with no icon config | Zero init.js icon lines, no loadPackage | All pre-existing config fixtures stay green (1251 lib tests); Regular subset active by default |
| Old text-only fixtures | Pre-Plan-112 package fixtures, git/markdown SDUI trees | Runtime suite 75/75; js_runtime git/markdown tests pass |
| Protocol mismatch | Malformed/unsupported wire snapshots rejected | `tests/runtime_update_protocol.rs::runtime_snapshot_rejects_invalid_active_icon_packs`, DTO fail-closed test |

## Security coverage (deny-case → test)

| Deny case | Test |
| --- | --- |
| Hostile fixture corpus (raw `svg` field, core-key impersonation) | `apply_icon_pack_rejects_hostile_fixture_at_record_time` (record-time + selection-time) |
| Trusted-name spoofing / non-inventory `@clay/` record | `active_icon_pack_requires_compiled_inventory_for_core_keys` (shell) |
| Core keys reserved to first-party inventory | `icon_pack_core_keys_reserved_to_first_party` (record) + `from_record` gate |
| Denied internal ops from package runtime | `op_clay_theme_set_icon_pack` in trusted extension only (97 vs 46 op-count tests); `plan061` admin-op baseline 43 |
| Namespace impersonation in package UI | `component_icon_references_validate_namespace_and_kind` |
| Runtime trees accept core keys only | `runtime_tree_rejects_non_core_icon_references` |
| Approval/adoption path, revocation fallback | `apply_icon_pack_requires_prior_enable_for_third_party_and_revocation_falls_back`; commit-path fallback in `src/server/mod.rs` |
| Stale generation / identity swap | frontend `icon-store.test.ts` (rejects stale generations, same-generation identity swap) |
| Oversized snapshots | `ActiveIconPack::validate` (`IconPackValidationError`), protocol `RuntimeStateSnapshotValidationError::InvalidIconPack`, `runtime_snapshot_rejects_invalid_active_icon_packs` |
| Theme color injection via icons | task-10 "embeds no colors" test + `third_party_raw_color_rejected_at_adopted_boundary…` (existing) |
| Hostile geometry (script/URL fragments, bad arcs, non-finite) | `src/shell/icons.rs` unit corpus (parser/validator rejection tests) |

## Performance

- Payload ceilings enforced in code: per-icon `ICON_GEOMETRY_PAYLOAD_BUDGET_BYTES` (2048 B),
  per-pack `ICON_PACK_PAYLOAD_BUDGET_BYTES` (64 KiB), ≤64 icons/pack — parse + wire + record layers.
- Measured pack sizes (this run): Regular manifest 11,974 B (gzip 2,891 B); Duotone manifest
  15,643 B (gzip 3,623 B); frontend fallback subset 8,851 B (gzip 2,688 B) — all ≪ ceilings.
- Bundle budget: shell 164.4/180 kB gzip, total 378.0/400 kB gzip (build log above) — icon work
  fits inside existing budgets; no budget raised.
- No package JS / IPC / SVG parsing on input or paint paths: geometry is resolved once at pack
  install/selection into the icon store; render is a cached dictionary lookup + static `<path>`
  elements (`frontend/src/state/icon-store.ts`, `frontend/src/components/icon.tsx`); no asset or
  network requests at render time (all geometry inline in manifest/fallback module).
- Pack swap does not recreate EditorView or reset undo/draft/focus: client contract pinned by
  "keeps DOM identity and focus across icon-pack switches" (task 7) and the task-10 semantics
  test; pack state travels only in runtime state snapshots, never as document edits.

## Blocked / unresolved

- Pre-existing (not Plan 112): `documentation_coverage::parity_ledger…` failure (Plan 109 WIP
  commit 1b65568 domain) and 12 frontend lint problems at HEAD baseline. No assertion suppressed,
  no budget inflated, no ledger row fabricated.
