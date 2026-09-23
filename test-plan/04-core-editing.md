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

Fresh Linux/GNOME Wayland review used the UI guidance current at execution time with the
`accessibility` category and `jakubkrehel/better-accessibility`, then
`computer-use-linux get_app_state`, `doctor`, and targeted AT-SPI inspection
against the isolated `ui-review-rust` fixture. The current build exposes the
editor as `Entry` with `EditableText,Text` interfaces, `editable` + `multi-line`
state, bounded text content, and caret metadata. Evidence:
`code-reviews/screenshots/2026-08-20-phase28.7-followups/editor-editable-text/`.

| Checks | Result | Evidence |
|---|---|---|
| E34 | PASS live | `editable-text.txt` records `supports_editable_text=true`, `Accessible,Component,EditableText,Text`, `character_count=94`, and focused multiline states; `accessibility.txt` records the Clay Entry/status tree; screenshot captures the active editor. |
| E35 | UNRESOLVED keyboard live; PASS structural + AT-SPI set-value/selection path | At execution time the (now-removed) native editor a11y unit test covered value, text run, stable ID, selection action, replacement, and undo; current equivalents are CodeMirror's built-in editable-text role plus `frontend/src/test/editor.test.tsx` region/label assertions. AT-SPI `SetTextContents` reached document v2; selection action returned success. `computer-use-linux doctor` reports no keyboard backend (`uinput` denied, no xdotool/ydotool, Wayland portal input unavailable), so physical keyboard insertion/undo is not falsely claimed. |
| E36 | PASS structural | Read-only mutation actions are omitted and existing inactive/hidden-pane stashing plus package-region accessibility tests keep package UI out of the editor target. |

## Phase 28.7 P2 visual and interaction recapture (2026-08-21)

UI preflight used the UI guidance current at execution time, category `accessibility`, selected
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

## Plan 097 Phase 12 Tauri/React visual and accessibility review (2026-08-24)

| Check | Result | Evidence |
|---|---|---|
| Editor chrome and CodeMirror document | PASS visual/a11y rest state | `code-reviews/screenshots/2026-08-24-tauri-react-parity/editor/fixture-*`; real `editor-opened/accessibility.txt` exposes named editor controls and Document editor entry |
| Diagnostics/completion/intelligence rest state | PASS static | `intelligence/fixture-*` shows syntax, diagnostic, fold, and inlay projections; AX snapshot remains bounded |
| Physical typing, undo, completion trigger | UNRESOLVED live; PASS structural | Host has no safe keyboard backend. Existing local-edit, CodeMirror, completion, editable-text, and hot-path tests pass |
| Absolute path safety | PASS | Editor fallback now uses sanitized workspace basename; `frontend/src/test/editor.test.tsx` prevents `/tmp/ws` from reaching chrome/region labels |

## Plan 126 completion-on-large-document step

| # | Action | Expected |
|---|--------|----------|
| E39 | With the ≥4 MiB `review.rs` fixture open (module 03 F56), type a word prefix and press `Ctrl+Space`; move the selection with `ArrowDown` and accept with `Enter`; then repeat and dismiss with `Escape`; finally type a no-match prefix (`zzzz`) and press `Ctrl+Space` again | The modeless popup opens at the caret with the provider's items (`@clay/rust`: keyword `fn` plus the `fn` snippet), the editor keeps focus and reports `has-popup`, and the popup width/row caps of E16 still hold. Accepting inserts the selected item's text (snippet expanded) at the caret; `Escape` closes the popup with no text change; a no-match prefix returns `Empty` — no popup, no blocking panel, no dialog. Opening, dismissing, and accepting must not scale with document size (see module 11 Q41). |

## Plan 127 package-parse-handler responsiveness step

| # | Action | Expected |
|---|--------|----------|
| E40 | With a bundled package parse handler registered for the open mode (`@clay/markdown` on a ≥1 MiB `notes.md`), type a burst of characters and then trigger completion (`Ctrl+J` on hosts where the compositor owns `Ctrl+Space`; `Ctrl+Space` elsewhere) | Input never waits for parser work: every keystroke echoes immediately, the caret tracks the text, and the editor stays focusable and scrollable while the parse handler keeps working in the background (measured acknowledgement on a 1,052,070-byte document: `server.edit_ack` p50 0.33 ms / p95 0.52 ms for 19 keystrokes, module 11 Q42). The completion popup opens modelessly at the caret without waiting for parse work, keeps the editor focused (`has-popup`), and dismisses with `Escape` with no text change; a mode without a completion provider (markdown) returns `Empty` — no popup, no blocking panel, no dialog. Negative: no input stall, no lost keystroke, no orphan loading state. Provider scheduling behind a held lane is automated-only (`latency_lane_unblocked_by_busy_general_lane`; see the plan 127 execution record for the reachability reason). |

