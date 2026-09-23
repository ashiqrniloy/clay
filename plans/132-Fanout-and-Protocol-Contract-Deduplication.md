# 132 — Fanout and Protocol-Contract Deduplication

Source: `code-reviews/2026-09-18-comprehensive-implementation-review.md` items
U1, U2, C5 (review §5, §4). Two deduplications: the five reimplemented
"broadcast + current-value store + lag replay" lanes, and the remaining
hand-mapped Tauri DTO layer. No wire-format or behavior changes.

## Objectives

- U2/C5: one generic `StateFanout<T>` (bounded broadcast + current-value store +
  subscribe + override getter + lag-replay semantics) replaces the five
  hand-rolled lanes (`editor_commands`, `caret_styles`, `editor_layouts`,
  `shell_preferences` in `src/server/js_runtime/mod.rs`; `ActiveTypographyState`
  and the state lanes of `RuntimeGenerationStore` in `src/server/mod.rs`), and
  collapses the repeated `.lock().expect(...)` ladders (20+ in `js_runtime/mod.rs`,
  ~30-line `production_reload` wiring).
- U1: extend the plan-119 ts-rs codegen (DTO → TS) to eliminate the remaining
  hand-maintained mapping inside `src-tauri/src/bridge/dto.rs`, so each protocol
  message family is defined once in Rust and projected, not transcribed.

## Expected Outcome

- One `StateFanout<T>` implementation with unit tests; the five lanes become
  field declarations; connection-loop subscription code unchanged in shape
  (`subscribe_*` methods remain, now delegating).
- `dto.rs` shrinks to projections/renames only (or is generated); a staleness
  check fails CI when Rust DTOs and generated TS drift (extending the existing
  `scripts/check-bindings.sh` pattern).
- All Linux gates green (root + clay-desktop + frontend typecheck/suite).

## Tasks

