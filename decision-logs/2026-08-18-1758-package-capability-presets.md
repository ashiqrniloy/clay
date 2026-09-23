---
date: 2026-08-18 17:58
status: approved
decision_about: "Package capability presets"
proposed_by: "agent"
explicitly_approved_by_user: true
---

# Decision: Capability presets for package manifests

## Decision

Package manifests gain a `preset` field that expands into the standard
permission set, `apiDependencies`, extension points, and contribution
families for a package archetype. Initial presets: `code-mode`, `prose-mode`,
and `lsp-bridge`. Presets are shorthand only — validation still runs on the
expanded set, and explicit declarations that deviate from a preset remain
possible and win over preset defaults.

## Context

The 2026-08-18 review found that a code-language package today hand-declares
~150 lines of boilerplate (6 permissions, 10 `apiDependencies`, 6 extension
points, 5 contribution families, 5–7 load-entry registration calls) that is
95% identical across `@clay/rust`, `@clay/typescript`, `@clay/javascript`,
and `@clay/markdown`. This multiplies the four-sources-of-truth problem for
every new format and is the main friction behind the "new format = new
package" goal.

## Approval

- Proposed by: agent (review recommendation list)
- Approved by: user
- Approval evidence: “Yes go ahead and log the decision items” — approving the
  proposed set: background axis, typography size ladder, capability presets,
  single-manifest package loading.

## Alternatives Considered

1. **Manifest-level presets expanded at validation** — selected; removes
   copy-paste boilerplate without weakening the permission/validation model.
2. **Per-family preset granules (one for grammar, one for completion, …)** —
   rejected for now; more flexible but reintroduces combinatorial declaration
   the preset is meant to remove; add later if real packages need mixing.
3. **Keep explicit declarations only** — rejected; every new package clones
   today's boilerplate and drifts.

## Rationale and Evidence

- `packages/*/package.json` — near-identical permission/apiDependencies/
  extension-points blocks across the four language packages.
- `src/protocol/mod.rs` — behavior rules already encode the prose/code split
  (`core.text`/`core.code` builtins, `WordSeparatorPolicy::{Code,Prose}`),
  giving the mode registry the archetype distinction.
- `docs/reference/primitives/registry.md` — primitive contributions already
  grouped by capability families; presets map onto them.

## References

- `packages/rust/package.json`, `packages/markdown/package.json` — boilerplate
  instances to collapse.
- `src/packages/manifest.rs` (manifest parsing/validation) — expansion point.
- Code review of 2026-08-18 (session), §5 "Grouping: capability presets".

## Consequences

- New code-language packages become a manifest with `preset: "code-mode"` plus
  only their deviating declarations.
- Preset definitions live in manifest validation (Rust side) and are
  documented in package authoring docs and the generated registry.
- Presets never bypass permission enforcement or budgets; the expanded set is
  what gets validated and surfaced in package inspection UI.
- Adding a new preset is a versioned manifest-schema change.
