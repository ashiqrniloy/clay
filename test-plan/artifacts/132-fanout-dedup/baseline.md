# Plan 132 — baseline gates and duplication inventory (2026-09-20/21)

Task: "Baseline gates and duplication inventory" (plan 132 task 1). No source
edits made before this run.

- Tree: branch `review/1909`, HEAD `36eabd2` ("WIP"), `git status --porcelain`
  empty (clean — plan targets untouched). Plan 129/131 evidence was recorded on
  HEAD `66648e3` with a dirty tree; that no longer applies.
- Environment: node v26.8.2, npm 11.19.1. Rust toolchain installed mid-task
  (rustup 1.29.1, rustc/cargo **1.98.1**, cargo-audit 0.22.2 installed locally);
  system deps present (webkit2gtk-4.1 2.52.6, gtk3, librsvg, libayatana-appindicator,
  openssl). The repo has **no `rust-toolchain.toml`** and CI floats
  `dtolnay/rust-toolchain@stable` (`.github/workflows/ci.yml:16`), so the gate
  set is toolchain-sensitive — see the 1.98.1 finding below. Plan 131's baseline
  used rustc/cargo 1.96.1.

## Gate result

Phase 1 (no toolchain on host): `scripts/check.sh full` → exit 127,
`cargo: command not found` at stage `audit`; frontend gates runnable and green
(`npm ci` 0, `npm run typecheck` 0, `npm test` 0 — 51 files / 497 tests).

Phase 2 (toolchain installed):

| Stage | Command | Result |
| --- | --- | --- |
| audit | `cargo audit` | exit 0 (9 allowed warnings) |
| fmt | `cargo fmt --check` | exit 0 |
| check | `cargo check --all-targets` | exit 0 |
| clippy (1.98.1) | `cargo clippy --all-targets -- -D warnings` | **exit 101** — one lint, `result_large_err` at `src/server/connection/documents.rs:270` |
| test | `cargo test --all-targets --quiet` | exit 0 — 1935 passed, 1 ignored |
| bench compile | `cargo bench --no-run --quiet` | exit 0 |
| bindings | `scripts/check-bindings.sh` | exit 101 first run (only `frontend/dist` missing — tauri build script requires it); exit 0 after `npm run build --prefix frontend` (`webview bindings up to date`) |
| frontend build | `npm run build --prefix frontend` | exit 0 |
| **full script, toolchain 1.96.1** | `scripts/check.sh full` (via `cargo +1.96.1` shim) | **exit 0 — full check PASSED, 389s** (all stages, bindings included) |

Raw logs: `baseline-check-full-196.log` (green end-to-end),
`baseline-check-full-198.log`, `baseline-stages-rest-198.log`,
`baseline-clippy-196.log`, `baseline-clippy-198.log`,
`baseline-check-full-no-toolchain.log`, `baseline-frontend.log`; exit codes:
`baseline-exit-codes.txt`.

**Pre-existing failure on new stable (1.98.1), not a plan-132 regression:**
clippy `result_large_err` — `src/server/connection/documents.rs:270` returns
`Result<BehaviorVersionDecision, ServerMessage>`, and `ServerMessage`'s largest
variant is ≥176 bytes (`CompletionResult`, `LanguageIntelligenceResult`,
`ActiveTypography` ≥160). The function and signature already existed at plan
131's baseline commit `66648e3`, and the **same tree passes clippy under
1.96.1**, so the lint is new-stable behaviour. It is the only warning in the
whole workspace (`baseline-clippy-198.log`). Because CI floats `@stable` and
there is no toolchain pin, CI is red on 1.98.1 until this is addressed; the
repo already has house precedent for the fix (`#[allow(clippy::result_large_err)]`
at `src/server/behavior.rs:94,254`, with size-assertion comments at
`src/packages/record/mod.rs:1304`, `src/server/ui.rs:3005`).

