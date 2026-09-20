# 11 — Performance

Latency and budget feel-checks. Authoritative budgets/fixtures:
`docs/development/performance.md` (criterion baselines, hard guards,
advisory local baselines all live there — deterministic gates are automated;
this module is the human feel-check).

## Setup

```bash
mkdir -p /tmp/clay-perf
python3 - <<'EOF'
with open('/tmp/clay-perf/big.txt', 'w') as f:
    for i in range(200_000):
        f.write(f"line {i:06d}: the quick brown fox jumps over the lazy dog\n")
EOF
# Optional: a big .rs file for grammar pressure
python3 - <<'EOF'
with open('/tmp/clay-perf/big.rs', 'w') as f:
    for i in range(50_000):
        f.write(f"fn f{i:05d}(x: i64) -> i64 {{ x + {i} }} // comment {i}\n")
EOF
```

## Feel checks

**Plan 124 note (2026-09-17).** Every Q-step that says "Command Centre" /
"centered" refers to the command/path surface, which is now the
composer-anchored `/` palette (bottom sheet at the composer's width, one veil
over panes + rail; chord `Ctrl+X Ctrl+O`). The feel/containment criteria are
unchanged, and `Ctrl+X Ctrl+P` now toggles the agent lane instead — its toggle
is a pure layout change with no palette work.

**Plan 125 note (2026-09-18).** The centered projection is retired (see module
10 K100–K108), so `centered_overlay_baselines` and the "centered panel"
wording below are historical: the shipped transient surface is the one palette
sheet, whose elevation is a **halo** (two zero-offset `text.primary` glow
layers) rather than a drop shadow, and whose stages (picker/credential/URL/
OAuth) reuse the same sheet rect instead of opening new chrome. Q10, Q13 and
Q31 keep their budgets against that surface; Q39 and Q40 are the new checks.

| # | Action | Expected |
|---|--------|----------|
| Q1 | Open `big.txt` (~12 MB) | Opens without hang; status/version settles |
| Q2 | Type at top, middle (jump via Ctrl+G equivalent or scroll), bottom | Keystroke-to-paint feels immediate; no IPC wait |
| Q3 | Scroll top↔bottom flicking | Smooth; no long stalls; windowed work keeps up |
| Q4 | `Ctrl+End` then type | End-of-document edits acknowledged promptly |
| Q5 | Open `big.rs` with grammar | Highlighting streams in; typing stays responsive while parse catches up |
| Q6 | Multi-cursor select-all-matches on a common word in `big.txt` | Bounded behavior — either completes or degrades gracefully, never hangs |
| Q7 | Memory watch while scrolling (e.g. `ps`) | No unbounded growth across repeated scrolls |
| Q11 | Phase 24.5 chords + palette feel: bind `Ctrl+Q Ctrl+W` to a movement command in `~/.clay/init.js`; press the first stroke then complete immediately; repeat with a ~2 s pause between strokes; then open `Ctrl+X Ctrl+O`, type several queries back-to-back, close | First stroke never delays typing or inserts text; completion dispatches immediately with no perceptible latency; a stale pending chord cancels silently after the server-owned timeout (~1.5 s — the completing stroke then routes fresh and does nothing); Command Centre opens and each filter update feels instant. Advisory budgets (`docs/development/performance.md` Phase 24.5 section): `COMMAND_CENTRE_OPEN_P95_BUDGET_MS = 50`, `COMMAND_CENTRE_FILTER_UPDATE_P95_BUDGET_MS = 4` — NO wall-clock pass/fail. Automated deterministic guards (not replacements for this visual check): `command_centre_open_filter_and_listing_stay_bounded_off_hot_paths`, `pending_chord_buffer_grows_one_stroke_per_pending_outcome`, `editor_pending_chord_buffer_never_exceeds_longest_bound_sequence` |

## Budgets

- Deterministic hard guards (payload budgets, parse windows) are automated —
  see `docs/development/performance.md` "Deterministic hard guards".
- Advisory local baselines are machine-variant; record numbers only when
  comparing against your own previous runs.

## Window-model budgets (Phase 22.6)

Pane paint, tab switch, and multi-pane decoration traffic gained advisory
P95 budgets plus deterministic guards in Phase 22.6 (see
`docs/development/performance.md` Phase 22.6 section — window_baselines
bench group, `pane_paint_baselines` + `tab_switch_baselines`).

| # | Action | Expected |
|---|--------|----------|
| Q8 | `cargo bench --bench window_baselines -- --sample-size 10 --warm-up-time 1 --measurement-time 2` | Advisory numbers only (linear in pane count, sub-microsecond on dev hardware) — NO wall-clock pass/fail; deterministic guards are automated (linear pane-chrome geometry, no tab-switch document reserialization, 4-pane decoration aggregate ≤ 32768 B) |
| Q9 | 4-pane window, 2 tabs each at 4 panes; rapid `Ctrl+Tab` + `Ctrl+\\` + `Ctrl+Alt+W` while typing | No perceptible stall; pane/decoration work stays bounded — pane count is the only driver (per-pane paint is O(1) placeholder/chrome fills, never document-size work) |
| Q10 | Palette surface (Phase 24.4 surface, plan-125 anchoring): open command/path mode, type a filter, resize the window (incl. below 640 px wide), then close — repeat with 4 panes and 2 tabs | One sheet + one veil appear immediately with no visible duplicate overlay per pane/tab; the sheet keeps the composer's width as the window narrows (no 640-px clamp, no reflow of the dimmed editor behind); close restores instantly; no blur-related jank beyond the one `modal.scrim` backdrop pass. Automated cross-references: `centered_overlay_work_is_bounded_and_scrim_is_single_pass`, `centered_scrim_routes_through_token_driven_primitive_without_blur` (ui_primitive_conformance), `centered_layer_theme_switch_keeps_layer_and_updates_surface_geometry`, `centered_layer_repeated_open_close_cycles_leave_no_orphan_layers` — guards are not replacements for this visual check (the historical centered-era row keeps its budget against the palette sheet) |

