---
date: 2026-07-09 03:52
status: approved
decision_about: "Tiered tree-sitter syntax engine, LSP-based themable syntax vocabulary, theme registry, layered decorations, completion extensions, and opt-in LSP language intelligence. Supersedes 2026-07-08-2316."
proposed_by: "both"
explicitly_approved_by_user: true
---

# Decision: Tiered tree-sitter syntax, LSP-based themable vocabulary, theme registry, layered decorations, and opt-in LSP language intelligence

**Supersedes:** `decision-logs/2026-07-08-2316-web-tree-sitter-host-adapter-for-syntax-highlighting-and-non-blocking-open-parse.md` (the web-tree-sitter-only engine choice and its rejection of native crates). The 2316 open-time-parse and silent-drop fixes remain binding and are carried forward unchanged.

## Decision

Clay adopts a layered, themable language-rendering architecture organized around a single LSP-based vocabulary contract. Six components:

1. **Tiered tree-sitter engine behind one generic grammar→category pipeline.** A generic grammar→`DecorationSpan` pipeline is engine-agnostic. Three backends sit behind it:
   - **Tier 1 — native compiled-in `tree-sitter-*` crates** for the first-party languages Clay ships: `tree-sitter-rust`, `tree-sitter-typescript` (typescript + tsx grammars), `tree-sitter-javascript`, and `tree-sitter-md`/`tree-sitter-markdown`. These register as first-party grammar contributions at startup and dispatch by grammar/extension lookup, **not** by `match language_id { "rust" => ... }`. The native `Language` objects are bundled grammar data; the pipeline is one generic function. The **"no per-language Rust branches" rule is preserved.**
   - **Tier 2 — host-side `web-tree-sitter` WebAssembly adapter** for third-party languages and for package override of a first-party language. Runs inside the existing Deno/V8 background worker.
   - **Tier 3 — per-package JS parser** fallback for languages with no tree-sitter grammar (the original `@clay/markdown` `parser.js` route, retained).
   All three emit the same vocabulary tokens; the theme does not know or care which engine produced a span.

2. **LSP-based syntax vocabulary as the theming contract.** Adopt LSP `SemanticTokenType` + `SemanticTokenModifiers` as the base token vocabulary (namespace, type, class, enum, interface, struct, typeParameter, parameter, variable, property, enumMember, event, function, method, macro, keyword, modifier, comment, string, number, regexp, operator, decorator; modifiers: declaration, definition, readonly, static, deprecated, async, abstract…), extended with a small Clay-owned **prose** set for text modes: `heading-1..6`, `strong`, `emphasis`, `code-span`, `code-block`, `list-marker`, `link`, `quote`. An **open-string escape with prefix fallback** (TextMate scope style) lets third-party packages mint new tokens a theme can still catch via prefix.

3. **Two-axis decoration model.** A `DecorationSpan` carries two **orthogonal** axes — `token_type` (what it is: keyword, function, heading-1) and text-attribute `modifiers` (how to draw it: bold, italic, underline, strikethrough) — replacing today's free-form `style_token: String`. This fixes the conflation where `bold`/`italic` were treated as peer token types: a markdown `**x**` span is pure `modifiers=bold` (no token-type color), while a rust `fn` keyword is `token_type=keyword`; a signature function name can be `token_type=function` + `modifiers=bold|declaration`. Backward compatibility: the current `style_token` families (`keyword.*`, `string.*`, `comment.*`, `punctuation.*`, `markup.*`) migrate to the new vocabulary during the implementation phase.

4. **Theme registry as the single source of color.** A `StyleRegistry` maps `token_type + modifiers → StyleSpec{color, bold, italic, underline, strike}` plus base UI colors (`panel.bg`, `text`, `accent`, `scrollbar`, `editor.gutter`). The **default theme is Clay-owned**: today's hardcoded `Color` constants in `src/editor/surface.rs` (`SYNTAX_KEYWORD_COLOR`, `PANEL_COLOR`, etc.) move into the registry and are **deleted from paint code**. Theme packages (`@clay/theme-*`) publish **inert style overrides only** (no code, no widgets, no ops), reusing the existing `ThemeTokenContributionDescriptor` shape (currently wired only to SDUI/UI components, not text). **One active theme**, selected by the user via `setTheme("@clay/...")` in `~/.config/clay/init.js`. Initial shipped themes: **Gruvbox Material Dark** and **Gruvbox Material Light**, both configurable from `init.js`.

