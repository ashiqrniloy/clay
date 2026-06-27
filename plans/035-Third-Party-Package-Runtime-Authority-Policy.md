# Third-Party Package Runtime Authority Policy

## Objectives

- Define the trust, permission, registry/integrity, denied-authority, rollback, and test policy required before any non-`@clay/*` package JavaScript executes.
- Keep third-party runtime execution blocked while policy and evidence are developed.
- Produce an approval-ready authority proposal and decision log only after the policy, sandbox evidence, and regression gates exist.

## Expected Outcome

- Clay has a documented third-party package execution policy covering install, enable, load, runtime execution, package-manager execution, client behavior delivery, rollback, and diagnostics.
- Tests fail if non-`@clay/*` execution is enabled before the policy and approved authority decision are in place.
- Any later implementation can widen `loadPackage` from first-party-only to third-party execution without guessing trust or permission rules.

## Tasks

- [x] Review existing package/runtime primitives and third-party authority gaps
  - Acceptance Criteria:
    - Functional: Inventory current install, enable, load, runtime execution, package-manager, sandbox, and client behavior boundaries for first-party packages and identify exact gaps for third-party execution.
    - Performance: Confirm all third-party policy and validation work remains startup/install/enable/load/reload/background work, never keypress, paint, layout, scroll, text-event, or edit-ack work.
    - Code Quality: Reuse existing package primitives and generic runtime/sandbox primitives before proposing new code; no package-specific Rust branches.
    - Security: Preserve deny-by-default for non-`@clay/*` execution during the review.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/create-plan/references/clay.md`
      - `.agents/skills/project-patterns/references/package-distribution.md`
      - `.agents/skills/project-patterns/references/authority-boundaries.md`
      - `.agents/skills/project-patterns/references/extensions-and-ai.md`
      - `docs/reference/primitives/package-security.md`
      - `docs/reference/primitives/package-loading.md`
      - `docs/design/persistent-runtime-sandbox.md`
      - `docs/wiki/modules/persistent-runtime-hardening.md`
      - `plans/034-Persistent-Runtime-Hardening-Before-Third-Party-Package-Authority.md`
    - Options Considered:
      - Treat the sandbox harness as sufficient. Rejected; trust, registry integrity, permissions, and rollback remain undefined.
      - Define policy first, then decide whether any runtime execution authority is granted. Chosen.
    - Chosen Approach:
      - Start with an inventory/gap document and docs-as-code guard so later tasks fill only real missing primitives.
    - API Notes and Examples:
      ```text
      install != enable != load != runtime execution != package-manager execution != client behavior delivery
      ```
    - Files to Create/Edit:
      - `docs/wiki/modules/third-party-runtime-authority.md`: Created package/runtime primitive inventory and third-party trust, registry/integrity, permission, sandbox, rollback, executable-gate, hot-path, and deny-by-default gaps page.
      - `docs/wiki/index.md`: Linked the new third-party runtime authority wiki page.
      - `tests/package_loading_docs.rs`: Added docs-as-code guard for the policy page and index link.
    - References:
      - Package distribution, authority boundaries, extension/AI patterns.
  - Test Cases to Write:
    - `cargo test --test package_loading_docs third_party_runtime_authority_policy_is_documented`: Passed; requires inventory, scope split, hot-path exclusion, third-party gap categories, and deny-by-default gate language.

- [x] Define package trust and identity policy
  - Acceptance Criteria:
    - Functional: Define accepted package identity fields, namespace ownership, publisher/source provenance, allowed source kinds, version constraints, compatibility fields, and conflict behavior.
    - Performance: Identity checks happen at install/enable/load time and use cached metadata where possible.
    - Code Quality: Policy maps onto existing `PackageRecord`, `PackageService`, and conflict primitives or names generic gaps.
    - Security: Unknown publishers, ambiguous package identity, namespace hijacks, typosquats, local path ambiguity, and unsigned/untrusted sources fail closed.
  - Approach:
    - Documentation Reviewed:
      - `docs/reference/primitives/package-security.md`
      - `src/packages/manifest.rs`
      - `src/packages/record.rs`
      - `src/packages/service.rs`
      - `src/packages/conflict.rs`
    - Options Considered:
      - Trust any npm package with Clay metadata. Rejected; metadata alone cannot prove source trust or namespace ownership.
      - Require explicit trust records per third-party package/source. Chosen initial policy target.
      - Trust all scoped packages from one registry namespace. Possible later shortcut, but too broad for first authority expansion.
    - Chosen Approach:
      - Add a declarative trust policy document and planned typed trust record before resolver changes.
    - API Notes and Examples:
      ```toml
      [[trusted_package]]
      name = "@vendor/example"
      version = "1.2.3"
      registry = "https://registry.npmjs.org/"
      integrity = "sha512-..."
      clay_prefix = "example"
      clay_api_compatibility = "^0.1"
      ```
    - Files to Create/Edit:
      - `docs/reference/primitives/package-security.md`: Added third-party trust/identity policy, exact trust-record fields, compatibility field, accepted source-kind policy, fail-closed identity checks, hot-path exclusion, and current generic primitive gaps.
      - `docs/wiki/modules/third-party-runtime-authority.md`: Added trust model details and mapped policy onto existing `PackageRecord`, `PackageService`, and conflict primitives.
      - `tests/package_loading_docs.rs`: Added trust policy docs-as-code coverage.
    - References:
      - Existing package manifest and conflict validators.
  - Test Cases to Write:
    - `cargo test --test package_loading_docs third_party_trust_identity_policy_is_documented`: Passed; requires package name/version/source/provenance/prefix/compatibility/conflict rules, trust-record identity fields, current primitive gaps, hot-path exclusion, and fail-closed language.

- [x] Define registry and integrity verification policy
  - Acceptance Criteria:
    - Functional: Define how Clay records registry source, resolved version, lockfile/integrity digest, package tarball/source path, package-manager output boundary, update policy, and offline/cache behavior.
    - Performance: Integrity verification is install/update/load-time only and does not affect editor hot paths.
    - Code Quality: Reuse npm-compatible package-manager/lockfile data when possible instead of writing a registry client.
    - Security: Lifecycle scripts remain disabled by default; registry/package-manager output is not trusted as runtime authority and diagnostics are sanitized.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/project-patterns/references/package-distribution.md`
      - `src/packages/manager.rs`
      - `src/packages/service.rs`
      - `docs/reference/primitives/package-loading.md`
    - Options Considered:
      - Build a Clay registry/integrity resolver. Rejected; project direction delegates package management to npm-compatible tooling.
      - Consume package-manager lockfile/integrity metadata and record Clay trust decisions separately. Chosen.
    - Chosen Approach:
      - Specify package-manager delegation plus Clay-owned integrity/trust recording and validation gates.
    - API Notes and Examples:
      ```bash
      pnpm add --ignore-scripts <pkg>@<version>
      # Clay records resolved version + integrity; install still does not execute runtime JS.
      ```
    - Files to Create/Edit:
      - `docs/reference/primitives/package-loading.md`: Added registry/integrity policy details, source provenance record shape, lifecycle-script default, sanitization requirements, offline/cache policy, update-as-new-identity rule, hot-path exclusion, and current generic primitive gaps.
      - `docs/wiki/modules/package-loading.md`: Added implementation notes/gaps for provenance storage, lockfile integrity parsing, diagnostic sanitization, offline/cache keys, and update enforcement.
      - `tests/package_loading_docs.rs`: Added registry/integrity docs/source guard.
    - References:
      - `PackageManagerBackend`, `PnpmBackend`, lifecycle-script suppression docs/tests.
  - Test Cases to Write:
    - `cargo test --test package_loading_docs third_party_registry_integrity_policy_is_documented`: Passed; requires `--ignore-scripts`, integrity/lockfile/source recording, sanitized package-manager diagnostics, offline/cache and update policy, install/runtime separation, and package-manager boundary primitives.

