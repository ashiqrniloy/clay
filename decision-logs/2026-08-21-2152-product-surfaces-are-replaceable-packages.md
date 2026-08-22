---
date: 2026-08-21 21:52
status: approved
decision_about: "Product landings and agent profiles are first-party packages"
proposed_by: both
explicitly_approved_by_user: true
---

# Decision: Product surfaces are replaceable first-party packages

## Decision

Clay product surfaces — the default landing, Chat, Coding Agent, and later
Work / PA / Research agents — are first-party JS packages on Clay primitives.
Users enable them with one-line `loadPackage`. A third-party package may
`replaces` a landing or agent, or `extends` declared extension points, through
the existing package graph and exact user approval. Replacement code stays in
the third-party runtime.

Clay core owns the host: pane topology, catalog widgets, Command Centre, tab
bar, native file/folder dialogs, the `clay-agent` Prism process, credential
vault, and `agent.*` IPC. Packages never spawn or speak to the daemon. Without
an entry-surface package, empty tabs show Open File / Open Folder only.

## Context

Phase 25 was first drafted as an irreplaceable Clay-native Chat widget
(`PaneContent::Agent`, packages cannot replace the agent view). That fought
Clay’s package model (`extends` / `replaces`, two trust domains, explicit
`init.js` loads) and the user’s rule that product behavior should be
configurable, replaceable, and extendable.

The no-ACP host decision still stands: Prism runs in a Clay-owned Node child
and talks Clay IPC. This decision only answers **who owns the product UI and
`AgentDefinition` registration** — packages — versus **who owns process,
secrets, and primitives** — core.

## Approval

- Proposed by: both (user: replaceable/extendable product surfaces; agent:
  `@clay/chat` / `@clay/coding-agent` on generic pane-content contribution)
- Approved by user: Yes
- Approval evidence: User said “log it” after the standing rule was stated:
  landing + Chat = `@clay/chat`; Coding Agent = `@clay/coding-agent`; third
  party `replaces` / `extends` with approval; core = host + primitives.

## Alternatives Considered

1. **Chat as irreplaceable native `PaneContent::Agent`** — rejected. Smaller
   first diff; blocks a different landing and makes every later agent a core
   stub.
2. **`clay-agent` itself as a Clay JS package** — rejected. Credentials and
   process spawn are core trust-boundary work, not package-triggered grants.
3. **Silent compiled Chat with no `loadPackage`** — rejected for a replaceable
   product surface. Unlike `core.text`, Chat is optional product chrome.
   Canonical default is `loadPackage("@clay/chat")`.
4. **Coding Agent as a core disabled picker row** — rejected. Phase 29
   registers it by loading `@clay/coding-agent`.
5. **Packages own Command Centre or native dialogs** — rejected. Pickers and
   OS dialogs stay host-owned; packages invoke the same `agent.*` / document /
   workspace commands.

## Rationale and Evidence

- User constraint: everything should be configurable, replaceable, extendable.
- `decision-logs/2026-07-21-0001-two-package-runtime-trust-domains.md`:
  third-party `replaces` of a first-party package withdraws it atomically;
  replacement stays third-party; Clay core/bootstrap is not package-replaceable.
- `decision-logs/2026-06-09-0219-explicit-init-js-package-loading-with-one-line-defaults.md`:
  packages are opt-in via `init.js`, preferably one `loadPackage` line.
- `decision-logs/2026-06-09-1431-clay-owned-shell-layout-and-package-ui-contribution-model.md`:
  Clay owns shell, slots, catalog, Masonry; packages declare inert SDUI. This
  decision keeps that split and opens empty-tab `main` as the missing
  contribution (today “not yet public” in
  `docs/reference/primitives/shell-layout-strategy.md`).
- `decision-logs/2026-08-21-1758-native-prism-host-no-acp-cli-parity.md` is
  **not** superseded: still no ACP/AG-UI; still one `clay-agent`.
- `@clay/settings` already ships product UI as a bundled package. Chat/landing
  follow that, not a second native chrome exception.
- Plan 087 / `creating-packages.md` “packages cannot replace welcome” remains
  true for the **core fallback** `WelcomeWidget`. It does not apply to the
  product landing once `@clay/chat` (or a replacement) contributes pane content.

Assumptions, not yet implemented: pane-content contribution API, `@clay/chat`
bundle, and `textArea` if the composer needs multiline. Those are Phase 25
plan work, not this decision’s runtime.

## References

- `decision-logs/2026-08-21-1758-native-prism-host-no-acp-cli-parity.md` —
  host/transport (unchanged).
- `decision-logs/2026-07-21-0001-two-package-runtime-trust-domains.md` —
  replace/extends + two Deno domains.
- `decision-logs/2026-06-09-0219-explicit-init-js-package-loading-with-one-line-defaults.md`
  — one-line `loadPackage`.
- `decision-logs/2026-06-09-1431-clay-owned-shell-layout-and-package-ui-contribution-model.md`
  — Clay-owned shell; package SDUI.
- `roadmap.md` Phase 25 / 29 (updated to match this decision).
- `plans/096-Phase25-AI-Native-Prism-Host-and-Chat.md`
- `docs/wiki/modules/phase25-agent-host-primitive-review.md`
- `docs/reference/packages/creating-packages.md` Plan 087 welcome contract
  (fallback only after this decision).
- `packages/settings/` — first-party SDUI package precedent.

## Consequences

- Phase 25 ships generic pane-content contribution + `@clay/chat` (Chat
  profile, default landing, extension points). Core fallback stays file/folder.
- Phase 29 ships `@clay/coding-agent` on the same host; no core disabled stub.
- Later agents are more first-party packages, not reserved core picker names.
- `creating-packages.md` and the clay-ui catalog must be updated in Phase 25:
  landing is replaceable; Command Centre / tab bar / dialogs are not.
- Revisit only if a surface must be non-replaceable for a security boundary
  (vault internals stay core) or if pane-content contribution proves unsafe.
  Do not revisit ACP here.
