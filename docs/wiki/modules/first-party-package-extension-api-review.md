# First-Party Package Extension API Review

## Status

Architecture review for Plan 061, rebaselined against current source on 2026-07-21, with generic trust-domain and extension schemas locked the same day. It records current shipped packages and the minimum public extension surfaces third-party packages need under the approved two-runtime boundary. The closed versioned schemas (`clay-extension-point-v1`, `clay-package-relation-v1`, `clay-package-approval-v1`, `clay-package-replacement-v1`, `clay-cross-domain-envelope-v1`) are canonical in `docs/reference/primitives/package-security.md#package-runtime-trust-domains-and-extension-authority`; exact JS facade names remain implementation tasks. Security and ownership rules are approved by `decision-logs/2026-07-21-0001-two-package-runtime-trust-domains.md`.

Locked generic-primitive decisions:

- Every first-party need maps to the generic registries and the six Plan 061 registry rows (`PackageTrustDomainClassification`, `ExtensionPointDeclaration`, `PackageRelationRequest`, `PackageAdoptionRecord`, `PackageReplacementRecord`, `CrossDomainRequestEnvelope`); no per-package Rust APIs, per-language branches, generic actor/plugin framework, JavaScript-visible bearer identities, or cross-runtime V8 object/function sharing.
- The only package-specific public data surface justified today remains the read-only `clay:git` status/refresh API. Task 8 decided `packages/lsp-shared` stays private behind the typed public `clay:language-server` APIs; no public pure-JS SDK extraction without a demonstrated third-party consumer.
- Schemas are bounded inert metadata evaluated at install/adopt/load/reload time; no package graph or cross-domain JavaScript work enters typing, paint, layout, scroll, pointer, or local edit paths.

## Source

- `packages/*/package.json`
- `packages/*/dist/*.js`
- `runtime/js/*.ts`
- `src/packages/{manifest,record,graph,service,conflict}.rs`
- `src/server/ops/*.rs`
- `src/server/js_runtime.rs`
- `tests/{package_loading,package_graph,package_conflicts,clay_js_api_inventory,primitives_docs}.rs`

## Overview

Clay currently ships eleven PackageService-managed JavaScript packages plus one private LSP helper directory. Most package behavior is already expressed through generic contribution registries. Third-party packages do not need access to first-party JavaScript objects; they need public registration APIs, target-declared extension-point metadata, exact user-approved mutation edges, and replacement semantics that preserve provenance.

The rebaseline corrected four current-state assumptions:

- `runtime/js/` contains 23 TypeScript files, but `ClayModuleLoader` admits 21 `clay:*` modules. `mod.ts` is a barrel source and `web-tree-sitter-host.ts` is not runtime-admitted.
- Current steady-state runtime topology is one main `JsRuntime` plus up to four dedicated document-analysis `JsRuntime` workers (`DOCUMENT_ANALYSIS_MAX_WORKERS`), not one process-wide runtime instance. Each owns a `clay-js-runtime` thread; timeout watchdog threads are transient.
- Production `ClayOpState` creates an in-memory `PackageService` with `FakeBackend` and an empty store. The CLI creates a separate process-local `PackageService` with `PnpmBackend`; installed third-party records are not injected into the server runtime today. Non-bundled `loadPackage` succeeds only when tests/host code prepopulate that runtime-local service.
- Open-time classification currently calls `serverListFirstPartyPackageSpecifiers()` and auto-loads bundled packages until a non-`core.*` mode matches, even without an explicit `init.js` load. Plan 061 must reconcile that compatibility path with explicit package loading/adoption instead of treating current behavior as already one-line-only.

The minimum design therefore avoids bespoke APIs such as `extendRustMode` or `patchMarkdownParser`. Third-party code registers its own provider/contribution in the third-party runtime and targets a declared first-party extension point through Rust-owned registries. A full semantic rewrite uses approved package replacement rather than internal monkey-patching.

## Runtime and Ownership Rules

- Trusted runtime executes Clay configuration/bootstrap and exact integrity-verified bundled first-party packages.
- Third-party runtime executes all adopted packages as one disclosed shared trust cohort.
- Installed op/module allowlists differ by runtime. Third-party code cannot import first-party implementation modules or call Clay-internal ops.
- Cross-domain requests carry inert values only and retain requester, target, contribution, runtime generation, and approval provenance.
- First-party extension requires target owner declaration plus user approval.
- A third-party replacement remains third-party; it never acquires the target's identity or trusted runtime placement.
- Clay core, `core.text`, `core.code`, server document authority, native shell, and Rust bootstrap are not packages and are not disabled by package replacement.

## Generic Public API Families Required in the Third-Party Runtime

