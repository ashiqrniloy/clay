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

## Known ceilings

- Very large files beyond documented open limits are rejected by design
  (MAX_OPENABLE_FILE_BYTES) — that rejection IS the correct behavior.
