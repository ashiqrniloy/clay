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
## 26-08-24 12:49 — tauri-react-visual-review

UI review repeatedly hit same AT-SPI/tooling friction: whole desktop enumeration blocks on a blank child, and the existing capture harness still matched native `clay`/`Clay working area shell` identifiers after Tauri cutover; wrapper cleanup also left `clay-desktop` children until explicitly tracked. Workaround: per-index bounded probes, current app/landmark matching, and explicit child cleanup. Prevent recurrence with one bounded AT-SPI discovery API keyed to app PID/name plus process-group ownership in the harness.
## 26-08-25 14:21 — edit-exact-duplicate-replacements

functions.edit requires every oldText to be unique and cannot select occurrence or replace all exact matches. Protocol field migration produced repeated identical test-fixture blocks, forcing multiple large context replacements and one count-asserted Python replacement. Add occurrence selection or explicit replace-all/count support to prevent repetitive retries.
## 26-08-26 01:43 — ui-review-hmr

Plan 098 fixture review hit existing Vite HMR fragility again: after source/build updates, the lazy fixture route held an invalid promise element and browser capture timed out. Workaround was restarting Vite before recapturing. Prevent repeat by making the fixture harness force a clean dev-server/page reload or by adding a small health check that rejects stale HMR module graphs before capture.
## 26-08-26 04:12 — Plan 098 real desktop manual test

Plan 098 desktop manual run hit repeatable host tooling friction: GNOME/portal focus moved Clay and the GTK chooser off-screen, and AT-SPI exposed only the native frame; repeated move/resize/coordinate retries were needed before a synthetic file could be selected, then loaded-editor interaction still could not be verified. Prevent with deterministic Tauri window placement/targeting plus a stable WebKitGTK AT-SPI bridge or a committed automation path that does not depend on portal focus.
## 26-08-27 04:15 — post-edit-format-order

Frontend format check failed twice because edits were made after the prior formatting pass (session.ts was reformatted, then edited again). Prevent by running formatting only after all source edits, or use a final changed-file format gate before the full suite.
## 26-08-28 16:01 — Tauri visual capture offscreen/stale-binary friction

Visual capture repeatedly launched Clay partly off-screen and portal screenshots captured unrelated desktop regions; workaround was list_windows → get_app_state → activate/resize/move → immediate targeted/portal capture. Capture helper should pin a visible window position/size or use targeted screenshot output directly, and should rebuild/validate the exact desktop binary before runs.
## 26-08-28 22:25 — legacy markdown formatting

Legacy Markdown references are not Prettier-clean; whole-file --write creates unrelated churn and can alter literal wildcard prose. Repeated workaround was targeted/manual edits plus source-aware doc tests. Add a repository Markdown formatting baseline or per-directory check so new docs can be validated without rewriting historical files.
## 26-08-29 00:02 — cargo test filter arity

Cargo test accepts one positional filter, but repeated attempts passed multiple test names and failed before running. Workaround was one `cargo test --test protocol` full suite. A helper or documented multi-filter wrapper would prevent this recurring CLI retry.
## 26-08-30 14:40 — repeated self-kill via pkill -f + orphaned clay processes during manual UI testing

Manual server/client orchestration twice shot my own shell down: pkill -f/pgrep -f patterns matched the bash -c command string itself, killing the parent shell mid-block (zero output, steps silently skipped), which then looked like app bugs. Also `clay server` ignores SIGTERM (needed -9) and clay-desktop children survive killing the `clay client` wrapper pid, leaving orphan windows that corrupted screenshot pixel probes. Need a repo helper script for isolated clay sessions with kill-by-exact-name (-x), child-process cleanup, and self-match-proof pgrep patterns — capture-ui-review.sh exists but is fixture-specific.
## 26-09-06 20:29 — computer-use click drift onto unrelated windows during UI review