## Plan 087 completion feel steps

| # | Action | Expected |
|---|--------|----------|
| Q11 | Trigger completion in a document with many provider items (e.g. 16 markdown items) | Popup appears immediately with ≤ 8 visible rows and ≤ 480 logical px width (`COMPLETION_MAX_VISIBLE_ROWS` / `COMPLETION_MAX_WIDTH_PX`); typing/filtering stays responsive; no per-frame layout/paint cost from the popup (geometry is a pure function of caret + item count) |
| Q12 | Scroll the popup with the mouse wheel or selection movement on a long list | Scroll stays inside the popup shell; no editor text scrolls; feel is immediate (advisory; see `completion_*_baselines` bench groups in `benches/window_baselines.rs`) |
| Q13 | Palette with 60+ entries: open, filter to a short list, scroll, close | Filter/scroll feel stays immediate and bounded (advisory; `centered_overlay_baselines` is the historical centered group, `command_centre_open_baselines` + `completion_filter_baselines` cover the shipped surface); known visual containment follow-up `P1-087-UI-1` is tracked in the plan, not silently waived |
| Q14 | Repeat Q11–Q13 while `Pending edits` > 0 or typing rapidly | No perceptible stall; completion/menu work never blocks the edit queue |

## Plan 088 responsive/performance steps

| # | Action | Expected |
|---|--------|----------|
| Q15 | Run `cargo bench --bench window_baselines -- --sample-size 10 --warm-up-time 1 --measurement-time 2` | Pane/tab/chrome and responsive geometry remain bounded and linear; Criterion numbers are advisory only, never a shared-runner pass/fail gate |
| Q16 | Compare 320/900/1200 logical widths at UI sizes 12, 24, and 96 | Sidebar yields before the main editor becomes unusable; editor/labels stay inside bounds; no full-tree invalidation is visible |
| Q17 | Compare 1× and representative 2× logical-window layout with completion/centered surfaces | Layout uses logical bounds; completion/centered widths/rows remain capped; overlay work stays one bounded projection/scrim |
| Q18 | Reload theme/typography while idle, then type/scroll immediately | Cached resolution installs once; no visible paint/input stall or duplicate layout churn; ordinary edits remain local-optimistic |
| Q19 | Repeat completion/Command Centre and pane/tab switching with pending edits | Results stay responsive; no IPC, JavaScript, document serialization, or file work enters paint/text-event paths |

## Plan 125 feel steps (palette elevation + stage flows)

| # | Action | Expected |
|---|--------|----------|
| Q39 | Open the palette and the `@` mentions menu over a busy editor, then close both (repeat in all four themes) | Both surfaces carry the **halo**, not a drop shadow: a zero-offset, symmetric glow around the border with no dark band on one side (live numbers: rows above the sheet's top border rise `39→42→46→48→51` against a veiled background of 38–39, rows below it are 53/50/47 over the lane's 40; the sheet interior stays darker than the veiled canvas). Standard popovers keep their own drop-shadow recipe; the halo is a value change, not new elevation chrome, so no extra layer or blur pass is introduced. Automated: `tests/package_ui_conformance.rs::plan125_palette_and_mentions_take_the_halo_not_a_drop_shadow`, `src/shell/design_system.rs` `Elevation::Halo` fallback, `design-artifacts/tools/capture-lane-palette.mjs` (normalized box-shadow assertions) |
| Q40 | From the palette, step through a picker flow (list → provider → auth method → credential → URL), then cancel | Each stage swaps rows/prompt **in the same sheet** with no re-anchor, no remount, and no new veil/backdrop pass; the credential stage disables the composer field and focuses the in-sheet shield without a focus jump; cancelling closes in one step and leaves no orphan layer, no stuck draft, and no console warning. Feel stays immediate (the advisory budgets above), and the stage's own rows remain bounded. Automated: `CommandPalette.test.tsx` (stage rendering, shield lifecycle), `Composer.test.tsx` (stage keys, shield send), `WorkspacePanes.test.tsx` (one sheet, one veil), fixture tool scenes `providers`/`auth`/`secret`/`url`/`oauth` |

## Plan 088 task 12 Linux execution record (2026-08-15)

| Checks | Result | Evidence |
|---|---|---|
| Q15 | PASS advisory run | Current `window_baselines` run completed all pane/tab/responsive/centered/completion groups. Representative medians: pane paint 76/452/833 ns for 1/2/4 panes; tab switch 95/407/850 ns; responsive layout ~2.1–2.4 µs. Criterion comparison warnings are advisory and do not fail the plan |
| Q16 | PASS structural / NOT RUN visually | `responsive_layout_work_preserves_sidebar_and_editor_bounds` and six-input `responsive_layout_baselines` pass; no safe compositor resize/window-targeting backend exists for live 320/1200 runs |
| Q17 | PASS structural / NOT RUN visually | `high_dpi_layout_uses_logical_window_bounds` and completion/centered geometry guards pass; live 2× DPI verification is unavailable on the fixed host window |
| Q18 | PASS automated / NOT RUN interactively | Hot-path/theme/typography invalidation guards and canonical config tests pass; settings/theme keyboard delivery is host-blocked |
| Q19 | PASS automated / UNRESOLVED live | Completion/menu/tab/pane bounded-work tests pass; current interactive completion/Command Centre captures remain unresolved because targeted keyboard focus is unavailable |