- [x] Define third-party permission model and denied authorities
  - Acceptance Criteria:
    - Functional: Define permission categories available to third-party packages, permission request syntax, grant source, runtime enforcement point, diagnostics, and denied authorities.
    - Performance: Permission checks happen at load/registration/request boundaries, not every editor hot-path event.
    - Code Quality: Extend existing package permission primitives where possible; do not expose raw ops as permissions.
    - Security: Filesystem, network, shell, WASM, AI mutation, package-manager execution, native-widget, client-JS, raw-op, remote listener, and workspace mutation authorities remain denied unless a later narrow capability is approved and tested.
  - Approach:
    - Documentation Reviewed:
      - `docs/reference/primitives/package-security.md`
      - `src/packages/permissions.rs`
      - `docs/design/persistent-runtime-sandbox.md`
      - `docs/wiki/modules/persistent-runtime-hardening.md`
    - Options Considered:
      - Single `trusted-third-party` permission. Rejected; too broad to audit or enforce.
      - Fine-grained declared permissions with deny-by-default host capabilities. Chosen.
    - Chosen Approach:
      - Define a minimal permission matrix and tests before changing runtime resolver behavior.
    - API Notes and Examples:
      ```json
      {
        "clay": {
          "permissions": ["mode-registration", "parse-document"]
        }
      }
      ```
    - Files to Create/Edit:
      - `docs/reference/primitives/package-security.md`: Added third-party permission model, allowed permission matrix, request syntax, explicit grant-source rule, parent enforcement points, diagnostics, broad/catch-all rejection, denied authorities, hot-path exclusion, and current generic primitive gaps.
      - `docs/wiki/modules/third-party-runtime-authority.md`: Added denied-authority and permission enforcement policy mapped to existing `parse_permission`/manifest validation primitives.
      - `tests/package_loading_docs.rs`: Added permission/denied-authority docs and source guard.
    - References:
      - Existing `parse_permission` and prohibited-authority handling.
  - Test Cases to Write:
    - `cargo test --test package_loading_docs third_party_permission_model_and_denied_authorities_are_documented`: Passed; requires explicit allowed permission matrix, grant source, parent enforcement boundaries, diagnostics, denied-authority list, hot-path exclusion, source-level prohibited authorities, and rejection of broad/catch-all permissions.

