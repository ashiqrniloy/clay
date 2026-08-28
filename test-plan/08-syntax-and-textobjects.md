# 08 — Syntax Highlighting, Text Objects, Smart Select

Grammar-backed highlighting, tree-sitter textobjects, smart select, engine
tier preference, advisory degradation. Deep reference:
`docs/development/manual-editor-capabilities-test-plan.md` (section F).

## Setup

- Load `@clay/rust` (+ optionally `@clay/typescript`, `@clay/javascript`,
  `@clay/markdown`) in init.js.
- Textobject/smart-select keys are bound via init.js (no defaults ship):

```js
bindKey("Alt+I", "editor.clientSelectTextobject.function.inner.current", { scope: "editor" });
bindKey("Alt+O", "editor.clientSelectTextobject.function.around.current", { scope: "editor" });
bindKey("Alt+A", "editor.clientSelectTextobject.argument.inner.current", { scope: "editor" });
bindKey("Alt+C", "editor.clientSelectTextobject.comment.around.current", { scope: "editor" });
bindKey("Alt+E", "editor.clientSmartSelect.expand", { scope: "editor" });
bindKey("Alt+R", "editor.clientSmartSelect.shrink", { scope: "editor" });
```

Open `/tmp/clay-manual/test.rs` (from module 05 setup).

## Highlighting

| # | Action | Expected |
|---|--------|----------|
| S1 | Open `test.rs` | Keywords/types/strings/comments colored per theme grammar styles |
| S2 | Edit inside a function | Highlighting updates incrementally, no full-file flicker |
| S3 | Scroll a long file fast | Windowed re-highlight keeps typing smooth |
| S4 | Open `plain.txt` (no grammar) | Plain text, no crash, core.text fallback mode |

## Text objects (Rust grammar)

| # | Action | Expected |
|---|--------|----------|
| S5 | Caret inside `compute_total_value` body, `Alt+I` | Selects function body (inner) |
| S6 | `Alt+O` | Selection grows to include the `fn` signature line (around) |
| S7 | Caret on `firstItem` in signature, `Alt+A` | Selects that parameter only |
| S8 | Caret in the leading comment, `Alt+C` | Selects the whole comment block |
| S9 | Bind a `.next` variant, press repeatedly | Walks to following functions |

## Smart select

| # | Action | Expected |
|---|--------|----------|
| S10 | Caret in a statement, `Alt+E` repeatedly | Grows: expression → statement → block → function → file |
| S11 | `Alt+R` after S10 | Shrinks back down the same chain |

## Advisory degradation (must never block editing)

| # | Action | Expected |
|---|--------|----------|
| S12 | Repeat S5 in `test.md` / `plain.txt` (no textobjects grammar) | No crash, no selection change — carets untouched |
| S13 | Smart select in a file with no grammar | Same: silent no-op |

## Engine tiers

| # | Action | Expected |
|---|--------|----------|
| S14 | `setSyntaxEnginePreference("rust", "native")` (default) | Grammar works |
| S15 | `setSyntaxEnginePreference("rust", "turbo")` (unknown tier) | Diagnostic/rejection; highlighting unaffected |

## Negative checks

- Parsing/decoration publication never happens in keystroke/paint hot paths
  (typing stays smooth while a large file re-parses in background).
- Decoration payloads stay bounded (large-file case → module 11).

## Known ceilings

- Textobjects.scm ships for rust/typescript/javascript only; other languages
  degrade silently.
- Third-party packages cannot contribute grammars/textobjects today
  (first-party-only contribution rule).

## Phase 26 rich highlighting and decoration background steps

Deep references: `docs/reference/primitives/syntax-vocabulary.md` (Phase 26
theme axes table), `docs/reference/primitives/rendering-strategy.md`
(background axis), `docs/reference/packages/creating-packages.md`
(textStyles `background`/`scale`). Setup: open the rust fixture
(`tests/fixtures/syntax/rust.rs`) and the markdown fixture
(`tests/fixtures/syntax/markdown.md`).