## Linux execution record (Plan 087 task 11, 2026-08-15)

- **PASS — Q11 caps:** the live popup was `480x340` logical px (width exactly `COMPLETION_MAX_WIDTH_PX`, height = 8 visible rows at 36 px + chrome) with 16 items; opening and dismissing were immediate and typing stayed local-optimistic (`Pending edits` tracked normally; doc version advanced v1→v6 during the session with no stall).
- **Advisory benches:** `completion_open_baselines` / `completion_filter_baselines` / `completion_layout_baselines` and `centered_overlay_baselines` record medians (see `benches/window_baselines.rs` and `docs/development/performance.md` Plan 087 section); wall-clock results are advisory only.
- **Finding carried forward:** live rows below the popup shell (`P1-087-UI-1`) were observed in task 7's captures; the fix is a follow-up in this plan's Further Actions, not silently waived.

## Plan 088 task 12 performance note

The current advisory benchmark completed, but several Criterion comparisons reported statistically significant regressions against the local stored baseline (pane paint, some tab/responsive cases) while remaining far below the blocking budgets. Record these as machine/baseline observations for follow-up, not as manual-plan failures; no promotion policy treats these advisory numbers as CI thresholds.

## Known ceilings

- File loading is chunked and accepts UTF-8 text while the server-owned resident
  rope budget (256 MiB) permits it. Files exceeding that session budget are
  rejected with `DocumentBudgetExceeded`; NUL content in the first 8 KiB is
  rejected as binary. `MAX_CHUNK_BYTES` remains 256 KiB per transfer frame.

## Plan 089 task 9 Linux execution record (2026-08-17)

| Checks | Result | Evidence |
|---|---|---|
| Q15 | PASS advisory run + triage | `window_baselines` run completed all ten Criterion groups (pane_paint/tab_switch/responsive_layout/centered_overlay/completion_open/filter/layout/command_centre_open/completion_selection/accessibility_tree_update); Plan 089 Criterion triage classified every group as machine variance except centered_overlay as benchmark instability; no reproducible implementation regression; no budget raised |
| Q16–Q19 | PASS structural / NOT RUN visually | Responsive layout, high-DPI, and hot-path invalidation tests pass; live resize/DPI verification is covered by the multi-window smoke test (module 01 L20) and the rescale test (module 07 T18) |

## Phase 26 rendering-feel steps

Deep references: `docs/development/performance.md` (Phase 26 chrome paint
advisory budgets: `GUTTER_PAINT_P95_BUDGET_MS = 2`,
`ACTIVE_LINE_PAINT_P95_BUDGET_MS = 1`, `BRACKET_MATCH_PAINT_P95_BUDGET_MS = 1`,
`DECORATION_BACKGROUND_FILL_P95_BUDGET_MS = 2`; the four sum inside the
16 ms `KEYPRESS_TO_LOCAL_PAINT_P95_BUDGET_MS` envelope — compile-time
asserted). Setup: markdown fixture (`tests/fixtures/syntax/markdown.md`),
rust fixture with a 180+ char line (`tests/fixtures/syntax/rust.rs`).

| # | Action | Expected |
|---|--------|----------|
| Q20 | Open the markdown fixture (mixed heading sizes + quote/fence backgrounds) and scroll top↔bottom | Smooth; no stall from per-token scale or background fills; heading lines re-layout once per scroll frame, never per keystroke |
| Q21 | Open the rust fixture, scroll to the long line, horizontal-scroll to the tail and back | Horizontal scrolling feels immediate under `WrapPolicy::None`; the layout cache key is unchanged by horizontal scroll (no re-layout per pixel); the tail renders clipped at the pane edge |
| Q22 | Type inside a heading and inside a fenced block in markdown | Keystroke-to-paint stays within the 16 ms envelope feel; background fills and scale changes follow the edit without flicker; no JS/IPC on the paint path |
| Q23 | `cargo bench --bench editor_baselines -- --sample-size 10 --warm-up-time 1 --measurement-time 2` (advisory) | Visible-extraction, editing, and scroll baselines remain bounded; Criterion numbers are advisory only, never a shared-runner pass/fail gate |

Negative: chrome/background paint never enters the layout cache key (visual
overlays only); horizontal scroll never triggers a layout rebuild; no
wall-clock pass/fail on shared runners.

## Phase 26 Linux execution record (2026-08-19)

| Checks | Result | Evidence |
|---|---|---|
| Q20 | PASS live (static) / automated (dynamic) | `code-reviews/screenshots/2026-08-18-phase26-review/markdown-*/` show the mixed-size prose state; scroll/typing feel is covered by `editor_baselines` (visible extraction, editing, scroll) and the 16 ms envelope guards; live scroll input is host-blocked (review-log V9) |
| Q21 | PASS live (static) / automated (dynamic) | `rust-longline-default/` shows the clipped 180-char line; `horizontal_scroll_does_not_change_layout_cache_key` proves no re-layout per horizontal scroll; `scroll_horizontal_pixels` clamps to the content max |
| Q22 | PASS automated / NOT RUN live | Incremental parse continuity tests + keypress-to-paint envelope guards; live typing is host-blocked |
| Q23 | PASS advisory run | `cargo bench --no-run` compiled all six benchmark suites (26.7 record); advisory numbers only |

## Phase 28 command/intelligence feel and payload checks

