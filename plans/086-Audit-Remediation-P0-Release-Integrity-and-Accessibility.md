# Audit Remediation P0: Release Integrity and Accessibility

Prerequisites: Plan 085 work may remain uncommitted. Preserve all current work; establish actual current behavior before editing. This plan blocks Plans 087–091.

Source review: `code-reviews/2026-08-14-comprehensive-implementation-and-ui-ux-review.md` P0-1 through P0-3, security hardening, and test gaps 1 and 3.

## Remediation Program Plan Set

| Audit area | Owning plan |
|---|---|
| P0 accessibility crash, rkyv advisories, red/hanging tests, dependency-warning triage | Plan 086 (this plan) |
| P1 default entry UX, completion geometry, repeatable screenshot/accessibility harness | Plan 087 |
| P1 broad Masonry visual modernization with existing theme configurability preserved | Plan 088 |
| P1 feedback loop, P2 performance/security generated cases, multi-window/DPI/Wayland coverage | Plan 089 |
| P2 responsibility-based refactor, UI ownership map, source-test simplification | Plan 090 |
| Post-stability Xilem compatibility evaluation | Plan 091 |

Execute in numeric order except Plan 089's isolated workflow work may overlap Plans 087–088 after Plan 086 is green. No later plan may waive an earlier blocking gate.

## Audit Coverage

- P0-1 and P2-2: stable virtual accessibility IDs, one construction helper, real AccessKit/AT-SPI lifecycle coverage.
- P0-2 and security: rkyv patch upgrade, malformed archive coverage, dependency-warning triage.
- P0-3: preserve current fixes for the canonical configuration test and Control Center hangs, then add bounded regression checks and complete one serial Linux gate.

## Objectives

- Eliminate the real AT-SPI startup crash without disabling accessibility.
- Remove all fixed direct RustSec vulnerabilities from Clay's IPC codec path.
- Make current configuration and Control Center regressions bounded, hermetic, and repeatably green.
- Restore a release-blocking Linux verification baseline before product/refactor work resumes.

## Expected Outcome

- Clay starts with AT-SPI enabled and survives repeated tab, pane, status, menu, and announcement updates without AccessKit consumer rejection.
- All virtual accessibility nodes have deterministic, collision-resistant identities and one shared construction policy.
- `rkyv >= 0.8.17` is locked; malformed archives fail closed; `cargo audit` reports no unignored vulnerabilities.
- The three previously failing/hanging focused tests finish under explicit deadlines, and one serial full Linux gate passes.

## Tasks

- [x] Reconfirm the dirty-branch baseline and lock P0 scope
  - Acceptance Criteria:
    - Functional: Reproduce the accessibility crash with AT-SPI enabled and record exact logs; rerun the three named configuration/Control Center tests. Current focused evidence (2026-08-14) is that all three now pass in 0.03–0.07 s, so preserve those fixes and do not duplicate them.
    - Performance: Record startup time and accessibility-update cost only as entry evidence; add no runtime work.
    - Code Quality: Inventory every `WidgetId::next()` virtual-node call, codec decode boundary, affected test fixture, and current dirty file before implementation.
    - Security: Confirm no workaround disables AT-SPI, skips archive validation, broadens IPC access, or adds a RustSec ignore.
  - Approach:
    - Documentation Reviewed:
      - `code-reviews/2026-08-14-comprehensive-implementation-and-ui-ux-review.md` P0 findings.
      - `docs/development/accessibility.md`, `docs/development/security.md`, `docs/development/build-and-test.md`.
      - Project patterns: `planning-checklist.md`, `protocol-and-performance.md`, `authority-boundaries.md`, `maintenance-validation.md`, `ui-visual-review.md`.
    - Options Considered:
      - Assume audit-time failures still describe current source: rejected; Plan 085 already appears to include hermetic config-root fixes.
      - Reproduce first and treat passing fixes as protected baseline: chosen; avoids rewriting current work.
    - Chosen Approach:
      - Capture process logs and focused commands, then scope edits only to still-open failures and missing regression bounds.
    - API Notes and Examples:
      ```bash
      cargo test --lib server::runtime_generation_tests::example_configuration_loads_cleanly_and_applies_effects -- --exact
      cargo test --lib server::connection::tests::control_center_opens_filters_activates_and_cancels -- --exact
      cargo test --lib server::connection::tests::runtime_generation_replacement_cancels_open_control_center -- --exact
      cargo audit
      ```
    - Files to Create/Edit:
      - `plans/086-Audit-Remediation-P0-Release-Integrity-and-Accessibility.md`: Record entry-gate evidence.
    - References:
      - `src/server/mod.rs:3002`, `src/server/connection.rs:7468`, `src/server/connection.rs:7797`.
      - Audit screenshots/log evidence under `code-reviews/screenshots/2026-08-14-clay-audit/`.
  - Test Cases to Write:
    - Entry gate: focused tests complete under an outer 5-second deadline and use isolated configuration roots.

