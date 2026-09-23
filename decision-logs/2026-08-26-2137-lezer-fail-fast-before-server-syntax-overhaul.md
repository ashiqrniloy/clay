---
date: 2026-08-26 21:37
status: approved
decision_about: "Narrow Plan 100 to a Lezer fail-fast gate before server syntax overhaul"
proposed_by: "agent"
explicitly_approved_by_user: true
---

# Decision: Fail fast on stock Lezer before resuming the server syntax overhaul

## Decision

Clay will narrow Plan 100 to one short, production-representative CodeMirror/Lezer fail-fast spike. The spike tests grammar freshness, WebKitGTK edit/main-thread latency, distant viewport freshness, 10-50 MiB documents, memory, and one/four-pane scaling in order, stopping on the first hard failure.

Plan 100 will not implement frontend-worker Tree-sitter, a hybrid parser, complete Lezer parity, or speculative client package APIs. If Lezer fails, Clay will confirm the approved server-side per-document Tree-sitter-session direction and replan Plan 099 in place. Frontend-worker Tree-sitter may reopen only if the completed server overhaul later misses approved metrics.

## Context

The original Plan 100 proposed comparing current server syntax, an optimized server reference, client Lezer, frontend-worker Tree-sitter, and a hybrid path under a full parity matrix. Subsequent repository, documentation, ecosystem, and isolated performance review found cheap risks capable of disqualifying stock Lezer before that work is justified.

Stock CodeMirror language parsing runs on the renderer main thread. Clay's exact `@codemirror/language 6.12.4` implementation allows up to 20 ms synchronous parse work during state updates and scheduled slices up to 100 ms. Lezer also parses forward from available syntax state rather than providing a generic correct arbitrary-offset base-language parse, creating risk when users jump deep into 10-50 MiB files.

Current official Lezer packages parsed Clay's existing small Rust, TypeScript, JavaScript, and Markdown fixtures without error nodes in an isolated probe. The same probe found recovery-node gaps for modern Rust async closures, let chains, and gen blocks, plus valid TypeScript decorator/import-attribute examples. Dense synthetic 10 MiB inputs also showed document-shape-sensitive latency and memory high-water risk. These probes are advisory, not product acceptance evidence; real Tauri/WebKitGTK traces remain decisive.

## Approval

- Proposed by: Agent recommended retaining Plan 100 but reducing it to ordered Lezer disqualification gates, then replanning Plan 099.
- Approved by user: Yes.
- Approval evidence: User stated, "Approved. Update plan 100 accordingly" after the recommendation to run a short Lezer fail-fast gate, stop on hard failure, defer worker Tree-sitter, and resume/replan Plan 099.

## Alternatives Considered

1. **Execute the original exhaustive Plan 100** - rejected because worker, hybrid, full semantic parity, and package-platform work are speculative until stock Lezer passes cheaper grammar, latency, viewport, and memory gates.
2. **Skip all client-local measurement and resume Plan 099 immediately** - rejected because one bounded real WebKitGTK spike resolves the remaining Lezer uncertainty before production architecture work.
3. **Adopt Lezer from documentation and isolated Node results** - rejected because those results do not represent CodeMirror scheduling, WebKitGTK main-thread behavior, real pane duplication, or Clay process memory.
4. **Run ordered Lezer fail-fast gates, then replan Plan 099** - selected as the smallest experiment that can change the parser-placement decision.
5. **Build frontend-worker Tree-sitter now** - deferred because existing server-native Tree-sitter preserves grammar/query assets, one shared per-document tree, package/headless behavior, and renderer isolation. Worker placement needs evidence that the completed server path still misses metrics.

## Rationale and Evidence

- Lezer's strongest advantages are direct CodeMirror integration, editor-coordinate trees, local highlighting/indent/fold services, and elimination of syntax IPC. These remain worth one bounded real-app test.
- Its main risks align directly with Clay hard requirements: renderer latency, 50 MiB files, distant viewport jumps, multi-pane memory, modern grammar freshness, server/headless consumers, and package trust.
- A failed hard requirement cannot be offset by better integration or a weighted score. Continuing implementation after failure would create throwaway code.
- Server-native Tree-sitter already has current first-party grammar/query investment, off-renderer execution, shared document ownership, and an approved per-document-session/atomic-patch design.
- Replanning Plan 099 in place preserves its audit and universal frontend fixes without creating another plan solely because a disposable spike occurred.

## References

- [Lezer system guide](https://lezer.codemirror.net/docs/guide/) - incremental LR/GLR parsing, tree fragments, error recovery, and grammar model.
- [CodeMirror language reference](https://codemirror.net/docs/ref/#language) - `LanguageSupport`, syntax trees, parsing, indentation, folding, and dynamic language support.
- [Lezer and Tree-sitter comparison by the maintainer](https://discuss.codemirror.net/t/question-difference-between-lezer-and-tree-sitter/3114) - browser/JavaScript integration and compactness tradeoffs.
- [Lezer tree worker-transfer discussion](https://discuss.codemirror.net/t/transfer-tree-to-web-worker/3940) - serialization and structural-sharing costs.
- [Tree-sitter advanced parsing](https://tree-sitter.github.io/tree-sitter/using-parsers/3-advanced-parsing.html) - exact incremental edits, tree reuse, and included ranges.
- `frontend/node_modules/@codemirror/language/dist/index.js` - exact installed 6.12.4 scheduling constants.
- `plans/100-Client-Local-Parsing-Spike-and-Parser-Placement-Decision.md`
- `plans/099-Clay-Editor-Performance-Overhaul.md`
- `decision-logs/2026-08-26-1838-server-syntax-sessions-and-atomic-viewport-patches.md`
- `decision-logs/2026-08-26-1857-spike-parser-placement-before-performance-implementation.md`
- `docs/development/editor-performance-review-2026-08-26.md`

## Consequences

- Plan 100 becomes materially smaller and stops on the first Lezer hard-gate failure.
- Frontend-worker Tree-sitter, hybrid ownership, arbitrary client parser extensions, and complete parity work are not Plan 100 deliverables.
- Plan 099 remains paused only through the fail-fast evidence and parser-placement approval. It is then replanned and resumed in place.
- A Lezer failure confirms the existing server-session direction; it does not trigger another parser prototype.
- An all-pass Lezer result does not silently adopt Lezer. It triggers a new explicit parser-placement approval before additional prototype work.
- Worker parsing reopens only after the completed server-session implementation still misses approved targets and traces attribute the remaining delay to server/bridge placement.
- No package permission, public API, configuration option, adopted-package webview execution, WASM authority, or production dependency is approved here.