## Plan 099 delayed-syntax editing steps

| # | Action | Expected |
|---|---|---|
| E37 | Open a large generated code/Markdown fixture, start a delayed parse, and type a burst at the top | Text and caret update locally before syntax completes; no input waits for IPC/parser work; no `editor.long_task` exceeds 50 ms and local paint stays within the 16 ms hard envelope. |
| E38 | After progressive load, undo/redo, detach/remount, and resync the pane | Programmatic head/chunk/resync installs add no history entries; user text/caret remain coherent and no partial document can be restored by undo. |

## Plan 099 Linux execution record (2026-08-28)

| Check | Result | Evidence |
|---|---|---|
| E37 | UNRESOLVED live; PASS structural companion | The real client launched, but keyboard input and a loaded WebKit editor were unavailable. Frontend hot-path, delayed-session, and long-task invariant suites remain green. |
| E38 | UNRESOLVED live; PASS automated companion | No document could be edited or remounted in this run; single-Text/no-history/remount tests remain the evidence. |

The harness recorded zero long tasks only for its bootstrap trace; this is not
a typing-flow claim.

## Plan 103 Editor Boundary & Design-System Cross-Reference (2026-08-30)

Editor pane container and scrollbars consume `--clay-ds-editor-view-*` recipe variables. The CodeMirror editor canvas text, carets, selections, search highlights, and syntax decorations strictly preserve theme color authority (`var(--clay-editor-*)` and `var(--clay-syntax-*)`). Design systems cannot modify syntax colors or text rendering. See [Module 15](15-ui-design-systems.md) for full design-system switching checks.


## Plan 105 Linux execution record (2026-09-01)

Plan 105 changed no editor behavior by design (connection-loop extraction,
test-module moves, unwrap/expect annotations). E37/E38 automated companions
re-verified on this branch:

| Check | Result | Evidence |
|---|---|---|
| E37 automated companion | PASS | Split `tests/editor_performance.rs` runtime suite (green 2026-08-31, 49.44 s combined) still covers server-authoritative typing/viewport/patch flows including the 10 MiB and 50 MiB cells — the same large-file delayed-parse paths E37 exercises; frontend hot-path suite green (194 tests, task 7). |
| E38 automated companion | PASS | Same suite plus the protocol/runtime `large_document` tests (chunk bounds, resync, no-history/remount invariants) green on this branch (task 6 full run). |
| E37/E38 live typing | UNRESOLVED | `computer-use-linux doctor` 2026-09-01: no capable input backend (`can_send_development_input=false`). Unchanged from Plan 098/099 records; not a Plan 105 regression. |

## Plan 127 execution record (2026-09-19, task 7)

Plan 127 split each runtime domain into a general lane and a latency lane,
bounded the worker command queues, restored the heap limit after near-heap
recovery, and gated commands from revoked or ungranted packages host-side.
Live run: isolated mode-700 root, bundled packages only, portal-driven keyboard
input (`computer-use-linux` `type_text`/`press_key`), AT-SPI probe.