- [x] Baseline gates and duplication inventory
  - Acceptance Criteria:
    - Functional: `scripts/check.sh` full set + clay-desktop gates pass untouched; record exit codes.
    - Performance: none.
    - Code Quality: record the five lanes' current implementations (file:line spans) and `dto.rs`'s hand-mapped surface (which families still manual).
    - Security: none.
  - Approach:
    - Documentation Reviewed:
      - `code-reviews/2026-09-18-comprehensive-implementation-review.md` §5 (U1, U2), §4 (C5).
      - `plans/119-…` SC-1 task evidence (ts-rs codegen decision: DTO layer, not protocol layer — this plan extends, not revisits).
    - Options Considered:
      - Skip inventory: the lane list is the work order. Recorded.
    - Chosen Approach:
      - Static inventory into task evidence.
    - Files to Create/Edit:
      - None.
    - References:
      - `src/server/js_runtime/mod.rs:57–100`, `src/server/mod.rs:166–304`.
  - Test Cases to Write:
    - None (evidence-recording task).
  - Evidence (2026-09-20/21):
    - Tree: branch `review/1909`, HEAD `36eabd2` ("WIP"), clean `git status`
      (plan targets untouched — no source edits in this task). Environment:
      node v26.8.2, npm 11.19.1; rustup 1.29.1 with rustc/cargo **1.98.1** and
      cargo-audit 0.22.2 installed mid-task (host had **no Rust toolchain** at
      first: `scripts/check.sh full` → exit 127 `cargo: command not found`);
      system deps present (webkit2gtk-4.1 2.52.6, gtk3, librsvg,
      libayatana-appindicator, openssl). Repo has **no `rust-toolchain.toml`**
      and CI floats `dtolnay/rust-toolchain@stable`
      (`.github/workflows/ci.yml:16`).
    - Gates on 1.98.1: `cargo audit` 0; `cargo fmt --check` 0; `cargo check
      --all-targets` 0; **`cargo clippy --all-targets -- -D warnings` exit 101**
      (one lint: `result_large_err` at `src/server/connection/documents.rs:270`
      — `Result<BehaviorVersionDecision, ServerMessage>` with largest variant
      ≥176 bytes); `cargo test --all-targets --quiet` 0 (**1935 passed, 1
      ignored**); `cargo bench --no-run` 0; `scripts/check-bindings.sh` 101
      first run (only `frontend/dist` missing — tauri build-script requirement;
      CI builds the frontend at `.github/workflows/ci.yml:45`) then **0**
      (`webview bindings up to date`) after `npm run build --prefix frontend`
      (0). Frontend: `npm ci` 0, `npm run typecheck` 0, `npm test` 0 — 51 files
      / 497 tests.
    - **Full gate set green end-to-end under the repo-verified toolchain:
      `scripts/check.sh full` with `cargo +1.96.1` → exit 0, "full check
      PASSED", 389s** (all stages including bindings).
    - The 1.98.1 clippy failure is **pre-existing, not a plan-132 regression**:
      the function/signature exists at plan 131's baseline commit `66648e3`, and
      the same tree passes clippy under 1.96.1 — new-stable lint behaviour, the
      only warning in the workspace. House precedent for the fix exists
      (`#[allow(clippy::result_large_err)]` at `src/server/behavior.rs:94,254`;
      size-assertion comments at `src/packages/record/mod.rs:1304`,
      `src/server/ui.rs:3005`).
    - Artifacts: `test-plan/artifacts/132-fanout-dedup/baseline.md` (write-up),
      `baseline-exit-codes.txt`, `baseline-check-full-196.log` (green),
      `baseline-check-full-198.log`, `baseline-stages-rest-198.log`,
      `baseline-clippy-196.log`, `baseline-clippy-198.log`,
      `baseline-check-full-no-toolchain.log`, `baseline-frontend.log`.
    - Re-run notes: add `~/.cargo/bin` to `PATH`; run `npm run build --prefix
      frontend` before the bindings stage.
    - Five-lane inventory (all hand-rolled `broadcast::Sender` + `Arc<Mutex<T>>`
      or `Sender` alone; all survive `production_reload` by handle clone;
      consumer replay/drop policy in `connection/delivery.rs`):
      - `editor_commands` — `src/server/js_runtime/mod.rs:146`, cap 16 `:347`,
        **no current-value store** (advisory: `Delivery::Advice`
        `connection/delivery.rs:129–153`), subscribe `:429–433`, delegation
        `src/server/mod.rs:330–334`, consumer `connection/mod.rs:536`,
        op_state publisher `src/server/ops/mod.rs:199–201` + `:722–749`, call
        site `ops/editor.rs:586`, reload `:292`, wire `:557–559`.
      - `caret_styles` — field+store `js_runtime/mod.rs:151–152`, cap 4 `:349`,
        store init `:350–351`, subscribe `:437–441`, getter (lock ladder)
        `:445–450`, reload `:293–294`, wire `:560–563`, op_state
        `ops/mod.rs:202–214` + `:751–793`, call site `ops/editor.rs:365`,
        delegation `server/mod.rs:338–346`, consumer `connection/mod.rs:541` +
        initial sync `:1271–1277`, `Delivery::State`
        `delivery.rs:155–173`.
      - `editor_layouts` — field+store `:156–157`, cap 4 `:355`, subscribe
        `:453–458`, getter `:462–467`, reload `:295–296`, wire `:564–567`,
        op_state `ops/mod.rs:215–222` + `:795–837`, call site
        `ops/editor.rs:425`, delegation `server/mod.rs:348–362`, consumer
        `connection/mod.rs:545` + sync `:1279–1285`, `Delivery::State`
        `delivery.rs:175–193`.
      - `shell_preferences` — field+store `:161–162`, cap 4 `:359`, subscribe
        `:470–474`, getter `:477–482`, reload `:297–298`, wire `:568–571`,
        op_state `ops/mod.rs:223–231` + `:839–878`, delegation
        `server/mod.rs:364–370`, consumer `connection/mod.rs:548` + sync
        `:1287–1293`, `Delivery::State` `delivery.rs:195–214`.
      - `ActiveTypographyState` — `server/mod.rs:251–297` (current `:253`,
        updates `:254`, cap 16 `:259`, snapshot `:268–270`, subscribe
        `:272–274`, test-only `replace` `:277–296` with validation + revision
        bump); store field `:164`, delegation `:314–322`, consumer
        `connection/mod.rs:530` + sync `:1265–1268`, `Delivery::State`
        `delivery.rs:108–128`.
      - `ActiveRuntimeStateFanout` — `server/mod.rs:170–249`: **not a pure state
        fanout** (per-client install acknowledgements `:174–175`, `:209–242` and
        client-filtered `latest_for` `:194–201`); only publish/subscribe/latest
        is fanout-shaped.
      - Adjacent, out of plan scope: `tab_registry_tx` (`server/mod.rs:621`,
        channel `:784`, consumer `connection/mod.rs:552–554`,
        `delivery.rs:216–234`) has the same replay-from-mutex shape.
    - Lock-ladder scale: `js_runtime/mod.rs` 18 `.lock()` sites (3 lane getters
      `:448/:465/:479`); `ops/mod.rs` 147, of which 14 are the four lanes'
      set/publish pairs `:722–878`. Each `publish_*` takes 2–3 locks per call
      (publisher `Mutex<Option<Sender>>` clone + store
      `Mutex<Option<Arc<Mutex<T>>>>` clone + inner store lock); a
      `StateFanout<T>` clone in op state removes the publisher half.
      `production_reload` `js_runtime/mod.rs:265–302` (38 lines) clones six lane
      handles `:292–298`; `wire_runtime_publishers` `:545–572`.
    - `dto.rs` hand-mapped surface (1,006 lines, 21 TS-derived types, 9 export
      roots `dto.rs:883–915`; `scripts/check-bindings.sh` already guards TS
      drift — the unguarded duplication is Rust-side: a hand-copied DTO keeps
      compiling and generating valid TS when its source type gains a field,
      silently dropping it from the wire). Transcriptions: `TypographySnapshotDto`
      `:177–183` ↔ `ActiveTypography` (`protocol/mod.rs:2639–2652`, 5/5 fields);
      `ComponentRecipeDto` `:250–279` (21 fields) ↔ `ResolvedComponentRecipe`
      (`shell/design_system.rs:1125–1151`); `ShadowLayerDto` `:282–292` ↔
      `ShadowLayer` (`design_system.rs:529–548`, drops `inset`);
      `InnerHighlightDto` `:295–302` ↔ `InnerHighlight` (`:598–603`);
      `DesignSystemVariableValueDto` `:305–321` (15 variants) ↔
      `DesignSystemValue` (`:629–641`, 10) + 5 recipe-derived variants;
      `DesignSystemProvenanceDto` `:239–247` duplicates both
      `DesignSystemProvenance` (`:1326–1331`) and `PackageUiProvenance`
      (`protocol/runtime.rs:74–80`); `PackageUiSnapshotDto` `:650–664` ↔
      `PackageUiSnapshot` (`runtime.rs:53–70`); `PackageSurfaceDto` `:667–674` ↔
      `EmptyTabContent` (`runtime.rs:215–221`, drops `package_name`) /
      `PackageComponentContent` (`:163–169`); `PackagePanelDto` `:678–687` ↔
      `PackagePanelContent` (`:120–128`); `PackageOverlayDto` `:691–699` ↔
      `PackageOverlayContent` (`:141–150`); `InitialDocumentDto` `:201–207` (5
      of 10 fields) ↔ `ClientInitialState` (`client/mod.rs:57–68`);
      `IconGeometryDto`/`IconPathDto` `:563–577` ↔ icon geometry
      (`shell/icons.rs:836+`, `to_d()`/`f32→f64`).
    - Projection bodies that must survive de-duplication:
      `DesignSystemSnapshotDto::resolve` `dto.rs:323–549` (227 lines: validation
      + per-field color-role denial + a second 21-field transcription into the
      `variables` table, 23 insert sites); `PackageUiSnapshotDto::parse`
      `:729–820` (five near-identical copy blocks; real work is
      `serde_json::from_str(component_json)`); `IconPackSnapshotDto::resolve`
      `:578–624`; `RuntimeSnapshotDto::resolve` `:701–727`;
      `InitialDocumentDto::from_initial_state` `:209–220`. Genuine projections
      to keep: `ThemeSnapshotDto` `:62–75` + `editor_style_snapshot` `:86–152`,
      `BootstrapDto` `:29–59`, `RuntimeSnapshotDto` `:628–647`,
      `BridgeEnvelope` `:825–872` (`Event`/`Routed` `ts(skip)` narrowing),
      `BridgeError`.
    - Reference drift recorded (no action): plan cites
      `js_runtime/mod.rs:57–100` (actual `:142–162`) and
      `server/mod.rs:166–304` (actual `:161–297`).