- [x] Define sandbox enforcement and parent-validation policy for third-party execution
  - Acceptance Criteria:
    - Functional: Specify production sandbox protocol requirements for third-party package load/evaluate/parse requests, parent validation, payload limits, timeout/heap behavior, restart semantics, and stale generation rejection.
    - Performance: Measure and document sandbox startup/evaluation/parse overhead targets before any production routing.
    - Code Quality: Treat current `runtime_sandbox` harness as evidence, not production API; no public Clay JS API exposes sandbox internals.
    - Security: Child receives no workspace roots, file descriptors, package-manager handles, raw op names, V8 handles, capability tokens, or client authority.
  - Approach:
    - Documentation Reviewed:
      - `docs/design/persistent-runtime-sandbox.md`
      - `src/server/runtime_sandbox.rs`
      - `src/bin/clay-runtime-sandbox.rs`
      - `tests/runtime_sandbox_harness.rs`
      - `src/protocol/codec.rs`
    - Options Considered:
      - Use newline-delimited JSON harness in production. Rejected unless separately justified; current harness is test evidence.
      - Promote a bounded typed protocol with parent-side validators. Chosen.
    - Chosen Approach:
      - Document the production enforcement contract and the missing implementation tasks separately from authority approval.
    - API Notes and Examples:
      ```text
      parent validates package metadata + permissions -> child evaluates -> parent validates inert outputs -> publish
      ```
    - Files to Create/Edit:
      - `docs/design/persistent-runtime-sandbox.md`: Added third-party production enforcement contract, bounded typed protocol requirements, request variants, parent pre/post-validation, timeout/heap/restart/stale-generation policy, child authority exclusions, and measurement targets.
      - `docs/wiki/modules/third-party-runtime-authority.md`: Added sandbox policy summary and mapped current harness evidence versus missing production API.
      - `tests/package_loading_docs.rs`: Added sandbox enforcement docs/source guard.
    - References:
      - Plan 034 sandbox design and harness tests.
  - Test Cases to Write:
    - `cargo test --test package_loading_docs third_party_sandbox_enforcement_policy_is_documented`: Passed; requires child authority exclusions, parent pre/post-validation, timeout kill/restart, payload budget, stale generation rejection, measurement targets, no hot-path dependency, and current harness/codec evidence.

