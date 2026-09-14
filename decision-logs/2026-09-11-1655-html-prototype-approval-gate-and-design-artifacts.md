---
date: 2026-09-11 16:55
status: approved
decision_about: "UI work requires an HTML prototype and explicit user approval; design-artifacts/ splits prototypes from approved designs"
proposed_by: "user (instruction), agent (contract wording)"
explicitly_approved_by_user: true
---

# Decision: HTML prototype plus explicit user approval gates all UI work

## Decision

Every Clay UI change is prototyped as checked-in HTML before it is implemented,
and implemented only from an explicitly user-approved artifact:

- `design-artifacts/prototypes/<slug>/` — exploratory, self-contained HTML
  (opens over `file://`, no build step), rendered against the shipped content
  themes with every shippable component state. **No authority**; may be replaced
  or discarded.
- `design-artifacts/approved/<slug>/` — the user-approved variant, copied there
  with a recorded approval (date, approving statement, chosen variant, requested
  changes, coverage). **Binding and append-only**: a later change is a new
  variant plus a new approval, never an in-place edit. Implementation tasks cite
  this path, and visual review compares the running app against it.
- `DESIGN.md` stays normative for the language itself; where an approved artifact
  and the specification disagree, the specification wins and the artifact is
  corrected through a new approval.
- `design-artifacts/README.md` is the contract page; the plan-writing duty is
  mandated in `.agents/skills/create-plan/` (prototype task + freeze/approval
  task before any implementation task), the execution duty in
  `.agents/skills/clay-execution/references/ui.md` and `planning-checklist.md`.

## Context

The 2026-09-11 design round produced four HTML screens; the user selected one
language and then hit a real workflow gap: the proposal artifacts documented both
the chosen and the rejected direction in one undifferentiated folder
(`design-artifacts/DS/`), nothing prevented a later agent from implementing
straight from source without a visual reference, and no rule made the chosen
design reproducible for anyone who was not in the conversation. The user
instructed a create-plan change so that future UI work cannot skip the visual
step and cannot confuse exploration with a decision. Two failure modes are being
closed: agents implementing UI from imagination (no prototype, no approval), and
approved designs drifting silently as implementation proceeds (no binding
artifact, no deviation record).

## Approval

- Proposed by: agent (folder contract, task templates, deviation policy);
  required behaviour stated by the user.
- Approved by user: Yes.
- Approval evidence: "The first thing you need to do is to update the
  `.agents/skills/create-plan/` skill to mandate html prototype and user approval
  for any future UI work and artifacts should be saved in `design-artifacts/`
  which are then strictly followed during UI development with differentiation
  between prototypes and approved designs."

## Alternatives Considered

1. **Keep a single flat `design-artifacts/` folder, mark approval in a README** —
   rejected: the rejected and the approved direction end up side by side, and the
   binding one is identified only by prose. The 2026-09-11 proposal already
   demonstrated that confusion.
2. **Prototype discipline only in the plan files** (write "prototype first" into
   each UI plan) — rejected: plans are pull-only history; the rule has to live in
   the skill that writes plans and the reference that executes them, or the next
   plan omits it.
3. **Screenshot-only approval (no HTML prototype)** — rejected: screenshots of a
   running build can only be produced after implementation, which inverts the
   gate, and they cannot cover states the build does not have yet.
4. **Approved artifacts as living files edited in place as implementation
   proceeds** — rejected: the reference mutates until it matches the code, so it
   can never prove conformance. Append-only preserves the approved baseline.
5. **Design-tool (Figma) files as the reference** — rejected: not diffable, not
   reviewable in-repo, needs an external service and licence; HTML artifacts open
   with `file://`, diff in git, and render on any machine.

## Consequences

- `design-artifacts/` is tracked in git and now holds `prototypes/`,
  `approved/`, `screenshots/`, and `tools/`; the 2026-09-11 proposal was split
  into `approved/quiet-instrument-language/` (binding) and
  `prototypes/quiet-instrument-language/` (rejected alternative, history).
- Two extra tasks exist in every UI plan; the freeze task cannot be closed by an
  agent alone, so undocumented approval blocks dependent tasks by construction.
- A missing state in the approved artifact stops implementation and requests a
  prototype rather than being improvised.
- `design-artifacts/approved/quiet-instrument-language/theme.css` keeps the two
  proposal-only palettes (Kanagawa Wave, Catppuccin Latte) annotated as
  non-shipping; the artifact contract governs presentation, shipping scope is
  decided by the migration plan.
