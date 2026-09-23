# Clay Manual Test Plan — Index

Manual verification guide for the whole Clay application. Automated suites
(`cargo test`, 4 declared suites) gate every change; these documents cover
what only a human at a real keyboard/screen can verify: rendering, blink,
ligature glyphs, IME feel, native dialogs, focus, timing.

## How to use this plan

1. Build once: `cargo build` (or `cargo run` which builds on demand).
2. Pick the module file(s) relevant to what you changed — the table below.
3. Each module file is self-contained: setup, numbered steps with expected
   results, negative checks, and known ceilings that are NOT bugs.
4. Record results inline (copy the table or keep notes). Failures that match
   a file's "known ceilings" section are expected behavior, not defects.
5. Every plan document that changes user-visible behavior must update the
   affected module file(s) and this index (enforced by the create-plan skill
   manual-test-plan task).

## Prerequisites (all modules)

- Linux host (primary platform), Rust toolchain, `cargo`.
- Optional: `~/.clay/` config tree (canonical example: `examples/` — copy with `cp -r examples/. ~/.clay/`).
- Scratch workspace: `mkdir -p /tmp/clay-manual` with sample files (each
  module file lists the files it needs, or points at a shared setup).
- Font for ligature checks: Fira Code (`FiraCode Nerd Font Mono` works).

## Plan 097 Phase 12 Tauri/React visual and accessibility review (2026-08-24)

Current desktop launch is Tauri v2 + React: `clay` and `clay client` launch
`clay-desktop`; `clay server` remains standalone. The dated review artifact is
`code-reviews/screenshots/2026-08-24-tauri-react-parity/`.

The review retains 20 app-only CDP screenshots and paired AX snapshots at
1440×900 and 780×900 for editor, intelligence, package UI, settings,
Command Centre (active/empty), Path Browser, the chat surface (since removed
by plan 118 — historical), splits, and combined loading/empty/error states.
Real Tauri AT-SPI dumps cover the then-welcome landing, the opened editor,
tabs/splits, and the then-chat surface. Static visual and rest-state accessibility checks
pass. Keyboard-only completion, command/path activation, native dialog,
settings, and tab/pane interaction remain explicitly `UNRESOLVED`: this host
has denied `/dev/uinput`, no `xdotool`/`ydotool`, and no Wayland portal path that
can target Clay. Full-desktop portal screenshots with unrelated windows were
removed; no retained screenshot contains host paths or secrets.

Review fixes: editor labels no longer expose an absolute workspace fallback,
and the shell connection status is a polite live region. The only low-priority
follow-up is the unselected Settings theme control's repeated `Theme Theme`
accessible name. See the per-module records below and
`docs/development/accessibility.md` for the current role contract.

## Plan 097 manual-test-plan execution record (2026-08-24, post-cutover)

Executed against the current `target/debug/clay` / `clay-desktop` Tauri build
via `scripts/capture-ui-review.sh` (isolated config/data/workspace per run):

| Fixture | Result | Evidence/notes |
|---|---|---|
| ui-review-default | PASS | AT-SPI tree exposes `clay-desktop` frame, `Clay workspace`, `Window tabs` page-tab list (selected `Workspace` tab), `Pane 1`, named Open File/Open Folder actions, and status bar |
| ui-review-large-typography | PASS | Large UI typography renders in bounds; controls legible |
| ui-review-command-centre | UNRESOLVED | Interactive state requires a TTY for keyboard capture — documented host ceiling (no `/dev/uinput`, no xdotool/ydotool, no Wayland portal input path) |
| ui-review-error | UNRESOLVED | New finding: the sanitized runtime diagnostic never appears in an AT-SPI name dump because WebKitGTK does not expose static text inside the footer/live region as accessible names or Text-interface content (verified with a targeted AtspiText probe; even the `Connected` status text is invisible to AT-SPI). Diagnostic delivery itself is covered by automated tests (server diagnostic broadcast → workspace-controller handling → `app-shell.tsx` resolvedStatus render). Follow-up: expose footer/live-region text to AT-SPI, then re-enable this step |
| Frontend production build budgets | PASS | shell 160.6/180 kB gzip; total 343.2/400 kB gzip |

Modules 05 (movement/selection), 06 (multi-cursor), and 12 (Windows) received
dated status records below: interactive keyboard steps stay UNRESOLVED on this
host; their logic is pinned by frontend editor tests and CodeMirror built-ins.
Stale native-era step references (deleted Masonry unit tests and wiki deep
references) were replaced with current equivalents in modules 01, 04, 07, 10,
13, and 14 — no existing behavior step was weakened.

## Plan 110 manual-test-plan execution record (2026-09-06, task 14)

Module 15 gained UI-DS-21…26 for plan 110 task 10's design-system selection UX
(Settings dropdown, command surface, invalid-specifier surface, server-enumerated
type list, appearance persistence, DS × theme visible differences). Executed on a
freshly rebuilt desktop build:

| Check | Result | Evidence/notes |
|---|---|---|
| UI-DS-21…26 automated legs | PASS | Settings-panel snapshot-driven choices + no-remount switch, validator allowlist/rejection, persistence + appearance-across-restart e2e, theme enumeration from enabled records (see module 15 record) |
| Real-app captures (`ui-review-default`, `ui-review-design-system`, `ui-review-design-system-light`, `ui-review-error`) | PASS | `test-plan/artifacts/110-ui-design-systems/`; window-cropped portal screenshots |
| Real-app package activation (removed Neobrutal/Glass harness states) | SUPERSEDED | Those two design-system packages were **removed** (plan 118 task 9) and their fixture states deleted; `ui-review-design-system` / `…-light` now capture the shipped `@clay/design-instrument`. The plan-110 reload deadlock was fixed in plan 110 task 18, so the replacement states are capturable. Fixture-layer DS × theme visual evidence from that era remains in `.impeccable/reviews/110-final/` |
| Capture tooling | IMPROVED | `scripts/capture-ui-review.sh`: waits for fixture SDUI trees, crops portal screenshots to the Clay window (never retains full-desktop captures with host windows); the removed fixture names are rejected by the argument check (`plan118_ui_review_harness_captures_the_shipped_system_and_rejects_removed_states`) |
| Build hygiene | NEW CEILING | Mixed stale/fresh binaries fail client-side rkyv deserialization (`ArchivedSduiTree` subtree pointer overran) and surface as `Session lost` — rebuild both `clay` and `clay-desktop` before captures |

## Plan 109 manual-test-plan execution record (2026-09-06)

Module 17 gained steps C1–C20 and negative checks C-N1–N4 covering every
user-visible plan 109 behavior (workspace binding, model/effort controls
and rebinding, full transcript with tool/skill/thinking/steer rows, Files
tab editor view + Ctrl+B toggle, context inspector with compaction
reflection, OM activity + worker-model retention, /resume restore,
Session Info auto-select, branch/extension strip, daemon-sourced slash
completion). Executed on a freshly rebuilt Linux build:

| Check | Result | Evidence/notes |
|---|---|---|
| Automated legs C1–C20, C-N1–N4 | PASS | cargo test green (lib 1227, protocol 208); clay-agent 97/1 skip; frontend 250; per-step suite citations in the module 17 record |
| Live-build launch gate | PASS | `test-plan/artifacts/109-coding-agent/launch-gate/` (fresh `cargo build --bin clay`, isolated config/socket, AT-SPI tree exposes the Coding Agent entry, window-cropped portal screenshot) |
| Interactive keyboard steps | UNRESOLVED | Documented host ceiling (no TTY/uinput input path) — unchanged since plan 097 |
| Real-provider streaming legs (C3/C4/C6/C10) | UNRESOLVED | Standing manual step pending a configured provider credential; mock-provider suites pin the same code paths |

## Plan 115 manual-test-plan execution record (2026-09-08)

Module 09 gained P43–P54 (install/remove/list/update CLI round-trips against
a local fixture registry, appended load line, adopt boundary, pinned-skip,
binary provisioning approval) and module 02 gained C30–C34 (install-appended
line reload semantics + startup budget). Coverage matrix row added.

| Modules/steps | Result | Evidence/notes |
|---|---|---|
| 09 P43–P54 (CLI legs) | PASS | Live drill, scratch HOME + local registry + real npm backend; per-step details in the module 09 record |
| 09 P46/P47, 02 C30/C31 | DEFECT FOUND + FIXED | Production server never discovered store packages (`packages.not_installed` after adoption). Fixed via `PackageService::open_production` (one discovery pass at boot); lib 1284 / security 116 green, fmt + clippy clean |
| 02 C30–C34 | PASS | Reload fail-closed/recovery semantics + 33 ms vs 60 ms startup budget (module 02 record) |
| Interactive GUI legs | UNRESOLVED | Standing host ceiling (no input-synthesis backend); package contribution rendering is covered by P16–P21 / Plan 097 records — no new GUI step was added for the CLI-only verb surface |

No existing manual step was deleted or weakened. Developer profile untouched;
all scratch state removed after the drill.

## Plan 117 manual-test-plan execution record (2026-09-10)

Module 17 gained steps C24–C37 and negative checks C-N5–C-N11 covering every
user-visible plan 117 behavior: skill discovery from three roots with the
skills card, MCP stdio wiring (user + repo config, per-server fault
isolation) with the MCP card + composer connections section, the agent
settings page (delivered files + provenance + editor round-trip), @ mentions
(skills + files, keyboard-driven), the token meter (occupancy/ceiling,
threshold tones, live per-turn updates, heuristic fallback), wiki-init and
default-on graft, labeled /resume with rich restore, branch at creation,
effort active from session start, and the composed system-prompt layers in
the context inspector (SYSTEM.md + AGENTS.md + base instructions).

| Check | Result | Evidence/notes |
|---|---|---|
| Automated legs C24–C37, C-N5–C-N11 | PASS | cargo test green (lib 1307, protocol 210, security 152); clay-agent 138 (137 pass / 0 fail / 1 skip); frontend 294/294; per-step suite citations in the module 17 record |
| Live launch gate (isolated scratch config from `examples/config/`) | PASS | `test-plan/artifacts/117-coding-agent/launch-gate/`; scratch isolation verified on disk (daemon seeded + read the scratch per-agent config root, never the real home); zero wire errors after the dist+binary rebuild |
| MCP fixture + skills discovery through the real daemon | PASS | Scratch-config daemon session: `mcpServers: [{serverId: "fixture", connected: true, tools: 2}]`; workspace + home skills discovered (module 17 record) |
| Interactive GUI legs | UNRESOLVED | Standing host ceiling (no input-synthesis path); server-side halves + card rendering verified by the suites above |
| Two defects found by the launch test, both fixed | FIXED | Tauri `stamp_client_id` non-exhaustive match (new agent-settings messages broke the desktop build); daemon ignored the server configuration root (config-root leak to the real home + MCP bare commands unresolvable — fixed via `--agent-config-root` + inheriting `HOME`/`USERPROFILE`/`PATH`) |

No existing manual step was deleted or weakened; the module 17 ceiling notes
were updated to name the plan 117 seams they supersede.

## Plan 118 manual-test-plan execution record (2026-09-13, task 26)

Executed against a freshly rebuilt Linux desktop build (`cargo build --bins`
**and** `cargo build --bins -p clay-desktop`; see finding 4) with the isolated
review harness; evidence in
`test-plan/artifacts/118-quiet-instrument-migration/` (one capture per state,
each with `accessibility.txt`, window-cropped `screenshot.png`, `metadata.txt`,
`runtime-tree.txt` where the fixture publishes one, and `review.status`).

Module changes: [01](01-launch-and-connection.md) gained the landing state pair
(L12 core fallback / L12a launcher landing) plus L14a/L14b handoffs and three
negative checks; [03](03-files-and-workspace.md) F32/F32a/F33–F37/F39 were
rewritten for the landing, and F37a/F37b add the recents-hygiene negatives;
[13](13-window-splits.md) S35 returns to the landing; [14](14-tabs.md) states
which tab model ships and which is approved-but-not-built;
[15](15-ui-design-systems.md) rewrote UI-DS-02/16/17/19/26 for the shipped
system and added UI-DS-38 (launcher landing), UI-DS-39 (hairlines), UI-DS-40
(boundary/state contrast) and UI-DS-41 (removed-specifier fallback);
[17](17-coding-agent-parity.md) added C38–C40 with C-N12/C-N13 (chat absence,
landing ownership); [16](16-agent-host.md), [09](09-packages-and-modes.md) and
[11](11-performance.md) replaced chat-surface wording, and no step anywhere
loads, selects or installs a removed package or the removed design systems.

| State (fixture) | Result | Evidence |
|---|---|---|
| Launcher landing (`ui-review-launcher`, new fixture) | PASS | `launcher-landing/`: `Start` heading + paragraph, `Recent workspaces` landmark with a real recents row, `Agents` pane with its first-run note, `Open folder…`, the action footer with the disabled primary `Open`; screenshot inspected against `design-artifacts/approved/quiet-instrument-migration/start.html` |
| Core fallback (`ui-review-default`) | PASS | `core-fallback/`: `Empty tab` + `Start with a file or folder` + `Open file`/`Open folder` only — no product name, no agent button |
| Shipped design system, dark + light (`ui-review-design-system`, `…-light`) | PASS | `design-system-dark/`, `design-system-light/`: `Design system review` panel with enabled/disabled rows; `active_design_system=@clay/design-instrument` in `runtime-tree.txt` |
| Error / recovery / loading | PASS | `error/` (sanitized `JavaScript runtime evaluation failed.` in the status bar, client stays connected), `recovery/` (`Reconnect session`, `Disconnected`), `loading/` (`Loading review` panel delivered through the snapshot) |
| Large typography | PASS | `large-typography/`: landing-free fallback state at ui 24 / mono 20 / proportional 21 in bounds |
| Coding Agent view (`ui-review-coding-agent`) | UNRESOLVED live | `coding-agent/` captured the empty tab, not the agent view: the pane is activated by a package-contributed command that the same `init.js` generation cannot dispatch, and this host has no input synthesis. Fixture fixed to load the package; view evidence remains the DEV-harness set under UI-DS-35 |
| Interactive legs (landing handoffs, close-return, theme switch, palette, dialogs, splits/tabs) | UNRESOLVED — host ceiling | `computer-use-linux doctor`: no `/dev/uinput`, xdotool/ydotool or portal input path; AT-SPI *action* invocation works but the harness tears the app down before a probe can act. Automated legs pin every one of these (per-module records) |
| Automated regression on the same tree | PASS | `--lib` 1336, presentation 61, protocol 214, runtime 75, security 153; frontend 357; `tsc`; Vite build + budgets (shell 173.3/180, total 392.9/400 kB gzip); fmt/clippy; component conformance 18/18 with 0 mismatches and 5/5 state probes |

