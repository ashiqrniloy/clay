# 04 — Core Editing

Typing, history, clipboard, newline/indent rules, IME. This is the baseline
module: if anything here breaks, stop and report before running other modules.

## Setup

```bash
mkdir -p /tmp/clay-manual && cd /tmp/clay-manual
printf 'line one\nline two\n' > plain.txt
```

```rust
// /tmp/clay-manual/indent.rs
fn main() {
    let x = 1;
    if x > 0 {
        // caret-here line for indent checks
    }
}
```

Open the workspace, open `plain.txt` then `indent.rs`.

## Typing and history

| # | Action | Expected |
|---|--------|----------|
| E1 | Type characters, spaces, emoji (paste `émoji 🎉`) | Correct insertion at caret; grapheme clusters intact |
| E2 | `Backspace` / `Delete` | Character-granular removal at caret |
| E3 | `Enter` on an indented line in `indent.rs` | New line inherits leading whitespace (indent rule) |
| E4 | `Enter` inside `// comment` line | Comment continuation (`//`) inserted if mode declares it; plain.txt does not |
| E5 | Type `}` after an indented block line | Electric outdent snaps the brace to the block's indent (code modes) |
| E6 | Type `(` then `)` | Pair handling per mode manifest (auto-close / skip-over as configured) |
| E7 | `Tab` | Spaces per mode `tabSpaces` (rust package: 4; markdown: 2) |
| E8 | `Ctrl+Z` / `Ctrl+Shift+Z` (or `Ctrl+Y`) | Undo/redo restores text AND caret position; redo chain survives new edits only as documented |
| E9 | Undo to document start, retype | History branches correctly (no ghost text) |

## Clipboard

| # | Action | Expected |
|---|--------|----------|
| E10 | Select word, `clientCopySelection` binding (bind if not bound), paste elsewhere | Clipboard round-trip exact |
| E11 | Cut selection | Selection removed to clipboard |
| E12 | Paste multi-line text | Line endings normalized consistently |

## IME (only if an IME is installed; skip otherwise)

| # | Action | Expected |
|---|--------|----------|
| E13 | Begin IME composition (e.g. Japanese/German dead keys) | Preedit text overlays with underline; caret renders in active shape |
| E14 | Commit | Composed text replaces preedit exactly once; no duplication |
| E15 | Cancel composition (`Escape`) | Preedit removed, buffer untouched |

## Completion projection (Plan 087)

| # | Action | Expected |
|---|--------|----------|
| E16 | In a markdown/rust document with the fixture binding, press `Ctrl+Space` | A modeless completion popup opens next to the caret: width ≤ 480 logical px, at most 8 visible rows, item rows labeled from the provider; `Recovery: Completion` appears in the entry/status; the editor keeps focus (no Dialog role, no modal trap) |
| E17 | ArrowDown/ArrowUp in the popup | Selected row moves; the virtual MenuItem for the selected row carries the `selected` state; scroll keeps the selected row visible for long lists |
| E18 | `Escape` | Popup closes; editor/status return to normal; no text is inserted or deleted |
| E19 | Type text with no completions (e.g. `zzzz`), then `Ctrl+Space` | Provider returns `Empty`: popup is dismissed, NO blocking `No completions` panel appears, no status diagnostic; typing continues normally |
| E20 | Negative: type text, open completion, then trigger an edit/version change before accepting | Stale completion cannot apply: results for a stale document/version/behavior are consumed and dismissed; no text mutation from a stale accept (automated: `completion_result_rejects_foreign_document_and_behavior_provenance`) |
| E21 | IME (only if an IME is installed): compose while the completion popup is open | IME preedit and the completion popup coexist; committing the preedit dismisses/refreshes completion as documented — completion never blocks IME commit |

## Plan 088 completion containment steps