| Family | Existing direction | Required third-party use | Boundary |
| --- | --- | --- | --- |
| Package inspection | `clay:packages`, `PackageService::inspect` | Inspect own provenance, active dependencies, declared extension points, approvals, active owners, and stale/revoked status. | Read-only; package code cannot approve/promote itself. |
| Syntax | `clay:syntax.serverRegisterSyntaxGrammar` | Register package-owned grammar/query/style contributions and target an owner-approved grammar extension/replacement point. | Native artifacts remain Clay-owned inventory; package assets remain confined/validated. |
| Modes | `clay:modes.serverRegisterModePattern` | Add mode patterns or approved additions/replacements to first-party mode contributions. | No direct mutation of `core.*`; registry keeps actual owner. |
| Commands/behavior | `clay:commands`, `clay:behavior`, key routing/text transforms | Add commands, key routes, and inert transforms or replace exact exposed contribution IDs. | No arbitrary client callback or hot-path JS. |
| Completion | `clay:completion.serverRegisterCompletionProvider` | Add/priority-route package-owned providers for first-party modes or replace an exposed provider. | Bounded request/output; provider callback stays in third-party runtime. |
| Parse/decorations/diagnostics | `clay:parse`, `clay:decorations`, `clay:diagnostics` | Add package-owned semantic layers and diagnostics for first-party modes/documents. | No first-party parser function crosses runtimes; document/version/payload checks remain host-owned. |
| Language intelligence | `clay:language.serverRegisterDocumentAnalyzer` and provider APIs | Add or replace approved analyzers/intelligence providers for a first-party language. | Exact package provenance, root, generation, limits, and stale rejection. |
| Language server | `clay:language-server` | Use a package's own approved fixed contribution or replace an exposed LSP bridge package. | No mutation of another package's executable/argv/grant; existing trusted-subprocess authority remains. |
| UI/SDUI | `clay:ui`, `clay:sdui` | Add inert panels/components/status/overlays or replace exact exposed package regions. | Clay owns native widgets/layout/action routing; no client JS/raw CSS/native handles. |
| Theme | `clay:theme` plus typed token contributions | Derive a theme from exposed token values, override typed tokens, or fully replace a theme package. | Inert typed values only; no renderer callbacks. |
| Git | `clay:git.serverListGitStatuses` and refresh API | Build commands, decorations, or UI from server-owned cached Git metadata. | Read-only status/refresh authority; no shell or mutating Git authority. |
| Composition | New package graph/extension-point inspection and registration facades | Declare exact `dependsOn`/`imports`/`extends`/`overrides`/`disables`/`replaces` targets and scopes. | Host checks owner declaration + user approval; no runtime self-approval. |

## Locked Extension-Point and Relation Schemas

First-party manifests publish inert, versioned extension points (`clay-extension-point-v1`); third-party manifests request exact relationships (`clay-package-relation-v1`) before execution; user approvals are durable host-written records (`clay-package-approval-v1`); whole-package replacement is a user-approved atomic record (`clay-package-replacement-v1`); cross-domain invocation uses the Rust-mediated bounded `clay-cross-domain-envelope-v1`. The canonical field lists, closed enums, caps, stale-approval rules, and rejection rules live in `docs/reference/primitives/package-security.md#package-runtime-trust-domains-and-extension-authority`; the adoption/replacement activation sequence lives in `docs/reference/primitives/package-loading.md#plan-061-adoption-replacement-and-two-domain-loading`. Package-authored `summary`/`justification` text may aid the UI but never substitutes for host-rendered authority facts.

## Current First-Party Package Inventory

### `@clay/markdown`

Current contributions: native syntax grammar metadata, mode pattern, commands, key routing, text transforms, completion, parse fallback, status component, and optional preview panel/SDUI.

Expose declaratively:

- Append/replace completion providers for Markdown mode.
- Append package-owned parse, decoration, diagnostic, and intelligence layers.
- Append/replace exact commands, key routes, and text-transform rules.
- Add/replace preview/status panel or SDUI region contributions.
- Replace grammar/query/style-map contribution under explicit approval.
- Replace entire package for alternate Markdown semantics.

Do not expose trusted `markdown-it` function/plugin objects across runtimes. A third-party parser runs as its own bounded provider; a complete parser change uses replacement.

### `@clay/rust`

Current contributions: native grammar metadata, Rust mode pattern, line-comment command, keyword/snippet completions, and status item.

Expose declaratively:

- Append/replace completion and snippet providers.
- Append package-owned commands, analyzers, diagnostics, and UI.
- Extend file/mode patterns without claiming `core.*` ownership.
- Replace exact grammar/mode/status/command contributions or entire package.