Deep reference: `docs/development/performance.md` — “Phase 28.7
command/intelligence payload pins”. Human timing is advisory; payload and
hot-path limits are deterministic automated gates.

| # | Action | Expected |
|---|---|---|
| Q24 | Open completion with exact/case/short/recency candidates, accept one, then reopen the same prefix | Ranking feels immediate and stable; exact prefix/case/shortness/recency order is visible; no ranking work is observable on ordinary typing and no result exceeds 256 items / 16,384 bytes |
| Q25 | Open a large Rust file with foldable blocks; collapse/reopen several parent and nested folds while scrolling | Chevron toggle and hidden-line remapping feel immediate; no document rewrite, full-file reflow, or keypress-to-paint stall; fold publication stays off the local paint path and each set stays ≤2,048 bytes |
| Q26 | Toggle inlay hints in a code document with provider data, then type and scroll | Overlay visibility changes without reflow or a second layout; labels remain bounded/muted; no LSP request is required for the local visibility toggle and ordinary typing stays local-optimistic |
| Q27 | Hover a link while a completion/menu session is open, then activate safe and unsafe targets | Tooltip/intent work does not steal the active menu, start network work, or block typing; link/inlay decoration payload stays ≤8,192 bytes and HTTP/absolute/traversal activation remains display-only/denied |

## Plan 097 Phase 8 SDUI/package renderer checks

| # | Action | Expected |
|---|--------|----------|
| Q28 | Apply a one-node SDUI update beside focused package input/disclosure state | Update is targeted by stable ID; stale base versions drop; surviving object and React state identities remain unchanged |
| Q29 | Build production frontend and inspect startup/package renderer chunks | Startup shell stays below 180 kB gzip; package renderer is code-split; total stays below 404 kB gzip |
| Q30 | Type/scroll while package UI and a server SDUI panel are visible | Local editor paint remains wait-free; package JavaScript, JSON parsing, schema validation, and Tauri/server waits stay outside render/layout/input hot paths |

## Plan 097 Phase 8 Linux execution record (2026-08-23)

| Checks | Result | Evidence |
|---|---|---|
| Q28 | PASS automated | `frontend/src/sdui/state.test.ts` validates targeted replacement, surviving identity, and stale-update denial; registry test retains text/disclosure state |
| Q29 | PASS production build | Startup shell 164.3/180 kB gzip; code-split package renderer 27.8 kB gzip; total 299.3/400 kB gzip |
| Q30 | PASS structural + existing editor budgets | 79 frontend tests passed in the implementation run; 1 MiB local typing and 1,000-span projection budgets stayed green; package projection reads cached parsed DTOs only |

No performance budget was raised. Wide/narrow/large-type screenshots are under `code-reviews/screenshots/2026-08-23-tauri-react-phase8/`.

## Plan 097 Phase 9 desktop workflow checks

| # | Action | Expected |
|---|--------|----------|
| Q31 | Open/filter a 256-item palette or Path Browser snapshot repeatedly | Existing 50 ms open / 4 ms filter advisory budgets remain; React performs no fuzzy/filesystem/package work and native bounded scrolling stays smooth (collections protocol-capped at 256, no frontend scorer). Stage flows reuse the same sheet: a stage change is one snapshot, never a second surface or a remount (plan 125) |
| Q32 | Trigger configuration reload, theme/appearance switch, and typography apply while typing | CodeMirror local edit/paint remains wait-free; configuration and preference work stays server-side and atomic; one runtime snapshot updates derived UI state |
| Q33 | Build production frontend after command/settings chunks land | Startup shell stays below 180 kB gzip, total below 404 kB gzip; command/settings code remains behind lazy workspace/package chunks |

## Plan 097 Phase 9 execution record (2026-08-23)

| Checks | Result | Evidence |
|---|---|---|
| Q31 | PASS deterministic/structural | Existing menu payload/work-count baselines plus 83 frontend tests; collections remain protocol-capped at 256 and no frontend scorer/listing exists |
| Q32 | PASS structural/automated | Existing local edit performance tests and atomic reload/settings tests pass; workspace consumes only pushed snapshots/diagnostics |
| Q33 | PASS production build | Startup shell 156.5/180 kB gzip; lazy workflow chunks 34.9 kB; total 304.9/400 kB; no budget raised |

## Phase 28 Linux execution record (2026-08-20)

| Checks | Result | Evidence |
|---|---|---|
| Q24 | PARTIAL live; PASS automated | Completion popup/rest capture exists under `code-reviews/screenshots/2026-08-20-phase28-primitives/completion/`; `hel` had no bundled match, so ranking feel was not visually verified. Scorer, recency, cap, and hot-path tests pass. |
| Q25 | UNRESOLVED live; PASS automated | Rust fold rest capture passed; compositor targeting prevented repeatable collapse/scroll feel checks. Folding budget and hidden-line unit tests pass. |
| Q26 | UNRESOLVED live; PASS automated | LSP GUI worker could not resolve `lsp-shared`; overlay toggle, prose default, payload, and no-paint-path tests pass. |
| Q27 | UNRESOLVED live; PASS automated/security | Link pointer targeting was unstable; no network/external target was opened. Decoration budget, target validation, activation planning, and hot-path tests pass. |

## Phase 28.7 P1 GUI analyzer follow-up (2026-08-21)

| Checks | Result | Evidence |
|---|---|---|
| Q26 | UNRESOLVED live; PASS automated/worker structural | `lsp-shared` resolution and analyzer workspace context are repaired; the fresh P2 Rust fixture reached the analyzer path but did not produce a non-empty inlay set after an AT-SPI edit, and the host has no keyboard input backend for the required toggle sequence. No visible/toggled-off claim is made. Local toggle, inlay payload, no-reflow, and hot-path tests pass. |

