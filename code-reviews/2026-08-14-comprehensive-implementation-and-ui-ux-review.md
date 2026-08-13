# Clay implementation and UI/UX review

**Review date:** 2026-08-14  
**Scope:** Current Rust app, server/protocol/runtime/package boundaries, test/build state, live Linux UI, accessibility, current uncommitted Phase 24.4/24.5 work, and Xilem fit.  
**Method:** Architecture/source review, `cargo audit`, live isolated server/client runs, desktop screenshots, Computer Use accessibility inspection, existing review/remediation evidence, and current documentation/test inventory.

## Executive summary

Clay has strong foundations: server-owned authority, bounded IPC and document lifecycle, two package runtime domains, atomic saves, a component/token catalog, and focused structural tests. The prior remediation plan closed many serious historical issues.

Current branch is **not releasable**. Three blockers surfaced:

1. Enabling real desktop accessibility crashes every Clay client at startup.
2. Direct `rkyv 0.8.16` has three RustSec vulnerabilities in the IPC codec path.
3. Current full library verification has a reproducible failing configuration test and two Control Center tests stalled beyond 60 seconds.

UI is functional only at a prototype level in the observed default experience: a mostly blank window still says “Welcome to Clay's Phase 4 IPC server.”; an empty completion session occupies the full width and 35% of editor height. This does not communicate a usable editor, available actions, status, or onboarding.

## Evidence and limitations

### Commands/results

| Check | Result |
|---|---|
| `cargo audit` | **Failed**: `RUSTSEC-2026-0233`, `-0234`, `-0235` in direct `rkyv 0.8.16`; upgrade target is `>=0.8.17`. Warnings: unmaintained `bincode`, `paste`, `ttf-parser`; unsound `event-listener`. |
| Existing `cargo test --lib --quiet` runs | **Failed/stalled**: `server::runtime_generation_tests::example_configuration_loads_cleanly_and_applies_effects` failed; `server::connection::tests::control_center_opens_filters_activates_and_cancels` and `runtime_generation_replacement_cancels_open_control_center` exceeded 60 seconds. Two concurrent pre-existing runs showed the same result. |
| `git diff --check` | Passed. |
| Live client with desktop AT-SPI enabled | **Panicked** in `accesskit_consumer 0.31.0`: `TreeUpdate includes 1 nodes which are neither in the current tree nor a child of another node from the update: [#1]`. |
| Isolated non-AT-SPI client | Launched; default and empty-completion screenshots captured. |

Live evidence:

- `code-reviews/screenshots/2026-08-14-clay-audit/default-desktop.png`
- `code-reviews/screenshots/2026-08-14-clay-audit/focus-check.png`
- `code-reviews/screenshots/2026-08-14-clay-audit/command-centre.png`

Screenshots are full-desktop portal captures because session window-targeting/screenshot permission was unavailable. `computer-use-linux get_app_state` did obtain AT-SPI state; attempting the normal Clay client with AT-SPI enabled triggered the crash above. Thus visual review is valid, while semantic control-by-control interaction remains blocked by the defect.

## P0 — fix before further UI/product work

### P0-1: Real accessibility activation crashes Clay

**Evidence:** Live isolated client startup under AT-SPI repeatedly panicked in `accesskit_consumer`; the same app launches when the AT-SPI bus is made unreachable. This is a user-facing denial of service for screen-reader users, not a cosmetic a11y gap.

**Likely root cause:** Accessibility methods mint new `WidgetId::next()`-derived virtual `NodeId`s on every tree update in:

- `src/masonry_shell.rs:2184-2215` (TabList, every Tab, announcement);
- `src/masonry_editor.rs:1274` (status); and
- `src/masonry_pane_document.rs:3363` (status).

Virtual nodes must retain deterministic identity across incremental AccessKit updates and be attached consistently. `src/masonry_package_region.rs:91` already has the right direction: derive IDs from retained region identity plus a stable slot.