| # | Action | Expected |
|---|--------|----------|
| S16 | Open the markdown fixture | Block quotes and fenced code blocks paint a background tint BEHIND the glyphs (between selection and text); inline code renders in the monospace role at 0.90 scale; the tint never covers the selection rects |
| S17 | Open the rust fixture and inspect token colors | Rich vocabulary renders distinctly: booleans, method calls, field access, type parameters, attributes, macros, operators, punctuation — each with its own color per the expanded native style map (25 entries); no two adjacent token kinds share a color in the default theme |
| S18 | Search-match state (if reachable via the search surface) | Search-match spans paint a background fill that wins over syntax/semantic backgrounds at overlap; diagnostic spans keep squiggles/underlines and never paint a foreground color |
| S19 | Type inside a fenced code block in markdown | The code-block background follows the edit incrementally; no full-file flicker; the background fill stays behind the glyphs while typing |
| S20 | Open markdown after Phase 27.6 | Headings/emphasis/code use closed `HeadingN` / `Paragraph`+Bold/Italic / `CodeSpan` colors from the theme table — not leftover `markup.*` producer tokens |

Negative: a decoration background never covers the caret or selection;
diagnostic decorations contribute no foreground color (squiggles only);
background fills are painted between selection rects and text — never over
glyphs.

## Phase 26 Linux execution record (2026-08-19)

| Checks | Result | Evidence |
|---|---|---|
| S16 | PASS live | `code-reviews/screenshots/2026-08-18-phase26-review/markdown-*/` (4 themes): quote + fence background tints visible behind glyphs; CodeSpan smaller |
| S17 | PASS live | `code-reviews/screenshots/2026-08-18-phase26-review/rust-*/`, `typescript-*/`, `javascript-*/` (4 themes each): distinct opaque colors for kw/type/string/number/macro/attr/property/method/regex; review-log V1 |
| S18 | PASS automated / NOT RUN live | `search_match_and_quote_backgrounds_join_style_runs` (SearchMatch wins over Syntax Quote at overlap), `style_run_backgrounds_paint_before_glyphs` (fill loop precedes render_text in source); live search surface not reachable this session |
| S19 | PASS automated / NOT RUN live | Incremental parse/decoration continuity tests (`plan057_first_party_languages_keep_continuity_across_edit_boundaries`, `plan058_*_shifted_boundary_continuity`) cover edit-following; live typing into fences is host-blocked |

## Phase 28 folding, link decorations, and inlay hints

Deep references: `docs/reference/primitives/ui-chrome-primitives.md`,
`docs/reference/clay-js-api/folding/server-publish-folding-ranges.md`,
`docs/reference/clay-js-api/decorations/server-publish-decorations.md`, and
`docs/reference/clay-js-api/editor/toggle-inlay-hints.md`.
Setup: load `@clay/rust`, `@clay/markdown`, and the authorized
`@clay/lsp-rust` fixture when testing LSP inlays; bind
`editor.clientToggleFold` and `editor.toggleInlayHints` in module 10.

