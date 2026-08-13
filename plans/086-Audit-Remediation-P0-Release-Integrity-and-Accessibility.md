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

- [ ] Reconfirm the dirty-branch baseline and lock P0 scope
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

- [ ] Review Clay UI and accessibility primitives before changing virtual nodes
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

- [ ] Implement stable virtual accessibility nodes and consumer-valid incremental updates
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

- [ ] Add a real Linux AT-SPI accessibility regression check
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

- [ ] Upgrade rkyv and harden malformed archive rejection
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

- [ ] Triage every remaining dependency warning with exact reachability and expiry
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

- [ ] Bound configuration and Control Center regression tests and run serial Linux gates
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

- [ ] Perform visual screenshot and accessibility review of changed UI
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

- [ ] Create or verify Clay JS APIs for public programmatic surfaces
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

- [ ] Create or verify Clay configuration APIs
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

- [ ] Execute and update the manual test plan (test-plan/)
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

- [ ] Update or verify the code wiki after implementation
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

## Compromises Made

- To be filled after tasks are completed and tests pass.

## Further Actions

- To be filled after task completion with improvements, rationale, and priority.