| # | Action | Expected |
|---|--------|----------|
| E22 | Trigger completion with a long result set (up to 256 items) in a split and inspect the popup boundary | Popup stays caret-adjacent, width ≤480 logical px, visible rows ≤8, and retained rows do not paint or announce below the shell; selection/scroll stays inside the popup |
| E23 | Trigger provider loading, empty, timeout, and error/recovery outcomes | Loading/error/recovery is observable through bounded status/diagnostic semantics; empty/stale results dismiss without a blocking panel or editor mutation |
| E24 | Open completion at a narrow pane and with large user UI typography | Anchor clamps inside the active pane/editor bounds; the popup does not cover the other pane or escape the window |

## Negative checks

- Editing a read-only observer session must not mutate the document.
- Rapid typing during `Pending edits > 0` stays responsive (local-optimistic).

## Linux execution record (Plan 087 task 11, 2026-08-15)

- **PASS — E16:** `Ctrl+Space` (fixture `completion.trigger` binding, `UiReactivePriority` from the installed markdown manifest) opened a modeless `Completion` popup in the live X11 instance: Menu `480x340` logical px at the caret area, 16 `@clay/markdown` items, ≤ 8 visible ListItem rows, virtual MenuItems `Completion # … Completion \`` with the first row `selected`, entry/status `— Recovery: Completion`, editor Entry still `focused` (no modal trap).
- **PASS — E18:** `Escape` dismissed the popup; Menu node gone, status back to `Clay — Connected — Editable — review.md — doc 3 — v1`, no text inserted/deleted.
- **PASS — E19:** typing `zzzz` then `Ctrl+Space` produced `CompletionResult status: Empty`; no popup, no blocking `No completions` panel, no status diagnostic, typing continued (doc v5).
- **Coverage note:** E17 (selection/scroll), E20 (stale accept), E21 (IME coexist) were not re-run live this session; they are covered by automated consumer/unit tests (`menu_selection_keeps_selected_row_in_scroll_viewport`, `completion_result_rejects_foreign_document_and_behavior_provenance`) and E21 is IME-installation dependent. Live completion capture artifacts from plan 087 task 7 (`code-reviews/screenshots/2026-08-14-plan087-ui-foundation/completion/`) remain valid for the same build.

## Plan 088 task 12 Linux execution record (2026-08-15)

| Checks | Result | Evidence |
|---|---|---|
| E22 | UNRESOLVED — live interactive input blocked | Retained Plan 087 completion artifacts show the modeless menu and caps, but current Task 8 interactive completion ended `UNRESOLVED`; the renderer-level P1-087-UI-1 containment fix has structural clipping/a11y tests but no current interactive visual pass |
| E23 | PASS for empty/error/recovery evidence; loading UNRESOLVED | `code-reviews/screenshots/2026-08-14-plan088-modernization/error/` and `recovery/` expose diagnostics/status; `loading/` captured welcome instead of the intended loading tree |
| E24 | PASS structural / NOT RUN visually | `completion_overlay_clamps_above_or_below_caret_inside_main_rect` and related layout tests pass; no safe resize/focus backend exists for a live narrow-pane run |

## Known ceilings

- IME preedit caret blink/shape parity is visual-only; blink phase timing is
  discrete (no alpha ramp) — see module 07 ceilings.

## Plan 089 task 9 Linux execution record (2026-08-17)

| Checks | Result | Evidence |
|---|---|---|
| E22 | PASS live | `code-reviews/screenshots/2026-08-14-plan089-platform-validation/visual-review/completion/` shows the completion popup as a bounded `Role::Menu` with 44 children (22 ListItems + 22 virtual MenuItems), `as` selected, no rows exceeding the 480×340 visible surface; P1-087-UI-1 containment is now visually verified |
| E23 | PASS | `error/` and `recovery/` captures expose diagnostics/status in accessible names; `loading/` delivers the published SDUI tree via RuntimeStateSnapshot |
| E24 | PASS structural | Completion anchor/clamp tests pass; live narrow-pane capture is covered by the responsive review (module 13 S36–S40 Plan 089 record)

## Phase 26 decoration background and typing-feel steps

Deep references: `docs/reference/primitives/rendering-strategy.md` (Phase 26
background axis), `docs/reference/primitives/syntax-vocabulary.md` (theme
axes). Setup: open the markdown fixture (`tests/fixtures/syntax/markdown.md`)
and a code fixture.

