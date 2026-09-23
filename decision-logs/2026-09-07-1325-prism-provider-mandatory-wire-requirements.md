---
date: 2026-09-07 13:25
status: approved
decision_about: "Provider-mandatory wire requirements are Prism-side, not host-side (OpenCode Go x-opencode-session)"
proposed_by: user
explicitly_approved_by_user: true
---

# Decision: Prism must enforce provider-mandatory wire structures deterministically; hosts stopgap only

## Decision

Where a provider's wire protocol makes a structure mandatory (e.g. OpenCode Go's required `x-opencode-session` header), Prism must enforce it deterministically on the Prism side for all providers and all internal call sites. Clay hosts must not need to know which optional request policies are actually mandatory per provider; Clay's host-side `createSessionCachePolicy()` wiring is an explicitly temporary stopgap to be removed once Prism enforces mandates itself. Clay formally requested this architectural change via a defect report filed in the Prism repo.

## Context

Live check of Clay's coding agent with the OpenCode Go provider (model `deepseek-v4-flash`) failed on the first prompt: the OpenCode Go gateway ("Console Go") hard-rejects every request lacking `x-opencode-session` with `400 MissingSessionID`. Prism 0.5.0's `opencode-go` adapter maps that header from `ProviderRequestOptions.sessionId ?? cacheKey`, but nothing in Prism populates those options unless the host registers `createSessionCachePolicy()` — a helper whose existence and relevance is discoverable only by reading Prism adapter source or hitting live 400s. Prism's contract sentence "Hosts decide which … request policies … become active" (`docs/provider-packages.md:104`) delegates exactly the knowledge hosts delegate to Prism to avoid.

The user directed the architectural position: mandatory structures are provider-side knowledge and must be handled deterministically by Prism; requiring hosts to infer and hand-wire per-provider policies is unnecessary hassle and structurally incomplete — Prism-internal call sites (observational-memory worker loop, compaction strategies) bypass host-visible agent config entirely, so even a diligent host cannot achieve coverage. Clay filed `2026-09-07-prism-provider-mandatory-wire-requirements-report.md` in the Prism repo requesting: deterministic session-correlation injection at the kernel/session choke point, declared mandatory request fields with fail-fast typed validation, package-declared policy auto-activation, and docs/matrix updates.

## Approval

- Proposed by: user ("where a particular structure is mandatory, this must be handled deterministically on Prism side as this is unnecessary hassle from Host side who delegates to Prism for such knowledge and methodologies … This should be handled on Prism side for all providers")
- Approved by user: Yes
- Approval evidence: "Create the decision log yes" (2026-09-07), approving the defect-report-plus-stopgap direction.

## Alternatives Considered

1. **Status quo (Prism 0.5.0 host-opt-in policies)** — rejected: hosts lack per-provider mandate knowledge; discovery is live-400-driven; coverage is structurally impossible for Prism-internal call sites (OM workers call `provider.generate` with no policy chain and no sessionId source).
2. **Clay-side permanent enforcement** (keep `createSessionCachePolicy` everywhere + plumb sessionId into OM worker providerOptions) — rejected: duplicates Prism's job, scales with Prism's provider count (18 first-party adapters), still cannot cover all Prism-internal call sites without reaching into Prism internals.
3. **Per-run options injection from the Clay daemon** (pass sessionId via run `providerOptions`) — rejected: same coverage holes as (2) with more host code; OM/compaction paths don't flow run options.

## Rationale and Evidence

- Live failure: 400 `{"type":"error","error":{"type":"MissingSessionID","message":"Error from provider (Console Go): Request is missing x-opencode-session and cannot be routed efficiently…"}}` on opencode-go + deepseek-v4-flash, Clay daemon 2026-09-07.
- Adapter maps the header only from request options: `prism-providers/src/opencode-go/cache.ts:9-29`, applied at `provider.ts:44`.
- Nothing populates the options by default: policy chain consumes host config + run options only and returns the request untouched when empty (`prism/dist/agent-session/session.js:372-376`); the opencode-go package `setup()` registers no policy (`index.ts:19-24`) despite Prism's own skeleton showing packages declaring one (`docs/provider-packages.md:300-317`).
- OM worker loop bypasses the policy chain entirely (`prism-memory/src/compaction/observational-memory/worker-loop.ts:52-63`); LLM compaction uses a strategy-supplied chain (`llm/strategy.ts:185-201`) — host coverage is structurally incomplete either way.
- The 2026-09-05 thinking-levels report established the cure pattern for this failure class: declare provider-owned contract facts in data, validate early, fail closed with typed errors. Same medicine applies; the report proposes the mechanism.
- Stopgap shipped and verified in Clay: `providerRequestPolicies: createSessionCachePolicy()` on both `createAgent` sites in `clay-agent/src/host.ts` (main agent + branch-summary worker); header flow verified end-to-end (`x-opencode-session: <sessionId>`); tsc clean, clay-agent suite 97 pass / 1 skip. Known stopgap limitation: OM worker models on opencode-go still lack the header until Prism-side enforcement lands.

## References

- `/home/arn/Projects/prism/2026-09-07-prism-provider-mandatory-wire-requirements-report.md` — the defect report / architectural change request filed with Prism (evidence index, requested mechanism, migration note).
- `prism docs/provider-packages.md:104,300-317` — the "Hosts decide" contract and the package-skeleton inconsistency.
- `prism docs/providers/opencode-go.md:51-52,72` — header documented as mapping, never as gateway mandate.
- `decision-logs/2026-09-06-0432-prism-0.5.0-clay-agent-family-pins.md` — the 0.5.0 pin decision this extends (hosts consume Prism 0.5.0 provider contracts).

## Consequences

- Positive: single authoritative enforcement point in Prism covering all current and future providers and internal call sites; hosts stop accumulating per-provider policy knowledge; fail-fast typed errors replace opaque upstream 400s.
- Interim: Clay carries the one-line `createSessionCachePolicy()` stopgap in `clay-agent/src/host.ts`; OM worker models set to opencode-go remain broken until Prism enforcement lands.
- Follow-up: when Prism ships deterministic enforcement, delete the host-side policy wiring and re-verify opencode-go end-to-end; watch Prism release notes/changelog for the architectural change responding to the 2026-09-07 report.
- Revisit if: Prism rejects the architectural change (then re-evaluate a deeper Clay-side plumbing design including OM workers), or Prism ships partial enforcement (then narrow the stopgap accordingly).