Notes for re-runs: the frontend must be built (`npm run build --prefix frontend`)
before `scripts/check-bindings.sh`; CI does this at
`.github/workflows/ci.yml:45`. Adding `~/.cargo/bin` to `PATH` is required
(rustup installs there and it is not on the default `PATH`).

## A. Current-value fanout lanes

Five in-plan lanes plus one adjacent lane, all hand-rolled as
`broadcast::Sender` + `Arc<Mutex<T>>` (or `Sender` alone) + subscribe + getter.
All lanes share: bounded channel, survives `production_reload` by handle clone,
consumer-side replay-or-drop policy in `connection/delivery.rs`.

| Lane | Value type | Cap. | Store | Publisher wiring (op_state) | Publish call site | Consumer policy |
| --- | --- | --- | --- | --- | --- | --- |
| `editor_commands` | `EditorCommandRequest` | 16 | none (advisory) | `ops/mod.rs:199–201`, `set_editor_command_publisher` `:722–733`, `publish_editor_command` `:735–749` | `ops/editor.rs:586` | `Delivery::Advice` `delivery.rs:129–153` |
| `caret_styles` | `Option<CaretStyle>` | 4 | `caret_style_state` | `ops/mod.rs:202–214`, `set_caret_style_publisher` `:751–767`, `publish_caret_style_override` `:769–793` | `ops/editor.rs:365` | `Delivery::State` `delivery.rs:155–173` |
| `editor_layouts` | `Option<WrapPolicy>` | 4 | `editor_layout_state` | `ops/mod.rs:215–222`, `set_editor_layout_publisher` `:795–811`, `publish_editor_layout_override` `:813–837` | `ops/editor.rs:425` | `Delivery::State` `delivery.rs:175–193` |
| `shell_preferences` | `ShellPreferences` | 4 | `shell_preferences_state` | `ops/mod.rs:223–231`, `set_shell_preferences_publisher` `:839–854`, `publish_shell_preferences` `:856–878` | `ops/mod.rs` (`setPaneFocusPolicy` path) | `Delivery::State` `delivery.rs:195–214` |
| `ActiveTypographyState` | `ActiveTypography` | 16 | `current` | n/a (store lives in `server/mod.rs`) | `replace` (test-only) | `Delivery::State` `delivery.rs:108–128` |
| `ActiveRuntimeStateFanout` | `RuntimeStateSnapshot` (latest) + `RuntimeGenerationId` (updates) | `RUNTIME_STATE_BROADCAST_CAPACITY` | `latest` + per-client `acknowledgements` | n/a | `publish` | `Delivery::State` `delivery.rs:236–267` |
| `tab_registry_tx` (adjacent, **not in plan scope**) | `TabRegistrySnapshot` | — | `tab_registry` mutex | n/a | `server/mod.rs:1873` | `Delivery::State` `delivery.rs:216–234` |

Exact spans:

- `src/server/js_runtime/mod.rs` (`ClayJsRuntimeService`, 1,705 lines):
  - fields `:142–162` (`editor_commands` `:146`, `caret_styles`+store `:151–152`,
    `editor_layouts`+store `:156–157`, `shell_preferences`+store `:161–162`);
  - `production_reload` `:265–302` (38 lines), lane clones `:292–298`;
  - fresh construction `:345–362` (channels `:347/:349/:355/:359`, stores
    `:350–351/:356–357/:360–362`), struct init `:379–385`;
  - accessors `:429–482` (`subscribe_editor_commands` `:429–433`,
    `subscribe_caret_styles` `:437–441`, `caret_style_override` `:445–450`,
    `subscribe_editor_layout` `:453–458`, `editor_layout_override` `:462–467`,
    `subscribe_shell_preferences` `:470–474`, `shell_preferences` `:477–482`);
  - `wire_runtime_publishers` `:545–572` (lane wiring `:557–571`);
  - `wire_domain_lanes` `:574–600` (wires both domains × 2 lanes through the
    same publisher set).
  - `.lock()` sites in this file: 18; the three lane getters at `:448`, `:465`,
    `:479` are the ladder sites plan 132 removes.