| # | Action | Expected |
|---|--------|----------|
| E25 | Search-match state (if reachable via the search surface) | Search-match spans show a background fill; at overlap with a quote/code-block background the search-match fill wins; the fill sits between selection rects and glyphs |
| E26 | Type rapidly inside a fenced code block and inside a heading in markdown | Keystroke-to-paint feels immediate; the background fill and heading scale follow the edit with no flicker; no full-file re-layout per keystroke |
| E27 | Select text across a background-painted span (quote/code block) | The selection rect paints OVER the decoration background (selection wins); the caret stays visible on the background |

Negative: decoration backgrounds never cover the caret or selection; no
background fill is painted over glyphs; typing never routes through
JavaScript or server IPC (local-optimistic).

## Phase 26 Linux execution record (2026-08-19)

| Checks | Result | Evidence |
|---|---|---|
| E25 | PASS automated / NOT RUN live | `search_match_and_quote_backgrounds_join_style_runs` (SearchMatch overrides Syntax Quote at overlap), `style_run_backgrounds_paint_before_glyphs`; live search surface not reachable this session |
| E26 | PASS automated / NOT RUN live | Incremental parse continuity tests (`plan057_*`, `plan058_*`) and the 16 ms keypress-to-paint envelope guards (`tests/editor_performance_invariants.rs`) cover edit-following and hot-path bounds; live typing is host-blocked (review-log V9) |
| E27 | PASS automated | Paint-order tests assert background fills are drawn between selection rects and `render_text` (`style_run_backgrounds_paint_before_glyphs`); live selection-over-background capture is host-blocked |

## Phase 28 editor command transforms and completion ranking

Deep references: `docs/reference/clay-js-api/editor/toggle-comment.md`,
`docs/reference/clay-js-api/editor/toggle-list-marker.md`,
`docs/reference/clay-js-api/editor/rotate-heading.md`, and
`docs/reference/clay-js-api/editor/toggle-inlay-hints.md`.
Setup: load `@clay/markdown` and `@clay/rust`; bind the argless commands in
module 10 when they have no default chord.

| # | Action | Expected |
|---|---|---|
| E28 | In Rust/TypeScript/JavaScript, place the caret on an indented line and press the default `Ctrl+/` | `//` is added after indentation; pressing again strips it. A multi-line selection adds the prefix to all touched lines, and a mixed selection uses strip-all/add-all semantics; no block-comment wrapper appears |
| E29 | In a Markdown list item, run `editor.toggleListMarker` (or the package alias `markdown.toggleList`) | The first configured marker toggles on/off; ordered-dot uses `1. ` / the next ordered marker as declared; empty-item behavior remains the manifest's `exitOnEmptyItem` policy |
| E30 | On a Markdown heading or plain line, run `editor.rotateHeading` (or `markdown.insertHeading`) repeatedly | ATX prefixes cycle through the manifest's `headingPrefixes`; after the last level the line returns to unheaded text; selection/caret remains usable |
| E31 | Repeat E28–E30 with two carets or a selection spanning several lines | Each touched line changes once, right-to-left history/selection remapping stays coherent, and an untouched line is not duplicated or skipped |
| E32 | On plain text with no comment/list/heading rule, invoke the corresponding transform | No text mutation; the command reports a bounded no-op diagnostic rather than an Accepted no-op |
| E33 | Open completion with a prefix that has exact, case-insensitive, short, and previously accepted candidates | Exact prefix ranks first, then case-insensitive prefix, then shorter labels, then bounded recency; ties are deterministic by label/insert text. The result remains capped and accepts without stale-version mutation |
| E34 | Query the focused editor through AT-SPI/AccessKit | The editor is an `Entry`/multiline input with `EditableText` + `Text` interfaces, bounded text value, caret/selection metadata, and a stable accessible name; package panels are not the edit target |
| E35 | With a screen-reader/AT-SPI keyboard path, insert text, select a range, then undo | Text mutation, selection, caret, and undo remain local/optimistic and are reflected in the accessibility tree without a full-document or package-runtime round trip |
| E36 | Repeat E34–E35 on a read-only observer or inactive/hidden pane | No editable-text mutation interface/action reaches read-only, inactive, hidden, or package-owned UI |