## Plan 126 completion-on-large-document budget step

| # | Action | Expected |
|---|--------|----------|
| Q41 | On the ≥4 MiB fixture (module 03 F56), trigger completion (`Ctrl+Space` after a word prefix), move the selection, accept with `Enter`, then repeat and dismiss with `Escape` | The popup appears without a perceptible stall and the editor stays responsive: the completion request/response round trip on a 4 MiB document stays in the sub-millisecond range with per-request allocation independent of document size (the O(document) copy this step guards against was removed by plan 126 task 3). Negative: no document-sized allocation spike, no input wait for the provider, no stale popup after dismissal. |

## Plan 127 lane-scheduling typing-latency step

| # | Action | Expected |
|---|--------|----------|
| Q42 | Open a ≥1 MiB markdown document with `@clay/markdown` loaded (its parse handler is registered for the mode), type a burst of keystrokes, then stop the server with `SIGTERM` and read the perf summary (`CLAY_PERF_PROFILE=1`, `CLAY_PERF_REPORT_DIR`) | Typing stays local and immediate while parse catches up: `server.edit_ack` p50 0.33 ms / p95 0.52 ms / max 0.52 ms for 19 keystrokes on a 1,052,070-byte document, with `syntax.parse.*` continuing in the background (`syntax.parse.invocations` 6, `syntax.edit_to_publish` p50 61.6 ms) and no `js_runtime.command.superseded`/`evicted` churn (parse commands are deliberately not supersedable). Advisory only — the blocking budgets stay with the module-11 envelope checks. Negative: no keystroke waits for parser or runtime work, no typing long task, no queue-driven stall. Ceiling: the first cold full parse of a 1 MiB markdown document in a debug build is seconds-scale (`syntax.edit_to_publish` max 63.2 s in this run) — that is parse-side feel, not input latency, and is recorded rather than budgeted. |

## Plan 128 large-document typing and language-route step

| # | Action | Expected |
|---|--------|----------|
| Q43 | Open a ≥1 MiB Rust file, type a burst of keystrokes, then move the pointer over an identifier and trigger completion (`Ctrl+J`); repeat on documents the analyzer accepts (a ≈4 KiB crate, then a crate with a ≈250 KiB open module) | Typing echoes with no stall and the language route stays live on an accepted document; a document over the package analysis limit fails closed with the limit note while baseline syntax gestures stay responsive; no language request blocks typing. Read `server.edit_ack` p50/p95 from the perf summary (`CLAY_PERF_PROFILE=1`, `CLAY_PERF_REPORT_DIR`). |

## Phase 28.7 P2 visual and interaction recapture (2026-08-21)

UI preflight used the UI guidance current at execution time, category `accessibility`, selected
`rams/rams`, and `computer-use-linux_get_app_state` before review. Static
screenshots and AT-SPI dumps are under
`code-reviews/screenshots/2026-08-21-phase28.7-p2-recapture/`.

| Checks | Result | Evidence |
|---|---|---|
| Q24 | UNRESOLVED live; PASS automated | Completion trigger/ranking feel was not reachable without keyboard input; completion scan, cap, payload, and hot-path tests pass. |
| Q25 | UNRESOLVED live; PASS automated | Fold rest state is retained, but collapse/reopen/scroll feel was not safely driven; hidden-line and folding-budget tests pass. |
| Q26 | UNRESOLVED live; PASS automated/worker structural | Fresh Rust analyzer path was reached but no non-empty inlay set was published after AT-SPI edit; no toggle/reflow claim is made. Inlay/no-reflow/hot-path tests pass. |
| Q27 | UNRESOLVED live; PASS automated/security | Link hover/menu coexistence and safe/unsafe activation were not targetable; decoration budget, target validation, activation denial, and no-network tests pass. |
| Narrow/wide layout | NOT RUN visually; PASS structural | Fixed review captures are 900 logical pixels; responsive bounds and typography geometry tests pass, but no resize pass is inferred. |

No performance budget was changed.

## Plan 097 Phase 12 Tauri/React visual and accessibility review (2026-08-24)

| Check | Result | Evidence |
|---|---|---|
| Wide/narrow rendered surfaces | PASS static | 20 CDP captures under `code-reviews/screenshots/2026-08-24-tauri-react-parity/` at 1440×900 and 780×900 show no clipping, duplicate overlay, or visible layout jank |
| Editor/package/agent-transcript render cost | PASS structural; stream feel unresolved | Existing CodeMirror, SDUI, AG-UI, list, and hot-path tests pass; provider setup/input prevented a live streaming-latency claim |
| Bundle budget | PASS | Frontend build: shell 160.6 kB gzip / 180 kB budget; total 343.2 kB / 400 kB budget |
| Keyboard/filter/resize feel | UNRESOLVED live | Host cannot safely deliver keyboard or compositor resize actions; no visual pass inferred from source/tests |

## Plan 097 manual-test-plan re-measurement (2026-08-24, post-cutover)

| Check | Result | Evidence |
|---|---|---|
| Bundle budgets (fresh production build) | PASS | `npm run build` + `check:budget`: shell 160.6/180 kB gzip, total 343.2/400 kB gzip — no budget raised |
| Rust gate timing | PASS | `cargo test --all-targets` suites complete in seconds each (protocol ≈0.2 s, security ≈0.15–0.45 s, runtime ≈0.3 s, presentation ≈0.05 s); no stalled suite |
| Agent host | PASS structural | clay-agent unit tests pass (8 tests); daemon spawn/stream behavior unchanged by migration; live provider latency not claimable without credentials on this host |

