---
name: create-decision-log
description: Maintain a chronological project decision log. Use whenever a conversation with the user results in, approaches, or appears to finalize a project decision, including architecture, dependency, API, workflow, implementation, product, policy, naming, storage, deployment, or process decisions. Before logging, use this skill to research evidence, compare alternatives, ask for explicit user approval if not already given, and then create or update files under the project-root decision-logs/ directory only after approval.
---

# Create Decision Log

## Purpose

Create an evidence-backed, chronological record of finalized project decisions in `decision-logs/` at the project root. Treat the log as project memory: concise enough to scan, complete enough to explain why the decision was made later.

## Required Workflow

1. **Detect a decision candidate**
   - Use this skill when the user and agent converge on a choice or when the agent recommends a direction that may be adopted.
   - Do not log brainstorms, rejected ideas, or tentative preferences unless the user explicitly approves recording them as a finalized decision.

2. **Research before finalizing**
   - Gather evidence appropriate to the decision.
   - For library/framework/SDK/CLI/cloud-service decisions, use `find-docs` first or the available documentation lookup tools.
   - For broader/current facts, use web search or authoritative project documentation.
   - Prefer official docs, source repositories, standards, issue trackers, release notes, and existing project files over unverified claims.
   - Capture references as links, file paths, commands, or docs consulted.

3. **Compare alternatives**
   - Identify realistic alternatives considered, including the status quo when relevant.
   - Record the main tradeoffs: benefits, costs, risks, constraints, and why alternatives were not chosen.

4. **Get explicit approval**
   - Before creating the record, ask the user to confirm the exact decision.
   - Proceed only when the user explicitly approves, e.g. “yes”, “approved”, “log this”, “finalize it”, or equivalent.
   - If approval is ambiguous, ask a follow-up question. Do not create a decision log from implied agreement alone.

5. **Write the decision log**
   - Create `decision-logs/` in the project root if it does not exist.
   - Determine the project root as the Git root when available (`git rev-parse --show-toplevel`), otherwise use the current working directory.
   - Use a chronologically sortable filename:
     - `YYYY-MM-DD-HHMM-brief-kebab-title.md`
     - Use local system time unless the user requests another timezone.
   - Keep existing logs immutable unless correcting factual errors or the user asks for an update. For superseding decisions, create a new log and reference the earlier log.

6. **Update project pattern memory when available**
   - If the project has `.agents/skills/project-patterns/`, use that skill after writing an approved decision log.
   - Extract only stable reusable planning guidance from the decision, not the full log text.
   - Add or update concise files under `.agents/skills/project-patterns/references/` when a new pattern is added or an existing pattern changes.

## Decision Log Template

Use this structure for each log:

```markdown
---
date: YYYY-MM-DD HH:MM
status: approved
decision_about: "Short topic"
proposed_by: "user|agent|both|name if known"
explicitly_approved_by_user: true
---

# Decision: Short title

## Decision

State the finalized decision in 1-3 sentences.

## Context

Explain the problem, constraint, or discussion that led to the decision.

## Approval

- Proposed by: ...
- Approved by user: Yes
- Approval evidence: Quote or summarize the user's approving message.

## Alternatives Considered

1. **Alternative name** — outcome/reason not selected.
2. **Alternative name** — outcome/reason not selected.

## Rationale and Evidence

Explain why this option was chosen. Include facts, research findings, project constraints, and tradeoffs.

## References

- [Reference title](URL) — why it matters.
- `path/to/file` — relevant project evidence.
- Command/output summary if useful.

## Consequences

- Positive outcomes expected.
- Risks or follow-up work.
- Conditions that would cause revisiting this decision.
```

## Quality Bar

- Be thorough on facts and evidence; do not record unsupported claims as fact.
- Separate evidence from assumptions.
- Include enough context that a future agent can understand the decision without reading the entire chat.
- If evidence is unavailable or research was intentionally skipped by user request, say so explicitly in the log.
- Do not disclose secrets, credentials, private tokens, or sensitive personal data in logs.