5. **Layered decorations, not override.** Syntax (tree-sitter), Semantic (LSP), Diagnostic, and Search layers paint independently; higher layers **refine** token specificity (`variable` → `variable.readonly`) rather than deleting lower layers. "Override native" is re-expressed as **engine selection** — a user config choosing Tier 1 vs Tier 2/3 for a language — and must be **user-initiated**, never silent package self-promotion. `priority` already exists for within-layer ordering; layer composition is additive.

6. **Completion extensions.** Keep today's priority-merge across providers. Add: an opt-in `exclusive` claim flag (a provider claims the whole result for its `(package_prefix, mode)`, suppressing lower-priority providers); a user `disableCompletion("@clay/...")` config to turn off the native/base provider; and a **snippet** kind on `CompletionItem` (LSP snippet syntax with tabstops). Clay ships a **priority-0 base keyword/snippet completion provider** so every mode has completion even with no package. Native completion is priority 0, not authority-overriding; third-party completion wins by higher priority or explicit user disable-native.

7. **LSP as opt-in package, never core.** Clay core defines the engine-agnostic **integration-target primitives**: range `DiagnosticSpan` (NEW — today's `RuntimeDiagnostic` is document-level only), `Hover`, `GoToDefinition`, `CodeAction`, `SignatureHelp`, and the Semantic decoration layer (`DecorationKind::Semantic` exists but is unused). These are the bridge targets. A first/third-party **`@clay/lsp-*` package** spawns a language server (`rust-analyzer`, `typescript-language-server`, `marksman`, etc.) under **explicit user-declared per-package subprocess + filesystem authority** (a new `"language-server"` permission, declared in `init.js`), and bridges LSP responses → Clay primitives. Because the vocabulary is LSP-based, the bridge is nearly identity (LSP `SemanticTokens`/`Diagnostic`/`Completion`/`Hover`/`Goto`/`CodeAction` map 1:1). **No language-server subprocess authority is granted implicitly.**

**Carried over from 2316 (still binding):**
- Open-time parse must be **non-blocking**: document text renders immediately on open, `DecorationSet` paints when background parse completes.
- `ParseCoordinator::finish_task` (`src/server/parse_coordinator.rs`) must **surface handler errors as `RuntimeDiagnostic`** instead of silently dropping them.
- **First-party-only resolver authority** (`@clay/*`); arbitrary third-party grammar/native-artifact loading stays out of scope until a dedicated security/trust decision.
- The **Markdown preview SDUI panel stays package-JS**; only Markdown's *decoration* role moves onto the generic pipeline. Preview and decoration are kept separate.
- No raw `Deno.core.ops` routes, no client-side JavaScript, no native-widget handles, no per-package raw CSS/Vello/Parley callbacks for language work.

## Context

Manual `cargo run` testing on Linux/GNOME (Plans 043/044) found that syntax highlighting does not appear for `.rs`/`.ts`/`.js` and that Markdown highlighting is hidden by a `clay.parse.open_activation_timeout` hang. 2316 proposed a web-tree-sitter-only engine as the fix.

After 2316 was approved, the user introduced two new design requirements that make the web-only engine choice insufficient and that add axes 2316 never considered:

1. **Themability as a first-class product goal.** Themes must control syntax highlighting rendering (color, bold/italic) for the editor — not just SDUI/UI component tokens. This is impossible today: colors are hardcoded `Color` constants in `src/editor/surface.rs`, and `style_token` is a free-form string with no contract a theme can bind to. A vocabulary contract and a theme registry are prerequisites.
2. **Performance + first-party quality.** The user wants native tree-sitter for first-party languages (Rust/TypeScript/JavaScript/Markdown) as Tier 1, with web-tree-sitter/JS as fallback for third-party. Native is fastest and ships good highlighting out of the box without package-load ordering or missing `.wasm` artifacts.

A full architecture analysis identified **nine flaws** in the user's initial framing, all corrected above: (1) conflating token-type with text-attributes → split into two axes; (2) "override native" as binary → layered merge + user-initiated engine selection; (3) "mode-specific code in Clay" misframing → native needs bundled grammar *data* + generic *engine*, not per-language code, so the no-branches rule survives; (4) closed-vocabulary risk → LSP base + extensible prefix fallback; (5) silent override = security hole → overrides are user-declared; (6) no range-diagnostic primitive exists → new `DiagnosticSpan`; (7) LSP authority undecided → opt-in package with explicit permission; (8) hardcoded colors defeat theming → registry is single source of color; (9) markdown dual-role → preview stays package-JS, only decoration folds.

The three objections that previously rejected native crates (2316 Context) are **answered by the tiered model**: split-brain is gone (all decoration logic is package-side; the native engine is generic), future third-party modes need no rebuild (Tier 2/3), and no-grammar languages use Tier 3. What native-first costs: Clay maintainers recompile to add a new *first-party* language — acceptable, since first-party is Clay's own maintenance burden.

## Approval

- Proposed by: **both**. The user set the design goals (themable, native-first for first-party, third-party override, completion flexibility, LSP exploration); the agent performed the architecture analysis, flagged the nine flaws, and recommended the tiered-engine + LSP-vocabulary + layered-decoration + opt-in-LSP direction.
- Approved by user: **Yes**.
- Approval evidence: User said *"I agree with all the recommendations that you have provided and I am fully aligned with those. So next step would be to revise @decision-logs/2026-07-08-2316-... Now for the rest of the implementation required, I want you to first divide them into phases and add them to @roadmap.md after Phase 18.14 and before Phase 19."* The user further specified: full TypeScript/JavaScript/Rust/Markdown support, native tree-sitter crate for all four, LSP packages for all four, theming with Gruvbox Material light and dark configurable via `init.js`.

## Alternatives Considered

From 2316 (re-evaluated under the new goals):

1. **Native `tree-sitter-*` crates as the *only* engine (2316 Option 1).** Rejected again as sole engine: no coverage for languages without a crate, no third-party extension without a rebuild. **Adopted as Tier 1 of a tiered stack** — best performance for first-party languages, with Tier 2/3 covering the gaps.
2. **tree-sitter native `wasm` feature + `wasmtime-c-api` in Rust core (2316 original proposal).** Rejected again: pulls a heavy second WASM runtime into core when the existing V8 worker already runs wasm. Covered by Tier 2 (`web-tree-sitter` in V8) instead.
3. **`web-tree-sitter` only (2316 adopted choice).** Now superseded: it optimizes for "no per-language Rust" purity at the cost of first-party performance and quality, and it ignored the theming/vocabulary axis entirely. Retained as Tier 2 for third-party/override.
4. **Per-package JS highlighter only (e.g. highlight.js/Prism).** Retained as the Tier 3 fallback, not the default: loses incremental reparse and weaker quality.

New axes considered in this decision:

5. **Closed token vocabulary (Clay enum).** Rejected as sole mechanism: every new language need (`regex`, `decorator`, `macro`, `builtin`) needs a Clay change. Replaced by LSP base enum + open-string escape with prefix fallback.
6. **Single-owner decorations ("native or override").** Rejected: decorations are inherently multi-source (tree-sitter syntax + LSP semantic + diagnostics + search coexist). Replaced by layered additive merge.
7. **LSP embedded in Clay core.** Rejected: a language server is a subprocess that reads the whole project — a massive authority escalation that contradicts Clay's confined-authority model. Replaced by opt-in package with explicit permission.
8. **Hardcoded surface colors + per-token-family `decoration_color()`.** Rejected: makes theming cosmetic. Replaced by the `StyleRegistry` as single source of color.

## Rationale and Evidence

- **The vocabulary is the contract.** Adopting LSP `SemanticTokenType`/`SemanticTokenModifiers` (proven by VS Code, the de-facto standard) gives: (a) 1:1 mapping when an LSP bridge lands, (b) familiarity for theme authors, (c) Clay does not maintain the taxonomy. The prose extension is the one place Clay invents tokens, because LSP has no prose concept.
- **Two axes match the TextMate/VS Code model.** `scope → textMateRules → {foreground, fontStyle}` already separates "what it is" from "how to draw it." `bold`/`italic` as text attributes (not token types) lets markdown inline formatting compose with prose token types and lets a function name be bold in a signature but not at a call site.
- **Native Tier 1 is the proven editor default.** Zed, Helix, and Neovim ship built-in native tree-sitter grammars for shipped languages and load extension grammars dynamically. Native gives incremental reparse, no wasm compile, no V8 contention, and the lowest latency — directly serving the stated performance goal.
- **The tiered model resolves the 2316 objections.** No mode-specific Rust code (generic engine + bundled grammar data); third-party modes need no rebuild (Tier 2/3); no-grammar languages supported (Tier 3).
- **Theme registry reuses existing scaffolding.** `ThemeTokenContributionDescriptor` (`src/packages/record.rs`) already exists for SDUI/UI component theming — text rendering just never adopted it. Extending it to text is mostly wiring, plus removing hardcoded colors from `surface.rs`.
- **LSP-vocabulary makes the LSP bridge nearly identity.** Choosing LSP token types now means the future `@clay/lsp-*` bridge maps LSP `SemanticTokens`/`Diagnostic`/`Completion` straight onto existing Clay primitives — no parallel taxonomy.
- **Layered decorations avoid provider wars.** Multiple packages decorating the same range compose (syntax + semantic + diagnostic) instead of fighting; override becomes user engine-selection, never silent package self-promotion.

## References

- `decision-logs/2026-07-08-2316-web-tree-sitter-host-adapter-for-syntax-highlighting-and-non-blocking-open-parse.md` — superseded on the engine choice; its non-blocking-open-parse and `finish_task` silent-drop fixes remain binding.
- `src/editor/surface.rs` — `decoration_color()`, `SYNTAX_*_COLOR` constants, `PANEL_COLOR`/`ACCENT_COLOR` (deleted in favor of the `StyleRegistry`).
- `src/protocol/decorations.rs` — `DecorationSpan{kind, style_token:String, priority, provenance}`, `DecorationKind{Syntax, Semantic, Diagnostic, SearchMatch}` (refactored to two-axis token_type + modifiers; `Semantic`/`Diagnostic` activated).
- `src/server/completion.rs` — `CompletionProviderRegistry`, priority ordering (`sort_by(|a,b| b.priority.cmp(&a.priority)…)`), `CompletionCoordinator` scheduling (adds `exclusive` claim and snippet kind).
- `src/protocol/completion.rs` — `CompletionItem{label, insert_text, detail, commit_characters, provenance}` (adds snippet kind).
- `src/protocol/mod.rs` — `RuntimeDiagnostic{severity, code, message}` document-level only; `DiagnosticSpan{range, severity, code, message, source}` is the NEW range-diagnostic primitive.
- `src/packages/record.rs` — `ThemeTokenContributionDescriptor{token, token_type, fallback}` (extended from SDUI to text rendering); `SyntaxGrammarContributionDescriptor`.
- `src/server/syntax.rs` — `TreeSitterSyntaxHandler` (test-only today) becomes the Tier 1 production handler shape.
- `packages/{rust,typescript,javascript,markdown}/package.json` — declare `tree-sitter-wasm` grammars, `highlights.scm` queries, `styleMap` (now mapping to vocabulary tokens).
- `roadmap.md` Phases 18.15–18.21 — implement this decision (vocabulary+theme, tiered engine, range diagnostics, first-party language packages, completion extensions, LSP primitives+authority, LSP bridge packages).
- `.agents/skills/project-patterns/references/language-capability-sequencing.md` — revised to reflect the tiered engine (native Tier 1 + web-tree-sitter Tier 2 + JS Tier 3).
- LSP spec — `SemanticTokenType`, `SemanticTokenModifiers`, `CompletionItem`, `Diagnostic`, `Hover`, `CodeAction` (the vocabulary and bridge source-of-truth).

## Consequences

- **Positive:** One LSP-based vocabulary drives highlighting, theming, semantic highlighting, and LSP integration; native performance for first-party Rust/TypeScript/JavaScript/Markdown; themes (Gruvbox Material Light/Dark first) control all editor rendering from `init.js`; layered decorations compose instead of fighting; completion supports exclusive claim, disable-native, and snippets; LSP intelligence arrives as opt-in packages without widening core authority; the non-blocking-open-parse and silent-drop fixes from 2316 still land.
- **Risks / follow-up work:** (1) Tier 1 adds `tree-sitter-rust/-typescript/-javascript/-markdown` as runtime dependencies (not dev-dependencies) — crate versions compatible with `tree-sitter 0.24` must be verified at the entry gate. (2) Real `highlights.scm` query files and (for Tier 2) `grammars/*.wasm` artifacts must be produced/vendored. (3) The two-axis `DecorationSpan` migration must preserve today's `style_token` behavior during transition (compat mapping). (4) A separate decision log is required before any LSP package ships, recording the `language-server` subprocess+filesystem authority and user opt-in. (5) The `StyleRegistry` becomes a hot-path read; it must be cheap to query per-span during paint.
- **Revisit when:** (a) a Tier 1 native crate proves incompatible with the host tree-sitter version and blocks a first-party language — then re-evaluate Tier 2 for that language; (b) theme overrides prove insufficient for some rendering need (e.g. animated decorations) and a richer style primitive is justified; (c) LSP subprocess authority needs broader or more granular trust than the single `language-server` permission provides; (d) third-party package grammars require trust/integrity rules the first-party-only resolver cannot enforce — then a dedicated security/trust decision is required before enabling third-party `tree-sitter-wasm` grammars, per `language-capability-sequencing.md`.
