# 13 — Window Splits

Equal-area window splits and per-pane document views: split/close/add-equal/
move/resize of panes in the working area, pane focus policies, one document
view per pane (Phase 22.2), the user-rebindable `shell.client*`
command surface, and the Phase 22.7 direction-named split aliases. Deep
references:
`docs/reference/primitives/shell-layout-strategy.md` (Phase 22.1 + 22.2
sections), `docs/reference/clay-js-api/shell/` (command + configuration docs),
`docs/reference/clay-js-api/editor/client-show-open-documents.md`,
`docs/development/file-open-save-reload-workflow.md` (document lifecycle),
`examples/init.js` section 6 (shell policy) and section 5 (key bindings).

## Setup

- Scratch workspace: `mkdir -p /tmp/clay-manual && cd /tmp/clay-manual`,
  three small files `a.md`, `b.md`, and `c.rs` (e.g. `fn main() {}`) with a
  few lines each.
- Start with NO shell bindings in init.js (defaults ship built-in); the
  override steps below add their own init.js lines.
- Launch: `cargo run` from the repository root.
- Default chords (all overridable via `bindKey` with `{ scope: "global" }`):

| Action | Default chord |
|--------|---------------|
| Split vertical (side by side) | `Ctrl+\` |
| Split horizontal (stacked) | `Ctrl+-` |
| Add equal pane (redivide) | `Ctrl+Shift+\` |
| Close pane | `Ctrl+Alt+W` |
| Focus prev/next pane | `Ctrl+Alt+Left` / `Ctrl+Alt+Right` |
| Resize pane left/right/up/down | `Ctrl+Alt+Shift+arrows` |
| Move pane prev/next | `Ctrl+Alt+[` / `Ctrl+Alt+]` |

Phase 22.7 direction aliases (`shell.clientSplitPaneRight` =
`clientSplitPaneVertical` beside; `clientSplitPaneDown` =
`clientSplitPaneHorizontal` below) have NO default chords — bind them in
init.js (S29–S32).

## Split creation

Phase 22.3 note: all S-steps run on the ACTIVE tab's split tree. Each tab is
an independent client view owning its own split tree — splits, closes,
resizes, moves, focus policies, and per-pane documents in one tab never
affect another tab (see module 14, T5/T8/T14).

| # | Action | Expected |
|---|--------|----------|
| S1 | With one pane focused, `Ctrl+\` | Two panes side by side, EQUAL widths, hairline divider between them; focus ring on the previously active pane; new pane shows empty placeholder surface |
| S2 | `Ctrl+-` on a single pane | Two panes stacked, EQUAL heights, horizontal divider |
| S3 | From 2 panes, `Ctrl+Shift+\` | Working area redivides into 3 EQUAL-area panes along the existing split axis; existing pane contents keep their reading order; the new pane is the free placeholder |
| S4 | Repeat S3 until 4 panes, then `Ctrl+\` and `Ctrl+Shift+\` again | NO-OP both times — 4 panes is the per-tab cap; no crash, no layout flicker |

## Close and merge

| # | Action | Expected |
|---|--------|----------|
| S5 | With 2 panes, `Ctrl+Alt+W` | Closed pane removed; remaining pane fills the whole working area; focus moves to the survivor |
| S6 | With a single pane, `Ctrl+Alt+W` | NO-OP — the last pane cannot be closed; editor stays as-is |

## Divider drag resize (mouse/trackpad)

| # | Action | Expected |
|---|--------|----------|
| S7 | Split vertical, then drag the divider with the pointer | Ratio follows the pointer smoothly; divider hit area is easy to grab; ratio clamps (a pane can never collapse to ~nothing — min/max ratio 0.05/0.95); release keeps the last ratio |

## Keyboard resize

| # | Action | Expected |
|---|--------|----------|
| S8 | Focus a pane bordering a divider, press the matching `Ctrl+Alt+Shift+arrow` (e.g. right pane + Left arrow grows it leftward) | Divider moves one fixed step (5% of the working area) per press; repeatable |
| S9 | Keep pressing the same chord past the clamp | Movement stops at the ratio bound; further presses are no-ops, no crash |
| S10 | Press a resize chord whose direction has NO bordering divider (e.g. Left on the leftmost pane of a vertical split) | NO-OP |

## Pane move (reorder)

| # | Action | Expected |
|---|--------|----------|
| S11 | With 2+ panes, `Ctrl+Alt+]` | Focused pane swaps with the next pane in reading order (left→right, top→bottom); tree shape/ratios unchanged; focus stays with the moved pane |
| S12 | `Ctrl+Alt+[` at the FIRST reading-order position, `Ctrl+Alt+]` at the LAST | NO-OP at both ends |

## Pane focus

| # | Action | Expected |
|---|--------|----------|
| S13 | With 2 panes (default `click` policy), pointer-down inside the inactive pane | That pane becomes active (focus ring moves); click position lands in its content normally |
| S14 | Add to init.js: `import { setPaneFocusPolicy } from "clay:shell"; setPaneFocusPolicy({ paneFocusPolicy: "cursor" });`, reload | Merely MOVING the pointer across the divider switches the active pane — no click needed |
| S15 | Under `cursor` policy, drag a divider across the other pane | Divider drag continues smoothly; focus does NOT flip mid-drag (focus changes skipped while dragging) |
| S16 | With 2+ panes, press `Tab` / `Shift+Tab` | Cycles pane focus forward/backward (Phase 20.3 behavior; see known ceilings — Tab does not insert indentation while splits exist) |
| S17 | `setPaneFocusPolicy({ paneFocusPolicy: "hover" })`, reload | Evaluation rejected — diagnostic names the invalid value and the two valid options (`click`/`cursor`); previous working configuration preserved |

## Fixed panels stay untouched

| # | Action | Expected |
|---|--------|----------|
| S18 | With side/top/bottom fixed panels visible (file browser etc.), run S1–S16 operations | Splits affect ONLY the working area: fixed panel sizes, positions, and content unchanged; dividers never overlap fixed panels |

## Keybinding override from init.js

init.js additions for these steps:

```js
import { bindKey } from "clay:keybindings";
bindKey("Ctrl+Shift+P", "shell.clientAddEqualPane", { scope: "global" });
```

| # | Action | Expected |
|---|--------|----------|
| S19 | Reload with the binding above, press `Ctrl+Shift+P` | Add-equal-pane runs from the user chord (bindings route through the same ClientUi path as the defaults) |
| S20 | `bindKey("Ctrl+X", "shell.clientNotARealCommand", { scope: "global" })`, reload | Rejected deny-by-default — diagnostic names the unknown command ID |

## Responsiveness (subjective)

| # | Action | Expected |
|---|--------|----------|
| S21 | Rapidly alternate `Ctrl+\` / `Ctrl+Alt+W` and `Ctrl+Shift+\` | Instant visual response, no visible stall, no torn frames; divider drag stays smooth during the session |
| S22 | 2 tabs open, tab A with a non-balanced divider drag (ratio ≠ 0.5) and a collapsed/fixed slot; quit and relaunch | Tab A's ratios + user-modified slot geometry survive the restart (persisted per tab in `layout.json` v2); tab B's layout untouched — persistence is per tab (module 14 T41/T47/T48) |

## Pane document views (Phase 22.2)

Setup for this section: keep the 2–4 panes from the sections above; open the
scratch workspace files via the workspace browser / fuzzy open or `Ctrl+O`.
Each pane hosts at most ONE open document; every pane that opened a file
shows its own status line with that document's name/dirty state.

| # | Action | Expected |
|---|--------|----------|
| D1 | With 2 panes, open `a.md` in the focused pane | Document view mounts in the FOCUSED pane (status line shows `a.md`); the other pane stays a placeholder; no connection-level disruption (chrome/panels unchanged) |
| D2 | Click the other pane, open `b.md` there | Both panes show their own documents with independent status lines; caret/selection/viewport in each pane are independent (scroll one pane — the other does not move) |
| D3 | Type in the `a.md` pane, then in the `b.md` pane | Keystrokes land ONLY in the pane whose editor has focus; per-pane dirty markers appear on the right status lines; both edits reach the server (save each, `cat` the files) |
| D4 | Open `c.rs` in a third pane while `a.md`/`b.md` stay open | Rust mode activates in the `c.rs` pane while markdown stays active in the `a.md` pane — concurrent major modes, no cross-pane bleed (mode keymaps/autocomplete triggers follow each pane's own document) |
| D5 | Focus routing: focus the `a.md` pane and press a mode-specific chord, then focus `b.md` and press it again | The chord is interpreted by the FOCUSED pane's document mode only (e.g. markdown- or rust-specific bindings fire in the pane that owns that document, never in both) |
| D6 | Duplicate open: from pane 2, open `a.md` (already open in pane 1) | NO second view — pane 1 is focused instead; `a.md` stays in pane 1 with its caret/content intact |
| D7 | Duplicate open from several panes (3rd and 4th panes, same file) | Always routes to the single owning pane; still no duplicate view |
| D8 | Open-documents switcher (`Ctrl+Shift+E` — `editor.clientShowOpenDocuments`) on the focused pane | Menu lists EVERY pane's open document (`pane N: <name>` entries with active/dirty markers) plus retained sessions; selecting a cross-pane entry switches the OWNING pane's document and focuses it; selecting an own-pane entry activates locally |
| D9 | Close pane, CLEAN document (`Ctrl+Alt+W` on a pane whose doc is saved) | Pane closes, tree merges; the document's lease is released (reopen it after — opens fresh, no stale caret/session) |
| D10 | Close pane, DIRTY document: type in a pane, then `Ctrl+Alt+W` | Close is BLOCKED — save-conflict menu appears on that pane; no topology change until resolved |
| D11 | From the D10 state, SAVE via the conflict menu, then `Ctrl+Alt+W` | Pane closes normally after the conflict resolves |
| D12 | From a dirty state again, CANCEL the conflict menu (`Esc`), then check the pane | Pane stays open; edits and dirty marker intact |
| D13 | Placeholder panes: 4 panes, one document open | The 3 document-less panes show the empty placeholder surface — no stale chrome, no ghost status; typing only affects the doc pane |
| D14 | Focus policy interplay: with `cursor` policy set, move the pointer into another pane | That pane's document view activates; status line/IME follow the newly active pane |
| D15 | Responsiveness: 4 panes, every pane with a document open, type rapidly in each in turn | No perceptible lag vs single-pane editing; status/decoration updates feel immediate |

## Phase 22.8 per-tab split/document verification

Deep references: `docs/development/file-open-save-reload-workflow.md` and
module 14 steps T63–T70. These checks distinguish one tab's pane/document
state from another tab's server-owned workspace state.

| # | Action | Expected |
|---|--------|----------|
| D16 | In one tab, split into 2 panes and open two different files from that tab's workspace, one per pane | Both documents open concurrently in that tab; each pane keeps its own caret, selection, viewport, mode, version, and dirty state |
| D17 | Edit and save the two documents from D16 in alternating panes | Edits and acknowledgements stay document-scoped; saving one pane does not clear the other pane's dirty marker or change its version |
| D18 | Open the first file from the second pane, then repeat from a third/fourth pane | Existing-document ownership wins: the owning pane focuses and no duplicate view/session is created; the four-pane cap remains bounded |
| D19 | Switch to another tab, then return to the tab from D16 | The original tab's split tree and both document sessions remain unchanged; the other tab's panes/documents are not mounted into it |

## Accessibility roles and announcements (Phase 22.6)

Phase 22.6 gives the window model an accessibility contract: numbered pane
labels, a `TabList`/`Tab` tree for the tab bar (module 14), and one polite
`Status` live-region node announcing pane/tab actions exactly once per user
action. Deep reference:
`docs/development/accessibility.md` (roles/names table, announcement
strings, sanitization budgets). Automated equivalents: the React shell
landmark/role/live-region assertions (`frontend/src/test/shell.test.tsx`)
and React Aria's focus/split-pane semantics — a screen reader is not
required for the tree shape, only for the human hearing check. Note: on the
current WebKitGTK stack static text inside live regions is not exposed via
AT-SPI accessible names or the Text interface, so announcements must be
verified with a screen reader, not an AT-SPI name dump.

| # | Action | Expected |
|---|--------|----------|
| S23 | 2 panes, `a.md` open in pane 1, pane 2 a placeholder; inspect the window's accessibility tree (AT inspector such as `accerciser`/`dogtail` where available, or the automated structural tests) | Shell group named "Clay working area shell. Active pane 1."; pane hosts expose `Pane` role with numbered names: `Pane 1 of 2: a.md` and `Empty pane 2 of 2` — sanitized basenames only (no absolute host paths, no document text, no control characters) |
| S24 | With a screen reader active, `Ctrl+\\` on a single pane (vertical split) and `Ctrl+-` (horizontal split) | Exactly ONE polite announcement per action: `Split pane vertically` / `Split pane horizontally`; pure focus moves (`Ctrl+Alt+Left`/`Right`) and repaints stay SILENT |
| S25 | With 2 panes, `Ctrl+Alt+W` (close); then `Ctrl+Alt+[` and `Ctrl+Alt+]` (move) | `Closed pane; 1 pane remains` once; `Moved pane forward` / `Moved pane backward` once per real change; the single-pane close no-op stays silent |
| S26 | Screen reader active; type, scroll, open the open-documents switcher (`Ctrl+Shift+E`), navigate it, dismiss it | No announcement spam — keystrokes, scrolling, and menu navigation never announce; only the pane/tab actions above do |
| S27 | Two consecutive identical actions (e.g. move forward twice in a row) | Known ceiling: an AT may skip an announcement identical to the previous one — documented, not a bug |
| S28 | Perf reference (advisory only): `cargo bench --bench window_baselines -- --sample-size 10 --warm-up-time 1 --measurement-time 2` | Pane-paint and tab-switch geometry numbers land linear in pane count (sub-microsecond on dev hardware); NO wall-clock pass/fail on shared runners — deterministic guards (linear chrome work, no tab-switch reserialization, 4-pane decoration aggregate ≤ 32768 B) are automated in `tests/editor_performance_invariants.rs` and `docs/development/performance.md` Phase 22.6 section |

## Split direction aliases (Phase 22.7)

Phase 22.7 added direction-named aliases that resolve to the canonical
split handlers: `shell.clientSplitPaneRight` = `SplitPaneVertical`
(side by side), `shell.clientSplitPaneDown` = `SplitPaneHorizontal`
(stacked). They are bindable command IDs with NO default chords; the
canonical `Ctrl+\` / `Ctrl+-` bindings are unchanged. Deep reference:
`docs/reference/clay-js-api/shell/client-split-pane-right.md` and
`client-split-pane-down.md`.

init.js additions for these steps:

```js
import { bindKey } from "clay:keybindings";
bindKey("Ctrl+Shift+Right", "shell.clientSplitPaneRight", { scope: "global" });
bindKey("Ctrl+Shift+Down", "shell.clientSplitPaneDown", { scope: "global" });
```

| # | Action | Expected |
|---|--------|----------|
| S29 | Reload with the bindings above; single pane focused; `Ctrl+Shift+Right` then `Ctrl+Shift+Down` | Two panes side by side, EQUAL widths (identical result to S1/`Ctrl+\`); then two panes stacked, EQUAL heights (identical result to S2/`Ctrl+-`) — the aliases resolve to the canonical handlers |
| S30 | After S29, press the canonical `Ctrl+\` and `Ctrl+-` | Canonical bindings unchanged — both still split side by side / stacked |
| S31 | Fresh launch WITHOUT the alias bindings (or with the lines commented), press `Ctrl+Shift+Right` | NO-OP — the aliases ship with no default chords; nothing binds, nothing splits, no diagnostic |
| S32 | Replace the string forms with the facade helpers: `import { clientSplitPaneRight, clientSplitPaneDown } from "clay:shell"; bindKey("Ctrl+Shift+Right", clientSplitPaneRight(), { scope: "global" }); bindKey("Ctrl+Shift+Down", clientSplitPaneDown(), { scope: "global" });`, reload | Same behavior as S29 — the helpers return the alias command IDs |

## Plan 087 completion-in-split steps

| # | Action | Expected |
|---|--------|----------|
| S33 | Open a document in pane 1, split vertically (`Ctrl+\`), trigger completion in pane 1 | Completion popup anchors to pane 1's caret and stays inside pane 1's rect; the split divider/pane 2 are unaffected |
| S34 | Move focus to pane 2 (click / `Ctrl+Alt+Arrow`), trigger completion there | Popup re-anchors to pane 2's caret; only the active pane's caret is used (`completion_anchor` comes from the active pane) |
| S35 | Close the last pane's document, then close the pane | The pane returns to the welcome entry state; splitting again from welcome yields a normal editable pane |

## Plan 088 responsive split/pane steps

| # | Action | Expected |
|---|--------|----------|
| S36 | Compare a narrow working area and a wide working area with the workspace browser visible | Browser/sidebar yields before the main editor becomes unusable; split ratios and pane hosts stay inside the working-area frame |
| S37 | Repeat S1–S18 with large UI typography | Tab/status/pane labels, dividers, focus ring, and hit targets remain in bounds; fixed slots do not cover editor content |
| S38 | Run the representative 2× logical-window layout checks | Pane hosts and tab bar use logical bounds; no physical-pixel overflow or duplicate scale compensation occurs |
| S39 | Split a document pane and trigger completion in each focused pane | Completion anchors to the active pane caret and is clipped inside that pane; inactive pane content/focus is unchanged |
| S40 | Inspect split/pane accessibility after focus, split, move, close, and placeholder transitions | Pane roles/names, active-pane state, focus ring, and one-per-action announcements stay synchronized; sanitized names contain no host paths |

## Plan 088 task 12 Linux execution record (2026-08-15)

| Checks | Result | Evidence |
|---|---|---|
| S36/S37 | PASS structural / NOT RUN visually | Responsive layout tests cover 320/900/1200 widths and 12/24/96 UI sizes; current host cannot resize/focus the Clay window for a live narrow/wide pass |
| S38 | PASS structural | `high_dpi_layout_uses_logical_window_bounds` passes; production visual 2× capture is unavailable because the review host window is fixed and targeted resize is disabled |
| S39 | UNRESOLVED live / PASS structural | Completion pane-anchor/clamp tests pass; interactive split/completion keyboard delivery remains blocked. Retained Plan 087 split evidence is comparison-only |
| S40 | PASS structural / partial live | Shell/pane AccessKit tests pass and current welcome tree is clean; live multi-pane focus/announcement re-run is blocked by window targeting and the host has no screen reader |

## Linux execution record (Plan 086 task 11, 2026-08-14)

- **PASS — S1/S3/S23/S24:** the real AT-SPI tree showed restored two-pane geometry and numbered pane labels; activating `Split Pane Vertical` through Control Center produced a third placeholder and `Split pane vertically` in the stable live announcement node. No malformed tree occurred.
- **PASS — S5:** clean `Ctrl+Alt+W` removed one pane, left the survivor filling the working area, kept client/server alive, and exposed `Closed pane; 1 pane remains` once.
- **FAIL/BLOCKER — D10/S5 dirty-close variant:** a dirty active pane close crashed the client in `accesskit_consumer` with `Focused ID #4 is not in the node list`; server survived. See `code-reviews/screenshots/2026-08-14-plan086-a11y/manual-dirty-pane-close-crash.log`. This needs a follow-up focus/a11y update fix before dirty-pane close can be called green.
- **PASS — security/labels:** pane names and announcements used sanitized basenames/action text; no absolute workspace path or document contents appeared in the pane/status labels. Isolated HOME/XDG roots were used.