## Phase 28 Linux execution record (2026-08-20)

| Checks | Result | Evidence |
|---|---|---|
| E28–E32 | UNRESOLVED live; PASS structural | The editor Entry reported `supports_editable_text=false`, so live keyboard mutation was not claimed. `toggle_comment*`, `toggle_list_marker_toggles_dash_and_ordered_dot`, and `rotate_heading_cycles_atx_levels` passed; evidence and blocker: `code-reviews/screenshots/2026-08-20-phase28-manual/manual-test-plan.md`. |
| E33 | PARTIAL live; PASS automated | Completion popup captured under `code-reviews/screenshots/2026-08-20-phase28-primitives/completion/`; the `hel` prefix returned no bundled match, so visual ordering was not verified. `score_prefers_*` and `ranking_scan_stops_at_item_and_payload_caps` passed. |

## Phase 28.7 P1 editable-text accessibility execution record (2026-08-21)

Fresh Linux/GNOME Wayland review used `npx ui-skills start` with the
`accessibility` category and `jakubkrehel/better-accessibility`, then
`computer-use-linux get_app_state`, `doctor`, and targeted AT-SPI inspection
against the isolated `ui-review-rust` fixture. The current build exposes the
editor as `Entry` with `EditableText,Text` interfaces, `editable` + `multi-line`
state, bounded text content, and caret metadata. Evidence:
`code-reviews/screenshots/2026-08-20-phase28.7-followups/editor-editable-text/`.

| Checks | Result | Evidence |
|---|---|---|
| E34 | PASS live | `editable-text.txt` records `supports_editable_text=true`, `Accessible,Component,EditableText,Text`, `character_count=94`, and focused multiline states; `accessibility.txt` records the Clay Entry/status tree; screenshot captures the active editor. |
| E35 | UNRESOLVED keyboard live; PASS structural + AT-SPI set-value/selection path | `masonry_editor::tests::editor_accessibility_exposes_editable_text_value_selection_and_stable_run` covers value, text run, stable ID, selection action, replacement, and undo. AT-SPI `SetTextContents` reached document v2; selection action returned success. `computer-use-linux doctor` reports no keyboard backend (`uinput` denied, no xdotool/ydotool, Wayland portal input unavailable), so physical keyboard insertion/undo is not falsely claimed. |
| E36 | PASS structural | Read-only mutation actions are omitted and existing inactive/hidden-pane stashing plus package-region accessibility tests keep package UI out of the editor target. |

## Phase 28.7 P2 visual and interaction recapture (2026-08-21)

UI preflight used `npx ui-skills start`, category `accessibility`, selected
`rams/rams`, then `computer-use-linux_get_app_state` and `doctor` before the
isolated review fixtures. Static evidence is under
`code-reviews/screenshots/2026-08-21-phase28.7-p2-recapture/`.

| Checks | Result | Evidence |
|---|---|---|
| E28–E32 | UNRESOLVED live; PASS structural | No development keyboard backend is available (`uinput` denied, no xdotool/ydotool, Wayland portal input unavailable), so comment/list/heading live mutation was not claimed. Existing transform tests pass. |
| E33 | UNRESOLVED live; PASS automated | Completion fixture could not receive its trigger; completion geometry/ranking/cap tests pass. `completion/review.status` records the unresolved trigger. |
| E34–E36 | PASS live interface/AT-SPI path; E35 keyboard portion remains UNRESOLVED | P1 evidence remains valid under `code-reviews/screenshots/2026-08-20-phase28.7-followups/editor-editable-text/`; the P2 static fixture dumps retain named shell/status semantics. Physical keyboard insertion/undo remains host-blocked. |
| Static shell/error/loading/recovery/large typography | PASS live | `default/`, `loading/`, `error/`, `recovery/`, and `large-typography/` screenshots and AT-SPI dumps were inspected; no new clipping, role/name, contrast, or status defect found. |

No existing step was deleted or weakened. Interactive unresolved states remain
explicit rather than inferred from static screenshots.
