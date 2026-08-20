---
date: 2026-08-18 17:58
status: approved
decision_about: "Single-manifest package loading"
proposed_by: "agent"
explicitly_approved_by_user: true
---

# Decision: Manifest contributions are the sole package data path

## Decision

Package `clay.contributions` in `package.json` become the single source of
package registration data. Imperative registration APIs
(`serverRegisterSyntaxGrammar`, `serverRegisterCompletionProvider`,
`serverRegisterModePattern`, `serverRegisterCommand`,
`serverRegisterComponentContribution`) stop being part of package load
ceremony; `loadEntry` exists only to execute code (import parse modules, wire
bridge factories). First-party native-grammar packages stop declaring
`syntaxGrammars` in `package.json` — the Rust `NativeGrammarDescriptor` is the
source of truth for Tier 1 grammars and style maps.

## Context

The 2026-08-18 review found three incompatible registration conventions across
the four language packages: empty-argument API calls reading from the manifest
(`@clay/rust`, `@clay/typescript`), fully explicit arguments plus a duplicated
`*PackageManifest()` literal of the whole `clay` field (`@clay/markdown`,
`@clay/typescript`), and every package re-declaring commands/keymaps/editor
rules in both `package.json` and `load.js`. For native grammars, the
package.json `styleMap` is dead data: the registry pre-registers
`FIRST_PARTY_NATIVE_GRAMMARS` with compiled style maps, and later package
registration is skipped (`is_shadowed_by_native_first_party`), so the two
copies can silently drift.

## Approval

- Proposed by: agent (review recommendation list)
- Approved by user: Yes
- Approval evidence: “Yes go ahead and log the decision items” — approving the
  proposed set: background axis, typography size ladder, capability presets,
  single-manifest package loading.

## Alternatives Considered

1. **Manifest contributions as the only data path; load entries execute only**
   — selected; removes ~80–120 lines per package, deletes duplicated manifest
     literals and empty-args calls, ends the "which copy wins?" ambiguity.
2. **Invert native ownership: Rust statics carry only grammar/query functions
   and read style maps from the package record** — deferred; attractive when
   third-party grammars arrive, but requires trusting package records for
   Tier 1 and a larger diff than dropping the dead copies.
3. **Keep imperative APIs and remove manifest contributions instead** —
   rejected; the manifest is already validated, fingerprinted, budgeted, and
   the natural place for declarative data; the imperative path is what
   duplicates it.

## Rationale and Evidence

- `packages/markdown/dist/load.js` — ~150-line `markdownPackageManifest()`
  duplicate of `package.json`'s `clay` field, plus stale
  `documentId: 1`/`sample.md` activation ceremony.
- `packages/rust/dist/load.js`, `packages/typescript/dist/load.js` —
  `serverRegisterSyntaxGrammar({})` and `serverRegisterCompletionProvider({})`
  empty-args calls that only trigger manifest reads.
- `src/server/syntax.rs:741` — `is_shadowed_by_native_first_party` skips
  package contributions for the four first-party languages, making their
  package.json `syntaxGrammars[].styleMap` inert.
- `src/server/ops/modes.rs` — imperative `serverRegisterModePattern` path
  duplicating what manifest `modes` contributions already provide.

## References

- `packages/{rust,typescript,javascript,markdown}/package.json` and
  `dist/load.js` — the duplication instances.
- `src/packages/bundled.rs` — fingerprinted bundled inventory.
- `src/server/syntax.rs` — native-first-party shadowing.
- Code review of 2026-08-18 (session), findings §"One style map too many" and
  §"Load ceremony".

## Consequences

- Imperative registration APIs remain available for user `init.js`
  configuration and runtime contributions, but stop appearing in first-party
  package load entries.
- First-party package.json files lose `syntaxGrammars` (Tier 1 owned by Rust
  descriptors); Tier 2/3 packages keep declaring grammars in the manifest.
- Load-entry documentation and package authoring docs must be updated to the
  execute-only contract; the `*PackageManifest()` duplicates are deleted.
- A later inversion (style maps owned by trusted package records) requires a
  separate decision referencing this log.