## Linux execution record (Plan 087 task 11, 2026-08-15)

- **PASS — S35 (welcome return):** closing the last pane's document resets the pane to the Clay-owned welcome state (`close_pane` clears sessions, reapplies the default surface, sets `welcome_visible`); the welcome surface is also the state shown on fresh empty-tab launches (module 01 L12) and was verified live this session.
- **BLOCKED by host — S33/S34 (completion in split panes):** this session's portal keyboard delivery could not drive the multi-stroke/split chords reliably, so live split+completion was not re-run; the split surface itself passed in plan 086 task 11 (S1/S3/S23/S24 above) and completion-in-pane anchoring is covered by automated tests (`completion_menu_observation_uses_caret_bounded_geometry`, `completion_overlay_clamps_above_or_below_caret_inside_main_rect`). Not a false pass.

## Negative checks

- A 5th pane is never created (cap = 4; S4).
- The last pane is never closed (S6).
- Move at reading-order ends is a no-op (S12).
- Unknown shell command IDs are unbindable (S20); unknown focus-policy values
  reject with a diagnostic (S17).
- Split commands grant no filesystem/network/extension authority — they are
  client-UI command IDs only (see the per-command docs under
  `docs/reference/clay-js-api/shell/`); the Phase 22.7 aliases resolve to
  the canonical handlers and grant nothing more (S29–S32).