- [x] Introduce `StateFanout<T>` and migrate the four js_runtime lanes
  - Acceptance Criteria:
    - Functional: a `StateFanout<T>` type (tokio broadcast + `Mutex<Option<T>>`/`Mutex<T>` state + subscribe + current + publish) with unit tests for lag-replay semantics (late subscriber gets current value; lagged receiver replays current); `editor_commands`, `caret_styles`, `editor_layouts`, `shell_preferences` become `StateFanout<…>` fields; all `subscribe_*`/`*_override` accessors delegate; `production_reload` shares fanouts by clone, no ladders.
    - Performance: no change in per-message cost (same broadcast primitive); publish path identical allocations.
    - Code Quality: js_runtime/mod.rs loses the 20+ lock ladders on these lanes; clippy clean; no behavior change in connection initial-sync/lag-replay (connection suites green).
    - Security: channel capacities and semantics (advisory drop vs replay) preserved per lane exactly — capacity constants unchanged.
  - Approach:
    - Documentation Reviewed:
      - `src/server/js_runtime/mod.rs` (five lanes + wiring), `src/server/connection/mod.rs` (consumer side: initial sync + lag replay comments per lane).
    - Options Considered:
      - Per-lane wrapper structs: five more copies of the same shape.
      - One generic with explicit replay policy (`Replay::Current` vs `Drop`), capacity in constructor. (Chosen.)
    - Chosen Approach:
      - `struct StateFanout<T> { tx: broadcast::Sender<T>, state: Arc<Mutex<T>> }` — constructors `with_capacity`, `with_initial`; `publish`, `subscribe`, `current`, `update`.
    - API Notes and Examples:
      ```rust
      // src/server/fanout.rs (tentative location)
      pub(crate) struct StateFanout<T: Clone> { … }
      impl<T: Clone> StateFanout<T> {
          pub(crate) fn publish(&self, value: T);      // send + store
          pub(crate) fn subscribe(&self) -> broadcast::Receiver<T>;
          pub(crate) fn current(&self) -> T;            // replay source
      }
      ```
      Delivered shape (task 2): `StateFanout::new(capacity, initial)` for state
      lanes plus a store-free `Fanout::new(capacity)` for the advisory
      `editor_commands` lane; both are `Clone` handles. Task 3 should build
      `StateFanout::new(RUNTIME_STATE_BROADCAST_CAPACITY, snapshot)` for
      typography/runtime-state lanes and keep the extra non-current-value state
      (`ActiveRuntimeStateFanout`'s acknowledgements, `RuntimeGenerationStore`'s
      `current`/`behavior_grace`) outside the fanout.
    - Files to Create/Edit:
      - `src/server/fanout.rs` (new), `src/server/mod.rs` (module line), `src/server/js_runtime/mod.rs` (migrate four lanes + reload wiring), `src/server/ops/mod.rs` (op-state wiring collapses to one handle per lane — added to scope; without it the service would hold fanouts while op state kept the sender/store ladder the task exists to delete).
    - References:
      - Review U2, C5.
  - Test Cases to Write:
    - `late_subscriber_replays_current`: subscribe after publish → initial sync sees latest.
    - `lagged_receiver_replays_current` (the Plan 071 semantics).
    - `capacity_preserved_per_lane`: channel capacity assertions matching current constants (16/4/4/4).
  - Evidence (2026-09-21):
    - Shape delivered: `src/server/fanout.rs` (new, 168 lines) with **two** shapes rather than one policy-parameterized generic — `Fanout<T>` (broadcast only; advisory lanes) and `StateFanout<T>` (broadcast + `Arc<Mutex<T>>` current value; state lanes). Rationale: the advisory lane has no current value, so a single type would need `Option<Arc<Mutex<Option<T>>>>` and make the caret lane's `current()` an `Option<Option<CaretStyle>>` needing `.flatten()` at the accessor. Composition gives `StateFanout<T>` the plan's exact API (`new`, `publish`, `subscribe`, `current`) with no meaningless states; both are `Clone` handles over one channel and one store.
    - Migrated: service fields 8 → 4 (`src/server/js_runtime/mod.rs:146–159`): `editor_commands: Fanout<EditorCommandRequest>`, `caret_styles: StateFanout<Option<CaretStyle>>`, `editor_layouts: StateFanout<Option<WrapPolicy>>`, `shell_preferences: StateFanout<ShellPreferences>`; `production_reload` lane carry-over 8 lines → 4 (`:289–292`); accessors delegate (`subscribe_*` `:420–464`, `caret_style_override` `:435`, `editor_layout_override` `:449`, `shell_preferences` `:461`); `wire_runtime_publishers` 4 handles, no `Arc::clone` (`:539–550`). Op state: 7 `Mutex<Option<…>>` fields → 4 (`src/server/ops/mod.rs:199–219`), `publish_*` now lock once and publish through the guard instead of cloning a sender plus a store handle (`:706–815`).
    - Lock ladders: `js_runtime/mod.rs` 18 → **15** `.lock()` sites (the three lane getters are gone); `ops/mod.rs` 147 → **138**; 12 `.lock()` lines removed and 0 added across both files; `fanout.rs` has one lock site (its `lock()` helper). Each state-lane publish is now 2 locks (outer publisher slot + store) down from 3, and the editor-command lane 1 down from 1 with no clone.
    - Semantics preserved exactly: capacities 16/4/4/4 unchanged; `editor_commands` still advisory (no store, lag drops); the three state lanes still replay the current value on initial sync and lag; `publish_*` still return `false` when unwired (only `publish_editor_command`'s result is read — `ops/editor.rs:586`); store-write-before-send order unchanged; `set_*_publisher` signatures narrowed from `(sender, store)` to one handle.
    - Tests: 4 unit tests in `fanout.rs` — `late_subscriber_replays_current`, `lagged_receiver_replays_current`, `publish_honors_configured_capacity` (exact-overflow probe: capacity 4 → publishing 5 reports `Lagged(1)`; `Receiver::len()` is not capacity-clamped and was rejected), `clones_share_channel_and_state`; plus `lane_channel_capacities_are_preserved` in `src/server/js_runtime/tests.rs` asserting the real lanes: 17 publishes on `editor_commands` → `Lagged(1)` (capacity 16), 5 on each state lane → `Lagged(1)` (capacity 4).
    - Gates: `scripts/check.sh full` under toolchain 1.96.1 → **exit 0, full check PASSED, 260s** (all stages). Under 1.98.1: `cargo test --all-targets` 0 (**1940 passed, 1 ignored**, lib suite 1410 → 1415), `cargo bench --no-run` 0, `scripts/check-bindings.sh` 0, `cargo clippy --all-targets` reports exactly the 1 pre-existing `result_large_err` warning recorded in task 1 (no new lints; `-D warnings` still fails on that pre-existing lint only).
    - Targeted re-checks green: `set_cursor_style_publishes_runtime_caret_override`, `set_editor_layout_publishes_runtime_wrap_override`, `set_pane_focus_policy_publishes_shell_preferences`, `shell_preferences_default_to_click_when_unset`, `editor_control_execute_publishes_gated_known_commands_only`, `lagged_state_lane_writes_the_current_value`, `lagged_advice_lane_writes_nothing`.
    - Diff: `src/server/fanout.rs` new (168 lines), `src/server/mod.rs` +1 module line, `src/server/js_runtime/mod.rs` 88 lines touched, `src/server/ops/mod.rs` 151 lines touched, `src/server/js_runtime/tests.rs` +55.
    - Logs: `test-plan/artifacts/132-fanout-dedup/task2-check-full-196.log`, `…/task2-gates-198.log`, `…/task2-gates-198-first.log`.

- [x] Migrate typography/runtime-state lanes and remaining connection wiring
  - Acceptance Criteria:
    - Functional: `ActiveTypographyState` and the state lanes of `RuntimeGenerationStore` use `StateFanout` where their semantics match (replay-current); generation signaling that is *not* current-value shaped (e.g. `RuntimeGenerationId` notifications) stays as-is with a comment saying why.
    - Performance: none.
    - Code Quality: one pattern for all current-value lanes; connection-loop subscription code compiles unchanged or with mechanical delegation edits; suites green.
    - Security: none.
  - Approach:
    - Documentation Reviewed:
      - `src/server/mod.rs:166–304` (`RuntimeGenerationStore`, `ActiveTypographyState`, fanout structs).
    - Options Considered:
      - Force-fit non-replay lanes: wrong semantics; keep the distinction explicit. (Chosen: migrate only matching lanes.)
    - Chosen Approach:
      - Migrate matching lanes; document the non-matching ones.
    - Files to Create/Edit:
      - `src/server/mod.rs`, `src/server/fanout.rs`.
    - References:
      - Review U2.
  - Test Cases to Write:
    - Existing runtime-state/typography subscription suites green.
  - Evidence (2026-09-21):
    - Delivered: both remaining hand-rolled lanes in `src/server/mod.rs` now take their channel from `fanout` — `ActiveRuntimeStateFanout.updates: Fanout<RuntimeGenerationId>` (`:181`, capacity `RUNTIME_STATE_BROADCAST_CAPACITY` unchanged) and `ActiveTypographyState.updates: Fanout<ActiveTypography>` (`:268`, capacity 16 unchanged); `subscribe` delegates, `send` → `publish` at `:313` (`replace`) and `:1700` (reload commit). `fanout.rs` gained name-only `Debug` impls (`:49`, `:55`) because both wrapper structs derive `Debug` and now hold a lane handle; `Fanout`/`StateFanout` deliberately print no `T` (no `T: Debug` bound, no state leakage).
    - **Deviation, deliberate: the two stores are NOT inside a `StateFanout`**, which is what the acceptance criterion's "where their semantics match" resolves to once the call sites are read:
      - `ActiveTypographyState.current` is held by `IpcServer::commit_runtime_generation` across a six-state conflict check (`behavior`, `sdui`, theme, typography, design system, icon pack at `:1607–1616`) and the broadcast is deliberately deferred until those guards drop (`:1698–1704`). A lane's publish-and-record API cannot express "check under guard, write under guard, send later"; forcing it would either send while the other five guards are held (changing event ordering) or read a snapshot outside the guard (losing conflict detection and letting a concurrent `replace_typography` be silently overwritten). `StateFanout::publish_with` was prototyped for this and removed again — no caller survived the analysis, so no speculative API was left behind.
      - `ActiveRuntimeStateFanout`'s lane carries the generation id while its store holds the snapshot (narrowed per client by `for_client`) and its third field is a per-client acknowledgement map: publish value ≠ channel value ≠ per-client state. Documented in the struct docs.
      - `RuntimeGenerationStore.current` (the live generation) and `tab_registry_tx` (channel value is a snapshot *derived* from the `TabRegistry` mutex, and the registry, not a store copy, is the replay source used by `delivery::tab_registry`) are likewise not current-value lanes; both now say so in place.
    - Code quality: no `broadcast::channel`/`broadcast::Sender` construction remains in the five-lane family — the remaining raw lanes are `agent.rs` events (advisory, `Arc` payloads) and `tab_registry_tx`, both commented and outside plan 132's scope. Connection-loop subscription code compiles unchanged (no edits in `connection/` or `delivery.rs`).
    - Gates: `scripts/check.sh full` under toolchain 1.96.1 → **exit 0, full check PASSED, 252s**. Under 1.98.1: `cargo test --all-targets` 0 (**1940 passed, 1 ignored**; lib 1415), `cargo bench --no-run` 0, `scripts/check-bindings.sh` 0, `cargo clippy --all-targets` reports only the task-1 pre-existing `result_large_err` warning (no new lints). Focused suites green: `typography` 17, `runtime_state` 4, `tab_registry` 18, `reload` 40, `server::fanout` 4.
    - Flake found while gating (pre-existing, not from this change): the `security` integration suite intermittently fails on timing-sensitive process/pipe tests — `language_server_authority::generic_fake_lsp_exit_early_surfaces_typed_sanitized_exit`, `…initialize_and_shutdown_through_process_service` (partial frame read) and `agent_session_isolation::two_workspaces_keep_their_agent_writes_in_their_own_root` (write racing session close). Reproduced **on the stashed baseline tree too (1 failure in 12 runs)** versus 3 in 13 runs with the change applied, so the rate is indistinguishable and the failing tests share no code path with the lanes (they spawn fake LSP/agent children). Two full-gate runs were aborted by it before the green run above; logs kept.
    - Logs: `test-plan/artifacts/132-fanout-dedup/task3-check-full-196.log` (green), `…/task3-check-full-196-flake-protocol.log`, `…/task3-check-full-196-flake-security.log`, `…/task3-gates-198.log`.

- [x] Eliminate hand-mapped DTO transcription in the Tauri bridge (U1)
  - Acceptance Criteria:
    - Functional: every message family currently hand-transcribed in `src-tauri/src/bridge/dto.rs` is either generated from the Rust DTO types via the existing ts-rs pipeline or reduced to a mechanical projection (field rename/wrap) with the mapping expressed as code (serde attributes / From impls) rather than duplicated struct definitions; generated TS under `frontend/src/bridge/generated/` remains the single TS truth.
    - Performance: no runtime cost (codegen-only path; `ts-bindings` feature stays off in shipped builds).
    - Code Quality: `scripts/check-bindings.sh` (or its successor) fails on drift for the migrated families; DTO struct count in `dto.rs` reduced (record before/after).
    - Security: projection boundary unchanged — no new fields cross the bridge; the trust boundary between protocol types and webview DTOs stays explicit.
  - Approach:
    - Documentation Reviewed:
      - `plans/119-…` SC-1 task + its decision log (`2026-09-14` ts-rs-from-DTO approval) — this plan extends the same approved approach, no new decision needed unless a family's shape forces one.
      - `src-tauri/src/bridge/dto.rs`, `frontend/src/bridge/generated/bridge.ts`, `scripts/check-bindings.sh`, `Cargo.toml` `ts-bindings` feature.
    - Options Considered:
      - Hand-fix drift as found: keeps N duplicate definitions.
      - Generate/derive the remaining families with the established pipeline. (Chosen.)
    - Chosen Approach:
      - Family-by-family migration, `check-bindings` green between steps.
    - Files to Create/Edit:
      - `src-tauri/src/bridge/dto.rs` (shrink), `frontend/src/bridge/generated/bridge.ts` (regenerated), `scripts/check-bindings.sh` (extend coverage).
    - References:
      - Review U1; plan 119 SC-1 evidence.
  - Test Cases to Write:
    - Existing bridge staleness/typecheck suites green per step; add a drift test for any family that lacked one.
  - Evidence (2026-09-21):
    - **Mechanism.** Every remaining hand-transcribed family now has exactly one projection site, and the projection is written so that it *cannot* drift silently:
      - *Pure copies become the source type.* `TypographySnapshotDto` is a newtype over `clay::protocol::ActiveTypography` (serde's newtype rule keeps the wire JSON identical; `From<&ActiveTypography>` is one line) — 5-field struct + 5-field `From` body deleted. `DesignSystemProvenanceDto` is deleted outright: both consumers (`DesignSystemSnapshotDto`, `IconPackSnapshotDto`) now carry the shell `DesignSystemProvenance` directly — 1 struct + 2×4-field copy blocks deleted, and the icon-pack site is a plain `.clone()`.
      - *Narrowing families get `From`/`TryFrom` impls whose bodies are **exhaustive destructures** (`let Source { a, b, ..no rest.. } = source;`).* A field added to, renamed in, or removed from the source breaks the build at the mapping, so "silently drops the field from the DTO" (the U1 harm) is now a compile error. Applied to `ResolvedComponentRecipe`, `ShadowLayer` (`inset: _`), `InnerHighlight` (`color_role` → `color`), `IconPath` (`commands` consumed by `to_d()`), `IconGeometry`, `ClientInitialState` (four deliberately-uncrossed fields named `_`), `EmptyTabContent` (`package_name: _`), `PackageComponentContent`, `PackagePanelContent`, `PackageOverlayContent`.
      - *`PackageUiSnapshotDto::parse` (92 lines of five near-identical copy blocks) is now four `TryFrom` impls + one shared `PackageSurfaceDto::parse`, so the empty-tab landing and the named component surfaces cannot drift apart.*
      - *`DesignSystemSnapshotDto::resolve` 227 → ~55 lines*: four near-identical color-denial `if` blocks became one table loop, the 21-field recipe literal + shadow loop + highlight match became `ComponentRecipeDto::from`, and the 24-entry variables table now reads the projected DTO instead of re-deriving `ThemeColorRef`/enum-string conversions from the source a second time.
      - *`IconPackSnapshotDto::resolve`* dropped its per-geometry `Result` plumbing for `From` impls.
    - **Drift guards, three layers.** (1) compile-time: the exhaustive destructures above; (2) wire-level: new assertions in `design_system_snapshot_dto_round_trip_and_variables` compare the projected wire key sets against the source's (`colorRole` → `color`, `inset` dropped are the only deltas) — this covers the one class destructuring cannot see, a *serde* rename on the source; a new `typography_dto_is_a_transparent_projection_of_the_protocol_type` test pins the newtype's JSON equal to the protocol type's; (3) DTO → TS: the existing `scripts/check-bindings.sh` regeneration guard, unchanged (regeneration verified idempotent: `sha256 bridge.ts` 7e7dffe3535f672d before and after a re-run).
    - **Core-type derives added (2), consistent with plan 119's executed evidence** ("plus 71 core-crate serde-JSON types reachable from those roots", `plans/119-…` SC-1): `ActiveTypography` (`src/protocol/mod.rs:2639`) and `DesignSystemProvenance` (`src/shell/design_system.rs:1326`) each gain the same `#[cfg_attr(feature = "ts-bindings", derive(ts_rs::TS))]` + `ts(export_to = "bridge.ts")` pair the DTO layer already requires of every type it references. Note for the record: the 2026-09-14 decision log's "core crates gain no TS-codegen derives" line does **not** match what plan 119 shipped (71 such derives) — this task follows the shipped code, and the wording is worth correcting (see Further Actions).
    - **Contract/TS surface:** `TypographySnapshotDto` keeps its name and field shape (now `export type TypographySnapshotDto = ActiveTypography`), `DesignSystemProvenanceDto` becomes `DesignSystemProvenance`; `frontend/src/bridge/generated/bridge.ts` +24/−4 lines, and the two re-export lists (`frontend/src/bridge/types.ts:58`, `frontend/src/theme/types.ts:11`) follow the rename. No other contract shape changed; `src/protocol`'s rkyv wire is untouched and the ts-rs feature stays off in shipped builds (no runtime cost).
    - **Counts:** `dto.rs` top-level struct/enum count **20 → 19**; lines **1006 → 1091** — the line growth is the destructures (each one *is* the guard) plus the comments explaining each narrowing. Field lists are now written twice at most (the contract struct, plus its single projection impl where one is needed); before, several families wrote them three times (`resolve`/`parse` literals and the variables table re-reading the source), and two families wrote them twice for no reason at all.
    - **Pre-existing rot found and fixed while verifying** (not caused by this task, but blocking its verification): `src-tauri/tests/dto_roundtrips.rs` had not compiled since 2026-09-17 (`ClientMessage::MenuQueryUpdate.scope` landed without the fixture update) and `src-tauri/tests/bridge_session.rs` asserted against a hardcoded `behaviorVersion: 2` while the server now runs behavior version 1 (`EditRejected { InvalidBehaviorVersion }` — diagnosed from the envelope stream). Both are fixed at the source: the fixture now derives `baseVersion`/`behaviorVersion` from the bootstrap, so a version bump cannot rot it again. Reason the rot went unnoticed: `scripts/check.sh full` runs at the workspace root, which selects **only the root `clay` package** — `clay-desktop`'s integration tests are compiled by nothing in the local gate set except `scripts/package-smoke.sh` (CI).
    - **Gates.** Root `scripts/check.sh full` under toolchain 1.96.1: `audit`, `fmt`, `check`, `clippy`, `test` (**1940 passed, 1 ignored**), `bench-compile` all green; the final `bindings` stage exits 1 with `stale webview bindings` — that is the guard correctly reporting an **uncommitted** generated-file change (`git status --porcelain -- frontend/src/bridge/generated` non-empty); the content itself is exactly what the DTO layer produces (regeneration is idempotent, see above) and the stage passes once the regenerated file is committed with the DTO change. One earlier run of this task hit the pre-existing `security`-suite flake (`agent_session_isolation::two_workspaces…`, task-3 evidence); the re-run was green. `clay-desktop` (not covered by `check.sh`): `cargo clippy -p clay-desktop --all-targets -- -D warnings` clean with **and** without `--features ts-bindings`; `cargo test -p clay-desktop --all-targets` green (32 lib + 2 `config_security` + 1 `bridge_session` + 4 `adoption_probe` + 16 `dto_roundtrips`). Frontend: `typecheck` 0, `test` 51 files / 497 tests, `build` ok, `lint` 0, `format:check` clean, `check:budget` 399.9 kB / 404 kB.
    - Logs: `test-plan/artifacts/132-fanout-dedup/task4-check-full-196-rerun.log` (root stages), `…/task4-check-full-196-flake.log`.

- [x] Execute and update the manual test plan (test-plan/)
  - Acceptance Criteria:
    - Functional: internal dedup with no user-visible change — record explicitly which regression modules were re-run (core editing + theme/caret apply paths, since fanout lanes carry those) and that no new steps are needed, with the reason.
    - Performance: none.
    - Code Quality: record per `.agents/skills/create-plan/references/clay.md` explicit-record rule.
    - Security: none.
  - Approach:
    - Documentation Reviewed:
      - `test-plan/index.md`.
    - Chosen Approach:
      - Regression-only pass on caret/typography/theme modules.
    - Files to Create/Edit:
      - None expected.
    - References:
      - None beyond index.
  - Evidence (2026-09-21): regression-only pass, **no new steps** — the change is internal, so the pass re-ran the modules whose behavior the migrated lanes carry and recorded every divergence it found as a *pre-existing client gap*, none of them a lane regression. Fresh Linux build (`target/debug/clay` + `clay-desktop`), new isolated harness at `test-plan/artifacts/132-fanout-dedup/live/` (modelled on the plan 129/130 harnesses; `run-live.sh` modes `caret|caret-invalid|wrap|editing|panes|panes-click|review`, AT-SPI `probe.py`, `pane-focus.sh`, grim `shot.sh`, eleven fixtures). Record: `test-plan/index.md` "Plan 132 fanout/DTO-dedup execution record (2026-09-21, task 5)".
    - **Live PASS:** 04 E1/E8 (real `wtype` keystrokes: `AB` → chars 107→109, caret 0→2; `Ctrl+Z` → 107/0) and 07 T9/T20 (joined ligatures under the 16 px pin); 07 T23/T24 (`columnCap: 40` → centered 40 ch column after a reload); 07 T8 (invalid caret shape → `[editor.invalid_set_cursor_style]`, editor unaffected); the editor-command lane's Advice semantics (init.js's `clientExecuteEditorCommand` dropped before the client subscribes — caret stays 0 — then delivered and executed after `Ctrl+Shift+R` re-runs init.js with the client connected — caret 0→2, `agentProfile.register` 1→2); module 15's theme + `@clay/design-instrument` activation renders styled (workspace chip in gruvbox green, no diagnostics).
    - **Live lane delivery for the migrated StateFanouts was proven by burst capture, not single frames:** after a reload the caret override paints (8 px frame byte-identical to the stashed pre-change build's frame, `sha256 67e737f3…`) and the wrap override applies — i.e. `caret_styles` and `editor_layouts` still deliver live publishes. A single grim frame lands on either blink phase and reads as a false "override not applied"; that false negative cost a stash-and-rebuild attribution test, which is itself the evidence that the behavior matches pre-change.
    - **Findings (pre-existing, need a fix-or-re-document decision — recorded in Further Actions, not fixed here):** (1) the *initial-sync* caret override is dropped by the webview (`frontend/src/editor/extensions/controller.ts:628` returns while `this.view` is null) — reproduced on the stashed pre-change build; (2) unit wrap policies throw in the client's layout handler (`controller.ts:661-672` does `"none" in wrap` on the string serde emits for a unit variant), so `wrapPolicy: "none"`/`"viewport"` never apply; (3) `hollow`/`blink` caret fields are delivered but unread, so module 07 T2/T3/T5 wording is stale; (4) `paneFocusPolicy` has no frontend consumer (module 13 S14/S17 unobservable live; server side pinned by `set_pane_focus_policy_publishes_shell_preferences` + `shell_preferences_default_to_click_when_unset`).
    - **Automated companions (fresh):** `server::fanout` 4, `caret` 6, `typography` 17, `editor_layout` 3, `shell_preferences` 2, `connection::` 96, `clay-desktop --test dto_roundtrips` 16, frontend `vitest run src/theme src/editor src/shell` 173 — all green.
    - Host note: this session *has* `wtype` (real keystrokes into the webview), `hyprctl 'hl.dsp.focus'`/`'hl.dsp.cursor.move'` (focus and pointer without `/dev/uinput`) and grim captures, so plan 129's keyboard/pointer ceilings do not apply here; the only blocked leg is the portal screenshot path (interactive "Allow Apps to Take Screenshots?" prompt).
    - None.

- [x] Create or verify Clay JS APIs for public programmatic surfaces
  - Acceptance Criteria:
    - Functional: no public programmatic surface changed; verify via diff; record "no JS API change".
    - Performance: none.
    - Code Quality: registry/doc-guard suites pass.
    - Security: none.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-execution/references/js-api.md`.
    - Chosen Approach:
      - Verify-only.
    - Files to Create/Edit:
      - None expected.
    - References:
      - Decision log 2026-05-08-1509.
  - Test Cases to Write:
  - Evidence (2026-09-21): **verify-only, no JS API change.** The plan's diff touches no public programmatic surface: `runtime/` (the `clay:*` JS facades, including the lane APIs `clientSetCursorStyle` at `runtime/js/editor.js:38`, `clientSetEditorLayout`, `clientExecuteEditorCommand`, and `setPaneFocusPolicy` at `runtime/js/shell.js:121`), `docs/reference/clay-js-api/` (`api-inventory.toml`, per-API Markdown) and `docs/generated/clay-js-api-registry.json` are all byte-identical to HEAD; the only diff mentions of those API names are doc-comment rewrites on the lane fields (`src/server/js_runtime/mod.rs`, `src/server/ops/mod.rs`). The new `src/server/fanout.rs` is entirely `pub(crate)` and the diff adds no `pub fn`/`pub struct`/`pub enum` anywhere, so decision log 2026-05-08-1509's boundary (internal Rust stays private; public capability must be an op wrapper + JS facade) is respected without new surface.
    - Guard suites (fresh, 1.96.1): `cargo test --test protocol` → 227 passed / 0 failed, which aggregates `clay_js_api_inventory` (schema completeness, `inventory_rust_paths_name_existing_source_files`, `source_paths_named_by_public_metadata_exist`, `public_inventory_docs_index_and_generated_matrix_match_exactly`, `documentation_validation_is_read_only`), `clay_js_doc_registry` (`clay_js_api_inventory_unchanged_or_documented`, canonical-example cross-checks), `clay_js_facade_layout`, `manual_smoke_docs`, `package_loading_docs` and `primitives_docs`. `cargo run --bin update-doc-registry` prints `updated docs/generated/clay-js-api-registry.json` but leaves `git status --porcelain -- docs/` empty, i.e. the checked-in registry is already current (the checker is non-mutating; the updater writes only real drift).
    - **Scope note recorded for the wiki task:** the *webview* contract is not the Clay JS API, and this plan's U1 deliberately renamed one type there (`DesignSystemProvenanceDto` → `DesignSystemProvenance`; `TypographySnapshotDto` is now `ActiveTypography`) — verified by `scripts/check-bindings.sh`, not by the JS API registry. Two wiki pages still carry the old DTO name (`docs/wiki/modules/desktop-typed-bridge.md:33,107`) and must be updated by the next task; `docs/wiki/modules/react-shell.md:18` keeps `TypographySnapshotDto` as the frontend alias (still correct).
    - Log: `test-plan/artifacts/132-fanout-dedup/task6-js-api-verification.txt`.

- [x] Update or verify the code wiki after implementation
  - Acceptance Criteria:
    - Functional: `docs/wiki/modules/desktop-typed-bridge.md` (and the runtime fanout page if separate) reflect `StateFanout` and the generated DTO contract.
    - Performance: notes replay semantics and capacities.
    - Code Quality: linked from `docs/wiki/index.md`.
    - Security: documents the unchanged projection boundary.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-execution/references/docs-as-code.md`; `docs/wiki/modules/desktop-typed-bridge.md`.
    - Chosen Approach:
      - Update once after gates pass.
    - Files to Create/Edit:
      - `docs/wiki/modules/desktop-typed-bridge.md`, fanout page, `docs/wiki/index.md`.
  - Test Cases to Write:
  - Evidence (2026-09-21): manual wiki review done; every acceptance criterion has a page behind it.
    - **Functional — generated DTO contract:** `docs/wiki/modules/desktop-typed-bridge.md` gained a "One projection per family (Plan 132 U1)" subsection (the two shapes, the family table with `TypographySnapshotDto` = `ActiveTypography`, the colour-denial pass, the variables-table-from-projected-DTO rule, the three drift guards, and the statement that the projection boundary did not move), the responsibilities table now names `DesignSystemProvenance` instead of the deleted `DesignSystemProvenanceDto`, the design-system section says the provenance record *is* the shell source type, and the Tests section lists the two new guards (`typography_dto_is_a_transparent_projection_of_the_protocol_type` and the recipe/shadow/inner-highlight wire-key-set check).
    - **Functional — fanout page (created, since the lanes are not the hot-reload page's subject):** `docs/wiki/modules/server-state-fanout.md` (199 lines) documents `Fanout<T>`/`StateFanout<T>` in `src/server/fanout.rs`, the per-lane table (capacity, shape, policy, current value), the `ClayOpState` publisher slots and the `published` bool, the lock order, the four accessors, `production_reload`'s four clones, the connection subscription/initial-sync spans, the per-family `Delivery` split, and the two lanes whose stores deliberately stay outside the fanout with the reason for each.
    - **Performance — replay semantics and capacities:** the lane table plus the invariants list carry the capacities (16/4/4/4/16/16), the drop-vs-replay rule per lane, "store never behind channel", and the `Receiver::len()`-is-not-clamped note that shaped the capacity tests.
    - **Code Quality — linked from the master index:** `docs/wiki/index.md` links the new page (module list + the Plan 119 map line for the connection lanes) and its Desktop Typed Bridge entries now mention the Plan 132 projections; `cargo test --test protocol -- documentation` (21 tests: `wiki_navigation_is_complete_and_current_page_paths_resolve` plus the index-coverage guards) and the full `protocol` suite (227 passed) are green, so the page is indexed and every intra-wiki link resolves.
    - **Security — unchanged projection boundary:** documented in the same bridge subsection (no field added to the wire, `ShadowLayer.inset`/`EmptyTabContent.package_name` still dropped, raw theme overrides never cross, package UI parsed Rust-side, identity stamped Rust-side, new `ts_rs::TS` derives feature-gated) — the pre-existing deny-fields/authority/bounds section is untouched.
    - Related-page sync kept accurate with one-line edits: `server-ipc-skeleton.md` (lane handles are fanout values now), `typography-registry-and-font-roles.md` (`current` + `updates: Fanout<ActiveTypography>` and why the store stays outside), `persistent-runtime-hot-reload.md` (runtime-generation lane = `Fanout<RuntimeGenerationId>`; step 6), `editor-movement-selection-caret.md` (the advisory editor-command lane).
    - Log: `test-plan/artifacts/132-fanout-dedup/task7-wiki-verification.txt`.

## Compromises Made
- **Two types instead of one policy-parameterised type.** The plan's API Notes
  described one generic with an explicit `Replay` policy. The delivered shape is
  `Fanout<T>` (advisory, no store) plus `StateFanout<T>` (store + replay): one
  policy flag would have forced `Option<Option<CaretStyle>>` at the caret
  accessor to express "no override yet". Capacities, value types and per-lane
  semantics are unchanged; documented in `src/server/fanout.rs` and in the task
  2 evidence.
- **Two lanes keep their store outside the fanout.** `ActiveTypographyState`
  (store guard held across the six-state commit conflict check) and
  `ActiveRuntimeStateFanout` (channel carries only a generation id while the
  snapshot is narrowed per client and acknowledged per client) publish through
  `Fanout` but are not `StateFanout`. Converting them would have changed what
  travels on the wire or broken the commit's atomicity; the plan's
  "where semantics match" wording covers it and both structs carry the reason.
- **`dto.rs` grew in lines while shrinking duplication.** 1006 → 1091 lines and
  20 → 19 top-level types, but ~100 hand-copied field assignments became zero:
  each narrowing family is now one exhaustive destructure plus doc comments.
  Line count was the wrong metric; the compile-time drift guard was the right
  one.
- **`scripts/check-bindings.sh` was not extended.** It already regenerates and
  diffs every exported root, which is exactly the DTO→TS guard; the remaining
  source→DTO direction is covered by exhaustive destructuring plus the new
  wire-key-set assertions in `dto_roundtrips.rs`. Adding a second mechanism
  would have duplicated the guard, not strengthened it.
- **The pre-existing `clippy::result_large_err` break on Rust 1.98.1 was left
  unfixed** (`src/server/connection/documents.rs:270`, pre-dating this plan).
  Fixing it is a one-line `#[allow]` or a boxing change outside this plan's
  scope; it is recorded in Further Actions with its CI impact.
- **The manual pass fixed no client gaps it found.** The four divergences
  (initial-sync caret drop, unit wrap policies throwing, unread
  `hollow`/`blink`, unconsumed `paneFocusPolicy`) are pre-existing webview
  behavior; changing them would have broken the plan's no-user-visible-change
  boundary, so they are recorded in Further Actions instead.
- **The regenerated `frontend/src/bridge/generated/bridge.ts` is uncommitted in
  the working tree**, which makes `scripts/check-bindings.sh` report "stale"
  until it is committed with the DTO change. The guard's input is `git status`,
  so this is structural, not a defect; regeneration was proven idempotent.

## Further Actions

Reviewed 2026-09-21 after task 7. Each item is marked **Resolved** (fixed in
place, evidence below) or **Deferred** (carried into a numbered plan).

- **Resolved — 1.98.1 clippy break (`clippy::result_large_err`,
  `src/server/connection/documents.rs:270`).** `#[allow(..., reason = "...")]`
  added, matching the `src/server/behavior.rs:94,254` precedent. `cargo clippy
  --all-targets -- -D warnings` is green on 1.98.1.
- **Resolved — a second 1.98.1 break the new desktop gate stage then caught
  (`clippy::useless_borrows_in_formatting`, `src-tauri/src/commands.rs:156`).**
  The `&` was removed; `cargo clippy -p clay-desktop --all-targets -- -D
  warnings` is green. This is exactly the rot the gate gap hid.
- **Resolved — gate gap that hid the rot.** `scripts/check.sh full` now runs
  `cargo clippy -p clay-desktop --all-targets -- -D warnings` and `cargo test -p
  clay-desktop --all-targets --quiet` after the root stages (before the
  bindings stage, which owns the codegen feature flavour). Warm cost is ~25 s
  clippy + ~30 s tests. The stage immediately caught the commands.rs lint.
- **Resolved — misleading bindings-guard message.** `scripts/check-bindings.sh`
  now records the path status *before* regenerating and distinguishes
  "uncommitted webview bindings … differs from HEAD" (content correct, commit
  missing) from real staleness. Exit code unchanged: the guard still fails for
  any uncommitted contract change, by design.
- **Resolved — commit the regenerated bindings with the DTO change.** No
  separate work item: the guard compares against `HEAD`, so the only action is
  committing `frontend/src/bridge/generated/bridge.ts` together with the DTO
  change (regeneration is idempotent; content verified). Recorded in
  `## Compromises Made`; the improved guard message now says so on failure.
- **Resolved — re-run the baseline gate set.** Re-run on 1.98.1 after the lint
  fixes and the new gate stages (`further-actions-check-full-198-attempt3.log`,
  EXIT=1 ELAPSED=414s): audit, fmt, check, clippy, test, bench-compile, and the
  two new `clay-desktop` stages are all green; the only red is the bindings
  stage, which fails with the corrected message ("uncommitted webview bindings
  … differs from HEAD") purely because the regenerated
  `frontend/src/bridge/generated/bridge.ts` is not committed yet. Earlier
  attempts hit the pre-existing flakes instead (`…-flake-security.log`,
  `…-rerun-flake-security.log`): the security races, and a new finding — the
  UI-review harness can block indefinitely on a stuck desktop portal, which is
  now in plan 148 (task 1 item d, task 2).
- **Resolved — four pre-existing client gaps found by the manual pass.** Not
  fixed here (they are webview behavior, not lane delivery): carried into
  `plans/147-Client-Lane-Gaps-Caret-Wrap-and-Pane-Focus.md` as tasks 2-4, with
  the initial-sync caret drop, the `"none" in wrap` throw, the unread
  `heightPct`/`hollow`/`blink`/`stopBlinkOnTyping` fields, and the missing
  `paneFocusPolicy` consumer all reproduced and inventoried there.
- **Resolved — method note for future manual passes.** The burst-capture rule
  for blink/caret evidence is now in `test-plan/index.md` → `## Conventions`, so
  a later pass reads it before judging a single frame.
- **Deferred — security integration suite flakiness.** Carried into
  `plans/148-Gate-Hygiene-Flake-Determinism-and-Toolchain-Pinning.md` task 2
  (unique per-test roots, explicit close/write ordering, collision-free daemon
  paths, 20-run verification loop). Baseline and mechanism in that plan's task 1.
  The gate review also found a second, different gate failure mode worth fixing
  there: the UI-review harness test can block indefinitely on a stuck desktop
  portal (no timeout on the portal capture step), which hung the protocol suite
  for 304 s and would hang `cargo test --all-targets` forever.
- **Deferred — toolchain pinning decision.** Carried into plan 148 task 3
  (`rust-toolchain.toml` vs an explicit CI version vs a recorded decision to
  float `stable`), which requires a decision log and user approval.
- **Deferred — correct the 2026-09-14 SC-1 decision log's boundary wording.**
  Carried into plan 148 task 4 as an errata amendment (three claims at lines 23,
  104, 143-144 contradict the 73 shipped feature-gated derives); needs explicit
  user approval per the decision-log skill.
- **No action — `ActiveRuntimeStateFanout` / `ActiveTypographyState` stores stay
  hand-rolled.** Rationale now lives in the struct doc comments and in
  `docs/wiki/modules/server-state-fanout.md`; revisit only if the commit path is
  restructured to publish per-state instead of transactionally.
- **No action — `tab_registry_tx` stays a raw broadcast lane.** Channel value is
  derived from the `TabRegistry` mutex (the replay source), documented in the
  same wiki page; converting the sender would touch ~20 mechanical call sites for
  a type swap.
- **No action — DTO narrowings preserved.** `ShadowLayer.inset` drop,
  `EmptyTabContent.package_name` drop, `component_json` → `serde_json::Value`
  parse, design-system validation + colour-role denial, and the generated-TS
  staleness guard are all recorded in task 4 evidence and in the wiki page; the
  new wire-key-set test pins the largest family.
