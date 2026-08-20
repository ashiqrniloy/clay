---
date: 2026-08-18 17:58
status: approved
decision_about: "Decoration style background axis"
proposed_by: "agent"
explicitly_approved_by_user: true
---

# Decision: Decoration spans gain a background color axis

## Decision

`StyleSpec` and the decoration-to-paint pipeline gain an optional background
color axis alongside the existing foreground color, owned by the
`StyleRegistry`/theme single source of color. Foreground colors (including the
default syntax palette) become full-opacity text colors, not background-tint
alphas repurposed as washes.

## Context

The 2026-08-18 editor implementation review found that `StyleSpec` was
originally documented as a "background tint" color, but the paint path
(`normalize_visible_text_style_runs` → parley brush) now consumes it as the
foreground text brush. The built-in default theme still carries `0x55` (≈33%)
and `0x2f` (≈18%) alpha values designed for tint fills, so every
syntax-highlighted token renders washed out under the default theme. There is
also no way to express a background at all, which blocks fenced code-block
panels, quote tints, search-match highlighting, unused-symbol fades, and
inline diffs.

## Approval

- Proposed by: agent (review recommendation list)
- Approved by user: Yes
- Approval evidence: “Yes go ahead and log the decision items” — approving the
  proposed set: background axis, typography size ladder, capability presets,
  single-manifest package loading.

## Alternatives Considered

1. **Add an optional background axis to `StyleSpec` and keep foreground as the
   existing field** — selected; smallest protocol-compatible extension that
   unlocks all background use cases while keeping the theme as the single
   source of color.
2. **Keep single-axis color and encode backgrounds as new `DecorationKind`s**
   — rejected; multiplies kinds for what is one visual property and forces
   kind-first layering rules where layer stacking is really a paint property.
3. **Leave as-is and fix only the alpha values** — rejected; fixes the
   washed-out text bug but leaves search matches, code fences, and quote
   panels unpaintable.

## Rationale and Evidence

- `src/editor/theme.rs:160–235` — default syntax palette built from
  `Color::from_rgba8(r, g, b, 0x55)` / `0x2f` alphas.
- `src/editor/theme.rs:28` — `StyleSpec` doc comment still describes its color
  as a "background tint".
- `src/editor/surface/decoration.rs::normalize_visible_text_style_runs` —
  style color consumed as the foreground text brush.
- `src/editor/surface/mod.rs:7109` — `DecorationKind::SearchMatch` exists with
  layer ranking but no client paint path, because a background cannot be
  expressed.
- First-party theme packages ship opaque syntax colors, confirming opaque
  foreground is the intended model.

## References

- `src/editor/theme.rs` — default style registry and `StyleSpec`.
- `src/editor/surface/decoration.rs` — style-run normalization into layout.
- `src/protocol/decorations.rs` — `DecorationKind`, `TokenType`, `Modifiers`.
- Code review of 2026-08-18 (session), finding §3.1/§3.4.

## Consequences

- Default palette entries move to opaque foreground colors (bug fix for the
  washed-out default theme).
- `DecorationSpan`/visible-run normalization carries an optional background;
  painting fills run backgrounds before text.
- Search-match painting, fenced code-block panels, quote tints, and future
  LSP "unused" fades/diff tints become expressible without protocol churn.
- Themes may set both axes per token; budgets and chunk serialization need
  updating for the added optional field.