- `src/server/ops/mod.rs` (`ClayOpState`, 2,434 lines): publisher/store fields
  `:198–231` (7 `Mutex<Option<…>>` fields), initialized `:407–413`; set/publish
  pair per lane `:722–878` — 14 `.lock()` sites for these four lanes out of 147
  in the file. Each `publish_*` takes 2–3 locks per call (outer publisher
  `Mutex<Option<Sender>>` clone, store `Mutex<Option<Arc<Mutex<T>>>>` clone,
  then inner store lock); a `StateFanout<T>` clone in op state collapses the
  publisher half to zero locks.
- `src/server/mod.rs`:
  - `RuntimeGenerationStore` `:161–168` (fields `current`, `typography`,
    `runtime_state`, `behavior_grace`);
  - `ActiveRuntimeStateFanout` `:170–249` (latest `:172`, updates `:173`,
    acks `:174–175`, `Default` cap `:179–187`, `subscribe` `:190–192`,
    `latest_for` `:194–201`, `publish` `:203–207`, `note_installed` `:209–235`,
    `acknowledged_generation` `:237–242`);
  - `ActiveTypographyState` `:251–297` (current `:253`, updates `:254`, cap 16
    `:259`, `snapshot` `:268–270`, `subscribe` `:272–274`, test-only `replace`
    `:277–296` with validation + revision bump + dedupe);
  - delegation layer `:314–370` (`active_typography` `:314–316`,
    `subscribe_typography` `:318–322`, `subscribe_runtime_state` `:324–326`,
    four lane delegations `:330–370`);
  - `reload_runtime_generation_inner` `:1191–1200+` calls
    `ClayJsRuntimeService::production_reload` `:1200`.
- Consumer side (`src/server/connection/mod.rs`): subscriptions `:530–548`;
  initial sync `:1265–1293` (`ActiveTypography` `:1265–1268`, caret
  `:1271–1277`, layout `:1279–1285`, shell `:1287–1293`);
  `Delivery` policies `delivery.rs:108–214`.
- Semantics per lane (preserve exactly): `editor_commands` has **no current
  value** (advice; lag drops, `delivery.rs:129–153`); the other four replay the
  current value on lag and at initial sync. `ActiveRuntimeStateFanout` is not a
  pure state fanout: it carries per-client install acknowledgements and a
  client-filtered `latest_for`, so only its publish/subscribe/latest part is
  fanout-shaped.

## B. `src-tauri/src/bridge/dto.rs` hand-mapped surface

1,006 lines, 21 TS-derived types, all exported through one
`export_webview_contract_bindings` codegen test (`dto.rs:883–915`, 9 roots;
ts-rs pulls referenced protocol types transitively into
`frontend/src/bridge/generated/bridge.ts`, 609 lines). `scripts/check-bindings.sh`
already fails on generated-TS drift, so **TypeScript** duplication is guarded.
The unguarded duplication is Rust-side: a hand-copied DTO struct keeps
compiling (and keeps generating valid TS) when its source type gains a field,
silently dropping it from the wire.

Families that are field-for-field transcriptions of a Rust source type
(migration targets):

