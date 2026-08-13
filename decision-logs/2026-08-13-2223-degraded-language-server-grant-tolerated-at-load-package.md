---
date: 2026-08-13 22:23
status: approved
decision_about: "loadPackage tolerates a missing language-server capability grant"
proposed_by: "agent"
explicitly_approved_by_user: true
---

# Decision: `loadPackage` tolerates a missing `language-server` capability grant

## Decision

`loadPackage` tolerates a package that declares the `language-server`
capability but has no current language-server grant: the package loads and
the capability stays inert until an authorized language-server session
starts. All other capability grants keep their hard requirement at load
time, and session start remains strictly grant-gated.

## Context

The canonical examples tree (`examples/init.js` + `examples/packages/first-party.js`)
mandates "grants first, loads second" and each `grantLanguageServer` call
degrades independently ("Tooling not installed (or root absent) — skip this
server only"). `authorizeLanguageServer` requires a matching installed
package, a `language_servers` contribution, a resolved executable, and at
least one workspace root. In an environment with zero workspace roots at
configuration-eval time (fresh temp config root, no tabs bound yet), every
grant degrades, so `@clay/lsp-typescript` — whose manifest declares the
`language-server` capability — failed `ensure_capability_grants` with
`MissingCapabilityGrant` ("requested capability `language-server` without a
user authorization grant"), failing the whole optional module with
`configuration.module_failed`.

The entry-gate test `example_configuration_loads_cleanly_and_applies_effects`
(`src/server/mod.rs:3019`, "example optional modules must load") therefore
failed deterministically on any machine, contradicting the examples
contract: a degraded grant should skip that server only, not kill the
module.

## Approval

- Proposed by: agent (root-caused during the Phase 24.5 entry gate, plan 085 task 1; approach recorded in plan 085 task 8).
- Approved by user: Yes
- Approval evidence: "Approved" (2026-08-13) in response to the exact
  decision statement in plan 085 task 8's Execution Record: "`loadPackage`
  tolerates a missing `language-server` capability grant (capability stays
  inert until an authorized session starts); implement with two regression
  tests — examples tree loads cleanly with zero workspace roots, and a
  language-server session still cannot start without a matching grant — plus
  a decision-log entry."

## Alternatives Considered

1. **Keep the hard failure; change the test to bind a workspace root before
   reload** — rejected: the examples contract explicitly says grants degrade
   independently and the tree still loads cleanly; the test was written to
   prove that contract.
2. **Patch the reload path to skip the real config root in tests** (for the
   related hang, not this failure) — rejected as a code hack; hermetic test
   setup was chosen instead.
3. **Tolerate ALL missing capability grants at load time** — rejected: only
   the language-server capability is inert-until-session-start by design;
   other capabilities (e.g. `parse-document`) are exercised at load/use
   without a further authorization gate.

## Rationale and Evidence

- The enforcement that failed is `ensure_capability_grants`
  (`src/packages/service.rs:1445`): (a) no authorization record → error
  naming the first declared permission; (b) any declared permission without
  a grant → error; (c) a `language-server` permission additionally requires
  a "current" contribution/root grant (`MissingLanguageServerGrant`).
- The language-server capability is the only capability whose runtime use
  re-checks authorization: `authorize_language_server` / session start
  (`src/packages/service.rs:871-919`) is strictly grant-gated and requires
  the executable and workspace roots again. A package loaded without a
  grant can start no session, so the load-time requirement is redundant
  with the session-time gate and contradicts the examples degrade contract.
- `examples/packages/first-party.js` grants `@clay/lsp-rust`,
  `@clay/lsp-typescript`, `@clay/lsp-javascript` with `workspaceRootIds: [1]`
  and catches each grant error with "skip this server only".
- Pre-existing defect at HEAD b6e2c86 (squashed history; `unknown_workspace_root`
  validation present since release commit 325692f); no Phase 24 regression window.

## References

- `src/packages/service.rs` — `ensure_capability_grants` (1445),
  `authorize_language_server` (864-919), session-grant checks.
- `src/server/mod.rs` — `example_configuration_loads_cleanly_and_applies_effects`
  (3000-3019), `temp_example_config_root` (2697).
- `examples/packages/first-party.js` — grants-first/loads-second contract.
- Plan 085 tasks 1 and 8 — entry-gate record and chosen approach.
- Git: HEAD b6e2c86; release 325692f (initial `unknown_workspace_root`).

## Implementation note (2026-08-13, after approval)

Three enforcement points needed the same tolerance to meet regression test
"examples tree loads cleanly with zero workspace roots":

1. `ensure_capability_grants` (`src/packages/service.rs`) — the load-time
   capability check now filters out the `LanguageServer` permission; all
   other capabilities keep their hard requirement.
2. `op_clay_language_register_document_analyzer`
   (`src/server/ops/document_analysis.rs`) — analyzer registration no longer
   requires the approved `LanguageServer` capability or a current exact
   grant (package must still be enabled and the contribution must still name
   a fixed package language server). The registration is inert: every
   invocation is denied by `document_analysis_authorized` (per-document
   current-grant + workspace-root check) and session start is still
   grant-gated.
3. Reload validation (`src/server/mod.rs`) — an analyzer registered without
   a current grant is skipped for the generation (stays inactive;
   re-registers once the grant lands on a later reload) instead of failing
   the whole generation.

Tests updated: `tests/package_loading.rs` —
`language_server_enable_tolerates_missing_grant_while_sessions_stay_grant_gated`
(replaces `language_server_enable_requires_current_exact_grant_and_revocation_fails_closed`):
enable succeeds without/stale/revoked grants while `language_server_grant`
stays empty (session gate); `bundled_defaults_never_auto_grant_language_server`
and `replacement_language_server_requires_own_fresh_grant` assert the
load-tolerated / no-inherited-grant split.

## Consequences

- Positive: examples tree loads cleanly with zero workspace roots;
  `cargo test --all-targets` gate passes on machines with a real user
  config; package authors' "degrade per-server" grants behave as documented.
- Risks: a package whose language-server grant was revoked or became stale
  now loads; its capability remains inert because session start is still
  grant-gated, so no authority leaks. The stale-grant case is visible via
  the session-start error at runtime.
- Conditions to revisit: if a future language-server session start path
  stops re-checking grants (authorization drift), restore the load-time
  requirement.