- [x] Review Clay UI and accessibility primitives before changing virtual nodes
  - Acceptance Criteria:
    - Functional: Inventory real child nodes and all synthetic TabList/Tab/Status/Menu nodes, their parents, bounds, lifetimes, and update triggers; prove which IDs must survive incremental updates.
    - Performance: Construction remains bounded by visible tabs/menu items and runs only during accessibility passes.
    - Code Quality: Reuse the retained-region ID scheme from `masonry_package_region`; propose one generic helper, not per-widget ID tricks or a new framework.
    - Security: Accessibility labels retain basename/path sanitization and current 64/256-character ceilings.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-ui/SKILL.md`, `references/components.md`, `references/tokens.md`.
      - `npx ui-skills get ibelick/fixing-accessibility` guidance: stable semantics, names, keyboard/focus, dialogs, announcements.
      - Local rustdocs: `/tmp/clay-plan-doc-target/doc/accesskit/index.html`, `/tmp/clay-plan-doc-target/doc/masonry/index.html` for access-tree and widget contracts.
      - `docs/development/accessibility.md`; `src/masonry_package_region.rs::stable_menu_a11y_node_id`.
    - Options Considered:
      - Disable accessibility or omit virtual nodes: rejected; removes required product behavior.
      - Keep calling `WidgetId::next()` per pass: rejected; identities churn and reproduced consumer validation failure.
      - Stable IDs derived from retained widget identity plus typed slot/key: chosen.
    - Chosen Approach:
      - Define typed virtual-node namespaces and derive deterministic IDs from the owning retained widget and stable slot/item key, with collision tests against real widget IDs.
    - API Notes and Examples:
      ```rust
      enum VirtualA11ySlot { TabList, Tab(u64), Status, Announcement, MenuCount }
      fn virtual_node_id(owner: WidgetId, slot: VirtualA11ySlot) -> NodeId;
      ```
    - Files to Create/Edit:
      - `src/editor/accessibility.rs` or a small `src/accessibility.rs` (choose existing shared owner): stable ID/helper policy.
      - `src/masonry_shell.rs`, `src/masonry_editor.rs`, `src/masonry_pane_document.rs`, `src/masonry_package_region.rs`: use shared policy.
      - `.agents/skills/clay-ui/references/components.md`: record internal virtual-node identity contract if changed.
    - References:
      - `.agents/skills/project-patterns/references/ui-visual-review.md`.
      - `decision-logs/2026-08-14-0200-mandatory-ui-visual-and-accessibility-review.md`.
  - Test Cases to Write:
    - Stable identity inventory: same owner/state yields same IDs across repeated updates; distinct owners/slots/items never collide.

- [x] Implement stable virtual accessibility nodes and consumer-valid incremental updates
  - Acceptance Criteria:
    - Functional: Replace every ephemeral virtual ID in shell/editor/pane status paths; repeated add/remove/reorder/update operations produce attached trees accepted by `accesskit_consumer`.
    - Performance: No per-pass global ID allocation; tab/menu work remains O(visible items), bounded by existing caps.
    - Code Quality: One helper owns namespace, derivation, parent attachment, and bounds conventions; no public API or package-facing surface is added.
    - Security: Labels remain sanitized; inactive tabs stay unreachable; modal menu containment and announcement semantics remain unchanged.
  - Approach:
    - Documentation Reviewed:
      - AccessKit 0.21.1 and `accesskit_consumer` 0.31.0 local registry source, especially consumer tree update validation.
      - Masonry 0.4.0 local rustdoc/source for `AccessCtx::tree_update`, `WidgetId`, `NodeId`, and `Node::set_children`.
    - Options Considered:
      - Persist allocated IDs as fields in each widget: valid but duplicates policy and storage.
      - Deterministic namespaced derivation: chosen; existing retained owner IDs already provide lifetime identity.
    - Chosen Approach:
      - Generalize the existing menu derivation and migrate all virtual nodes atomically, preserving tree order and exact labels.
    - API Notes and Examples:
      ```rust
      let status_id = virtual_node_id(ctx.widget_id(), VirtualA11ySlot::Status);
      ctx.tree_update().nodes.push((status_id, status));
      node.set_children([status_id]);
      ```
    - Files to Create/Edit:
      - Same source files identified in the primitive review.
      - `docs/development/accessibility.md`: deterministic identity and consumer-validation contract.
    - References:
      - `src/masonry_shell.rs:2152-2220`, `src/masonry_editor.rs:1250-1290`, `src/masonry_pane_document.rs:3351-3380`, `src/masonry_package_region.rs:91-98`.
  - Test Cases to Write:
    - Build initial tree, then apply unchanged, tab-add, tab-reorder, tab-remove, selected-tab, announcement, status, menu-query, menu-selection, and menu-close updates through `accesskit_consumer::Tree` without panic.
    - Inactive pane nodes and stale removed virtual nodes are absent from reachable tree.

- [x] Add a real Linux AT-SPI accessibility regression check
  - Acceptance Criteria:
    - Functional: Launch an isolated server/client with desktop accessibility enabled, observe a live Clay window, exercise representative updates, and fail if process exits or accessibility tree cannot be queried.
    - Performance: Smoke check has a fixed deadline and cleanup; it does not run in ordinary unit-test hot paths.
    - Code Quality: Extend existing GUI smoke/config-fixture machinery; do not create a second app launcher or screenshot framework.
    - Security: Use user-owned mode-700 temporary IPC/config directories; never connect to ambient user server/config or retain document contents in logs.
  - Approach:
    - Documentation Reviewed:
      - `docs/development/launch-and-gui-smoke.md`, `docs/development/ui-observability.md`, computer-use-linux `get_app_state` workflow.
      - `accesskit_unix 0.17.2` and `accesskit_consumer 0.31.0` exact local dependency source.
    - Options Considered:
      - Structural `TreeUpdate` tests only: rejected; audit proved they missed desktop-consumer failure.
      - Always-on headless GPU/AT test: rejected; unavailable/reliability unknown in CI.
      - Linux desktop smoke plus deterministic consumer unit test: chosen.
    - Chosen Approach:
      - Keep consumer validation blocking and deterministic; add environment-gated real AT-SPI smoke with strict pass/fail when the required desktop bus is available.
    - API Notes and Examples:
      ```bash
      CLAY_LIVE_A11Y_SMOKE=1 cargo test --lib live_atspi_accessibility_smoke -- --ignored --exact --test-threads=1
      ```
    - Files to Create/Edit:
      - `src/main.rs` or existing smoke test module (tentative): reuse `smoke-gui` fixture path.
      - `tests/manual_smoke_docs.rs`: command/documentation drift guard.
      - `docs/development/launch-and-gui-smoke.md`: exact live-AT command and prerequisites.
    - References:
      - `code-reviews/2026-08-14-comprehensive-implementation-and-ui-ux-review.md` evidence limitations.
  - Test Cases to Write:
    - Live AT-SPI startup remains alive through tab/menu/status mutations and exits cleanly before deadline.
    - Missing desktop accessibility bus reports skip/prerequisite, never a false pass.

- [x] Upgrade rkyv and harden malformed archive rejection
  - Acceptance Criteria:
    - Functional: Lock `rkyv >=0.8.17`; all existing protocol round trips pass; malformed/truncated/misaligned/hash-table/Rc/Arc-shaped corpus inputs return `CodecError` without panic or unsafe access.
    - Performance: Preserve 1 MiB pre-allocation frame gate and current encode/decode budgets; patch upgrade causes no material regression in protocol baselines.
    - Code Quality: Keep rkyv behind `src/protocol/codec.rs`; do not spread validation logic or add a second serializer.
    - Security: Remove RUSTSEC-2026-0233/0234/0235 by upgrade; never ignore them; all received archives remain bytechecked before deserialization.
  - Approach:
    - Documentation Reviewed:
      - Local rkyv 0.8.16 rustdoc/source generated with `CARGO_TARGET_DIR=/tmp/clay-plan-doc-target cargo doc -p rkyv --no-deps`.
      - RustSec advisory URLs for 0233/0234/0235; fixed version `0.8.17`.
      - `.agents/skills/project-patterns/references/protocol-and-performance.md`.
    - Options Considered:
      - Add audit ignores: rejected; fixed direct dependency.
      - Replace codec: rejected; patch upgrade addresses issue with minimal risk.
      - Upgrade rkyv patch and retain checked `from_bytes`: chosen.
    - Chosen Approach:
      - Update direct requirement/lockfile, inspect patch release source, preserve existing generic `CheckBytes` bounds, and add deterministic malformed corpus tests at one codec boundary.
    - API Notes and Examples:
      ```toml
      rkyv = "0.8.17"
      ```
      ```rust
      rkyv::from_bytes::<T, rancor::Error>(aligned_payload.as_slice())
      ```
    - Files to Create/Edit:
      - `Cargo.toml`, `Cargo.lock`: patch upgrade.
      - `src/protocol/codec.rs`: regression tests only unless patch API requires a surgical change.
      - `tests/performance_protocol.rs`: payload/latency non-regression.
      - `docs/development/security.md`: record remediation.
    - References:
      - `src/protocol/codec.rs`, `.cargo/audit.toml`.
  - Test Cases to Write:
    - Fixed malformed corpus cases and deterministic pseudo-random byte mutations all fail closed or decode only valid exact frames.
    - Oversized declaration rejects before payload allocation/read.
    - `cargo audit` no longer reports 0233/0234/0235.

- [x] Triage every remaining dependency warning with exact reachability and expiry
  - Acceptance Criteria:
    - Functional: Re-run inverse trees for `event-listener`, `bincode`, `paste`, and `ttf-parser`; update or document each current disposition and upstream constraint.
    - Performance: Dependency changes do not add duplicate runtime stacks or materially increase binary/build size.
    - Code Quality: One security-policy table remains source of truth; no speculative direct dependency or vendored fork.
    - Security: Attempt compatible upstream updates first; any unavoidable warning has exact path, reachability, owner, upstream reference, and recheck date.
  - Approach:
    - Documentation Reviewed:
      - `docs/development/security.md`; `cargo tree -i` output for exact locked versions.
      - Current advisory records RUSTSEC-2025-0141, 2024-0436, 2026-0192, 2026-0221.
    - Options Considered:
      - Ignore warnings silently: rejected.
      - Force incompatible transitive versions: rejected.
      - Upgrade reachable parent when compatible; otherwise document and time-bound: chosen.
    - Chosen Approach:
      - Keep direct fixes surgical and track upstream-constrained warnings without weakening audit.
    - API Notes and Examples:
      ```bash
      cargo tree -i event-listener
      cargo tree -i bincode
      cargo tree -i paste
      cargo tree -i ttf-parser
      ```
    - Files to Create/Edit:
      - `Cargo.lock` only for compatible updates.
      - `docs/development/security.md`, `.cargo/audit.toml` only when policy requires.
      - `tests/primitives_docs.rs`: warning/exception documentation coverage.
    - References:
      - `.agents/skills/project-patterns/references/maintenance-validation.md`.
  - Test Cases to Write:
    - Every configured ignore has unexpired matching documentation; direct fixed advisories cannot be ignored.

- [x] Bound configuration and Control Center regression tests and run serial Linux gates
  - Acceptance Criteria:
    - Functional: The three named tests use isolated config roots, explicit whole-workflow timeouts, and cleanup assertions; one non-concurrent `cargo test --all-targets` passes.
    - Performance: Full gate is run once serially; timeout diagnostics identify pending session/runtime cleanup instead of waiting indefinitely.
    - Code Quality: Preserve existing root-cause fixes already present; add only missing deadline/cleanup coverage.
    - Security: Tests never load ambient `~/.config/clay`; runtime replacement still revokes old sessions and authority.
  - Approach:
    - Documentation Reviewed:
      - Current implementations at `src/server/mod.rs:3002`, `src/server/connection.rs:7468/7797`.
      - `docs/development/build-and-test.md` Linux gates.
    - Options Considered:
      - Increase test timeout: rejected; masks leaked work.
      - Hermetic roots plus bounded workflow and cleanup checks: chosen.
    - Chosen Approach:
      - Assert completion and cancellation explicitly, then run required gates without concurrent Cargo invocations.
    - API Notes and Examples:
      ```rust
      tokio::time::timeout(Duration::from_secs(5), scenario).await.expect("scenario hung");
      ```
      ```bash
      cargo fmt --check
      cargo check --all-targets
      cargo clippy --all-targets -- -D warnings
      cargo test --all-targets
      cargo bench --no-run
      cargo audit
      ```
    - Files to Create/Edit:
      - `src/server/mod.rs`, `src/server/connection.rs`: test-only bounds if absent.
      - `docs/development/build-and-test.md`: record green baseline.
    - References:
      - Linux blocking policy in `AGENTS.md`.
  - Test Cases to Write:
    - Ambient config containing a blocking/invalid module cannot affect these tests.
    - Runtime replacement closes an open menu and leaves no pending session/reply receiver.

- [x] Perform visual screenshot and accessibility review of changed UI
  - Acceptance Criteria:
    - Functional: Launch real Linux UI with AT-SPI enabled; capture default, multi-tab, multi-pane, menu, completion, and announcement/status states; no crash or malformed tree.
    - Performance: Review finds no repeated a11y invalidation or visible interaction stall.
    - Code Quality: Store screenshots/logs under `code-reviews/screenshots/2026-08-14-plan086-a11y/` and record findings in this plan.
    - Security: Screenshots use fixture data and contain no host paths/secrets; labels remain sanitized.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/project-patterns/references/ui-visual-review.md`; computer-use-linux skill.
    - Options Considered:
      - Source/structural review only: rejected.
      - Real screenshot plus `get_app_state` semantic inspection: chosen.
    - Chosen Approach:
      - Call `get_app_state` first, exercise keyboard-only flows, re-query tree after each mutation, and retain evidence.
    - API Notes and Examples:
      ```text
      get_app_state → keyboard interaction → get_app_state → screenshot
      ```
    - Files to Create/Edit:
      - `code-reviews/screenshots/2026-08-14-plan086-a11y/*.png`: evidence.
      - This plan: findings and unresolved blockers.
    - References:
      - `decision-logs/2026-08-14-0200-mandatory-ui-visual-and-accessibility-review.md`.
  - Test Cases to Write:
    - Keyboard focus/order, roles/names/states, modal containment, live announcements, and process liveness.