| Steps | Result | Evidence |
|---|---|---|
| E16/E18/E19/E39 (bundled `@clay/rust` active, 65 KiB `review.rs`) | PASS live | Typing `fn live_probe` and `let value = std.` echoed immediately (2,798 → 2,827 chars, caret tracked); the `.` trigger opened the completion popup (`list box Completions` at `507,183,250x130`, rows `rust` group / `as` selected / `fn` / `fn function snippet` / `if` / `in`), editor reported `editable,focused,has-popup`; `Enter` accepted `as` (2,827 → 2,829 chars); a second trigger opened the popup again and `Escape` closed it with no text change (2,830 chars from the trigger keystroke only). `Ctrl+Space` itself is consumed by this host's GNOME input-source switch — recorded as a host ceiling, not a Clay defect; the `.` autocomplete trigger and `Ctrl+J` both work. Artifacts: `test-plan/artifacts/127-lane-scheduling/live-completion/` (`window-popup.png`, `tree-popup.txt`, `server.log`, `client.log`, `clay-server-perf-summary.json`). |
| E40 (bundled `@clay/markdown` parse handler on a 1,052,070-byte `notes.md`) | PASS live (typing) / PASS automated (provider lane) | Typing `typed while parsing` echoed immediately (806 → 825 chars, caret 19) with the package parse handler registered for the mode; the editor stayed focused and the popup path stayed responsive (`Empty` for markdown, which has no completion provider). The provider-lane half — a latency-lane provider that keeps answering while a package parse handler holds the general lane — has no live trigger on this build and is carried by `latency_lane_unblocked_by_busy_general_lane` (4.1–24.9 ms completion latency under a 100–500 ms general-lane hold versus the ~454 ms single-worker baseline, `code-reviews/2026-09-18-plan127-baseline/README.md`). Reachability reason: no bundled package registers a JS completion provider, and a third-party package cannot be enabled with `parse-document`/`completion-provider` because those capability grants have no user-facing surface yet (module 09 P56). Artifacts: `test-plan/artifacts/127-lane-scheduling/live-markdown-parse-handler/`. |
| Automated companions | PASS | `server::js_runtime::tests::latency_lane_unblocked_by_busy_general_lane`, `lane_poison_replaces_only_that_lane`, `third_party_lane_denies_trusted_ops`, `lanes_share_their_domain_op_set`, `queue_bounded_under_flood`, `distinct_documents_never_superseded`, `queue_evicts_oldest_at_capacity`, `near_heap_limit_recovers_with_original_cap`, `revoked_package_commands_refused_per_lane`, `reload_shares_third_party_lanes_untouched`; full serial lib suite 1,414 passed / 1 ignored on the frozen tree. |

Harness: `test-plan/artifacts/127-lane-scheduling/run-live.sh` (isolated launch,
`fixture|completion|markdown` modes, `CLAY_PERF_PROFILE` + `CLAY_PERF_REPORT_DIR`
wired, tree-kill teardown), `probe.py` and `portal-shot.py` (copied from the plan
126 artifact set), `init-markdown.js`, plus the two fixture packages described in
the plan 127 artifact README.

## Plan 126 execution record (2026-09-19, task 6)

| Steps | Result | Evidence |
|---|---|---|
| E16/E18/E19 (re-run on a ≥4 MiB document) | PASS live | Same live run as module 03 F56 (4,231,903-byte document). Typing `fn` auto-activated completion (word context + `activateOnTyping`), and `Ctrl+Space` opened the popup: AT-SPI rows `rust` (group), `fn` (selected), `fn function snippet` at `281,201` next to the caret; editor state `editable,focused,has-popup,supports-autocompletion`. `Escape` closed it (rows 0, text unchanged at 2,761 chars). `zzzz` + `Ctrl+Space` returned `Empty`: text 2,765 chars, popup rows 0, dialogs 0, editor still focused. |
| E39 (accept on a ≥4 MiB document) | PASS live | `ArrowDown` + `Enter` accepted the snippet: document head went from `fn…` to `fn name(args) {    }pub fn helper_000000…` (2,761 → 2,779 chars, caret 7), popup rows 0, editor still focused. `test-plan/artifacts/126-access-paths/live-large-completion/` holds `screenshot.png` (popup open over the 4.2 MB document), `accessibility-popup.txt`, `accessibility-accepted.txt`, `editor.txt`, `escape-dismissal.txt`, `empty-result.txt`, `accept-result.txt`, `drive.txt`. |
| E39 latency | UNRESOLVED live (probe resolution) | The popup is present at the first AT-SPI observation after the keypress, but the probe's tree walk costs ~0.9 s warm (~15 s per fresh process) over D-Bus, so no live keypress→paint number is claimed. Server-side completion round trip on the same 4 MiB document is the measured evidence: ~161 µs median with constant per-request allocation (module 11 Q41). |
| Automated companion | PASS | `cargo test --lib -- --test-threads=1 static_completion_on_large_document_matches_small_document_results` — a 4 MiB document returns exactly the small-document items, so result parity is pinned independently of the live run. |

## Plan 129 connection-loop decomposition execution record (2026-09-20)

Regression-only pass over the refactored connection dispatcher (pure refactor:
`handle_connection_loop` now routes all 31 arms to per-family handlers; no
user-visible behavior intended). Isolated live launch
(`test-plan/artifacts/129-connection-loop/run-live.sh start editing`, 65 KiB
`review.rs`, bundled `@clay/rust` + completion fixture), portal keyboard input,
every step verified against the live AT-SPI tree.