- [x] Define rollback, disable, update, and incident response policy
  - Acceptance Criteria:
    - Functional: Define how Clay disables a trusted third-party package, rolls back updates, handles failed load/evaluation, withdraws contributions, preserves prior validated client state, and reports diagnostics.
    - Performance: Rollback/disable work does not block typing/rendering; stale parse/UI updates are rejected by generation/version checks.
    - Code Quality: Reuse `PackageService`, runtime generation, parse coordinator, and conflict primitives where possible.
    - Security: A malicious or broken package cannot leave half-registered handlers, behavior manifests, SDUI state, commands, or package-manager side effects active after rollback.
  - Approach:
    - Documentation Reviewed:
      - `docs/wiki/modules/persistent-runtime-hot-reload.md`
      - `docs/wiki/modules/parse-task-lifecycle.md`
      - `docs/wiki/modules/package-loading.md`
      - `src/server/parse_coordinator.rs`
      - `src/server/js_runtime.rs`
    - Options Considered:
      - Disable only future loads. Rejected; active contributions and parse handlers must also be invalidated.
      - Runtime-generation rollback with parent-owned last-valid state. Chosen.
    - Chosen Approach:
      - Define rollback semantics first; implementation can later wire service state and runtime generation invalidation.
    - API Notes and Examples:
      ```text
      failed third-party generation -> keep prior validated manifest/UI -> cancel generation parse -> require explicit reload/update
      ```
    - Files to Create/Edit:
      - `docs/wiki/modules/third-party-runtime-authority.md`: Added rollback/incident model covering active withdrawal, update-as-new-identity, generation rollback, stale output rejection, fail-closed incident response, package-manager side-effect boundary, reusable primitives, missing generic pieces, and hot-path exclusion.
      - `docs/reference/primitives/package-loading.md`: Added disable/update/rollback policy notes covering PackageService state withdrawal, contribution cleanup, prior validated generation preservation, sandbox kill/restart, diagnostics, and current implementation gaps.
      - `tests/package_loading_docs.rs`: Added rollback docs/source guard.
    - References:
      - Phase 19 hot reload generation model.
  - Test Cases to Write:
    - `cargo test --test package_loading_docs third_party_rollback_disable_update_incident_policy_is_documented`: Passed; requires disable/update/rollback semantics, stale generation rejection language, fail-closed incident response, package-manager side-effect boundary, no hot-path dependency, and current hot-reload/parse coordinator primitive evidence.

- [ ] Add executable gates that keep third-party execution disabled until approval
  - Acceptance Criteria:
    - Functional: Tests fail if resolver/source docs allow non-`@clay/*` execution without an approved authority decision-log reference and policy gate.
    - Performance: Gates are static/docs/source tests, not runtime hot-path checks.
    - Code Quality: Tests are deterministic and point to the centralized resolver/policy files.
    - Security: Regression coverage includes bare, scoped, URL, local path, traversal, and registry-style specifiers plus package-manager metadata not implying execution.
  - Approach:
    - Documentation Reviewed:
      - `tests/package_loading_docs.rs`
      - `src/server/js_runtime.rs` resolver tests.
      - `tests/package_loading.rs` package-manager metadata tests.
    - Options Considered:
      - Trust review discipline. Rejected; authority boundaries need failing tests.
      - Docs/source/inventory gates. Chosen.
    - Chosen Approach:
      - Add focused tests that keep non-`@clay/*` execution blocked until the later authority decision explicitly changes the expected text and source gate.
    - API Notes and Examples:
      ```bash
      cargo test op_clay_packages_load_package_by_specifier_rejects_non_first_party_specifier --lib
      cargo test --test package_loading_docs third_party_execution_requires_approved_authority_decision
      ```
    - Files to Create/Edit:
      - `tests/package_loading_docs.rs`: Add decision-gate and policy coverage tests.
      - `src/server/js_runtime.rs`: Add resolver regression cases only if gaps are found.
      - `tests/package_loading.rs`: Add install/metadata regression cases only if gaps are found.
    - References:
      - Plan 034 third-party rejection tests.
  - Test Cases to Write:
    - `third_party_execution_requires_approved_authority_decision`: fails without explicit decision-log reference and deny-by-default source gate.
    - Existing resolver and install/metadata tests continue passing.

