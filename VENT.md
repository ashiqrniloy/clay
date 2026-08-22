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
## 26-08-15 06:00 — computer-use-window-targeting

Task 5 interactive overlay capture hit same GNOME Wayland limitation: targeted window focus fails because GNOME Shell Introspect/window-targeting extension is unavailable, and an unscoped portal Ctrl+Alt+P could not be verified to open Clay's Command Centre. Workaround was a live TTY harness plus global input attempt; capture stayed UNRESOLVED, so no interactive visual pass was claimed. Prevent recurrence by enabling the GNOME window-targeting backend or adding a harness command/action path that opens review surfaces without compositor focus.
## 26-08-15 16:56 — computer-use-window-targeting

Plan 088 Task 8 interactive visual/a11y review repeated the GNOME Wayland window-targeting failure: can_query_windows=false/can_focus_windows=false, coordinate clicks were refused as unsafe, targeted app_id input failed, and global portal chords landed in the focused Firefox instead of Clay. Workaround was isolated TTY captures plus semantic AT-SPI inspection and retained pre-task screenshots; completion/Command Centre/file-browser/settings/narrow-wide remained explicitly unresolved rather than false passes. Prevention: enable GNOME window-targeting/Introspect backend or add a harness action that opens review states without compositor focus.
## 26-08-15 18:28 — manual-test-plan-window-targeting

Plan 088 Task 12 hit same GNOME Wayland window-targeting blocker again: AT-SPI and portal screenshots work, but can_query_windows/can_focus_windows remain false, coordinate clicks are refused, and global chords land in the focused app. Repeated workaround: isolated capture harness plus AT-SPI/structural tests and explicit BLOCKED/UNRESOLVED records. Prevention: enable GNOME window-targeting/Introspect and log out/in, or add a Clay harness path that opens representative states without compositor focus.
## 26-08-16 02:09 — ui-review-harness-loading-readiness

Plan 089 loading review needed repeated manual debugging because capture harness's `Loading workspace` readiness marker can be supplied by the opened fixture document, while AT-SPI dynamic SDUI child names were intermittently blank; runtime/client logs plus a Clay-window crop were required to distinguish true SDUI rendering. The supplied crop_clay_review.py also failed on current gold-border rows (top threshold missed), requiring fixed-offset pure-Python cropping. Prevent recurrence by making fixture/document markers distinct or exposing a structural runtime-tree probe, and make crop detection tolerate 1px border variance.
## 26-08-17 02:40 — visual-review-harness

Plan 089 visual review hit two repeatable harness gaps: scripts/capture-ui-review.sh only builds target/debug/clay when binary is missing, so source edits can be visually re-tested against a stale binary; make capture runs rebuild or accept an explicit build fingerprint. Native Open File semantic action opened an ambient Pick Files chooser outside the private fixture root, exposing real user locations and blocking safe selection; add a no-focus fixture action or an isolated dialog path before retrying file-browser/settings review.
## 26-08-17 18:54 — UI screenshot capture off-screen

Visual review harness repeatedly captured unrelated primary-monitor content because Clay launches at stale/off-screen compositor bounds (~x=1845 on a 1920px capture). Repeated workaround: activate Clay, move/resize via GNOME backend, targeted portal screenshot, GNOME ScreenshotWindow, interactive portal, X11/ximagesrc; move left bounds unchanged, target returned only 75px, ScreenshotWindow denied, interactive returned code 2, ximagesrc black. Prevent next retry by making capture harness resolve the actual monitor/window surface or resetting compositor window placement before launch; invalid screenshots had to be deleted because they contained unrelated user desktop data.
## 26-08-17 20:07 — live-atspi-probe

Manual live AT-SPI smoke rerun failed twice because the desktop-wide probe could not discover Clay, even though the isolated capture-ui-review harness found Clay and produced valid Clay-only trees. Repeated workaround was to use the harness and retain prior Plan 089 live evidence. Prevent this by making live probes target the spawned Clay PID/application directly or isolating AT-SPI desktop state before polling.
## 26-08-20 13:47 — lsp-shared test module resolution

Direct `node --test packages/lsp-rust/rust-package.test.mjs` and the full protocol matrix fail before tests because `dist/server.js` imports `lsp-shared` through the Rust harness-only register hook. Repeated workaround: run via `cargo test --test runtime lsp_bridge::...`, which injects `tests/fixtures/lsp/register-lsp-shared.mjs`. Prevent with a package-local Node test command or stable workspace resolver so direct and CI invocations share module resolution.
## 26-08-20 14:23 — Wayland live-review window targeting

