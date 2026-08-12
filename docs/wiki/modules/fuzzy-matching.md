# Fuzzy Matching (Phase 24.2)

One shared, bounded, Clay-owned fuzzy subsequence scorer for every transient
menu query path. Replaces the per-caller substring filters (`ControlCenter`
and the file browser) with deterministic subsequence ranking.

## Source

- `src/shell/fuzzy.rs` (registered in `src/shell/mod.rs`)
- Consumers: `src/server/control_center.rs` (`ControlCenter::session` query
  ranking), `src/shell/file_browser.rs` (`FileBrowserState::fuzzy_session`)
- `docs/reference/primitives/shell-layout-strategy.md` — transient menu
  family contract
- `plans/082-Phase24.2-Command-Execution-Mode.md`

## What it does

`fuzzy_score(query, candidate) -> Option<i32>` returns `None` unless every
query character occurs in the candidate in order (case-insensitive,
Unicode-aware); empty queries score zero. `fuzzy_score_fields(query,
candidates)` returns the best score across bounded searchable fields — the
Control Center scores label, ID, detail, and accessibility label per item
(`query_score` in `src/server/control_center.rs`).

Scoring rewards:

| Term | Value | Effect |
|------|-------|--------|
| `MATCH_SCORE` | 10 | Base per matched character |
| `WORD_BOUNDARY_BONUS` | 8 | Match starts a word (`_` or alphanumeric transition) |
| `CONSECUTIVE_BONUS` | 8 | Matched chars are adjacent in the candidate |
| `EARLY_POSITION_BONUS` | 4, decaying by index | Earlier matches outrank later ones |
| `GAP_PENALTY` | 1 per gap char | Denser matches outrank spread-out ones |

So `ccop` matches "Control Center Open" where a substring filter fails,
"Control" outranks "sc" for query `c` (word boundary), and `ab` in "ab"
outranks `ab` in "a-b" (consecutive). The DP runs one row per query
character over the candidate, tracking best-gap scores, so it is
`O(query_len × candidate_len)` on bounded inputs.

## Bounds and hot-path policy

- `MAX_INPUT_CHARS` (256): query and candidate are lowercased with
  Unicode-aware `char::to_lowercase` but only after capping to 256 chars,
  so a malformed or over-long label cannot turn menu filtering into
  unbounded work. Queries longer than 256 chars never match.
- File-browser entry scans take at most `MAX_FUZZY_ITEMS` (64) results
  after scoring; the Control Center scans the already-bounded catalogue
  (≤ `TRANSIENT_MENU_MAX_ITEMS`) once per query.
- Deterministic ties: `ControlCenter::session` sorts score descending, then
  label, then ID, then source order; an empty query keeps source order.
- No registry re-consultation, no package JavaScript, no IPC, no allocation
  beyond the two bounded vectors — scoring runs on the server outside
  paint/layout, and never in client hot paths.

## Why shared

Both the Control Center (command mode), the file-browser fuzzy-open, and
(Phase 24.3) the Path Browser filter use the same ranking, so query
behavior is consistent across every transient menu. The scorer is a small
generic module with no dependency; per-menu scoring forks and tunable fuzzy
weights were rejected (weights stay Clay-owned constants — Phase 24.2 task
11). The Path Browser scores its installed entry names only — filter-only
edits never touch the filesystem (see [Path Browser](path-browser.md)).

## Tests

- `src/shell/fuzzy.rs`: subsequence-when-substring-fails, word-boundary
  outranks interior, consecutive outranks gapped, non-subsequence returns
  `None`, case-insensitivity, Unicode (e.g. `Ä` vs `ä`) stays panic-free,
  empty query scores zero, over-long query returns `None`.
- `src/server/control_center.rs`: `catalogue_snapshot_is_not_rebuilt_for_query_updates`
  (no-match query yields empty items), `filtering_matches_label_id_binding_and_provenance`.
- `src/shell/file_browser.rs`: `file_browser_fuzzy_session_filters_locally`.

Run with:

```text
cargo test --lib shell::fuzzy --quiet
cargo test --lib control_center --quiet
cargo test --lib shell::file_browser --quiet
```

## Related

- [Control Center](control-center.md) — command-mode consumer
- [Workspace File Browser](workspace-file-browser.md) — fuzzy-open consumer
- [Path Browser](path-browser.md) — Phase 24.3 filter consumer
- [Transient Menu Session](transient-menu-session.md) — the session model
  the scorer ranks
- [Transient Menu Round Trip](transient-menu-round-trip.md) — server-owned
  menu transport