- [ ] Prepare the approval-ready third-party runtime authority decision log
  - Acceptance Criteria:
    - Functional: Draft an approval-ready decision log proposal with exact authority, trust policy, registry/integrity policy, permission model, denied authorities, sandbox enforcement, rollback/revisit conditions, and tests.
    - Performance: Decision evidence includes measured sandbox/heap overhead and confirms no editor hot-path dependency.
    - Code Quality: Decision distinguishes install, enable, load, runtime execution, package-manager execution, and client behavior delivery.
    - Security: Do not mark the decision approved or enable execution until the user explicitly approves the final decision-log content.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/create-decision-log/SKILL.md`
      - Prior authority logs in `decision-logs/`
      - Policy docs produced by this plan.
    - Options Considered:
      - Write an approved log now. Rejected by user; policy work must happen first.
      - Draft approval-ready content and ask for explicit approval after evidence exists. Chosen.
    - Chosen Approach:
      - Create either a clearly unapproved draft outside `decision-logs/` or present final text in chat; write to `decision-logs/` only after explicit approval.
    - API Notes and Examples:
      ```text
      status: proposed # not in decision-logs/ until approved
      ```
    - Files to Create/Edit:
      - `docs/wiki/modules/third-party-runtime-authority.md`: Link policy evidence and approval status.
      - `decision-logs/YYYY-MM-DD-HHMM-third-party-package-runtime-authority.md`: Create only after explicit approval.
      - `.agents/skills/project-patterns/references/package-distribution.md`: Update only after approved decision if durable guidance changes.
    - References:
      - `create-decision-log` workflow and prior package authority logs.
  - Test Cases to Write:
    - Manual approval gate: no approved log is written and no resolver widening occurs without explicit user approval.

- [ ] Create or verify Clay configuration APIs
  - Acceptance Criteria:
    - Functional: Trust policy, registry/integrity policy, sandbox policy, and third-party execution gates are not hidden `init.js` keys unless intentionally exposed as documented Clay JS APIs.
    - Performance: Configuration review adds no runtime hot-path work.
    - Code Quality: No ad hoc JSON/TOML key can enable third-party execution, disable sandboxing, raise heap/time budgets, or bypass trust/integrity validation.
    - Security: User configuration cannot grant filesystem, network, shell, package-manager, raw-op, AI, WASM, native-widget, or client-JS authority.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/create-plan/references/clay.md` Clay Configuration Task.
      - `docs/reference/clay-js-api/configuration.md`
      - `docs/reference/clay-js-api/api-inventory.toml`
      - `tests/clay_js_api_inventory.rs`
    - Options Considered:
      - Add `enableThirdPartyPackages` config. Rejected; authority must require trust/integrity policy and approval.
      - Keep policy as internal server-owned gates until a user-facing trust UI/API is deliberately designed. Chosen.
    - Chosen Approach:
      - Add/update docs and inventory tests proving no hidden configuration switch grants authority.
    - API Notes and Examples:
      ```text
      clay.configuration.enableThirdPartyPackages // forbidden unless future approved API task creates it
      ```
    - Files to Create/Edit:
      - `docs/reference/clay-js-api/configuration.md`
      - `tests/clay_js_api_inventory.rs`
    - References:
      - Clay configuration plan requirement.
  - Test Cases to Write:
    - Inventory test rejects hidden third-party/sandbox/trust/integrity bypass configuration APIs.

