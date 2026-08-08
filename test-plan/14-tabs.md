# 14 — Tabs (Independent Client Views)

Tabs as independent client views (Phase 22.3) with keyboard tab management
(Phase 22.4) and window-state persistence (Phase 22.5): one client
connection per tab, a server-authoritative in-memory tab registry, the tab
bar chrome, per-tab split trees and document views, dirty-guarded closing,
default key chords for next/prev/new/close/move/activate-by-number,
auto-reconnect, reclaim of registry entries on local restart, and
client-owned `layout.json` v2 persistence of tab order, workspaces, split
trees, and per-pane documents across full restarts. Deep references:
`docs/reference/primitives/shell-layout-strategy.md` (Phase 22.3 + 22.4 +
22.5 sections), `docs/wiki/modules/masonry-shell.md` (tab bar + lifecycle),
`docs/wiki/modules/tabs-and-clients.md` (registry + driver policies),
`docs/wiki/modules/multi-document-sessions.md` (reconnect restoration),
`docs/reference/clay-js-api/shell/client-tab-*.md` + `examples/init.js`
sections 7–8 (the tab command IDs/chords and per-active-tab pane
commands/focus policy), `docs/development/launch-and-gui-smoke.md`.

## Setup

```bash
mkdir -p /tmp/clay-manual /tmp/clay-manual-tab2
cd /tmp/clay-manual
echo "tab one alpha" > one.md
echo "# Two" > two.md
cd /tmp/clay-manual-tab2
echo "tab two beta" > other.md
echo "fn main() {}" > main.rs
```

- Launch: `cargo run` from the repository root; open `/tmp/clay-manual` as
  the initial workspace.
- Default tab UI:
  - Click a tab card to switch tabs.
  - `+` (right end of the tab bar) or `Ctrl+T` opens a new tab via the
    native folder picker.
  - `✕` on a tab card closes that tab (blocked while the tab has dirty
    documents); `Ctrl+Shift+W` closes the ACTIVE tab (same guard).
  - Keyboard tab management ships in Phase 22.4 — full chord table in the
    Keyboard section below (T25+).
- The tab bar sits below the top fixed panel and above the working area; it
  shows only when more than one tab exists.

## Tab bar and open-second-tab