- A file open in one pane can never be opened a second time in another pane
  (D6–D7) — the one-view-per-document rule is enforced client-side after the
  server's canonical-path duplicate detection.
- Closing a dirty pane never loses edits (D10–D12): the close is blocked
  until the save-conflict menu resolves.
- Document opens/leaves flow only through the server's capability-gated
  open/close path (per-document leases); panes cannot be granted document
  authority by the client (see `file-open-save-reload-workflow.md`).

## Known ceilings (NOT bugs)

- **Placeholder panes**: panes show an empty placeholder surface until a
  document is opened in them (22.2 behavior; opening focuses the pane).
- **No per-pane tabs or chrome**: each pane shows at most one document with
  its own status line; tab strips / per-pane chrome arrive with Phase 22.3.
- **Splits are per tab (22.3)**: the working area belongs to the active tab;
  each tab carries its own split tree, pane focus policy, and per-pane
  documents. Module 14 covers the tab bar and cross-tab behavior.
- **Window-scoped chrome**: SDUI sidebars and package panels/overlays are
  connection-wide chrome, not per-pane; they do not move or duplicate when
  panes split (packages cannot contribute per-pane chrome yet).
- **Persistence scope**: pane trees, ratios, user-modified slots, and
  per-pane documents survive restart per tab through `layout.json` v2
  (module 14 S22/T41); unsaved edits and caret/viewport positions still do
  not (module 14 ceilings).
