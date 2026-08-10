# 13 — Window Splits

Equal-area window splits and per-pane document views: split/close/add-equal/
move/resize of panes in the working area, pane focus policies, one document
view per pane (Phase 22.2), the user-rebindable `clay.shell.client*`
command surface, and the Phase 22.7 direction-named split aliases. Deep
references:
`docs/reference/primitives/shell-layout-strategy.md` (Phase 22.1 + 22.2
sections), `docs/reference/clay-js-api/shell/` (command + configuration docs),
`docs/reference/clay-js-api/editor/client-show-open-documents.md`,
`docs/development/file-open-save-reload-workflow.md` (document lifecycle),
`examples/init.js` sections 7–8.

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

Phase 22.7 direction aliases (`clay.shell.clientSplitPaneRight` =
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
bindKey("Ctrl+Shift+P", "clay.shell.clientAddEqualPane", { scope: "global" });
```

| # | Action | Expected |
|---|--------|----------|
| S19 | Reload with the binding above, press `Ctrl+Shift+P` | Add-equal-pane runs from the user chord (bindings route through the same ClientUi path as the defaults) |
| S20 | `bindKey("Ctrl+X", "clay.shell.clientNotARealCommand", { scope: "global" })`, reload | Rejected deny-by-default — diagnostic names the unknown command ID |

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
| D8 | Open-documents switcher (`Ctrl+Shift+E` — `clay.editor.clientShowOpenDocuments`) on the focused pane | Menu lists EVERY pane's open document (`pane N: <name>` entries with active/dirty markers) plus retained sessions; selecting a cross-pane entry switches the OWNING pane's document and focuses it; selecting an own-pane entry activates locally |
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
strings, sanitization budgets). Automated equivalents: the shell's
structural a11y tests (`cargo test --lib accessibility` in
`src/masonry_shell.rs`) build the exact `TreeUpdate` and assert every role,
name, and announcement string below — a screen reader is not required for
the tree shape, only for the human hearing check.

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
split handlers: `clay.shell.clientSplitPaneRight` = `SplitPaneVertical`
(side by side), `clay.shell.clientSplitPaneDown` = `SplitPaneHorizontal`
(stacked). They are bindable command IDs with NO default chords; the
canonical `Ctrl+\` / `Ctrl+-` bindings are unchanged. Deep reference:
`docs/reference/clay-js-api/shell/client-split-pane-right.md` and
`client-split-pane-down.md`.

init.js additions for these steps:

```js
import { bindKey } from "clay:keybindings";
bindKey("Ctrl+Shift+Right", "clay.shell.clientSplitPaneRight", { scope: "global" });
bindKey("Ctrl+Shift+Down", "clay.shell.clientSplitPaneDown", { scope: "global" });
```

| # | Action | Expected |
|---|--------|----------|
| S29 | Reload with the bindings above; single pane focused; `Ctrl+Shift+Right` then `Ctrl+Shift+Down` | Two panes side by side, EQUAL widths (identical result to S1/`Ctrl+\`); then two panes stacked, EQUAL heights (identical result to S2/`Ctrl+-`) — the aliases resolve to the canonical handlers |
| S30 | After S29, press the canonical `Ctrl+\` and `Ctrl+-` | Canonical bindings unchanged — both still split side by side / stacked |
| S31 | Fresh launch WITHOUT the alias bindings (or with the lines commented), press `Ctrl+Shift+Right` | NO-OP — the aliases ship with no default chords; nothing binds, nothing splits, no diagnostic |
| S32 | Replace the string forms with the facade helpers: `import { clientSplitPaneRight, clientSplitPaneDown } from "clay:shell"; bindKey("Ctrl+Shift+Right", clientSplitPaneRight(), { scope: "global" }); bindKey("Ctrl+Shift+Down", clientSplitPaneDown(), { scope: "global" });`, reload | Same behavior as S29 — the helpers return the alias command IDs |

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
- **No topology/document persistence**: pane trees and per-pane document
  layout reset on restart; persistence arrives with Phase 22.5 (layout.json
  extension).
- **Persistence restored (22.5)**: pane trees, ratios, user-modified slots,
  and per-pane documents now survive restart per tab (`layout.json` v2,
  module 14 S22/T41); unsaved edits and caret/viewport positions still do
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
