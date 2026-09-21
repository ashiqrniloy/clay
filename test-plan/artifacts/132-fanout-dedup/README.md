# Plan 132 fanout/DTO-dedup manual-pass artifacts (2026-09-21)

Regression-only manual pass for plan 132: the five state lanes moved onto
`src/server/fanout.rs` (`StateFanout<T>` / store-free `Fanout<T>`), the
typography + runtime-generation lanes re-pointed at `Fanout`, and the Tauri
bridge DTO transcription deduplicated (ts-rs projections + `From`/`TryFrom`
destructures). **No user-visible behavior was supposed to change**, so this pass
re-ran the modules whose behavior the lanes carry — 04 (core editing), 07
(caret/typography/wrap), 10 (editor commands), 13 (pane focus), 15 (design
system/theme) — and recorded every divergence it found as a *pre-existing*
client gap with its code pointer.

Modules/steps and verdicts live in `test-plan/index.md` ("Plan 132 … execution
record"). Findings and ceilings are repeated here with the raw evidence.

## Harness (all paths under `live/`)

- `run-live.sh start caret|caret-invalid|wrap|editing|panes|panes-click|review
  [bytes] | stop` — isolated mode-700 root (`/tmp/clay-plan132-live`), private
  server socket, fixture workspace + `layout.json` (single pane, two panes, or
  an empty pane for SDUI fixtures), tree-kill teardown. `CLAY_LIVE_INIT=<path>`
  swaps the init.js of any mode (used for the wrap/caret A/B legs).
- `probe.py` — AT-SPI probe (copied from the plan 129 harness): `dump`,
  `editor` (chars + caret offset), `rows`, `act <role> <name>`, `focus`,
  `wait-ready`.
- `pane-focus.sh` — reports which pane's editor holds focus (pane 1 = left).
- `shot.sh` — grim capture of the Clay window at its **current** geometry;
  refuses when another window holds compositor focus.
- `portal-shot.py` — hardened copy (plan 130's version, AT-SPI-anchored crop).
  Not used: this host's portal screenshot path raises an interactive
  "Allow Apps to Take Screenshots?" prompt (seen and dismissed during the
  pass), so every capture here is grim.
- Eleven fixtures: `init-caret.js` (typography pin + hollow block caret),
  `init-caret-blink.js`, `init-caret-underline.js`, `init-invalid-caret.js`,
  `init-wrap-column.js` / `init-wrap-column40.js` / `init-wrap-none.js` /
  `init-wrap-invalid.js`, `init-editor-command.js`, `init-pane-focus.js`,
  `init-pane-focus-invalid.js`.

Host capabilities this pass **has** that plan 129's record listed as ceilings:
`wtype` works on this Hyprland 0.56 session (real keystrokes into the webview),
`hyprctl dispatch 'hl.dsp.focus({window="class:clay-desktop"})'` focuses the
window, `hl.dsp.cursor.move({x,y})` moves the pointer without `/dev/uinput`,
and `grim` captures the window directly. Input synthesis is therefore no longer
a blocker here; the remaining blockers are the client-side gaps below.

## Evidence

| Area | Files |
|---|---|
| Caret + typography (module 07 T9/T20/T23/T24, 04 E1/E8) | `live/caret/01-rest-hollow-block-ligatures.png` (joined ligatures `⇒ ≠ = →` under the 16 px pin), `live/caret/02-typed-ab.png` + `live/caret/03-caret-zoom-after-typing.png` (typed `AB`: chars 107→109, caret 0→2; `Ctrl+Z` back to 107/0) |
| Caret lane live delivery (module 07 T1/T5 family) | `live/caret/08-caret-override-after-reload-burst.png` + `live/caret/09-caret-after-reload-other-phase.png` (burst pair: the 8 px override paints), `live/caret/10-prechange-build-after-reload.png` (same frame from the stashed pre-change build — identical sha256 `67e737f3…`), `live/caret/reload-run1..3.png` (three reload runs) |
| Caret blink | `live/caret/caret-blink-1..8.png` (alternating frames = caret blinks), `live/caret/caret-solid-1..8.png` (config `blink: "solid"` also blinks — see finding 2) |
| Caret deny-by-default | `live/caret-invalid/server.log` (`clay server configuration failed [editor.invalid_set_cursor_style]`, editor alive, 35 chars) |
| Editor-command lane (module 10/04) | `live/editing/editor-command-server.log`; caret 0 **before** reload (init.js published before the client subscribed → advisory lane drops, as designed) and caret 2 **after** `Ctrl+Shift+R` (init.js re-ran, `agentProfile.register` 1→2) |
| Editor-layout lane (module 07 T22–T24) | `live/wrap/01-column-100.png`, `live/wrap/03-column40-after-reload.png` (40 ch centered column = `columnCap` honored), `live/wrap/02-none.png` + `live/wrap/03-none-after-reload.png` (see finding 3), `live/wrap/invalid-server.log` (`[editor.invalid_set_editor_layout]`) |
| Pane focus policy (module 13 S14/S17) | `live/panes/status-before.png`, `live/panes/status-after-reload-and-move.png`, `pane-focus.sh` output (see finding 4) |
| Design system / theme (module 15) | `../design-system/design-system-dark.png` (shipped `@clay/design-instrument` + gruvbox-material-dark active: the workspace chip renders in the theme's green instead of the default blue, shell styled throughout), `../design-system/dark/{accessibility.txt,runtime-tree.txt,metadata.txt}` from `scripts/capture-ui-review.sh` (its portal screenshot step is the blocked leg) |
| Automated companions | `live/automated-companions.txt` |

## Findings (all pre-existing client gaps, none introduced by plan 132)

1. **Initial-sync lane messages are dropped by the webview for the caret.**
   `frontend/src/editor/extensions/controller.ts:628` returns early when
   `this.view` is null, and the initial sync delivers the caret override before
   the editor view exists, so an override set from init.js never paints on a
   fresh connect (thin default bar). A `Ctrl+Shift+R` reload delivers the same
   value on the live channel and it paints. Reproduced identically on the
   stashed pre-change build, so this is not a plan-132 regression.
2. **`hollow` and `blink` are accepted and delivered but never consumed.**
   `hollow` occurs only in the generated `bridge.ts` DTO; nothing reads
   `blink`. Module 07's T2/T3/T5 wording ("solid = non-blinking", "hollow =
   outlined block") therefore describes behavior the client does not have.
3. **Unit wrap policies throw in the client's layout handler.**
   `controller.ts:661-672` does `"none" in wrap` / `"column" in wrap`, but
   `WrapPolicy::None`/`Viewport` arrive as the *strings* `"none"`/`"viewport"`
   (`#[serde(rename_all = "camelCase")]` unit variants), so `in` raises a
   `TypeError` and the override is dropped; `{"column": N}` works and was
   verified live (40 ch column after a reload). Module 07 T22's "no wrap for
   the rust fixture" also needs the mode manifest loaded (`@clay/rust`) — the
   fixture used here had no mode package, so its default role wrapped.
4. **The pane-focus preference never reaches the UI.** The Rust client forwards
   `ClientConnectionEvent::ShellPreferences` (`src/client/mod.rs:1337`), but no
   frontend module consumes `paneFocusPolicy` (grep over `frontend/src` finds
   only unrelated package-UI `focusPolicy` fields), so module 13 S14/S17 cannot
   be observed live; the lane's server side is covered by
   `set_pane_focus_policy_publishes_shell_preferences` and
   `shell_preferences_default_to_click_when_unset`.
5. **Capture methodology:** a caret's shape must be sampled as a *burst* — a
   single grim frame lands on either blink phase and reads as "override not
   applied". Three single-frame captures during this pass produced exactly that
   false negative before the burst settled it.

## Automated companions (fresh on the refactored tree)

```
cargo test --lib server::fanout          4 passed
cargo test --lib caret                   6 passed
cargo test --lib typography             17 passed
cargo test --lib editor_layout           3 passed
cargo test --lib shell_preferences       2 passed
cargo test --lib connection::           96 passed
cargo test -p clay-desktop --test dto_roundtrips   16 passed
frontend npx vitest run src/theme src/editor src/shell  173 passed
```