- [x] Create or verify Clay JS APIs for public programmatic surfaces
  - Acceptance Criteria:
    - Functional: Inventory changed Rust visibility; accessibility/codec internals remain private or `pub(crate)` and require no new JS API.
    - Performance: No JS/IPC call enters accessibility or codec paint/input paths.
    - Code Quality: Any unavoidable public server capability has explicit op, facade, authoritative Markdown, index, registry, and coverage.
    - Security: No raw archive, NodeId, AT-SPI, native widget, or diagnostic internals become package/user authority.
  - Approach:
    - Documentation Reviewed:
      - Project patterns `clay-js-api-boundary.md`, `clay-js-api-naming.md`, `clay-js-api-schema.md`, `documentation-as-code.md`, `doc-registry-tests.md`.
    - Options Considered:
      - Expose debug internals to JS: rejected.
      - Keep remediation internal: chosen.
    - Chosen Approach:
      - Run visibility/API registry checks and document “no new API” if true.
    - API Notes and Examples:
      ```bash
      cargo test --test protocol clay_js_doc_registry::
      cargo test --test security rust_visibility_api_mapping::
      ```
    - Files to Create/Edit:
      - `docs/reference/clay-js-api/**`, `docs/index.md`, generated registry only if inventory proves required.
    - References:
      - `.agents/skills/create-plan/references/clay.md` Clay JS API task.
  - Test Cases to Write:
    - Visibility mapping rejects newly public unmapped server functions.

- [x] Create or verify Clay configuration APIs
  - Acceptance Criteria:
    - Functional: Confirm accessibility remains unconditional and rkyv/test behavior has no user setting.
    - Performance: No configuration evaluation occurs during accessibility update or codec decode.
    - Code Quality: Add no hidden key; if a behavior-changing option unexpectedly appears, document it as a Clay JS API.
    - Security: Configuration cannot disable archive validation or accessibility safety checks.
  - Approach:
    - Documentation Reviewed:
      - Project pattern `configuration-system.md`; `docs/reference/clay-js-api/configuration.md`.
    - Options Considered:
      - Add “disable accessibility” escape hatch: rejected.
      - Fix behavior unconditionally: chosen.
    - Chosen Approach:
      - Verify no configuration delta and preserve current `init.js` model.
    - API Notes and Examples:
      ```text
      No new configuration API expected.
      ```
    - Files to Create/Edit:
      - Configuration docs/registry only if implementation introduces behavior-changing configuration (not planned).
    - References:
      - `docs/development/accessibility.md` unconditional announcements contract.
  - Test Cases to Write:
    - Registry/config inventory stays unchanged unless an explicitly documented API is added.

- [x] Execute and update the manual test plan (test-plan/)
  - Acceptance Criteria:
    - Functional: Execute modules 01, 03, 10, 13, and 14 on real Linux; add stable IDs for live AT startup, tab/pane/menu updates, and sanitized announcements.
    - Performance: Record perceived startup/update responsiveness without inventing wall-clock gates.
    - Code Quality: Preserve existing steps and document exact evidence/blockers.
    - Security: Include no-path-leak and ambient-config isolation negative checks.
  - Approach:
    - Documentation Reviewed:
      - `test-plan/index.md` coverage matrix and relevant modules.
    - Options Considered:
      - Keep evidence only in this plan: rejected.
      - Maintain module steps: chosen.
    - Chosen Approach:
      - Add minimal numbered steps and execute them after automated gates.
    - API Notes and Examples:
      ```bash
      cargo build
      target/debug/clay smoke-gui --config-fixture <fixture>
      ```
    - Files to Create/Edit:
      - `test-plan/01-launch-and-connection.md`, `10-keybindings-and-commands.md`, `13-window-splits.md`, `14-tabs.md`, `test-plan/index.md` as coverage changes require.
    - References:
      - `.agents/skills/create-plan/references/clay.md` Manual Test Plan Task.
  - Test Cases to Write:
    - Manual IDs verify AT-enabled launch, virtual-node updates, and no host-path announcement.

