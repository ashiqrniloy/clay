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

| # | Action | Expected |
|---|--------|----------|
| Q1 | Open `big.txt` (~12 MB) | Opens without hang; status/version settles |
| Q2 | Type at top, middle (jump via Ctrl+G equivalent or scroll), bottom | Keystroke-to-paint feels immediate; no IPC wait |
| Q3 | Scroll top↔bottom flicking | Smooth; no long stalls; windowed work keeps up |
| Q4 | `Ctrl+End` then type | End-of-document edits acknowledged promptly |
| Q5 | Open `big.rs` with grammar | Highlighting streams in; typing stays responsive while parse catches up |
| Q6 | Multi-cursor select-all-matches on a common word in `big.txt` | Bounded behavior — either completes or degrades gracefully, never hangs |
| Q7 | Memory watch while scrolling (e.g. `ps`) | No unbounded growth across repeated scrolls |
| Q11 | Phase 24.5 chords + Command Centre feel: bind `Ctrl+Q Ctrl+W` to a movement command in `~/.config/clay/init.js`; press the first stroke then complete immediately; repeat with a ~2 s pause between strokes; then open `Ctrl+X Ctrl+P`, type several queries back-to-back, close | First stroke never delays typing or inserts text; completion dispatches immediately with no perceptible latency; a stale pending chord cancels silently after the server-owned timeout (~1.5 s — the completing stroke then routes fresh and does nothing); Command Centre opens and each filter update feels instant. Advisory budgets (`docs/development/performance.md` Phase 24.5 section): `COMMAND_CENTRE_OPEN_P95_BUDGET_MS = 50`, `COMMAND_CENTRE_FILTER_UPDATE_P95_BUDGET_MS = 4` — NO wall-clock pass/fail. Automated deterministic guards (not replacements for this visual check): `command_centre_open_filter_and_listing_stay_bounded_off_hot_paths`, `pending_chord_buffer_grows_one_stroke_per_pending_outcome`, `editor_pending_chord_buffer_never_exceeds_longest_bound_sequence` |

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
| Q10 | Centered Command Centre surface (Phase 24.4): open command/path mode, type a filter, resize the window (incl. below 640 px wide), then close — repeat with 4 panes and 2 tabs | One centered panel + one full-window scrim appear immediately with no visible duplicate overlay per pane/tab; width clamps smoothly to the window with no reflow of the dimmed editor behind; close restores instantly; no blur-related jank (no backdrop filter runs). Automated cross-references: `centered_overlay_work_is_bounded_and_scrim_is_single_pass`, `centered_scrim_routes_through_token_driven_primitive_without_blur` (ui_primitive_conformance), `centered_layer_theme_switch_keeps_layer_and_updates_surface_geometry`, `centered_layer_repeated_open_close_cycles_leave_no_orphan_layers` — guards are not replacements for this visual check |

## Plan 087 completion feel steps

| # | Action | Expected |
|---|--------|----------|
| Q11 | Trigger completion in a document with many provider items (e.g. 16 markdown items) | Popup appears immediately with ≤ 8 visible rows and ≤ 480 logical px width (`COMPLETION_MAX_VISIBLE_ROWS` / `COMPLETION_MAX_WIDTH_PX`); typing/filtering stays responsive; no per-frame layout/paint cost from the popup (geometry is a pure function of caret + item count) |
| Q12 | Scroll the popup with the mouse wheel or selection movement on a long list | Scroll stays inside the popup shell; no editor text scrolls; feel is immediate (advisory; see `completion_*_baselines` bench groups in `benches/window_baselines.rs`) |
| Q13 | Command Centre with 60+ entries: open, filter to a short list, scroll, close | Filter/scroll feel stays immediate and bounded (advisory; `centered_overlay_baselines` + `completion_filter_baselines`); known visual containment follow-up `P1-087-UI-1` is tracked in the plan, not silently waived |
| Q14 | Repeat Q11–Q13 while `Pending edits` > 0 or typing rapidly | No perceptible stall; completion/menu work never blocks the edit queue |

## Plan 088 responsive/performance steps

| # | Action | Expected |
|---|--------|----------|
| Q15 | Run `cargo bench --bench window_baselines -- --sample-size 10 --warm-up-time 1 --measurement-time 2` | Pane/tab/chrome and responsive geometry remain bounded and linear; Criterion numbers are advisory only, never a shared-runner pass/fail gate |
| Q16 | Compare 320/900/1200 logical widths at UI sizes 12, 24, and 96 | Sidebar yields before the main editor becomes unusable; editor/labels stay inside bounds; no full-tree invalidation is visible |
| Q17 | Compare 1× and representative 2× logical-window layout with completion/centered surfaces | Layout uses logical bounds; completion/centered widths/rows remain capped; overlay work stays one bounded projection/scrim |
| Q18 | Reload theme/typography while idle, then type/scroll immediately | Cached resolution installs once; no visible paint/input stall or duplicate layout churn; ordinary edits remain local-optimistic |
| Q19 | Repeat completion/Command Centre and pane/tab switching with pending edits | Results stay responsive; no IPC, JavaScript, document serialization, or file work enters paint/text-event paths |

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

- Very large files beyond documented open limits are rejected by design
  (MAX_OPENABLE_FILE_BYTES) — that rejection IS the correct behavior.

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
| Q29 | Build production frontend and inspect startup/package renderer chunks | Startup shell stays below 180 kB gzip; package renderer is code-split; total stays below 400 kB gzip |
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
| Q31 | Open/filter a 256-item Command Centre or Path Browser snapshot repeatedly | Existing 50 ms open / 4 ms filter advisory budgets remain; React performs no fuzzy/filesystem/package work and native bounded scrolling stays responsive |
| Q32 | Trigger configuration reload, theme/appearance switch, and typography apply while typing | CodeMirror local edit/paint remains wait-free; configuration and preference work stays server-side and atomic; one runtime snapshot updates derived UI state |
| Q33 | Build production frontend after command/settings chunks land | Startup shell stays below 180 kB gzip, total below 400 kB gzip; command/settings code remains behind lazy workspace/package chunks |

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
| Editor/package/Chat render cost | PASS structural; stream feel unresolved | Existing CodeMirror, SDUI, AG-UI, list, and hot-path tests pass; provider setup/input prevented a live streaming-latency claim |
| Bundle budget | PASS | Frontend build: shell 160.6 kB gzip / 180 kB budget; total 343.2 kB / 400 kB budget |
| Keyboard/filter/resize feel | UNRESOLVED live | Host cannot safely deliver keyboard or compositor resize actions; no visual pass inferred from source/tests |

## Plan 097 manual-test-plan re-measurement (2026-08-24, post-cutover)

| Check | Result | Evidence |
|---|---|---|
| Bundle budgets (fresh production build) | PASS | `npm run build` + `check:budget`: shell 160.6/180 kB gzip, total 343.2/400 kB gzip — no budget raised |
| Rust gate timing | PASS | `cargo test --all-targets` suites complete in seconds each (protocol ≈0.2 s, security ≈0.15–0.45 s, runtime ≈0.3 s, presentation ≈0.05 s); no stalled suite |
| Agent host | PASS structural | clay-agent unit tests pass (8 tests); daemon spawn/stream behavior unchanged by migration; live provider latency not claimable without credentials on this host |