- **Global bindings need editor focus**: `Global`-context chords route through
  the focused pane's editor key path; with a placeholder pane active (no
  document) the chords don't fire. Click a document pane first.
- **No screen reader on the dev host**: announcement behavior is verified
  structurally (the `cargo test --lib accessibility` suite asserts the exact
  `TreeUpdate` labels and that focus moves/repaints do not re-announce);
  real-AT hearing (e.g. Orca) is the remaining human check and is a known
  ceiling on hosts without a screen reader.
- **Identical consecutive announcements may be skipped by an AT** (S27) —
  the label is replaced in place; upgrade path is a clear-then-set update if
  real-AT testing shows dropped announcements.
- **Tab is pane cycling while splits exist**: with more than one pane, `Tab` /
  `Shift+Tab` cycle pane focus instead of inserting indentation (Phase 20.3
  contract, unchanged).
- **`Ctrl+Shift+\` character**: whether the chord arrives as `\` or `|`
  depends on the platform/keyboard; rebind if yours differs.
- **Open-documents switcher follows pane focus**: `clientShowOpenDocuments`
  opens on the focused pane; cross-pane entries switch the owning pane instead
  of creating local duplicates (22.2 semantics).

## Plan 089 validation steps

| # | Action | Expected |
|---|--------|----------|
| S41 | Run `CLAY_LIVE_WINDOW_SMOKE=1 cargo test --test security live_atspi_smoke::live_multi_window_scale_smoke -- --ignored --exact --test-threads=1` on a Wayland host with AT-SPI prereqs | Two real Clay client processes launch; AT-SPI exposes two distinct frames (PID-separated); both frames have positive physical bounds with scale factors between 0.5 and 4.0 |
| S42 | Inspect the responsive narrow/wide captures (`code-reviews/screenshots/2026-08-14-plan089-platform-validation/visual-review/responsive/`) | Narrow (500 px) and wide (1200 px) captures show the welcome card, status bar, and pane hosts within bounds; the narrow welcome shortcut text adapts to card width |

## Plan 089 task 9 Linux execution record (2026-08-17)

| Checks | Result | Evidence |
|---|---|---|
| S36–S38 | PASS structural + partial live | Responsive layout tests pass; Plan 089 visual review captured narrow (500 px) and wide (1200 px) states with PASS artifacts showing the welcome card and status bar within bounds |
| S39 | PASS structural | Completion pane-anchor/clamp tests pass; live split+completion is covered by the completion capture (module 04 E22 Plan 089 record) |
| S40 | PASS structural + partial live | Shell/pane AccessKit tests pass; Plan 089 focus repair fix (`request_welcome_render`, focus-on-remove) ensures the welcome status and pane focus stay synchronized after connection events and pane removal |
| S41 | PASS live | `CLAY_LIVE_WINDOW_SMOKE=1` multi-window smoke test launched two real Clay clients; AT-SPI exposed two PID-separated frames with positive bounds and scale factors within 0.5–4.0 |
| S42 | PASS | `code-reviews/screenshots/2026-08-14-plan089-platform-validation/visual-review/responsive/` shows narrow and wide captures with the welcome card, status bar, and pane hosts within bounds |

## Phase 26 dirty-pane close fix and per-pane chrome steps

Deep references: `docs/development/accessibility.md` (focus/consumer tree),
`docs/reference/primitives/rendering-strategy.md` (chrome axis),
`docs/reference/packages/creating-packages.md` (editorRules.chrome).
Background: Plan 086 task 11 recorded a BLOCKER — closing a dirty active
pane crashed the client in `accesskit_consumer` (`Focused ID #4 is not in
the node list`; crash log
`code-reviews/screenshots/2026-08-14-plan086-a11y/manual-dirty-pane-close-crash.log`).
Phase 26.7 fixed the root cause (stashed-widget early return in the
accessibility pass + focus clamp + layout invalidation on document open).

| # | Action | Expected |
|---|--------|----------|
| S43 | Repeat the Plan 086 crash sequence: open a document in a pane, type to make it dirty, `Ctrl+Alt+W` | NO crash — the save-conflict menu appears on that pane; the client and server stay alive; the accessibility consumer tree keeps a live focus at every step (menu shown, `FileOperationFailed DirtyDocument`, discard, close) |
| S44 | From the S43 state, discard and close the pane | Pane closes; the survivor fills the working area; focus moves to a live node; no orphaned focus ID in the AT tree |
| S45 | 2 panes, code document in one, markdown in the other | Chrome follows each pane's document mode: gutter/active-line/indent guides/bracket match in the code pane, none in the prose pane; chrome is per-pane, never cross-pane bleed |
| S46 | Split a code pane and scroll the gutter side | Gutter digits stay right-aligned and clipped to the pane; the active-line wash tracks the caret line in the focused pane only |

Negative: closing a dirty pane never loses edits (D10–D12 unchanged); the
last pane is never closed (S6 unchanged); chrome grants no authority and is
not SDUI — packages contribute chrome only as inert manifest data.

## Phase 26 Linux execution record (2026-08-19)

| Checks | Result | Evidence |
|---|---|---|
| S43/S44 | PASS automated regression; live partial | `dirty_focused_pane_menu_and_discard_keep_consumer_focus_live` exercises the exact crash path (dirty pane → save-conflict menu via `apply_menu_sync` → `FileOperationFailed DirtyDocument` → discard → close) asserting the consumer focus stays live at every step; `dirty_pane_close_rejection_and_discarded_removal_keep_focus_consumer_safe` covers the rejection path. The Plan 086 crash log is superseded — the panic no longer reproduces in the automated suite. Live attempt (2026-08-19): a real Clay instance with an open document accepted typed input (doc v2, dirty) and stayed alive with an intact AT-SPI tree; the `Ctrl+Alt+W` chord itself is host-blocked (portal delivers single keys only — review-log V9), so the live menu path was not re-driven |
| S45/S46 | PASS live (single-pane) / structural (multi-pane) | `code-reviews/screenshots/2026-08-18-phase26-review/rust-*` (chrome on) vs `markdown-*` (chrome off) show per-mode chrome; per-pane chrome isolation is covered by the pane-scoped paint tests (`pane_paint_baselines`, per-pane decoration aggregate guard) |

## Plan 097 Phase 12 Tauri/React visual and accessibility review (2026-08-24)

| Check | Result | Evidence |
|---|---|---|
| Two-pane composition | PASS static visual/a11y | `code-reviews/screenshots/2026-08-24-tauri-react-parity/splits/fixture-*` shows editor and welcome pane within bounds at wide/narrow sizes |
| Real split/tab tree | PASS AT-SPI structure | `tabs-splits/accessibility.txt` exposes Pane 1 editor, separator, Pane 2 Empty tab and named actions |
| Split resize/focus keyboard flow | UNRESOLVED live; PASS structural | Host cannot safely target the Tauri window or deliver chords; split-tree and workspace-controller tests pass |
| Path-label safety | PASS | Split fixture now shows sanitized `ws` basename rather than `/tmp/ws`; editor regression test covers the root cause |
