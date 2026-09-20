# Plan 131 — baseline gates (unmodified tree, 2026-09-20)

Task: "Baseline gates on the unmodified tree" (plan 131 task 1). No source edits
made before this run.

- Environment: HEAD `66648e3` ("WIP", dirty tree — in-flight plans 127–135 work,
  79 entries in `git status --porcelain`); rustc/cargo 1.96.1, node v24.19.0.
- All plan-131 deletion targets still present at baseline: `src/server/cross_domain.rs`
  (468 lines), `src/server/locks.rs` (271 lines), `src/server/folding.rs:95–161`
  registry, `clay-agent/dbg5.mjs`, `tests/suites/protocol.rs.tmp`.

## Gate result

`scripts/check.sh full` → **exit 0**, 331 s. Raw log:
`baseline-check-full.log`; exit code: `baseline-exit-codes.txt`.

| Stage | Command | Result |
| --- | --- | --- |
| audit | `cargo audit` | pass (9 allowed warnings, pre-existing advisory allowances) |
| fmt | `cargo fmt --check` | pass |
| check | `cargo check --all-targets` | pass, **0 compiler warnings** |
| clippy | `cargo clippy --all-targets -- -D warnings` | pass |
| test | `cargo test --all-targets --quiet` | pass — 1421 lib + 62 + 227 + 75 + 152 + 9 + 1 passed, 0 failed, 1 ignored |
| bench-compile | `cargo bench --no-run` | pass |
| bindings | `scripts/check-bindings.sh` | pass ("webview bindings up to date") |

Warning baseline: `cargo check --all-targets --message-format short` replays
**0** warnings (the module-level `#[allow(dead_code)]` suppressions are doing the
hiding; the audit-stage "9 allowed warnings" are RUSTSEC allowances, not rustc
lint output). Re-arming the lint (plan task 4) must therefore be measured against
0, and any newly surfaced lint is a new finding, not baseline noise.

## Grep evidence per doomed symbol (raw `grep -rn`, excludes target/.git/graft/node_modules)

| Symbol | Hits | Non-plan/code-review locations |
| --- | --- | --- |
| `cross_domain` | 62 | `src/server/cross_domain.rs`, `src/server/ops/mod.rs` (live `absorb_cross_domain_evaluation`), `src/server/ops/packages.rs:529` (same live call), `src/server/mod.rs`, `tests/rust_visibility_api_mapping.rs:89`, docs (see below) |
| `ScopedLockManager` | 20 | `src/server/locks.rs`, `src/server/mod.rs:117,788`, `src/server/tests.rs:91`, `docs/wiki/flows/document-leases-and-region-locks.md` |
| `ScopedLockTarget` | 21 | same files as `ScopedLockManager` |
| `FoldingRangeRegistry` | 12 | `src/server/folding.rs`, `docs/reference/clay-js-api/folding/server-publish-folding-ranges.md` |
| `DocumentFolds` | 5 | `src/server/folding.rs` only |

`graft grep` confirms the same shape: `cross_domain` 27 hits / 12 symbols, of
which only `absorb_cross_domain_evaluation` (`src/server/ops/mod.rs:493`) has a
production in-edge; `ScopedLockManager` 10 hits, all constructor call sites or
its own tests; `FoldingRangeRegistry`/`DocumentFolds` 0 in-edges.

## Findings that change later tasks (recorded now, not acted on)

1. `tests/rust_visibility_api_mapping.rs:89` lists `"src/server/cross_domain.rs"`
   in its file allowlist — deleting the module requires editing that guard test,
   or the mapping guard fails on a missing path.
2. Docs referencing deleted symbols (wiki/doc-guard work in tasks 2/3/7):
   `docs/development/architecture-ownership.md`,
   `docs/reference/primitives/package-security.md`,
   `docs/wiki/flows/document-leases-and-region-locks.md` (A4 page, 3 hits in
   `docs/wiki/modules/third-party-runtime-authority.md`),
   `docs/wiki/modules/embedded-js-runtime.md`,
   `docs/wiki/modules/persistent-runtime-hot-reload.md`,
   `docs/wiki/modules/react-sdui-package-ui.md`,
   `docs/reference/clay-js-api/folding/server-publish-folding-ranges.md`,
   `docs/reference/clay-js-api/api-inventory.toml`,
   `docs/generated/clay-js-api-registry.json`.
3. `src/server/locks.rs` contains only `ScopedLock*` items plus its own tests —
   no `RegionLock`/`EditableLease` — so whole-file deletion matches the live
   ownership model (task 3's A4 record is accurate).
4. Client `#[allow(dead_code)]` event structs have test-only references
   (`EditorCompletionRequestEvent` 8 hits, `EditorLanguageIntelligenceRequestEvent`
   5, `EditorSelectionQueryRequestEvent` 2) — those references must be removed
   with the structs in task 4.