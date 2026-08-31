---
date: 2026-08-30 21:59
status: approved
decision_about: "Web/browser stack (Obscura) and Linux desktop capability"
proposed_by: user
explicitly_approved_by_user: true
---

# Decision: Obscura-based web/browser stack for clay+st; computer-use-linux for st

## Decision

Web capability rides the Obscura engine: `@arnilo/prism-obscura` over a
host-installed binary, plus `@arnilo/prism-web-tools` and
`@arnilo/prism-browser` with optional `playwright-core`. The clay
coding agent (and `st` by inheritance) must fully use everything
Obscura is capable of: `web_search` and `web_fetch` through the
replaceable HTML search profile; native `obscura_fetch`/`obscura_scrape`
with public-HTTP(S)-only URL validation, byte/count/timeout caps,
`allowEval`-gated expressions, and untrusted-content labeling; browser
automation through CDP (`connectObscuraCdp` managed/external,
`prism-browser` surface: observe, policy-gated `browser_evaluate`,
block/throttle/emulate); and Playwright-driven e2e testing via the
CDP composition (`connectOverCDP`). The complete advertised Obscura MCP
surface (`obscura_*` tools) is exposed. All capabilities hidden when the
binary is absent. Additionally, `@arnilo/prism-computer-use-linux`
ships as an **`st` extension capability** (host-owned MCP binary,
accessibility tree, screenshots, input synthesis; deny-by-default
DeviceAdapter admission, opt-in setup tools, mutating calls require
approval per the acceptance policy).

## Context

The roadmap needed the web-tools story. The user chose Obscura over
Prism's alternative web options. Clay's daemon already has the
host-binary lifecycle pattern from `agy`; Obscura reuses it
(fail-closed spawn, readiness, group close).

## Approval

- Proposed by: user
- Approved by user: Yes
- Approval evidence: "I would adopt obscura based web search, CDP and
  Playwright for browser automation. Make sure clay agent is able to
  fully use obscura for doing everything that obscura is capable of
  doing. That means the clay and st coding agents should be able to do
  web search, web content fetch, do browser automation through CDP, do
  e2e testing with playwright. Also I want to add the computer-use-linux
  capability for the st agent extension." (2026-08-30). Approved for
  logging: "Go ahead and log the decisions as decided already"
  (2026-08-30).

## Alternatives Considered

1. **Direct `WebProvider` backends via `prism-web-tools` alone** —
   rejected as primary: requires per-provider API keys and lacks the
   browser engine; Obscura gives search + fetch + scrape + CDP in one
   host-owned engine. `prism-web-tools` remains the base the Obscura
   tools are composed from.
2. **Browserless-only (fetch/scrape without CDP)** — rejected: no
   automation or e2e capability.
3. **computer-use-linux for clay base agent** — rejected: desktop
   control is autonomy-tier capability; it stays `st`-only, consistent
   with "no autonomy in the base agent" (Phase 2 boundary).
4. **Always-on web tools** — rejected: hidden when binary absent keeps
   the base agent clean (mirrors `agy`/graft handling).

## Rationale and Evidence

- `@arnilo/prism-obscura` 0.3.0 exists (Prism 0.3.1 changelog): optional
  Obscura headless-browser engine over a host-installed binary —
  fail-closed `spawnObscuraProcess` lifecycle, `createObscuraMcpTools`
  (complete advertised MCP surface, `obscura_` prefix), `connectObscuraCdp`
  + Playwright `connectOverCDP` composition, `createObscuraWebTools`
  with the security constraints listed above. Peers: prism, web-tools,
  mcp, browser, optional playwright-core.
- `@arnilo/prism-computer-use-linux` exists (Prism 0.3.0 changelog):
  wraps a host-owned computer-use-linux MCP binary with the security
  posture adopted here.
- Phase 2 exit gate already includes the web/browser fixture checks
  (search answers, CDP drives a fixture page, Playwright e2e passes,
  no residue when disabled).

## References

- `roadmap.md` — Readiness table rows (web search/fetch, browser
  automation+e2e, Linux desktop use), Phase 0 (pins), Phase 1 (engine
  plumbing), Phase 2 (agent surface + exit gate), Phase 5 (desktop
  capability), resolved item 7.
- Prism 0.3.0/0.3.1 changelogs — package descriptions and constraints.
- Decision log 2026-08-30-2157 — acceptance policy the desktop mutating
  calls route through.

## Consequences

- Positive: one engine covers search, fetch, scrape, automation, e2e;
  consistent untrusted-content handling; `st` can verify GUI work on
  real desktops.
- Risks: Obscura binary availability per host — fail-closed hiding;
  Playwright peer version pinning (`playwright-core@1.61.0` at time of
  research).
- Revisit if: Obscura development stops (fallback: `prism-web-tools`
  direct providers + `prism-browser` standalone).