The computer-use click flow drifted: after a failed click attempt the session focus moved off the Clay review window (clicks at screen coords started hitting the user's unrelated Firefox/terminal windows, one showed personal content). Root causes: (1) no guard that the Clay window is still focused before each synthesized click, (2) long manual click-coordinate loops instead of AT-SPI perform_action on the located node, which would remove coordinate drift entirely. Fix for future UI-review runs: use Atspi perform_action("click"/"press") directly on the target accessible node, and re-verify focused_window == Clay before every input event.
## 26-09-07 04:42 — Tauri embedded-frontend staleness + sandbox process reaping during live debugging

Two hours lost to an invisible-failure class: Tauri v2 embeds frontend/dist into the clay-desktop binary at compile time, so `npm run build` without a subsequent `cargo build -p clay-desktop` silently serves the stale bundle — the app looks rebuilt but runs old frontend code. Compounded by the sandbox killing backgrounded GUI processes between bash calls (SIGTERM to clay-desktop within seconds, inconsistently), which made every live launch-and-probe cycle a coin flip. Next time: after ANY frontend change, always rebuild the desktop binary before live verification, and check `strings target/debug/clay-desktop | grep <new-asset-hash>` (or check the bundle hash in the served index.html) before concluding a client-side bug is unfixed.
## 26-09-11 05:23 — ui-visual-verification-tooling

Design-artifact QA (4 HTML files × 6 themes × 4 viewports = 96 visual states) needed three things this host did not provide, and each cost several turns of workaround: (1) no Pillow/ImageMagick, so cropping-and-zooming screenshots to read 11px text meant hand-writing a stdlib zlib PNG decoder+cropper; (2) no Playwright/Puppeteer module even though browser binaries are cached in ~/.cache/ms-playwright, so JS-error + overflow + per-theme verification meant hand-writing a ~90-line raw CDP client over Node's built-in WebSocket; (3) no headless screenshot flag for "click this selector first", so state-dependent screens (palette open, tab switched, transcript after send) needed ad-hoc RUN-expression plumbing bolted onto that same script. Prevention: ship a tiny repo-local UI verification helper (screenshot + console-error + overflow probe + optional pre-screenshot JS + crop/zoom) in scripts/, documented for agents, so visual review doesn't start from zero each session.
## 26-09-11 21:36 — ad-hoc CDP probe rewrites

Third time this session I hand-rolled a throwaway /tmp CDP probe (probe2.mjs + /tmp/expr.js) to inspect computed styles and scene visibility, then threw it away. The checked-in verifier exists but has no "evaluate one expression against one page/state" mode, so every "why is this element hidden?" question costs a fresh 30-line driver. Next time: add `--eval='<expr>' --page=<id> --theme=<t> --scene=<s>` to design-artifacts/tools/capture-prototypes.mjs and delete the ad-hoc probes for good. Also cost: /tmp/cdp.mjs silently ignored a RUN env file and printed its own probe, which briefly looked like the page was clean when I was asking a different question — a stale helper lying is worse than no helper.
## 26-09-11 22:57 — verifier measured stale document

The prototype verifier lied for several runs and I debugged it with four throwaway CDP probes (again). Root cause: it trusted `Page.loadEventFired`, which can arrive from the *previous* document, so the probe measured a half-built page — reporting the component catalog's clipped boxes under `shell`, an empty tree, and `--c-surface` from a stale sheet. The fix (confirm `document.URL` committed, wait for the theme to resolve, screenshot before behaviour probes) took one line each; finding it took ~40 minutes of ad-hoc scripts because the tool reported plausible-looking numbers instead of failing loudly. Two process fixes: (1) any harness that navigates must assert it is looking at the document it asked for — a mismatch should be a hard error, not a measurement; (2) stop hand-rolling /tmp probe files: add `--only=<page>:<theme>:<state>` (now done) plus a `--eval=<expr>` debug mode so the next "why is this element invisible" question is one command, not a new 30-line script. Also: the verifier's `remote requests: 0` claim was vacuous all along because `performance.getEntriesByType('resource')` returns nothing for file:// URLs in this Chrome — the "no remote resource" assertion needs a real check (DOM scan for non-file:// URLs plus a network-domain listener), otherwise it is a green light wired to nothing.
## 26-09-13 21:22 — at-spi-wedge-and-pkill-self-match

Two workflow traps cost ~40 minutes in the visual-review run and will recur for anyone capturing the desktop build. (1) AT-SPI wedged: after the bus was restarted, `Atspi.get_desktop(0).get_child_at_index(i)` hung forever and the harness reported "Clay window/accessibility shell did not appear" for every capture. Root cause: stale `/run/user/1000/at-spi2-*V3` per-app dirs plus an old `/run/user/1000/at-spi/bus` socket; hundreds of leftover headless-Chrome processes (each registering with AT-SPI) made it worse. The fix that worked, and should be in the harness as a preflight: kill `at-spi2-registryd` + `at-spi-bus-launcher`, `rm -rf /run/user/1000/at-spi2-*V3 /run/user/1000/at-spi/bus*`, then let dbus reactivate `org.a11y.Bus`. Capture scripts should also reap their own Chrome processes (my dev-capture script used detached+unref and leaked ~157 of them). (2) `pkill -f <pattern>` matched the calling shell's own command line twice (patterns like "clay-desktop" and "clay-review-matrix" appeared verbatim in the `bash -c` string), so the call died silently mid-script with no output. Always write those patterns bracketed ("clay-deskt[o]p") in scripts and in agent commands.
## 26-09-13 23:44 — Wiki "verify unchanged" task has no coverage for the pages it must verify

Doing plan 118's "Update or verify the code wiki after implementation" task, the docs-as-code guards only scan a hard-coded list: `current_state_docs_reject_removed_native_architecture_terms` checks ~13 `docs/` pages (+ docs/reference/primitives), and `wiki_navigation_is_complete_and_current_page_paths_resolve` validates `src/...` code-span paths for exactly 7 wiki pages. Result: 26 evergreen `docs/wiki/modules|flows/*.md` pages still cite `src/masonry_{editor,package_region,sdui,sdui_region,pane_document}.rs` — files the Phase 12 cutover deleted — and nothing fails. I only found this because the plan-118 surface I had to refresh (centered command centre) happened to be one of them, and I had to hand-audit each page I touched for dead paths. Prevention: extend the path-existence scan from those 7 pages to every evergreen wiki page (archive/ exempt), and let the failure list be the work queue; then add banners + archive moves for the ~26 pages in one pass. That turns "verify the wiki" from a manual grep-and-pray into a gate.
