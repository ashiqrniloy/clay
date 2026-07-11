---
date: 2026-07-11 14:18
status: approved
decision_about: "User-owned typography profiles and semantic package font roles"
proposed_by: "both"
explicitly_approved_by_user: true
---

# Decision: User-owned typography profiles and semantic package font roles

## Decision

Clay will provide three user-owned typography profiles—`monospace`, `proportional`, and `ui`—where each profile contains an ordered font-family fallback stack and a logical-pixel base size. Packages and modes may declare semantic font roles, but may not choose concrete font families or absolute sizes; Clay resolves each role through the active user configuration.

Typography is layout-affecting client state, transported separately from `ActiveTheme` and resolved through a dedicated client-side `TypographyRegistry`. One role selects both family and size. `core.code` defaults to monospace, `core.text` and Markdown default to proportional, Markdown code spans/blocks use monospace, and Clay/package component text defaults to UI unless a validated component role opts into document typography.

## Context

Clay's current text rendering has no font-family configuration. Editor text uses `TEXT_FONT_SIZE = 20.0`, status text uses `STATUS_TEXT_SIZE = 12.0`, and SDUI uses fixed body/title sizes. Parley layout applies `FontSize` but no configured `FontStack`; layout cache identity excludes configured typography. Editor scrolling and viewport calculations assume one fixed size multiplied by `1.4`.

Phase 18.15 added `StyleRegistry` for theme colors and text attributes, while Phase 18.16 is adding tiered syntax output. Neither phase defines who owns font choices or how code/prose ranges affect layout. Adding font roles after diagnostics and full language packages would force those later rendering contracts to be revised.

## Approval

- Proposed by: Both—the user specified three separately configurable fixed-width, variable-width, and UI families/sizes and package-declared fixed/proportional use; the agent mapped this requirement onto current rendering, protocol, package, and configuration architecture.
- Approved by user: Yes.
- Approval evidence: On 2026-07-11 the user asked to create an implementation plan for the full “Font configuration impact analysis” and explicitly requested a decision log. The supplied analysis included the recommended contract and defaults recorded above.

## Alternatives Considered

1. **Extend `ActiveTheme` and `StyleRegistry` with typography.** Rejected: themes own semantic colors/attributes, while installed font families and preferred sizes are user/client concerns that invalidate layout geometry. Combining them would conflate ownership and invalidation behavior.
2. **Expose concrete family and size overrides to packages.** Rejected: package appearance would override user preference, create inconsistent UI, and widen inert style contracts unnecessarily. Semantic roles preserve user ownership.
3. **Expose independent family-role and size-role properties.** Rejected: this permits incoherent combinations and doubles contract surface without a demonstrated use case. One role resolves both values.
4. **Use a single editor profile.** Rejected: proportional prose with monospace code spans/blocks is an explicit requirement and a normal mixed-document use case.
5. **Add a dedicated `TextPresentationSpan` protocol immediately.** Deferred in favor of extending syntax/semantic decoration output with an optional role and normalizing layout style runs. A separate primitive should be introduced only if overlap or ownership constraints prove the shared transport unsafe.
6. **Keep fixed `TEXT_FONT_SIZE × 1.4` geometry.** Rejected as a permanent model. Initial implementation may use a conservative shared line-height baseline derived from active document profiles, but should consume Parley metrics wherever available and preserve a path to metric-driven scrolling.
7. **Bundle/download fonts or add a font picker.** Rejected for this phase: family stacks plus Fontique/system fallback satisfy configuration without asset, licensing, network, or new UI scope.

## Rationale and Evidence

- Parley 0.6 supports `StyleProperty::FontStack`, `StyleProperty::FontSize`, and byte-range styles through its ranged layout builder. Fontique supplies installed-font discovery and glyph fallback.
- Typography changes text shaping, wrapping, caret geometry, hit testing, layout height, and scroll range. It therefore needs a revision in layout cache identity and layout—not render-only—invalidation.
- Package declarations remain inert data and preserve Clay's shell/rendering authority. No package JavaScript, renderer callback, raw CSS, native widget, or IPC work enters Masonry paint/input/layout paths.
- Ordered family arrays provide an unambiguous validated contract. Server validation checks shape and bounds, while the client resolves installed fonts because server and client may run on different machines.
- Generic fallbacks (`monospace`, `sans-serif`, `system-ui`) prevent a missing named family from making text unavailable. “Monospace” is semantic intent; Clay does not attempt to prove arbitrary named-font metrics.
- A separate atomic `setTypography` configuration call prevents clients from observing partially updated profiles and needs one protocol revision.

## References

- Context7 `/linebender/parley` — Parley rich-text layout, ranged attributes, and Fontique fallback documentation, queried 2026-07-11.
- Cargo-local dependency resolution: `cargo tree -i parley` resolved `parley v0.6.0` through Clay and Masonry 0.4.0.
- `src/editor/layout.rs` — current `FontSize`-only Parley builder and layout cache.
- `src/editor/surface.rs` — hardcoded editor size and uniform line-height/scroll calculations.
- `src/masonry_editor.rs` — hardcoded status size and connection-event invalidation.
- `src/masonry_sdui.rs` — Clay/package UI text layout sites.
- `src/shell/theme.rs` — current absolute body/title typography tokens.
- `src/protocol/decorations.rs` — syntax/semantic span transport to extend with optional semantic role.
- `src/protocol/mod.rs` — `ActiveTheme` and server message boundary.
- `src/client/mod.rs` — bootstrap and live connection-event state.
- `runtime/js/theme.ts` — existing `clay:theme` facade precedent.
- `decision-logs/2026-07-09-0352-tiered-tree-sitter-themable-syntax-vocabulary-theme-registry-and-opt-in-lsp.md` — theme ownership, inert package styling, layered decorations, and no-hot-path package execution.

## Consequences

- Users can configure family stacks and sizes for code, prose, and UI atomically through `~/.config/clay/init.js`.
- Packages gain semantic role declarations but no authority over user font choice.
- Editor layout cache, scrolling, viewport extraction, caret geometry, SDUI layout, accessibility bounds, protocol/bootstrap state, package contracts, docs, and tests require coordinated changes.
- Mixed-role documents become first-class and expose the Phase 1 uniform-line-height compromise; initial conservative geometry must not block later migration to Parley line metrics.
- Missing named fonts fall back locally and do not require server-side font discovery.
- Bundled fonts, remote fonts, font picker UI, independent font axes, and variable-font controls remain out of scope until demonstrated need.
- This work should be inserted after Phase 18.16 and before Phase 18.17 so diagnostics and full language packages build on role-aware layout.
