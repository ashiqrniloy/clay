# VENT

Feedback log. Repeated/systemic workflow friction that should become future automation, docs, or workflow fixes.

## 26-08-12 01:15 — conformance-test-target-discovery

UI conformance checks initially used direct cargo test targets, but Cargo.toml sets autotests = false and registers only suite wrappers; both direct targets failed before rerunning through cargo test --test editor filters. Add a project test-command map or expose stable wrapper aliases so plan tasks can invoke conformance suites directly.
## 26-08-12 17:16 — git-checkout-destroyed-uncommitted-phase-work

During the real_server_end_to_end harness fix, I ran `git checkout -- src/server/connection.rs src/client/mod.rs src/protocol/codec.rs` to strip temporary debug instrumentation and accidentally destroyed ~1100 lines of UNCOMMITTED Phase 24.1/24.2 working-tree changes in connection.rs and client/mod.rs (the Plan 081/082 preserved baseline). Recovery required: finding a mid-24.1 stash commit via `git fsck --lost-found` (4a3eb152, only partial state), then manually reconstructing the task-4/6/8/11 wiring and 7 tests from session observations — hours of rework with real risk of subtle divergence. Prevention: never use `git checkout -- <file>` to revert debug edits when the tree contains long-running uncommitted work; instead revert debug lines with targeted edits (or `git stash push -- <paths>` first), and commit/stash the working tree before starting a new task. A project rule like "phase working trees must be committed at task boundaries" would have made this a non-event.
## 26-08-13 03:48 — ctx7-masonry-lookup

Context7 library lookup could not resolve Linebender Masonry after two targeted queries (returned unrelated GTK/layer libraries); workaround used the already-known `/linebender/xilem` docs ID plus version-exact local Masonry 0.4.0 source. Prevent by indexing Masonry as a first-class Context7 library or documenting the canonical ID.
## 26-08-13 07:28 — plan-doc-sync

Plan 084 task execution needed repeated manual doc/catalog synchronization and task checkbox/record edits across plan, wiki, reference docs, and test-plan files; one manual-step number collision was introduced and fixed, and static guard tests initially overfit implementation strings then were simplified. Add a plan-completion helper/checker that validates task records, numbered manual tables, and required catalog/wiki paths before final gates.
## 26-08-14 17:18 — plan086-task8

Visual review required repeated workarounds: GNOME window introspection/targeting unavailable, portal clicks refused coordinate mapping, and every shell command raised another desktop window so screenshots had to be captured before bash then cropped with a custom PNG tool. More importantly, AT-SPI grab_focus on Clay's top-level Frame crashed Clay (`Cannot send event to non-existent widget #8`), while focusing the editor Entry worked. Prevent by making window targeting/raise available in the host and guarding top-level accessibility focus events in Masonry/Clay.
## 26-08-14 17:47 — test-filter-path

Targeted lib test filter with `--exact` matched zero because Clay's test path includes `server::configuration::tests`; the successful workaround was rerunning with the bare function-name filter. This exact-name mismatch already occurred for prior config tests; add a documented test-filter helper or make suite commands use fully qualified names so zero-test runs cannot look like verification.
## 26-08-14 18:05 — plan086-task11-manual-run

Manual task 11 repeated host friction: computer-use portal/window-targeting could not safely focus or select Clay controls, so native dialog, observer, and full quit/relaunch steps had to be marked blocked; AT-SPI entry grab_focus only changed accessibility focus, not OS keyboard focus. Separate product blocker surfaced during dirty Ctrl+Alt+W pane close: client panicked in accesskit_consumer with 'Focused ID #4 is not in the node list' while server survived. Prevent recurrence by exposing reliable Wayland window targeting and adding a regression test/guard for focus reconciliation when removing a dirty focused pane.
## 26-08-14 19:02 — missing-timing-tool

Baseline timing command hit missing /usr/bin/time three times; workaround used shell date +%s%N. Prefer a portable timing helper or document coreutils as a host prerequisite for repeatable benchmark scripts.
## 26-08-14 19:49 — atspi-probe-hang

Plan 087 harness runs hit the host's Python GI AT-SPI cache edge: walking desktop children hangs on GNOME Shell/stale `/org/a11y/atspi/cache` even while computer-use `get_app_state` works. Workaround now probes top-level app indices in timed subprocesses, then walks only Clay; missing/hung probes write UNRESOLVED. Prefer a stable app-enumeration/targeted AT-SPI API for future capture tooling.
## 26-08-14 20:21 — cargo-test-filter

Cargo test accepts one positional filter only; attempting to pass multiple module filters fails before running tests. Workaround: use one shared prefix filter (for example `masonry_`) or run separate commands; a small helper/documented convention would prevent repeated wasted retries.
## 26-08-14 21:31 — ui-review-window-targeting

Plan 087 completion visual capture retried twice: isolated Clay and AT-SPI tree came up, but computer-use-linux could not target/focus Wayland windows because GNOME Shell Introspect/window backend is unavailable; un-targeted portal key events reached the wrong focus and could not prove completion. Workaround was structural/accessibility tests plus an explicit UNRESOLVED artifact. Prevent with a working GNOME window-target backend or a fixture-level deterministic keyboard-input hook; never treat blind portal input as visual validation.
## 26-08-15 04:01 — computer-use-window-targeting

Two UI-baseline attempts hit same GNOME window-targeting failure: `list_windows` and targeted `press_key` both cannot access a supported window list (GNOME Introspect denied; extension unavailable). Workaround was fixture-only captures and explicitly unresolved settings/narrow/wide states. Prevent recurrence by enabling the computer-use GNOME window-targeting extension/session permission or documenting this host as visual-review-limited before agents start interactive capture.
## 26-08-15 04:44 — ui-review-harness-repo-resolution

Temporary UI capture scripts copied to /tmp fail because capture-ui-review.sh derives repo from its own location, yielding `/` and `cargo build` cannot find Cargo.toml. Repeated workaround hardcodes `/home/arn/Projects/clay`; prevent by adding a `--repo`/environment override or resolving Git root before relocating the harness.