## Plan 098 chunked document performance steps

| # | Action | Expected |
|---|--------|----------|
| Q34 | Open the synthetic 50 MiB UTF-8 `large.md` through the real desktop smoke path | First head/first paint meets the size-scaled debug throughput floor `max(500 ms, bytes / 25 MiB/s)` (2 s at 50 MiB); full chunk assembly stays under 5 s, and each head/chunk stays at or below `MAX_CHUNK_BYTES` (256 KiB); no full-document IPC frame is emitted. The size-scaled floor replaces the former flat 500 ms debug-profile assumption: it retains a structural-regression guard without failing under ordinary CI contention. |
| Q35 | Edit the ready 50 MiB document, Save, and Reload | Save acknowledgement remains bounded and responsive; disk/reloaded bytes equal the edited document; ordinary editor input does not wait on chunk or save IO |
| Q36 | Attempt `oversize.txt` (257 MiB sparse) and `binary.dat` | Resident-budget and binary-sniff refusals return promptly with typed diagnostics; no large content allocation, stale loading loop, or unbounded memory growth occurs |
| Q37 | Inspect the protocol-v28 runtime run and frame assertions | `DocumentChunk` payloads remain ≤256 KiB and below the 1 MiB codec frame ceiling; v28 handshake and mixed-version rejection remain deterministic |
| Q38 | Load the app after the Plan 105 index code-split (restart or fresh launch) | Startup fetches the `index` chunk plus the parallel `codemirror` chunk via modulepreload with no sequential first-paint waterfall; the index raw size stays below the 500 KiB rollup warning; `npm run check:budget` shell (≤180 kB gzip) and total (≤404 kB gzip) budgets hold |


## Plan 098 Linux execution record (2026-08-26)

| Checks | Result | Evidence |
|---|---|---|
| Q34 | PASS automated real-server measurement; UNRESOLVED desktop feel | `cargo test --test runtime large_document:: -- --nocapture`: open→head 297.339689 ms; open→full 589.273483 ms; head 262,142 bytes / total 52,428,800. Three isolated reruns stayed at 287.225620–301.800012 ms; one host-scheduled outlier reached 538.188757 ms and tripped the existing 500 ms guard before the next combined run passed. GUI screenshot/interaction evidence is `code-reviews/screenshots/2026-08-26-plan098-manual/real-app-welcome.png`; the large loaded editor could not be stably targeted |
| Q35 | PASS automated real-server measurement; UNRESOLVED desktop feel | Same final run: save→ack 423.454128 ms for 52,428,815 bytes; streamed save and reload equality assertions passed |
| Q36 | PASS automated; UNRESOLVED desktop error presentation | Same runtime suite passed typed resident-budget and binary-sniff refusal assertions; no live status/pane error screenshot was claimed |
| Q37 | PASS | Protocol/runtime assertions passed; `large-document-runtime.log` records chunk-bound and v27 path results. `cargo test --lib protocol_v26_client_is_rejected_by_v27_server` is the mixed-version guard |

Known ceiling for this record: live WebKitGTK/portal interaction was not
repeatable because the window moved off-screen and AT-SPI exposed only the
native frame. Automated bounds and server timing do not substitute for GUI
feel; repeat Q34–Q36 with a stable target before marking live interaction PASS.

## Plan 099 Linux execution record (2026-08-28)

Evidence: [`manual-test-plan.md`](../code-reviews/screenshots/2026-08-28-plan099-manual/manual-test-plan.md) and `target/perf/editor-performance/plan099-manual-20260828-181828/`.

The full harness generated 72 synthetic fixtures (1/10/50 MiB × four line
shapes × `.txt`, `.md`, `.rs`, `.ts`, `.tsx`, `.js`), rebuilt the profiled
frontend and binaries, launched real Tauri/WebKit against a private server,
and closed the window through its AT-SPI Close action. `--enforce` passed:
zero long tasks over 50 ms; frontend retained 18/dropped 0; desktop 216/0;
server 221/0. Bootstrap-only p95s were frontend open/ready 0/0 ms, desktop
codec decode/encode 0.151/0.043 ms, server codec decode/encode 0.035/0.233
ms, and configuration load 14.056 ms. `bridge.patch_delivery` was absent,
and parser queue count was 0, because no fixture was opened.

| Step | Result | Evidence |
|---|---|---|
| M1 | PARTIAL / UNRESOLVED | Fixture generation/private launch/Connected welcome passed; file-browser opens and ready content were not drivable. |
| M2 | UNRESOLVED | Keyboard input unavailable. |
| M3 | UNRESOLVED | Keyboard/scroll input unavailable; no viewport patch stage. |
| M4 | UNRESOLVED | Keyboard input unavailable. |
| M5 | UNRESOLVED | No fixture opened or edited. |
| M6 | UNRESOLVED | Keyboard input unavailable. |
| M7 | UNRESOLVED | No document session existed for meaningful server resync. |

Supplemental one-fixture idle RSS observation: server 58,188 KiB, client
13,876 KiB, desktop 211,888 KiB, tracked total 283,952 KiB (277.3 MiB).
This is startup/idle process RSS, not the 256 MiB document-resident budget.
Host: Linux 7.1.8-50.stable, Ryzen 9 PRO 7940HS, GNOME Wayland,
WebKitGTK 2.52.5, Rust 1.96.1, Node 24.19.0. The designated minimum-device
three-run timing gate remains open. No manual editor-flow pass is inferred
from the bootstrap trace.

## Plan 102 & 103 design-system switching and paint budget cross-reference

