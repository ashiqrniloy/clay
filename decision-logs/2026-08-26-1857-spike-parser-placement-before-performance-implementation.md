---
date: 2026-08-26 18:57
status: approved
decision_about: "Parser-placement spike and decision before editor performance implementation"
proposed_by: "both"
explicitly_approved_by_user: true
---

# Decision: Spike parser placement before editor performance implementation

## Decision

Clay will pause every unchecked implementation task in Plan 099 and run a production-representative parser-placement spike on `spike/client-local-parsing`. After the spike, Clay will obtain explicit approval for one parser direction and create a new combined migration-and-performance plan rather than resume Plan 099 as written.

This decision changes work order only. It does not choose client-local, frontend-worker, server-side, or hybrid syntax architecture.

## Context

Plan 099 originally selected optimized server-side syntax sessions and deferred client-local parsing until after implementation. Follow-up review found that CodeMirror can supply nearly all document-local structural behavior, while the unresolved issues are performance, package trust, Tree-sitter/Lezer compatibility, duplicate trees, headless consumers, and adopted-package fallback.

Implementing Plan 099 first could spend substantial effort on server syntax sessions, patch transport, and parser migration that a later parser-placement decision would replace. A reversible spike can compare current server syntax, an optimized server reference, CodeMirror/Lezer, frontend-worker Tree-sitter, and a single-owner hybrid before production migration begins.

## Approval

- Proposed by: Agent recommended moving the client-local parser spike ahead of server syntax implementation; user selected the branch and replanning workflow.
- Approved by user: Yes.
- Approval evidence: User directed, "create a new plan document for finalizing the recommended spike and the decision before making any performance change implementation in @plans/099-Clay-Editor-Performance-Overhaul.md" and "We do the spike in the spike/client-local-parsing branch."

## Alternatives Considered

1. **Continue Plan 099, then test local parsing** - rejected because parser placement may invalidate server syntax-session, viewport-patch, and package migration work.
2. **Adopt client-local parsing immediately** - rejected because large-file main-thread behavior, package trust, existing Tree-sitter query parity, multi-pane memory, and headless behavior remain unmeasured.
3. **Delete Plan 099 and start migration now** - rejected because its audit, universal frontend findings, targets, and test matrix remain useful evidence.
4. **Pause Plan 099, spike all viable placements, decide, then replan** - selected because it is reversible and orders architectural evidence before production implementation.

## Rationale and Evidence

- CodeMirror 6 supports local Lezer parsing, `LanguageSupport`, dynamic compartment reconfiguration, mixed languages, highlighting, indentation, folding, bracket/comment language data, and custom tree-backed extensions.
- CodeMirror parsing normally remains browser-main-thread work, so 10-50 MiB documents, fling scrolling, multiple panes, and inactive tabs require real WebKitGTK measurement.
- Existing Clay first-party syntax packages use Tree-sitter grammars, SCM highlight/text-object queries, and Clay vocabulary style maps; a Lezer path requires porting or adaptation.
- Arbitrary CodeMirror extensions are executable webview JavaScript, not inert grammar data. Adopted-package behavior therefore requires a trust decision or server-only fallback.
- Plan 099 documents universal bottlenecks such as O(document) position mapping, duplicate sessions, unbounded retained decorations, and broad React work. Those findings remain inputs to the successor plan regardless of parser placement.

## References

- `plans/099-Clay-Editor-Performance-Overhaul.md`
- `plans/100-Client-Local-Parsing-Spike-and-Parser-Placement-Decision.md`
- `docs/development/editor-performance-review-2026-08-26.md`
- `decision-logs/2026-08-26-1838-server-syntax-sessions-and-atomic-viewport-patches.md`
- `docs/reference/primitives/syntax-vocabulary.md`
- `.agents/skills/project-patterns/references/protocol-and-performance.md`
- `.agents/skills/project-patterns/references/tauri-react-client.md`
- CodeMirror documentation resolved through Context7 IDs `/websites/codemirror_net` and `/codemirror/lang-javascript` on 2026-08-26.

## Consequences

- Plan 099 is paused and non-executable until Plan 100 completes its spike and decision workflow.
- Spike code and dependencies stay isolated on `spike/client-local-parsing` and gain no automatic production status.
- No public API, package permission, client-runtime authority, or user configuration is added merely to run the spike.
- The spike must compare a fair optimized-server reference rather than benchmark local candidates only against known current server defects.
- A later explicit decision log will select parser placement and supersede the parser-placement portion of the earlier server-syntax decision.
- The successor plan will combine selected migration work with still-valid Plan 099 performance fixes and will mark Plan 099 superseded.