No Rust-specific privileged op is needed.

### `@clay/typescript`

Current contributions: native grammar metadata, TypeScript mode pattern, command, keyword/snippet completions, and status item.

Expose the same generic grammar/mode/command/completion/analyzer/UI extension points as `@clay/rust`. JSX/TSX pattern changes remain typed mode/grammar contributions, not direct trusted-module mutation.

### `@clay/javascript`

Current contributions: native grammar metadata, JavaScript mode pattern, command, keyword completion, and status item.

Expose the same generic grammar/mode/command/completion/analyzer/UI extension points as the TypeScript package. Third-party packages may add framework-specific providers without modifying trusted JavaScript objects.

### `@clay/git`

Current contributions: read-only server-cached status model and one SDUI region. Existing `clay:git` status listing/refresh APIs are the only package-specific public data surface currently justified.

Expose:

- Read cached statuses through bounded public `clay:git` APIs.
- Add package-owned commands, decorations, status items, panels, or SDUI based on that data.
- Replace the `git.status` region or whole package with approval.

Do not expose shell commands, repository handles, arbitrary paths, or mutating Git authority as part of this plan.

### `@clay/lsp-rust`, `@clay/lsp-typescript`, `@clay/lsp-javascript`, `@clay/lsp-markdown`

Current contributions: one fixed language-server descriptor, completion provider, language-intelligence provider, and document analyzer/bridge per language.

Expose generically:

- Add another completion/intelligence/analyzer provider with explicit routing/priority.
- Replace an exact provider or the complete LSP bridge package.
- Use a third-party package's own separately approved fixed language-server contribution.

Do not expose mutation of a shipped package's executable, argv, inherited environment, roots, descriptor fingerprint, or grant. Replacement receives a new grant bound to replacement provenance.

### `@clay/theme-gruvbox-material-dark` and `@clay/theme-gruvbox-material-light`

Current contributions are inert text/UI/syntax style token values.

Expose declaratively:

- Derive from an identified theme package/version.
- Override typed token values while preserving core fallbacks and schema validation.
- Fully replace the selected theme package.

No runtime callback API is needed.

### `packages/lsp-shared`

This directory is a private pure-JavaScript helper, not a PackageService package. It contains framing, UTF-8 position, mapping, client, and TypeScript-language-server adapter utilities.

Decided at Plan 061 task 8: keep it private. Third-party language-server bridges use the public `clay:language-server` session APIs with their own approved fixed contribution and do not import `lsp-shared` or any other package's modules. Do not load this helper in the trusted runtime merely to share JavaScript objects across domains.

## Full First-Party Replacement

A third-party package may declare `replaces` for a PackageService-managed first-party target. Adoption must show the target, replacement provenance, withdrawn contribution categories/IDs, requested capabilities/processes, compatibility claims, and rollback behavior.

Activation is host-owned and atomic:

1. Validate/install replacement without execution.
2. Verify target exists and is package-managed.
3. Require exact user approval and `package-control`.
4. Build candidate third-party generation and contributions.
5. Check replacement/compatibility/conflicts.
6. Withdraw first-party package contributions.
7. Publish candidate as active owner while preserving replacement provenance.
8. Revoke target handlers/sessions/outputs.
9. Roll back to target if candidate activation fails before commit.

Replacement does not grant the target's trusted runtime, package identity, executable grants, or internal APIs. Dependencies satisfied by a replacement require an explicit compatibility contract; they must not be inferred from package name alone.

## Adoption and Replacement Interaction Contract (Plan 061 task 9)

Finalized interaction design. Catalog review outcome: the existing implemented kinds compose to the full flow — `portal` > `overlay` (anchor `WorkingArea`, modal focus policy) > `scroll` > `flex` column of `label` section headers and `list` fact rows, with a footer `flex` row of `button` actions. No new component kind is justified; the reserved `modal` kind stays reserved because the flow is deliberately non-blocking (the editor remains interactive and the package simply stays disabled until a decision).

### States

State-complete adoption flow (every transition is host-owned; none executes package-authored UI):

```text
installed (non-executing)
  -> pending inspection      first third-party load creates a pending adoption record
  -> approved                exact user approval; package may execute
  -> rejected                stays installed-not-enabled; pending record retained, re-prompted on next load
approved
  -> stale approval          identity/version/integrity drift or target replacement invalidates; re-inspection required
  -> authority expansion     wider capabilities/processes/scopes requested; diff shown, fresh approval required
  -> disabled                user disable; contributions withdrawn
  -> revoked                 approval record revoked; immediate disable + withdrawal
disabled
  -> approved                re-inspection not required while approval is exact and current
replacement
  -> pending replacement     replacement candidate validated without execution
  -> danger confirmation     withdrawn target contributions/categories shown; explicit danger action required
  -> candidate activation    atomic host-owned activation (Plan 061 activation sequence)
  -> failed candidate        activation failure before commit -> automatic rollback to target; informational overlay
  -> rollback complete       target restored; replacement provenance retained in records
```

