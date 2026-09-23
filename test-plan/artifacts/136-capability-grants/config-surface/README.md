# Plan 136 task 10 — capability grant as a configuration surface

Evidence for the `init.js` grant surface, its documentation, and the
self-grant refusal.

## Surfaces

| Surface | Mechanism |
| --- | --- |
| Grant from configuration | [`packages.authorize`](../../../../docs/reference/clay-js-api/packages/authorize.md), attributed `approvedBy: "config"` |
| Grant from the CLI | `clay package authorize <name> --capability <cap>...` (attributed `cli`, requires a current adoption so the grant is durable) |
| Adoption / revocation | `clay package adopt`, `clay package revoke` — host CLI only, never package JavaScript |
| Consumption | [`packages.loadPackage`](../../../../docs/reference/clay-js-api/packages/load-package.md) — consumes a recorded grant, never creates one |
| Enforcement | `PackageService::capability_granted` via `ensure_capability_grants` + package-op dispatch → `MissingCapabilityGrant` (compiled, not tunable) |

Documented order: install → adopt → `authorize` → `loadPackage`. Granting before
adopting still fails closed at enable with the adoption diagnostic.

## Configuration guide

New section `## Plan 136 third-party capability grant configuration review` in
`docs/reference/clay-js-api/configuration.md`: the surfaces table, the reviewer
`init.js` example, the package-author manifest side, the rejected hidden-key list
(`capabilityGrant`, `packages.authorizedCapabilities`, …), the trusted-only and
attribution statements, and the configuration-time-only hot-path statement. The
plan 060/061 closure table gained the grant row and its "JavaScript cannot approve
itself" paragraph now covers capability grants.

Pinned by `tests/clay_js_api_inventory.rs::plan136_configuration_documents_the_capability_grant_surface`
(guide markers + the page's authority/hot-path/separately-implemented-API
statements + non-empty `custom_properties`). The task 8 guard already walks
`packages.authorize`'s declared options with every other public API (236 keys /
59 APIs), so this test does not duplicate the option-surface check.

## Ordering and self-grant coverage

| Case | Test |
| --- | --- |
| `authorize` before `loadPackage` succeeds from a real config root; enable before the grant fails closed with `MissingCapabilityGrant`; repeating the call is idempotent | `src/server/js_runtime/tests/package_adoption.rs::config_authorize_grants_declared_capability_and_enables_package` (plan 136 task 3) |
| Package code cannot grant itself a capability: the op refuses while a package activation is open | `src/server/js_runtime/tests/package_adoption.rs::package_code_cannot_self_grant_capabilities_during_activation` (added here) |
| Third-party package code cannot even reach the op: `clay:packages` is `Facade::trusted`, absent from the shared third-party runtime | `src/server/facades.rs` table + `tests/rust_visibility_api_mapping.rs` facade allowlist guards |
| Undeclared capability, provenance mismatch, unknown package | plan 136 task 3 tests in `package_adoption.rs` |

Mutation check (`mutation-self-grant-gate-removed.txt`): deleting the
`ensure_grant_authority_open(clay_state)?` call from
`src/server/ops/packages.rs::op_clay_packages_authorize` makes the new test fail
with `package activation must not grant capabilities: self-granted:true` — the
gate is load-bearing, not decoration. Reverted immediately.

## Gate

`gate-check-full-task10.log` — `scripts/check.sh full` PASSED (exit 0, nine
stages; protocol 229 including the new configuration-guide test, lib 1436
including the self-grant refusal test, security 162).