**Required work:** Replace all ephemeral virtual IDs with a collision-proof, stable ID scheme keyed by retained widget ID plus typed slot/item key. Test against a real AccessKit consumer/AT-SPI session, not only `TreeUpdate` structure. Add app-level regression coverage that enables the access tree, mutates tabs, menus, status, and announcements across multiple updates, and fails on consumer validation or process abort.

### P0-2: IPC deserialization dependency has three known vulnerabilities

**Evidence:** `Cargo.toml` directly pins `rkyv = "0.8.16"`; `cargo audit` reports `RUSTSEC-2026-0233` (UAF), `-0234` (hash-table OOB), and `-0235` (Rc/Arc OOB), all fixed in `0.8.17`. Clay uses rkyv for local IPC archives, so a same-user process that reaches the endpoint can supply crafted frames.

**Required work:** Upgrade rkyv to `>=0.8.17`, regenerate `Cargo.lock`, run codec malformed/archive validation tests plus all Linux gates, and add an expiring audit policy for remaining transitive warnings. Do not add an exception for direct fixed advisories.

### P0-3: Current suite is red and Control Center tests can hang

**Evidence:** Two independent running `cargo test --lib --quiet` processes both report the canonical-example configuration test failed and two Control Center tests beyond the test framework's 60-second warning. This current branch also has ~3,557 net new lines across 73 modified files.

**Required work:** Reproduce each test in isolation with a short Tokio timeout; fix root cause, then run one non-concurrent full Linux test invocation. Prevent recurrence with explicit test deadlines and cleanup assertions for menu sessions/runtime generation replacement. Do not layer further UI work on this branch until `cargo test --all-targets` passes once.

## P1 — next release

### P1-1: Replace prototype default screen with usable editor entry experience

**Visual evidence:** `default-desktop.png` shows a large blank white canvas with only “Welcome to Clay's Phase 4 IPC server.” near the top. It presents a stale milestone message rather than editor identity, file/workspace actions, active status, discoverable shortcuts, or a useful empty state.

**Required work:** Design one Clay-owned empty/workspace welcome state using existing `panel`, `label`, `button`, `kbd` hint, `flex`, and token primitives. Include primary “Open file” / “Open folder” actions, recent/known workspace entry if available, concise shortcut help, connection/runtime diagnostic state, and a real editor first-use state. Keep all package UI inert and token-driven. Validate default, loading, disconnected/error, and narrow-window states visually.

### P1-2: Redesign completion and transient-menu geometry

**Visual evidence:** `focus-check.png` and `command-centre.png` show an empty “Completion / No completions” surface taking the editor's whole width and 35% of its height. This blocks content, reads as an error dialog, and remains visible when no action is available.

**Code evidence:** `src/shell/package_ui.rs:878-886` intentionally gives every bottom menu full main-pane width and 35% height (120–240 px); completion is projected through that generic bottom menu path.

**Required work:** Use existing overlay/portal primitives but give completion a cursor/line-adjacent compact popup with bounded width/height, list scrolling, selected-row visibility, and automatic dismissal for empty/expired results. Keep command/path centre distinct: centered, searchable, modal only when appropriate. Test IME, selection, narrow panes, multi-pane overlap, keyboard navigation, and screen-reader focus.

### P1-3: Establish real UI acceptance and regression evidence

Structural tests are valuable but intentionally do not test production GPU output (`docs/development/ui-observability.md`). The new skill/plan policy now requires screenshots and computer-use accessibility review for all UI plans.

**Required work:** Add a small repeatable Linux GUI review harness: known config fixture, fixed window dimensions, representative document fixtures, portal/desktop screenshot capture, and a retained artifact path. Start with review evidence and crash tests; add GPU pixel goldens only after documented deterministic-render prerequisites exist. This is not a new rendering framework.

### P1-4: Modernize Clay look and feel within Masonry capability

This is a required product task. Aim for a coherent native code-editor product, not a web imitation or an icon-library rewrite:

1. Define visual direction and states: restrained editor-first chrome, clear active/inactive hierarchy, deliberate spacing and typography scale, functional contrast-safe color system, compact but discoverable commands.
2. Apply it consistently to shell, empty state, file browser, tabs, panes, status, menus/completion, command centre, dialogs, settings, diagnostics, and package panels through existing tokens/primitives.
3. Add missing generic primitives only after catalog review: e.g. iconography policy, actionable empty state, toast/progress, compact list-row metadata. Do not add bespoke per-surface widgets.
4. Implement responsive/pane-constrained layouts and high-DPI/font-scale checks; retain keyboard and AT semantics as non-negotiable.
5. Review screenshots/a11y tree for light, dark, user typography, narrow/wide, empty, busy, error/recovery, multi-pane, multi-tab, and overlay states.

### P1-5: Restore a reliable, focused local feedback loop

`target/` is currently 17 GiB, and background test runs are long enough to overlap and contend. The earlier suite consolidation was useful, but current practice still permits conflicting background full runs.

**Required work:** Document one supported quick suite and one serial full suite; use a repo-local lock/CI job policy so heavyweight GUI/V8 tests are not run concurrently by automation; emit timeout diagnostics for deadlocked async tests; report artifact size. Avoid creating alternate target trees by default.

## P2 — architecture/refactoring

### P2-1: Split orchestration by existing responsibilities

The source/test/bench/package inventory is 199,302 lines. Several files remain difficult to review safely: `src/server/js_runtime.rs` (~549 KB), `src/server/connection.rs` (~455 KB), `src/editor/surface.rs` (~308 KB), `src/server/mod.rs` (~239 KB), `src/masonry_shell.rs` (~226 KB), `src/packages/record.rs` (~216 KB), and `src/main.rs` (~152 KB).

**Required work:** Extract plain modules/functions, not new trait hierarchies:

- connection dispatch families plus one lifecycle/cleanup owner;
- runtime facade source/validation/domain bootstrap;
- shell tab/window layer, overlay coordinator, and accessibility virtual-node builder;
- package record contribution-family validators; and
- app event routing split from launch/CLI/window creation.

Keep the existing authority boundaries and hot paths explicit. Each extraction needs behavior-parity tests; do not combine it with UI redesign.

### P2-2: Centralize virtual accessibility-node construction

There are three status-node implementations and multiple virtual-node paths. Introduce one small internal helper that owns stable ID derivation, bounds, parent attachment, and update semantics. Reuse it for status, TabList, menu rows/result count, and live announcements. This both fixes P0-1 and prevents new a11y tree corruption.

### P2-3: Make UI state ownership legible

The current tree combines `Driver`, `ClayShellWidget`, `EditorWidget`, `PaneDocumentView`, `PackageOverlayHost`, and server menu sessions. Preserve it, but write a one-page state/ownership map and extract only duplicated bridge code. In particular, command-centre presentation should have one owner for session lifecycle, focus restoration, visual geometry, and accessibility—not parallel state mirrored across driver/editor/overlay host.

### P2-4: Reduce test-only source churn

Documentation and conformance checks are now substantially better than the historical prose-needle tests, but new feature work still added 283 lines of static performance-invariant test assertions and 307 lines of visibility mapping assertions in one change. Prefer compact reusable check helpers and focused behavioral tests where a source-text assertion does not protect a unique contract.

## P2 — performance

- Measure editor typing, menu filtering, tab switching, and a11y-update cost after P0-1. Repeated `WidgetId::next()` a11y allocation/churn must disappear before interpreting menu benchmarks.
- Make transient-menu list virtualization/row bounds explicit before increasing item counts; completion should not relayout a full-width surface on every keystroke.
- Profile exact Control Center timeout/hang path before optimizing. It is a correctness issue first.
- Keep current wins: parsing, filesystem I/O, package execution, and external processes stay off paint/layout/keypress paths. Do not reintroduce them during UI modernization.

## P2 — security hardening