- [x] Update or verify the code wiki after implementation
  - Acceptance Criteria:
    - Functional: Wiki explains stable virtual IDs, consumer validation, codec remediation, and bounded tests; index links every changed/new page.
    - Performance: Document bounded accessibility work and unchanged codec ceilings.
    - Code Quality: Include ownership, invariants, source/test paths, extension guidance, and commands.
    - Security: Document archive trust boundary and sanitized accessibility data without sensitive examples.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/project-wiki/SKILL.md`.
    - Options Considered:
      - Update per task: rejected as churn.
      - Update once after green gates: chosen.
    - Chosen Approach:
      - Update relevant existing pages once and verify master index.
    - API Notes and Examples:
      ```text
      docs/wiki/index.md
      docs/wiki/modules/masonry-shell.md
      docs/wiki/modules/protocol-codec.md
      ```
    - Files to Create/Edit:
      - `docs/wiki/index.md`, `docs/wiki/modules/masonry-shell.md`, `masonry-editor.md`, `pane-document-views.md`, `protocol-codec.md`.
    - References:
      - `.agents/skills/create-plan/references/wiki-task.md`.
  - Test Cases to Write:
    - Manual wiki index/link review and existing documentation coverage tests.

## Entry-Gate Evidence (Task 1, reconfirmed 2026-08-14)

Branch and dirty state:
- `main` at `bcd1b23`, working tree clean. Plan 085 work (hermetic config roots, runtime-generation fixes) is committed, not dirty; nothing to preserve or merge.
- Real desktop AT-SPI bus live (`at-spi-bus-launcher` + `at-spi2-registryd`, session bus `unix:path=/run/user/1000/bus`).

Focused tests (all pass, matching 2026-08-14 audit evidence 0.03–0.07 s):
- `example_configuration_loads_cleanly_and_applies_effects` — 0.07 s. Uses `temp_example_config_root("example-configuration")` (`src/server/mod.rs:3002`, helper at `:2700`): fresh `std::env::temp_dir()` root keyed by pid+nanos, copies `examples/` tree; never ambient `~/.config/clay`.
- `control_center_opens_filters_activates_and_cancels` — 0.03 s (`src/server/connection.rs:7468`, `temp_workspace("control-center-config")` at `:4485`).
- `runtime_generation_replacement_cancels_open_control_center` — 0.03 s (`src/server/connection.rs:7797`, `temp_workspace("control-center-generation-config")`).
- Fixes are protected baseline: do not rewrite; only add missing bounds in Task 7.

AT-SPI crash reproduced, exit 101 (logs: `/tmp/clay-p086-run1.log`, `run2.log`):
- `panicked at accesskit_consumer-0.31.0/src/tree.rs:136:13: TreeUpdate includes 1 nodes which are neither in the current tree nor a child of another node from the update: [#1]`
- Fires on the **initial** tree: `State::update ← Tree::new ← accesskit_atspi_common-0.14.2 adapter ← accesskit_unix-0.17.2::update_if_active ← accesskit_winit-0.29.2` during first redraw, right after `create window` log line. Node `#1` is the first ephemeral `WidgetId::next()` allocation of the first accessibility pass.
- Startup entry evidence: window created and first redraw within the same second of server start; no runtime work added.

`cargo audit` baseline (571 crates):
- 3 unfixed **direct** vulnerabilities: `rkyv 0.8.16` → RUSTSEC-2026-0233/0234/0235 (fix `>=0.8.17`; Task 5 target). `Cargo.toml:26` pins `rkyv = "0.8.16"`.
- 4 allowed warnings, none ignored: `bincode 1.3.3` (RUSTSEC-2025-0141), `paste 1.0.15` (2024-0436), `ttf-parser 0.25.1` (2026-0192), `event-listener 5.4.1` (2026-0221) — Task 6 triage list.
- `.cargo/audit.toml` has exactly 2 ignores (quick-xml RUSTSEC-2026-0194/0195), both documented, last touched in commit `9cab6f4` predating this plan. No ignore added by this plan.

Codec decode boundary (single): `src/protocol/codec.rs::decode_frame` (`:153-194`) — 4-byte length prefix, `declared_len > max_frame_size` rejected **before** payload copy, length-mismatch rejected, single bytechecked `rkyv::from_bytes::<T, rancor::Error>` at `:190` with `CheckBytes` bounds. No second deserialization path.

Ephemeral virtual-node inventory (`WidgetId::next()` → `NodeId`):
- `src/masonry_shell.rs:2184` TabList, `:2190` Tab per card, `:2215` live-announcement Status node.
- `src/masonry_editor.rs:1274` status node.
- `src/masonry_pane_document.rs:3363` status node.
- `src/driver/mod.rs:850-940` `WidgetId::next()` uses are unit-test scaffolding only, not accessibility nodes.
- Retained-derivation precedent already exists: `src/masonry_package_region.rs:91` `stable_menu_a11y_node_id` (prefix `0xD000_0000_0000_0000` | region id << 9 | slot), used at `:775`/`:788`. Generalize this for Tasks 2–3.

Security gate: no AT-SPI-disable path, no archive-validation skip, no IPC-access broadening, no RustSec ignore added anywhere in this task.

## Primitive Review Findings (Task 2, 2026-08-14)

### Node inventory (real widget nodes vs synthetic virtual nodes)

Retained real widget nodes (arena-assigned `WidgetId`, stable for widget lifetime; emitted by Masonry's accessibility pass walk):
- Window wrapper (`Role::Window`, children = layer-0 root), layer root, shell (`Role::Group`), per-tab pane host (`Role::Pane`), editor / pane document (`Role::MultilineTextInput`), package region (`Role::Group`/`Menu`), `panel_host`, `overlay_host`, region pod subtree.
- Emission is driven by the walk over `children_ids()`, not by what `accessibility()` lists: `masonry_core-0.4.0/src/passes/accessibility.rs::build_accessibility_tree` recurses via `recurse_on_children(children_ids())` and pushes a node for every widget with `needs_accessibility` set (`request_access_all_in` sets it for the whole tree on `EnableAccessTree`; per-update via `request_accessibility_update()` + `merge_up`).

Synthetic virtual nodes (built inside `accessibility()`, currently `NodeId::from(WidgetId::next())` — global counter churn per pass):
- `masonry_shell.rs:2184/2190` TabList + one Tab per card (only when `tab_cards.len() >= 2` and tab-bar geometry exists; bounds = bar/card rects; labels = `sanitize_document_display_name(&card.name)`; selected = active card).
- `masonry_shell.rs:2215` polite announcement node (`Role::Status`, `Live::Polite`, no bounds; label = `self.announcement`).
- `masonry_editor.rs:1274` status node (`Role::Status`, bottom strip bounds, `compose_status_accessibility_label`).
- `masonry_pane_document.rs:3363` status node (same role/bounds convention).
- `masonry_package_region.rs:775/788` menu items + status — ALREADY stable via `stable_menu_a11y_node_id` (prefix `0xD000_0000_0000_0000` | owner bits `<<9` | slot; slots 1 = status, 2+ = items).

Update triggers (each calls `request_accessibility_update()`): shell tab registry/geometry changes (`:719`), `announce()` (`:1064`), tab-bar scroll (`:2062`); pane-document edit outcomes (`:2056`, `:2500`, `:2768`) and sync-recovery state changes (`:2967-3105`); region menu open/filter/close via `set_menu_a11y`. Construction cost is O(visible tabs / menu items) and runs only inside the accessibility pass; the fix must not move it elsewhere.

### Proven crash root cause (consumer probe, 2026-08-14)

Probe test (temporary, removed after evidence) fed the real first `TreeUpdate` into `accesskit_consumer::Tree::new` exactly as `accesskit_unix` does; it reproduced the live panic byte-identically: `TreeUpdate includes 1 nodes which are neither in the current tree nor a child of another node from the update: [#1]`. Consumer invariant (`accesskit_consumer-0.31.0/src/tree.rs:95-141`): every node in `update.nodes` must be in the current tree or referenced as a child by another node in the update, and every child ID must resolve; otherwise it panics.

Two orphan mismatches found (children emitted by the walk but not listed by the parent's `accessibility()` children):
1. **Editor omits the region child when no sidebar geometry exists** (`masonry_editor.rs:1257-1259` `if self.sdui.sidebar_geometry(...).is_some()`) while `children_ids()` (`:1297-1304`) always includes region → region node `#1` orphaned on every startup without a sidebar → the live crash. (Panel/overlay hosts are always listed even when empty; the region should follow the same convention.)
2. **Shell lists only active-tab hosts** (`masonry_shell.rs:2158-2163`) while `children_ids()` (`:2221-2227`) includes all tabs' hosts + pending orphans → two-tab probe panics with 3 orphans `[#11, #19, #15]` (both regions + the inactive host). Latent for any second tab and re-triggerable on later passes when an inactive tab's subtree requests an accessibility update (flags propagate via `merge_up`).

Conclusion: **stable IDs alone do not fix the crash.** Task 3 must (a) make `accessibility()` children cover exactly what the walk emits (or stop the walk from emitting it), and (b) migrate synthetic nodes to deterministic IDs. `masonry_pane_document.rs` is safe (children_ids empty, only the status child listed).

### Which IDs must survive incremental updates

- Retained widget IDs: already stable (arena assignment, `WidgetId::next()` counter starts at 1, sequential; `masonry_core-0.4.0/src/core/widget.rs:506`). No work.
- Synthetic IDs: must be stable — they are referenced from persistent parent children lists; each churn forces the consumer to remove+re-add (live-region re-registration, Tab selection loss, churn in `change` events). Deterministic derivation from the owning retained widget ID + typed slot gives lifetime identity without per-widget storage.
- Parent-chain rule for Task 3 tests: a synthetic node is legal iff its parent's node is in the same update or already in the tree (all synthetic parents are retained widgets, so both hold once the two mismatches above are fixed).

### Generic helper design (Task 3 spec)

- Home: `src/editor/accessibility.rs` (existing shared label policy owner; already used by shell, editor, pane-document, region).
- Signature: `pub(crate) fn virtual_a11y_node_id(owner: WidgetId, slot: u16) -> NodeId` generalizing `stable_menu_a11y_node_id`; move that function's body into the helper and migrate the region call sites.
- Layout: prefix `0xD000_0000_0000_0000` | `(owner.to_raw() & 0x0000_7FFF_FFFF_FFFF) << 9` | slot; `debug_assert!(slot < 512)` (9-bit slot space). Existing slot space: menu ≤ `TRANSIENT_MENU_MAX_ITEMS` (256) + 2, tabs ≤ `MAX_ACTIVE_CONNECTIONS` (64) → fits. Cross-owner collisions impossible (owner bits differ); in-owner collisions impossible if slots are unique per typed slot.
- Slot namespaces per owner (typed, per plan's `VirtualA11ySlot`): shell — TabList = 1, Announcement = 2, Tab(i) = 3 + i (i < 64); editor/pane-document — Status = 1; region — Status = 1, Item(i) = 2 + i (preserve existing numbering to avoid test churn).
- Keep bounds conventions: TabList/Tab and statuses keep their geometry bounds; announcement and menu nodes remain bounds-less (retain current behavior).

### Sanitization ceilings (verified intact; no regression risk)

- Display names (tabs, announcements): `sanitize_document_display_name` — basename-only, 64-char cap, rejects separators/control chars (`src/editor/accessibility.rs:33-64`).
- Recovery summaries, menu prompt/status: `sanitize_recovery_summary` — 256-char cap (`TRANSIENT_MENU_MAX_ACCESSIBILITY_LABEL_CHARS`, `src/perf/budgets.rs:223`).
- Announcements: `compose_announcement` sanitizes names + caps at 256 (`ANNOUNCEMENT_MAX_CHARS`, `src/masonry_shell.rs:1619`).
- Finding: menu **item** labels are cloned raw (no char ceiling; count-bounded at 256 items). Not a regression (unchanged), but Task 3 should document the bound in the helper contract.

### Files touched this task

- `Cargo.toml`: added `accesskit_consumer = "0.31.0"` dev-dependency (version-exact match to the transitive locked dep) — required for the Task 3 consumer-validation tests.
- `src/masonry_shell.rs`: temporary consumer probe test added and removed after capturing evidence; no production change.
- Source sites remain untouched (implementation is Task 3).

## Implementation Evidence (Task 3, 2026-08-14)

### Stable virtual node policy (implemented as specced)

- `src/editor/accessibility.rs`: `VIRTUAL_A11Y_NODE_PREFIX` (0xD000_0000_0000_0000), 55-bit owner mask `<< 9`, 9-bit slot space with runtime assert; `virtual_a11y_node_id(owner, slot)` + `virtual_a11y_slots` namespace module (shell TabList=1/announce=2/Tab=3+client_id; editor+pane-document status=1; region status=1/items=2+i). Region's `stable_menu_a11y_node_id` deleted; call sites use the shared helper (identical IDs, no churn).
- Migrated: `masonry_shell.rs` TabList/Tab/announcement, `masonry_editor.rs` status, `masonry_pane_document.rs` status. Zero `WidgetId::next()` virtual-node allocations remain (the only remaining `WidgetId::next()` uses in `src/driver/mod.rs` are unit-test scaffolding).
- Tab slots derive from client id (bounded by `MAX_ACTIVE_CONNECTIONS` = 64), so reorders/selection changes keep node identity.

### Orphan-mismatch fixes (the actual crash root causes, proven in Task 2)

1. `masonry_editor.rs::accessibility` now ALWAYS lists the region child (was conditional on sidebar geometry) — the startup crash `[#1]`.
2. `masonry_package_region.rs::accessibility` now always attaches the reconciled pod alongside the semantic menu nodes — the walk emits the pod (children_ids, required for pass-based painting; no per-child paint API exists) and the consumer rejects unattached nodes, so a menu-open pass with a changed pod crashed live.
3. `masonry_shell.rs::layout` now **stashes** inactive tab hosts and pending orphans (`LayoutCtx::set_stashed`); children_ids stays all-tabs (register_children and explicit zero-size placement require it). Masonry skips stashed subtrees in paint + the accessibility walk and propagates the stash through the subtree; unstash on tab activation auto-requests layout + a11y. This fixes the two-tab first-pass panic `[#11, #19, #15]` and keeps inactive tabs unreachable.
- `masonry_pane_document.rs` was already safe (no children_ids/a11y mismatch).

### Tests (consumer-validated, `accesskit_consumer` 0.31.0 dev-dependency added)

- `consumer_accepts_single_tab_initial_tree_with_region_attached`: first update through `Tree::new` (previously panicked `[#1]`); stable status node attached; unchanged redraw clean.
- `consumer_accepts_multi_tab_incremental_updates_and_drops_stale_nodes`: initial two-tab tree (previously panicked with 3 orphans), unchanged redraw, announcement, tab-add + selected-tab, tab-reorder (ids stable), tab-remove (stale "alpha" Tab gone), pane document-name update (basename sanitized, no host path). Exactly one reachable Pane node (inactive host absent).
- `consumer_accepts_menu_query_selection_and_close_updates` (region): initial menu, selection move (same ids), narrowed query, close (no stale items reachable).
- Existing structural tests kept/updated: `install_tab_switches_to_new_tab_and_retains_previous` and `shell_accessibility_tree_leads_with_tablist_and_hides_inactive_tabs` now document stashing as the hiding mechanism.

### Live verification (real AT-SPI, desktop bus active)

- `cargo run -- smoke-gui --config-fixture runtime-sdui` previously exit 101 within ~2 s; now survives 40 s (killed by timeout), zero panics.
- `computer-use-linux get_app_state` during the run: `clay` registered as an AT-SPI Application; tree queryable: Frame → shell Group "Clay working area shell. Active pane 1." → Pane "Pane 1 of 1: editor" → Entry (editor label) + region Panel "Server-driven UI region" + panel/overlay hosts + StatusBar status node. Region attached, status stable, window focused.

### Gates

- `cargo test --lib`: 1525 passed, 0 failed, 2 ignored.
- `cargo clippy --all-targets`: clean. `cargo fmt`: applied.
- The three Task 1 focused tests still pass (0.06 s / 0.03 s / 0.03 s).

### Docs

- `docs/development/accessibility.md`: added "Virtual node identity" + "Consumer-validation contract" sections; inactive-tab mechanism updated to stashing; verification section now includes consumer + live AT-SPI checks.

## Implementation Evidence (Task 4, 2026-08-14)

### Live AT-SPI regression check (implemented)

- `tests/live_atspi_smoke.rs` (security suite; `#[ignore]` + `CLAY_LIVE_A11Y_SMOKE=1` gate, exact plan-style command):
  ```bash
  CLAY_LIVE_A11Y_SMOKE=1 cargo test --test security live_atspi_smoke::live_atspi_accessibility_smoke -- --ignored --exact --test-threads=1
  ```
  Refinement vs. the tentative plan sketch: the test is an integration test (not `--lib`) because it must spawn the real binary via `env!("CARGO_BIN_EXE_clay")`, which cargo only exposes to integration tests; it joins the existing `tests/suites/security.rs` inventory (the suite that already spawns the binary for `runtime_sandbox_harness`).
- Reuses the existing CLI + fixture machinery: `clay server <sock> --config-fixture runtime-sdui` + `clay client <sock>`, both spawned with `XDG_CONFIG_HOME`/`XDG_DATA_HOME` inside one mode-700 temp dir; socket also inside that dir. No ambient `~/.config/clay`, `~/.local/share/clay`, or default endpoint; a v2 `layout.json` in the temp config home restores a two-tab window (the live two-tab crash shape).
- AT-SPI probing via a small embedded python3 GI Atspi script (`prereq` + `dump` modes). Missing prerequisites (no `org.a11y.Bus`, no python3/`gir1.2-atspi-2.0`) print an explicit skip reason and return — never a false pass.
- Verifies against the real desktop stack: window alive past startup, `Workspace tabs` TabList with both restored cards (exactly one selected), active pane, connected status line, attached `Server-driven UI region`; then a second query must expose identical node paths (no virtual-node churn). Every exit path kills both children and removes the temp dir (KillGuard).

### Live run (this host, real GNOME session + AT-SPI bus)

- `PASS (window alive, tree stable, 2 tabs restored)` — the exact startup shape that panicked pre-fix (two-tab restore previously rejected nodes `[#11, #19, #15]`).
- Observed live roles: shell `Panel`, `page tab list`/`page tab` (AT-SPI role names differ from accesskit `TabList`/`Tab`), announce `Status` node carried "Switched to tab 1: runtime-sdui" during restore; status line "Clay — Connected…"; region attached.
- Skip path verified: without the env var the test prints the skip reason and returns in 0.00 s.

### Docs + drift guard

- `docs/development/launch-and-gui-smoke.md`: "Live AT-SPI accessibility regression check (plan 086 task 4)" — exact command, prerequisites, what it verifies, isolation contract.
- `tests/manual_smoke_docs.rs` (protocol suite): `plan086_live_atspi_smoke_command_and_prerequisites_are_documented` asserts the doc keeps the exact command, env var, prerequisite names, isolation wording, and the runtime-sdui fixture contract.

### Gates

- `cargo fmt` applied; `cargo clippy --test security` clean; `cargo test --test security` and `--test protocol` pass with the new tests (skip paths green).

## Implementation Evidence (Task 5, 2026-08-14)

### Upgrade

- `Cargo.toml`: `rkyv = "0.8.17"`; `Cargo.lock`: rkyv/rkyv_derive 0.8.16 → 0.8.17 (an initial `cargo update` slipped to 0.8.18, corrected with `--precise 0.8.17`).
- Patch-source inspection confirmed the fixes land in the advisory areas: `src/validation/shared/mod.rs` reworked shared-pointer validation (metadata identity + erased pointers — RUSTSEC-2026-0233/0235) and `src/collections/swiss_table/table.rs` added element-count validation (`WrongNumberOfElements` — RUSTSEC-2026-0234).
- `cargo audit`: **0 vulnerabilities** (was 3); the 4 allowed warnings are unchanged and belong to Task 6. No ignore added; no codec API change needed — the generic `CheckBytes`/`Deserialize` bounds and single bytechecked `rkyv::from_bytes::<T, rancor::Error>` boundary at `decode_frame` are preserved byte-for-byte.
- Clay's wire types contain no `HashMap`/`Rc`/`Arc` fields (0234/0235's archived shapes are unreachable in the protocol); the adversarial corpus therefore targets the offset/pointer + length validation surface those advisories hardened.

### Malformed-corpus tests (`src/protocol/codec.rs`, 4 new, all deterministic, no new dependency)

- `malformed_corpus_truncations_fail_closed`: cut sweep over both a 256-item completion result (~string-heavy) and the representative SDUI snapshot, declared lengths rewritten per cut; asserts bytecheck rejection dominance (≥3/4 of cuts) and baseline round-trip.
- `malformed_corpus_mutations_fail_closed`: 300 seeded-LCG 1–8-byte flips per message (≥1/4 rejection — string-body/padding mutations legitimately keep archives valid) plus zero/0xFF run-corruption windows of 1/4/8/16/32 bytes; every input is rejected or decodes as a bytecheck-validated archive — never a panic, and every survivor is bounds-checked against the actual buffer (deserialization allocations bounded by buffer size).
- `malformed_corpus_misaligned_declared_lengths_fail_closed`: odd declared lengths (1/3/5/7/9/15/17) with matching payloads all fail closed.
- `malformed_corpus_oversized_declaration_rejected_on_read_side`: duplex-stream read pump rejects a 9-byte declaration under an 8-byte cap with `FrameTooLarge` before payload read (frame gate stays 1 MiB).
- Empirical note recorded in the test docs: rkyv 0.8 enum roots sit at the buffer tail (tag + rel_ptr), so truncations/mutations can alias into a *different* valid message (observed `Welcome` alias of a truncated SDUI snapshot); bytecheck's bound checks make such survivors memory-safe — the codec contract is reject-or-validate, never panic, never OOB.

### Gates & performance

- `cargo test --lib protocol::codec`: 38 passed. `tests/performance_protocol.rs`: 19 passed (payload budgets intact). `protocol_server_baselines` bench runs clean (hello round-trip ~108 ns, edit/16 ~136 ns, edit/1024 ~274 ns, edit/16384 ~1.26 µs).
- `docs/development/security.md`: new "Remediated vulnerabilities" table records the upgrade, patch evidence, and corpus coverage.

## Implementation Evidence (Task 6, 2026-08-14)

### Compatible update applied

- `event-listener 5.4.1 → 5.4.2` (`cargo update --precise 5.4.2`; only `Cargo.lock` changed): RUSTSEC-2026-0221 (`!Send` tags across threads via `StackSlot`, smol-rs PR 163) has `patched = [">= 5.4.2"]`, so the advisory explicitly requested this upgrade. Single lockfile copy (5.4.2) on the same `async-broadcast 0.7.2 → zbus 5.15.0 → accesskit_unix/atspi` path — no duplicate stack, no binary growth (patch bump within the same crate).
- `cargo audit` now reports **3 warnings, 0 vulnerabilities** (was 4 warnings + 3 vulnerabilities).

### Remaining warnings — exact reachability + expiry (all three `patched = []`, no compatible upstream fix)

| Advisory | Crate | Path | Reachability | Owner | Upstream | Recheck |
| --- | --- | --- | --- | --- | --- | --- |
| RUSTSEC-2025-0141 | bincode 1.3.3 | `deno_core 0.400 → bincode` | V8 snapshot (de)serialization; host-produced snapshots, not untrusted data | deno_core | bincode 3 rewrite (git.sr.ht) | next deno_core upgrade; hard 2026-12-16 |
| RUSTSEC-2024-0436 | paste 1.0.15 | `v8 → paste` (proc-macro) | build-time only | v8 (deno/rusty_v8) | dtolnay/paste | next v8 upgrade; hard 2026-10-07 |
| RUSTSEC-2026-0192 | ttf-parser 0.25.1 | `winit → sctk-adwaita → ab_glyph → owned_ttf_parser → ttf-parser` | bundled window-decoration fonts; not untrusted | sctk-adwaita/ab_glyph | harfbuzz/ttf-parser#217, successor read-fonts | next winit-chain upgrade; hard 2026-12-28 |

### Docs + drift guard

- `docs/development/security.md`: classified-warnings table gained Owner / Upstream reference / Disposition-recheck columns (full fields for all three rows); RUSTSEC-2026-0221 added to the remediated-vulnerabilities table with the 5.4.2 fix and single-copy evidence.
- `tests/primitives_docs.rs`: new `classified_dependency_warnings_and_remediated_ids_are_documented` — every current warning ID must have a classified row containing path, upstream URL, and a recheck date; the four directly fixed advisories (0221, 0233, 0234, 0235) must not appear outside the remediated table and can never be re-introduced into `.cargo/audit.toml`. 24 primitives_docs tests pass.
- `.cargo/audit.toml` unchanged: still exactly the two quick-xml ignores with unexpired documented expiries.

### Gates

- fmt/check/clippy clean; `cargo test --all-targets` all suites green (lib 1529 passed); `cargo audit` exit 0 with the 3 classified warnings.

## Implementation Evidence (Task 7, 2026-08-14)

### Bounds added (test-only, no production code changed)

- All three config tests (`example_configuration_loads_cleanly_and_applies_effects`, `control_center_opens_filters_activates_and_cancels`, `runtime_generation_replacement_cancels_open_control_center`) now run their scenario under a 5 s whole-workflow `tokio::time::timeout` whose message names the failure class ("pending session or reply-receiver cleanup") instead of waiting indefinitely. Scenario bodies moved to `*_scenario` fns; the `#[tokio::test]` wrappers own the bound.
- Hermetic-root proof: both Control Center tests write a **sentinel typography** into their temp config root (`import { setTypography } from "clay:theme"; setTypography({ monospace: { size: 21 }, ... })`) and assert `active_typography().monospace.size == 21.0` after the reload — an ambient `~/.config/clay` fallback would load 20 px defaults, failing loudly. (The API is import-only; bare `setTheme`/`setTypography` globals throw `runtime.exception`, verified via a temporary direct-reload probe that was removed.)
- Runtime replacement cleanup assertion: after the generation replacement closes the open menu, a stale `MenuSelectionMove` against the cancelled session id must yield the bounded `menu.unknown_session` diagnostic — no pending session, no reply from a live session, no hang.
- Existing Phase 24.5 root-cause fixes (hermetic `temp_workspace`/`temp_config_root` roots, `drain_bounded` + `close` cleanup) preserved untouched; only missing deadline/cleanup coverage was added.
- Debugging note: a broken sentinel (global call, no `clay:theme` import) made the reload fail with `runtime.exception`; the reloaded=false path pushes only that diagnostic and the old fanout loop waited forever for `reload_succeeded`/snapshot — exactly the class the 5 s wrapper now bounds.

### Serial gates (one non-concurrent run, all green)

- `cargo fmt --check` clean; `cargo check --all-targets` 0 warnings; `cargo clippy --all-targets` clean; `cargo test --all-targets` all suites green (lib 1529 passed + 2 ignored, security 62, runtime 152, editor 198, protocol 128 + 1 ignored live AT-SPI); `cargo bench --no-run` builds; `cargo audit` 0 vulnerabilities + the 3 classified warnings.
- Times: example 0.06 s, Control Center 0.03 s, runtime replacement 0.03 s — far inside the 5 s bound.
- `docs/development/build-and-test.md` gained the "Bounded configuration and Control Center tests" subsection recording the green baseline, the bound semantics, and the serial-gate note.

## Implementation Evidence (Task 8, 2026-08-14)

### Real Linux review completed

- Launched real Clay server/client on GNOME Wayland with the live AT-SPI bus, explicit isolated Unix socket, temporary mode-700 config/data homes, and fixture-only data. `get_app_state` was called before interaction; every mutation was keyboard-only and followed by an AT-SPI re-query.
- Captured and inspected cropped Clay-window evidence under `code-reviews/screenshots/2026-08-14-plan086-a11y/`:
  - `default-single-tab.png` — one pane, focused editor, connected/editable status.
  - `multi-tab-status.png` — `Workspace tabs`, two named `PageTab` nodes, one selected.
  - `multi-pane.png` — two-pane split with `Pane 1 of 2` and `Empty pane 2 of 2`.
  - `control-center-menu.png` and `control-center-filtered.png` — Control Center unfiltered and bounded filtered states.
  - `completion-empty.png` — Completion menu empty-result state.
  - `announcement-status.png` — selected tab 2 plus announcement-triggering state.
  - `review-log.md` — method, semantic tree observations, findings, and cleanup evidence.
- Normal representative flows stayed alive with no malformed-tree error: tab close, Control Center open/filter/Enter/Escape, split-pane activation, text insertion, completion trigger, and tab activation. AT-SPI exposed shell/pane/tab/status roles and the live announcement `Switched to tab 2: syntax-grammars`.

### Findings and unresolved blockers

- **Control Center overflow:** unfiltered 60-item menu extends below the 900x1116 Clay window; filtering to 7 items fits. Retained as `control-center-menu.png`; geometry follow-up belongs to Plan 087.
- **Completion coverage ceiling:** this fixture rendered `Completion` + `No completions` correctly but did not produce buffer-word items after synthetic text; successful item geometry remains a Plan 087 follow-up.
- **Top-level AT-SPI focus edge:** a tool-only `Atspi.Accessible.grab_focus()` on Clay's top-level Frame caused the client panic `Cannot send event to non-existent widget #8`; the sanitized excerpt is `focus-frame-crash.log`. Focusing the actual editor Entry succeeded and all ordinary keyboard flows remained alive. This is recorded as an accessibility-adapter follow-up, not an ordinary user-flow failure.
- Non-fatal AT-SPI cache queries (`GetApplicationBusAddress` / `/org/a11y/atspi/cache`) remained observable but did not prevent tree queries or state validation.

No production code changed for Task 8; the temporary fixture, client, server, temp homes, and typed synthetic document were removed after capture.

## Implementation Evidence (Task 9, 2026-08-14)

- Inventory of Plan 086 Rust changes found no new bare-public server function, `deno_core` op, runtime JS export, API Markdown page, `docs/index.md` registry link, inventory entry, or generated registry entry. Accessibility identity helpers are intentionally `pub(crate)` (`virtual_a11y_node_id`, `virtual_a11y_slots`, and the virtual-ID prefix/slots); codec production visibility is unchanged and the new codec corpus is test-only.
- Added `plan086_virtual_accessibility_helpers_are_not_public_programmatic_surfaces` to `tests/rust_visibility_api_mapping.rs`. It rejects bare-public helper promotion and any leakage through server ops, `runtime/js`, or `docs/generated/clay-js-api-registry.json`.
- Existing Clay JS contract checks passed without documentation or registry changes:
  - `cargo test --test protocol clay_js_doc_registry::` — 40 passed.
  - `cargo test --test protocol clay_js_api_inventory::` — 10 passed.
  - `cargo test --test protocol clay_js_facade_layout::` — 5 passed.
  - `cargo test --test security rust_visibility_api_mapping::` — 11 passed.
  - `cargo test --test protocol clay_js_doc_registry::generated_registry_is_current -- --exact` — passed.
  - `cargo fmt --all -- --check` — passed.
- No `cargo run --bin update-doc-registry` rewrite was needed because authoritative API Markdown and generated registry inputs were unchanged.

## Implementation Evidence (Task 10, 2026-08-14)

- No new configuration API is needed for Plan 086. The authoritative configuration contract remains the exact six existing `clay:configuration` exports: three runtime-backed APIs (`loadConfigurationModule`, `getConfigurationState`, `setPackageOption`) and three planned/unavailable stubs (`setModePreference`, `setDecorationTheme`, `setParsePolicy`). `runtime/js/configuration.js`, `docs/reference/clay-js-api/configuration/**`, `docs/index.md`, `api-inventory.toml`, and the generated registry required no changes.
- Accessibility identity/label budgets and protocol archive validation remain compiled implementation controls. Production portions of `src/editor/accessibility.rs` and `src/protocol/codec.rs` contain no configuration evaluation or `init.js` access; configuration remains outside accessibility update and codec decode hot paths.
- Extended `src/server/configuration.rs::plan060_internal_security_and_performance_controls_are_not_configurable` with Plan 086 rejection cases for `accessibility.enabled`, `accessibility.validation`, `protocol.archiveValidation`, `protocol.codecValidation`, and `protocol.rkyvValidation`. The rejected `setPackageOption` attempt leaves configuration state unchanged.
- Configuration closure and authority checks passed:
  - `cargo test --lib plan060_internal_security_and_performance_controls_are_not_configurable -- --test-threads=1` — passed.
  - `cargo test --test protocol configuration -- --test-threads=1` — 16 passed.
  - `cargo fmt --all -- --check` — passed.
- No hidden key, authority-bearing setting, or user-controlled archive/accessibility bypass was added; `examples/init.js` remains unchanged.

## Implementation Evidence (Task 11, 2026-08-14)

### Real Linux execution

- Executed the manual coverage on GNOME Wayland with a live AT-SPI bus using isolated `clay server <temp-socket>` + `clay client <temp-socket>` processes. HOME, XDG config/data, socket, scratch workspaces, and `layout.json` were under mode-700 temporary roots; no ambient `~/.config/clay` or default endpoint was used.
- Updated `test-plan/index.md` and modules `01-launch-and-connection.md`, `03-files-and-workspace.md`, `10-keybindings-and-commands.md`, `13-window-splits.md`, and `14-tabs.md` with exact results, stable live-node evidence, negative checks, and host/tooling blockers.
- PASS evidence: live startup/status and attached region; restored two-tab/two-pane documents; Control Center open/filter/cancel; split creation and clean pane close; tab selection/close; bounded pane/tab/announcement labels; deterministic virtual object paths; no absolute host paths or secrets in the inspected tree; isolated configuration roots and fixture keybindings.
- PASS stable identities: live AT-SPI exposed `Workspace tabs` as a `page tab list` with two `page tab` children, stable shell IDs `14987979559889014273` (TabList), `14987979559889014274` (announcement), and `14987979559889014276/14277` (cards), owner-derived pane status IDs, and Control Center virtual status/item IDs `14987979559889054209+` across menu updates. `Split pane vertically` / tab-switch / tab-close announcements remained bounded and attached without malformed-tree errors.

### Findings and blockers

- **Follow-up blocker:** dirty active-pane close (`Ctrl+Alt+W` after editing `a.txt`) crashed the client at `accesskit_consumer-0.31.0/src/tree.rs:34:13` with `Focused ID #4 is not in the node list`; the isolated server remained alive. Clean close passed and announced `Closed pane; 1 pane remains`. Sanitized evidence: `code-reviews/screenshots/2026-08-14-plan086-a11y/manual-dirty-pane-close-crash.log`. This is a focus/a11y follow-up, not marked as a pass.
- Native dialog selection, observer/restart/local-fallback keyboard flows, and full quit/relaunch persistence were not re-run: the host's portal/window-targeting backend cannot safely target Clay controls. The module records explicitly mark these as blocked/unexecuted; automated coverage and the earlier Task 8 live review remain separate evidence.
- Task 11 is complete as a documented manual execution task with the blocker preserved for follow-up; no production code was changed during manual verification.

## Implementation Evidence (Task 12, 2026-08-14)

- Added and indexed `docs/wiki/modules/plan086-release-integrity-and-accessibility.md`, documenting deterministic virtual AccessKit IDs, the Masonry reachable-child/stashing invariant, sanitized labels/announcements, checked `rkyv 0.8.17` decoding, hermetic configuration closure, audit policy, live AT-SPI verification, source/test paths, commands, and known follow-ups.
- Synchronized existing implementation pages: `masonry-shell.md`, `masonry-editor.md`, `masonry-sdui-region.md`, `pane-document-views.md`, `tabs-and-clients.md`, `protocol-codec.md`, `configuration-runtime.md`, `maintenance-validation.md`, and `docs/wiki/index.md`. Stale per-pass `WidgetId::next()` a11y allocation, zero-size-only inactive-tab wording, conditional editor-child attachment, and unchecked archive-validation wording were corrected.
- Verification: `cargo test --test protocol primitives_docs -- --test-threads=1` — 24 passed; `git diff --check` — passed. The wiki index validator confirms every wiki page, including the new Plan 086 page, is linked.
- No public JS API or configuration key was added; the wiki records the dirty-pane-close and top-level-frame-focus findings as follow-up blockers rather than presenting them as passes.

## Compromises Made

- Live AT-SPI smoke remains `#[ignore]` and environment-gated because desktop-bus availability is host-dependent; deterministic consumer validation stays blocking and the smoke reports prerequisite skips rather than false passes.
- Native dialog, observer/restart, local-fallback, and full quit/relaunch manual flows remain explicitly blocked on this GNOME host's window-targeting backend; no unsafe coordinate or focus workaround was added.
- Transient menu item labels keep existing package-authored text for this plan; item count remains capped, while a per-item character ceiling is deferred to follow-up rather than changing menu semantics in the P0 patch.

## Further Actions

- **Plan 087 — UI foundation/review harness:** bound package-authored transient-menu item accessibility labels at the shared menu projection boundary, including sanitized/truncated corpus coverage; fix Control Center result overflow/scroll containment for 60+ results; and make empty/expired completion disappear instead of occupying a full-width surface. These are tracked in the Plan 087 completion, transient-label, regression, and visual-review tasks.
- **Plan 089 — validation/platform feedback:** fix shared focus reconciliation for dirty active-pane close so removing or rejecting a focused pane cannot leave `accesskit_consumer` with a stale focused node; guard top-level Frame/window focus events against nonexistent Masonry widget targets while retaining the working editor-Entry path; and add consumer/live regression coverage.
- **Plan 089 — Linux window validation:** add safe Wayland window targeting/raise/focus support to the review/manual-test workflow, or record an explicit host prerequisite blocker without blind-input workarounds, then rerun native dialog, observer/restart/local-fallback, and full quit/relaunch persistence cases when targeting is available.
