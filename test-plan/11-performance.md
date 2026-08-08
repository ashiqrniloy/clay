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

## Known ceilings

- Very large files beyond documented open limits are rejected by design
  (MAX_OPENABLE_FILE_BYTES) — that rejection IS the correct behavior.
