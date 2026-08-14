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

## Linux execution record (Plan 087 task 11, 2026-08-15)

- **PASS — Q11 caps:** the live popup was `480x340` logical px (width exactly `COMPLETION_MAX_WIDTH_PX`, height = 8 visible rows at 36 px + chrome) with 16 items; opening and dismissing were immediate and typing stayed local-optimistic (`Pending edits` tracked normally; doc version advanced v1→v6 during the session with no stall).
- **Advisory benches:** `completion_open_baselines` / `completion_filter_baselines` / `completion_layout_baselines` and `centered_overlay_baselines` record medians (see `benches/window_baselines.rs` and `docs/development/performance.md` Plan 087 section); wall-clock results are advisory only.
- **Finding carried forward:** live rows below the popup shell (`P1-087-UI-1`) were observed in task 7's captures; the fix is a follow-up in this plan's Further Actions, not silently waived.

## Known ceilings

- Very large files beyond documented open limits are rejected by design
  (MAX_OPENABLE_FILE_BYTES) — that rejection IS the correct behavior.
