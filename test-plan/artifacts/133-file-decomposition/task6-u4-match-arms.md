# Plan 133 task 6 (U4) — resolve the `clippy::match_same_arms` sites

**Result: 38 → 0.** The pedantic run over the finished tree (`cargo clippy
--all-targets -- -W clippy::pedantic`, clippy 0.1.98) reports zero
`match_same_arms` warnings, production and test-side; the standard gate
(`-D warnings`) is green too.

## Baseline (task 1 artifact, re-measured after tasks 2-5)

`test-plan/artifacts/133-file-decomposition/baseline-clippy-match-same-arms.txt`
listed 38 sites / 32 production. After the protocol/server/test-file splits the
same 38 sites sat in new locations (e.g. the `src/server/mod.rs:5890` test-side
site moved into `src/server/runtime_generation_tests.rs`); the re-measured list
is in `task6-u4-sites.md`.

## Method

Each site is a `match` whose arms have identical bodies. Clippy already emits
the exact merge for those (`"a" | "b"` on one pattern, delete the duplicate
arm), so the fix was taken from clippy's own machine-checkable suggestion set
rather than hand-merged:

1. `cargo clippy --all-targets --message-format=json -- -W clippy::pedantic`
   → every `match_same_arms` warning with its multi-span suggestion
   (merge-pattern edit + duplicate-arm deletions).
2. Apply the suggestion edits in **byte offsets**, descending per file.

Verification at each step: the merged pattern must be present, the deleted arm
must be gone, `cargo check --all-targets` must pass, and `cargo fmt --check`
must be clean.

### Mistake and recovery (kept in the record)

The first application used the JSON's `byte_start`/`byte_end` as *Python string*
indices on files containing non-ASCII characters (box-drawing separators in
comments, `§` in docs). The offsets are byte offsets, so every site after the
first non-ASCII character was spliced a few bytes early: 16 files were corrupted
(visible as `}─────────` and glued identifiers, and caught by `cargo check`).

Recovery: the 16 files whose only working-tree change was this task's edit were
restored from `HEAD` and the edits re-applied with `read_bytes()`/byte slicing;
the 6 files that also carry tasks 2-5 work (or are new/untracked:
`src/client/tests/*`, `src/protocol/caret.rs`, `src/server/runtime_generation_tests.rs`,
plus `src/client/mod.rs` and `src/protocol/agent.rs`) were repaired by hand
against the pre-edit spans recorded in the clippy JSON. A re-measured pedantic
run then reported exactly the 30 sites in the restored files; the 8 hand-handled
sites were gone, confirming the repairs. All 61 remaining edits were applied
byte-safely in one pass.

## Semantic safety

Merging arms is only sound because clippy proved the bodies identical; the
stronger checks recorded here are:

- **`src/shell/theme.rs` (12 sites, the bulk):** the core token map's key set is
  unchanged — 147 token keys / 112 unique aliases before and after, zero added,
  zero removed (`"text.primary" | "border.strong"` keeps both names).
- **`src/packages/record/documentation.rs` (command → permission map, the
  security-relevant one):** the merged `=> None` arm contains exactly the same
  command names as the five arms it replaced
  (`packages.serverLoadPackage`, `behavior.buildCodeEditingManifest`,
  `completion.completionTriggerCharactersFromEditorRules`, the seven
  `ui.serverRegister*`, `agent.profileRegister`, `agent.skillRegister`,
  `git.serverListGitStatuses`, `git.serverRefreshGitStatus`, `sdui.publishTree`)
  and every permission arm is untouched.
- The four sites whose merge collapsed the match to a single real arm
  (`tests/editor_performance.rs:562`, `src/client/tests/client_events.rs:398`,
  `:439`, `src/server/runtime_generation_tests.rs:2968`) then tripped the
  default `clippy::single_match` gate, so they became `if let` with the same
  body — the gate caught this, not the review.

## Doc guards updated (source-scanning guards, both directions)

- `tests/package_ui_conformance.rs::core_token_catalog_matches_tokens_md` now
  collects *every* quoted token on a `=> CoreThemeValue` line, because aliased
  tokens share one arm (`"text.primary" | "border.strong" =>`); the old
  first-quote/last-quote extraction produced `text.primary" | "border.strong`.
- `tests/primitives_docs.rs::phase20_1_token_catalog_is_complete_and_matches_core_registry`
  — same fix (walk every quoted name on the arm line).
- `tests/primitives_docs.rs::no_component_kind_or_token_renamed` — the token
  marker is now the quoted name (aliased arms have no `"token" => CoreThemeValue`
  text); see the U5 note for the component-kind marker change.

## Files

U4 touched 22 site files (16 restored + 6 hand-handled) plus those three guard
files. Diff sizes are in `task6-u4-sites.md`; the biggest are
`src/shell/theme.rs` (12 sites) and `src/packages/record/documentation.rs`.
