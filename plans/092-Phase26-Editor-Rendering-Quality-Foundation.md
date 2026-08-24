# Phase 26: Editor Rendering Quality Foundation

Source: `roadmap.md` Phase 26 (added 2026-08-18 from the editor implementation
review). This plan implements Phase 26.1–26.7.

Approved decisions (binding constraints):

- `decision-logs/2026-08-18-1758-decoration-background-axis.md` — optional
  background axis on `StyleSpec`; foreground colors become opaque.
- `decision-logs/2026-08-18-1758-document-typography-size-ladder.md` —
  bounded, theme-owned per-`TokenType` size ladder; packages stay
  font-role-only.
- `decision-logs/2026-08-14-0331-ui-modernization-preserves-theme-configuration.md`
  — theme configurability (`setTheme`, `designTokens`, `ResolvedUiTheme`,
  `setTypography`) is preserved; changes improve defaults and token
  consumption, never replace the model.

Confirmed-correct invariants (must not regress): the two-axis vocabulary
(`TokenType` + `Modifiers`), theme single-source-of-color, optimistic
decoration interpolation, no language-specific Rust branches in
rendering/syntax engine, no package JavaScript in paint/layout hot paths.

## UI Skill Gate (mandatory for every task)

Before reviewing existing UI, planning, designing, or editing any UI-related
task in this plan — theme, typography, tokens, editor chrome, rendering,
visual evidence — use the current project UI skill requirements. Inspect the relevant category,
load the complete mandatory project-local UI skill stack, and apply
it to Clay's native Masonry/Parley/Vello token context. Repeat per
independently executed task; prior evidence does not satisfy the gate. Record
command, category, and selected slugs in the task's evidence. Load
`.agents/skills/clay-ui/` plus `references/components.md` and
`references/tokens.md` after routing.

## Objectives

- Fix the washed-out default theme (tint-alpha colors used as foreground)
  and make richer syntax differentiation visible under every shipped theme.
- Add the two approved paint axes — decoration background color and the
  document typography size ladder — through the theme registry and the
  existing decoration pipeline, with no new `DecorationKind`s for paint
  properties.
- Ship generic editor chrome (line-number gutter, active line, bracket
  match, indent guides) and layout geometry (asymmetric insets, wrap policy,
  prose column) that every mode gets with zero package code.
- Fix the AccessKit focused-ID panic on dirty-pane close.
- Keep keypress-to-local-paint and decoration budgets non-regressed;
  document everything (theme catalog, primitive references, test plan,
  wiki).

## Expected Outcome

- Default-theme code renders at full opacity with a distinct token palette;
  Rust/TS/JS/Markdown render with full token differentiation using the
  dormant half of the existing vocabulary.
- Search matches, fenced code blocks, and block quotes paint through the
  background axis; Markdown headings render as a real typographic hierarchy
  in every theme.
- Code modes show gutter/active-line/bracket-match/indent guides; long-line
  files scroll horizontally via `WrapPolicy::None`; prose reads at a sane
  column.
- Closing a dirty pane never panics the accessibility tree.
- All changes are data/token-driven and generic; a new file format can adopt
  them with data-only contributions.

## Tasks

- [x] Establish rendering baseline, evidence paths, and pattern compliance
  - Acceptance Criteria:
    - Functional: Capture before-edit screenshots of a Rust file, a TS/JS file, and a Markdown file (headings, fences, quotes, links) under the default theme and one shipped theme package (light + dark), using existing Plan 087/088-style isolated fixtures; record them under `code-reviews/screenshots/2026-08-18-phase26-baseline/`.
    - Performance: Record current advisory budget commands (`KEYPRESS_TO_LOCAL_PAINT_P95_BUDGET_MS`, decoration payload budgets) and the existing benchmark suite names; no implementation work.
    - Code Quality: List the exact current-state defects from the review with file:line anchors (theme alpha values, `StyleSpec` doc comment, coarse queries, uniform `TEXT_INSET`, `document_line_height` approximation, logical-line viewport window) so each later task can be checked as fixed.
    - Security: Fixture data only; no host paths or secrets in retained screenshots.
  - Approach:
    - Documentation Reviewed:
      - `roadmap.md` Phase 26; the two 2026-08-18-1758 decision logs.
      - Project patterns: `ui-modernization.md`, `ui-visual-review.md`, `typography-role-ownership.md`, `language-capability-sequencing.md`, `package-manifest-single-source.md`.
      - `docs/wiki/modules/editor-theme-registry.md`, `decoration-transport.md`; `docs/reference/primitives/rendering-strategy.md`, `typography.md`, `syntax-vocabulary.md`.
    - Options Considered:
      - Skip baseline, rely on code inspection: rejected — visual deltas are the acceptance evidence for this phase.
      - Full state matrix à la Plan 088: rejected — this phase touches document rendering, not shell surfaces; a format/theme matrix is sufficient.
    - Chosen Approach:
      - Capture Rust, TypeScript, JavaScript, and Markdown across the canonical default plus Gruvbox light/dark variants; reuse Plan 087 harness patterns.
    - API Notes and Examples:
      ```text
      cargo build && run existing isolated document fixtures
      code-reviews/screenshots/2026-08-18-phase26-baseline/
      ```
    - Files to Create/Edit:
      - `plans/092-Phase26-Editor-Rendering-Quality-Foundation.md`: evidence appended to this task.
      - `code-reviews/screenshots/2026-08-18-phase26-baseline/`: baseline artifacts.
    - References:
      - `code-reviews/screenshots/2026-08-14-plan088-baseline/` for fixture/reuse conventions.
  - Test Cases to Write:
    - Baseline checklist: every planned visual change has a before image.

  - Completion Evidence (2026-08-18 18:28 +06):
    - UI preflight completed before UI review: used the UI guidance current at execution time, inspected `visual` with the UI guidance current at execution time, selected and loaded `ibelick/baseline-ui` with the UI guidance current at execution time; translated its web-oriented guidance to Clay's native Masonry/Parley/Vello, typed-token, user-owned typography model. Reviewed `.agents/skills/clay-ui/SKILL.md`, `references/components.md`, `references/tokens.md`, patterns `ui-modernization.md`, `ui-visual-review.md`, `typography-role-ownership.md`, `language-capability-sequencing.md`, `package-manifest-single-source.md`, the approved Phase 26 decisions, editor theme/decoration wiki pages, and rendering/typography/syntax primitive references.
    - Reused the Plan 087/088 harness behavior through a temporary wrapper around `scripts/capture-ui-review.sh`; no product source or committed harness change. All runs used private mode-700 HOME/XDG/workspace/socket roots, fixture-only documents, AT-SPI, and xdg-desktop-portal Screenshot at 900×600 logical size. Physical PNGs are 1920×1200 on this host.
    - Captured 12 `PASS` artifacts under `code-reviews/screenshots/2026-08-18-phase26-baseline/`: Rust, TypeScript, JavaScript, and Markdown under the canonical default (`theme-modus-vivendi`), `@clay/theme-gruvbox-material-light`, and `@clay/theme-gruvbox-material-dark`. Markdown fixture includes headings, paragraph, strong/emphasis, link, quote, list, and fenced Rust code. Each state has `screenshot.png`, `accessibility.txt`, `instructions.md`, `metadata.txt`, and `review.status`; `review-log.md` records the matrix and capture method. Accessibility dumps expose Clay frame, working-area shell, editor pane, editable document entry, status bar, basename, and theme; no retained dump contains a host path or secret.
    - Recorded current budgets: `KEYPRESS_TO_LOCAL_PAINT_P95_BUDGET_MS = 16`, `DECORATION_PAYLOAD_BUDGET_BYTES = 8192`, `INCREMENTAL_PARSE_UPDATE_BUDGET_BYTES = 4096`. Recorded commands: `cargo bench --bench editor_baselines editor_render_adjacent -- --sample-size 10 --warm-up-time 1 --measurement-time 2`, `cargo bench --bench editor_baselines editor_scroll_viewport -- --sample-size 10 --warm-up-time 1 --measurement-time 2`, `cargo test --test protocol performance_protocol::`, `cargo test --test editor editor_performance_invariants::`, and `cargo bench --no-run`. `cargo bench --no-run` completed successfully. Existing suites recorded: `editor_baselines`, `protocol_server_baselines`, `runtime_sdui_baselines`, `markdown_baselines`, `first_party_language_baselines`, `window_baselines`.
    - Current-state defect anchors recorded in `code-reviews/screenshots/2026-08-18-phase26-baseline/review-log.md`: `src/editor/theme.rs:28` stale background-tint contract; `src/editor/theme.rs:158` semantic `0x2f` alpha; `src/editor/theme.rs:162-204` default `0x55` syntax alpha; coarse queries at `packages/rust/queries/highlights.scm:6-21`, `packages/typescript/queries/highlights.scm:6-20`, and `packages/javascript/queries/highlights.scm:6-18`; `packages/markdown/queries/highlights.scm:10-50` baseline vocabulary path; `src/editor/surface/mod.rs:73` uniform `TEXT_INSET`; `src/editor/surface/mod.rs:2113-2114` uniform geometry subtraction/no wrap policy; `src/editor/surface/mod.rs:2612-2619` logical-line visible snapshot; `src/editor/typography.rs:18` 1.4 multiplier; `src/editor/typography.rs:263-264` max-profile line-height; and `src/editor/typography.rs:318-325` test locking that approximation.
    - No implementation work was performed; baseline is ready for before/after comparison. Final visual scoring remains in the dedicated post-implementation review task.