Note: the accessible editor text is a bounded window (~2.7–2.8k chars) that
follows the caret, so recorded character counts are window sizes, not document
sizes; every comparison below is like-for-like inside one window.

| Step | Result | Evidence |
|---|---|---|
| E1 (typing, graphemes) | PARTIAL live | ASCII echo exact: `AB` and `zzmark2` inserted at the caret with the head, caret, and count advancing together (2,798 → 2,801 / 2,805). The non-ASCII/grapheme leg is UNRESOLVED: the portal keyboard path never delivered `é` or `🎉` (`émoji 🎉` produced `moji ` — the dropped characters never reached the app), so no grapheme claim is made. |
| E2 (Backspace/Delete) | PASS live | Character-granular: `Backspace` removed the trailing space (2,803 → 2,802), `Delete` removed the character at the caret (`mojiub…`, 2,802 → 2,801). |
| E3 (indent inheritance) | PASS live | `Enter` on the 4-space-indented comment line kept the block indent on the new line (trailing whitespace on the split line trimmed; net +1 char, caret at the new line). |
| E4 (comment continuation) | **FAIL live** | `Enter` at the end of `    // caret-here comment line` produced `\n    ` — indentation only, no `// ` continuation, although `@clay/rust` declares `"comments":[{"linePrefix":"//","continuePrefix":"// "}]`. `frontend/src/editor/extensions/behavior.ts::applyEnterRule` handles only `continueLineMarkers`/`insertNewlineOnly`/default indent, and `continuePrefix` has no consumer anywhere in `frontend/src` (only the type at `extensions/types.ts:155`; `controller.ts:723` reads `linePrefix` for toggle-comment). Documentation claims the opposite (`docs/reference/packages/creating-packages.md`: “`continuePrefix` controls comment continuation after Enter”; `docs/wiki/modules/behavior-runtime-registration.md`). Pre-existing (no commit ever implemented it) and client-local, i.e. **not** caused by the connection-loop refactor — recorded as a defect to fix or re-document. |
| E5 (electric outdent) | PASS live | `}` typed on a 4-space-indented line snapped to column 0 (2,702 → 2,699: four spaces replaced by the brace). |
| E6 (pair handling) | PASS live | `(` inserted `()` with the caret inside (+2 chars); the following `)` skipped over the auto-close with no text change. |
| E7 (Tab width) | PASS live | `Tab` at line start inserted exactly 4 spaces for the rust mode (`tabSpaces: 4`; 2,701 → 2,705). |
| E8 (undo/redo) | PARTIAL live | Undo PASS twice: `Ctrl+Z` removed `AB` then the earlier stray `k` (2,801 → 2,799 → 2,798) and a second run undid `zzmark2` (2,805 → 2,798) with the caret restored. Redo UNRESOLVED: `Ctrl+Shift+Z` never landed (portal keystrokes dropped, see the host ceiling below), so redo is not claimed live. |
| E9 (history branching) | UNRESOLVED live | Needs more consecutive keystrokes than this host's input path delivered this run; covered by the editor history suite. |
| E16/E18/E19/E39 (completion) | UNRESOLVED live | The completion trigger needs a keypress sequence the portal path could not deliver after the first burst, and the editor node exposes no `EditableText` on this host (module 126 ceiling), so the popup was not re-driven. The refactor moved the `CompletionRequest`/completion-lane code without changing it; fresh automated companions below, and the last live completion passes remain the plan 126/127 records (they predate plan 129). |
| Automated companions (fresh on the refactored tree) | PASS | `cargo test --lib connection::` 96 passed; `cargo test --lib completion::` 31 passed; `frontend` `npx vitest run src/editor` 9 files / 75 tests passed (editor extensions, position map, hot-path invariants); `cargo test --lib control_center` 28 passed. Artifact log: `test-plan/artifacts/129-connection-loop/automated-companions.txt`. |

Host ceiling observed this run: `xdg-desktop-portal-gnome` segfaults on
RemoteDesktop keyboard sessions (journal `xdg-desktop-portal-gnome.service:
Main process exited, code=dumped, status=11/SEGV`), dropping the in-flight
keystroke; keystrokes land only in short bursts after a fresh app launch.
Steps are recorded UNRESOLVED rather than inferred. Artifacts:
`test-plan/artifacts/129-connection-loop/live-editing/` (`steps.txt`,
`steps2.txt`, `screenshot-edited-tail.png`, `server.log`, `client.log`).