| # | Action | Expected |
|---|--------|----------|
| T1 | Launch with a single workspace open (setup above) | NO tab bar — single-tab behavior identical to pre-22.3: the top panel + working area fill the window as before |
| T2 | Click `+`, pick `/tmp/clay-manual-tab2` in the folder dialog | A second tab card appears (name `clay-manual-tab2`), the tab bar appears below the top panel, and the new tab becomes active showing the empty editor with `clay-manual-tab2` as its workspace |
| T3 | Inspect the tab bar | Cards ordered oldest→newest: `clay-manual` then `clay-manual-tab2`; active card visibly distinct (token-driven fill/underline); `+` at the right end |
| T4 | Open `one.md` in the `clay-manual` tab, `other.md` in `clay-manual-tab2` | Each tab's editor shows its own document; status lines are per tab |
| T5 | Split the second tab into 2 panes (`Ctrl+\`), open `main.rs` in the second pane | The first tab's single-pane layout is untouched; each tab keeps its OWN split tree (switching back shows `one.md` in the first tab's single pane exactly as left) |
| T6 | Type in each tab in turn (switch via card click) | Keystrokes land ONLY in the active tab's focused pane; typing in one tab never affects the other's text, caret, viewport, history, or dirty markers — edit isolation per tab (verified end-to-end: save both, `cat` the files) |
| T7 | Close all but one tab (see T15–T18), then look for a close affordance | NO UI path to close the last tab — the tab bar is hidden at one tab, so there is no `✕` to click; the editor always keeps at least one tab (the client also refuses the close defensively) |

## Switch and retention

| # | Action | Expected |
|---|--------|----------|
| T8 | With 2 tabs open and different documents/layouts in each, click the inactive tab's card | Switch feels instant; the previously active tab's panes, documents, caret, viewport, history, and dirty state are retained exactly (return and verify) |
| T9 | Rapidly click back and forth between cards | No flicker, no re-mounting, no document re-download — widgets stay stable across switches (idle tabs are retained, not rebuilt) |

## Concurrent modes across tabs

| # | Action | Expected |
|---|--------|----------|
| T10 | Open `main.rs` in tab 2 while tab 1 holds `one.md` | Rust mode active in tab 2, markdown in tab 1 — concurrent major modes across tabs with no bleed (mode-specific chords fire only in the tab that owns the document) |
| T11 | Focus tab 1, press a markdown-mode chord; switch to tab 2, press a rust-mode chord | Each chord is interpreted by the ACTIVE tab's document mode only |

## Open/close target the active tab

| # | Action | Expected |
|---|--------|----------|
| T12 | With tab 1 active, open `two.md` via `Ctrl+O` / browser; then switch to tab 2 and open a file there | Each open lands in the ACTIVE tab's focused pane; the other tab's documents and panes are untouched |
| T13 | Duplicate open across tabs: with `two.md` open in tab 1, switch to tab 2 and open `two.md` | The file opens in tab 2 as its OWN view — the one-view-per-document rule is per tab (each tab is an independent client); both tabs may hold the same file independently |
| T14 | Dirty in tab 1, switch to tab 2, `Ctrl+Alt+W` on its pane | Close targets the active tab's pane tree only; tab 1's dirty state is preserved |

## Close flows

| # | Action | Expected |
|---|--------|----------|
| T15 | Close a CLEAN tab (`✕` on the card) | Tab closes; its connection is released (server registry entry removed — see negative checks); the tab bar reflows to the remaining cards; the remaining tab becomes active with its layout/documents intact |
| T16 | Close a DIRTY tab: type in a tab, then `✕` | Close is BLOCKED — the save-conflict menu appears on that tab's dirty document; no tab removal until resolved |
| T17 | From T16, SAVE via the conflict menu, then `✕` | Tab closes normally after the conflict resolves |
| T18 | From a dirty state again, CANCEL the conflict (`Esc`) | Tab stays open; edits and dirty marker intact |

## Reconnect and restart reclaim

| # | Action | Expected |
|---|--------|----------|
| T19 | With 2 tabs open and documents in both, kill the client process (e.g. kill the `clay` process) and `cargo run` again with the SAME server still running | Both tabs restore — same cards, same active tab, same workspaces, same open documents — because the in-memory server registry survives the client restart (entries are reclaimed by the fresh connections) |
| T20 | Connection drop: kill the server process and relaunch it, then interact | The tab whose connection dropped shows the disconnect diagnostic ("reconnects automatically"); after the server returns, the tab reconnects by itself and its documents re-open (workspace + documents restored on the new connection) |
| T21 | During reconnect, close another tab | The reconnecting tab's task is cancelled; no stale events from the old connection reach the shell |

## Responsiveness (subjective)

| # | Action | Expected |
|---|--------|----------|
| T22 | With 2 tabs open (one document in each), type rapidly in the active tab | Feels identical to single-tab editing — no lag, no visible stall |
| T23 | With 2 tabs open, click cards back and forth while typing | Tab switch feels instant; typing immediately after a switch lands in the new active tab |
| T24 | 3+ tabs open (open a third workspace), type in each in turn | Still no perceptible lag; inactive tabs cost no visible render work |

## Keyboard tab management (Phase 22.4)

All chords ship by default with `Global` scope (same context the 22.1 pane
chords use); override them with `bindKey` in `init.js` (module 10,
K15–K18). Policies: numbering follows the card order (registry order,
entry-less mounted tabs appended); next/prev wrap at both ends; moves never
wrap (boundary = silent no-op); numbered families are 1-based and capped at
9 by design; numbered activation/move-to beyond the tab count are silent
no-ops. Deep reference: `docs/reference/primitives/shell-layout-strategy.md`
Phase 22.4 section and the `docs/reference/clay-js-api/shell/`
`client-tab-*.md` docs.

| # | Action | Expected |
|---|--------|----------|
| T25 | 2 tabs open (card order A B, A active); `Ctrl+Tab` repeatedly | Active tab advances one card per press in card order (A → B); from the LAST tab it WRAPS to the FIRST (B → A); no flicker, switch feels instant |
| T26 | `Ctrl+Shift+Tab` repeatedly | Active tab steps back one card per press (B → A); from the FIRST tab it WRAPS to the LAST (A → B) |
| T27 | 2 tabs open; `Ctrl+1`, `Ctrl+2`, `Ctrl+3` | `Ctrl+1` activates the first card, `Ctrl+2` the second (1-based, card order); `Ctrl+3` is a SILENT no-op (beyond tab count) |
| T28 | One tab open; `Ctrl+1`, `Ctrl+Tab`, `Ctrl+Shift+Tab` | All silent no-ops — next/prev need two tabs, numbered activation has no second position; nothing flickers, active tab unchanged |
| T29 | `Ctrl+T` with 2 tabs open | Same flow as the `+` affordance: native folder picker opens; picking a folder mounts a new tab (becomes active) with the new workspace; a second `Ctrl+T` while the picker is open is ignored |
| T30 | Clean tab active; `Ctrl+Shift+W` | The active tab closes (connection released, registry entry removed, bar reflows); the remaining tab becomes active with layout/documents intact — same contract as `✕` (T15) |
| T31 | Single tab open; `Ctrl+Shift+W` | NO-OP — the last tab is protected from the keyboard too (same contract as T7; the client also refuses defensively) |
| T32 | Type in a tab (dirty); `Ctrl+Shift+W` | Close is BLOCKED — the tab-confirm menu appears listing the dirty document(s) by name with three items: "Save all and close", "Discard and close", "Cancel" |
| T33 | From T32 choose "Save all and close" | Every listed document saves (disk-verify with `cat`); after the last save ack the tab closes and the bar reflows — no edits lost |
| T34 | From a dirty state again choose "Discard and close" | The tab closes and the unsaved edits are dropped — ONLY via this explicit menu item (no silent loss) |
| T35 | From a dirty state again choose "Cancel" (or `Esc`) | The tab stays; edits and dirty marker intact; menu dismissed |
| T36 | 3 tabs open (order A B C, A active); `Ctrl+Shift+]` twice | First press moves A right → B A C (A stays active); second press moves A right again → B C A; a further press at the LAST position is a silent no-op; moves NEVER wrap; active-tab status survives (switch away and back, A still active) |
| T37 | 3 tabs open (order A B C, A active); `Ctrl+Shift+[` | NO-OP — A is already first (boundary); with B active, `Ctrl+Shift+[` moves B left → B A C; at the first position moves are silent no-ops |
| T38 | 3 tabs open (order A B C, C active); `Ctrl+Shift+1` | C moves to position 1 → C A B (C stays active); `Ctrl+Shift+2` on the result moves C to position 2 → A C B; `Ctrl+Shift+4` (beyond count) is a silent no-op; numbered moves are 1-based, capped at 9 |
| T39 | Move a tab (T36–T38), then `Ctrl+<N>` at its new position | Numbered activation follows the NEW card order — the registry is authoritative (switch-then-activate round trip stays consistent) |
| T40 | With 3 tabs open, run `Ctrl+Tab`, `Ctrl+1`, `Ctrl+Shift+]`, `Ctrl+Shift+2` back-to-back | Every chord lands immediately with no lag — switch = one layout pass; move/close reflow is immediate (subjective responsiveness check) |

## Window-state persistence (Phase 22.5)

Client-owned `layout.json` v2 (in `~/.config/clay/` or `$XDG_CONFIG_HOME`)
persists tab order, the active tab, each tab's workspace root + split tree,
and each pane's open document; a full quit/relaunch (client AND server)
restores the window. Deep reference:
`docs/reference/primitives/shell-layout-strategy.md` Phase 22.5 section;
`docs/wiki/modules/tabs-and-clients.md` Phase 22.5 section.

| # | Action | Expected |
|---|--------|----------|
| T41 | Build a 3-tab window: tabs A/B/C in that order, each its own workspace; split tab A into 2 panes (`Ctrl+\`) with a different document in each; tab C active with `Ctrl+Shift+2` used first so B is active last (or click C last); quit the window (close the GUI) and relaunch `cargo run` (fresh client AND server) | The window restores: same 3 cards in the SAME order (A B C), same workspaces (card labels), tab A's split tree shape + ratios intact with its two documents in the SAME panes, C's single-pane document intact, and the last-active tab + its active pane focused (tabs mount sequentially — brief flip-through as they connect, settling on the persisted active tab) |
| T42 | Single-tab window: one workspace, one document open; quit and relaunch | Single-tab behavior unchanged: the editor opens with the same document as before; no tab bar; pre-22.5 bootstrap identical apart from the document reopen |
| T43 | Replace `layout.json` with a legacy v1 file (pre-22.5 shape: `splits`/`slots` keys, no `version`) and launch | Legacy file still restores ratios/slots on the single bootstrap tab exactly as Phase 20.3 (v1 path preserved; a v2 file is silently ignored by the v1 apply) |
| T44 | Corrupt `layout.json` (truncate it, or write `{ not json`), then launch | Clean default launch — single tab, default layout, no crash, no hang (parse failure falls back to defaults; see also the hostile-file negative check below) |
| T45 | Delete one persisted workspace directory (keep its entry in `layout.json`), then launch | That tab is SKIPPED with a `clay.tabs.open_failed` diagnostic on the chrome; every other tab restores normally (missing root degrades to fewer tabs, never a stall) |
| T46 | Type unsaved text in a tab, quit and relaunch | The unsaved edits are GONE (documented expectation: restore persists open documents, not unsaved buffers) — the restored document shows the last saved content |
| T47 | 2 split tabs (A: 2 panes, B: 2 panes); run pane chords in A only (`Ctrl+\` once more, `Ctrl+Alt+W` on a pane, divider drag, `Ctrl+Alt+Shift+Left` resize), then switch to B | B's tree, ratios, and documents are byte-identical to before — pane/split operations are scoped to the ACTIVE tab; repeat in B and verify A untouched (composition contract) |
| T48 | Move tabs (T36–T38) into a new order, quit and relaunch | The MOVED order persists (registry order is persisted), and each tab's internal split tree + documents survived the move + restart intact |
| T49 | Restore a 3-tab / 4-pane window and watch startup | Reaches interactive state promptly — tabs connect sequentially with no visible stall beyond normal startup (subjective check; restore is startup-only, off the edit hot path) |

## Accessibility and cross-tab isolation (Phase 22.6)

Phase 22.6 tab-surface contract: the shell exposes a `TabList` (`Workspace
tabs`) with one `Tab` per card — sanitized workspace basename, `selected`
on the active card — only when two or more cards exist; announcements fire
on tab activate/create/close. Cross-tab authority is per-connection: the
registry binds identity and grants nothing, so document leases never cross
tabs (automated regression coverage: `src/server/tab_registry.rs`,
`src/server/connection.rs`, `src/packages/approvals.rs` tests from plan 077
task 6). Deep reference: `docs/development/accessibility.md` and
`docs/wiki/modules/tabs-and-clients.md`.

| # | Action | Expected |
|---|--------|----------|
| T50 | 2 tabs (workspaces `alpha`/`beta`); inspect the accessibility tree | `TabList` named `Workspace tabs` with one `Tab` per card in card order, names = sanitized workspace basenames; the ACTIVE card carries `selected`; pane hosts of the INACTIVE tab are absent from the tree (its panes are never announced or visited) |
| T51 | With a screen reader active, `Ctrl+Tab` a real switch (2 tabs) | `Switched to tab {position}: {name}` announced ONCE, politely; `selected` moves to the new card; typing right after lands in the new active tab |
| T52 | `Ctrl+T` (or `+` card) to mount a new tab, then `Ctrl+Shift+W` to close it | `Opened tab {position}: {name}` on mount; `Closed tab {position}: {name}; {n} tabs open` on close — each exactly once; the dirty-close confirm flow (T32–T35) stays silent until the menu resolves |
| T53 | Single tab: inspect the tree, then attempt `Ctrl+Tab` | NO `TabList` node at one tab (tree matches the pre-22.6 shape — no extra noise); the switch is a silent no-op with NO announcement |
| T54 | Grant isolation sanity: tab A opens `secret.md`; in tab B (different workspace) list open documents and try to open `secret.md` | Tab B sees only its own workspace's documents; opening a path outside B's workspace root is rejected server-side (`OutsideRoot`); tab A keeps its lease — no cross-tab document surface exists |
| T55 | Close tab A (connection released), then recreate a tab on the same workspace and reopen `secret.md` | Fresh connection, fresh grants: the document opens through the ordinary open path; nothing from the old connection survives (no stale leases, no ghost docs, no inherited access) |
| T56 | 2–3 tabs, each with a 4-pane split tree; rapid `Ctrl+Tab` + pane chords + typing | Switch feels instant (one layout pass, off the edit hot path); pane/decoration work stays bounded — advisory `window_baselines` bench numbers in `docs/development/performance.md`; deterministic guards automated |

## Negative checks

- The last tab can never be closed (T7) — the editor always keeps one tab.
- A dirty tab can never be closed silently (T16–T18, T32–T35): the close is
  blocked until the save-conflict/tab-confirm menu resolves; no edits are
  lost.
- Closing a tab releases its server connection: the registry snapshot no
  longer lists the closed tab, and the per-connection document leases are
  cleaned up server-side (see module 01 connection steps for the observer
  view).
- Events for a closed tab's connection never reach the shell: after T15,
  the closed tab's chrome is gone — no stale diagnostics, no ghost panes.
- Tab open/close/switch grant no filesystem/network/extension authority —
  they are server-internal `TabCommand` messages over the existing
  per-connection capability/lease path; the server registry only binds
  already-authorized connections (reclaim rebinds a surviving registry
  entry to a fresh handshake-authenticated connection only).
- Cross-tab document isolation is per-connection: a tab's leases never
  appear in another tab's view, out-of-root opens are rejected, and a
  closed tab's grants die with its connection (T54–T55) — the registry
  binds identity and grants nothing.
- Opening a 65th connection (tab) while the server is at its 64-connection
  cap is refused (connection limit = `MAX_ACTIVE_CONNECTIONS` 64); the new
  tab fails cleanly with a diagnostic instead of destabilizing existing
  tabs.
- Numbered activation/move-to beyond the tab count are silent no-ops (T27,
  T38); numbered IDs beyond 9 do not exist as command IDs and are rejected
  deny-by-default at bind time (module 10, K17).
- Boundary moves are silent no-ops — the card order never wraps (T36–T37).
- Dirty-close "Cancel" preserves the tab (T35); "Discard and close" drops
  edits only via that explicit item (T34).
- The last tab is protected from the keyboard too: `Ctrl+Shift+W` on a
  single tab is a no-op (T31), matching the missing `✕` at one tab (T7).
- A hostile `layout.json` cannot grant authority or crash launch: v2 parses
  are bounded and panic-free (tabs capped at 64, panes ≤ 4 per tab, node
  count ≤ 64, ratios finite in 0.05..=0.95, non-zero unique pane ids); a
  structurally invalid `splitTree` degrades that tab to a single pane;
  out-of-root document paths are rejected by the server at reopen (T44's
  corrupt file, T45's missing root — both degrade cleanly).
- Restore reuses the existing per-connection validation: every restored tab
  rides the handshake + `TabCommand::New` `add_root` checks, and every
  restored document rides the plain `OpenDocument` path — persistence grants
  no filesystem/network/extension authority.

## Known ceilings (NOT bugs)

- **Numbered switch capped at 9 by design**: `clientTabActivate.<N>` and
  `clientTabMoveTo.<N>` exist for N in 1..=9 only — reach tab 10+ with
  `Ctrl+Tab`/`Ctrl+Shift+Tab` or a card click (T27, T38).
- **Tab moves never wrap**: `Ctrl+Shift+[`/`]` and `Ctrl+Shift+<N>` are
  boundary/beyond-count no-ops by design — wraparound would fight the
  explicit card order (T36–T38).
- **No drag-to-reorder**: card order changes only via the 22.4 move chords;
  drag reordering is not implemented.
- **No tab persistence to disk**: tab structure and per-tab split trees live
  in the in-memory server registry only — a full server restart (not just
  the client) resets them to the single initial workspace. Disk persistence
  for the registry and split trees arrives with Phase 22.5.
- **Restart drops unsaved state (22.5)**: the persisted window state is tab
  order, active tab, per-tab workspace + split tree, and per-pane open
  documents only — unsaved edits, caret/viewport/scroll positions, and
  per-tab pane-focus-policy runtime changes are NOT restored (T46); the
  `setPaneFocusPolicy` config API stays the policy source.
- **No quit-time dirty confirm**: closing the window with unsaved edits
  closes without asking — the edits are lost (save before quitting; the
  per-tab close confirm menu (T32) only guards closing a TAB).
- **No per-card tab focus**: tab cards are informational a11y nodes, not
  focusable widgets — Phase 22.6 delivered roles/names/announcements;
  per-card widget focus for AT focus handling remains deferred (Further
  Actions). Keyboard tab switching stays on the 22.4 tab commands.
- **No per-tab package chrome**: packages cannot contribute tab or tab-bar
  chrome; their panels/overlays stay connection-wide (later phase).
- **No multi-client tab reclamation**: the registry reclaims entries only
  for local client restarts; concurrent external clients reclaiming a tab
  entry is deferred to Phase 21 semantics.
- **Single-tab match-today**: with one tab the tab bar is hidden and the
  window matches pre-22.3 behavior exactly (T1) — by design, not a bug.
- **No screen reader on the dev host**: announcement behavior is verified
  structurally (the `cargo test --lib accessibility` suite asserts the exact
  announcement strings and silence on no-ops/focus moves); real-AT hearing
  (e.g. Orca) is the remaining human check and is a known ceiling on hosts
  without a screen reader.
- **Identical consecutive announcements may be skipped by an AT** (T51
  repeated twice in a row) — documented, not a bug.
- **Per-tab pane cap still 4**: each tab's split tree caps at
  `MAX_PANES_PER_TAB = 4` (module 13, S4) — the cap is per tab.