| dto.rs DTO | Fields | Source type | Delta (only real projection) |
| --- | --- | --- | --- |
| `TypographySnapshotDto` `:177–183` | 5 | `ActiveTypography` `protocol/mod.rs:2639–2652` | none; `From<&ActiveTypography>` `dto.rs:185–197` |
| `ComponentRecipeDto` `:250–279` | 21 | `ResolvedComponentRecipe` `shell/design_system.rs:1125–1151` | `ThemeColorRef.0` unwrap + enum `as_str()` (source already serializes that way); `shadow` skips `ShadowLayer.inset` |
| `ShadowLayerDto` `:282–292` | 6 | `ShadowLayer` `design_system.rs:529–548` | drops `inset` (deliberate narrowing) |
| `InnerHighlightDto` `:295–302` | 3 | `InnerHighlight` `design_system.rs:598–603` | color-role unwrap |
| `DesignSystemVariableValueDto` `:305–321` | 15 variants | `DesignSystemValue` `design_system.rs:629–641` (10) | +5 recipe-derived variants (`ThemeColorRole`, `SpacingToken`, `Shadow`, `InnerHighlight`, `OutlineStyle`) |
| `DesignSystemProvenanceDto` `:239–247` | 4 | `DesignSystemProvenance` `design_system.rs:1326–1331` **and** `PackageUiProvenance` `protocol/runtime.rs:74–80` | three identical 4-field structs exist; the DTO is used for design-system and icon-pack provenance while package-UI DTOs reuse `PackageUiProvenance` |
| `PackageUiSnapshotDto` `:650–664` | 7 | `PackageUiSnapshot` `protocol/runtime.rs:53–70` | `component_json` parsed to `serde_json::Value` |
| `PackageSurfaceDto` `:667–674` | 4 | `EmptyTabContent` `runtime.rs:215–221` / `PackageComponentContent` `runtime.rs:163–169` | drops `package_name`; parses `component_json` |
| `PackagePanelDto` `:678–687` | 6 | `PackagePanelContent` `runtime.rs:120–128` | parses `component_json` |
| `PackageOverlayDto` `:691–699` | 7 | `PackageOverlayContent` `runtime.rs:141–150` | parses `component_json` |
| `InitialDocumentDto` `:201–207` | 5 of 10 | `ClientInitialState` `client/mod.rs:57–68` | drops client_id/behavior_manifest/theme/typography; renames `document_version`→`version` |
| `IconGeometryDto`/`IconPathDto` `:563–577` | 2/2 | `ActiveIconPack` geometry `shell/icons.rs:836+` | `IconPath::to_d()`, `f32`→`f64` |

Projection bodies that would remain after de-duplication (they carry the
validation/narrowing that must not be lost):

- `DesignSystemSnapshotDto::resolve` `dto.rs:323–549` (227 lines): validation +
  color-role denial per field + recipe copy + a **second** transcription of all
  21 recipe fields into the `variables` table (23 `variables.insert` sites).
- `PackageUiSnapshotDto::parse` `dto.rs:729–820` (92 lines): five near-identical
  copy blocks that differ only in field list; the real work is
  `serde_json::from_str(component_json)` (trust-boundary parse).
- `IconPackSnapshotDto::resolve` `:578–624`, `RuntimeSnapshotDto::resolve`
  `:701–727`, `InitialDocumentDto::from_initial_state` `:209–220`.

Genuine projections (not transcriptions; keep as-is): `ThemeSnapshotDto`
`:62–75` + `editor_style_snapshot` `:86–152` (37 token styles resolved through
`StyleRegistry`; raw overrides never cross), `BootstrapDto` `:29–59`
(composition), `RuntimeSnapshotDto` `:628–647` (composition), `BridgeEnvelope`
`:825–872` (envelope; `Event`/`Routed` `ts(skip)` narrowing), `BridgeError`
(`bridge/errors.rs`).

## Consequences for later plan-132 tasks

- The `StateFanout<T>` constructor must support the `editor_commands` advisory
  shape (send-only, no current value) as well as replay-current; capacities
  differ (16/4/4/4/16) and must stay per-lane constants.
- `ActiveRuntimeStateFanout` cannot be migrated wholesale: acknowledgements and
  client-filtered `latest_for` stay.
- DTO de-duplication must preserve: `ShadowLayer.inset` drop, `EmptyTabContent.package_name`
  drop, `component_json`→`Value` parse, design-system validation + color-role
  denial, and the generated-TS staleness guard.
- Plan references `src/server/js_runtime/mod.rs:57–100` and
  `src/server/mod.rs:166–304`; actual spans are `:142–162` and `:161–297` (drift
  recorded; no action needed).