| # | Action | Expected |
|---|---|---|
| S21 | Open a Rust file containing nested named blocks | Foldable multiline ranges show a Clay-owned gutter chevron; the range label is optional, the body remains unchanged, and chevrons are not separate Tab/AT-SPI focus targets |
| S22 | Place the caret on a fold start and invoke `editor.clientToggleFold` twice | First invocation hides interior lines and preserves the fold-start line/caret; second restores all lines. Cursor movement and line metrics skip hidden lines without changing document text |
| S23 | Collapse an outer range containing an inner range, then reopen it | The parent hides the complete nested interior; reopening restores child visibility and chevron state without stale layout or selection mapping |
| S24 | Negative: publish a folding range from a package without `render-folding`, or use an oversized/malformed range fixture | Publication is denied with a bounded diagnostic; no fold reaches the editor and no package JavaScript enters paint/layout. If no manual fixture is available, record N/A and retain the automated denial result |
| S25 | Open the Markdown fixture and inspect relative and absolute links | Workspace-relative links use Clay link styling/underlines; HTTP/HTTPS, fragments, and other display-only targets do not become network actions |
| S26 | Hover a relative link, then move away; keep a completion/command menu open while doing so | Link tooltip chrome appears only for the hovered target, clears on leave, and does not steal or replace the active transient completion/command menu |
| S27 | Activate a same-document or workspace-relative link | Same-document targets jump to their range; retained workspace documents focus; otherwise Clay opens the resolved workspace file through the existing document path |
| S28 | Negative: activate `https://example.com`, an absolute path, or `../outside.md` | No browser/network/external process starts; unsafe targets are display-only or denied, no browse grant is minted, and the editor remains available |
| S29 | Open a code document with LSP inlay hints enabled | Inlay labels render as muted overlays before/after the anchor without shifting existing glyph layout, changing wrapping, or becoming normal syntax spans |
| S30 | Toggle `editor.toggleInlayHints` off and on | Labels disappear/reappear locally; semantic decorations and document text remain; no refetch or full-document reflow is required |
| S31 | Open prose mode with no override, then code mode with no override | Prose defaults inlay visibility off; code defaults on when the provider publishes hints; a user toggle overrides only the active pane/mode |
| S32 | Inspect inlay/fold/link accessibility and keyboard paths | Fold chevrons and decorative inlay text are not tab stops; link activation has a keyboard/caret command path even if the custom editor does not expose native link nodes; status/error text is announced without relying on color alone |

## Phase 28 Linux execution record (2026-08-20)

| Checks | Result | Evidence |
|---|---|---|
| S21 | PASS rest state; UNRESOLVED collapse interaction | `code-reviews/screenshots/2026-08-20-phase28-primitives/rust/` shows two fold chevrons and no chevron tab stops. Compositor targeting prevented a repeatable collapse/restore action. |
| S22–S24 | UNRESOLVED live; PASS structural/security | `toggle_fold_hides_and_restores_interior_lines`, nested-fold coverage, and folding permission/budget tests pass; no manual package publication fixture was exposed. |
| S25 | PASS rest state | `code-reviews/screenshots/2026-08-20-phase28-primitives/markdown/` shows readable underlined links. |
| S26–S28 | UNRESOLVED live; PASS structural/security | Pointer targeting was unstable and no external target was opened. `link_span_round_trip_with_workspace_target`, activation planning/denial, and decoration validation tests pass. |
| S29–S31 | UNRESOLVED live; PASS structural | The LSP GUI worker failed to resolve the existing `lsp-shared` helper; no inlay screenshot/toggle claim is made. `toggle_inlay_hides_overlay` and `prose_chrome_defaults_inlays_off` pass. |
| S32 | PARTIAL | Rest-state AT-SPI checks pass for fold chevrons; links are visually underlined but are not separate accessible link objects. The P2 discoverability follow-up remains in the Phase 28 review log. |

## Phase 28.7 P1 GUI analyzer follow-up (2026-08-21)

| Checks | Result | Evidence |
|---|---|---|
| S29–S31 | UNRESOLVED live; PASS worker/bridge structural | Fixed `lsp-shared` session options, analyzer workspace-root context, and decoration viewport byte length. Fresh GUI run reaches `@clay/lsp-rust` with no `analysis.worker_failed`; the worker emits an `InlayHint` set, but its first response is empty before rust-analyzer finishes analysis. Input backend is unavailable (`uinput` denied, no `ydotool`, no keyboard-capable RemoteDesktop portal), so no visible/toggled-off interaction claim is made. Evidence: `code-reviews/screenshots/2026-08-20-phase28.7-followups/inlay-visible/` and `inlay-toggled-off/`, both explicitly `UNRESOLVED`. |
| S32 | UNRESOLVED live; PASS structural | The same input ceiling prevented the toggle/accessibility recapture; decorative inlay and fold semantics remain covered by the structural accessibility tests. |