- Resolve direct rkyv advisories first.
- Triage unmaintained/unsound transitive packages (`event-listener`, `bincode`, `paste`, `ttf-parser`) by actual reachable path and upstream update availability; track each exception with owner/expiry.
- Keep existing package principal, connection-identity, bounded-read, atomic-save, connection-cap, and two-runtime-domain tests as blocking. This review found no evidence to weaken them.
- Add malformed archive fuzz/property inputs after the rkyv update, targeting the same codec boundary that deserializes untrusted local IPC bytes.

## P2 — test coverage gaps

1. Real AT-SPI/AccessKit consumer lifecycle, including repeated virtual tree updates — missing and now proven necessary.
2. Screenshot-based acceptance across UI states — absent; structural snapshots are not visual proof.
3. Test timeouts/cancellation for menu/runtime reload — missing or ineffective, given current hangs.
4. Property/fuzz coverage for protocol archive and key-sequence/menu state machines — thin compared with trust boundary importance.
5. Real multi-window DPI/font-scale/Wayland validation — not evidenced by unit tests.
6. End-to-end default onboarding, empty completion dismissal, command-centre filtering, and recovery UX — no stable visual regression evidence.

## Xilem evaluation

### Recommendation: do **not** adopt Xilem in production now; schedule a bounded compatibility spike after P0/P1 stability

Clay already uses Masonry 0.4 plus `masonry_winit` as a custom retained-widget host. Clay's editor, pane document view, retained SDUI reconciliation, package UI, and imperative driver rely deeply on Masonry widget IDs, direct `RenderRoot` mutation, and custom paint/event hot paths.

Current Xilem documentation describes Xilem and Masonry as **experimental**. Xilem is a higher-level reactive view architecture built on Masonry; it is not a renderer replacement. It may reduce boilerplate for conventional shell UI, but it does not solve current rendering/accessibility defects and a full migration would create a second UI state/update model while the editor must remain bespoke.

**Potential fit, later:** conventional Clay-owned, low-frequency screens—welcome/onboarding, settings, package management, static inspector panels, and perhaps file-browser composition—provided they can coexist with current Masonry version and host the existing bespoke editor widget without input/focus/a11y regressions.

**Do not migrate:** editor text canvas; client/server edit synchronization; pane document hot paths; package-declared SDUI runtime; shell/pane/tab infrastructure until a spike proves ownership/focus/reconciliation compatibility.

**Spike acceptance criteria (time-boxed):**

- Pin exact compatible Xilem/Masonry versions in an isolated branch; no blanket dependency upgrade.
- Build one noncritical shell surface around the existing editor as an opaque retained child.
- Prove unified window/event loop, focus traversal, AccessKit tree stability, theme-token mapping, command routing, and no per-frame reactive rebuild on typing.
- Measure cold start, tab switch, and typing latency against current baseline.
- Delete the spike if it cannot host the editor cleanly or adds duplicate state/ownership.

**Evidence:** Context7 documentation for `/linebender/xilem` identifies Xilem as experimental, says it is reactive and built on Masonry, and shows `xilem_masonry` as the Masonry backend. Masonry documentation positions Masonry as a foundational retained-widget toolkit and suggests Xilem for application-level abstractions. This supports a narrow experiment, not wholesale replacement.

## Priority execution order

1. P0-1 stable a11y virtual-node IDs + live AT regression test.
2. P0-2 rkyv 0.8.17 upgrade + malformed-codec tests/audit clean.
3. P0-3 fix failing/hanging tests; one successful serial all-target Linux run.
4. P1-1 default empty/onboarding state and P1-2 compact completion/menu UX.
5. P1-3 reusable screenshot/a11y acceptance harness, then execute P1-4 visual modernization across every core surface.
6. P1-5 test/target feedback-loop cleanup.
7. P2 refactor by responsibility, then run Xilem compatibility spike only after baseline is green.

## Strengths to preserve

- Server-owned document/file/package authority and bounded client/server routing.
- Two runtime trust domains with host-stamped package provenance.
- Typed theme tokens, contrast validation, inert package UI, and retained Masonry reconciliation.
- Explicit hot-path boundaries and performance budgets.
- Structural documentation/conformance tests as a complement—not substitute—to live UI review.
