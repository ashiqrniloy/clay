---
date: 2026-09-20 20:49
status: approved
decision_about: "Agent autonomy default and resume behavior"
proposed_by: user
explicitly_approved_by_user: true
---

# Decision: Agent autonomy is on by default (opt-out); a resumed session restores the autonomy recorded with it

## Decision

1. **Default is autonomy.** A new agent session is fully autonomous unless the
   caller explicitly blocks it: the daemon's `session.new` reads
   `params.fullAutonomy !== false`, so a missing flag means autonomy on. A
   caller blocks autonomy by passing `fullAutonomy: false` at creation (the
   protocol's `NewSession.full_autonomy`) or afterwards with
   `agent.setFullAutonomy({ sessionId, enabled: false })`.
2. **The code is the source of truth here; the docs catch up.** Decision 2157's
   statement that the autonomy toggle is "off by default" is superseded for the
   default only. 2157's acceptance split still stands unchanged for sessions
   with autonomy **off**: inside the workspace everything is free,
   out-of-workspace mutations and shell-metacharacter commands require an
   approval decision.
3. **A resumed session restores the autonomy recorded with it.** `ensureLive`
   builds its live record from the session's recorded `fullAutonomy` value, and
   records written before the field existed default to autonomy (default on) —
   the same rule as creation. Resuming no longer silently re-arms approvals.

## Context

Plan 130's Clay-JS-API verification surfaced a three-way divergence that had
been recorded but never resolved: the daemon has created sessions opt-out since
the 2026-09-05 user decision (`clay-agent/src/host/sessions.ts`, comment
"Approvals are opt-out by default (user decision 2026-09-05)"), while
`docs/reference/clay-js-api/agent/set-full-autonomy.md`,
`docs/reference/clay-js-api/api-inventory.toml` and
`tests/clay_js_api_inventory.rs`
(`agent_configuration_options_are_documented_custom_properties_with_decision_defaults`)
still asserted `default:boolean=false` from decision 2157. The manual test pass
found a second, related inconsistency: `ensureLive` built the session from the
recorded autonomy but wrote the *live record* — the value the run loop actually
gates on — with `fullAutonomy: false`, so a resumed session came back
non-autonomous while its own agent had been built autonomous.

The plan's follow-up item asked for one explicit choice. The user resolved it:
"Default should be autonomy. User can configure to block autonomy."

## Approval

- Proposed by: user
- Approved by user: Yes
- Approval evidence: "Default should be autonomy. User can configure to block
  autonomy" (2026-09-20, resolving plan 130's follow-up item "Autonomy default:
  decide doc-or-code").

## Alternatives Considered

1. **Docs win — make the daemon default `fullAutonomy: false`** (approvals on
   by default, 2157 as written) — rejected by the user: sessions should run
   straight through by default; approval round trips on gated calls stay
   available as an explicit choice, not the default.
2. **Add a global "default autonomy" configuration key** (init.js or
   `preferences.json`) — rejected: autonomy is session state and the existing
   per-session surfaces already let a caller block it; a new config key would
   contradict the documented "no hidden configuration keys" rule and the
   existing statement that autonomy has no `default` init.js key.
3. **Keep the divergence documented in both places** (status quo) — rejected:
   the docs asserted a default the code never had, and the manual test plan had
   to warn readers about it; a guard test that reads both the doc metadata and
   the code expression is cheaper than explaining the gap again.
4. **Re-arm approvals on resume** (keep `ensureLive`'s hardcoded `false`, treat
   a restart as a fresh consent gate) — rejected: it silently contradicts the
   session's own recorded state and surprised the resume path during the A2
   work; the explicit opt-out is the consent gate, and it now survives a
   restart.

## Rationale and Evidence

- Runtime behavior (before the docs change): `clay-agent/src/host/sessions.ts`
  — `const fullAutonomy = params.fullAutonomy !== false;` in `sessionNew`.
- What the flag actually controls, verified in code:
  `clay-agent/src/host/session-run.ts` (`interruptBeforeTool: !live.fullAutonomy`
  — autonomy on streams through, autonomy off suspends gated calls as
  `agent_suspended` with `pendingDecisions`) and
  `clay-agent/src/coding-tools.ts` (`createClayAcceptancePolicy`: reads free,
  in-root mutations free; `approveGate` allows out-of-root mutations when
  autonomy is on, otherwise asks the host approval path).
- The gate itself is unaffected: the acceptance policy, the host approval
  callback, and `agent.resumeRun` are unchanged — only the default of the
  toggle and the resume value are decided here.
- The live-record bug: `ensureLive` called `createSession(... fullAutonomy:
  metadata.fullAutonomy !== false ...)` but then `liveSessionFor(..., {
  fullAutonomy: false })`.
- Divergence sites corrected by this decision: the reference page, the API
  inventory, the generated registry, the canonical
  `examples/config/init.js` comment block, the code wiki's acceptance-policy
  paragraph, and the guard test — which now also pins the code expression so
  the two sides cannot drift apart again.

## References

- `decision-logs/2026-08-30-2157-default-acceptance-workspace-free-host-write-gated.md`
  — the acceptance policy whose toggle default is superseded here.
- `docs/reference/clay-js-api/agent/set-full-autonomy.md`,
  `docs/reference/clay-js-api/api-inventory.toml` — the documented surface
  updated to the default.
- `tests/clay_js_api_inventory.rs` — guard pinning the documented default and
  the daemon expression.
- `clay-agent/src/host/sessions.ts`, `clay-agent/src/host/session-run.ts`,
  `clay-agent/src/coding-tools.ts` — the behavior this decision describes.
- `test-plan/16-agent-host.md` steps A5–A7 — the manual steps that recorded the
  divergence, now stating the resolved policy.

## Consequences

- Positive: sessions run to completion by default (no approval round trip on
  gated calls); a caller who wants gating blocks autonomy per session, and that
  choice now survives a daemon restart.
- Positive: one source of truth — the docs, the inventory, the generated
  registry, the example config, and the wiki all state the opt-out default and
  cite this log.
- Risk (accepted, explicitly): with autonomy on by default, out-of-workspace
  writes, delete/move, and shell-metacharacter commands execute **without** an
  approval prompt in every new session, including sessions created by the
  coding-agent panel (which sends no `full_autonomy` field). Users who want
  prompts must block autonomy explicitly. The reference page's security note
  says this in plain language.
- Follow-up (not blocking): the panel has no autonomy toggle yet; the protocol
  field `NewSession.full_autonomy` exists for one, and the panel's choice would
  then become the effective default for UI-created sessions.
- Revisit if: the coding-agent panel gains an autonomy toggle, or host-write
  gating must be enforced independently of the per-session toggle.
