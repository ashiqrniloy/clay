---
name: clay-patterns
description: Apply Clay's project-specific architecture patterns when writing, reviewing, or updating plan documents, roadmap entries, implementation approaches, decision logs, or architecture/design documentation. Use this skill any time an agent writes a plan document for Clay, updates a plan after a decision, designs server/client/protocol/extension/documentation behavior, or records a decision log that may add or change Clay patterns. After a new decision log is approved, use this skill to update pattern references so future plans inherit the decision.
---

# Clay Patterns

Use this skill to keep Clay plans aligned with approved architecture decisions and roadmap patterns.

## Required Workflow

1. **Load relevant pattern references before writing a plan.**
   - For any Clay plan, read `references/planning-checklist.md` first.
   - Then read only the pattern files relevant to the work.
2. **Apply ownership boundaries explicitly.**
   - State what belongs to the server, client, protocol, behavior manifest, extension runtime, documentation registry, or future phase.
3. **Avoid temporary architecture drift.**
   - If a phase omits enforcement, document it as a scoped limitation of the final model, not as a different model.
4. **Preserve performance constraints.**
   - Ordinary typing must remain client-first and must not synchronously wait on IPC/server/JavaScript.
5. **Keep documentation as code.**
   - Any new public API/protocol/command/manifest/permission surface must include an inspectable documentation path.
6. **When a new decision log is recorded, update this skill.**
   - Read the new decision log.
   - Decide whether it adds a new reusable Clay pattern or changes an existing one.
   - Edit or add files under `references/`.
   - Update the References section below with a concise description.
   - Do not duplicate full decision-log text; extract stable planning guidance.

## References

- `references/planning-checklist.md` — Required checklist for any Clay plan: authority, performance, documentation, tests, security, phase boundaries, and decision-log updates.
- `references/authority-boundaries.md` — Server/client ownership model: server-authoritative documents, optimistic client shadows, editable leases, read-only observers, and per-document ordering.
- `references/behavior-manifests.md` — How server-owned behavior becomes client-executed hot-path behavior through inert, versioned manifests and routing policies.
- `references/extensions-and-ai.md` — How JavaScript extensions, hot reload, AI mutations, locks, and future WASM behavior modules fit without putting arbitrary code on the client hot path.
- `references/documentation-as-code.md` — Self-documenting program pattern: documentation registry, generated human/agent references, and tests/CI that reject undocumented public surfaces.
- `references/protocol-and-performance.md` — IPC/protocol performance patterns: rkyv boundary, version metadata, deltas instead of full documents, bounded queues, and no synchronous keypress round trips.

## Output Requirements for Plans

When writing or updating a Clay plan, include or verify:

- Chosen approach references the relevant Clay patterns.
- Server/client/protocol/extension/documentation responsibilities are explicit.
- Performance acceptance criteria mention hot-path constraints where relevant.
- Security acceptance criteria say what authority is not introduced in the phase.
- Phase compromises are described as limitations of the approved model, not new architecture.
- Tests include documentation/manifest/protocol coverage when new public surfaces are added.
