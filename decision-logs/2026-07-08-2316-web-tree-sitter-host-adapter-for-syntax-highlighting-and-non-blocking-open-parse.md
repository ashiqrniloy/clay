---
date: 2026-07-08 23:16
status: superseded
superseded_by: 2026-07-09-0352-tiered-tree-sitter-themable-syntax-vocabulary-theme-registry-and-opt-in-lsp
decision_about: "Syntax highlighting engine for Rust/TypeScript/JavaScript/Markdown decorations and the open-time parse pipeline"
proposed_by: "agent"
explicitly_approved_by_user: true
---

# Decision: Generic web-tree-sitter host adapter for syntax highlighting, with markdown decoration folding and non-blocking open-time parse

> **SUPERSEDED (2026-07-09)** by `decision-logs/2026-07-09-0352-tiered-tree-sitter-themable-syntax-vocabulary-theme-registry-and-opt-in-lsp.md` on the **engine choice** (web-tree-sitter-only → tiered: native Tier 1 + web-tree-sitter Tier 2 + JS Tier 3) and on the **vocabulary/theming** axis this decision never considered. The **non-blocking open-time parse** and **`ParseCoordinator::finish_task` silent-drop fix** below remain binding and are carried forward verbatim into the superseding decision.

## Decision

Adopt a generic host-side `web-tree-sitter` (tree-sitter compiled to WebAssembly, run inside the existing Deno/V8 worker) adapter as the single syntax-highlighting path for packages that declare a `tree-sitter-wasm` grammar. Packages own grammar (`grammars/*.wasm`) + query (`queries/highlights.scm`) assets; the host loads them generically through the existing `clay.parse.serverRegisterParseHandler` JS path. Markdown decorations fold onto this same generic path; the Markdown preview SDUI panel stays package-JS. Languages with no tree-sitter grammar remain supported through the existing package-JS parse-handler fallback (the route `@clay/markdown` uses today). Separately, fix `ParseCoordinator::finish_task` to surface a `RuntimeDiagnostic` on handler errors instead of silently dropping them, and make open-time parse non-blocking so the document renders immediately and `DecorationSet` paints when background parse completes.

Native `tree-sitter-*` Rust crates compiled into Clay core are **rejected** as the highlighting engine.

## Context

Manual `cargo run` testing on Linux/GNOME (Plans 043/044) found that syntax highlighting does not appear for `.rs`/`.ts`/`.js` files and that Markdown highlighting is hidden by a `clay.parse.open_activation_timeout` hang. Investigation established three facts:

1. `@clay/rust`, `@clay/typescript`, and `@clay/javascript` register only grammar/mode metadata via `clay.syntax.serverRegisterSyntaxGrammar`. They declare `permissions: ["...parse-document","render-decorations"]` and `contributions.syntaxGrammars` but never register a parse handler, so `schedule_open_parse` (`src/server/connection.rs`) finds no registered handler and publishes no `DecorationSet`.
2. The native `TreeSitterSyntaxHandler` (which produces `DecorationSpan`s from tree-sitter) has **zero production call sites** — it is constructed only in `tests/syntax_grammar.rs` using `tree-sitter-rust/-typescript/-javascript` crates that are `[dev-dependencies]` only. So the grammar metadata declared by first-party packages is never bound to an executing parser in production.
3. `ParseCoordinator::finish_task` (`src/server/parse_coordinator.rs`) silently drops handler errors (`let Ok(update) = result else { failed_tasks += 1; return; }`), so when a registered handler (e.g. Markdown's `parser.js`) throws or times out, no update and no diagnostic are published. `schedule_open_parse` waits 6s on `parse_coordinator.next_update()`, then emits `clay.parse.open_activation_timeout`, and the editor never receives decorations. The visible "no highlighting" symptom is a missing `DecorationSet`, not a rendering/color bug (the per-token-family `decoration_color()` mapping added in Plan 044 is correct).

The packages' own `grammars/*/README.md` already states the intended contract: "Clay loads it as a package-root-confined `tree-sitter-wasm` artifact and ... is bound only by Clay-owned server syntax code." The `.wasm` artifacts themselves are not committed today, so any route that executes the declared grammars must also produce/vendor those artifacts.

The user raised three structural objections to the native-crates route: (1) "all future modes depend on the core app" (a rebuild + new `[dependencies]` line per language, violating the package-owns-language-authority model); (2) split-brain redundancy between package-JS mode logic and host-Rust decoration logic for one language; (3) no coverage for languages with no tree-sitter grammar crate. These objections are decisive.

## Approval

- Proposed by: agent (after ruling out the native-crate, wasmtime-wasm, and package-only-JS options against the user's performance and architecture questions).
- Approved by user: Yes
- Approval evidence: User said "Go ahead with creating the plan document with @create-plan and decision log with @.agents/skills/create-decision-log/ as proposed" after the agent proposed Option 2b (generic `web-tree-sitter` host adapter, package-owned grammar/query, markdown decoration folding, package-JS fallback for non-tree-sitter langs, plus the silent-drop + non-blocking open-parse fix).

## Alternatives Considered

1. **Native `tree-sitter-*` crates compiled into Clay core (Option 1).** Lowest latency, incremental reparse, no wasm runtime, reuses the existing tested `TreeSitterSyntaxHandler`. Rejected: hard-codes the supported language set into the host (every new language needs a Clay rebuild + new dep), splits language logic between package JS and host Rust (split brain), and cannot highlight languages with no `tree-sitter-foo` crate. It is a per-language Rust branch in disguise, which Plan 044's primitive review and `language-capability-sequencing.md` ("Do not start by bundling Rust/TypeScript/JavaScript grammar ownership into Clay core") already forbid.

2. **Enable tree-sitter's native `wasm` feature + `wasmtime-c-api` on the Rust side (original proposal).** Loads package `.wasm` grammars generically. Rejected: pulls a heavy `wasmtime` native dependency and a second WASM runtime into Clay core when the project already ships a JS runtime (Deno/V8) that runs wasm. Keeps host generic but adds the heaviest dependency for a capability V8 already has.

3. **Per-package JS highlighter only (Option 3, e.g. `highlight.js`/`Prism`).** Zero host change, fully generic, covers non-tree-sitter langs. Rejected *as the default*: loses incremental reparse (full re-tokenize per edit), weaker quality (regex can't do nested JSX/templates), and per-language maintenance grows with every package. Retained as the documented per-package fallback for languages with no tree-sitter grammar.

4. **Status quo + raise timeouts.** Not viable: the silent-drop in `finish_task` hides the real failure class, and a 6s blocking wait on open makes every file-open sluggish regardless of engine.

## Rationale and Evidence

- **One generic mechanism for all declared grammars.** A host-side `web-tree-sitter` module loads any package-declared `tree-sitter-wasm` grammar + `queries/highlights.scm` and emits `DecorationSpan`s through the same `clay.parse.serverRegisterParseHandler` JS path Markdown already uses. Adding a new language is a new package, **no Clay rebuild**. This matches the first-party package authority model and the existing pattern (`packages/<rust|typescript|javascript>/package.json` already declare `contributes.syntaxGrammars` with `kind: "tree-sitter-wasm"`, `grammar.path: "./grammars/<lang>.wasm"`, `queries.highlights`, and `styleMap`).

- **No split brain.** All language-specific logic (mode patterns, commands, completion, decoration) lives in package JS; the host supplies one generic engine, not per-language Rust. Markdown decoration logic moves from `packages/markdown/dist/parser.js` to the same generic adapter; only the preview SDUI panel stays package-JS.

- **Proven long-term performance and incremental reparse.** tree-sitter-wasm runs at roughly 1–2× native speed in modern engines with full `old_tree` incremental reparses (O(changed ranges)). This is the same engine family VS Code, Neovim (wasm), and Pulsar ship. It runs on the existing background parse lane, isolated from edit/render hot paths, so the perf ceiling is the open-time timeout/contention — addressed by the non-blocking parse fix.

- **Non-tree-sitter languages remain supported.** The package-JS parse handler (current Markdown route) is retained as the documented opt-in fallback, so a DSL/regex/toy language with no grammar crate still gets highlighting via its own `parser.js`.

- **Fixes the Markdown timeout root cause, not just the symptom.** Surfacing handler errors as diagnostics and making open-time parse non-blocking removes the 6s blocking starvation that hides every handler failure (Markdown today, every language after) and lets the document render immediately on open.

## References

- `src/server/connection.rs::schedule_open_parse` and `open_document_followup_messages` — the 6s blocking `parse_coordinator.next_update()` loop and `clay.parse.open_activation_timeout` emission.
- `src/server/parse_coordinator.rs::finish_task` — the silent `let Ok(update) = result else { failed_tasks += 1; return; }` drop.
- `src/server/js_runtime.rs::evaluate_js_parse_handler` and `src/server/ops/parse.rs::op_clay_parse_register_parse_handler` — the JS parse-handler bridge and `runtimeBridge` flag the generic adapter will use.
- `src/server/ops/syntax.rs::op_clay_syntax_register_syntax_grammar` and `src/packages/record.rs` (`SyntaxGrammarContributionDescriptor`, `validate_package_asset_path` accepting `.wasm`) — grammar metadata validation already in place.
- `src/server/syntax.rs::TreeSitterSyntaxHandler` — native handler currently with zero production call sites; remains available for tests and as the reference shape for decoration output.
- `packages/rust/package.json`, `packages/typescript/package.json`, `packages/javascript/package.json` — declare `tree-sitter-wasm` grammars, `highlights.scm` queries, and `styleMap` already.
- `packages/rust/grammars/README.md`, `packages/typescript/grammars/README.md` — document the package-root-confined `tree-sitter-wasm` contract and that Clay binds the artifact through Clay-owned server syntax code.
- `packages/markdown/dist/parser.js` and `packages/markdown/dist/sdui.js` — current package-JS decoration parser and (separate) preview SDUI panel.
- `.agents/skills/project-patterns/references/language-capability-sequencing.md` — "Do not start by bundling Rust/TypeScript/JavaScript grammar ownership into Clay core"; grammar contributions expressed through generic package primitives.
- `.agents/skills/project-patterns/references/mode-primitive-first.md` — parse work stays background/bounded via the permanent JS runtime + token-backed JS→Rust `ParseHandler` bridge; no new mode-specific Rust branches.

## Consequences

- **Positive:** Rust/TypeScript/JavaScript/Markdown highlighting on one generic package-owned path; new tree-sitter languages need no Clay rebuild; Markdown preview unaffected; open-time no longer blocks on parse; handler failures become visible diagnostics instead of silent 6s hangs; the existing `decoration_color()` token-family mapping needs no change.
- **Risks / follow-up work:** real `grammars/*.wasm` grammar artifacts must be produced and committed (currently only READMEs exist) — a vendored upstream-release or build step is required; the `web-tree-sitter` WASM runtime must be bundled with the host (bundle-size cost, no native `wasmtime` dep on the Rust side); parse still runs under the JS worker timeout so large/slow files must degrade gracefully (the non-blocking open fix is what makes this acceptable); the native `TreeSitterSyntaxHandler` remains test-only unless a later decision reconsiders a hybrid fast path for compiled-in languages.
- **Revisit when:** a measurable highlight latency regression in production builds shows the wasm-worker path is the bottleneck (at that point consider Option 1 as a compiled-in fast-path fallback, layered on top of, not replacing, the generic adapter), or when third-party package grammars require trust/integrity rules that the current first-party-only resolver cannot enforce (then a dedicated security/trust decision is required before enabling third-party `tree-sitter-wasm` grammars, per `language-capability-sequencing.md`).