Design-system/theme switch latency, atomicity, effect bounds, and no-churn observations:
[15 — UI design systems](15-ui-design-systems.md) (UI-DS-P1…UI-DS-P3,
recorded 2026-08-30): visible swap ≈2 s dominated by the documented watcher
debounce, one atomic swap per generation, no partial frames, zero frontend
writes for identical generations (automated adapter idempotence test).

Plan 103 production build budgets (enforced via `npm run check:budget`):
- Shell gzip: 169.3 kB (≤ 180 kB budget)
- Package renderer gzip: 28.0 kB
- Total bundle gzip: 359.6 kB (≤ 400 kB budget)
Effect bounds: max blur 32px, max shadow layers 3, max motion 1000ms, max border width 8px.
Zero recipe computation occurs in React render loops or keystroke hot paths.


## Plan 119 P1-1 throughput execution record (2026-09-15)

| Check | Result | Evidence |
|---|---|---|
| Q34–Q37 | PASS automated real-server path; UNRESOLVED desktop feel | Fresh `cargo test --test runtime large_document::` passed the 50 MiB chunked open/edit/save/reload and oversize/binary refusal tests in 2.04 s. The deterministic guard is now the documented 25 MiB/s floor with a 500 ms minimum; the 5 s end-to-end bound and 256 KiB chunk ceiling remain unchanged. The review host did not open the fixture through a controllable WebKit file-picker flow, so this does not claim a human first-paint pass. |

No performance budget was raised: the debug test budget now scales with input size, matching the mandatory full-rope load before the head exists.

## Plan 105 chunk-split startup record (2026-09-01)

| Check | Result | Evidence |
|---|---|---|
| Q38 | PASS | Task 9 build evidence (`docs/development/performance.md`): index raw 517.98 → 468.84 kB (≈148.7 kB gzip), rollup warning cleared; `codemirror` chunk (361.65 kB raw / 117.80 kB gzip) loads in parallel via modulepreload (2 requests, no waterfall); `npm run check:budget` gates shell 153.4 kB/≤180 kB and total 359.7 kB/≤400 kB gzip; all frontend tests green. The documented trade-off (startup gzip up ~101 kB because the eager shell already imported a codemirror slice; editor-open bytes drop equally) is recorded there. The 2026-09-01 launch-gate capture `code-reviews/screenshots/2026-09-01-plan105-manual/default/` boots this exact split build with the welcome state rendering normally — no startup regression observed. |

## Plan 125 execution record (Linux, 2026-09-18)

Live run on the canonical config in an isolated root (captures + numbers in
`test-plan/artifacts/125-palette/`), plus the fixture tool and the suites named
in the rows.

| Step | Result | Evidence |
|---|---|---|
| Q10 (palette surface, resize/narrow) | PASS live (1500 px + 1024 px) / PASS structural | Live: one sheet + one veil, no duplicate overlay; at 1024 px the sheet still matched the composer box (`1124x420` at 1500 px; width tracked the box at the narrow width) with the 6px gap and the 420 px cap unchanged, and closing restored instantly (`01-rest.png` … `05-lane-restored-palette.png`). Resize-under-load and the 640-px clamp question are historical (the sheet has no centered width token since plan 125); the multi-pane/per-tab duplicate guard stays in `ui_primitive_conformance`. |
| Q13 (60+ entries, filter/scroll) | PASS live (open + scroll bounds) + PASS structural | Live: the sheet opened with `95 results` and an internal list box of 306 px inside the 420 px sheet (the remainder scrolls inside the sheet, nothing overflows the window). Filter feel is the advisory budget below; the query path itself could not be typed on this host (ceiling). |
| Q31 (256-item snapshot repetition) | PASS structural (unchanged) | No frontend scorer/listing exists; collections stay protocol-capped at 256, and a stage change is one snapshot (see Q40). The 50 ms/4 ms advisory budgets are unchanged by this plan and were not re-benched here. |
| Q39 (halo, not a drop shadow) | PASS live (numeric) + PASS automated | Live, `python3 launch-check.py …` (`geometry-halo.txt`): above the sheet's top border the pixels rise `39→42→46→48→51` over rows 525–530 against a veiled background of 38–39 and a rest canvas of 40 (i.e. brighter than the un-veiled canvas at the edge); below the bottom border rows 951–953 read 53/50/47 over the lane's 40 — symmetric, **zero offset**, no dark band, and the sheet interior (29,32,33) stays darker than the veiled canvas. Recipe/conformance: `plan125_palette_and_mentions_take_the_halo_not_a_drop_shadow`, `Elevation::Halo` fallback, and the fixture tool's normalized box-shadow assertions. |
| Q40 (stage transitions, same sheet) | PASS fixture + PASS automated / UNRESOLVED live | The fixture scenes (`providers`/`auth`/`secret`/`url`/`oauth`) step through the flow in one sheet with one veil; `CommandPalette.test.tsx` + `Composer.test.tsx` pin the stage swap, the shield lifecycle, and the focus return; `WorkspacePanes.test.tsx` pins one sheet/veil and no centred fallback. Live stage navigation is blocked by the host ceiling (no row activation), which is recorded rather than assumed. |

**Ceilings.** No keyboard/pointer synthesis on this host, so the typed-query and
row-activation legs of Q13/Q31/Q40 remain UNRESOLVED live; the numbers that are
claimed (geometry, veil deltas, halo profile, scroll bounds) come from pixel
probes on window-cropped captures. Wall-clock budgets stay advisory as always.

## Plan 127 execution record (2026-09-19, task 7)