Live visual review repeatedly lost Clay's compositor target: GNOME window bounds alternated between visible and off-screen, window IDs changed after each AT-SPI refresh, and move/resize calls reported success without stable geometry. Workaround was repeated list_windows/activate_window/screenshot cycles plus the TTY capture harness; keyboard eventually worked only when the harness kept a FIFO-backed TTY. Prevent with a stable window handle/targeted screenshot API that does not remap Wayland foreign-toplevel IDs between queries.
## 26-08-20 15:09 — stale Phase 28 test baselines

Full `cargo test --all-targets` exposed repeated stale Phase 28 baseline assumptions: hardcoded facade/op counts, behavior-version/command-count assertions, FoldingRangeSet message ordering, package payload estimates, and fixture exports. Workaround was rebaseline Plan 061 + extension counts and rely on focused API/doc/security gates; remaining runtime fixture failures still need their own phase-specific rebaseline. Prevent with generated command/message/package fixture expectations or one Phase 28 post-implementation baseline task instead of scattered literals.
## 26-08-20 16:08 — full-suite baseline drift

After rebaselining three configuration-manifest assertions for the Phase 28 `Ctrl+/` default, full Linux `cargo test --all-targets` improved to 1636 passed/11 failed. Remaining failures are the same older Phase 28/runtime fixtures (folding message/payload baselines, package exports/duplicate commands, native syntax ownership, menu activation); focused configuration/API gates are green. Generated phase baselines would avoid repeatedly triaging these unrelated failures during later tasks.
## 26-08-21 00:19 — manual-gui-tty-blocker

Manual GUI harness still blocks interactive Phase 28 capture because scripts/capture-ui-review.sh requires a TTY before keyboard input; both completion and Command Centre attempts hit the same blocker. Repeated workaround: retain partial AT-SPI dumps, rely on focused structural tests, and mark live rows UNRESOLVED. Prevent next time by adding a supported non-TTY input bridge or a computer-use-driven harness mode that accepts injected key events and records the same evidence.
## 26-08-21 01:28 — wayland-keyboard-backend

P1 GUI recapture hit broader Wayland input friction after the TTY workaround: computer-use reports no keyboard-capable backend (`/dev/uinput` denied, no xdotool/ydotool, RemoteDesktop AvailableDeviceTypes=0), so no-op edit and `Ctrl+Alt+I` cannot be delivered. Workaround was to preserve paired UNRESOLVED artifacts with AT-SPI/client/server logs and prove worker resolution structurally. Prevent next time by enabling a keyboard-capable RemoteDesktop portal or a supported ydotool/uinput backend in the review host.
## 26-08-21 03:29 — live-atspi-desktop-scan

Live AT-SPI aggregate smoke repeatedly scans the entire desktop tree and timed out despite the targeted Clay fixture working (`supports_editable_text=true`). Workaround: use GNOME window/PID-targeted AT-SPI inspection and retain explicit host-blocked status. Prevent next retry by making live smoke locate the Clay application/window by PID or bounded app-name polling before traversing unrelated Firefox/GNOME trees.
## 26-08-21 04:30 — p2-visual-recapture-input

P2 recapture repeated the same Wayland limitation: static fixtures pass, but completion/Command Centre/fold/link/transform/inlay-toggle/resize states cannot be driven because doctor reports no keyboard backend (`/dev/uinput` denied, no xdotool/ydotool, RemoteDesktop AvailableDeviceTypes=0). Workaround was static capture plus explicit UNRESOLVED artifacts and structural/security tests. Prevent recurrence with a deterministic no-input fixture action path or keyboard-capable review host.
## 26-08-22 17:55 — ui-review-harness-input

UI review needed repeated temporary fixture directories and copied capture scripts because `scripts/capture-ui-review.sh` hard-codes fixture names and has no generic `--init`/package fixture path. Interactive menu verification also repeated as unresolved because Wayland host lacks keyboard input (`/dev/uinput` denied, no xdotool/ydotool/portal keyboard). Add a generic isolated package/init fixture option and a documented semantic/keyboard backend check to avoid this workaround.
