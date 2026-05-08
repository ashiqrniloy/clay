---
date: 2026-05-08 14:19
status: approved
decision_about: "Markdown-authoritative documentation registry"
proposed_by: "both"
explicitly_approved_by_user: true
---

# Decision: Markdown-authoritative documentation registry

## Decision

Clay documentation source of truth will be authored as Markdown files. A master Markdown index will enumerate and link documented public surfaces, and generated/programmatic documentation registries will be derived from those Markdown files rather than authored separately.

Cargo tests must run the documentation registry generation/check path and fail when a public interface is missing from the generated registry, when required Markdown metadata is missing, or when checked-in generated registry artifacts are stale.

## Context

Phase 3 introduces Clay's self-documenting program contract before protocol, behavior manifest, command, extension, and AI-tool surfaces grow. An earlier plan separated generated indexed artifacts and app/programmatic lookup too strongly, which risked long-term drift between human documentation, agent-readable indexes, and in-app documentation.

The user clarified that all documentation should be Markdown, indexed by a master Markdown file, then converted into registries for app/programmatic access. The registry should be generated or updated from Markdown, and public interface tests should fail if public functions or interfaces are missing registry documentation.

## Approval

- Proposed by: both
- Approved by user: Yes
- Approval evidence: The user said, "Okay. I agree with the plan now. Update the @plans/004-Phase3-SelfDocumentingProgramContract.md plan and the @.agents/skills/project-patterns/references/documentation-as-code.md file based on this method. Also record a decision about this using the @.agents/skills/create-decision-log/ skill."

## Alternatives Considered

1. **Separate authored registry plus generated docs** — Rejected because maintaining registry entries separately from Markdown creates drift risk between human docs, agent indexes, and app lookup.
2. **Generated Markdown from code/metadata as the source of truth** — Rejected for Phase 3 because the desired authoring model is human-readable Markdown first, with generated app registries derived from it.
3. **Markdown-only documentation with no generated registry** — Rejected because the app, agents, command palette/help, extension tooling, and tests need structured programmatic lookup rather than Markdown scraping.
4. **Cargo tests silently update generated registry files** — Rejected as the default because tests should be deterministic and should fail with instructions when checked-in artifacts are stale. A separate update function/command may write the registry.

## Rationale and Evidence

- Markdown is the most inspectable format for users, contributors, and AI agents. Making it authoritative keeps human docs close to the actual explanation of each public surface.
- A master Markdown index provides a single navigable map of public documentation and an explicit inclusion list for registry generation.
- Generating registries from Markdown prevents separate human and app/agent documentation sources from diverging.
- Cargo tests already form Clay's standard verification workflow in earlier plans, so public-interface documentation coverage belongs in `cargo test`.
- The generated registry remains necessary because the app and AI tooling need stable structured lookup by public surface ID, kind, owner, tags, and metadata.

## References

- `plans/004-Phase3-SelfDocumentingProgramContract.md` — Phase 3 plan being updated to make Markdown authoritative and registries generated.
- `.agents/skills/project-patterns/references/documentation-as-code.md` — Project pattern updated so future plans preserve the Markdown-authoritative registry workflow.
- `roadmap.md` — Requires Clay to become a self-documenting program with documentation as a code contract.
- `decision-logs/2026-05-08-0408-server-authoritative-documents-client-behavior-manifests.md` — Earlier decision requiring public protocol/API/command/manifest/permission/tool surfaces to be documented and inspectable.

## Consequences

- Future public surfaces must be documented in Markdown and included in the master Markdown index.
- Generated registries are derived artifacts; they must not become the independently edited source of truth.
- Phase 3 must include generation and update functions/commands, plus lookup APIs over the generated registry.
- `cargo test` must fail when public interface documentation coverage or generated registry freshness is missing.
- This decision should be revisited only if Markdown becomes insufficient to express required metadata or if registry generation becomes too costly for normal tests.
