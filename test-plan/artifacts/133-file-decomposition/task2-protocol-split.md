# Plan 133 task 2 — `src/protocol/mod.rs` family split (2026-09-21)

Task: "Split `src/protocol/mod.rs` into family modules". Pure type/module move —
no logic, wire-shape, or test-body edits (one doc line and one guard file were
updated for moved paths, below).

## Result

`src/protocol/mod.rs`: **3,832 → 163 lines**. It is now the module hub:
`pub mod`/`pub use` declarations for 22 family files, the `PROTOCOL_VERSION`
history, the 11 shared id aliases (`ClientId`, `DocumentId`, …), and the
`menu_session_id_serde` bridge helper (`src/protocol/menu.rs:226` still reaches
it through `crate::protocol::menu_session_id_serde`). No type is defined in it
beyond those shared cross-family aliases.

Method: a line-range extraction script (`task2-split-protocol.py`, kept here for
provenance) sliced every top-level item (with its attributes/doc comments) out
of the original file into its family. Every new family file starts with a short
module doc and `use super::*;` (same precedent as `src/packages/record/*.rs`),
so intra-family references resolve, and `mod.rs` re-exports each family, so
every `crate::protocol::X` caller path is unchanged.

## Module map (lines / struct+enum+type declarations)

| File | Lines | Decls | Family contents |
| --- | --- | --- | --- |
| `behavior.rs` (new) | 1,202 | 11 | `BehaviorManifest`, default keymaps/commands + `ctrl_*` helpers, `BehaviorScope`, `KeyBindingRule`, `KeyStroke`, `KeyCode`, `KeyModifiers`, `KeyBindingContext`, `CommandDeclaration`, `CommandAuthority`, `RoutingPolicy`, `LockScope`, and the 13 protocol tests (moved with their unit) |
| `editor_rules.rs` (new) | 656 | 20 | `EditOperation`, `EditorIntent`, word/paragraph/movement rules, `EditorBehaviorRules`, `EditorLayoutRules`, `WrapPolicy`, `EditorChrome`, `TextEditCapability`, enter/tab/pair/comment/electric/autocomplete rules |
| `typography.rs` (new) | 460 | 10 | `FontRole`, `DocumentFontRole`, `TextThemeOverride`, font/ligature budgets, `FontProfile`, `LigaturePolicy`, validation errors, `ActiveTypography`, `UiTypographyHierarchy` |
| `messages.rs` (new) | 598 | 6 | `ClientMessage`, `ServerMessage`, `ProtocolErrorCode`, `RegionLockConflict`, `LockOwner`, `EditRejection` |
| `caret.rs` (new) | 197 | 4 | `CaretShape`, `BlinkStyle`, `CaretStyle` + budgets/validation error |
| `theme.rs` (new) | 164 | 5 | `ActiveTheme`, `Appearance`, `ResolvedAppearance`, `WireDesignTokenValue`, `UiDesignTokenOverride` |
| `document.rs` (new) | 149 | 5 | `DocumentTextHead`, `DocumentChunkRejection`, `bounded_document_chunk_bytes`, `DocumentMetadata`, `FileErrorCode`, `DocumentAccess` |
| `shell.rs` (new) | 143 | 4 | `ShellPreferences`, `TabEntry`, `TabRegistrySnapshot`, `TabCommand` |
| `launcher.rs` (new) | 71 | 3 | `LauncherWorkspaceEntry`, `LauncherAgentEntry`, `LauncherEntries` |
| `agent.rs` | 1,492 | 24 | + `AgentSettingsFileInfo` (agent-settings family) |
| `diagnostics.rs` | 363 | 5 | + `DiagnosticSeverity`, `RuntimeDiagnostic` (also drops the now-local name from its `crate::protocol` import) |
| `mod.rs` | 163 | 11 | hub only (aliases + version + serde helper) |
| pre-existing families | 79–1,619 | 1–24 | `codec.rs` untouched (round-trip tests unchanged) |

## Verbatim-move check

Multiset of non-blank lines of the original `src/protocol/mod.rs`
(`git show HEAD:src/protocol/mod.rs`) against the whole new `src/protocol/` tree:
**0 lines missing**. Only additions are the family headers/`use super::*;` lines
and the moved `#[cfg(test)]` wrapper. No type body, doc comment, derive, or test
line was rewritten.

## Forced guard/doc updates (not wire changes)

- `tests/documentation_coverage.rs:242` parsed `ClientMessage`/`ServerMessage`
  variants out of `src/protocol/mod.rs`; it now reads
  `src/protocol/messages.rs` (family move is the only reason).
- The protocol-version history comment in `src/protocol/mod.rs` now spells the
  named cap (`FOLDING_RANGE_PAYLOAD_BUDGET_BYTES`) next to `FoldingRangeSet`;
  `tests/performance_budgets.rs::folding_and_inlay_payloads_deny_above_cap`
  requires any file whose production body names `FoldingRange` to carry the cap,
  and the moved enum left only the version-history mention in the hub. The
  variant in `messages.rs` kept its original cap doc line.
- No other guard or caller changed: no caller churn is needed because every
  family is glob re-exported from the hub.

## Gates and compile time

`scripts/check.sh full` → **exit 0**, 325 s, `PASSED` (`task2-check-full.log`).
Test totals identical to the task-1 baseline: **1996 passed, 0 failed, 1
ignored** (15 test targets). clippy `-D warnings` zero warnings; `cargo bench
--no-run` compiles; desktop stages and bindings green.

Compile-time A/B (incremental crate check after touching the protocol module,
steady state, 3 samples each):

| Tree | Samples |
| --- | --- |
| split (this change) | 4.71 s / 4.70 s / 4.65 s |
| pre-split `mod.rs` (stashed tree) | 15.66 s* / 4.62 s / 4.69 s |

\* first pre-split sample included the stashed file-set change (different
fingerprint); steady state is identical, so the split is compile-time neutral.

## Operational note for later tasks

Interrupting the suite while `tests/manual_smoke_docs.rs::plan118_…` runs leaves
`scripts/capture-ui-review.sh`, `target/debug/clay…`, `clay-desktop`, WebKit and
`portal_capture.py` processes alive; the next run's harness then blocks on the
live desktop and the test appears to hang (>60 s warning). Kill leftovers
(`pkill -f "capture[-]ui-review"`, `pkill -f "clay[-]desktop"`, …) before
rerunning; with a clean process table the test passes in ~50 s.