Findings recorded by this execution (details in module
[15](15-ui-design-systems.md#findings-from-this-execution-recorded-not-fixed-here)):
the harness `mkdir`/`chmod` break that made every capture impossible (fixed);
the coding-agent fixture's package-load gap (fixed); the landing's side chrome
still mounting a zero-document outline rail (design finding carried to the
visual review with a required disposition); and the `src-tauri` build-hygiene
ceiling (rebuild `clay-desktop`). No step was weakened to pass.

## Plan 119 manual-test-plan execution record (2026-09-15)

Plan 119 adds manual coverage for its user-visible changes without silently
relaxing existing steps: 03 F55 records the size-scaled large-document debug
floor; 11 Q34 now uses the same floor; 17 gains C41–C44 and C-N14 for the MCP
connection floor, per-workspace sessions, daemon restart/resume, fail-closed
root loss, and the decomposed agent panel.

| Modules/steps | Result | Evidence |
|---|---|---|
| 03 F55; 11 Q34–Q37 | PASS real-server automated; UNRESOLVED loaded-editor interaction | Fresh `cargo test --test runtime large_document::`: 2 pass in 2.04 s. It holds the 25 MiB/s throughput floor with a 500 ms minimum, 5 s full-load guard, 256 KiB chunks, exact edit/save/reload bytes, and oversize/binary refusal. The review host cannot drive the native picker into a stable WebKit editor, so no GUI first-paint claim is made. |
| 17 C41 | PASS automated | Fresh `clay-agent npm test`: 149 pass / 1 skip; slow boot connects under the host-owned 5 s floor while `timeoutMs: 200` remains a call ceiling. |
| 17 C42/C43/C-N14 | PASS real server + daemon integration | Fresh `cargo test --test security agent_session_isolation`: 2 pass. Distinct roots only receive their own writes/listings; tab closure fails closed; real daemon mock mode restores each root after daemon restart. |
| 17 C44 | PASS live rest state; PARTIAL state/a11y review | New real-app capture at `test-plan/artifacts/119-editor-agent-remediation/live-agent/` passed at 1280×1104 with AT-SPI Agent selection. Full wide/narrow fixture/keyboard state review is retained under `code-reviews/screenshots/2026-09-15-plan119-sc4-agent-review/`. Portal PNG was inspected then removed because the isolated root appeared in the status bar; root-redacted AT-SPI/drive/diagnostic evidence remains. The isolated harness logged unavailable optional `npm` package discovery, but registered 13 agent lines and had zero configuration-failure lines; this is not package-install coverage. Inspector-strip overflow (D2) and approval alertdialog focus semantics (F3) remain explicit follow-ups, not passes. |
| 17 live two-tab visual run | UNRESOLVED | The live fixture has no configured provider, so it cannot create/multiplex daemon sessions. This is not claimed as a GUI pass; the real-server Layer A/B tests above cover isolation and resume. |
| Frontend regression | ASSERTIONS PASS; suite exit 1 — existing blocker | Fresh `frontend npm test`: 49 files / 426 assertions passed; two known unhandled `invoke` rejections in `src/test/shell.test.tsx` made Vitest exit 1. Not waived or attributed to Plan 119. |

No existing test-plan step was deleted or weakened. The former flat 500 ms
large-document debug expectation is explicitly superseded by the documented
size-scaled 25 MiB/s floor because the server must finish its full resident
rope before it can send the bounded head.

## Plan 120 manual-test-plan execution record (2026-09-16)

Plan 120 (Prism 0.7.0 family pins, Node >= 22 runtime floor, unknown Prism
`AgentEvent` types dropped instead of mapping to `Started`) adds **no
user-visible chrome and no new interactive step**. It is recorded here as
automated-only. No module 16/17 step was deleted, weakened, or re-scoped, and
no GUI launch was claimed: the pin and the event-mapper default arm are not
observable in the agent surface, so there is no live step to run.

| Modules/steps | Result | Evidence |
|---|---|---|
| 16 (pin/event automated coverage) | PASS automated | Fresh `cargo test --test protocol phase25_dependencies_deny_acp_agui_mcp`: 1 pass / 215 filtered. Asserts the exact 0.7.0 pins for all seven `@arnilo/prism*` packages plus `better-sqlite3@13.0.3` and `playwright-core@1.63.0`, README `0.7.0` + `Node >= 22`, and the ACP/AG-UI/MCP dependency deny list. Fresh `cargo test --lib map_event`: 2 pass, including `map_event_drops_unknown_event_types` (`attention_compiled`, `subagent_started`/`subagent_stopped`, `delegation_*`, arbitrary unknown → dropped, never a fabricated `Started`). |
| 17 (existing agent steps) | PASS automated baseline unchanged | Fresh `clay-agent npm test`: 149 pass / 0 fail / 1 skip (the documented skip). Parsed from `npm test`'s TAP summary. No 17 step changed; this cut did not touch agent-surface behavior. |
| Live GUI | NOT RUN — not user-visible | Host ceiling unchanged (no safe window/keyboard targeting; same as the plan 119 record). Pin, Node floor, and event drop add no panel, prompt, or control. |

Verified on Node v24.19.0, which satisfies the new floor; the floor is a
private daemon startup guard, not an `init.js` option. Existing module 16
procedure text still names the historical `examples/init.js` path — the
canonical example file that the automated doc-registry gates read is
`examples/config/init.js` (a stale-reference cleanup, not a plan-120
behavior change).

## Plan 121 manual-test-plan execution record (2026-09-16)

Plan 121 adds module 17 C45–C49 for wiki ingest and graft command/help
surfaces, plus module 16 A20 for the daemon-only `session.prompt.toolNames`
seam. No existing step was deleted or weakened.

| Modules/steps | Result | Evidence |
|---|---|---|
| 16 A20; 17 C45–C49 automated legs | PASS | `cd clay-agent && npm test`: 163 pass / 1 skip; toolNames, attention, wiki-ingest, and graft suites are green. |
| Real Linux build + isolated launch gate | PASS | `cargo build --bin clay`, `cargo build -p clay-desktop --bins`, isolated mock server/desktop launch under `/tmp/clay-manual-121`, coding profile registration, scratch roots only. |
| 17 C45–C49 interactive legs | UNRESOLVED — host input blocker | AT-SPI and window discovery work, but `computer-use-linux doctor` reports `can_send_development_input=false`: `/dev/uinput` is root-only, no connectable `ydotoold` socket, `wtype` is incompatible with this compositor, and portal input consent requires a human. No GUI pass is claimed. |

### Plan 121 follow-up (auto-compaction + graft deep model, 2026-09-16)

Module 17 gains C50–C52: the automatic compaction gate (compact ratio,
two-truncated-turns signal, `compactAfterTokens` ceiling), its arming
predicate, and the explicit `graftDeepModel` option with vault-resolved keys.
No existing step was deleted or weakened.

| Modules/steps | Result | Evidence |
|---|---|---|
| 17 C50–C52 automated legs | PASS | `cd clay-agent && npm test`: 169 pass / 1 skip (170); `cargo test --test protocol`: 216/216; `cargo fmt --check` + `clippy -D warnings` clean. `auto-compaction.test.ts` and the extended `graft-knowledge.test.ts` cover each expected result. |
| 17 C50–C52 interactive legs | UNRESOLVED — same host input blocker | No new blocker; the `can_send_development_input=false` ceiling above still applies, so no GUI pass is claimed. |

## Plan 123 manual-test-plan execution record (2026-09-16)

Plan 123 (Prism 0.7 per-prompt work-scopes on OM coding runs) is an
**automated-only** cut with no new user-visible chrome: scopes are daemon-side
`om.scope.*` ledger entries, so there is no live step to run. No module 16/17
step was deleted, weakened, or re-scoped — in particular the A11 recall rule
(exact-id only, no auto-injection) and the C53–C55 spawn expectations are
unchanged, and the Memory tab keeps showing the existing OM activity without
being required to draw a scope outline.

| Modules/steps | Result | Evidence |
|---|---|---|
| 16 (work-scope record; existing A-steps unchanged) | PASS automated | Fresh `clay-agent npm test`: 180 pass / 0 fail / 1 pre-existing skip (181 total). `clay-agent/src/__tests__/om.test.ts` pins the OM-on run scope (`opened`/`entered`/`left`), the OM-off zero-entry case, invalid-id fail-closed before any provider turn, scoped per-run projection with exact-id recall still resolving sibling observations, and the delegation child scope (`parentId` = run scope, never entered). |
| 17 (existing agent steps unchanged) | PASS automated baseline unchanged | Same fresh `clay-agent npm test` run; C53–C55 spawn steps and their suites are untouched by this cut (see the Plan 123 note in module 17). Git-worktree isolation remains out of coverage per plan 122. |
| Live GUI | NOT RUN — not user-visible | Standing host ceiling (no safe window/keyboard targeting; same as the plan 119/120 records). Work-scopes add no panel, prompt, or control, and no Memory-tab scope outline is required, so no GUI pass is claimed. |

## Plan 124 manual-test-plan execution record (2026-09-17, task 15)

Plan 124 retired the window-centered command sheet, re-anchored the command and
path sessions as the composer-width `/` palette, and made the agent lane a
shell-wide chrome strip. Modules executed/amended: **10** (K64/K70/K84 and the
`Ctrl+X Ctrl+P` → `Ctrl+X Ctrl+O` references in K19/K29/K40/K47/K48/K49 amended;
new K92–K99), **13** (new S47–S49), **14** (new T79–T82), **16** (new A21–A22),
**17** (C38 amended, new C57–C64), plus drift notes in **11** (Q-series
chord/surface note + Q11 chord) and **03**/**15** (path-browser and DEV-harness
fixture notes). No existing step was deleted or weakened: the centered-era rows
keep their semantics under an explicit plan-124 supersession note.

| Modules/steps | Result | Evidence |
|---|---|---|
| 10 K92 (lane toggle) | PASS live + automated | Live: `hide lane` hint (same command as the chord) removed the lane from flow *and* tree (0 `Agent lane` nodes), hint flipped to `lane Ctrl X P`, showing it restored the lane with its draft. Chord matching itself: `shell-chords` tests; command route: `shell.toggleAgentLane` ClientUiCommand tests. |
| 10 K93/K94 (palette open, filter, scope) | PASS live (open + scope filter) / UNRESOLVED live (free-text query) + automated | Live: titlebar trigger seeded `/` and opened dialog `Commands` at the composer's width with chips `All · Session · Shell · Files` and `95 results`; the `Session` chip filtered to `14 results` (server-side filter + one snapshot). Typing a query and clicking rows is not drivable on this host (no keyboard synthesis; WebKitGTK exposes the textarea as `Text` only and rows without actions) — automated legs: `CommandPalette.test.tsx`, `Composer.test.tsx`, `WorkspacePanes.test.tsx` (scope on the wire). |
| 10 K96 (cancel/dismissal, defect D6) | PASS live + automated | Live: hiding the lane while the sheet was open removed the sheet **and** the veil (0 dialog nodes, 0 lane nodes; pane/rail brightness 1.00/0.99) and showing the lane restored the draft and scope; fixed in `frontend/src/shell/WorkspacePanes.tsx` (`fieldMenuOpen` now also requires lane visibility), pinned in `WorkspacePanes.test.tsx`. `Escape`/tab-switch legs: automated. |
| 10 K97 (sheet anchoring + veil) | PASS live + automated | Live: sheet exactly the composer's width, anchored above it, one veil over panes + inspector rail, never the lane (pane/rail ratio 0.88 each, lane 1.00; the lane band brightens to 1.33 because the sheet paints there); `workspace-composition.test.tsx` pins the grid placement and lane z=41 > veil z=40; `capture-lane-palette.mjs` covers the prototype matrix. |
| 13 S47/S48 (lane-in-workspace-view, veil in the working area) | PASS live | AT-SPI extents: `footer Agent lane` `26,946 1280x200` (full content width) and rail landmark `Document outline` `966,110 340x836` → bottom `946` == lane top; sidebar and rail both end at the lane in the captures. Veil numbers as above; the same geometry was measured live in task 9 (lane 1160 px → 1500 px after the fix). |
| 14 T79/T80 (per-tab lane state, draft/session per tab) | PASS live (state + draft) + automated (switch cancel) | Live: hiding the lane on one tab and creating a new tab showed the lane on the new tab while the first kept it hidden; hiding/showing with `/` + `Session` in the sheet restored both. Tab-switch cancellation and store adoption: `WorkspacePanes.test.tsx`. |
| 14 T81/T82 (persistence, tab/launch) | PASS automated / NOT RUN live | `laneVisible` per tab is pinned by `frontend/src/shell/tab-store.test.ts` + `frontend/src/shell/workspace-controller.test.ts` + `src/shell/layout_persist.rs` tests; the live scratch `layout.json` carries the field. A live two-tab quit/relaunch was not driven (this host's AT-SPI default action on a tab node closes the tab). |
| 16 A21/A22 (one session per tab, lane is host chrome) | PASS automated + live (no-provider truth) | Live: the attached lane reported `no provider configured · Settings · Providers` with the disabled `Configure a provider` model trigger. Store hoisting/boundary: `WorkspacePanes.test.tsx`, `AgentLane.test.tsx`, package UI conformance tests. |
| 17 C57–C64 (lane composer, controls, approval strip, streaming cues, agent-less tab, slash + mentions, negatives) | PASS live (C59 states, C62, C63 palette rows, C64 no-provider + D6) / PASS automated (C57/C58/C60/C61) | Live: agent-less lane keeps its place and stays typable; attaching `Coding Agent` from the lane picker (centered `Agent type` dialog) switched to the attached lane with the disabled model trigger and the no-provider foot; `Session` scope listed 14 daemon slash rows. Submit/stream/approval/effort/meter legs are pinned by `AgentLane.test.tsx`, `CodingAgentPanel.test.tsx`, `Composer.test.tsx`, `workspace-composition.test.tsx` (mark pulse), and the approved prototype captures (no provider is configured in the canonical config by design). |
| 11 Q11 chord, Q-series surface note | PASS (amended) | `Ctrl+X Ctrl+O` is the palette chord; `Ctrl+X Ctrl+P` now toggles the lane; the centered-feel criteria keep their semantics under the new anchoring note. |
| Launch gate (module 01 L-series) | PASS live | Isolated root, canonical `examples/config/init.js`, fresh build: Connected, canonical theme/design-system/icons, scratch workspace listed, no configuration diagnostics; status-bar hints render the shipped chords. |

Artifacts: `test-plan/artifacts/124-agent-lane/` (`drive.txt` action log,
`accessibility.txt` AT-SPI extracts, `geometry.txt` extents + veil pixels,
`server.diagnostics.txt`, and 9 window-cropped screenshots). Gates on the same
tree: `vitest run` 478 passed, `tsc -b`, `eslint`, `prettier --check`,
`npm run check:budget` (shell 172.5/180 kB gzip, total 400.0/404 kB),
`cargo fmt --check`, protocol suite 219 passed, presentation suite 61 passed.

**Ceilings recorded here (not false passes).** No host keyboard/pointer
synthesis reaches the window: the actual chord strokes, free-text queries,
`Enter`, `Escape`, `Shift+Tab` and pointer interactions were not executed live
and are covered by the suites named per step. WebKitGTK exposes palette rows as
list items with no AT-SPI actions and the composer textarea without
`EditableText`, so row activation and typing are unreachable through the
accessibility bus on this machine. The canonical config configures no provider,
so streaming, approval-strip, and token-meter states were verified from their
automated legs and the approved prototype captures rather than live. One
transient non-plan observation (a bridge `NotConnected` diagnostic after an
unusual rapid tab open/close sequence) is recorded in module 14.

## Plan 125 manual-test-plan execution record (2026-09-18, task 16)

Plan 125 consolidated every transient selection/input flow into the one
composer-anchored `/` palette (mode-aware stages, shielded credential stage,
stage-back navigation, `Alt+↵` secondary activation), retired the window-centered
sheet and its projection, replaced the palette/mentions drop shadow with the
halo, confined the agent lane to the view pane, and made new tabs adopt the
default agent. Modules executed/amended: **10** (centered-era supersession
extended; new K100–K108), **11** (Q10/Q13/Q31 re-aimed at the palette; new
Q39–Q40), **13** (S47 amended — defect D7, fixed 2026-09-18 by the plan-126 rail work; new S50 — defect D9 fixed),
**14** (new T83–T84), **16** (new A23–A24), **17** (C59/C62/C64 amended; new
C65–C68), plus notes in **03** (path mode) and **15** (UI-DS-37 measures the
palette now). No step was deleted or weakened: the centered-era rows keep their
semantics under the plan-124 + plan-125 supersession notes, and every new step
is numbered per module and referenced exactly once in the parity ledger
(`cargo test --test protocol documentation_coverage` passes).

| Modules/steps | Result | Evidence |
|---|---|---|
| 10 K100–K108 (stage flows, retired centered anchor, inert picker ids) | PASS live (K100 row presence, K106 sigil, K107 no centered nodes) / PASS automated + fixture (K101–K105, K108) | Live: the one sheet (`dialog Commands 44,531 1124x420`, `95 results`) listed `Configure Provider`, `Choose Agent/Model/Provider`, `Resume Session` with 0 centered nodes in every captured state. Stage rendering/back/shield: `agent_picker.rs` (`every_picker_stage_is_a_palette_session_with_its_mode`, `stage_back_derives_the_previous_stage_and_drops_the_secret`, `flow_entry_is_the_picker_list_alone`, `secret_is_not_in_snapshot_query_or_labels`), `menu_sessions.rs` (`picker_backspace_walks_the_flow_and_closes_at_its_entry`, `no_session_constructor_produces_the_retired_centered_origin`), `CommandPalette.test.tsx`, `Composer.test.tsx`, `WorkspacePanes.test.tsx`, fixture tool `capture-lane-palette.mjs` (291 checks). Row activation/typing stay UNRESOLVED live (host ceiling). |
| 11 Q10/Q13/Q31/Q39/Q40 (surface feel, halo, stages) | PASS live (Q10/Q13 geometry + Q39 halo numbers) / PASS automated + fixture (Q40) | Live: sheet `44,531 1124x420` with a 6px gap and a 306 px internal list box; halo profile rows 525→530 `39→42→46→48→51` on a veiled background of 38–39 and rows 951–953 `53/50/47` over the lane's 40 (symmetric, zero offset, no dark band); one veil (pane −4.7, rail −5.0, lane/composer/status bar 0.0). Automated: `plan125_palette_and_mentions_take_the_halo_not_a_drop_shadow`, `Elevation::Halo` fallback, fixture staging. |
| 13 S47/S48/S50 (lane confinement, sheet + veil, rail independence) | **S47 FAIL live — defect D7, fixed 2026-09-18** / S48 PASS live / S50 PASS live post-fix | Live (plan-125 run): the lane started at the sidebar's left edge (`Agent lane 26,946 1160x200` while `Pane 1` starts at 270) although the rails were full height (`Document outline 1186,110 340x1036`); the sheet/veil numbers above; and hiding the lane pre-fix also collapsed the rail (defect **D9**, fixed in `frontend/src/shell/layout-state.ts`, pinned by `frontend/src/shell/layout-state.test.ts`, re-verified live in both directions — `rail-independence.ax.txt`). **D7 fix (plan-126 rail work, on the user's decision):** the SDUI tree's `dimension.sidebar.default` region is host-placed in the shell's own left rail, so the lane is the pane's strip between two full-height rails — live re-run `Agent lane 270,946 916x200` (starts at the sidebar's inner edge, ends at the inspector) with the sidebar's content running through the lane's row to the status bar; deterministic 3/3 widths at 1500/1024/900 (`capture-sidebar.mjs`), 497 frontend tests. Evidence: `code-reviews/screenshots/2026-09-18-plan126-rails/review-log.md`, `test-plan/artifacts/126-rails/`. |
| 14 T83/T84 (default agent, folder-less landing) | PASS live + PASS automated | Live: fresh tabs showed `Coding Agent` and the label `workspace agent` before any pick, with the typable agent-less field; automated: `WorkspacePanes.test.tsx` (adopt once / never re-pick / landing survives), `AgentLane.test.tsx`, `tab-store.test.ts`. |
| 16 A23/A24 (palette authority, agent-less truth) | PASS automated (A23) / PASS live + automated (A24) | Authoring contract + halo + centered-removal guards (`tests/clay_js_doc_registry.rs`, `tests/package_ui_conformance.rs`, `src/shell/package_ui.rs`); live `no provider configured · Settings · Providers` with the disabled `Configure a provider` trigger and a typable field. |
| 17 C59/C62/C64 amended, C65–C68 (picker flows, adoption, typable lane) | PASS live (C59/C62/C66/C67 states, C64 D6) / PASS automated + fixture (C65/C68) | Live: adopted `Coding Agent`, disabled model trigger, typable `entry Message`, D6 (hiding the lane removes sheet + veil). Automated/fixture: the suites named per row in module 17. |
| 03/15 notes | PASS (documentation) | Module 03: the path session is `mode=path` on the one sheet. Module 15: UI-DS-37 now measures the composer palette (halo elevation, picker stages, no centered sheet) instead of the retired centered fixture drawing. |
| Launch gate (module 01 L-series) | PASS live | Isolated root, canonical `examples/config/init.js`, fresh build: Connected, canonical theme/design system, scratch workspace listed, no configuration diagnostics, palette + lane rendered as above. |

Artifacts: `test-plan/artifacts/125-palette/` — `launch.txt`, `accessibility.txt`,
`geometry-halo.txt`, `launch-drive.sh`, `launch-check.py`, `atspi.py`,
`rail-independence.ax.txt`, `04-metadata.txt`, and window-cropped captures
`screenshots/01…05` plus `04a-lane-hidden-rail-collapsed.*` (the D9 defect
capture). Gates on the same tree: `vitest run` 495 passed (492 + the 3 new
`layout-state` tests), `tsc -b`, `eslint`, `prettier --check`,
`verify-component-conformance.mjs` 18/18, `cargo fmt --check`, protocol suite
(with `documentation_coverage`) and the presentation suite.

**Defects found by this run.** **D7** (lane covered the sidebar's column — module
13 S47 — **fixed 2026-09-18**: the workspace sidebar is the shell's own left rail
now; see the S47 row above and
`code-reviews/screenshots/2026-09-18-plan126-rails/review-log.md`), **D9** (one shared visibility store: the lane
toggle also collapsed the inspector rail — found while measuring S50, fixed and
pinned in this task). **Ceilings (not false passes).** No host
keyboard/pointer synthesis (`can_send_development_input: false`), no
`EditableText` on the composer over AT-SPI, and palette rows expose no usable
action, so typing a query, arrow moves, `Enter`/`Alt+Enter`/`Escape` and row
activation are UNRESOLVED live and are carried by the named automated/fixture
legs; the canonical config configures no provider, so provider-dependent stages
(credential store, OAuth poll) are fixture-verified rather than live.

## Plan 126 manual-test-plan execution record (2026-09-19, task 6)

Plan 126 hardened the document access path: rope-backed bounded windows for
completion and language intelligence, a 256 KiB package `documents.open`/`reload`
budget, mode-activation-cache LRU eviction, and one lock scope in
`release_single_document_access`. Modules executed/amended: **03** (new F56),
**04** (new E39), **09** (new P55), **11** (new Q41). No existing step was
deleted or weakened, and every new step is referenced exactly once in the parity
ledger (`cargo test --test protocol documentation_coverage` passes).

| Modules/steps | Result | Evidence |
|---|---|---|
| 03 F56 (≥4 MiB document open) | PASS live + PASS automated | Live: 4,231,903-byte `review.rs` (808,003 words) opened in the real build through the chunked path; the editor exposed `role=entry name='Document editor'` with bounded AT-SPI text (2,759 chars of the document head) and `editable,focused,supports-autocompletion`; the status bar carried the analysis-limit note. Automated: `cargo test --test runtime large_document::` — 50 MiB open→head 256 ms, chunked edit/save/reload round trip, oversize/binary refusals, 2 passed in 2.11 s. Artifacts: `test-plan/artifacts/126-access-paths/live-large-open/` and `live-large-completion/screenshot.png`. |
| 04 E39 (completion on a ≥4 MiB document) | PASS live | Typing `fn` auto-activated completion and `Ctrl+Space` opened the popup (`rust` group, `fn` selected, `fn function snippet`) next to the caret; `ArrowDown` + `Enter` accepted the snippet (`fn name(args) {    }` inserted, 2,761 → 2,779 chars); `Escape` closed it with no text change; `zzzz` + `Ctrl+Space` returned `Empty` with no popup, no panel, no dialog. Artifacts: `test-plan/artifacts/126-access-paths/live-large-completion/`. |
| 09 P55 (package documents-op budget) | PASS live + PASS automated | Live: `serverOpenDocument` on the 4 MiB workspace file produced `clay server runtime reload failed [documents.document_too_large]: Document/workspace operation failed server validation.` with the sanitized sentence in the status bar, no path/content leakage, and the client still editing the same file. Automated: `documents_open_over_budget_returns_typed_error` (open + reload legs, no text to JS) and `documents_open_under_budget_unchanged` (golden contract). Artifacts: `test-plan/artifacts/126-access-paths/op-budget-live/`. |
| 11 Q41 (completion latency/allocation on a large document) | PASS measured + PASS live presence | Server-side measurement on a 4 MiB document: completion round trip ~161 µs median (was 427–569 µs) with 31,118 bytes allocated per request, identical to the 64 KiB case — the O(document) copy is gone (`code-reviews/2026-09-18-plan126-baseline/README.md`). Live presence/acceptance as above; the AT-SPI probe cannot resolve sub-second paint (tree walk ~0.9 s warm), so no live frame number is claimed. |
| Launch gate (module 01 L-series) | PASS live | Isolated mode-700 root, fixture-only config, private socket: `Connected`, canonical shell, scratch workspace listed, no configuration diagnostics. |

Artifacts: `test-plan/artifacts/126-access-paths/` — `launch-live.sh` (isolated
launch that stays up, with tree-kill teardown), `probe.py` (AT-SPI dump/editor
facts/focus/insert/popup-watch), `portal-shot.py` (portal screenshot cropped to
the Clay window), `op-budget-init.js`, `live-large-open/`,
`live-large-completion/`, `op-budget-live/`. Harness change:
`scripts/capture-ui-review.sh` gained the `ui-review-large-document` fixture
(≥4 MiB `review.rs`, completion init.js, restored through `layout.json`) and an
`insert` drive action (`InsertText` at the caret/end) — documented in
`docs/development/launch-and-gui-smoke.md` and
`docs/wiki/modules/ui-review-harness.md`.

**Host note (changed ceiling).** Input synthesis works on this host through the
**xdg-desktop-portal remote-desktop keyboard session** (computer-use-linux MCP
`press_key`/`type_text`), which is how E39 and P55 were driven live: typing,
`Ctrl+Space`, `ArrowDown`, `Enter`, and `Escape` all reached the Clay window and
were verified in the document text and the AT-SPI tree. `ydotool`/`wtype` remain
unusable and the WebKit editor node still exposes no `EditableText`, so the
AT-SPI `--drive` path cannot type into the editor — the earlier "no keyboard
backend at all" ceiling no longer holds for portal-driven runs.

**Ceilings recorded here (not false passes).** No live frame-timing number is
claimed for completion on a large document: the AT-SPI probe's resolution (one
tree walk) is coarser than the event. The retained harness screenshot of the
large-document run is a window crop; when the Clay window sits off-screen or
another window overlaps it, the PNG is discarded and only the a11y tree and
metadata are kept (this happened once during the run and is recorded in the
artifact directory).

## Plan 127 manual-test-plan execution record (2026-09-19, task 7)

Plan 127 changed runtime scheduling only (two lanes per domain, bounded
per-lane command queues with supersession, heap-limit restoration after
near-heap recovery, and a host-side revocation gate). Modules executed/amended:
**04** (new E40), **09** (new P56), **11** (new Q42). No existing step was
deleted or weakened, and every new step is referenced exactly once in the parity
ledger (`cargo test --test protocol documentation_coverage` passes).

| Modules/steps | Result | Evidence |
|---|---|---|
| 04 E16/E18/E19/E39 (bundled `@clay/rust`, 65 KiB `review.rs`) | PASS live | Portal-driven typing (`fn live_probe`, `let value = std.`) echoed immediately; the `.` autocomplete trigger opened the popup (`list box Completions`, group `rust`, `as` selected, `fn`, `fn function snippet`), `Enter` accepted `as`, a second trigger reopened it and `Escape` closed it with no text change. `Ctrl+Space` is consumed by this host's GNOME input-source switch (host ceiling, not a Clay defect). |
| 04 E40 + 11 Q42 (bundled `@clay/markdown` parse handler, 1,052,070-byte `notes.md`) | PASS live (typing) + PASS measured | Typing echoed immediately on a 1 MiB document with a package parse handler registered for the mode: `server.edit_ack` p50 0.33 ms / p95 0.52 ms for 19 keystrokes, parse continuing in the background. The provider-lane half (a latency-lane provider answering while a package parse handler holds the general lane) is automated-only — see the reachability ceiling below. |
| 09 P56 (third-party fixture with ungrantable capabilities) | PASS live | `clay package adopt` succeeded, `clay package enable` failed closed (`MissingCapabilityGrant { capability: CompletionProvider }`), the app started, the document opened, typing worked, and no package code ran. Only the sanitized `packages.load_failed: JavaScript runtime evaluation failed.` diagnostic surfaced. |
| Launch gate (module 01 L-series) | PASS live | Isolated mode-700 root, fixture-only config, private socket: `Connected`, workspace listed, document opened, no package contribution applied. |

**Reachability ceiling (recorded, not a false pass).** The plan asks for a live
step "completions remain responsive while a package parse handler runs". The
typing half is live above; the completion-under-a-held-lane half has no live
trigger on this build: no bundled package registers a JS completion provider
(the shipped completion items come from the built-in host-side Rust provider),
and a third-party package cannot be enabled with `parse-document`/
`completion-provider` because those capability grants are recorded by
`PackageService::authorize_package`, which has no CLI/desktop/JS surface yet
(bundled packages get `authorize_bundled_defaults`, language servers get
`authorizeLanguageServer`). The lane behaviour is therefore pinned by
`latency_lane_unblocked_by_busy_general_lane` and the rest of the plan 127
scheduling suites (4.1–24.9 ms completion latency under a 100–500 ms
general-lane hold versus the ~454 ms single-worker baseline,
`code-reviews/2026-09-18-plan127-baseline/README.md`), plus a ready-to-run
fixture pair (`@fixture/lane`, `@fixture/laneblocked`) for the day a grant
surface exists.

Artifacts: `test-plan/artifacts/127-lane-scheduling/` — `run-live.sh`
(isolated launch: `fixture|completion|markdown` modes, `CLAY_PERF_PROFILE` +
`CLAY_PERF_REPORT_DIR` wired, tree-kill teardown), `store-list.py`, `probe.py`
and `portal-shot.py` (AT-SPI dump/editor/focus/completion probe; portal
screenshot cropped to the Clay window), `init-fixture.js`, `init-markdown.js`,
the fixture packages (`fixture-package/` module-backed provider,
`fixture-package-inline/` inline-provider A/B), `grant-gate-live/`,
`live-completion/`, `live-markdown-parse-handler/`, and the README that records
how the store was seeded (`pnpm add <path>` in `~/.clay/packages`) and why.

**Host note (unchanged ceilings).** Input synthesis works through the
xdg-desktop-portal remote-desktop keyboard session
(`computer-use-linux` `type_text`/`press_key`) once the Clay window is
activated; `Ctrl+Space` is consumed by GNOME input-source switching, so the
fixtures also bind `Ctrl+J`. The AT-SPI probe still cannot resolve sub-second
paint, so no live keypress→paint number is claimed for completion.

## Module map

| # | Module file | Covers | Deep-reference doc |
|---|-------------|--------|-------------------|
| 01 | [Launch and connection](01-launch-and-connection.md) | server/client lifecycle, lease, read-only observer, restart, status line, and the empty-tab landing (plan 118: the bundled `@clay/launcher` panel when its package is loaded — `Start`, Workspaces/Agents panes, recents, handoffs — versus the Clay-owned `Start with a file or folder` fallback when no empty-tab contribution is installed; L12/L12a/L14a/L14b + negative checks) | `docs/development/launch-and-gui-smoke.md` |
| 02 | [Configuration (init.js)](02-configuration-init-js.md) | init.js evaluation, modular loading, diagnostics, live reload, watcher auto-reload, default reload chord, planned-API denial, install-appended load line (C30–C34) | `docs/reference/clay-js-api/configuration.md`, `examples/` tree, `tests/fixtures/configuration/plan080-manual/` |
| 03 | [Files and workspace](03-files-and-workspace.md) | open/save/reload, dirty state, conflicts, sanitized file-browser/workspace labels, hidden-pane toggle, `Ctrl+O` while hidden, multi-document (incl. pane-scoped switcher, duplicate-open focus routing), Path Browser (24.3): seed fallback, fuzzy filter, descend/ascend/direct jump, invalid-path recovery, file open + duplicate-open focus + active-pane targeting, `Alt+Enter` current-tab workspace load, cancellation, tab-switch/reload dismissal, native-dialog fallback, navigation-no-grant/symlink/cross-tab security checks, centered-era modal surface/accessibility/containment (24.4 — plan 124 re-anchored the path session to the composer-width sheet; the module carries the supersession note); plan 125 makes it a mode of the one palette session (`mode=path`, the `/` sigil only in catalogue/path modes, `Esc` cancels) | `docs/development/file-open-save-reload-workflow.md`, `docs/reference/clay-js-api/configuration.md` (Phase 24.3 review) |; plan 126 adds F56 (≥4 MiB review fixture: bounded accessible text, chunked open, analysis-limit status note) |
| 04 | [Core editing](04-core-editing.md) | typing, undo/redo, clipboard, newline/indent rules, IME preedit, completion projection/ranking, Phase 28 comment/list/heading transforms and inlay toggle, bounded AT-SPI/AccessKit editable-text semantics | `docs/reference/clay-js-api/editor/` command docs, `docs/development/accessibility.md` |; plan 126 adds E39 (completion trigger/accept/dismiss on a ≥4 MiB document); plan 127 adds E40 (typing/completion responsiveness while a bundled package parse handler works a ≥1 MiB document) |
| 05 | [Movement and selection](05-movement-and-selection.md) | word/paragraph/line movement, sticky column, line/word selection, prose vs code | `docs/development/manual-editor-capabilities-test-plan.md` |
| 06 | [Multi-cursor editing](06-multi-cursor.md) | Ctrl+D match selection, column select, add-cursor, cursor undo, escape priority | `docs/development/manual-editor-capabilities-test-plan.md` |
| 07 | [Caret and typography](07-caret-and-typography.md) | caret shape/blink, width, ligature policies per font role, user-owned hierarchy, large/small UI typography and theme contrast | `docs/development/manual-editor-capabilities-test-plan.md` |
| 08 | [Syntax and text objects](08-syntax-and-textobjects.md) | grammar highlighting, textobject/smart-select, engine tiers, advisory degrade, Phase 28 folding ranges, link intent, and inlay overlays; plan 128 adds S35 (typing and language-route behaviour on a large open document, including the fail-closed size-cap and stopped-analyzer states) | `docs/development/manual-editor-capabilities-test-plan.md`, `docs/reference/primitives/ui-chrome-primitives.md` |
| 09 | [Packages and modes](09-packages-and-modes.md) | package loading, mode classification/activation, settings UI, theme switching, clipped/scrollable package panels, state/disabled/provenance semantics, Phase 27 inspect/preset/one-line load, Phase 28 behavior/keymap/LSP contributions, package install/remove/list/update CLI + adopt boundary + binary provisioning (P43–P54) | `docs/development/launch-and-gui-smoke.md`, `docs/reference/packages/creating-packages.md`, `docs/development/distribution.md` |; plan 126 adds P55 (package `documents.open`/`reload` over the 256 KiB op budget returns the typed `documents.document_too_large` diagnostic without gating the client path); plan 127 adds P56 (an adopted third-party package whose declared capability has no grant surface never executes: `clay package enable` fails closed with `MissingCapabilityGrant` and only a sanitized diagnostic surfaces); plan 136 flips P56 to the positive grant → enable → load path while keeping the no-grant negative sub-step, and adds P57 (the `clay package authorize`/`revoke` lifecycle: auditable grant set, replacement semantics, undeclared-capability refusal, fail-closed revocation) and P58 (the config `authorize` call: durable, process-independent grant with no self-grant reachability) |
| 10 | [Keybindings and commands](10-keybindings-and-commands.md) | bindKey override, unbind, deny-by-default, execution push channel, Global-scope tab command bindings (22.4), Control Center menu round trip + tab-switch dismissal (24.1), Control Center command execution mode (24.2), Path Browser keybinding surface (24.3), centered modal surface/accessibility/input containment (24.4), sequence chords (24.5), Phase 28 client-command aliases/package keymaps, and the plan 124 lane toggle + `/` palette session (K92–K99; the centered-era K54–K59/K70/K75 rows carry a supersession note); plan 125 adds the palette stage flows and the centered-sheet retirement (K100–K108: picker rows in the one sheet, `Esc`/`Alt+←` stage back, `Alt+↵` secondary activation, the shielded credential stage, draft hygiene across modes, the retired centered anchor, inert picker command ids) | `docs/development/manual-editor-capabilities-test-plan.md`, `docs/reference/primitives/shell-layout-strategy.md`, `docs/reference/clay-js-api/keybindings/bind-key.md` |
| 11 | [Performance](11-performance.md) | large files, scroll/type latency, parse feel, window-model budgets (22.6: pane paint / tab switch / decoration aggregate), centered Command Centre rendering feel (24.4: one panel + scrim, width clamping, no duplicate overlays, no blur jank), Command Centre open/filter feel + chord pending feel (24.5 advisory budgets), completion popup feel/caps (Plan 087), Plan 088 responsive/high-DPI/typography geometry, Phase 28 fold/link/inlay/ranking budgets, and the plan 124 palette anchoring note (Q11 chord = `Ctrl+X Ctrl+O`); plan 125 amends the centered-era feel rows to the shipped palette surface and adds the halo profile + stage-transition checks (Q39–Q40) | `docs/development/performance.md` |; plan 126 adds Q41 (completion on a ≥4 MiB document: presence/accept live, sub-millisecond server round trip, no O(document) allocation); plan 127 adds Q42 (perceived typing latency with packages active: measured edit-ack p50/p95 on a ≥1 MiB markdown document with a package parse handler registered); plan 128 adds Q43 (typing/language-route responsiveness on a ≥1 MiB document after the shared position index became incremental: adapter cost flat in size and measured, live typing leg blocked by host input consent); plan 136 adds Q44 (a granted third-party package's module-backed completion provider served from the latency lane while its parse handler holds the general lane busy, with the `server.edit_ack` envelope and the new `js_runtime.lane.*` occupancy counters) |
| 12 | [Platform: Windows](12-platform-windows.md) | MSVC toolchain, named pipes, native dialogs | `docs/development/windows.md` |
| 13 | [Window splits](13-window-splits.md) | split/close/add-equal/move/resize panes, pane focus policies, per-pane document views + concurrent modes (22.2), Phase 22.8 per-tab multi-document isolation, shell keybinding overrides (per active tab since 22.3), direction-named split aliases (22.7), per-tab persistence cross-check (22.5), pane a11y roles + split/pane announcements (22.6), and the plan 124 lane-chrome geometry + palette-veil steps (S47–S49); plan 125 amends S47 to the confined lane (defect D7, fixed 2026-09-18 by the plan-126 rail work: the workspace sidebar is the shell's own full-height left rail and the lane is the pane's strip), adds S50 for rail/lane independence (defect D9 fixed) and records the palette confinement numbers | `docs/reference/primitives/shell-layout-strategy.md`, `docs/development/accessibility.md` |
| 14 | [Tabs (independent client views)](14-tabs.md) | tab bar, selected-root tab binding and per-tab workspace/document isolation (22.8), open/switch/close tabs, per-tab connections + split trees + documents, edit isolation, dirty-guarded close, keyboard tab management incl. numbered activate/move + confirm close (22.4), reconnect + restart reclaim, window-state persistence incl. restore/failure/hostile-file steps (22.5), tab a11y (TabList/Tab roles, activate/create/close announcements) + cross-tab grant isolation/denial checks (22.6/22.8), tab-bar overflow scroll (22.7), active-typography geometry and sanitized tab labels (Plan 088), single-tab match-today, and the plan 124 per-tab lane state steps (T79–T82); plan 125 adds the default-agent adoption and typable-lane steps (T83–T84) | `docs/reference/primitives/shell-layout-strategy.md`, `docs/wiki/modules/react-tabs-and-splits.md`, `docs/wiki/modules/tabs-and-clients.md`, `docs/development/accessibility.md` |
| 15 | [UI design systems](15-ui-design-systems.md) | built-in fallback startup, `@clay/core` baseline versus the shipped `@clay/design-instrument` default (the former Neobrutal/Glass packages were removed by plan 118 task 9), watcher reload switching, Settings-panel + command-surface design-system selection with server-enumerated theme/DS choices and appearance persistence (plan 110), invalid/removed selection recovery with sanitized diagnostics and previous-generation retention, no-adoption security checks, color-authority conformance, the composited content-theme contrast gate (UI-DS-31), catalog currency (UI-DS-32), component-level conformance against the approved specimen (UI-DS-33), shell/Workspace composition adoption (UI-DS-34), Coding Agent composition adoption (UI-DS-35), Settings panel and Agent Settings composition adoption (UI-DS-36), command centre and overlay-family composition adoption (UI-DS-37), the launcher landing composition (UI-DS-38), hairline zoning (UI-DS-39), composited boundary/state contrast (UI-DS-40), the removed-specifier fallback and choice set (UI-DS-41), the tab's two views (UI-DS-42), agent types and the per-tab picker (UI-DS-43), the agent Files tab as the session's file history (UI-DS-44), the workspace sidebar's filter head (UI-DS-45), the 2026-09-13 visual and accessibility review of the migrated app (42 captured states, deviation dispositions, defect log, live AT-SPI walk and hairline contrast probe), restart persistence through `init.js`, full recipe migration, DOM/state continuity, forced-colors/reduced-motion/transparency accessibility fallbacks, cross-theme recoloring consistency, and Quiet Instrument conformance (hairline zoning, radius ladder, transient-only elevation, mono-for-data, measure, state/keyboard/typography checks) (Plans 102, 103, 104 & 118; `DESIGN.md`); plan 125 retires the centered overlay family and makes UI-DS-37 measure the composer palette (halo elevation, picker stages, no centered sheet) | `docs/reference/clay-js-api/theme/set-design-system.md`, `docs/reference/clay-js-api/settings/set-design-system.md`, `docs/reference/ui-design-systems.md`, `docs/development/ui-design-system-conformance.md`, `DESIGN.md`, `.impeccable/review/plan-104/` |
| 16 | [Agent host (clay-agent)](16-agent-host.md) | (agent surface, no chat) `clay:agent` facade configuration (autonomy default-off 2157, compaction strategies + OM `compactAfterTokens` 2158, workspace-scoped search metadata-only, session tree/checkout/fork/clone/checkpoint), init.js section 12 documentation cross-check, no-credential/no-hidden-key checks, MCP allow-list fail-closed validation, Obscura hidden-when-missing; the removed Chat surface is named nowhere as shipped (plan 118); coding-tool dirty-buffer/approval/durable-run behavior pinned by automated suites, and the plan 124 one-session-per-tab / lane-is-host-chrome steps (A21–A22); plan 125 adds the palette authority and agent-less-truth steps (A23–A24) | `clay-agent/README.md`, `docs/wiki/modules/clay-agent.md`, `docs/reference/clay-js-api/agent/`, `examples/init.js` (section 12) |
| 17 | [Coding agent pi-parity (@clay/coding-agent)](17-coding-agent-parity.md) | Phase 2 pi-parity conformance: prompt→stream→tool ordering, steering, cancel, /compact manual+auto, /new, session list/resume/delete, provider/model switch, /tree+/fork+/clone, session-picker/open-as-fork equivalents, plan-file round-trip, composer growth, Shift+Tab effort cycle, status-row truth, extension strip; negative checks (cross-workspace search invisibility, disabled knowledge bases, secrets, unknown slash command, search-hit context) and stream-latency/UI-responsiveness budgets (plan 108 task 15); plan 109 C1–C20 + C-N1–N4: per-tab workspace binding + per-workspace model auto-load, /model + dropdown, effort control + rebinding, full chronological transcript (tools/skills/thinking/steer), Files-tab editor view + Ctrl+B tree toggle, context inspector drawer + compaction reflection, OM activity + worker-model retention, /resume restore, Session Info auto-select, real git branch + truthful extension strip + daemon-sourced slash completion, and negative checks (cross-workspace session leakage, unconfigured-provider filtering, redaction, fail-closed effort); plan 117 C24–C37 + C-N5–C-N11 (plus plan 118 C38–C40 + C-N12/C-N13: the landing→agent-view launch route, the Settings tab as the Agent Settings surface, the Files tab session history, chat-surface absence, landing ownership): skills card from three discovery roots + skills.json gating, MCP card + composer connections (user/repo config, per-server isolation), agent settings page with provenance, @ mentions, token meter with threshold tones + heuristic fallback, /wiki-init flow, graft default-on, labeled /resume with rich restore, branch at creation, effort from session start, and composed system-prompt layers (SYSTEM.md/AGENTS.md/base) in the context inspector; plan 121 C45–C49: wiki-ingest text/path + missing-Obscura failure, graft skill/help, non-interactive init, and deep-build fail-closed behavior; plan 122 C53–C55 + C-N15: coding sessions list spawn/wait/cancel supervisor tools (catalog exactly `test`/`validation`), sync spawn renders child lifecycle rows via the `subagent_*` → Tool mapping, parent abort stops async children, unknown childId / foreign delegationId fail closed, and the plan 124 lane-mounted composer/controls/approval/streaming steps (C57–C64; C38 amended); plan 125 adds the picker flows through the palette, default-agent adoption, and the agent-less typable lane (C59/C62/C64 amended, C65–C68) | `packages/coding-agent/docs/parity-checklist.md`, `packages/coding-agent/docs/index.md`, `docs/wiki/modules/clay-agent.md`, plan 108, plans/117, plans/121, plans/122 |
| 18 | [Icon packs (Plan 112)](18-icon-packs.md) | zero-config bundled Regular fallback, `setIconPack` Regular/Duotone selection (load ≠ select), watcher swap/fail-closed break + restore recovery, unloaded/unknown selection bounded diagnostics, third-party own-prefix + hostile-fixture rejection (live adoption blocked: no pnpm), file-browser/git/markdown semantic icons, icon-only control contract (names, tooltips, hit targets, retained text labels), AT-SPI a11y pass, no-network inlined-geometry rendering, responsive/large-typography scaling and pack-swap feel | `docs/reference/clay-js-api/theme/set-icon-pack.md`, `docs/reference/icon-packs.md`, `test-plan/artifacts/112-icons/` |

## Coverage matrix (what to run when)

| Change touches | Minimum manual modules |
|----------------|------------------------|
| Client rendering / surface / paint / transient overlays | 01, 04, 07, 10, 11 |
| UI design systems / component recipes / visual migration | 15, 02, 07, 09, 11 |
| Editor movement/selection/caret primitives | 05, 06, 07, 10 |
| Typography / font features | 07 |
| Protocol / IPC / connection | 01, 03, 04 |
| Configuration surface / init.js APIs | 02, 10 + the module of the feature configured |
| Icon packs / `setIconPack` / icon-only controls / semantic icon references | 18, 02 (selection persistence), 09 (package trust), 15 (theme/DS independence) |
| Syntax / grammar / decorations | 08 |
| Package loading / modes / trust boundary | 09, 02 |
| Package install/remove/list/update CLI (`clay install` family, adopt boundary, appended load line, binary provisioning) | 09 (P43–P54), 02 (C30–C34), 01 (launch gate) |
| File IO / save / dialogs | 03 |
| Keybinding routing / commands | 10, 05, 11 |
| Shell layout / panes / splits / pane focus / split aliases | 13, 10, 01 |
| Tabs / selected-root workspace binding / cross-tab authority / tab bar / keyboard tab chords / multi-connection / reconnect / window-state persistence / tab-bar overflow scroll | 14, 13, 03, 01 |
| Pane/tab/transient-menu accessibility (roles, names, announcements) / cross-tab isolation | 10, 13, 14, 03 |
| Pane document views / concurrent modes / duplicate-open routing | 13, 03, 09 |
| Anything user-visible | 01 always (launch gate) |
| Plan 118 Quiet Instrument migration & launcher landing (design-system/theme values, component recipes, shell/workspace/agent/settings/palette/overlay compositions, empty-tab landing) | 01 (L12/L12a/L14a/L14b), 15 (UI-DS-31…45), 03 (F32/F32a/F37a/F37b), 13 (S35), 14 (tab-model note), 17 (C38–C40, C-N12/C-N13), 02 (C16–C18 watcher reload), 11 (switch feel) |
| Welcome entry state / completion projection / centered Command Centre / review harness (Plan 087) | 01 (L12–L14), 03 (F32–F37), 04 (E16–E21), 10 (K69–K72), 11 (Q11–Q14), 13 (S33–S35) |
| Plan 088 shell/theme/package modernization and responsive layout | 01 (L15–L19), 02 (C20–C24), 03 (F38–F41), 04 (E22–E24), 07 (T14–T17), 09 (P16–P21), 10 (K73–K77), 11 (Q15–Q19), 13 (S36–S40), 14 (T71–T76) |
| Plan 089 validation, performance, timeout diagnostics, and multi-window/scale/Wayland platform checks | 01 (L18–L22), 04 (E22–E24), 07 (T15/T18–T19), 09 (P16–P21), 10 (K73–K77), 11 (Q15–Q19), 13 (S36–S42), 14 (T71–T76) |
| Phase 26 rendering quality (theme color/background/scale axes, heading size ladder, wrap policies, editor chrome, decoration backgrounds, dirty-pane close fix) | 04 (E25–E27), 07 (T20–T27), 08 (S16–S19), 09 (P22–P24), 11 (Q20–Q23), 13 (S43–S46) |
| Phase 28 editor commands/intelligence (comment/list/heading transforms, package keymaps, folding, links, inlays, completion ranking, editable-text accessibility) | 01 launch gate, 04 (E28–E36), 08 (S21–S32), 09 (P29–P31), 10 (K78–K83), 11 (Q24–Q27) |
| Plan 097 Phase 8 React SDUI/package UI and trust domains | 01 launch gate, 09 (P32–P36), 11 (Q28–Q30) |
| Plan 097 Phase 9 React Command Centre, paths, configuration, settings, and desktop workflows | 01 launch gate, 02 (C26–C29), 03 (F42–F47), 09 (P37–P42), 10 (K85–K91), 11 (Q31–Q33) |
| Plan 097 Phase 5 CodeMirror editing + optimistic document sync | 04 (E-series), 03 (open/save/reload), 11 (type latency) |
| Plan 097 Phase 6 panes/splits/tabs/per-tab workspaces/persistence | 13, 14, 03 (workspace roots), 01 (reconnect) |
| Plan 097 Phase 7 editor interaction/rendering/completions/language intelligence | 04, 05, 06, 07, 08, 11 |
| Plan 097 Phase 10 AG-UI chat over Tauri channels (historical: the `@clay/chat` surface was removed by plan 118; the same transport lane carries the Coding Agent) | 09 (package lane), 11 (agent-transcript stream feel), 17 (agent view) |
| Plan 097 Phase 11 release hardening/packaging/updates/security | 01 (launch identity), 11 (budgets), 12 (platform policy) |
| Plan 097 Phase 12 parity certification/cutover/native removal | 01, 13, 14 + full regression pass of modules above |
| Plan 102 UI design systems (selection, switching, fallback/revocation recovery) | 15, 02 (C16–C18 watcher reload), 09 (adoption/revocation), 11 (switch latency) |
| Plan 098 chunked document loading | 01 (L23–L24), 03 (F48–F52), 11 (Q34–Q37) |
| Plan 099 server-authoritative editor performance and manual matrix | 01 (L25–L26), 03 (F53–F54), 04 (E37–E38), 08 (S33–S34), 11 (M1–M7), 13 (D20–D21), 14 (T77–T78); deep reference: `docs/development/performance.md` |
| Agent host configuration surfaces (`clay:agent` facades, autonomy/compaction defaults, init.js agent section, MCP/Obscura fail-closed wiring) | 16 (incl. A25: resume binds the recorded workspace root and re-activates its capabilities/graft binding), 02 (C24 raw-op denial still applies) |
| Plan 109 coding-agent defects/UX + Prism 0.5.0 (I2–I10, R1–R3) | 17 (C1–C20, C-N1–N4), 16 (host config), 10 (effort + file-browser bindings), 14 (per-tab workspace binding), 01 (launch gate) |
| Plan 113 Prism 0.5.1 kernel request construction (host stopgap deleted, `RunOptions.thinkingLevel`) | 17 (C21–C23), 16 (0.5.1 pins via agent_protocol pin asserts) |
| Plan 119 editor/agent remediation (large-document throughput, MCP connect floor, workspace-scoped sessions, Coding Agent decomposition/review) | 03 (F55), 11 (Q34–Q37), 17 (C41–C44, C-N14); existing two-tab shell coverage remains in 14 |
| Plan 120 Prism 0.7.0 pins + Node >= 22 floor + unknown `AgentEvent` drop (automated-only, no new chrome) | 16 (pin/event record; existing A-steps unchanged), 17 (existing agent steps apply unchanged) |
| Plan 121 Prism 0.7 toolNames, attention, wiki ingest, and graft command surfaces | 16 (A20 daemon RPC note), 17 (C45–C49), 01 (isolated Linux launch gate) |
| Plan 121 follow-up auto-compaction + graft deep model | 17 (C50–C52) |
| Plan 123 Prism 0.7 per-prompt work-scopes on OM coding runs (automated-only, no new chrome) | 16 (work-scope record; existing A-steps unchanged), 17 (Plan 123 note; existing agent steps unchanged) |
| Plan 124 persistent agent lane + composer palette (shell chrome, `/` palette, retired centered command sheet) | 10 (K92–K99; K64/K70/K84 + `Ctrl+X Ctrl+O` chord references amended), 13 (S47–S49), 14 (T79–T82), 16 (A21–A22), 17 (C38 amended, C57–C64), 03 (path-browser note), 11 (Q-series note + Q11 chord), 15 (UI-DS-37 fixture note), 01 launch gate |
| Plan 130 agent-host decomposition + resume binding (injected lane authority, `clay-agent/src/host/*`, resumed sessions re-activate capabilities/graft) | 16 (A25 new, A1–A24 regression), 17 (C43 extended, C41–C44) |
| Plan 126 document access-path hardening (rope-window completion/language intelligence, documents-op budget, mode-cache LRU, single-lock release) | 03 (F56), 04 (E39), 09 (P55), 11 (Q41); automated companions: lib `connection`/`workspace`/`js_runtime` tests, `cargo test --test runtime large_document::`, `performance_budgets` pins |
| Plan 127 JS runtime lane scheduling, bounded command queues, heap-limit restore, revocation gate | 04 (E40), 09 (P56), 11 (Q42); automated companions: `latency_lane_unblocked_by_busy_general_lane`, `lane_poison_replaces_only_that_lane`, `third_party_lane_denies_trusted_ops`, `lanes_share_their_domain_op_set`, `queue_bounded_under_flood`, `queue_evicts_oldest_at_capacity`, `near_heap_limit_recovers_with_original_cap`, `revoked_package_commands_refused_per_lane`, `reload_shares_third_party_lanes_untouched`; baselines in `code-reviews/2026-09-18-plan127-baseline/` |
| Plan 136 third-party capability grants + provider lane observability (grant API/CLI/config surface, durable provenance-bound grants, lane occupancy counters, option-surface guard) | 09 (P56 flipped to the positive path + P57 CLI verb + P58 config call), 11 (Q44 latency lane + occupancy counters), 02 (C-section: the `authorize` config call), 01 launch gate; automated companions: `granted_third_party_provider_serves_from_latency_lane_when_general_lane_busy`, `ungranted_third_party_provider_still_fails_closed`, `durable_grant_survives_a_new_service_process`, `store_without_grants_still_fails_closed`, `grant_is_inert_after_provenance_change`, `revoke_withdraws_the_durable_grant`, `package_code_cannot_self_grant_capabilities_during_activation`, `lane_command_counters_track_general_and_latency_separately`, `declared_option_keys_are_documented_for_every_public_api`; evidence in `test-plan/artifacts/136-capability-grants/` |

## Plan 097 Phase 9 Linux execution record (2026-08-23)

React fixture review covered active/empty/narrow Command Centre, labelled
search/listbox/options/live count, settings collapsed/expanded/narrow/invalid
states, and token-only containment. Evidence:
`code-reviews/screenshots/2026-08-23-tauri-react-phase9/`. C26–C29,
F42–F47, P37–P42, and K85–K91 record exact automated/live boundaries.
`computer-use-linux_get_app_state` ran first; AT-SPI worked but development
keyboard input was unavailable, so CDP supplied DOM interaction/accessibility
evidence and native picker selection remains explicitly blocked. No existing
step was removed or weakened.

## Plan 097 Phase 8 Linux execution record (2026-08-23)

React fixture review covered package slots, SDUI editor composition, settings controls, status contribution, dropdown interaction, narrow/wide layout, and large typography. Evidence: `code-reviews/screenshots/2026-08-23-tauri-react-phase8/`. P32–P36 and Q28–Q30 record exact automated/live boundaries. CDP provided the rendered accessibility tree because AT-SPI exposed only the Chrome frame and compositor targeting omitted Chrome-for-Testing. No existing step was removed or weakened.

## Plan 088 task 12 Linux execution record (2026-08-15)

Executed against the current `cargo build` on real Linux/GNOME Wayland. Mandatory UI preflight for this task was the UI guidance current at execution time; selected review skills were `wshobson/wcag-audit-patterns` (accessibility/testing) and `vercel-labs/web-design-guidelines` (visual/accessibility), applied to Clay's token/AT-SPI context. `computer-use-linux get_app_state` and `doctor` ran before launch; AT-SPI/screenshot capture works, but `can_query_windows=false` and `can_focus_windows=false`, so targeted keyboard, resize, and native-dialog actions are not claimed as passes.

| Modules/steps | Result | Evidence |
|---|---|---|
| 01 L15–L17, 20; 03 F39/F41 | PASS | Current Clay-only review artifact `code-reviews/screenshots/2026-08-15-plan088-task12-manual/default/` has `review.status PASS`, 900×600 logical metadata, named welcome actions/status, and no absolute path; retained error/light/large captures add dark/light/error/typography coverage |
| 01 L18, L19; 04 E23; 10 K74/K75; 13 S39; 14 T75 | FAIL/UNRESOLVED follow-ups | Recovery capture has stale WelcomeWidget Connected status; loading capture renders welcome instead of intended loading tree; interactive completion/Command Centre/split/tab states cannot be driven safely. Findings remain explicit in module records |
| 02 C20–C24; 07 T14/T17 | PASS automated/headless; manual reload partial | Canonical example test, Node checks, typography validation, and atomic rejection tests pass; live reload input is host-blocked |
| 03 F14/F38/F40; 09 P16–P21; 10 K76/K77 | PASS structural / blocked live | Sanitization, clipping, package/theme contract, typed-token validation, modal Escape, disabled/status, stale-session, and authority tests pass; settings/native/package interaction cannot be focused |
| 11 Q15–Q19; 13 S36–S38; 14 T71–T74/T76 | PASS advisory/structural; blocked visual extremes | Window benchmarks completed; responsive/high-DPI/tab-overflow/layout tests pass; live window resize and multi-tab targeting unavailable |

The full step additions and per-module evidence are recorded in modules [01](01-launch-and-connection.md), [02](02-configuration-init-js.md), [03](03-files-and-workspace.md), [04](04-core-editing.md), [07](07-caret-and-typography.md), [09](09-packages-and-modes.md), [10](10-keybindings-and-commands.md), [11](11-performance.md), [13](13-window-splits.md), and [14](14-tabs.md). No existing step was deleted or weakened. `P1-087-UI-1`, the recovery WelcomeWidget status mismatch, and loading-fixture observability remain explicit follow-ups.

## Plan 089 task 9 Linux execution record (2026-08-17)

Real Linux/GNOME Wayland execution used `cargo build`, the isolated mode-700 review harness, xdg-desktop-portal PNG capture, Python GI/AT-SPI dumps, and the now-active GNOME Shell extension for window targeting (`can_query_windows=true`, `can_focus_windows=true`).

| Modules/steps | Result | Evidence |
|---|---|---|
| 01 L18; 14 T75 | PASS | `code-reviews/screenshots/2026-08-14-plan089-platform-validation/visual-review/recovery/` shows `Connection lost` / `Connection: Disconnected` consistently after the `request_welcome_render` fix; the Plan 088 P1 stale WelcomeWidget Connected status is resolved |
| 01 L19 | PASS (delivered-RuntimeStateSnapshot evidence) | `loading/` with `runtime-tree.txt` confirms the published loading SDUI tree was delivered via `RuntimeStateSnapshot`; the restore-gate fix and kind-changed reconcile fix ensure the tree reaches the accessibility layer |
| 01 L20–L22; 07 T18–T19; 13 S41–S42 | PASS live/headless | `CLAY_LIVE_WINDOW_SMOKE=1` multi-window smoke test launched two real Clay clients; AT-SPI exposed two PID-separated frames with positive bounds and scale factors within 0.5–4.0; `rescale_event_recomputes_logical_bounds_from_physical_size` passes; responsive narrow/wide captures show the welcome card and status bar within bounds |
| 04 E22; 10 K74 | PASS live | `completion/` capture shows the bounded completion popup with 44 children, `as` selected, no rows exceeding the visible surface; P1-087-UI-1 containment is visually verified |
| 10 K75 | UNRESOLVED live / PASS structural | Command Centre remains UNRESOLVED because `Ctrl+Alt+P` is consumed by GNOME before reaching Clay; structural clipping/single-scrim/modal-role tests pass |
| 10 K77 | PASS automated | Plan 089 added `compact_generated_frame_mutations_fail_closed_without_panicking`, `editor_generated_chord_sequences_preserve_prefix_mismatch_and_timeout_transitions`, and `generated_menu_intent_ordering_preserves_lifecycle_and_authority` |
| 11 Q15 | PASS advisory run + triage | Plan 089 Criterion triage classified every group as machine variance except centered_overlay as benchmark instability; no reproducible implementation regression; no budget raised |
| 09 P16–P21 | PASS structural / NOT RUN package-panel visually | Plan 089 did not add new package features; settings/package panels remain unrendered because `settings.open` does not persist or make the panel visible |

## Plan 090 task 11 Linux execution record (2026-08-17)

Plan 090 is a responsibility-preserving refactor: existing server/runtime,
editor/shell, package, and app-driver code moved into private modules without
changing user-visible behavior, layout, labels, commands, keybindings, or
platform contracts. No new numbered steps or coverage-matrix entries were
needed; existing module records remain the behavioral baseline. Manual parity
was checked against the current Linux debug build, with direct interaction
steps not duplicated where the plan made no user-facing change. The approved
Plan 090 visual-review waiver remains separate and unchanged.

| Modules/steps | Result | Evidence |
|---|---|---|
| 01 L12–L19; 02 C20–C24 | PASS current-build parity | `cargo build`; `node --check` on all three example files; canonical configuration fixture/test; `scripts/capture-ui-review.sh` default/loading/error/recovery/large-typography fixtures all returned `PASS`. Current AT-SPI trees expose the Clay shell, welcome actions, Connected/Editable status, delivered loading snapshot, runtime error, and Disconnected recovery state. |
| 03 F14/F38–F41; 04 E22–E24; 09 P16–P21; 10 K73–K77; 13 S36–S42; 14 T71–T76 | N/A for new manual rerun; parity retained | No user-facing behavior changed in these modules, so no new steps were added or weakened. Existing Plan 089 Linux records remain the latest direct interaction evidence; current `cargo test --all-targets --quiet` passed all editor/protocol/runtime/security suites and all benchmark harness cases. |
| 11 Q15–Q19 | PASS advisory / no blocking budget failure | Current `window_baselines` and `editor_baselines` Criterion runs completed. Small-sample comparisons reported host/baseline variance, while absolute measurements stayed within the documented budgets; per module 11, Criterion comparisons are advisory and not a shared-runner pass/fail gate. |
| Security negative checks | PASS automated; live probe blocked by host | Current all-target test run: security 130 passed, 2 ignored; package/runtime/file/workspace/modal/visibility denial checks remained green. The standalone live AT-SPI smoke probe timed out without discovering Clay on this host, while the capture harness found Clay and produced valid accessibility trees; no source failure is inferred, and the prior Plan 089 live pass remains retained. |

No `test-plan` module instructions were changed, deleted, or weakened. The
coverage matrix is unchanged. Temporary current-run harness output was kept
under `/tmp/plan090-manual/`; no new screenshot artifact was retained because
Plan 090's visual review is N/A by approved scope exception.

## Plan 087 task 11 Linux execution record (2026-08-15)

Real Linux (X11-backend client on the review host) execution used the isolated mode-700 root + `ui-review-completion` fixture init.js with an empty-tab launch. The welcome entry state, the native Open File dialog flow, and the live completion popup were driven through AT-SPI/portal and verified in the accessible tree; client and server stayed alive throughout.

- **PASS:** welcome entry state (module [01](01-launch-and-connection.md#linux-execution-record-plan-087-task-11-2026-08-15)); Open File → native dialog → `review.md` opened with sanitized basenames (module [03](03-files-and-workspace.md#linux-execution-record-plan-087-task-11-2026-08-15)); completion popup 480×340 at caret with 16 items, 8 visible rows, selected row, Escape dismissal, and empty-result dismissal without a blocking panel (module [04](04-core-editing.md#linux-execution-record-plan-087-task-11-2026-08-15)); fixture `completion.trigger` binding and Command Centre non-regression via the plan's task-7 live capture, same build (module [10](10-keybindings-and-commands.md#linux-execution-record-plan-087-task-11-2026-08-15)); completion caps and advisory benches (module [11](11-performance.md#linux-execution-record-plan-087-task-11-2026-08-15)); welcome-return on pane close (module [13](13-window-splits.md#linux-execution-record-plan-087-task-11-2026-08-15)).
- **BLOCKED (host, not a false pass):** this session's xdg-desktop-portal keyboard delivery could not hold Ctrl across the two strokes of `Ctrl+X Ctrl+P`, so the Command Centre/split re-runs were not repeated in this instance; task-7 live captures and automated tests cover those surfaces. `P1-087-UI-1` (popup rows painting below the shell) remains a tracked follow-up.

## Plan 086 task 11 Linux execution record (2026-08-14)

Real Linux/Wayland execution used isolated `clay server <temp-socket>` and `clay client <temp-socket>` processes with mode-700 temporary HOME/XDG roots, a private socket, and a v2 two-tab/two-pane layout. The live AT-SPI tree was queryable and showed deterministic TabList/Tab, pane, status, menu, and announcement nodes. Representative stable IDs were shell TabList `14987979559889014273`, announcement `14987979559889014274`, cards `14987979559889014276/14277`, and Control Center status/items beginning `14987979559889054209`.

- **PASS:** representative launch/status, restored multi-document panes, Control Center open/filter/cancel, split creation/clean close, tab selection/close, bounded announcements, stable virtual object paths, and no-path/ambient-config negative checks. Module-specific records: [01](01-launch-and-connection.md#linux-execution-record-plan-086-task-11-2026-08-14), [03](03-files-and-workspace.md#linux-execution-record-plan-086-task-11-2026-08-14), [10](10-keybindings-and-commands.md#linux-execution-record-plan-086-task-11-2026-08-14), [13](13-window-splits.md#linux-execution-record-plan-086-task-11-2026-08-14), [14](14-tabs.md#linux-execution-record-plan-086-task-11-2026-08-14).
- **FAIL/BLOCKER:** dirty active-pane close crashed the client with `Focused ID #4 is not in the node list` in `accesskit_consumer`; isolated server survived. Evidence is retained under `code-reviews/screenshots/2026-08-14-plan086-a11y/manual-dirty-pane-close-crash.log` and is not a false pass.
- **BLOCKED:** native dialog selection, observer/restart/local-fallback keyboard flows, and full quit/relaunch persistence were not manually re-run because this host's window-targeting/portal backend cannot safely target Clay controls; automated coverage remains separate.

## Conventions

- Steps are numbered `<module><step>` (e.g. `E3`) so failures can cite them.
- "Expected" columns describe the product contract; visual judgments
  (smoothness, glyph shape, blink rhythm) are part of the check.
- Caret and blink evidence must be a **burst capture** (at least four frames
  spanning one blink cycle), never a single frame: a single frame lands on
  either blink phase and reads as a false "override not applied". A live lane
  value and an initial-sync value must be checked separately — the initial-sync
  path can drop an override the live path paints.
- Restart = `cargo run` again; live reload = settings appearance switch
  (module 02) unless stated otherwise.

## Phase 26 Linux execution record (2026-08-19)

Executed against the current `cargo build` on real Linux/GNOME Wayland.
Rendering evidence: fresh captures with the current build
(`code-reviews/screenshots/2026-08-19-phase26-manual-test-plan/` — rust,
markdown, long-line fixtures, all `review.status=PASS`) plus the 17-capture
post-implementation visual review
(`code-reviews/screenshots/2026-08-18-phase26-review/`). Interactive
keyboard delivery is partial: single keys reach the app (typed input made a
document dirty live), but modifier chords (`Ctrl+Alt+W`) and scroll are
host-blocked (portal limitation — review-log V9), so dynamic steps are
covered by the automated suites named in each module record.

| Modules/steps | Result | Evidence |
|---|---|---|
| 07 T20–T22, T25 (dark/gruvbox), T26; 08 S16/S17; 09 P22/P24; 11 Q20/Q21 | PASS live (static states) | Phase 26 review captures: heading ladder + prose column wrap (markdown-*), wrap-none long-line clip (rust-longline-default), gutter/active-line/indent-guides/bracket-match on code (rust-*), chrome off on prose (markdown-*), quote/fence backgrounds + distinct rich vocabulary across 4 themes |
| 07 T23/T24/T27; 08 S18/S19; 04 E25–E27; 11 Q22/Q23 | PASS automated / NOT RUN live | `set_editor_layout_*`, `user_wrap_override_beats_manifest`, `column_wrap_is_narrower_than_viewport`, `search_match_and_quote_backgrounds_join_style_runs`, `style_run_backgrounds_paint_before_glyphs`, `size_scale_ladder_descends_headings_and_clamps_theme_overrides`, theme parser validation, `tests/theme_packages.rs`, incremental parse continuity tests, `editor_baselines` + 16 ms envelope guards; live reload/typing/scroll input is host-blocked |
| 13 S43/S44 (dirty-pane close fix) | PASS automated regression; live partial | `dirty_focused_pane_menu_and_discard_keep_consumer_focus_live` + `dirty_pane_close_rejection_and_discarded_removal_keep_focus_consumer_safe` exercise the exact Plan 086 crash path (menu apply → DirtyDocument → discard → close) with the consumer focus live at every step; the Plan 086 `accesskit_consumer` panic no longer reproduces. Live attempt: typed input dirtied a real document (doc v2) with the client alive and the AT-SPI tree intact; the `Ctrl+Alt+W` chord itself is host-blocked (single-key delivery only) |
| 13 S45/S46 (per-pane chrome) | PASS live (single-pane) / structural (multi-pane) | rust-* vs markdown-* captures show per-mode chrome; pane-scoped paint tests + per-pane decoration aggregate guard cover multi-pane isolation |
| 07 T25 light-theme gutter digit | DEFECT — V4 | `*-modus-operandi/` code captures: current-line gutter digit invisible (`gutterFgActive` 0xf4f1ff vs light `lineHighlight`/panel). Tracked in `code-reviews/screenshots/2026-08-18-phase26-review/review-log.md` V4; fix = light themes define `gutterFgActive` or theme-aware default. Not a blocker for the other Phase 26 steps |

## Phase 28 manual test-plan execution record (2026-08-20)

Executed against a fresh `cargo build --bin clay` on real Linux/GNOME
Wayland. UI preflight for this task: the UI guidance current at execution time; categories
`accessibility` and `testing` inspected; selected
`jakubkrehel/better-accessibility`. AT-SPI and xdg-desktop-portal captures were
available. Full evidence: `code-reviews/screenshots/2026-08-20-phase28-manual/manual-test-plan.md`.

| Modules/steps | Result | Evidence |
|---|---|---|
| 01 launch gate; default/error/recovery; large typography | PASS | Fresh `code-reviews/screenshots/2026-08-20-phase28-manual/{default,error,recovery,large-typography}/` captures expose named controls, bounded status/diagnostic text, and Connected/Disconnected state |
| 04 E28–E32; 10 K78/K80/K82/K83 | UNRESOLVED live; PASS structural | Editor Entry reports `supports_editable_text=false`; no keyboard mutation or preview/comment round-trip claim. Transform, alias, preview registration, routing, and menu-consumption tests pass |
| 04 E33; 11 Q24 | PARTIAL live; PASS automated | Completion rest popup captured; `hel` had no bundled match, so visual prefix order is not claimed. Scorer/recency/cap/hot-path tests pass |
| 08 S21–S24; 11 Q25 | PASS rest / UNRESOLVED collapse feel; PASS automated | Rust fold chevrons captured; compositor targeting blocked repeatable collapse/scroll. Fold visibility and permission/budget tests pass |
| 08 S25–S28; 11 Q27 | PASS rest / UNRESOLVED interaction; PASS automated/security | Markdown link styling captured; pointer targeting blocked hover/activation. Target planning, traversal/HTTP denial, decoration cap, and no-network tests pass |
| 08 S29–S32; 09 P30; 11 Q26 | UNRESOLVED live; PASS worker/bridge structural | P1 repaired `lsp-shared` session options, analyzer workspace-root context, and decoration viewport bytes. Fresh GUI reaches Rust bridge with no `analysis.worker_failed` and emits an inlay set, but first provider response is empty during rust-analyzer warm-up; keyboard backend is unavailable, so visible/toggled-off states remain unresolved under `code-reviews/screenshots/2026-08-20-phase28.7-followups/` |
| 09 P29/P31; 10 K79/K81/K83 | PASS automated / NOT RUN live | Package manifest/keymap parser, Markdown preview registration, closed command backing, malformed chord, permission, and activation tests pass; editable focus prevented false live claims |

No existing manual step was deleted or weakened. Unresolved live rows retain
explicit host/tooling blockers and remain linked to Plan 095 follow-ups.

## Phase 28.7 P2 visual, interaction, and accessibility recapture (2026-08-21)

Executed against the current `target/debug/clay` on real Linux/GNOME Wayland.
The UI preflight ran for this task: the UI guidance current at execution time, category
`accessibility`, selected `rams/rams`, then
`computer-use-linux_get_app_state` and `computer-use-linux_doctor` ran before
interaction. Static fixture states passed; the desktop reports no development
keyboard backend. Full evidence:
`code-reviews/screenshots/2026-08-21-phase28.7-p2-recapture/review-log.md`.

| Modules/steps | Result | Evidence |
|---|---|---|
| 01 launch gate; default/error/loading/recovery/large typography | PASS | Fresh `default/`, `loading/`, `error/`, `recovery/`, and `large-typography/` captures have `review.status=PASS`; AT-SPI dumps expose named controls, bounded panel/status text, recovery menu selection, and Connected/Disconnected state. |
| 04 E28–E36; 08 S21–S32; 09 P29–P31; 10 K78–K83; 11 Q24–Q27 | Mixed live; PASS structural | Completion/Command Centre triggers, fold/link/inlay interactions, comment/list/heading/preview keyboard mutation, and live resize remain explicit `UNRESOLVED`; retained P1 EditableText interface evidence remains valid, with physical keyboard mutation host-blocked. No false visual interaction pass is claimed. |
| Automated companions for E/S/P/K/Q rows | PASS | 39 editor invariants, 24 performance budgets, 19 performance protocol tests, 5 decoration-intent authority tests, focused transform/fold/inlay/completion tests, and prior full Linux suite pass. |

No step was deleted or weakened. Static states are refreshed; unresolved live
rows remain explicit and linked to module records and Plan 094 evidence.

## Plan 098 manual-test-plan execution record (2026-08-26)

The current Linux build was exercised through the workspace-private
`scripts/large-document-smoke.sh` path. It built `target/debug/clay`, started
`clay server <private-socket>`, launched the Tauri client, opened the native
file chooser, and selected a synthetic 52.4 MB Markdown fixture. The chooser
and window target became unstable before a repeatable loaded-editor state, so
live F48–F52/Q34–Q36 interaction remains UNRESOLVED. No GUI pass is inferred
from server tests.

The real server/protocol flow passed `cargo test --test runtime
large_document:: -- --nocapture` (2 tests) with fresh measurements: open→head
297.339689 ms, open→full 589.273483 ms, save→ack 423.454128 ms for 52,428,815
bytes. The run asserted 256 KiB chunk bounds, UTF-8 equality, edit/save/reload,
257 MiB resident-budget refusal, and binary refusal. One host-scheduled
combined run reached 538.188757 ms for open→head and tripped the existing
500 ms guard; isolated reruns and the final combined run passed, so this is
recorded as host variance rather than hidden. The v27 mixed-version negative
check `protocol_v26_client_is_rejected_by_v27_server` also passes.

Sanitized evidence: `code-reviews/screenshots/2026-08-26-plan098-manual/`.
It contains the app-only welcome screenshot, runtime output, status markers,
and this execution note. No user content, host paths, or secrets were
retained. Existing steps were preserved; only L23/L24, F48–F52, and Q34–Q37
were added.

## Plan 099 manual-test-plan execution record (2026-08-28)

Evidence: [`manual-test-plan.md`](../code-reviews/screenshots/2026-08-28-plan099-manual/manual-test-plan.md) and source-free reports under `target/perf/editor-performance/plan099-manual-20260828-181828/`.

The current build generated 72 synthetic variants across 1/10/50 MiB, four
line shapes, and six language extensions, then launched the real profiled
Tauri/WebKit client against a private server socket. AT-SPI/window focus and a
Clay welcome screenshot worked; the WebKit document tree was not exposed and
no keyboard-capable input backend was available. The `--enforce` harness
passed with zero long tasks over 50 ms, 18 frontend retained events/0 drops,
and the only warning was absent `bridge.patch_delivery` because no fixture was
opened. Bootstrap-only p95s were frontend open/ready 0/0 ms, desktop
codec decode/encode 0.151/0.043 ms, server codec decode/encode 0.035/0.233 ms,
and configuration load 14.056 ms. Parser queue count was 0 because no parse
flow ran. An idle supplemental run observed 283,952 KiB tracked RSS across
server/client/desktop; no document was installed, so this is not a document
resident-budget measurement.

| Module | Result | Evidence |
|---|---|---|
| 01 launch/recovery | PASS private launch; UNRESOLVED recovery interaction | Private socket, real Tauri launch, Connected welcome, and AT-SPI frame; keyboard/server-resync flow not driven |
| 03 files/progressive loading | UNRESOLVED live; retained PASS evidence | Current file-open path not drivable; final-build large-loading and four-pane artifacts remain linked in the execution note |
| 04 editing/delayed syntax | UNRESOLVED live | No keyboard backend |
| 08 syntax/viewport/empty patch | UNRESOLVED live | No keyboard/scroll backend; structural tests remain green |
| 11 performance M1–M7 | M1 PARTIAL; M2–M7 UNRESOLVED | Harness and reports above; no false editor-flow pass |
| 13 splits/routing | UNRESOLVED live; retained PASS evidence | Four-pane routing artifact and automated isolation coverage remain available |
| 14 tabs/reconnect | UNRESOLVED live; retained PASS evidence | Prior final-build tab/recovery artifacts and automated isolation coverage remain available |

No existing step was deleted or weakened. Synthetic roots were removed and
retained reports contain no user files, source text, credentials, or ambient
paths.


## Plan 105 manual test-plan execution record (2026-09-01)

Plan 105 (repository review remediation) changes no user-visible behavior by
design: junk/dead-code removal (task 2: tracked scratch files; task 3: never-
compiled native dialog backends), internal refactors (tasks 5, 6, 7, 8, 12),
build config (task 9, index code-split), drift guards/docs (tasks 4, 10, 11,
13). The one deliberate shipping change is the chunk split's startup-loading
behavior, recorded as new step Q38 in [module 11](11-performance.md) instead
of the one-time chunk-split work rather than as a chat-surface step (the
`@clay/chat` surface was removed by plan 118).

| Modules/steps | Result | Evidence |
|---|---|---|
| 01 launch gate (L1/L12/L15 class) + AT-SPI structure | PASS | `scripts/capture-ui-review.sh` fresh-build capture `code-reviews/screenshots/2026-09-01-plan105-manual/default/` (`review.status=PASS`); Clay-only crop retained, full-desktop PNG deleted per evidence policy; welcome state, tab bar, named open actions, sanitized status; footer-invisible-to-AT-SPI stays the documented WebKitGTK ceiling. |
| 04 E37/E38 | PASS automated / UNRESOLVED live | Split editor_performance runtime suite + frontend hot-path suites green on this branch; live typing blocked by the same no-input-backend host ceiling (doctor 2026-09-01). |
| 13 D20/D21, split/tree structure | PASS automated / UNRESOLVED live | Split/pane isolation suites green in full runs; retained Plan 099/plan-097 artifacts unchanged; no input backend for interactive splits. |
| 11 Q38 (chunk-split startup) | PASS | Vite modulepreload parallel load verified with `npm run check:budget` (153.4/180 + 359.7/400 kB gzip) and boot capture of the split build (module 11 record). |
| 12 platform Windows | N/A | Linux-primary validation per plan; no Windows claims made or weakened. |

No manual test step was deleted or weakened. Ledger coverage: Q38 added to
`performance.budgets.feel` (`docs/development/tauri-react-parity-ledger.json`)
with verification evidence; `tests/documentation_coverage.rs` passes.

## Plan 107 Phase 1 agent host execution record (2026-09-02)

Plan 107 tasks 1–13 shipped the Phase 1 agent host without a Chat UI change:
configuration surfaces are the `clay:agent` facades and `examples/init.js`
section 12. New module [16](16-agent-host.md) (steps A1–A19) covers the
user-visible configuration; coding-tool dirty-buffer/approval/search-isolation
behavior is pinned by the automated suites cited in that module, not manual
steps. No chat surface is claimed (the package was removed by plan 118; the
module names the agent surface instead).

| Modules/steps | Result | Evidence |
|---|---|---|
| 16 A1–A4 (config/docs cross-check) | PASS | `node --check examples/init.js` clean; canonical-example doc-registry tests green; inventory coverage gate pins `default:boolean=false` (2157) and `compactAfterTokens:number=80000` (2158); no credential/hidden-key surfaces added |
| 16 A5–A15 (autonomy/compaction/search/tree/checkpoints) | PASS automated | clay-agent suites 49/49 incl. approval default-off, OM settings-provider override, workspace-scoped search with no context injection, checkpoint restore fail-closed |
| 16 A16–A19 (MCP/Obscura fail-closed) | PASS automated | Empty/non-canonical allow-list rejection, missing-binary-hidden, no-vendor-imports tests; Rust `phase25_dependencies_deny_acp_agui_mcp` green |
| Live GUI steps | NOT RUN (host ceiling, per index records) | Same no-input-backend ceiling documented for Plans 097/099/105; no Phase 1 manual step requires driving the agent surface (Phase 2 UI) |

## Plan 112 icon-pack execution record (2026-09-07)

Plan 112 tasks 1–15 shipped configurable icon packs (module
[18](18-icon-packs.md), steps ICON-01–ICON-12): two first-party packages
(`@clay/icons-phosphor-regular` recommended default,
`@clay/icons-phosphor-duotone`), a zero-config bundled fallback subset,
user-global `setIconPack` selection (load ≠ select, trusted-extension-only
op), semantic icon references on SDUI/package surfaces, and the icon-only
control contract. Visual captures (`test-plan/artifacts/112-icons/visual/`)
and isolated example-config launches (`example-launch/`) were executed live.

| Modules/steps | Result | Evidence |
|---|---|---|
| 18 ICON-01–ICON-05, ICON-07–ICON-12 | PASS | `test-plan/artifacts/112-icons/` (`visual/` 4 captures, `example-launch/` default/duotone/unloaded/recovery, `automated/` gates) |
| 18 ICON-06 (third-party adoption, live) | UNRESOLVED | `pnpm` absent on host; identical fail-closed path covered by unloaded/unknown legs and task 5/13 fixture suites |
| 18 ICON-08 (hover/focus tooltips, live) | UNRESOLVED | No input-synthesis backend (keyboard/pointer); AT-SPI names + jsdom tooltip/focus suites cover structure |

Existing modules keep their steps unchanged; affected cross-references were
added to modules 02, 03, 09, 14, 15, and 17. No manual test step was deleted
or weakened. The task 15 performance observation (bounded server logs, no
reload loops) and the one-run transient first-reload `theme.load_failed`
flake are recorded in module 18's ceilings section.

## Plan 129 connection-loop decomposition execution record (2026-09-20)

Regression-only manual pass over the pure refactor that turned
`handle_connection_loop` into a dispatcher (all 31 `ClientMessage` arms route to
per-family handlers in `src/server/connection/{documents,menus,tabs,workspace,runtime}.rs`)
and boxed the cold reload paths to clear the `large_futures` warnings. Modules
[04](04-core-editing.md), [06](06-multi-cursor.md), [07](07-caret-and-typography.md),
and [10](10-keybindings-and-commands.md) were re-run on a freshly rebuilt Linux
build (`cargo build --bins` + `cargo build --bins -p clay-desktop`, both stamped
14:34/14:35) through a new isolated harness
(`test-plan/artifacts/129-connection-loop/`).

| Modules/steps | Result | Evidence |
|---|---|---|
| 04 E2/E3/E5/E6/E7 (editing mechanics) | PASS live | Isolated 65 KiB `review.rs` + bundled `@clay/rust`: Backspace/Delete char-granular, Enter indents the new line, `}` electric-outdents to column 0, `(`/`)` pair insert + skip-over, Tab = 4 spaces — each verified by exact AT-SPI character counts. |
| 04 E1 ASCII typing, E8 undo | PASS live; non-ASCII + redo UNRESOLVED | `AB`/`zzmark2` insert at the caret; `Ctrl+Z` restores text and caret twice. `é`/`🎉` never reached the app through the portal keyboard path, and `Ctrl+Shift+Z` was dropped — recorded UNRESOLVED, not inferred. |
| 04 E4 comment continuation | **FAIL live (defect found)** | `Enter` on a `//` line continues indentation but never the `//` prefix, although `@clay/rust` declares `continuePrefix: "// "` and the docs promise it. `frontend/src/editor/extensions/behavior.ts::applyEnterRule` has no comment branch and `continuePrefix` has no consumer in `frontend/src`. Pre-existing and client-local, **not** caused by this refactor; recorded for a fix-or-re-document decision. |
| 06 X1–X15 (multi-cursor) | UNRESOLVED live; PASS automated | Client-local CodeMirror operations; fresh `npx vitest run src/editor` 9 files / 75 tests passed on the refactored tree. |
| 07 T9 ligatures + invalid-caret negative | PASS live | Rest-state capture shows joined Fira Code ligatures under the 16 px pin; `clientSetCursorStyle({shape:"triangle"})` produced `clay server configuration failed [editor.invalid_set_cursor_style]` with the app still up. Caret-shape paint and layout-reload steps UNRESOLVED (no caret without real input). T25's gutter expectation is stale: the full-bleed editor has no line-number gutter by design (`docs/wiki/modules/react-shell.md`). |
| 10 K19/K29/K30, K22/K35 (Control Center round trip) | PASS live | AT-SPI `press` on the shell's `palette Ctrl X O` button opened the Control Center with 88 catalogue rows (built-ins + `shell.client*` + package commands with provenance); activating `Reload Configuration and Packages` executed `runtime.reloadConfiguration` (init.js re-ran, client stayed connected, no `Session lost`). |
| 10 query/arrow/Escape/chord steps (K20/K21/K23/K24/K31/K33/K34/K38/K42/K60–K68) | UNRESOLVED live | Keyboard/arrow legs and the client shell bridge need key delivery; AT-SPI row `DoAction` is not an `Enter` substitute for client-first rows. Fresh automated companions: `cargo test --lib control_center` 28 passed. |
| Fresh automated regression on the crafted tree | PASS | `cargo test --lib connection::` 96 passed; `cargo test --lib completion::` 31 passed; `cargo test --lib control_center` 28 passed; frontend `vitest run src/editor` 75 passed. |

New host ceiling recorded by this execution: `xdg-desktop-portal-gnome`
segfaults on RemoteDesktop keyboard sessions (`code=dumped, status=11/SEGV`,
`g_hash_table_lookup` assertion), so keystrokes land only in short bursts after
a fresh app launch and every longer interactive sequence is recorded UNRESOLVED
with its reason instead of being inferred. No manual step was deleted or
weakened. Artifacts, including the harness, per-run probe logs, screenshots, and
the finding details: `test-plan/artifacts/129-connection-loop/`.


## Plan 130 agent-host decomposition execution record (2026-09-20)

Regression pass over the agent-host ownership change (the agent-host handle is
injected into each runtime lane; no process-global authority, per-lane
registration queue, `ClayAgentHost` split into `clay-agent/src/host/*.ts` with no
function above the 80-line budget) plus the one behavior fix it carried: a
resumed coding session re-activates its recorded workspace's capabilities and
graft binding (module [16](16-agent-host.md) A25, module
[17](17-coding-agent-parity.md) C43). Fresh Linux build: `target/debug/clay`
19:36, `clay-desktop` 19:37, `clay-agent/dist` 19:37, `frontend/dist` unchanged
(no frontend source newer than its 2026-09-18 build).

| Modules/steps | Result | Evidence |
|---|---|---|
| 16 A1–A4 (config level) | PASS live | `node --check examples/config/init.js`; section 12 documents the six `clay:agent` exports with commented-only examples; inventory defaults (`compactAfterTokens:number=80000`, `default:boolean=false`); no credential option or secret-shaped string in `examples/`. Stale `examples/init.js` paths and the "five exports" count in the steps were corrected |
| 16 A5–A7 (autonomy) | PASS automated + **expectation corrected (finding)** | Steps asserted the stale "approvals on by default (decision 2157)"; creation is actually opt-out (`fullAutonomy !== false`, user decision 2026-09-05, test `session.setAutonomy toggles full autonomy; default stays true`) while `docs/reference/clay-js-api/agent/set-full-autonomy.md` + the inventory still document `default:boolean=false`, and a resumed session starts non-autonomous because `ensureLive` writes its live record with `fullAutonomy: false`. Rows corrected; the doc-vs-code divergence is recorded for a fix-or-re-document decision |
| 16 A8–A20 (compaction/OM, search, tree, fork/clone, MCP/Obscura, toolNames) | PASS automated + live where drivable | Real-daemon protocol probe (`live-daemon.log`, 24 legs): prompt to `agent_finished`, workspace-scoped `session.search` (hit / no-hit), `session.resumable` root scoping, `session.clone`, `session.fork`, `session.setAutonomy` both ways, `session.compact` + persisted entry, allow-listed MCP connect (`connected:true, tools:2`), Obscura hidden. `session.checkpoint` stays automated-only (needs a document backend) |
| 16 A25 / 17 C43 (resume binding, **new step**) | PASS live + automated, falsified | Session recorded in workspace B (real graft repo), daemon restarted in an unrelated cwd resumes it: `session.resume` reports B, graft skill/tool re-bound, `environment.list` shows the graft extension + MCP re-connected. Removing the activation fails exactly those three legs (`live-daemon-falsify.log`); `clay-agent` case `resumed session binds its tools to the recorded workspace root, not the daemon cwd` |
| 16/17 lane + palette steps (C57–C64, K92–K99, A21–A24) | PASS live via AT-SPI, no input synthesis | Isolated launch (`CLAY_AGENT_MOCK=1`, canonical example config): `Agent lane`, `Message` entry, `Coding Agent Agent type` picker at rest; palette opened from the status-bar button lists every daemon slash command (`/resume`, `/branch`, `/fork`, … `server-first — @clay/coding-agent@0.1.0`); `Session` scope narrows to those 14 rows; lane hide/restore verified by node counts (`test-plan/artifacts/130-agent-host/gui/`) |
| Composer typing, send, approval drill, session create/resume from the lane | UNRESOLVED live | Standing host ceilings (no `/dev/uinput`, no `xdotool`/`ydotool`, portal keyboard crash loop recorded by plan 129) plus no provider credentials in an isolated profile — the lane sits in `no provider configured · Settings · Providers` (`[agent] ensure_tab_session(1): empty selection`). Every step keeps its automated leg; the resume semantics are covered live at protocol level |
| Clay JS API surface (plan 130 verify-only step) | PASS — **no JS API change** | Diffed against pre-decomposition `HEAD`: identical daemon method set (50 dotted ids, 51 dispatch cases) and 11 `op_clay_agent_*` wrappers, `runtime/js/agent.js` + `.d.ts` diff 0 lines, all 11 `agent/*.md` pages and their 11 generated-registry entries deep-equal, `update-doc-registry` a byte-identical no-op. The 74 `clay_js_*` + 14 `documentation_coverage` guards pass (the latter forced one ledger row for the new A25 step), as does `rust_visibility_api_mapping`. Full record: `test-plan/artifacts/130-agent-host/js-api-verification.txt` |
| Code wiki (plan 130) | PASS, pinned | `clay-agent.md` gained the host module map (12 modules behind the class facade + contract rules) and the server page the injected-ownership/queue/fail-closed text; `embedded-js-runtime.md` names the single per-lane wiring point; both index entries updated. New guard `documentation_coverage::plan130_wiki_pages_describe_host_modules_and_injected_ownership`; the 11 `agent/*.md` backing-path citations were audited (still true as delegates) and the one moved wiki citation (`labelFirstPromptStore` → `host/internals.ts`) corrected |
| Autonomy default (decision 2026-09-20-2049) | PASS automated + **decided** | The plan-130 pass recorded a docs-vs-code divergence on `agent.setFullAutonomy` (`default:boolean=false` documented, opt-out in the daemon) and a resumed session coming back non-autonomous. User decision: "Default should be autonomy. User can configure to block autonomy" → docs/inventory/registry/example config/wiki moved to `default:boolean=true`, `ensureLive` restores the recorded autonomy, and `tests/clay_js_api_inventory.rs` pins both the documented default and the daemon expressions. `test-plan/16-agent-host.md` A5–A7 state the resolved policy |
| Regression suites | PASS | `cargo test` 0 failures (lib 1421 pass / 1 ignored; presentation 62; protocol 226 incl. `agent_protocol::*`/`agent_session_isolation::*`; runtime 75; security 152); fresh `clay-agent npm test` 182 tests (181 pass / 1 skip / 0 fail); `frontend vitest run src/agent src/shell` 11 files / 125 tests |
| Authority ownership (plan 130 A1) | PASS live + automated | Live server log: each registration resolves the **lane's injected host** (`[agent-reg] … host=live -> Ok({"queued": true})`, i.e. the A1 ownership path — not the `host=absent` per-lane-queue branch) and the daemon then reports `[daemon] … applied`; `server::tests::two_servers_in_one_process_own_independent_agent_hosts`, `agent_protocol::initialize_handshake_carries_the_built_mcp_allow_list` |

Ceilings and follow-ups recorded by this run (no step deleted or weakened):

- **Portal screenshots for the isolated client are not available on this host.**
  The portal screenshot surface exposes only the currently visible
  workspace/monitor while the isolated client opens on another workspace, so the
  plan-126/129 `portal-shot.py` copy cropped an unrelated desktop window. That
  image was deleted before it entered the repo — no screenshot from this run is
  retained. `test-plan/artifacts/130-agent-host/portal-shot.py` is now hardened
  (Clay's live AT-SPI frame is the crop source, a compositor rect is trusted
  only when it lies inside that frame, and it exits 2 without writing a PNG when
  the geometry disagrees or the crop escapes the screenshot); it refused every
  capture attempt, which is why the live evidence is AT-SPI trees + action
  results. **Follow-up:** the copies in `test-plan/artifacts/126-*/`, `127-*/`
  and `129-*/` still trust the compositor listing alone and can retain another
  window's pixels; copy this directory's version forward.
- Load-sensitive flakes seen once each, neither plan-130 related and both green
  standalone and in the serial rerun: `server::config_watch::tests::watcher_detects_new_and_deleted_watched_files`
  (2 of 3 inotify events while three suites ran in parallel) and
  `clay-agent`'s `resumable list is workspace-scoped, most-recent first, and bounded`
  (timestamp tie; 0 failures over the following 13 runs).
- **Autonomy default: documentation and code disagree.** `session.new` without
  `fullAutonomy` creates an **autonomous** session (approvals opt-out;
  `clay-agent/src/host/sessions.ts`, user decision 2026-09-05), and
  `session.setAutonomy toggles full autonomy; default stays true` pins it, while
  module 16 A5–A7, `docs/reference/clay-js-api/agent/set-full-autonomy.md` and
  `api-inventory.toml` all still say `default:boolean=false` (decision 2157).
  Separately, a **resumed** session comes up non-autonomous: `ensureLive`
  creates the session from the recorded autonomy but writes the live record with
  `fullAutonomy: false` (the oddity flagged by plan 130 task 3; now pinned by the
  A2 resume test). Steps corrected, divergence recorded — needs a doc-or-code
  decision, no behavior changed by this pass.
- Path/step staleness fixed rather than carried: module 16 A1–A3 now name
  `examples/config/init.js` (the example tree moved under `examples/config/`),
  and A2 counts the six `clay:agent` exports (`agent.knowledgeSetOptions` was
  missing from the list).

Artifacts: `test-plan/artifacts/130-agent-host/` (`README.md`, `live-daemon.log`
+ `.json`, `live-daemon-falsify.log` + `.json`, `live-daemon.mjs`, `gui/`,
`gui-live.sh`, `probe.py`, hardened `portal-shot.py`, `automated-legs.txt`,
`config-legs.txt`).

## Plan 132 fanout/DTO-dedup execution record (2026-09-21, task 5)

Regression-only manual pass over plan 132: the five state lanes now share
`StateFanout<T>`/`Fanout<T>` (`src/server/fanout.rs`), the typography and
runtime-generation lanes publish through `Fanout`, and the Tauri bridge DTO
transcription was deduplicated behind ts-rs projections plus `From`/`TryFrom`
destructures. No user-visible change was intended, so modules
[04](04-core-editing.md) (core editing), [07](07-caret-and-typography.md)
(caret/typography/wrap), [10](10-keybindings-and-commands.md) (editor
commands), [13](13-window-splits.md) (pane focus) and
[15](15-ui-design-systems.md) (design system/theme) were re-run on a freshly
rebuilt Linux build (`target/debug/clay` + `clay-desktop`, both 16:46/17:05
stamps) through a new isolated harness
(`test-plan/artifacts/132-fanout-dedup/live/`, modelled on the plan 129/130
harnesses). No step was added, deleted or weakened: every divergence found is a
pre-existing client gap, not a lane regression.

| Modules/steps | Result | Evidence |
|---|---|---|
| 04 E1/E8 + 07 T9/T20 (typing, undo, ligatures, typography pin) | PASS live | Real `wtype` keystrokes into the webview: `AB` typed → editor chars 107→109, caret 0→2; `Ctrl+Z` → 107/0. The ligature sample renders joined glyphs (`⇒ ≠ = →`) under the 16 px pin. `live/caret/01..03*.png` |
| 07 T1/T5/T23/T24 (caret override + column wrap, live lane delivery) | PASS live, burst-verified | The caret override set from init.js does not paint on a fresh connect (client drops pre-view messages, finding 1) but **does** paint after `Ctrl+Shift+R` re-runs init.js on the live channel: burst captures show the 8 px override frame, byte-identical to the stashed pre-change build's frame (`sha256 67e737f3…`). `columnCap: 40` wraps into a centered 40 ch column after the reload. `live/caret/08..10*.png`, `live/wrap/01`, `live/wrap/03-column40-after-reload.png` |
| 07 T22 + wrap `none`/`viewport` overrides | **FAIL live (pre-existing client defect)** | `wrapPolicy: "none"` never applies: `controller.ts:661-672` tests `"none" in wrap` on the string serde emits for a unit variant, so the handler throws and the override is dropped. `{"column": N}` (an object) works. Module 07 T22's "no wrap" default also needs the mode manifest (`@clay/rust`) loaded. `live/wrap/02-none.png`, `live/wrap/03-none-after-reload.png` |
| 07 T2/T3/T5 wording (`blink`, `hollow`) | **stale expectation (pre-existing)** | `hollow` appears only in the generated DTO and nothing reads `blink`, so the client caret always blinks (verified by alternating frames) and never renders a hollow block. `live/caret/caret-blink-*.png`, `caret-solid-*.png` |
| 07 T8 negative (invalid caret shape) | PASS live | `clientSetCursorStyle({shape:"triangle"})` → `clay server configuration failed [editor.invalid_set_cursor_style]`, editor unaffected and still connected. `live/caret-invalid/server.log` |
| 10/04 editor-command lane (advisory, no replay) | PASS live | init.js's `clientExecuteEditorCommand` is published before the client subscribes → dropped (caret stays 0, the Advice policy the plan preserved); after `Ctrl+Shift+R` re-runs init.js with the client connected the command arrives and executes (caret 0→2, `agentProfile.register` 1→2). `live/editing/editor-command-server.log` |
| 13 S14/S17 (pane focus policy) | UNRESOLVED live / PASS automated | The Rust client forwards `ClientConnectionEvent::ShellPreferences`, but no frontend module consumes `paneFocusPolicy`, so moving the pointer across the divider cannot switch panes. Server side pinned by `set_pane_focus_policy_publishes_shell_preferences` + `shell_preferences_default_to_click_when_unset`. `live/panes/status-*.png` |
| 15 theme + design-system activation | PASS live (partial) | The shipped fixture activates `@clay/design-instrument` + `@clay/theme-gruvbox-material-dark` + dark appearance with no diagnostics and the shell renders styled (workspace chip in the theme's green, not the default blue) — i.e. the design-system snapshot projections still drive `--clay-ds-*`. The fixture's SDUI panel does not surface on the empty tab under this harness, and `scripts/capture-ui-review.sh`'s portal screenshot step is blocked by an interactive "Allow Apps to Take Screenshots?" prompt on this host (observed and dismissed); captures here use grim. `design-system/design-system-dark.png`, `design-system/dark/*` |
| Fresh automated companions | PASS | `cargo test --lib`: `server::fanout` 4, `caret` 6, `typography` 17, `editor_layout` 3, `shell_preferences` 2, `connection::` 96; `cargo test -p clay-desktop --test dto_roundtrips` 16; frontend `vitest run src/theme src/editor src/shell` 173. `live/automated-companions.txt` |

Findings recorded by this pass (no behavior changed; each needs a
fix-or-re-document decision):

- **Initial-sync caret override is dropped client-side** (`controller.ts:628`
  returns while `this.view` is null). Reproduced on the stashed pre-change
  build, so it predates plan 132.
- **Unit wrap policies throw** in `controller.ts:661-672` (`in` on a string) —
  `wrapPolicy: "none"`/`"viewport"` from init.js cannot apply.
- **`hollow` and `blink` caret fields are delivered but unread**, so module
  07's T2/T3/T5 expectations are stale.
- **`paneFocusPolicy` has no frontend consumer**, so module 13's S14/S17 stay
  unobservable live.
- **Method note:** caret shape evidence must be a burst capture; a single
  frame lands on either blink phase and reads as a false "override not
  applied".

Artifacts: `test-plan/artifacts/132-fanout-dedup/` (`README.md` with the
harness, per-leg evidence and the finding details; `live/` run harness,
fixtures and captures; `design-system/`).

## Plan 133 file/complexity decomposition execution record (2026-09-22, task 8)

Regression-only manual pass over plan 133, which is a pure internal refactor:
`src/protocol/mod.rs` split into family modules, `src/server/mod.rs` into
`runtime_state`/`runtime_reload`, the giant inline test modules moved to sibling
suites, `src/server/ui.rs` validation became table-driven, the string-enum
triples came from one macro, and `src/shell/theme.rs` split into
parse/resolve/validate modules. No user-visible behavior changed, so **no step
was added, deleted, or weakened**; modules [09](09-packages-and-modes.md)
(package/mode UI-token validation) and [15](15-ui-design-systems.md) (theme
tokens, contrast, design-system activation) were re-run as a regression pass
instead, and every existing step remains current.

| Modules/steps | Result | Evidence |
|---|---|---|
| 09 package/mode UI validation (`server/ui.rs` table-driven validation, task 5) | PASS automated | `cargo test --lib server::ui` 24; `cargo test --lib packages::` 83; `cargo test --test security` 152 (package loading/graph/conflicts/primitive-gate suites); `package_ui_conformance` guards show every contribution/override validation error kind unchanged |
| 15 theme tokens/contrast + design-system activation (task 7 split) | PASS automated | `cargo test --lib shell::theme` 29 (same 22 + 7 test names after the split); `cargo test --test presentation` 62 (`theme_packages` 15 + `package_ui_conformance`); contrast floors and shipped-theme re-derivation green |
| Protocol/enum string forms + doc guards (tasks 2 and 6) | PASS automated; 2 stale pointers fixed | `cargo test --lib str_enum` 2 (`string_roundtrip_per_enum` pins exact shipped strings + unknown rejection); `cargo test --test protocol` 227 (incl. `primitives_docs` 35 and `package_loading_docs`); `cargo test --test runtime` 75. Task 2 also left two live pointers stale — the frontend guard (row below) and the `default_keymaps()` “source of truth” comments in `examples/config/init.js` + its `plan080-manual` fixture copy, now retargeted to `src/protocol/behavior.rs` |
| Root gate over the whole refactor | PASS | `scripts/check.sh full` audit/fmt/check/clippy `-D warnings`/test/bench-compile green; root totals **1944 passed / 0 failed / 1 ignored**, identical to the task-1 baseline record. No observable behavior difference through any of them. Re-run after WebKitGTK 4.1 was installed (2026-09-22 19:2x): **exit 0, `full check PASSED`**, desktop-clippy + desktop-test (55 passed) and the bindings guard (`webview bindings up to date`) now green too — `task8-check-full-desktop.log` |
| Server launch smoke (module 01 launch-gate class, server half) | PASS live | Fresh `target/debug/clay` on an isolated mode-700 root/socket with `--config-fixture ui-review-default`: `clay server listening on /tmp/clay133/clay.sock`, mode-700 socket bound, process alive, no configuration diagnostics, clean kill |
| Desktop GUI steps (modules 01, 09, 15) | PASS live (4 fixtures) | WebKitGTK 4.1.2/2.52.6 installed; `scripts/capture-ui-review.sh` (portal screenshot + AT-SPI, isolated mode-700 root, private socket) on a freshly built `clay`/`clay-desktop`: `ui-review-default` (1906×1099) shell + file browser + empty-tab Open File/Open Folder landing, no configuration diagnostics; `ui-review-design-system` (1906×1099) shipped `@clay/design-instrument` + gruvbox-material-dark active with the SDUI review panel, Primary action button, Enabled/Disabled list rows and editor pane (AT-SPI landmark/button/listbox/listitem present); `ui-review-design-system-light` same layout under gruvbox-material-light (UI-DS-15 cross-theme invariant, no stale color/geometry shift); `ui-review-large-typography` enlarged user-owned UI typography in outline rows, status bar, landing heading/buttons. All four `review.status=PASS`, `configuration_failed_lines=0`. Captures: `test-plan/artifacts/133-file-decomposition/live/<fixture>/` |
| Frontend suites | PASS; one stale guard fixed | `npm ci` + `npm run build` (tsc + vite) + `npm test` **51 files / 497 tests passed**; `npm run check:budget` shell 178.6/180 kB + total 403.5/404 kB gzip. **Finding (fixed):** `frontend/src/test/core-baseline-hierarchy.test.ts` still read `src/protocol/mod.rs` for the typography `DEFAULT` block that task 2 moved to `src/protocol/typography.rs`, so its slice was empty and the test failed; the frontend suite is outside `scripts/check.sh`, which is why the Rust gate missed it. The guard now reads `src/protocol/typography.rs` (values unchanged); suite, typecheck and prettier green |
| Clay JS API surface (task 9, verify-only) | PASS — no change | Diffed vs the pre-plan baseline `6ee3b4c`: `runtime/js/`, `docs/reference/clay-js-api/`, `docs/generated/`, `docs/index.md`, `frontend/src/bridge/`, `src-tauri/bindings/` byte-identical (`cargo run --bin update-doc-registry` a byte-identical no-op); `src/server/ops/ui.rs` byte-identical; `UiContributionRule` unchanged (17 variants); all 21 closed-choice `ui.*` diagnostics recompose byte-for-byte from the task-5 table and the moved tests assert every full string. Guards: `server::ui` 24, `str_enum` 2, protocol 227, presentation 62, security 152. Report `test-plan/artifacts/133-file-decomposition/task9-js-api-verification.txt` |
| Code wiki (task 10) | PASS | `protocol-codec.md` gained the protocol family module map (task 2) + `src/str_enum.rs` golden-string note (task 6); `slot-aware-package-ui.md` the task-5 table-driven validation section (cost: one ≤21-row static scan per closed-choice field, no allocation on the accepted path, no hot-path reads) and the task-7 theme parse/validate/resolve split, both stating the allowed sets, 17 rule kinds, messages, and op boundary are unchanged; `maintenance-validation.md` the Plan 133 decomposition/test-layout section and the corrected suite topology; `server-ipc-skeleton.md` the `runtime_state.rs`/`runtime_reload.rs` split. 61 test-path references across 18 pages rewritten to the moved suite directories plus 23 retargets to the moved test files; six `docs/wiki/index.md` entries updated. Guards: protocol 227 (`primitives_docs` wiki-index/link checks, `documentation_coverage`, the plan-088/125/129 wiki pins), presentation 62. Report `test-plan/artifacts/133-file-decomposition/task10-wiki-verification.txt` |

No manual test step was deleted or weakened; no module file needed an edit
because no step changed. Artifacts:
`test-plan/artifacts/133-file-decomposition/task8-manual-regression.txt` (raw
regression output, launch-smoke record, blockers) plus the per-task records
`task2`–`task7` in the same directory.

## Plan 134 robustness/resource-hygiene execution record (2026-09-22, task 6)

Regression-only pass over plan 134 (float comparisons, mutex poison policy,
async hot-path filesystem, snapshot swap): no user-visible behavior was supposed
to change, so modules [15](15-ui-design-systems.md) (theme/appearance) and
[03](03-files-and-workspace.md) (files/workspace, open/save) were re-run. **No
step was added, deleted, or weakened** — both module step tables are unchanged.

| Modules/steps | Result | Evidence |
|---|---|---|
| 15 theme/design-system activation (UI-DS-01/UI-DS-15) | PASS live | `scripts/capture-ui-review.sh` on a freshly built `clay`/`clay-desktop`: `ui-review-design-system` and `ui-review-design-system-light` PASS at viewport 1906×1099 (distinct screenshots; landmark `Design system review`, button `Primary action`, entry `Document editor`; `configuration_failed_lines=0`); `ui-review-default --theme @clay/theme-gruvbox-material-light --appearance light` PASS with `metadata.txt` recording the seeded theme/appearance — the live path through the reactor-side persisted-appearance read made async in task 4 |
| 03 workspace open / folder registration (F1/F11 class) | PASS live | `ui-review-workspace` PASS at 1906×1099: file browser lists `review.md`, landmark `Editor review.md`, Document editor present, 0 configuration failures. Isolated live session (plan-132 harness, `run-live.sh start editing`): the workspace root was opened from `layout.json` and `launcher.json` was written with that root (async `record_recent_workspace`) |
| 03 save (F4/F5 class) | PASS live | Live `wtype` typed `// plan134 p3 probe` (editor chars 64→85 via AT-SPI) and `Ctrl+S` wrote the file to disk (content verified), server log 0 errors/diagnostics |
| 03 external-edit reload (F7 class) | UNRESOLVED — host surface ceiling | `documents.reload` is exposed only as the Clay JS API in this build; the Control Center offers only `Reload Configuration and Packages`, so a live keyboard-driven reload has no surface. Covered automated (reload suites in the task-5 gate); recorded as a ceiling, not a false pass |
| Launch gate (module 01 server half) | PASS live | Every isolated run bound a private mode-700 socket with 0 configuration failures |

Harness note: the first capture on the focused compositor tag tiled the window
to a 949×521 viewport (below the 900×600 floor) and correctly recorded
`UNRESOLVED`; all captures above ran on an empty tag and measured 1906×1099.

Artifacts: `test-plan/artifacts/134-robustness/manual-pass.md` plus
`…/live/{design-system-dark,design-system-light,theme-seeded-light,workspace-open}/`
(screenshot, AT-SPI dump, metadata, review.status=PASS).

## Plan 136 manual-test-plan execution record (2026-09-23, task 13)

Executed on a fresh `cargo build --bin clay -p clay` / `cargo build -p clay-desktop`
on Linux, through the isolated harness (`run-live.sh` modes `granted-lane`,
`ungranted-lane`, `config-granted-lane` — the last one added by this task — plus
`drive-granted-lane.sh`), with a private mode-700 root per run for
`HOME`/XDG/`TMPDIR`, a private socket, and a local fixture package. Teardown is
PID-based (`run-live.sh stop`); the developer profile was never opened and the
store never left the scratch root.

| Steps | Result | Evidence |
|---|---|---|
| 09 P56 positive (grant → enable → load) | PASS live + PASS structural | `clay package authorize @fixture/lane --capability completion-provider --capability mode-registration --capability parse-document` → `Authorized … (native-trust)`, `granted by: cli`; `clay package enable` → `Enabled @fixture/lane`; the live launch (client connected, window focused at 952×1150+963+45, typing landed 29 → 35 chars) logged **zero** `configuration failed`/`packages.load_failed` lines, so the granted package's code ran. Contributions are structural: the fixture's registrations appear on their lanes (`js_runtime.lane.third_party.latency.dispatched` 3 = module-backed completion provider, `…general.dispatched` 5 = parse handler) and the automated lane test asserts the provider answers while the general lane is busy |
| 09 P56 negative (no grant) | PASS live | Same fixture adopted but never granted: `clay package enable` → `Error: MissingCapabilityGrant { package_name: "@fixture/lane", capability: CompletionProvider }`; the launch logged only the sanitized `clay server configuration failed [packages.load_failed]: JavaScript runtime evaluation failed.`; no contribution, no crash |
| 09 P57 CLI grant verb lifecycle | PASS live | `inspect` before → no `Grants:` line; after `authorize` → `Grants: completion-provider, mode-registration, parse-document (native-trust)` + `Granted by: cli`; `enable` succeeded. `revoke` → `Adoption: approval revoked` + `Ungranted: … (declared, not granted)` and `enable` → `AdoptionRequired { code: "package_approval.revoked" }`. Replacement: authorizing only `mode-registration` printed `Ungranted: completion-provider, parse-document` and `enable` failed closed with `MissingCapabilityGrant`; re-granting the full set with `--approved-by user` recorded `granted_by: user` and enabled. Negatives: `--capability package-control` → `package @fixture/lane does not declare capability package-control in its manifest`; `authorize` on a revoked record refused (`run clay package adopt … first`) |
| 09 P58 config `authorize` call | PASS live | `init.js` calling `authorize({package, capabilities, runtimeProfile: "native-trust", approvedBy: "config"})` wrote `grant: {capabilities: [completion-provider, mode-registration, parse-document], runtime_profile: native-trust, granted_by: config, granted_at: …}` into the store's approval record; a **separate** `clay` process then printed `Grants: … (native-trust)` / `Granted by: config` and its `enable` succeeded; the relaunch on the same store loaded the package with zero failure lines. Negatives (automated): `clay:packages` is trusted-domain-only so package code cannot import `authorize`, and `package_code_cannot_self_grant_capabilities_during_activation` pins the `packages.grant_during_activation` refusal; `grant_is_inert_after_provenance_change` pins fail-closed after a reinstall |
| 11 Q44 latency lane while the general lane is busy | PASS measured + PASS live presence; live popup ceiling recorded | `cargo test --lib lanes_and_queues -- --nocapture` (17 tests): `PLAN136_GRANTED_LANE busy_ms=500 idle_median_us=1619 busy_completion_us=2937 workers_started=4` (plan 127 record 1567/2594 µs) with `granted_third_party_provider_serves_from_latency_lane_when_general_lane_busy` and `ungranted_third_party_provider_still_fails_closed` green. Live on the granted fixture: 6 typed characters acknowledged at `server.edit_ack` p50 165.0 µs / p95 191.3 µs (plan 136 baseline p50 231.8 / p95 282.1 µs) and `server.document.apply_edit` p50 42.6 µs, with the occupancy counters present in the summary for the first time (all 20 `js_runtime.lane.<domain>.<lane>.{dispatched,pending,peak_pending,superseded,evicted}` keys; 175 retained events, 0 dropped) |
| 11 Q44 negative/ceiling | Ceiling recorded | The live completion popup cannot be driven: package-owned modes do not activate for open documents and a package mode rejects the built-in `completion.trigger` command, so the AT-SPI probe reports `no completion surface or status text` and no live frame number is claimed. The ungranted run's lane counters come from the bundled packages the fixture config loads, not from the ungranted fixture — the fail-closed evidence for it is the `MissingCapabilityGrant` + `packages.load_failed` pair above |

Ledger and module updates: `test-plan/09-packages-and-modes.md` (P56 flipped to
the positive path with the negative sub-step kept, new P57/P58, plan 127 section
reduced to its historical record), `test-plan/11-performance.md` (new Q44), the
module map rows for 09/11 and the coverage matrix row in this index, and
`docs/development/tauri-react-parity-ledger.json` (P57/P58 under
`packages.modes.settings.themes`, Q44 under `performance.budgets.feel`).
Artifacts: `test-plan/artifacts/136-capability-grants/manual-plan/`
(`granted/`, `granted-typed/`, `ungranted/`, `config/` with the store JSONs and
per-verb logs, `automated-lane.txt`) and the harness fixtures
`test-plan/artifacts/136-capability-grants/init-config-granted-lane.js`.
