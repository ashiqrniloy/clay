---
date: 2026-08-18 17:58
status: approved
decision_about: "Document typography size scale for decorated tokens"
proposed_by: "agent"
explicitly_approved_by_user: true
---

# Decision: Theme-owned typography size ladder for document tokens

## Decision

Document-side typography gains a bounded, theme-owned size-scale ladder keyed
by `TokenType` (heading levels, small/code), applied in the layout rebuild
next to the existing font-role override. Typography range overrides remain
font-role-based for packages; size scaling is a theme/registry concern, not a
package concern.

## Context

The 2026-08-18 review found that `VisibleTextStyleRun` can vary font role and
4 boolean attributes and color, but never point size. Markdown `Heading1..6`
decorations render at body size with only weight/color differences, so prose
has no typographic hierarchy and looks flat. The UI side already implements
the correct pattern (`UiTypographyHierarchy` with bounded ratios and
`UiTextVariant`); the document side needs the mirror of it.

## Approval

- Proposed by: agent (review recommendation list)
- Approved by user: Yes
- Approval evidence: “Yes go ahead and log the decision items” — approving the
  proposed set: background axis, typography size ladder, capability presets,
  single-manifest package loading.

## Alternatives Considered

1. **Bounded per-TokenType scale ladder in `StyleRegistry`/typography,
   theme-owned** — selected; mirrors the proven UI hierarchy, keeps sizes out
   of package hands, single source of scale.
2. **Extend package typography range overrides with arbitrary size values** —
   rejected; packages then control point sizes, complicating budgets, line
   metrics, and theme consistency; no current package needs it.
3. **No size axis (status quo)** — rejected; heading hierarchy is a core
   prose-rendering requirement and already flagged by the review as a top
   visual gap.

## Rationale and Evidence

- `src/editor/surface/decoration.rs` — `VisibleTextStyleRun` shape (role,
  attributes, color; no size).
- `docs/reference/primitives/typography.md` — range override is documented as
  font-role only.
- `src/editor/typography.rs` — `UiTypographyHierarchy`/`UiTextVariant`
   establish the bounded-ratio pattern on the UI side.
- `src/protocol/mod.rs` — `DocumentFontRole` exists but `Heading1..6`
  decorations render at the mode's base size.

## References

- `src/editor/typography.rs` — UI hierarchy to mirror.
- `src/editor/layout.rs` — rebuild applies per-run font stack/size/features;
   the ladder lands here.
- Code review of 2026-08-18 (session), finding §3.3.

## Consequences

- `StyleRegistry` gains scale entries (e.g. headings 1.0/0.87/0.75…, small
  0.9) bounded like the UI hierarchy; themes may override the ladder.
- Mixed-size lines invalidate the single `document_line_height =
  max(mono, prop) × 1.4` approximation; per-line metrics or a recalibrated
  uniform line height must be addressed with this work.
- No package API changes; scaling keys off the existing closed token
  vocabulary.