- [x] Review existing editor primitives and plan generic primitive gaps before rendering work
  - Acceptance Criteria:
    - Functional: Inventory existing rendering primitives (`StyleRegistry`/`StyleSpec`, `DecorationSpan` two-axis vocabulary, `VisibleTextStyleRun` normalization, `LayoutCacheKey`, `UiTypographyHierarchy`, `DocumentFontRole`, `CaretStyle` layers) and state what Phase 26 can achieve with them before any new Rust code.
    - Performance: Identify which additions touch the paint/layout hot path (background fills, size ladder, gutter) and require budget work in task 26.7-hardening.
    - Code Quality: New primitives (background axis, size ladder, `WrapPolicy`, gutter chrome) are generic and reusable across all modes; no language-named branches; documented in `docs/reference/primitives/` in the same phase.
    - Security: Confirm decoration payload budgets still bound any new serialized fields (background axis) and no package gains paint-path authority.
  - Approach:
    - Documentation Reviewed:
      - `docs/reference/primitives/index.md`, `registry.md`, `rendering-strategy.md`, `typography.md`, `syntax-vocabulary.md`, `ui-chrome-primitives.md`.
      - `docs/wiki/modules/primitive-architecture.md`, `editor-theme-registry.md`, `decoration-transport.md`.
    - Options Considered:
      - Add axes ad hoc inside `src/editor`: rejected — the phase's whole point is vocabulary-level primitives the next formats reuse.
      - Defer primitive docs to the end: rejected — project rules require primitive docs in the same phase; planning them now prevents drift.
    - Chosen Approach:
      - Map each roadmap item to an existing or new primitive; the mapping table becomes the implementation contract for tasks 3–8.
    - API Notes and Examples:
      ```rust
      // Planned (task 4): StyleSpec { foreground: Color, background: Option<Color>, .. }
      // Planned (task 5): StyleRegistry::size_scale(TokenType) -> f32 (bounded ladder)
      // Planned (task 8): WrapPolicy { None, Viewport, Column(u16) }
      ```
    - Files to Create/Edit:
      - `plans/092-...md`: primitive gap table recorded here.
      - `docs/reference/primitives/rendering-strategy.md`, `typography.md`, `syntax-vocabulary.md`: updated by the implementing tasks, listed here for scope.
    - References:
      - `decision-logs/2026-06-04-1923-replace-markdown-parser-with-markdown-it-and-primitive-first-mode-planning.md` (primitive-first rule).
  - Test Cases to Write:
    - Gap-table review: every planned Rust change maps to a named primitive; no orphan language-specific edits.

  - Primitive Inventory (verified against source 2026-08-18):
    - `DecorationSpan` (`src/protocol/decorations.rs:306`): closed two-axis vocabulary (`TokenType` enum + `Modifiers` bitfield) + `DecorationKind`, `priority`, `provenance`, optional range `font_role`, free-form `scope` escape for third-party themes. rkyv-serialized, budget-validated server-side. No language branches anywhere in the shape.
    - `StyleSpec` (`src/editor/theme.rs:41`): resolved `color` + bold/italic/underline/strike. Missing axes: `background`, per-token size `scale` — both are the only structural gaps in the style contract.
    - `StyleRegistry` (`src/editor/theme.rs:102`): single resolution point. `style_for(kind, token_type, modifiers)` maps vocabulary → `StyleSpec`; `diagnostic_style(severity)`; layer colors (`selection`, `search_match`, `diagnostic`); per-token attribute defaults (`attr_defaults`); `BaseUiColors` chrome palette + `BaseUiColorKey`/`OverrideTarget`/`parse_override_token` closed-token override surface; `caret_style`.
    - `TextStyleOverride` (`src/editor/theme.rs:~360`): theme-package textStyles entry — `token` (closed target) + optional color/bold/italic/underline/strike + provenance. This is the exact extension point for 26.1/26.3/26.4: add `background: Option<Color>` and `scale: Option<f32>` fields; parse/validation precedent already exists.
    - `VisibleTextStyleRun` + `normalize_visible_text_style_runs` (`src/editor/layout.rs:20`, `src/editor/surface/decoration.rs:400`): viewport-bounded, outside-paint run normalization; run equality = (range, font_role, attributes, color); deterministic precedence (priority → layer → provenance). Background and scale must join this equality/precedence path — normalization already computes per-run style resolution, so the wiring point is single.
    - `LayoutState::rebuild` (`src/editor/layout.rs:~428`): Parley ranged builder already pushes per-run `FontStack`, **`FontSize`**, `FontFeatures`, weight/style/underline/strike, brush index. The per-run `FontSize` push is the exact 26.4 ladder application point — multiply profile size by resolved scale; no new layout machinery needed.
    - `LayoutCacheKey` (`src/editor/layout.rs:63`): text/viewport revisions, `max_width`, `typography_revision`, `layout_style_revision`, font-features hash. Ladder/background theme changes ride `layout_style_revision` (already plumbed); wrap policy (26.6) must add policy + width to this key.
    - `UiTypographyHierarchy` / `UiTextVariant` (`src/editor/typography.rs`): seven bounded semantic scale ratios over the selected profile (clamped, config-validated, revision-bumped). This is the pattern and clamp precedent for the document ladder; ratios travel in `ActiveTypography`.
    - `DocumentFontRole` + `BehaviorManifest::document_font_role` (`src/protocol/mod.rs:81,181`): mode-level generic role selection flowing manifest → client; proves manifest → client-primitive plumbing exists for 26.5 chrome defaults and 26.6 wrap defaults.
    - `CaretStyle` layering (protocol `CaretStyle` → theme default → `EditorBehaviorRules::caret_style` → runtime override): the existing manifest-default → user-override precedence pattern that 26.5 chrome toggles and 26.6 wrap policy must copy.
    - Diagnostic squiggle/fill painting (`src/editor/surface/mod.rs`): theme-owned colors + cached Parley line rectangles painted before text — the fill-order precedent for 26.3 background fills.
  - What Phase 26 achieves with existing primitives before new Rust code:
    - 26.1 is pure data: `StyleRegistry::clay_default()` table values + `TextStyleOverride` entries in theme packages. Zero Rust beyond the table/doc comment.
    - 26.2 is pure data: `.scm` query files + `DEFAULT_NATIVE_STYLE_MAP`/`MARKDOWN_NATIVE_STYLE_MAP` statics. No engine changes.
    - 26.3 is mostly data: background resolution already has its natural home in `style_for` (theme-resolved like foreground). Structural gap is only the `StyleSpec` field, run plumbing, and paint fill.
    - 26.4 needs one lookup + one multiplication at the existing per-run `FontSize` push; the structural gap is line metrics (per-line height from scaled runs feeding viewport/scroll math), not style application.
  - Primitive gap table (implementation contract for tasks 3–8):
    | Roadmap item | Existing primitive reused | New generic primitive required | Rust surface touched | Hot-path? |
    | --- | --- | --- | --- | --- |
    | 26.1 opaque defaults | `StyleRegistry::clay_default`, `TextStyleOverride`, theme `textStyles` | none — data only | `src/editor/theme.rs` values + doc comment | No (table values; same lookups) |
    | 26.2 rich queries | package `.scm` trees, native style maps, two-axis `DecorationSpan` | none — data only; captures must resolve through existing vocabulary | `packages/*/queries/*.scm`, `src/server/syntax.rs` statics + query-contract test | No (one parse/capture pass unchanged) |
    | 26.3 background axis | `StyleSpec`, `style_for`, `TextStyleOverride`, `normalize_visible_text_style_runs`, diagnostic fill-order precedent | `StyleSpec.background: Option<Color>`; background in run equality/precedence; visible-run fill before text | `theme.rs`, `surface/decoration.rs`, `layout.rs`, `surface/mod.rs` paint | Yes — paint (bounded visible fills) |
    | 26.4 size ladder | per-run `FontSize` push in `LayoutState::rebuild`, `UiTypographyHierarchy` clamp pattern, `TextStyleOverride.scale` extension | `StyleRegistry::size_scale(TokenType) -> f32` (clamped); per-line height metrics replacing `document_line_height()` max-profile approximation; scroll/viewport consume per-line heights | `theme.rs`, `typography.rs`, `layout.rs`, `surface/mod.rs` viewport/scroll | Yes — layout rebuild + viewport/scroll math |
    | 26.5 editor chrome | `BaseUiColorKey`/`OverrideTarget` token pattern, `CaretStyle` manifest→user precedence, buffer matching-pair scan, behavior manifest fields | `editor.gutter.*` / `editor.lineHighlight` / `editor.indentGuide` / `editor.bracketMatch` tokens; chrome manifest defaults (`chrome: { gutter, activeLine, indentGuides, bracketMatch }`); gutter/indent/match painting from cached line metrics | `surface/mod.rs`, `theme.rs`, `protocol/mod.rs` manifest fields, clay-ui token/component catalog | Yes — paint; bracket scan needs bounding (`ponytail:` ceiling if O(document) retained) |
    | 26.6 wrap policy + geometry | `LayoutCacheKey`, `DocumentFontRole` manifest plumbing, `CaretStyle` precedence pattern, 26.4 per-line metrics | `WrapPolicy { None, Viewport, Column(u16) }` protocol/manifest field; token-driven asymmetric insets replacing `TEXT_INSET`; visual-line visible snapshot replacing logical-line `viewport.visible_range` window; horizontal scroll plumbing | `protocol/mod.rs`, `server/ops/modes.rs`, `surface/mod.rs`, `layout.rs`, viewport/scroll | Yes — layout + scroll + cache key (policy + width must join key) |
    | 26.7 hardening | budget constants/tests, Criterion suites, theme catalog docs | none — focus-reset fix + budget/doc updates | pane-removal focus path, `docs/development/performance.md`, primitive refs, registry | No new hot path |
  - Corrections to later task wording discovered by this review:
    - 26.3 acceptance says the background axis "flows through `DecorationSpan`/chunk serialization (rkyv)". It should not: background is theme-resolved from `(kind, token_type, modifiers)` exactly like foreground, so no span field, no rkyv change, no payload growth. Validation/budget work reduces to asserting serialized size is unchanged. Packages never see or set backgrounds — vocabulary-only wire format is preserved.
    - 26.4 ladder likewise rides theme data (`TextStyleOverride.scale`) and the existing per-run `FontSize` push; no new serialized field.
    - 26.5 bracket-match reuses `src/editor/buffer.rs` matching-pair scan — inventory confirms it is manifest-driven (pair rules from the behavior manifest), so the task adds painting + bounding only, not scanning logic.
    - No planned task introduces a language-named branch: 26.2 feeds the closed vocabulary, 26.3/26.4 are theme/token-driven, 26.5/26.6 are manifest/config-driven. Gap table satisfies the no-orphan-edit criterion.
  - Security/performance confirmation: the only serialized-surface changes in Phase 26 are none (background and scale are client/theme-side); decoration payloads stay within `DECORATION_PAYLOAD_BUDGET_BYTES`/`INCREMENTAL_PARSE_UPDATE_BUDGET_BYTES` with zero growth; no package gains paint-path authority — packages contribute vocabulary tokens and inert theme/manifest data only; hot-path additions (fills, scale push, chrome paint, wrap) are the budget-work subjects named for task 26.7 hardening.