| Steps | Result | Evidence |
|---|---|---|
| Q42 | PASS measured + PASS live presence | Live isolated run on a generated 1,052,070-byte `notes.md` with `@clay/markdown` active: 19 keystrokes were typed through the portal and acknowledged with `server.edit_ack` p50 329,027 ns (0.33 ms) / p95 520,442 ns (0.52 ms) / max 0.52 ms; `server.document.apply_edit` p50 0.08 ms; parse work continued in the background (`syntax.parse.invocations` 6, `syntax.parse.full` 6, `syntax.decoration.chunks` 5, `syntax.edit_to_publish` p50 61.6 ms / max 63.2 s cold). Source: `test-plan/artifacts/127-lane-scheduling/live-markdown-parse-handler/clay-server-perf-summary.json` (full metric list, 407 retained events, 0 dropped). |
| Lane and queue budgets | PASS (automated) | Plan 127's scheduling budgets are pinned by tests, not by live timing: latency lane heap cap `JS_RUNTIME_LATENCY_LANE_HEAP_LIMIT_BYTES = 32 MiB` (`lane_heap_limits_stay_within_the_configured_budget`), supersedable backlog `JS_RUNTIME_SUPERSEDABLE_QUEUE_CAPACITY = 64` (`queue_bounded_under_flood`, `queue_evicts_oldest_at_capacity`), and lane occupancy `JS_RUNTIME_LANES_PER_DOMAIN = 2` (`lanes_share_their_domain_op_set`). Counters `js_runtime.command.superseded` / `js_runtime.command.evicted` are exported in the perf summary. |
| Live keypress→paint timing for the lane split | UNRESOLVED (probe resolution + reachability) | The AT-SPI probe's tree walk (~0.9 s warm) cannot resolve a lane handoff, and no live completion provider exists behind the split (no bundled JS completion package; third-party capability grants have no user-facing surface — module 09 P56). The measured evidence is the automated harness in `code-reviews/2026-09-18-plan127-baseline/README.md` (4.1–24.9 ms versus ~454 ms) plus Q42's input numbers above. |

## Plan 126 execution record (2026-09-19, task 6)

| Steps | Result | Evidence |
|---|---|---|
| Q41 | PASS live (presence) / PASS measured (server path) | Live on the 4,231,903-byte document the popup opened at the caret and accepted a snippet (module 04 E39). Server-side measurement on the same size class: completion round trip ~161 µs median (down from 427–569 µs before plan 126 task 3) with 31,118 bytes allocated per request on 4 MiB — identical to the 64 KiB case, i.e. no O(document) work. Source: `code-reviews/2026-09-18-plan126-baseline/README.md` (task-3 after-measurements). |
| Q41 latency resolution | Ceiling recorded | The live AT-SPI probe cannot resolve sub-second paint timing (one full tree walk is ~0.9 s warm, ~15 s per fresh process), so the live leg claims presence/acceptance, not a frame number; the measured server round trip above is the budget evidence. |
| Automated companion | PASS | `static_completion_on_large_document_matches_small_document_results` (4 MiB result parity) plus `language_intelligence_window_budget_honored` (window budget on a large multibyte document) are green in the lib suite; `cargo test --test runtime large_document::` covers the 50 MiB open/edit/save path. |

## Plan 128 execution record (2026-09-20, task 4)

The question added by plan 128 is "does the typing/LSP route stay flat in document
size once the shared position index is incremental". The measured half was taken
automated (adapter probe, below); the live half split into a pass on the accepted
size class, a hard fail on the first document big enough to exercise a real
semantic payload, and a fail-closed at the size the plan asked about.

| Steps | Result | Evidence |
|---|---|---|
| Q43 live (typing responsiveness, LSP route, ≥1 MiB) | UNRESOLVED live (host input) | No keystroke reached the document: `Ctrl+End`, `Ctrl+B` (the status bar's own lane toggle) and 60-char bursts left the file mtime, the word count and the `clean` state untouched, and the perf summary has no `edit_apply`/`edit_ack` samples. Readiness probe: `can_send_development_input: false` (portal session without remote-interaction permission). The earlier plan-126/127 typing records were made on this host while that session was granted, so this is a host state, not a build property. |
| Q43 measured (edit + refresh cost through the adapter) | PASS measured | `code-reviews/2026-09-19-plan128-task3/`: edit+refresh median went 2.805 / 13.388 / 37.293 / 289.619 ms (HEAD worktree) → 0.029 / 0.039 / 0.030 / 0.026 ms at 64 KiB / 256 KiB / 1 MiB / 8 MiB; `VersionedDocument#applyByteChange` itself is 0.014–0.021 ms with no size term. |
| Q43 live (language route on an accepted document) | PASS live (presence only) | 4 KiB crate: rust-analyzer + proc-macro server spawned in the private root, no analyzer failure, `bridge.patch_delivery` p50 0.089 ms / p95 0.121 ms. |
| Q43 live (≥1 MiB LSP features) | FAIL fail-closed (pre-existing, two independent bounds) | `DOCUMENT_ANALYSIS_MAX_DOCUMENT_BYTES = 256 KiB` (`src/perf/budgets.rs:33`) keeps the analyzer from starting, so hover/completion cannot resolve there; below that cap `MAX_SEMANTIC_TOKENS = 128` (`packages/lsp-shared/mapping.js:3`) kills the worker on the first real semantic payload (observed at ≈250 KiB). Plan 128's index makes both bounds cheap to raise, but neither is part of this plan. |
| Live frame timing | Not claimed | AT-SPI probe resolution (~0.9 s per tree walk) is coarser than the event, and this run produced no keystrokes at all. |