## Phase 28.7 P2 visual and interaction recapture (2026-08-21)

UI preflight used the UI guidance current at execution time, category `accessibility`, selected
`rams/rams`, and `computer-use-linux_get_app_state` before review. Static and
unresolved fixture evidence is under
`code-reviews/screenshots/2026-08-21-phase28.7-p2-recapture/`; retained fold/link
rest captures remain under `code-reviews/screenshots/2026-08-20-phase28-primitives/`.

| Checks | Result | Evidence |
|---|---|---|
| S21 | PASS rest; UNRESOLVED collapse interaction | Retained Rust rest capture shows fold chevrons with no chevron tab stops; no keyboard/pointer backend could repeat collapse/restore. |
| S22–S24 | UNRESOLVED live; PASS structural/security | Fold hide/restore, nested-fold, malformed-range, permission, and budget tests pass; no live action was falsely claimed. |
| S25 | PASS rest; UNRESOLVED activation interaction | Retained Markdown rest capture shows underlined link styling; pointer/caret activation was not safely targetable. |
| S26–S28 | UNRESOLVED live; PASS structural/security | Hover/leave/menu coexistence and safe/HTTP/traversal activation tests pass; no browser/network/external process was started. |
| S29–S31 | UNRESOLVED live; PASS worker/bridge structural | Fresh Rust fixture reached the analyzer path but did not publish a non-empty inlay set after an AT-SPI SetValue edit; no visible/toggled-off claim was made. |
| S32 | PARTIAL; unresolved live discoverability | Fold/inlay semantics remain structural PASS. Link rest styling is visible, but custom editor exposes no separate AT-SPI Link node or purpose announcement; this remains an explicit product follow-up. |

No existing step was deleted or weakened.

## Plan 097 Phase 12 Tauri/React visual and accessibility review (2026-08-24)

| Check | Result | Evidence |
|---|---|---|
| Syntax, diagnostic, folding, and inlay rest state | PASS static | `code-reviews/screenshots/2026-08-24-tauri-react-parity/intelligence/fixture-*` shows the bounded CodeMirror projection and diagnostic/inlay/fold styling |
| Link/inlay/fold interaction | UNRESOLVED live; PASS structural/security | Host keyboard/pointer targeting is unavailable; decoration, target-denial, folding, inlay, and no-network tests pass |
| Accessibility semantics | PASS rest state / known link ceiling | AX snapshot contains editor region/document entry; decorative inlays/folds are not controls. Custom editor still has no separate AT-SPI Link node, as previously documented |

## Plan 099 viewport continuity steps

| # | Action | Expected |
|---|---|---|
| S33 | Fling/jump-scroll a 1–50 MiB Rust, TypeScript, JavaScript, and Markdown fixture from top to bottom and back | Each viewport request receives one current request-id patch; stale patches are dropped, authoritative coverage advances, and no scroll path blocks on parser work or produces a stuck overlay. No long task exceeds 50 ms. |
| S34 | Reach a viewport with no syntax/decorations and inspect the explicit empty response | An explicit empty completion patch clears only its covered range in one render transaction; sibling package/feature ranges remain intact and the request pipe is immediately reusable. |

## Plan 099 Linux execution record (2026-08-28)

| Check | Result | Evidence |
|---|---|---|
| S33 | UNRESOLVED live; PASS structural companion | No keyboard/scroll backend was available. Atomic patch ordering, stale-id rejection, and bounded syntax-session tests remain green; the harness warning confirms no viewport patch was driven. |
| S34 | UNRESOLVED live; PASS structural companion | No empty viewport was reachable. Explicit empty/rejected patch tests cover scoped clearing and immediate completion. |

The zero parser queue in the manual harness means no syntax flow ran; it is
not evidence that fling/jump scrolling had no parser work.