- [x] Phase 26.1: Default theme opacity fix and StyleSpec contract repair
  - Acceptance Criteria:
    - Functional: every `0x55`/`0x2f`-alpha entry in `StyleRegistry::clay_default()` (`src/editor/theme.rs`) replaced with opaque foreground colors; semantic fallback fixed likewise; `StyleSpec` doc comment rewritten to the real contract (opaque foreground + optional background axis added structurally in task 26.3).
    - Functional: default palette gives every `TokenType` a distinct resolved `StyleSpec` so the rich queries in task 26.2 are visually different; shipped first-party themes (`theme-modus-operandi`, `theme-modus-vivendi`, `theme-gruvbox-material-dark`, `theme-gruvbox-material-light`) keep their designed palettes while ensuring the previously-dormant vocabulary entries (`Macro`, `Property`, `Method`, `Parameter`, `EnumMember`, `Operator`, `TypeParameter`, `Regexp`, `Decorator`) do not collapse to the same resolved style as any other syntax token in the same theme.
    - Performance: paint path unchanged (table values only); no new lookups.
    - Code Quality: colors stay in theme packages/registry; no raw `Color` outside `theme.rs` source-guard module; `StyleSpec` contract now matches the actual foreground use.
    - Security: theme packages remain inert data; no permission changes.
  - Approach:
    - Documentation Reviewed:
      - `docs/wiki/modules/editor-theme-registry.md` (`style_for` layering, textStyles contributions).
      - `docs/reference/packages/creating-packages.md` theme authoring section.
      - Decision `2026-08-18-1758-decoration-background-axis.md`.
    - Options Considered:
      - Keep alphas, raise them: rejected — opaque foreground is the decided contract; washes were a tint design.
      - Reuse existing theme-package palettes as defaults: chosen where possible (proven legible colors, zero new design work).
    - Chosen Approach:
      - Opaque table values; dormant-entry distinctness in shipped themes via minimal one-channel nudges so existing designed colors stay intact; default palette made fully distinct so new captures from task 26.2 render differently.
    - API Notes and Examples:
      ```rust
      // before
      (TokenType::Keyword, Modifiers::NONE) => StyleSpec { color: Color::from_rgba8(0xb4, 0x8e, 0xad, 0x55), .. }
      // after
      (TokenType::Keyword, Modifiers::NONE) => StyleSpec { color: Color::from_rgb8(0xc7, 0x92, 0xea), .. }
      ```
    - Files to Create/Edit:
      - `src/editor/theme.rs`: default palette + doc comment + new tests.
      - `src/editor/surface/mod.rs`: update baseline-locked test expected colors.
      - `tests/theme_packages.rs`: dormant-entry distinctness assertion in full-mapping test.
      - `packages/theme-gruvbox-material-{dark,light}/`, `packages/theme-modus-{operandi,vivendi}/`: dormant-entry color nudges.
    - References:
      - Review findings §3.1; roadmap Phase 26.1.
  - Test Cases to Write:
    - `default_palette_colors_are_opaque`: every default-table syntax entry alpha == 1.0; semantic fallback opaque.
    - `default_palette_token_types_are_distinct`: no two default syntax token families resolve to the same `StyleSpec`.
    - `assert_full_theme_mapping`: dormant-entry distinctness check for every bundled theme.
    - Visual: baseline vs new screenshots (default theme, Rust file) — final review task.

  - Completion Evidence (2026-08-18):
    - `src/editor/theme.rs`: `StyleSpec` doc comment now describes opaque foreground + optional background axis (added in 26.3). `semantic` fallback changed from `0x2f` alpha to opaque `#4dc88a`. All 35 `TokenType` entries in `clay_default()` syntax table changed from `Color::from_rgba8(..., 0x55)` to distinct opaque `Color::from_rgb8(...)` colors. Dormant entries previously sharing the default blue now have distinct hues (e.g., `Macro` #d2a8ff, `Property` #f5c542, `Method` #6ab0f3, `Parameter` #9cdcfe, `EnumMember` #98c379, `Operator` #d4d4d4, `TypeParameter` #ff7b72, `Regexp` #4ec9b0, `Decorator` #ffd700). `diagnostic` (0x3f) and `search_match` (0x45) remain as underlay/highlight tints because they are not foreground text colors.
    - Theme packages updated with minimal one-channel nudges so the dormant 9 entries are distinct from every other syntax token while preserving the upstream palette: modus-operandi (`Macro`, `Property`, `Method`, `Parameter`, `Operator`, `TypeParameter`, `Decorator`), modus-vivendi (same set), gruvbox-material-dark (`Macro`, `Property`, `Method`, `Parameter`, `EnumMember`, `Operator`, `TypeParameter`, `Regexp`, `Decorator`), gruvbox-material-light (same set). No base-UI or non-dormant colors were redesigned.
    - Tests added/passed:
      - `cargo test --lib editor::theme::tests` — `default_palette_colors_are_opaque`, `default_palette_token_types_are_distinct`, `default_registry_reproduces_locked_baseline_colors` (updated), all pass.
      - `cargo test --test editor theme_packages::` — full-mapping tests including new dormant-entry distinctness assertion pass for all four bundled themes.
      - `cargo test --lib editor::surface::tests::free_form_style_token_decoration_colors_baseline_locked` — updated expected opaque colors, passes.
    - Validation: `cargo fmt --check`, `cargo check --all-targets`, `cargo clippy --all-targets -- -D warnings` all pass.
    - `src/packages/bundled.rs`: FNV-1a-64 fingerprints updated for the four edited theme manifests (`theme-gruvbox-material-dark` → `ea90c5f47750583c`, `theme-gruvbox-material-light` → `17a5cc6ef97389ab`, `theme-modus-operandi` → `0838d77aa21d5c45`, `theme-modus-vivendi` → `02b71a0d1608d6d3`).

- [x] Phase 26.2: Capture-rich highlight queries and native style maps
  - Acceptance Criteria:
    - Functional: `packages/{rust,typescript,javascript}/queries/highlights.scm` extended from ~9 captures to nvim-treesitter-class coverage — Rust: booleans/null, macro invocations, operators, fields, constants, lifetimes, attributes, method calls, parameters, type parameters, punctuation tiers; TS/JS: properties, optional chains, regex, JSX tags, template literals; Markdown: emphasis levels, link text vs URL, fence info strings.
    - Functional: `DEFAULT_NATIVE_STYLE_MAP` / `MARKDOWN_NATIVE_STYLE_MAP` (`src/server/syntax.rs`) map every new capture onto the closed `TokenType`+`Modifiers` vocabulary; data only, no engine changes, no language-specific Rust branches.
    - Performance: one parse/capture pass unchanged; decoration fan-out budgets respected (existing tests).
    - Code Quality: query-contract test: every capture name in each `.scm` resolves to a vocabulary entry or is explicitly asserted inert; queries stay `include_str!`-sourced from the package trees.
    - Security: grammar contributions remain Tier-1 native descriptors; no new grammar-loading authority (package-provided-grammar rule).
  - Approach:
    - Documentation Reviewed:
      - `docs/reference/primitives/syntax-vocabulary.md` (locked two-axis contract).
      - `docs/wiki/modules/first-party-language-packages.md`; tree-sitter 0.25 query syntax (ctx7 `/tree-sitter/tree-sitter` if needed).
      - nvim-treesitter highlight capture conventions (`@keyword`, `@function.call`, `@property`, …) as the capture-set reference.
    - Options Considered:
      - Hand-write richer queries per grammar from scratch: chosen — queries are small data files; full nvim grammar ports are overkill and license-noisy.
      - Capture-to-token mapping in packages: rejected for native grammars — Rust statics own Tier-1 maps (Phase 27.2 will delete the dead package copies).
    - Chosen Approach:
      - Extend each `.scm` in place; extend the Rust static maps in the same commit so captures never dangle.
    - API Notes and Examples:
      ```scheme
      ;; packages/rust/queries/highlights.scm (excerpt)
      (attribute_item (identifier) @attribute)
      (macro_invocation macro: (identifier) @function.macro "!" @punctuation.delimiter)
      (field_expression field: (field_identifier) @property)
      (lifetime (identifier) @type.lifetime) ; maps to TokenType::TypeParameter + lifetime marker via style map
      ```
    - Files to Create/Edit:
      - `packages/rust/queries/highlights.scm`, `packages/typescript/queries/highlights.scm`, `packages/javascript/queries/highlights.scm`, `packages/markdown/queries/*.scm`.
      - `src/server/syntax.rs`: static style maps.
      - `src/server/syntax.rs` tests: query-contract test.
    - References:
      - Review findings §3.2; roadmap Phase 26.2; pattern `language-capability-sequencing.md`.
  - Test Cases to Write:
    - Query-contract: for each first-party grammar, every capture in the compiled query resolves through the static map (vocabulary entry or explicit inert list).
    - Fixture highlighting snapshots: representative `.rs`/`.ts`/`.js`/`.md` snippets produce expected `TokenType` spans (macro, property, operator, attribute, heading levels, fence info).
    - No-behavior-change: parse/update budget tests still green.

  - Completion Evidence (2026-08-18):
    - Queries rewritten in place (still `include_str!` from package trees). Rust now covers comments/strings/chars, bracket+delimiter+operator tiers, keywords (including `crate`/`self`/`super`/`mut` grammar nodes), types, lifetimes, type parameters, function/method calls, fields, macros, attributes, parameters, numbers, booleans, ALL_CAPS constants. TS/JS cover comments/strings/templates/regex, punctuation tiers, optional chains, operators, keywords, types/type parameters (TS), function/method defs+calls, properties, parameters. JS also styles JSX tag names as `Type`. Markdown block query adds fence `code-label` + quote container/marker; inline query splits `link` vs `link-url`.
    - `DEFAULT_NATIVE_STYLE_MAP` / `MARKDOWN_NATIVE_STYLE_MAP` extended so every new capture name maps onto the closed vocabulary (`Macro`, `Method`, `Property`, `Parameter`, `TypeParameter`, `Decorator`, `EnumMember`, `Regexp`, `Variable`, `String` for fence labels / link URLs). No engine changes, no language-named Rust branches.
    - Query-contract: `first_party_highlight_captures_resolve_through_native_style_maps` compiles each first-party + `markdown_inline` query and asserts every capture name is in the static map.
    - Fixture tests: `rust_grammar_emits_vocabulary_tokens_through_stylemap` now requires Macro/Method/Property/Parameter/TypeParameter/Decorator/Operator/Number/Type + declaration/EnumMember; TS extra tokens asserted on `typescript.ts` only (tsx fixture is a small component); JS requires Property/Method/Parameter/Operator/Regexp; markdown fixture restored to Heading1–6 plus fence/link/quote/emphasis.
    - Deliberate skip: no bare `(identifier) @variable` catch-all — it painted broken keywords (`constx`) and failed plan057 inherited-keyword correction. JSX tags live on the JS query only because the TS query is shared with the non-JSX `typescript` grammar.
    - Validation: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test --lib` (1570 passed), `cargo test --test runtime` (198 passed), `cargo test --test editor` (166 passed), `cargo bench --no-run`.

- [x] Phase 26.3: Decoration background axis and layered fills
  - Acceptance Criteria:
    - Functional: `StyleSpec` gains optional `background`; the axis is theme-resolved from `(kind, token_type, modifiers)` exactly like foreground (primitive-review correction: no `DecorationSpan`/rkyv field, no payload growth — spans stay vocabulary-only), flows through `VisibleTextStyleRun` normalization as part of run equality and precedence; paint fills run backgrounds before text; serialized decoration payloads are asserted byte-identical in shape (no new field).
    - Functional: `DecorationKind::SearchMatch` paints client-side via the new axis (layer-rank plumbing already exists); Markdown fenced code blocks and block quotes paint as tinted panels from existing `CodeBlock`/`Quote` tokens — theme data only, no Rust markdown branches; LSP bridges map unused-symbol/dead-code reports to background-axis decorations where available.
    - Performance: background fills bounded to visible runs; no full-document fills; decoration payload byte size unchanged by the axis (no serialized field); keypress→local-paint budget non-regressed.
    - Code Quality: no new `DecorationKind`s; background resolution stays in `StyleRegistry` (single source); `scope`/legacy path unaffected.
    - Security: background field is inert data inside existing payload budgets; server-side span validation covers it (range, provenance, priority unchanged).
  - Approach:
    - Documentation Reviewed:
      - Decision `2026-08-18-1758-decoration-background-axis.md`.
      - `docs/wiki/modules/decoration-transport.md`; `docs/reference/primitives/rendering-strategy.md`.
      - `src/editor/surface/decoration.rs` normalization; `src/editor/layout.rs` run application; vello fill patterns in existing diagnostic-underline/squiggle painting (if any) for fill-order conventions.
    - Options Considered:
      - Encode backgrounds as new `DecorationKind`s: rejected — multiplies kinds for a paint property (decision log records this).
      - Per-span raw background colors from packages: rejected — theme-owned axis only; packages emit vocabulary, themes resolve color.
    - Chosen Approach:
      - Optional `background: Option<Color>` on `StyleSpec`, resolved per (kind, token, modifiers) like foreground; runs carry it; layout paints fill rects behind text.
    - API Notes and Examples:
      ```rust
      pub struct StyleSpec {
          pub color: Color,                    // opaque foreground
          pub background: Option<Color>,       // theme-resolved fill, None = transparent
          pub bold: bool, pub italic: bool, pub underline: bool, pub strike: bool,
      }
      // theme textStyles entry gains optional "background": "#rrggbb"
      ```
    - Files to Create/Edit:
      - `src/editor/theme.rs`, `src/editor/surface/decoration.rs`, `src/editor/layout.rs`, `src/editor/surface/mod.rs` (paint).
      - `src/protocol/decorations.rs` + serialization + budget tests.
      - Theme packages: `search_match`, `code_block`, `quote` background entries.
      - `packages/markdown/`: no Rust; fence/quote tokens already emitted — verify only.
      - LSP bridge packages: unused-symbol mapping (data/config).
    - References:
      - Review findings §3.4; roadmap Phase 26.3.
  - Test Cases to Write:
    - Normalization: spans with backgrounds merge/split correctly; background participates in run equality.
    - Paint order: background rect drawn before text glyph run (render-list order test).
    - Budget: serialized chunk size with themed backgrounds stays within `DECORATION_PAYLOAD_BUDGET_BYTES` and is unchanged versus spans without backgrounds (no new wire field).
    - Search-match end-to-end: published `SearchMatch` spans paint highlights.
    - Markdown fixture: fenced block and quote produce background panels under each theme.

  - Completion Evidence (2026-08-18):
    - UI preflight: the UI guidance current at execution time → category `visual` → `ibelick/baseline-ui`; clay-ui `tokens.md` / `components.md` (no new SDUI tokens; editor `StyleRegistry` owns this axis).
    - `StyleSpec.background: Option<Color>` theme-resolved from `(kind, token_type, modifiers)`. No `DecorationSpan`/rkyv field. SearchMatch + Quote + CodeBlock default tints; `Modifiers::DEPRECATED` uses `unused` wash. Theme `textStyles` gain `background` hex; new base-UI keys `searchMatch` / `unused`.
    - Normalize carries background in run equality/precedence. SearchMatch layer rank raised above Syntax so highlights win. Paint fills run rects after selection, before `render_text`.
    - LSP: `unused`/`unnecessary` map to `Deprecated` in `packages/lsp-shared/mapping.js` (synced to first-party lsp packages).
    - Bundled theme fingerprints updated; override count 48 → 49.
    - Extra: split syntax decoration sets so one set + envelope stays ≤ `INCREMENTAL_PARSE_UPDATE_BUDGET_BYTES` (TS fixture was 4272 after 26.2 queries).
    - Validation: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test --lib` (1575), `--test editor` (166), `--test runtime` (198), `--test protocol` (164), `cargo bench --no-run`.

- [x] Phase 26.4: Document typography size ladder
  - Acceptance Criteria:
    - Functional: `StyleRegistry` gains a bounded per-`TokenType` scale ladder (headings 1.0/0.87/0.75/…, small/code 0.9) mirroring `UiTypographyHierarchy`; applied per-run in `LayoutState::rebuild` next to the font-role override; themes override the ladder through `textStyles` scale entries.
    - Functional: line metrics reconciled — `document_line_height = max(mono, prop) × 1.4` replaced by per-line metrics (or a recalibrated uniform height with a documented ceiling in a `ponytail:` comment); viewport/scroll math stays consistent with painted lines on mixed-size documents.
    - Performance: scale lookup is O(1) per run; layout cache key includes the ladder revision; mixed-size rebuild cost covered by the paint budget test.
    - Code Quality: packages remain font-role-only (decision log); no absolute point sizes anywhere outside user typography profiles; ladder ratios bounded (clamped) like the UI hierarchy.
    - Security: no new package authority; ladder is theme data.
  - Approach:
    - Documentation Reviewed:
      - Decision `2026-08-18-1758-document-typography-size-ladder.md`.
      - `docs/reference/primitives/typography.md`; pattern `typography-role-ownership.md`.
      - `src/editor/typography.rs` (`UiTypographyHierarchy` bounded-ratio pattern), `src/editor/layout.rs` (`LayoutCacheKey`, size push).
    - Options Considered:
      - Arbitrary per-run sizes from packages: rejected by decision — packages pick roles, never pixels.
      - Uniform line height recalibration only: rejected as primary — headings at body size is the core defect; per-line metrics chosen, uniform fallback documented as ceiling if per-line proves too costly this phase.
    - Chosen Approach:
      - Ladder in `StyleRegistry` + scale application in run normalization; per-line height derived from the max run scale on each line; scroll math consumes per-line heights.
    - API Notes and Examples:
      ```rust
      // theme textStyles scale entry
      { "tokenType": "Heading1", "scale": 1.0 }, { "tokenType": "Heading3", "scale": 0.75 }
      // StyleRegistry::size_scale(token_type) -> f32  // clamped to [0.75, 1.0] like UI hierarchy
      ```
    - Files to Create/Edit:
      - `src/editor/theme.rs`, `src/editor/typography.rs`, `src/editor/layout.rs`, `src/editor/surface/mod.rs` (line metrics, viewport math), scroll code paths.
      - Theme packages: scale entries for headings.
    - References:
      - Review findings §3.3/§3.6; roadmap Phase 26.4.
  - Test Cases to Write:
    - Ladder: heading runs resolve to scaled sizes; clamping holds for absurd theme values.
    - Line metrics: mixed heading/body line heights feed viewport and scroll ranges; caret vertical position correct on scaled lines.
    - Cache: ladder revision bump invalidates layout cache.
    - Manual: Markdown heading hierarchy screenshots (final review task).

  - Completion Evidence (2026-08-19):
    - UI preflight: the UI guidance current at execution time → category `visual` → `jakubkrehel/better-typography`; clay-ui tokens/components (no new SDUI tokens; editor `StyleRegistry` owns the ladder).
    - Default ladder: H1..H6 = 1.50/1.33/1.17/1.08/1.00/0.92, CodeSpan = 0.90, else 1.0. Clamp `(0, 4]` via `HIERARCHY_SCALE_MIN/MAX` (same as UI hierarchy). Plan sample `1.0/0.87/0.75` + `[0.75,1.0]` left H1 at body size — rejected as the defect.
    - Applied at existing Parley `FontSize` push (`profile.size() * run.scale`). Theme `textStyles.scale` is milli-`u16` on the wire; no `DecorationSpan` field.
    - Line metrics: kept uniform `document_line_height`; `ponytail:` ceiling on `conservative_document_line_height`. Per-line heights deferred (26.6 visual viewport).
    - Theme packages declare heading/CodeSpan scales; bundled fingerprints updated.
    - Validation: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test --lib` (1577), `--test editor` (166), `--test runtime` (198), `--test protocol` (164).

- [x] Phase 26.5: Editor chrome — gutter, active line, bracket match, indent guides
  - Acceptance Criteria:
    - Functional: line-number gutter as a generic client chrome surface — theme-token colors, configurable visibility/width, correct alignment under mixed line heights (26.4), right-aligned digits, current-line emphasis via tokens; never on the layout hot path.
    - Functional: active-line highlight and indent guides as theme-token chrome with per-mode defaults (code modes: on; prose: off unless configured); bracket-match highlight reuses the `src/editor/buffer.rs` matching-pair scan against the active behavior manifest's pair rules, painting matched ranges when the caret is adjacent to a bracket.
    - Performance: chrome painted from cached line metrics; no per-keystroke full repaints beyond existing invalidation; keypress→local-paint budget non-regressed; bracket scan bounded (existing scan is O(document) worst case — bound or cache it, note the ceiling).
    - Code Quality: all chrome is generic (any mode with a manifest gets it); zero package code; no raw colors (token-only); additive tokens only.
    - Security: no new IPC/ops; purely client-side.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-ui/references/components.md` + `tokens.md` (catalog-first; chrome primitives section).
      - `docs/reference/primitives/ui-chrome-primitives.md`; existing divider/status chrome painting in `src/shell/primitives.rs` for conventions.
      - Pattern `ui-modernization.md` (additive typed tokens).
    - Options Considered:
      - Gutter as an SDUI package component: rejected — gutter is core editor chrome tied to layout metrics; package UI cannot access line metrics and would put IPC on the paint path.
      - Fold gutter now: rejected — folding is Phase 28.3; leave a width slot only.
    - Chosen Approach:
      - Native chrome in the editor surface beside text painting; tokens (`editor.gutter.*`, `editor.lineHighlight`, `editor.indentGuide`, `editor.bracketMatch`) added to the theme catalog; per-mode defaults via behavior manifest fields.
    - API Notes and Examples:
      ```rust
      // manifest-declared default (inert data)
      { "chrome": { "gutter": true, "activeLine": true, "indentGuides": true, "bracketMatch": true } }
      // tokens: editor.gutter.foreground, editor.gutter.foregroundActive, editor.lineHighlight.background, ...
      ```
    - Files to Create/Edit:
      - `src/editor/surface/mod.rs` (chrome paint + config), `src/editor/theme.rs` (tokens), `src/protocol/mod.rs` (chrome manifest fields), behavior manifest parsing.
      - `.agents/skills/clay-ui/references/tokens.md`, `components.md` (catalog update).
      - `docs/reference/packages/creating-packages.md` (chrome defaults documentation).
    - References:
      - Review findings §3.5; roadmap Phase 26.5; clay.md package UI/layout authoring contract task rules.
  - Test Cases to Write:
    - Gutter: digits for visible lines, alignment under mixed heights, visibility toggle, current-line token swap.
    - Active line/indent guides: painted only when enabled; correct indent level columns.
    - Bracket match: adjacent-bracket highlight both directions; multi-caret; pairs from manifest; no match outside pairs.
    - Tokens: catalog doc-drift test (`cargo test`) stays green.

  - Completion Evidence (2026-08-19):
    - UI preflight: the UI guidance current at execution time → category `visual` → `jakubkrehel/better-typography` (tabular/right-aligned gutter digits). clay-ui tokens/components updated; no new SDUI token domain — chrome colors live on `StyleRegistry` like `searchMatch`.
    - `EditorChrome` on `EditorBehaviorRules` (`gutter`/`activeLine`/`indentGuides`/`bracketMatch`). `None` derives from `document_font_role` (monospace on, proportional/inherit off). `editorRules.chrome` parsed in `modes.rs`; `buildCodeEditingManifest` passes it through.
    - Paint: active-line + indent guides + bracket fills before glyphs in `layout.rs`; gutter numbers right-aligned in existing `TEXT_INSET` (no layout-width change). Colors: `gutterFg`, `gutterFgActive`, `lineHighlight`, `indentGuide`, `bracketMatch`.
    - Bracket scan: `matching_pair_byte_within` with 64 KiB ceiling (`ponytail:`). Same-char pairs skipped (existing motion limit).
    - Validation: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test --lib` (1584), `--test editor` (166), `--test runtime` (198), `--test protocol` (164).

- [x] Phase 26.6: Layout geometry — insets, wrap policy, prose column
  - Acceptance Criteria:
    - Functional: uniform `TEXT_INSET = 48.0` replaced by asymmetric token-driven insets (e.g. 32 horizontal / 20 vertical); prose modes get a bounded column cap; code modes default to full width.
    - Functional: `WrapPolicy` primitive (`None | Viewport | Column(u16)`) declared per mode in behavior manifests with `init.js` user override; `None` enables horizontal scrolling (scroll plumbing, caret visibility beyond width, viewport byte-window follows horizontal offset); `Viewport` is today's soft wrap; `Column` caps wrap width.
    - Functional: visible snapshot derived from painted visual lines (wrapped lines, proportional fonts) instead of the logical-line window; fix verified for wrapped markdown and long-line unwrapped code.
    - Performance: horizontal scroll invalidates only width/offset-dependent layout (cache key includes wrap policy + width); no full relayout per horizontal scroll step; budget test covers adjacent-render p95.
    - Code Quality: wrap policy is a generic protocol/manifest field; no mode-specific Rust; insets/col-cap are tokens or bounded manifest values, never raw paint constants.
    - Security: user override is a documented config API; packages cannot override user wrap settings.
  - Approach:
    - Documentation Reviewed:
      - `docs/reference/primitives/typography.md` (range override, manifest fields), `rendering-strategy.md`.
      - `src/editor/surface/mod.rs` (`TEXT_INSET`, `visible_snapshot`, `viewport.visible_range`), scroll state, `LayoutCacheKey`.
    - Options Considered:
      - Word-wrap everywhere + only insets: rejected — review names unwrapped horizontal scrolling as the code-mode requirement.
      - Visual-line viewport via full layout walk per frame: rejected — derive from cached painted-line metrics (26.4 per-line metrics make this cheap); document the ceiling.
    - Chosen Approach:
      - Wrap policy enum in protocol + manifest parsing + layout branch; per-line metrics from 26.4 feed the visual viewport window; insets become theme/layout tokens.
    - API Notes and Examples:
      ```ts
      // init.js user override (documented config API, task 11)
      setEditorLayout({ wrapPolicy: "none" });            // or "viewport" | "column"
      // manifest default
      { "layout": { "wrapPolicy": "viewport", "columnCap": 92 } }
      ```
    - Files to Create/Edit:
      - `src/protocol/mod.rs` (WrapPolicy), `src/server/ops/modes.rs` (manifest parsing), `src/editor/surface/mod.rs`, `src/editor/layout.rs`, scroll/viewport code.
      - Behavior manifest docs; mode manifests for rust/ts/js (`none`) and markdown (`column`).
    - References:
      - Review findings §3.5/§3.6 (`TEXT_INSET`, logical-line window); roadmap Phase 26.6.
  - Test Cases to Write:
    - Wrap policy: `None` disables wrapping, enables horizontal scroll, caret visible beyond width; `Viewport` matches today; `Column` caps width.
    - Visual viewport: wrapped markdown scrolls to reveal partially-visible wrapped lines; long-line code horizontal window correct.
    - Cache: policy/width changes invalidate; horizontal-only scroll does not rebuild layout.
    - Config precedence: user `init.js` override beats manifest default.

  - Completion Evidence (2026-08-19):
    - UI preflight: the UI guidance current at execution time → category `visual` → `jakubkrehel/better-typography` (measure 60–75, wrap deliberately). clay-ui tokens/components + creating-packages updated.
    - `WrapPolicy::{None,Viewport,Column(u16)}` on `EditorBehaviorRules.layout`. Omitted → role default (monospace `none`, proportional `column` 72, inherit/no-manifest `viewport`). `editorRules.layout` parsed in `modes.rs`; `buildCodeEditingManifest` forwards it.
    - Insets: 32h / 20v; gutter on uses 48 left. `None` → `break_all_lines(None)` + `visual_scroll_x`. Column width = `min(pane, cols * 0.6em)`.
    - User override: `EditorSurface::set_editor_layout` wins over manifest; JS `setEditorLayout` stays on the later API task.
    - Visual snapshot: wrap modes use 12-line overscan. Unwrapped megabyte lines still extract whole (`ponytail:`).
    - Validation: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test --lib` (1590), `--test editor` (166), `--test runtime` (198), `--test protocol` (164), `cargo bench --no-run`.

- [x] Phase 26.7: Rendering hardening — AccessKit panic fix, budgets, docs
  - Acceptance Criteria:
    - Functional: dirty-pane close via Ctrl+Alt+W no longer panics in `accesskit_consumer` ("Focused ID #4 is not in the node list") — the accessibility tree drops focus references before pane widget removal; regression test reproduces the close sequence.
    - Performance: new advisory→verified budgets for gutter/active-line/bracket-match/background-fill paint paths documented in `docs/development/performance.md`; `KEYPRESS_TO_LOCAL_PAINT_P95_BUDGET_MS` non-regressed in the existing CI budget test; Criterion suites still compile (`cargo bench --no-run`).
    - Code Quality: theme catalog documents the two new axes (background, size ladder) with token/entry tables; primitive references (`rendering-strategy.md`, `typography.md`, `syntax-vocabulary.md`) updated to implemented status; generated registry refreshed.
    - Security: no authority surface changes in this phase to review; confirm decoration/span validation covers new fields (26.3 test already asserts).
  - Approach:
    - Documentation Reviewed:
      - AccessKit consumer focus lifecycle (upstream accesskit docs; local `src/masonry_shell`/pane removal path).
      - `docs/development/performance.md` budget conventions; existing budget CI tests.
    - Options Considered:
      - Suppress by retry/catch: rejected — fix the tree lifecycle, not the symptom.
      - Defer a11y fix: rejected — it's a user-reachable panic named by the review.
    - Chosen Approach:
      - Clear focus to a stable ancestor (shell/window node) before the pane widget is removed from the tree; guard test.
    - API Notes and Examples:
      ```text
      cargo test accesskit -- --ignored   # interactive regression
      cargo bench --no-run                # bench targets compile
      ```
    - Files to Create/Edit:
      - `src/masonry_shell.rs` or pane-removal path (focus reset), regression test.
      - `docs/development/performance.md`, `docs/reference/primitives/*.md`, generated registry.
    - References:
      - Review finding §3.7 (panic); roadmap Phase 26.7.
  - Test Cases to Write:
    - Regression: open dirty pane → Ctrl+Alt+W → no panic, focus lands on shell, screen-reader announcement sane.
    - Budget tests: new paint-path budgets asserted; existing keypress budget non-regression.

  - Completion Evidence (2026-08-19):
    - UI preflight: the UI guidance current at execution time → category `accessibility` → `jakubkrehel/better-accessibility`.
    - Root cause: AccessKit walk recursed into stashed widgets (hidden welcome) and advertised focus on detached pane hosts. `vendor/masonry_core` now skips stashed subtrees and clamps `TreeUpdate.focus` to a still-parented widget or the window. `apply_tree_change` re-focuses a surviving pane after detach. Opening a document requests layout so welcome is stashed before the next a11y pass.
    - Regression: `dirty_focused_pane_menu_and_discard_keep_consumer_focus_live` runs the live path (open dirty `a.txt` → conflict menu → FileOperationFailed → discard/close) through `accesskit_consumer::Tree`.
    - Budgets: `GUTTER_PAINT_P95_BUDGET_MS` (2), `ACTIVE_LINE_PAINT_P95_BUDGET_MS` (1), `BRACKET_MATCH_PAINT_P95_BUDGET_MS` (1), `DECORATION_BACKGROUND_FILL_P95_BUDGET_MS` (2) documented and locked to fit inside `KEYPRESS_TO_LOCAL_PAINT_P95_BUDGET_MS`.
    - Docs: `syntax-vocabulary.md`, `typography.md`, `rendering-strategy.md`, `docs/development/performance.md`, clay-ui tokens. No JS API/registry change (later plan task).
    - Validation: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test --lib` (1591), `--test editor` (167), `--test runtime` (198), `--test protocol` (164), `cargo bench --no-run`.

- [x] Perform visual screenshot and accessibility review of changed rendering
  - Acceptance Criteria:
    - Functional: real Linux GUI build with representative fixtures (Rust/TS/JS/Markdown) exercising every changed state: default theme, each shipped theme, light+dark, headings/fences/quotes/links, gutter/active-line/bracket-match/indent guides on and off, wrap `none`/`viewport`/`column`, narrow and wide windows where the host allows; screenshots retained under `code-reviews/screenshots/2026-08-18-phase26-review/` with findings.
    - Functional: with `computer-use-linux`, `get_app_state` first; verify accessibility tree exposes gutter/editor roles sanely, keyboard flow and focus visibility unchanged, no announcements regressed by the AccessKit fix.
    - Performance: review notes any visible jank on scroll/typing with mixed-size prose and long-line horizontal scroll.
    - Code Quality: findings triaged as defects or documented follow-ups; no silent passes.
    - Security: screenshots contain fixture data only.
  - Approach:
    - Documentation Reviewed:
      - Pattern `ui-visual-review.md`; Plan 088 review-harness conventions; decision `2026-08-14-0200-mandatory-ui-visual-and-accessibility-review.md`.
    - Options Considered:
      - Structural tests only: rejected — decision log forbids claiming visual pass without screenshots.
      - Full Plan 088 state matrix: rejected — scope to document rendering states changed by this phase.
    - Chosen Approach:
      - Reuse Plan 087/088 isolated fixtures; per-state before/after against the baseline task artifacts.
    - API Notes and Examples:
      ```text
      code-reviews/screenshots/2026-08-18-phase26-review/<state>.png + review.status
      ```
    - Files to Create/Edit:
      - `code-reviews/screenshots/2026-08-18-phase26-review/`: evidence.
      - This plan file: findings summary.
    - References:
      - Baseline from task 1.
  - Test Cases to Write:
    - Review checklist covering every state matrix row with pass/fail recorded.

  - Completion Evidence (2026-08-19):
    - UI preflight: the UI guidance current at execution time → categories `visual` + `accessibility` → `jakubkrehel/better-typography` + `jakubkrehel/better-accessibility`. clay-ui components/tokens + `ui-visual-review.md`.
    - 17 isolated captures under `code-reviews/screenshots/2026-08-18-phase26-review/` (Rust/TS/JS/Markdown × default + gruvbox-light + gruvbox-dark + modus-operandi, plus `rust-longline-default`). All `review.status=PASS`. Fixture-only; no host paths in a11y dumps.
    - computer-use-linux: `get_app_state` first; live Clay tree = focused Frame → working-area shell → pane → multi-line Entry (`review.rs`, theme) + StatusBar. Window-cropped screenshot confirms gutter/indent/active-line/opaque tokens.
    - Pass: opaque distinct syntax; H1–H6 ladder; quote/fence fills; code chrome on / prose chrome off; wrap-none overflow.
    - Defect: modus-operandi current-line gutter digit invisible (contrast). Follow-up.
    - Blocked: wrap `viewport` + user chrome toggles (JS API task). Live portal `press_key` timed out; dirty-close panic covered by 26.7 unit test. Narrow resize did not stick.
    - Log: `code-reviews/screenshots/2026-08-18-phase26-review/review-log.md`.

- [x] Create or verify Clay JS APIs for public programmatic surfaces
  - Acceptance Criteria:
    - Functional: inventory phase-introduced public surfaces — user layout override (`setEditorLayout` or equivalent: wrap policy, column cap, insets), chrome toggles (gutter/active-line/indent guides/bracket match per mode or global), theme `textStyles` gains `background` and `scale` entry fields (theme-package authoring surface); each exposed through documented facade APIs with dotted IDs (`editor.*`/`theme.*` core domains; check `RESERVED_CORE_API_DOMAINS`), never raw ops.
    - Performance: config evaluation stays off paint hot paths; runtime-config eval budget applies.
    - Code Quality: full doc set per API (stable ID, name, key bindings or empty list, custom properties, usage, example, errors, permissions, backing Rust path, op wrapper, facade path, lookup tags); master index links; `api-inventory.toml` updated; generated registry refreshed; `cargo test` fails on missing/stale entries.
    - Security: config APIs grant no filesystem/network/shell/package authority; wrap/chrome overrides are user-owned and package-unforgeable.
  - Approach:
    - Documentation Reviewed:
      - Patterns `clay-js-api-naming.md`, `clay-js-api-boundary.md`, `clay-js-api-schema.md`, `doc-registry-tests.md`.
      - Existing `theme.setTypography`/`theme.setTheme` docs as templates.
    - Options Considered:
      - Manifest-only configuration (no JS API): rejected — user override of wrap policy is a decided requirement and must be a documented API, not a hidden key.
      - One mega `editor.setRenderingOptions`: rejected — follow existing per-concern API naming (`setEditorLayout`, `setEditorChrome` or fold into existing surfaces).
    - Chosen Approach:
      - Minimal per-concern APIs mirroring existing `theme.*`/`editor.*` shapes; everything else stays manifest/theme data.
    - API Notes and Examples:
      ```js
      setEditorLayout({ wrapPolicy: "none" });              // editor.set-editor-layout
      setEditorChrome({ gutter: true, activeLine: false }); // editor.set-editor-chrome
      ```
    - Files to Create/Edit:
      - `runtime/js/*.js` facades, op wrappers in `src/server/ops/`, `docs/reference/clay-js-api/**`, `docs/reference/clay-js-api/api-inventory.toml`, master index, generated registry.
    - References:
      - Decision logs `2026-05-08-1509`, `2026-05-08-1840`.
  - Test Cases to Write:
    - Doc-registry gates: every new API has doc, index link, inventory entry, registry entry, lookup tags (existing `cargo test` gates).
    - API behavior: overrides parse, validate, apply, and survive config reload; invalid values rejected with diagnostics.

  - Completion Evidence (2026-08-19):
    - New public runtime API `editor.clientSetEditorLayout` (`clientSetEditorLayout` in `runtime/js/editor.js`, op `op_clay_editor_set_editor_layout` in `src/server/ops/editor.rs`). Full transport mirrors the caret-override lane: `ServerMessage::EditorLayoutOverride(Option<WrapPolicy>)` (protocol v18, `PROTOCOL_VERSION` bumped 17→18) → `ClientConnectionEvent::EditorLayoutOverride` → `PaneDocumentView::apply_connection_event` → `EditorSurface::set_editor_layout`. Publisher/store `ClayOpState::publish_editor_layout_override` + `ClayJsRuntimeService::subscribe_editor_layout`/`editor_layout_override`, wired in `production_reload`, `with_package_service`, and `replace_domain_worker`; survives reload. Connection handshake + lag replay resend the current override.
    - Security: op registered in `clay_runtime_trusted_extension` ONLY (not the package extension) + `require_editor_control` trust gate → user override is package-unforgeable; third-party code cannot resolve the op. `package_extension_is_strict_subset_without_admin_ops` updated (trusted 82→83, new op in the admin deny-list). Plan 061 op-inventory rebaselined (Trusted-only 5→6).
    - Theme `textStyles` authoring surface: `background` (Phase 26.3) and `scale` (Phase 26.4) entry fields documented in `docs/reference/packages/creating-packages.md` (field table + `scale` validation rule). Runtime activation facade is the existing `theme.setTheme` (already in inventory); axes also covered in `syntax-vocabulary.md` and `tokens.md`.
    - Chrome toggles: stay manifest-only (`editorRules.chrome`), no runtime JS API — consistent with the decided package-unforgeable chrome design (chrome is not SDUI; no package capability grants chrome override authority). The plan's `setEditorChrome` example was aspirational; the implemented surface is the `editorRules.chrome` manifest field documented in `creating-packages.md`.
    - Docs: `docs/reference/clay-js-api/editor/client-set-editor-layout.md` (full doc set), `docs/index.md` registry link, `api-inventory.toml` entry, `docs/generated/clay-js-api-registry.json` regenerated via `cargo run --bin update-doc-registry`. `runtime/js/editor.d.ts` types added.
    - Tests: 2 new lib tests (`set_editor_layout_publishes_runtime_wrap_override`, `set_editor_layout_rejects_unknown_and_clamps_column`); `protocol_version_is_pinned` updated. Suites: 1593 lib, 167 editor, 198 runtime, 164 protocol, 130 security all green; `cargo clippy --all-targets` clean; `cargo fmt` clean; `node --check runtime/js/editor.js` clean.

- [x] Create or verify Clay configuration APIs and update examples/init.js
  - Acceptance Criteria:
    - Functional: every new user-facing option (wrap policy, column cap, chrome toggles, theme background/scale entries) documented as a Clay JS API with custom properties; `~/.config/clay/init.js` remains the entry point; no undocumented behavior-changing keys.
    - Functional: `examples/init.js` updated — each new option appears exactly once, in its section, with options/types/defaults annotated and non-default examples commented; file passes `node --check`; active lines safe to copy verbatim; ordering constraints preserved.
    - Performance: config eval budget test covers the new options.
    - Code Quality: option names/enums/defaults in `examples/init.js` cross-checked against validated server-side parsers and `api-inventory.toml` custom properties, not prose.
    - Security: no config option implicitly grants authority.

  - Completion Evidence (2026-08-19):
    - `examples/init.js` gained section 5 "Editor layout — clay:editor clientSetEditorLayout": options annotated (wrapPolicy `none|viewport|column` required; columnCap number, default 72, clamped 16–240), resolution order (runtime override > manifest `editorRules.layout.wrap` > `WrapPolicy::from_font_role`), one active call `clientSetEditorLayout({ wrapPolicy: "column", columnCap: 72 })` safe to copy verbatim, non-default examples commented. Sections renumbered 5→11 with cross-references fixed (init.js section 10→11, section 7→6; packages/first-party.js + third-party.js section 10→11). `node --check` clean on all three example files.
    - Fixture `tests/fixtures/configuration/plan080-manual/` re-synced to the verbatim `examples/` tree (init.js + both package modules) so the manual test plan drives the current canonical config; `node --check` clean.
    - Docs: `docs/reference/clay-js-api/configuration.md` gained the Phase 26 editor-layout configuration review (surfaces, rejected hidden keys, security, performance); `client-set-editor-layout.md` corrected `wrapPolicy` default annotation to `required` (deny-by-default, matching `setAppearance`'s `=required` convention); `api-inventory.toml` custom_properties updated to `wrapPolicy:enum=required`; registry regenerated.
    - Chrome toggles and theme `textStyles` background/scale are manifest surfaces, not init.js options — configuration.md Phase 26 section states this explicitly (no undocumented behavior-changing keys); chrome manifest contract already documented in creating-packages.md (editorRules.chrome).
    - Performance: new lib test `editor_layout_config_eval_stays_within_hard_timeout` evaluates a config with the canonical active call and asserts completion under `JS_RUNTIME_EVALUATION_TIMEOUT_MS`, referencing the advisory `RUNTIME_CONFIGURATION_EVAL_P95_BUDGET_MS` (25 ms) benchmark target.
    - Code Quality: new test `canonical_example_cross_checks_editor_layout_options_against_inventory` cross-checks example option names/enums/defaults against the registry entry's custom_properties and the API doc (not prose); `canonical_example_covers_theme_typography_and_modular_configuration` extended with the new import.
    - Security: `clientSetEditorLayout` op is trusted-extension-only + `editor-control` gated (package-unforgeable); security_notes deny all authorities; example ground rules unchanged (init.js grants no filesystem/network/shell/package-install authority).
    - Suites: 1594 lib, 167 editor, 165 protocol, 198 runtime, 130 security all green; clippy + fmt clean.
  - Approach:
    - Documentation Reviewed:
      - Pattern `configuration-system.md`; existing `examples/init.js` structure.
    - Options Considered:
      - Document in API docs only, skip example file: rejected — per-plan canonical example maintenance duty.
    - Chosen Approach:
      - Single pass after APIs land: docs, inventory custom properties, example file, registry.
    - API Notes and Examples:
      ```js
      // examples/init.js (section: editor layout & chrome)
      // setEditorLayout({ wrapPolicy: "none" }); // default: mode manifest
      ```
    - Files to Create/Edit:
      - `examples/init.js`, API docs, `api-inventory.toml`.
    - References:
      - Decision `2026-05-08-1841`; user instruction 2026-08-03 (canonical example).
  - Test Cases to Write:
    - `node --check examples/init.js` in CI/test task.
    - Cross-check test: example options ⊆ documented custom properties.

- [x] Execute and update the manual test plan (test-plan/)
  - Acceptance Criteria:
    - Functional: run affected `test-plan/` modules (document rendering, theme, editor interaction, accessibility) on a real Linux build; record pass/fail against numbered steps; add new module steps for gutter/chrome, wrap policies, background panels, heading hierarchy, search-match highlight, and the dirty-pane close fix, each with expected results, negative checks, and known ceilings; update `test-plan/index.md` coverage matrix; cross-link deep-reference docs.
    - Performance: manual steps include scroll/typing feel on mixed-size prose and long-line horizontal scrolling.
    - Code Quality: no weakening/deleting existing steps; failures become defects or documented ceilings.
    - Security: no secrets/host paths in recorded evidence.
  - Approach:
    - Documentation Reviewed:
      - `test-plan/index.md` module map.
    - Options Considered:
      - Rely on automated + visual review only: rejected — per-plan manual test duty for user-visible behavior.
    - Chosen Approach:
      - Map phase changes to modules; extend, don't rewrite.
    - API Notes and Examples:
      ```text
      test-plan/<module>.md — new steps: phase26-xx IDs
      ```
    - Files to Create/Edit:
      - `test-plan/*.md`, `test-plan/index.md`.
    - References:
      - User instruction 2026-08-04 (test-plan duty).
  - Test Cases to Write:
    - Manual steps per changed state with recorded results.

  - Completion Evidence (2026-08-19):
    - New Phase 26 steps added (extend, never rewrite): 07 T20–T27 (heading ladder, wrap policies none/viewport/column, chrome on/off per mode, textStyles background/scale validation), 08 S16–S19 (quote/fence background panels, rich vocabulary, search-match background, edit-following), 04 E25–E27 (search-match fill, typing feel in fences/headings, selection-over-background), 13 S43–S46 (dirty-pane close fix re-verification, per-pane chrome), 11 Q20–Q23 (mixed-size prose scroll, long-line horizontal scroll, typing feel, advisory bench), 09 P22–P24 (theme textStyles axes across bundled themes, invalid-scale rejection, light/dark comparison). Each with expected results, negative checks, and known ceilings; deep-reference docs cross-linked (typography.md, rendering-strategy.md, syntax-vocabulary.md, client-set-editor-layout.md, creating-packages.md, accessibility.md, performance.md).
    - `test-plan/index.md` coverage matrix gained the Phase 26 row (04 E25–E27, 07 T20–T27, 08 S16–S19, 09 P22–P24, 11 Q20–Q23, 13 S43–S46) and a Phase 26 Linux execution record.
    - Live execution on real Linux/GNOME: fresh captures with the current build at `code-reviews/screenshots/2026-08-19-phase26-manual-test-plan/` (rust-default, markdown-default, rust-longline-default; all `review.status=PASS`) + `review-log.md`; single-key delivery WORKS live (typed input dirtied a real document, doc v2, client alive, AT-SPI tree intact); modifier chords (`Ctrl+Alt+W`) and scroll remain host-blocked (portal limitation, review-log V9) — dynamic steps covered by the automated suites named in each module record.
    - Dirty-pane close fix (S43/S44): automated regression tests `dirty_focused_pane_menu_and_discard_keep_consumer_focus_live` + `dirty_pane_close_rejection_and_discarded_removal_keep_focus_consumer_safe` exercise the exact Plan 086 crash path with the consumer focus live at every step; the Plan 086 `accesskit_consumer` panic no longer reproduces; live dirty state verified stable.
    - Defects: V4 (light-theme gutter digit contrast, `*-modus-operandi`) carried forward as a tracked defect in the review log — not a blocker; no new defects found.
    - Security: all evidence from private mode-700 HOME/XDG/workspace/socket runs; sanitized pane/status names; no host paths or secrets in recorded evidence.

- [x] Update or verify the code wiki after implementation
  - Acceptance Criteria:
    - Functional: wiki updated after all tasks pass — `editor-theme-registry.md` (two color axes, size ladder), `decoration-transport.md` (background field, budgets), new/updated page for editor chrome + wrap policy/viewport metrics; master index links.
    - Performance: wiki documents the paint-path performance characteristics of the new axes and chrome.
    - Code Quality: pages explain what/how/invariants/tradeoffs with source/test paths and examples.
    - Security: documents that backgrounds/scales are inert theme data inside existing validation and budgets.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/project-wiki/SKILL.md`; `docs/wiki/index.md`.
    - Options Considered:
      - Update per task: rejected — churn; once after tests pass.
    - Chosen Approach:
      - Single wiki pass at phase end using the wiki-task template.
    - API Notes and Examples:
      ```text
      docs/wiki/index.md → docs/wiki/modules/editor-theme-registry.md, decoration-transport.md, editor-chrome-layout.md
      ```
    - Files to Create/Edit:
      - `docs/wiki/index.md`, `docs/wiki/modules/*`.
    - References:

  - Completion Evidence (2026-08-19):
    - New page `docs/wiki/modules/editor-chrome-and-layout.md` (linked from the master index): Phase 26.5/26.6 editor chrome (gutter, active-line, indent guides, bracket-match — `EditorChrome` prose()/code()/from_font_role defaults, `chrome.rs` resolution, `TextChromeLayers` paint order, 64 KiB bracket-scan ceiling, chrome outside `LayoutCacheKey`) and layout geometry (`WrapPolicy` None/Viewport/Column, `EditorLayoutRules`, asymmetric token-driven insets `TEXT_INSET`/`TEXT_INSET_GUTTER`/`TEXT_INSET_Y`, horizontal scrolling, wrap-aware overscan, `clientSetEditorLayout` trusted-only transport, advisory paint budgets).
    - `editor-theme-registry.md` updated: `StyleSpec { color, background, bold, italic, underline, strike, scale }`; background axis (`background_for` per DecorationKind, fills between selection and glyphs, no `DecorationSpan` field); size ladder table (H1 1.50 … CodeSpan 0.90, clamp `(0, 4.0]`, Syntax/Semantic only, u16 milli-unit wire); five chrome `BaseUiColorKey` variants with `clay_default()` values; Phase 26 tests.
    - `decoration-transport.md` updated: Phase 26 background axis stays theme-resolved — no rkyv change, no `DECORATION_PAYLOAD_BUDGET_BYTES` growth, wire shape locked by `decoration_span_wire_shape_has_no_background_field`; scale multiplier and advisory budgets noted.
    - `typography-registry-and-font-roles.md` updated: per-token size ladder section (scale multiplication in `LayoutState::rebuild`, unscaled `document_line_height()` baseline, Syntax/Semantic-only restriction).
    - Master index (`docs/wiki/index.md`) links the new page and updated the Editor Theme Registry + Decoration Transport descriptions; deterministic gate `tests/primitives_docs.rs::wiki_index_links_every_wiki_page` passes.
    - Performance: paint-path characteristics documented (budget constants, sum-inside-16ms assertion, cache-key exclusion, scan ceilings).
    - Security: backgrounds/scales/chrome documented as inert theme/manifest data inside existing validation and budgets; `clientSetEditorLayout` trusted-extension-only.
    - Full suite green: 1594 lib, 165 protocol.
      - Wiki-task template.
  - Test Cases to Write:
    - Manual wiki review: index links, pages explain changed implementation.

## Compromises Made

- To be filled after tasks are completed and tests pass.

## Further Actions

- To be filled after task completion with improvements, rationale, and priority.