### Layout and composition

- Surface: `portal` hosting an `overlay` anchored to `WorkingArea`, modal focus policy while open, dismissal = reject (fail closed).
- Body: `scroll` region (bounded height) containing one `flex` column. Sections in fixed order, each a `label` header plus `list` rows (`title` + `detail`):
  1. **Identity**: source, version, integrity/provenance fingerprint.
  2. **Runtime disclosure**: shared third-party trust-cohort sentence (plain language, always present for third-party packages).
  3. **Capabilities**: requested permissions rows.
  4. **External processes**: language-server executables/args rows (danger emphasis).
  5. **Dependencies and relations**: depends/imports/extends rows with target extension point, version, and operation.
  6. **Mutation scopes**: per-relation scope rows; any `prefix.*` wildcard renders a wildcard-warning row with danger emphasis.
  7. **Withdrawals** (replacement only): disabled/replaced packages and withdrawn contribution categories/IDs.
  8. **Package summary** (optional, last): package-authored `summary`/`justification`, visually muted, labeled as package-provided.
- Footer: `flex` row of actions. Adoption: `Approve` (primary) + `Reject` (default). Replacement: `Replace` (danger) + `Keep current` (default). Revocation flow adds `Revoke approval` (danger). Rollback outcome is informational with a single `Dismiss` (default).
- Long lists overflow inside the `scroll` region only; the footer never scrolls away.

### Keyboard, focus, and motion

- Initial focus on the non-mutating action (`Reject`/`Keep current`); approval and danger actions require deliberate traversal. `Tab`/`Shift+Tab` cycles footer actions then the scroll region; arrow keys scroll; `Esc` rejects/dismisses (fail closed); `Enter` activates the focused action.
- No decorative motion: no fade, slide, or scale transitions on open/close or state changes; the surface appears/disappears immediately. This satisfies reduced-motion by construction.
- Editor remains painted and interactive underneath; approval is asynchronous and bounded — no startup or GUI-client deadlock wait, and noninteractive paths fail closed.

### Security rules

- Every fact row is host-rendered from `PackageInspection`, authorization, approval, and graph records; package-authored text can appear only in the muted final section and can never hide, reorder, or replace authority facts or actions.
- Action intents are inert command intents handled by host code; the approval write path is `PackageService` only.
- Approval records persist with owner-only (0o600) permissions and fail closed on corruption.

### Design checklist (verified against this contract)

- [x] Keyboard/focus: safe-default initial focus, full traversal, Esc fail-closed.
- [x] Reduced motion: zero transitions by construction.
- [x] Overflow/long lists: bounded scroll region, fixed footer.
- [x] Wildcard warning: `prefix.*` scopes render danger-emphasis rows.
- [x] External process disclosure: dedicated section with executables/args.
- [x] Rejection: installed-not-enabled, re-prompt on next load.
- [x] Stale state: identity/version drift re-inspection with diff emphasis.
- [x] Rollback: failed candidate auto-rolls back; informational overlay retains provenance.

## Performance and Security Constraints

- Baseline source permits one main runtime plus at most four document-analysis runtimes; Plan 061 replaces that topology with exactly two persistent application runtime domains and routes analyzer callbacks to their owner domain.
- No per-package isolate/thread by default.
- Separate op/module allowlists and heap/time generations.
- Cross-domain queues and payloads are bounded and off editor hot paths.
- Third-party runtime can be terminated/rebuilt without replacing trusted runtime.
- No package JavaScript runs before adoption approval.
- Target runtime records are exact-provenance, generation-scoped, durable, inspectable, and revocable. Current package authorization/revocation state is in-memory and not yet durable.
- Shared third-party runtime is not a hostile sibling sandbox; docs/UI state this plainly.

## Tests

- `tests/primitives_docs.rs`: inventory/index/decision coverage.
- Plan 061 will add executable two-domain, op/module denial, adoption, extension, replacement, revocation, and resource tests.

```bash
cargo test --test protocol primitives_docs::
```

## Related

- [Package Principal and Result Routing Primitive Review](package-principal-and-result-routing-primitive-review.md)
- [Third-Party Runtime Authority](third-party-runtime-authority.md)
- [Embedded JavaScript Runtime](embedded-js-runtime.md)
- [Primitive Architecture](primitive-architecture.md)
- `decision-logs/2026-07-21-0001-two-package-runtime-trust-domains.md`