- [ ] Create or verify Clay JS APIs for public programmatic surfaces
  - Acceptance Criteria:
    - Functional: Any public trust, package inspection, diagnostics, developer command, or approval workflow introduced by the plan has documented Clay JS API docs and inventory entries; internal policy helpers remain private or `pub(crate)`.
    - Performance: API docs state policy work is install/enable/load/reload/background work, not typing/rendering hot path.
    - Code Quality: Raw `Deno.core.ops` names, sandbox protocol messages, package-manager internals, and trust-file internals are not public APIs.
    - Security: API docs preserve denied-authority language, sanitized diagnostics, and approval-gate requirements.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/create-plan/references/clay.md` Clay JS API Task.
      - `docs/reference/clay-js-api/api-inventory.toml`
      - `docs/generated/clay-js-api-registry.json`
      - `tests/clay_js_api_inventory.rs`
      - `tests/clay_js_doc_registry.rs`
    - Options Considered:
      - Public API for policy bypasses. Rejected.
      - Internal-only policy docs plus future explicit trust UI/API task if needed. Chosen unless implementation creates real public behavior.
    - Chosen Approach:
      - Inventory changes at the end and document only intentional public surfaces.
    - API Notes and Examples:
      ```bash
      cargo run --bin update-doc-registry
      cargo test --test clay_js_api_inventory
      cargo test --test clay_js_doc_registry
      ```
    - Files to Create/Edit:
      - `docs/reference/clay-js-api/**` as needed.
      - `docs/reference/clay-js-api/api-inventory.toml` as needed.
      - `docs/generated/clay-js-api-registry.json` as needed.
      - `tests/clay_js_api_inventory.rs`
      - `tests/clay_js_doc_registry.rs`
    - References:
      - Clay JS API naming/discovery decision logs.
  - Test Cases to Write:
    - Registry and inventory tests pass, or internal-only tests prove no public API was added.

- [ ] Verify policy with focused and repository gates
  - Acceptance Criteria:
    - Functional: Trust, integrity, permission, sandbox, rollback, deny-by-default, configuration, and API documentation gates pass.
    - Performance: Sandbox/heap overhead evidence is recorded; no editor hot-path dependency is introduced.
    - Code Quality: Formatting, focused tests, and relevant all-target tests pass without broad lint/test skips.
    - Security: Non-`@clay/*` execution remains blocked unless a later explicit approved decision changes that expectation.
  - Approach:
    - Documentation Reviewed:
      - `docs/wiki/modules/maintenance-validation.md`
      - `docs/development/performance.md`
    - Options Considered:
      - Docs-only review. Rejected; policy gates need executable coverage.
      - Focused gates plus final repository gate. Chosen.
    - Chosen Approach:
      - Run focused authority tests first, then broader package/runtime/API gates.
    - API Notes and Examples:
      ```bash
      cargo fmt --check
      cargo test --test package_loading_docs
      cargo test --test package_loading
      cargo test js_runtime --lib
      cargo test --test runtime_sandbox_harness
      cargo test --test clay_js_api_inventory
      ```
    - Files to Create/Edit:
      - `plans/035-Third-Party-Package-Runtime-Authority-Policy.md`: Record final verification commands/results.
    - References:
      - Maintenance validation wiki.
  - Test Cases to Write:
    - All focused gates from prior tasks pass together.

- [ ] Update or verify the code wiki after implementation
  - Acceptance Criteria:
    - Functional: The project code wiki is updated after all implementation tasks are complete, or explicitly verified as unchanged for non-code work.
    - Performance: Wiki updates add no runtime work and document performance-relevant implementation details changed by the plan.
    - Code Quality: Wiki pages explain what changed code does, how it works, invariants/tradeoffs, source/test paths, examples where useful, and links from the master wiki index.
    - Security: Wiki pages document touched security boundaries, permissions, validation, secrets handling, or external authority without exposing secrets.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/project-wiki/SKILL.md`: Use the project wiki workflow and quality bar.
    - Options Considered:
      - Update after each task: more granular, but noisy and likely to churn.
      - Update once after tests pass: keeps docs aligned with final code.
    - Chosen Approach:
      - After implementation and verification pass, update the Markdown code wiki once using `project-wiki`, including the master index and relevant pages.
    - API Notes and Examples:
      ```text
      docs/wiki/index.md
      docs/wiki/modules/<module>.md
      ```
    - Files to Create/Edit:
      - `docs/wiki/index.md`: Add or update navigation links for changed implementation areas.
      - `docs/wiki/**`: Add or update implementation wiki pages for changed code.
    - References:
      - `.agents/skills/project-wiki/SKILL.md`
  - Test Cases to Write:
    - Manual wiki review: Confirm the master index links relevant pages and updated pages explain what changed implementation does and how it works.

## Compromises Made

- To be filled after tasks are completed and tests pass.

## Further Actions

- To be filled after task completion with improvements, rationale, and priority.
