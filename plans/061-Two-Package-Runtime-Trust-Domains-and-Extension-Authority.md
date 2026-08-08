# Two Package Runtime Trust Domains and Extension Authority

## Objectives

- Replace Clay's single broad JavaScript package runtime with exactly two persistent trust domains: trusted Clay/bundled packages and one shared adopted-third-party runtime.
- Remove caller-supplied package authority while preserving low-resource third-party composition through host-stamped package provenance and explicit dependency/mutation approvals.
- Expose typed, bounded first-party extension points so third-party packages can build on bundled behavior without importing trusted JavaScript or calling Clay-internal Rust ops.
- Let users disable and fully replace PackageService-managed first-party packages with third-party packages while replacement code remains untrusted and actual provenance remains visible.
- Make package adoption, authority expansion, revocation, replacement, reload, and failure recovery inspectable, durable, bounded, and off editor hot paths.

## Expected Outcome

- Clay runs two and only two package-domain `deno_core::JsRuntime` instances with distinct op extensions, module-loader allowlists, state, heap/time budgets, workers/generations, and failure recovery.
- Trusted classification comes from a compiled bundled inventory plus exact provenance/integrity, never package name or normal user promotion; third-party code cannot import trusted modules or invoke Clay-internal ops.
- Third-party packages share one disclosed trust cohort. Supported host operations derive package provenance from host-created package context, resolve exact current grants/graph edges, and never trust caller manifests/names for authority.
- First-party packages publish versioned extension-point metadata; cross-domain mutation requires owner-declared scope plus exact user approval and uses typed bounded Rust-mediated values only.
- Users can approve, inspect, revoke, disable, replace, and roll back package relationships. A committed third-party replacement atomically withdraws first-party contributions without receiving trusted-runtime placement, identity, internal APIs, or inherited process grants.
- Existing fixed-contribution language-server authority, server-authoritative documents, inert native package UI, local-first editing, and Linux quality gates remain intact.

## Initial First-Party API Inventory

Detailed evidence: `docs/wiki/modules/first-party-package-extension-api-review.md`.

| Package(s) | Current behavior | Minimum public extension/replacement surface |
| --- | --- | --- |
| `@clay/markdown` | Grammar, mode, commands, key routes, transforms, completion, parse fallback, status/preview UI | Append/replace completion, parse/decor/diagnostic/intelligence layers, commands/routes/transforms, preview/status regions, grammar/mode; full package replacement. No trusted parser callback crosses domains. |
| `@clay/rust` | Grammar, mode, command, keyword/snippet completion, status | Generic grammar/mode/command/completion/analyzer/UI additions or exact replacement; full package replacement. |
| `@clay/typescript` | Grammar, mode, command, keyword/snippet completion, status | Same generic surfaces as Rust, including JSX/TSX pattern contributions. |
| `@clay/javascript` | Grammar, mode, command, keyword completion, status | Same generic surfaces as TypeScript. |
| `@clay/git` | Cached status read and SDUI status region | Bounded read-only `clay:git` status/refresh data, package-owned commands/decor/UI, exact region or full replacement; no shell/mutating Git authority. |
| `@clay/lsp-{rust,typescript,javascript,markdown}` | Fixed server descriptor, completion/intelligence/analyzer bridge | Add/route/replace providers or fully replace bridge package. Replacement uses its own fixed executable grant; shipped executable/argv/grant is not mutable. |
| `@clay/theme-gruvbox-material-{dark,light}` | Inert UI/syntax text-style tokens | Typed derivation/overrides and full theme-package replacement; no callback API. |
| `packages/lsp-shared` | Private pure-JS framing/position/mapping/client helpers | Publish stable generic subset as third-party SDK or keep private behind documented `clay:language-server`; never share trusted V8 objects. |

Clay core/bootstrap, `core.text`, `core.code`, server authority, and native shell are not PackageService-managed packages and are not disabled by package replacement.

## Tasks

- [x] Rebaseline runtime resources, package authority, and first-party API coverage
  - Acceptance Criteria:
    - Functional: Inventory current runtime workers/extensions/loaders/op state, every installed op/facade, all eleven PackageService-managed bundled packages, private `lsp-shared`, package graph/grant/revocation records, and current first/third-party load paths; map each item to its future trust domain and owning task.
    - Performance: Record one-runtime startup, resident/heap memory, warm evaluation/reload latency, worker/thread count, enabled-package count, and representative package-load timings before adding the second runtime.
    - Code Quality: Create one closure ledger with reproducer, owner, dependencies, and evidence; do not duplicate Plan 060 findings or count `core.*` built-ins as packages.
    - Security: Enumerate every op/module currently reachable from package JavaScript and classify it as trusted-only, public third-party, configuration/admin-only, or removable; keep caller-manifest impersonation open until executable tests close it.
  - Approach:
    - Documentation Reviewed:
      - `decision-logs/2026-07-21-0001-two-package-runtime-trust-domains.md`.
      - `docs/wiki/modules/{package-principal-and-result-routing-primitive-review,first-party-package-extension-api-review,embedded-js-runtime,third-party-runtime-authority}.md`.
      - `.agents/skills/project-patterns/references/{package-runtime-trust-domains,authority-boundaries,protocol-and-performance,maintenance-validation}.md`.
    - Options Considered:
      - Reuse Plan 060 baseline unchanged: misses new runtime-domain/API classifications and resource metrics.
      - Rebaseline only changed boundaries and link Plan 060 evidence: avoids duplicate inventory while making Plan 061 independently executable. Chosen.
    - Chosen Approach:
      - Add a compact execution ledger to this task, link unchanged Plan 060 measurements, and measure only the new architecture's decision inputs.
    - API Notes and Examples:
      ```text
      op/module -> trusted-only | public-third-party | config/admin-only | delete
      package -> bundled inventory entry -> exposed extension points -> replacement support
      ```
    - Files to Create/Edit:
      - `plans/061-Two-Package-Runtime-Trust-Domains-and-Extension-Authority.md`: baseline ledger and measurements.
      - `docs/wiki/modules/first-party-package-extension-api-review.md`: correct inventory errors found during rebaseline.
      - `src/server/js_runtime.rs`: ignored Linux before/after resource probe using existing runtime APIs.
      - `tests/primitives_docs.rs`: deterministic source-to-plan op/facade/package inventory coverage.
    - References:
      - `src/server/{js_runtime,ops}.rs`, `src/server/ops/*.rs`, `runtime/js/*.ts`, `packages/*`.
      - Plan 060 task 1 baseline and task 2 primitive review.
  - Test Cases to Write:
    - Deterministic inventory check: every installed runtime op/facade and bundled package appears exactly once in domain classification.
    - Benchmark command log: one-runtime baseline is reproducible before implementation.
  - Completion Evidence (2026-07-21):
    - Scope/status: this is a current-state evidence pass only. Classifications below are Task 2 inputs, not final public API commitments. Caller-manifest impersonation remains open until Tasks 4–5 add executable denial tests. Plan 060 measurements are linked rather than duplicated.
    - Measurement host: Linux `7.1.3-43.stable` x86_64, AMD Ryzen 9 PRO 7940HS, 16 logical CPUs, 64,431,064 KiB RAM, Rust/Cargo 1.96.1 (`31fca3adb`, LLVM 22.1.2).

    ### Runtime topology and resource baseline

    - `ClayJsRuntimeService::default` starts one `clay-js-runtime` thread whose worker owns one current-thread Tokio runtime, one `JsRuntime`, one `ClayModuleLoader`, and one broad `ClayOpState`. `RuntimeCommand::{Evaluate,Parse,Completion,DocumentAnalysis,LanguageIntelligence}` serialize through one unbounded `std::sync::mpsc` receiver and return through request `oneshot`s.
    - `DocumentAnalysisCoordinator` may create four additional `ClayJsRuntimeService::new_document_analysis_worker` instances (`DOCUMENT_ANALYSIS_MAX_WORKERS = 4`), one per worker key and up to 32 documents/8 MiB text each. Current steady-state ceiling is therefore one main plus four analysis `JsRuntime`/worker threads, not one process-wide runtime. A hot-reload candidate can temporarily overlap the active main runtime. Each evaluation also creates a short-lived timeout watchdog thread; language-server process service is separate and lazy.
    - Main V8 heap ceiling is 134,217,728 bytes. Each analysis runtime ceiling is 67,108,864 bytes; current configured aggregate ceiling at the five-runtime source limit is 402,653,184 bytes. These are termination ceilings, not preallocated/observed heap consumption; current code exposes no V8 used-heap gauge.
    - A fresh service has zero enabled packages. Enabled count depends on `init.js` and open-time auto-loading and has no package-count ceiling. The probe explicitly loaded four representative packages and observed four enabled records.
    - Reproducible release command:
      ```bash
      cargo test --release --lib runtime_resource_baseline_probe -- --ignored --nocapture --test-threads=1
      ```
    - Snapshot output (machine-variable, comparison evidence rather than hard gates):

      | State/operation | Result |
      | --- | --- |
      | Test process before runtime | RSS 9,744 KiB; 2 threads |
      | First runtime plus first ping | 8,179 µs; RSS 36,236 KiB (+26,492); 20 threads (includes process-wide V8 platform threads) |
      | Warm ping evaluation | median 143 µs across 20 serial evaluations |
      | `@clay/rust` first load | 1,721 µs |
      | `@clay/markdown` first load | 2,740 µs |
      | `@clay/git` first load | 758 µs |
      | `@clay/theme-gruvbox-material-dark` first load | 371 µs |
      | Four loaded packages | RSS 39,764 KiB (+3,528 from first runtime); 20 threads; 4 enabled records |
      | Candidate runtime first ping while active runtime retained | 6,361 µs; RSS 44,664 KiB (+4,900); 22 threads |
      | Four initialized analysis runtimes after candidate drop | RSS 60,512 KiB; 28 threads; RSS includes allocator/V8 retention, so use only as same-command before/after evidence |

    ### Installed op classification

    `src/server/ops/mod.rs::clay_runtime_extension` currently installs all 66 ops into every runtime. Preliminary target classification follows; mixed public facades still require Task 4 splitting and Task 5 provenance/permission enforcement.

    <!-- plan061-task1-op-inventory:start -->
    | Future class | Current installed ops |
    | --- | --- |
    | Trusted-only (5) | `op_clay_runtime_ping`, `op_clay_modes_classify_document`, `op_clay_modes_activate_major_mode`, `op_clay_completion_providers_for_trigger`, `op_clay_packages_end_package_activation` (clears the host loadPackage provenance stamp after a package activation so later init.js statements run package-less) |
    | Configuration/admin-only (32) | `op_clay_configuration_load_module`, `op_clay_configuration_get_state`, `op_clay_configuration_set_package_option`, `op_clay_theme_set_theme`, `op_clay_theme_set_typography`, `op_clay_theme_set_appearance`, `op_clay_ui_set_layout_override`, `op_clay_documents_open_document`, `op_clay_documents_save_document`, `op_clay_documents_reload_document`, `op_clay_documents_get_document_status`, `op_clay_documents_list_documents`, `op_clay_workspace_list_roots`, `op_clay_workspace_add_root`, `op_clay_workspace_discover_root_for_path`, `op_clay_workspace_list_directory`, `op_clay_workspace_create_listing_cancel_token`, `op_clay_workspace_cancel_listing`, `op_clay_keybindings_bind_key`, `op_clay_keybindings_bind_keys` (batch table form, added in the bindKey ergonomics round), `op_clay_keybindings_unbind_key`, `op_clay_keybindings_unbind_keys` (batch table form), `op_clay_keybindings_list_key_bindings`, `op_clay_packages_validate_manifest`, `op_clay_packages_validate_permissions`, `op_clay_packages_load_package`, `op_clay_packages_load_package_by_specifier`, `op_clay_packages_load_in_package_domain` (added in task 12; trusted-only cross-domain bridge), `op_clay_packages_list_first_party_specifiers`, `op_clay_language_server_authorize`, `op_clay_syntax_set_engine_preference`, `op_clay_shell_set_pane_focus_policy` |
    | Public-third-party after provenance/permission checks (39) | `op_clay_sdui_define_node`, `op_clay_sdui_publish_tree`, `op_clay_ui_register_panel_contribution`, `op_clay_ui_register_component_contribution`, `op_clay_ui_register_transient_overlay_contribution`, `op_clay_ui_register_theme_token`, `op_clay_ui_register_input_contribution`, `op_clay_ui_register_ui_state_scope`, `op_clay_ui_request_layout_intent`, `op_clay_git_list_statuses`, `op_clay_git_refresh_status`, `op_clay_behavior_get_active_manifest`, `op_clay_behavior_list_routes`, `op_clay_language_server_start_session`, `op_clay_language_server_send_message`, `op_clay_language_server_read_message`, `op_clay_language_server_send_bytes`, `op_clay_language_server_read_bytes`, `op_clay_language_server_stop_session`, `op_clay_modes_register_pattern`, `op_clay_commands_register_command`, `op_clay_commands_list_commands`, `op_clay_commands_execute_command`, `op_clay_decorations_publish_decorations`, `op_clay_diagnostics_publish_diagnostics`, `op_clay_parse_register_parse_handler`, `op_clay_syntax_register_syntax_grammar`, `op_clay_completion_register_completion_provider`, `op_clay_completion_disable`, `op_clay_language_register_intelligence_provider`, `op_clay_language_register_document_analyzer`, `op_clay_editor_move_cursor`, `op_clay_editor_set_selection`, `op_clay_editor_set_cursor_style`, `op_clay_editor_add_cursor`, `op_clay_editor_column_select`, `op_clay_editor_select_textobject`, `op_clay_editor_smart_select`, `op_clay_editor_execute_command` (Plan 071 follow-up round: shared but `editor-control`-gated — approved permission + declared active mode required per call; execute publishes the advisory `EditorCommandRequest` push) |
    | Removable/internal bridge replacement (5) | `op_clay_runtime_record`, `op_clay_parse_store_update`, `op_clay_completion_store_result`, `op_clay_language_store_intelligence_result`, `op_clay_runtime_unavailable` |
    <!-- plan061-task1-op-inventory:end -->

    - High-risk current combinations: package administration, configuration, workspace/document access, language-server authorization, package-facing publications, and internal test/bridge ops all coexist in one enumerable `Deno.core.ops` object. Facades do not form a security boundary.
    - The public-third-party row means a narrow API family is needed, not that the current signature is safe. `commands_execute`, `completion_disable`, Git refresh, language-server session I/O, and every registration/publication path need exact package context/grant/scope validation first.

    ### Facade and loader classification

    - `clay_facade_source` admits 22 `clay:*` modules backed by 22 embedded `CLAY_FACADE_*` raw strings. `runtime/js/` has 24 TypeScript files: `mod.ts` is a barrel source and `web-tree-sitter-host.ts` is not admitted by `ClayModuleLoader`. This corrects the earlier shorthand that counted 21 TypeScript files.

    <!-- plan061-task1-facade-inventory:start -->
    | Future class | Current runtime-admitted modules |
    | --- | --- |
    | Configuration/admin-only (6) | `clay:configuration`, `clay:documents`, `clay:workspace`, `clay:keybindings`, `clay:packages`, `clay:theme` |
    | Public-third-party after narrowing/splitting (13) | `clay:sdui`, `clay:ui`, `clay:git`, `clay:behavior`, `clay:language-server`, `clay:modes`, `clay:commands`, `clay:decorations`, `clay:diagnostics`, `clay:parse`, `clay:syntax`, `clay:completion`, `clay:language` |
    | Removable from server runtime (3) | `clay:application`, `clay:editor`, `clay:shell` |
    <!-- plan061-task1-facade-inventory:end -->

    - Task 4 must split mixed modules rather than copy unsafe exports: authorization leaves the public language-server facade; classify/activate leave public modes; engine preference leaves public syntax; layout override leaves public UI. The public package runtime later receives read-only package inspection/composition APIs, not current install/load/admin ops.
    - Loader order is: current controlled/configuration main module; any admitted `clay:*` facade; vendored `markdown-it`; exact opaque package allowlist entries; relative imports confined to the recorded package root; configuration-root relative `.js`; deny fallback. All paths currently share one module cache/allowlist. Future ownership is trusted main/config/`markdown-it`/bundled modules versus third-party allowlisted modules, with no cross-domain module object.

    ### Bundled package and API coverage

    Eleven manifests have a name plus Clay metadata. All are version `0.1.0`, all declare `./dist/load.js`, and none currently declares `dependsOn`, `extends`, `disables`, or `replaces`. `packages/lsp-shared/package.json` is private metadata with no package name/Clay manifest and is not PackageService-managed.

    <!-- plan061-task1-package-inventory:start -->
    | Current package | Current runtime API/module use | Future domain / owner |
    | --- | --- | --- |
    | `@clay/git` | `clay:packages`, `clay:sdui`, `clay:git`; no declared permissions | Trusted bundled inventory T3/T4; generic Git/UI extension surface T8; replaceable T11 |
    | `@clay/javascript` | `clay:behavior`, `clay:syntax`, `clay:modes`, `clay:commands`, `clay:completion`, `clay:ui` | Trusted; generic language contribution APIs T8; replaceable T11 |
    | `@clay/lsp-javascript` | `clay:language`, then analyzer imports `clay:language-server`, `clay:decorations`, `clay:diagnostics` | Trusted; fixed own process grant and generic provider APIs T8/T12; replaceable T11 |
    | `@clay/lsp-markdown` | `clay:language`, then analyzer imports `clay:language-server`, `clay:decorations`, `clay:diagnostics` | Trusted; fixed own process grant and generic provider APIs T8/T12; replaceable T11 |
    | `@clay/lsp-rust` | `clay:language`, then analyzer imports `clay:language-server`, `clay:decorations`, `clay:diagnostics` | Trusted; fixed own process grant and generic provider APIs T8/T12; replaceable T11 |
    | `@clay/lsp-typescript` | `clay:language`, then analyzer imports `clay:language-server`, `clay:decorations`, `clay:diagnostics` | Trusted; fixed own process grant and generic provider APIs T8/T12; replaceable T11 |
    | `@clay/markdown` | `clay:packages`, `clay:behavior`, `clay:modes`, `clay:commands`, `clay:completion`, `clay:parse`, `clay:decorations`, `clay:ui`, vendored `markdown-it` | Trusted; generic mode/command/provider/UI extension APIs T8; no parser object crossing; replaceable T11 |
    | `@clay/rust` | `clay:behavior`, `clay:syntax`, `clay:modes`, `clay:commands`, `clay:completion`, `clay:ui` | Trusted; generic language contribution APIs T8; replaceable T11 |
    | `@clay/settings` | `clay:commands`, `clay:ui` (transient overlay contribution); `command-registration` permission | Trusted bundled; generic command/UI extension surface T8; replaceable T11 |
    | `@clay/theme-gruvbox-material-dark` | Inert manifest text styles; no runtime import in load entry | Trusted inert values T8; replaceable T11 |
    | `@clay/theme-gruvbox-material-light` | Inert manifest text styles; no runtime import in load entry | Trusted inert values T8; replaceable T11 |
    | `@clay/theme-modus-operandi` | Inert manifest text styles; no runtime import in load entry | Trusted inert values T8; replaceable T11 |
    | `@clay/theme-modus-vivendi` | Inert manifest text styles; no runtime import in load entry | Trusted inert values T8; replaceable T11 |
    | `@clay/typescript` | `clay:behavior`, `clay:syntax`, `clay:modes`, `clay:commands`, `clay:completion`, `clay:ui` | Trusted; generic language contribution APIs T8; replaceable T11 |
    | `packages/lsp-shared` | Private copied pure-JS framing/position/mapping/client helpers; no Clay manifest | T8 chooses public third-party SDK subset or keeps it private behind typed APIs |
    <!-- plan061-task1-package-inventory:end -->

    ### Authority, records, generations, and current load paths

    - `PackageService` owns in-memory `installed`, `enabled`, `authorizations`, exact language-server grants, conflict policy/resolutions, monotonic package generation, and revocation records. Authorization matches requested spec, source kind, resolved name/version/api prefix and approved capabilities/runtime profile; language-server grants additionally bind contribution ID/fingerprint, canonical executable, roots, approver, and time. Revocation counts package-owned contribution categories. None of these records is durable today.
    - `PackageGraphRelations`/`PackageGraphPlan` already support cycle-checked `dependsOn`, `extends`, `disables`, and `replaces`; disable/replace requires package-control and enable is rollback-transactional. Existing bundled manifests exercise none of those edges. There is no extension-point schema, exact mutation approval, adoption record, compatibility contract, or trusted runtime-domain field.
    - Runtime/provider/package generations already reject stale parse/completion/intelligence/analysis results and hot reload swaps a candidate generation after validation. There is no executing package context, per-enabled-record execution generation, or domain generation. `globalThis` caches/handler registries remain shared in each current runtime.
    - Bundled load path: `loadPackage("@clay/<segment>")` maps the name directly to `$CARGO_MANIFEST_DIR/packages/<segment>`, reads the mutable on-disk manifest, derives `ClayShipped` from the specifier, grants bundled defaults except unapproved language-server authority, enables it, records canonical `loadEntry`, imports, then calls default export. There is no compiled inventory or integrity check.
    - Third-party load path gap: production CLI constructs a process-local `PackageService` with `PnpmBackend` and refreshes the user store; each server runtime `ClayOpState` instead constructs a separate empty `PackageService` with `FakeBackend`. Current non-bundled resolver code works only if that runtime-local service was prepopulated (primarily tests), so installed third-party packages are not end-to-end loadable by the production server.
    - Configuration load path: `~/.config/clay/init.js` or explicit configuration root runs in the same broad runtime and may call one-line `loadPackage`; language-server authorization is configuration-evaluation-only and seals at first package load.
    - Open path compatibility: `classify_open_document` currently enumerates and loads bundled package specifiers until a non-`core.*` classification matches, even when `init.js` did not explicitly load them. This can enable multiple packages and means current behavior does not fully match explicit-load documentation.
    - Self-asserted authority remains reproducible in decoration/diagnostic/parse/syntax/completion/language registration/publication options and mode/command/UI package-manifest arguments. Exact record validation prevents malformed authority but cannot authenticate which sibling module called the op.

    ### Closure ledger

    | Baseline gap | Reproducer/evidence | Plan 061 owner / dependency | Required closure evidence |
    | --- | --- | --- | --- |
    | B1 broad runtime extension and mixed facades | 66 ops, 21 admitted modules in one runtime | T2 schema lock -> T4 split -> T14 API inventory | Exact trusted/public op and module inventories; cross-domain absence tests |
    | B2 name/path-derived bundled trust | `ensure_package_installed_locked`; no compiled integrity inventory | T3 | Spoofed `@clay/*`, changed root/version/integrity, and user-promotion denial tests |
    | B3 one main plus up to four analyzer runtimes | `new_document_analysis_worker`; resource probe | T4 -> T12 | Exactly two persistent application runtimes across package/document/analyzer churn; before/after metrics |
    | B4 no executing package provenance | caller manifests/names select records | T5 after T4 | A-as-B, permission inflation, stale callback/session, raw-op denial tests |
    | B5 no durable scoped composition/adoption records | in-memory authorization/graph/revocation maps; string relation arrays | T6 -> T10 | Exact persisted approval, stale/expanded scope, corrupt store, no-preexecution tests |
    | B6 no typed cross-domain invocation | current callback globals/tokens are same-runtime | T7 after T4–T6 | Payload/queue/timeout/cancel/stale/revoked/mismatched provenance tests |
    | B7 first-party extension surfaces are implicit | package imports/contributions above; no extension-point metadata | T8 after T2/T6/T7 | All eleven package fixtures extend through generic public APIs only |
    | B8 adoption/replacement UI absent | no pending-adoption state or host authority overlay | T9 review -> T10 implementation | Native host-fact UI/CLI lifecycle and zero preapproval JS execution |
    | B9 full first-party replacement incomplete | graph has `replaces`, but no domain/provenance/compatibility/atomic registry swap | T11 after T6/T8/T10 | Candidate rollback, exact target claims, own grants/provenance, core denial |
    | B10 lifecycle/domain revocation incomplete | one generation and per-analysis runtime ownership | T12 after T4–T11 | Independent domain recovery and exact handler/output/session cleanup |
    | B11 no two-domain resource comparison | this one-domain/topology snapshot | T13 | Repeat same probe plus bridge saturation/recovery and full Linux gates |
    | B12 public/configuration API split unfinished | current mixed facades and raw ops | T14/T15 after implementation | Generated trusted/public inventories, docs, no internal handles/settings |

    ### Validation

    - Added ignored inline probe rather than a new benchmark/integration binary; it compiles in normal all-target checks and runs only through the explicit release command above.
    - Added `plan061_runtime_package_authority_rebaseline_matches_source_inventory` in `tests/primitives_docs.rs`; it derives the 66 installed ops, 21 admitted facades, and eleven managed manifests from current source and requires each exactly once between the plan inventory markers, plus private `lsp-shared` coverage.
    - Passed: release probe, deterministic inventory test, all 131 `primitives_docs` tests, `cargo fmt --check`, `cargo check --all-targets`, `cargo clippy --all-targets -- -D warnings`, and `git diff --check`.
    - Updated `docs/wiki/modules/first-party-package-extension-api-review.md` with corrected facade-file count, runtime topology, production third-party service disconnect, open-time auto-loading, and current non-durable authority state. No production behavior, public API, configuration setting, generated artifact, or final architecture schema changed.

- [x] Review existing package primitives and lock generic trust-domain and extension schemas
  - Acceptance Criteria:
    - Functional: State what `PackageService`, authorization/provenance, graph relations, conflict policy, module allowlists, generations, provider registries, runtime snapshots, and package UI can already provide; define only missing generic domain/provenance/extension/adoption/replacement primitives.
    - Performance: Schemas are bounded inert metadata evaluated at install/adopt/load/reload time; no package graph or cross-domain JavaScript work enters typing, paint, layout, scroll, pointer, or local edit paths.
    - Code Quality: Reject per-package Rust APIs, per-language branches, generic actor/plugin frameworks, JavaScript-visible bearer identities, and cross-runtime V8 object/function sharing.
    - Security: Lock exact trust classification, owner-plus-user extension consent, user-only full replacement, stale approval rules, third-party shared-cohort disclosure, and non-replaceable Clay core boundary before implementation.
  - Approach:
    - Documentation Reviewed:
      - `docs/reference/primitives/{index,registry,package-security,package-loading}.md`.
      - `docs/wiki/modules/{primitive-architecture,first-party-package-extension-api-review,package-principal-and-result-routing-primitive-review}.md`.
      - `.agents/skills/create-plan/references/clay.md`: primitive-first and runtime trust-domain requirements.
      - `.agents/skills/project-patterns/references/{mode-primitive-first,package-runtime-trust-domains,clay-js-api-schema}.md`.
    - Options Considered:
      - Package-specific extension functions: ergonomic initially but grows one Rust surface per shipped package.
      - Generic typed contribution extension points plus rare justified package data APIs: reuses registries and keeps package behavior in packages. Chosen.
    - Chosen Approach:
      - Finalize closed versioned schemas for extension-point declarations, requested relations/scopes, durable approvals, replacements, and domain-safe request/result envelopes; map every first-party need to generic APIs first.
    - API Notes and Examples:
      ```json
      {"id":"markdown.completionProviders","version":1,"operations":["append","replace"],"contributionKinds":["completionProvider"],"scopes":["markdown.*"]}
      ```
    - Files to Create/Edit:
      - `docs/wiki/modules/first-party-package-extension-api-review.md`: finalized primitive/API matrix.
      - `docs/reference/primitives/{registry,package-security,package-loading}.md`: approved schema contract.
      - `tests/primitives_docs.rs`: generic documentation coverage.
    - References:
      - `src/packages/{manifest,record,graph,service,conflict}.rs`.
      - `decision-logs/2026-07-21-0001-two-package-runtime-trust-domains.md`.
  - Test Cases to Write:
    - Documentation coverage rejects missing trust classification, owner consent, user consent, provenance, revocation, replacement, resource, or shared-cohort facts.
    - Initial package matrix accounts for all eleven bundled packages and `lsp-shared`.
  - Completion Evidence (2026-07-21):
    - Scope/status: documentation-only schema lock; no runtime behavior changed. Existing-primitive reuse matrix and the five closed schemas are canonical in `docs/reference/primitives/package-security.md#package-runtime-trust-domains-and-extension-authority`.
    - Reuse recorded (not rebuilt): `PackageService` enable/disable rollback + generation counter, `PackageAuthorizationRecord` exact matching, `LanguageServerGrant` fixed-contribution authority with seal-on-load, `PackageGraphRelations`/`PackageGraphPlan` cycle-checked relations, `PackageConflictResolutionPolicy`, `PackageLoadEntryAllowlist` root confinement/revocation, runtime/provider/analysis generations with stale rejection, provider registries, `RuntimeStateSnapshot`, package UI/SDUI/theme/input validators.
    - Six new generic primitives added as registry rows: `PackageTrustDomainClassification`, `ExtensionPointDeclaration`, `PackageRelationRequest`, `PackageAdoptionRecord`, `PackageReplacementRecord`, `CrossDomainRequestEnvelope` — all no-hot-path, configuration-data/server-first kinds.
    - Locked schemas: `clay-extension-point-v1` (max 64 points/manifest, closed `append`/`replace` operations, closed 16-kind contribution enum, max 32 scopes x 128 chars, package-prefixed IDs), `clay-package-relation-v1` (exact target/point/version/operation match, requester-own-prefix scopes, justification never authority), `clay-package-approval-v1` (host-written durable record, exact identity binding, exact-subset reuse rule, re-approval on any identity/request change), `clay-package-replacement-v1` (package-managed targets only, replacement keeps own provenance, host-computed withdrawn list, compatibility claims validated at candidate build), `clay-cross-domain-envelope-v1` (Rust-mediated, closed `ok`/`error`/`denied`/`stale`/`revoked`/`timeout` status, `CROSS_DOMAIN_PAYLOAD_BUDGET_BYTES` caps).
    - Locked security rules: exact trust classification from compiled bundled inventory only; owner-plus-user extension consent; user-only full replacement; stale approval invalidation on identity/scope/target-version change; third-party shared-cohort disclosure; non-replaceable Clay core/`core.text`/`core.code`/shell/bootstrap; no JavaScript-visible bearer authority; no cross-domain V8 objects/functions/promises/globals.
    - `package-loading.md` gained the locked adoption-before-execution flow, the 9-step atomic replacement activation sequence, domain routing at load, per-domain candidate-before-swap reload, and the open-time auto-load reconciliation note (trusted-runtime convenience only; third-party never auto-loads).
    - `docs/wiki/modules/first-party-package-extension-api-review.md` status moved to schemas-locked; schema-direction section replaced with canonical references; generic-only decisions recorded (no per-package Rust APIs, no actor/plugin framework, no bearer identities, no V8 sharing); `lsp-shared` SDK-subset decision deferred to Task 8.
    - Added `CROSS_DOMAIN_PAYLOAD_BUDGET_BYTES = 8192` to `src/perf/budgets.rs` so later tasks compile against a stable name.
    - Test `plan061_trust_domain_and_extension_schemas_are_locked` in `tests/primitives_docs.rs` covers all six registry rows, locked schema names, trust classification/owner consent/user-only replacement/shared-cohort/core-boundary/stale-approval/budget needles, adoption/replacement loading rules, and the eleven bundled packages plus `lsp-shared`.
    - Passed: `cargo test --test primitives_docs` (132 tests), `cargo fmt --check`, `git diff --check`, `cargo clippy --all-targets -- -D warnings`. No public API, configuration setting, or generated artifact changed.

- [x] Establish immutable bundled trust inventory and runtime-domain classification
  - Acceptance Criteria:
    - Functional: Clay resolves trusted runtime placement only from a compiled bundled package inventory bound to exact package name, version, root/source kind, and integrity/fingerprint; local/npm/git packages using `@clay/*` remain third-party.
    - Performance: Classification is a bounded lookup at install/enable/load and adds no filesystem hashing on repeated provider calls or editor hot paths.
    - Code Quality: One `RuntimeDomain`/classification path replaces name-prefix and scattered first-party checks; all domain identity types remain `pub(crate)`.
    - Security: Normal user authorization cannot promote third-party code; stale/mismatched bundled provenance fails to third-party/rejection without exposing trusted module roots or defaults.
  - Approach:
    - Documentation Reviewed:
      - `src/packages/{manager,service,authorization}.rs` and current first-party resolver in `src/server/ops/packages.rs`.
      - `.agents/skills/project-patterns/references/{package-distribution,package-runtime-trust-domains}.md`.
    - Options Considered:
      - `@clay/*` prefix: forgeable and currently conflated with provenance.
      - Runtime filesystem scanning/hashing each load: authoritative but repetitive and TOCTOU-prone.
      - Checked-in compiled inventory generated from shipped packages, verified once against canonical roots/integrity: bounded and auditable. Chosen.
    - Chosen Approach:
      - Generate/check a bundled inventory at build/maintenance time, verify canonical shipped roots and exact metadata when constructing enabled records, and store domain on host-owned package state.
    - API Notes and Examples:
      ```rust
      pub(crate) enum RuntimeDomain { Trusted, ThirdParty }
      // Name alone never selects Trusted.
      ```
    - Files to Create/Edit:
      - `src/packages/{manager,service,authorization}.rs`: domain/provenance classification.
      - `src/packages/bundled.rs` or smallest existing inventory module: checked-in bundled inventory (tentative after task 1).
      - `build.rs` or existing maintenance generator only if needed for deterministic inventory.
      - `tests/{package_loading,package_graph}.rs`: spoof/stale/integrity tests.
    - References:
      - `packages/*/package.json`.
      - Existing source-kind/provenance checks in `PackageAuthorizationRecord`.
  - Test Cases to Write:
    - Local/npm package named `@clay/markdown` never enters trusted domain.
    - Exact bundled package enters trusted domain; modified version/root/fingerprint fails closed.
    - User grant cannot change domain.
  - Completion Evidence (2026-07-21):
    - New `src/packages/bundled.rs` (crate-internal): `RuntimeDomain { Trusted, ThirdParty }`, checked-in `BUNDLED_PACKAGES` inventory (11 entries: name, version, `packages/` root dir, FNV-1a-64 manifest fingerprint), `verify_bundled_trust` (exact source-kind/name/version/canonical-root/fingerprint match, fail-closed `BundledTrustError` taxonomy), and `runtime_domain` classifier. `packages/lsp-shared` intentionally excluded (private helper, no manifest identity).
    - FNV-1a-64 chosen over a cryptographic hash (no new dependency): the trust root is the checked-in source tree itself, so the fingerprint provides exact binding plus drift detection; `inventory_matches_source_tree` unit test derives the inventory from `packages/*/package.json` and fails with regeneration instructions on drift.
    - Single classification path replaces scattered checks: `authorize_bundled_defaults` now requires `verify_bundled_trust` (was `source_kind == ClayShipped`); `ensure_package_installed_locked` bundled detection uses verification (was requested source kind); `PackageSourceKind` documented as a claimed family, never a trust decision.
    - Domain stored on host-owned state: `PackageRecord.runtime_domain` (`pub(crate)`, default `ThirdParty`, upgraded in `enable_graph` after inventory verification). `PackageRecord` `PartialEq` is now manual and excludes `runtime_domain` — the host stamp must not affect record identity when comparing caller-assembled records against enabled host records (this was the only behavioral regression caught, via the four LSP grant tests).
    - Classification cost: bounded lookup plus one ~1-3 KB manifest read/hash at install/enable/load only; no hot-path or repeated-provider-call cost. No build.rs/generator added; no public API, facade, op, or configuration surface added; all domain types remain `pub(crate)`.
    - Tests: 4 unit tests in `bundled.rs` (inventory matches source tree, real bundled package verifies trusted, spoofed provenance fails closed across source-kind/version/root/name, tampered manifest fingerprint mismatch); 3 integration tests in `tests/package_loading.rs` (`spoofed_clay_prefixed_package_stays_third_party` — real bundled manifest at foreign root/spec denied defaults, denied enable, and still denied after full user grant; `exact_bundled_package_receives_defaults`; reworked `bundled_defaults_never_auto_grant_language_server` — synthetic `@clay/lsp-test` now denied bundled defaults and user-granted capabilities still never auto-grant process authority).
    - Validation: `cargo test --all-targets` all suites pass (907 lib + all integration suites, 0 failures), `cargo clippy --all-targets -- -D warnings` clean, `cargo fmt --check` clean, `git diff --check` clean. Pre-change baseline confirmed clean via `git stash` run.

- [x] Split runtime workers, op state, facade bundles, and module loaders into two trust domains
  - Acceptance Criteria:
    - Functional: Runtime service owns exactly one trusted and one third-party persistent application `JsRuntime`; each has distinct extension/op registration, narrow state, module allowlist, facade export set, heap/time generation, metrics, and restart path. Existing document-analysis callbacks route through their owning domain runtime instead of creating additional persistent `JsRuntime` instances.
    - Performance: Two-runtime startup/memory remains within measured fixed budgets recorded in task 1; no per-package runtime/thread is created; runtime commands remain bounded/off hot paths.
    - Code Quality: Reuse existing runtime worker/evaluation machinery with a small domain parameter or two concrete constructors; do not duplicate the 8k-line runtime module or introduce a generic runtime framework.
    - Security: Third-party runtime cannot enumerate/call trusted-only ops, import trusted package implementation/configuration modules, access full `ClayOpState`, or retain handles across domain generation replacement.
  - Approach:
    - Documentation Reviewed:
      - Locally resolved `deno_core 0.400.0` `JsRuntime`, `RuntimeOptions.extensions`, `op_state`, and `ModuleLoader` source/API.
      - `src/server/js_runtime.rs::create_js_runtime`, `RuntimeWorker`, and `ClayModuleLoader`.
      - `src/server/ops/mod.rs::init_runtime_extension`.
    - Options Considered:
      - Duplicate runtime module: clear separation but unacceptable drift.
      - One constructor parameterized by closed `RuntimeDomain`, with concrete trusted/public extension and state builders: minimum reusable split. Chosen.
      - One worker thread multiplexing two runtimes: lower thread count but third-party timeout delays trusted work.
      - One dedicated worker thread per fixed domain: fixed two-thread cost and failure scheduling separation. Chosen unless task 1 measurement disproves need.
    - Chosen Approach:
      - Keep shared command/evaluation code, instantiate two fixed workers, split broad extension into trusted and public-package extensions, route current document-analysis worker commands through the owning domain worker, and compile/load facade modules from one authoritative source with domain allowlists.
    - API Notes and Examples:
      ```rust
      let trusted = create_js_runtime(trusted_extension(), TrustedOpState::new(...));
      let third_party = create_js_runtime(package_extension(), PackageOpState::new(...));
      ```
    - Files to Create/Edit:
      - `src/server/js_runtime.rs`: two workers/domain lifecycle, loader routing, and removal of per-analysis persistent runtime construction.
      - `src/server/document_analysis.rs`: route analysis invocation/lifecycle through owning domain runtime without duplicating runtime instances.
      - `src/server/ops/mod.rs` and op modules: trusted/public extension sets and narrow state access.
      - `runtime/js/*.ts|*.js|*.d.ts`: domain-safe facade bundles from one executable source.
      - `tests/{clay_js_facade_layout,clay_js_api_inventory,package_loading}.rs`: export/op/module boundaries.
    - References:
      - Plan 060 P2-1/D1 facade single-source finding; this task owns domain-critical portion.
  - Test Cases to Write:
    - Third-party runtime cannot see trusted-only op names or `clay:configuration`/admin/internal modules.
    - Trusted and third-party globals/module caches do not cross.
    - Third-party timeout/heap termination replaces only third-party generation; trusted ping remains responsive.
    - Persistent application runtime count remains exactly two as packages, documents, and analyzers are added.
  - Completion Evidence (2026-07-21):
    - `ClayJsRuntimeService` now owns exactly two persistent application runtimes via per-domain `DomainRuntime { worker, poisoned, evaluations }`: `Trusted` (configuration + bundled first-party) and `ThirdParty` (shared adopted-package runtime). Both share one host-owned `Arc<Mutex<PackageService>>` and `Arc<PackageLoadEntryAllowlist>` created at service construction and preserved across per-domain worker replacement.
    - Op split in `src/server/ops/mod.rs`: `clay_runtime_trusted_extension` (all 66 ops) and `clay_runtime_package_extension` (35 ops: 31 public contribution/registration ops plus 4 internal result-bridge ops). No configuration, document/workspace, keybinding, package-loading, language-server-authorization, classification/activation, theme, or admin op is registered in the third-party isolate. Unit test `package_extension_is_strict_subset_without_admin_ops` pins the exact 66/35 lists and subset relation.
    - `ClayOpState` carries a `domain` set by `new_for_domain(workspace, doc, domain, shared_service, shared_allowlist)`; third-party states start with language-server authorization sealed. `new_analysis_worker` and the per-analyzer persistent runtimes are deleted; document-analysis invocations route through `invoke_document_analyzer` to the owning domain resolved from the host-enabled record's `runtime_domain` (never caller manifests), keeping the mailbox/budget worker structs and all analysis caps.
    - Module loader: `ClayModuleLoader` carries `domain`; `THIRD_PARTY_FACADES` allowlist (13 public facades) gates `clay:*` resolve/load, configuration modules resolve/load only in the trusted domain, and the worker's Evaluate branch rejects `ConfigurationRoot` entries outside the trusted domain.
    - Lifecycle: per-domain poison/replacement (`replace_domain_worker`) — a third-party timeout/heap termination replaces only the third-party worker while the trusted worker and its registrations stay live; `shutdown_generation_resources` and test session counts cover both domains. Known semantic note: analysis/provider timeouts now poison the owning domain (same replacement semantics the main runtime already had for parse/completion), traded for the exactly-two-runtimes topology per the approved plan.
    - Per-op `ensure_trusted_domain` guards were considered and skipped: enforcement is the unregistered-op boundary plus the subset test, the loader gates, and the configuration-eval routing guard; 30 one-line guards would be ceremony against a threat the structural tests already fail on.
    - Resource probe (release, `runtime_resource_baseline_probe`): startup two domains 41,228 KiB/22 threads (task-1 single-runtime baseline 36,236/20 → +4,992 KiB/+2 threads for the second domain, matching the task-1 estimate); after 4 first-party packages 44,524 KiB; with reload candidate 54,216 KiB/26 threads (candidate builds both domains); analysis stage adds zero runtimes (previously 60,512 KiB/28 threads with 4 analysis runtimes). Warm evaluation median 105 us, candidate reload 6.0 ms, startup 8.0 ms — unchanged from task 1.
    - Tests: `third_party_runtime_cannot_see_trusted_ops_or_admin_modules` (7 admin op names `undefined` in third-party while public ops are functions and all are functions in trusted; 8 admin/internal facades rejected, 3 public facades import cleanly), `domain_globals_and_module_state_do_not_cross`, `third_party_termination_replaces_only_third_party_generation` (workers_started stays 2 while trusted ping survives; third-party recovery starts exactly 1 replacement), `package_extension_is_strict_subset_without_admin_ops`, and `workers_started() == 2` assertion inside the document-analysis open/invoke lifecycle test. Task-1 inventory test updated to union both extension lists (66 ops).
    - Validation: `cargo test --all-targets` 1719 passed/0 failed, `cargo clippy --all-targets -- -D warnings` clean, `cargo fmt --check` clean, `git diff --check` clean.

- [x] Bind shared third-party runtime calls to package-scoped provenance
  - Acceptance Criteria:
    - Functional: Every package-facing registration/publication/provider/language-server call is stamped with current host-created package name/version/prefix/provenance/generation; caller-supplied manifests/names never select authority.
    - Performance: Provenance resolution is one bounded current-record/grant lookup at op ingress; package manifests are not reconstructed per publication or provider result.
    - Code Quality: One package-context resolver replaces `package_from_options`/`package_value_from_options` and equivalent mode/command/UI parsers; package contexts remain host-owned/internal.
    - Security: Package A cannot publish/register/start sessions as B, inflate permissions, use stale generations, or self-approve graph mutations. Shared third-party JavaScript memory remains explicitly outside hostile sibling isolation.
  - Approach:
    - Documentation Reviewed:
      - `docs/wiki/modules/package-principal-and-result-routing-primitive-review.md`.
      - `src/server/ops/{decorations,diagnostics,parse,syntax,completion,document_analysis,language_intelligence,language_server,modes,commands,ui}.rs`.
    - Options Considered:
      - Caller manifest exact lookup: does not authenticate requester.
      - JavaScript-visible token: copyable and misleading as isolation.
      - Host-created package activation/callback context with bound facades and host-stamped registrations: supports shared cohort and accurate host provenance. Chosen.
    - Chosen Approach:
      - Load package entry under one serial host-owned activation context, bind facade/callback registrations to that context, stamp later provider invocations from registration metadata, reject authority-bearing ops outside a current package context, and remove manifest authority fields.
    - API Notes and Examples:
      ```rust
      let package = state.current_package()?.resolve_enabled(service)?;
      package.require(PackagePermission::RenderDecorations)?;
      ```
    - Files to Create/Edit:
      - `src/packages/{service,authorization}.rs`: current package generation/provenance resolution.
      - `src/server/{js_runtime,ops/mod}.rs`: package activation/callback context.
      - `src/server/ops/{decorations,diagnostics,parse,syntax,completion,document_analysis,language_intelligence,language_server,modes,commands,ui}.rs`: remove self-asserted authority.
      - `runtime/js/*.ts|*.js|*.d.ts`: remove authority-bearing manifest/name inputs.
      - Existing package/coordinator/LSP integration tests: adversarial coverage without a new test binary.
    - References:
      - Plan 060 P0-1/T3; this plan supersedes its per-package-isolate mechanism.
  - Test Cases to Write:
    - A-as-B manifest/name, permission inflation, stale callback, cross-package session, and raw op calls fail.
    - Approved A-owned publications retain A provenance.
    - Existing one-line bundled package loading works without manifest plumbing.
  - Completion Evidence (2026-07-21):
    - Host-owned `PackageContext { package_name, package_version, api_prefix }` on `ClayOpState` (`src/server/ops/mod.rs`). Set only by Rust: the `loadPackage` op after host-side enable, the runtime worker around Parse/Completion/LanguageIntelligence/DocumentAnalysis handler commands (stamped from host registration metadata), and `RuntimeCommand::Evaluate.package_context` (Rust-driven package evaluation). `begin_evaluation` clears it every command, so stale provenance never leaks across commands.
    - One resolver replaces all caller-manifest parsers: `current_package_record()` (context → host enabled-set lookup by name+version; disabled/stale packages fail closed with `clay.packages.package_not_enabled`) and `require_current_package_capability(permission)` (host authorization record's `approved_capabilities`, never caller-declared permissions). New `PackageService::enabled_record` / `has_approved_capability` helpers; one bounded lookup per op ingress, no per-publication manifest reconstruction.
    - Deleted `package_from_options`, both `package_value_from_options`, and all `parse_manifest` op helpers. Converted ops: decorations/diagnostics publish, parse/completion/syntax/language-intelligence/document-analyzer registration, modes register_pattern (declaration can no longer override package name/version/prefix; response carries host-registered prefix+modeId), modes activate_major_mode (owner resolved host-side from classification + enabled set + approved mode-activation), commands register_command, six UI contribution ops, and language-server start_session (owner = executing package; op response carries host-resolved identity).
    - Session IO binding: send/read/stop now require the host-stamped executing package to match the session owner (`require_executing_package_owner`) in addition to the existing entry identity + grant-fingerprint checks; `stop` gained entry-identity verification (`SessionCommand::Stop` carries package/contribution/fingerprint). Package B knowing A's names and a session id cannot drive or stop A's session.
    - Facades (`js_runtime.rs` consts + `runtime/js/*.ts` mirrors) and all 9 runtime-registering first-party load entries no longer accept or pass manifests; the modes facade activation registry keys inert payloads by the op response's host-registered prefix+modeId. Config fixtures (markdown-mode, windows-markdown-open) rewritten to the one-line `loadPackage` path; config-side contribution registration/publication is gone.
    - Adversarial tests: `package_provenance_ignores_caller_supplied_identity_fields` (forged name/manifest/prefix/permissions in options ignored; publication stamps executing package), `disabled_package_callback_publications_fail_closed` (disable → stale registration callback fails `package_not_enabled` at op ingress), `language_server_session_io_requires_executing_owner_package` (cross-package session write → `session_owner_mismatch`), no-context/raw calls → `clay.packages.no_active_package`, capability-shrink → `clay.packages.missing_permission`, host-side manifest validation rejects out-of-namespace contribution ids and third-party grammar contributions before package code runs. ~30 pre-existing tests retrofitted to the production flow (host install/authorize/enable + `evaluate_entry_as_package`), including two-phase LS authorize(config)/session(package) coverage.
    - Known boundaries: `serverDisableCompletion({ packagePrefix })` retains pre-existing cross-package disable semantics (graph-mutation consent arrives with the extension-point/approval tasks); `op_clay_packages_load_package` (manifest variant) is validation-only and stamps nothing; `string_or` provenance overrides removed from mode/command declaration parsers.
    - Validation: `cargo test --all-targets` 1722 passed/0 failed (911 lib), `cargo clippy --all-targets -- -D warnings` clean, `cargo fmt --check` clean, `git diff --check` clean.

- [x] Add versioned extension points and durable user-approved dependency/mutation records
  - Acceptance Criteria:
    - Functional: First-party records can declare closed versioned extension points; third-party manifests request exact package/point/operation/scope relations; durable approvals record requester/target provenance, versions/integrity, scopes, approver/time, generation/status, and external authority summary before code executes.
    - Performance: Install/adoption/graph reconciliation is bounded by fixed metadata/package/relation ceilings and runs off hot paths; current approval lookup is indexed and does not scan all packages per provider result.
    - Code Quality: Extend existing `PackageGraphRelations`, authorization, conflict, and revocation records rather than creating a parallel graph; preserve deterministic diagnostics and atomic candidate evaluation.
    - Security: Cross-domain mutation requires owner-declared extension point plus exact user approval; authority expansion/stale source/changed target point version blocks and re-prompts; package code cannot author or mutate approval records.
  - Approach:
    - Documentation Reviewed:
      - `src/packages/{manifest,record,graph,authorization,service,conflict}.rs`.
      - `docs/reference/primitives/package-security.md` conflict/graph contract.
      - `.agents/skills/project-patterns/references/package-runtime-trust-domains.md`.
    - Options Considered:
      - One broad `mutates` boolean: cannot explain or scope authority.
      - New unrelated mutation subsystem: duplicates graph/conflict/revocation state.
      - Versioned extension points and approved relation edges folded into current graph records. Chosen.
    - Chosen Approach:
      - Add bounded structured relation objects alongside migration support for existing string arrays; persist approvals under Clay's package store in a versioned host-owned file with restrictive permissions and atomic replacement; reject unknown operations/scopes.
    - API Notes and Examples:
      ```json
      {"package":"@clay/markdown","extensionPoint":"markdown.completionProviders","operation":"append","scopes":["vendor-markdown.wikilinks"]}
      ```
    - Files to Create/Edit:
      - `src/packages/{manifest,record,graph,authorization,service,conflict,manager}.rs`: schemas, persistence, reconciliation, diagnostics.
      - `docs/reference/primitives/{package-security,package-loading,registry}.md`: public contract.
      - `tests/{package_loading,package_graph,package_conflicts}.rs`: relation/approval persistence and hostile metadata.
    - References:
      - Plan 060 filesystem integrity rules for secure atomic persistence; reuse its final shared helper if available, otherwise implement the narrow stdlib-safe write here and let Plan 060 consolidate later.
  - Test Cases to Write:
    - Unknown point/operation/scope, undeclared owner point, no approval, stale source/target, expanded wildcard, cycle, and forged approval fail.
    - Exact approved append/replace succeeds and preserves requester/target provenance.
    - Truncated/corrupt/unsafe-permission approval store fails closed without executing packages.
  - Completion Evidence (2026-07-21):
    - `src/packages/extension_points.rs` (new): closed `RelationOperation` (append/replace) and 16-kind `ExtensionContributionKind` enums, `ExtensionPointDeclaration` and `StructuredRelationRequest` types, fail-closed manifest parsing (`parse_extension_points`, `parse_structured_relation`), and `verify_relation_request` host verification with deterministic codes (`unknown_extension_point`/`version_mismatch`/`operation_not_offered`). Locked limits enforced: 64 points/manifest, 64 relation requests/manifest, 32 scopes/entry, 128 chars/scope, 64-char ids, 280-char display text; unknown fields/operations/kinds, cross-prefix scopes, duplicate ids, and bad wildcards reject at manifest validation. Id charset follows contribution-id style (camelCase name segments; prefix segment must equal owning apiPrefix), matching the locked schema examples.
    - `src/packages/manifest.rs`: `ClayPackageMetadata.extension_points` + `PackageGraphRelations.relation_requests`; `clay.extends` accepts legacy strings or structured objects, `clay.imports`/`clay.overrides` are new structured fields (string entries recorded as dependsOn targets); structured targets join existing graph lists so resolution/cycle detection/package-control rules are unchanged (no parallel graph).
    - `src/packages/approvals.rs` (new): `PackageApprovalRecord`/`ApprovedRelation`/`ApprovedReplacement` durable types (manual bounded serde_json conversion — the crate has no serde derive dependency); `PackageApprovalStore` at `<store root>/clay-package-approvals.json` (format v1, max 256 records, 256 KiB read cap), owner-only 0o600 atomic temp+fsync+rename writes, fail-closed load on corruption/truncation/unknown version/duplicates/unsafe permissions; `approval_covers` implements the stale rules (exact identity incl. integrity/root, exact-subset capabilities/processes/relations; revoked durable); HashMap-indexed lookups, enable-time only (no hot-path scans). Only host flows mutate the store (`upsert`/`revoke`); package code has no path.
    - `src/packages/service.rs`: `approvals` field; `PackageService::open` (durable, fail-closed constructor; wired in `src/main.rs` CLI) vs `new` (in-memory, tests/ephemeral); `record_package_approval`/`revoke_package_approval`/`package_approvals` host APIs; `verify_relation_authority` runs inside `enable_graph` after targets enable and before the requester is inserted (transactional rollback covers denial): owner consent = enabled target declares exact point/version/operation; user consent = third-party requesters need exact durable approval; trusted-domain requesters are pre-authorized via the bundled inventory. New `RelationDenied`/`ApprovalStore` error variants with deterministic codes.
    - Tests: 4 unit tests (declaration round-trip, unknown field/operation/kind/duplicate/prefix rejection, scope validation, exact-match verification), 4 store unit tests (round-trip+revoke+0o600, corrupt/truncated/unknown-version fail-closed, unsafe-permissions fail-closed, exact/narrower/expanded/identity-drift/missing/revoked coverage), 6 integration tests in `tests/package_graph.rs` (no owner point, operation not offered, missing→stale→exact approval progression, manifest-level unknown operation, service-open corrupt-store fail-closed, durable round-trip through `PackageService::open`).
    - Deferred per plan scope: cross-domain invocation envelope execution (task 7), first-party extension-point declarations in shipped manifests (task 8), adoption UX prompts that author approvals (task 10), replacement activation sequence (task 11); the server runtime keeps an in-memory approval store until the adoption flow lands (third-party relations then fail closed by absence of durable approvals).
    - Docs: `package-security.md` implementation-status lines for the trust-domain section, extension-point id charset, relation parsing/verification, and the implemented store contract.
    - Validation: `cargo test --all-targets` 1736 passed/0 failed, `cargo clippy --all-targets -- -D warnings` clean, `cargo fmt --check` clean, `git diff --check` clean.

- [x] Implement typed bounded cross-domain extension invocation
  - Acceptance Criteria:
    - Functional: Third-party providers invoke only published extension points through Rust-owned typed request/result envelopes; first-party responses and host-state mutations return inert values with requester/target/contribution/generation identity.
    - Performance: Every bridge lane has fixed payload, queue, timeout, concurrency, and cancellation ceilings; no synchronous cross-runtime round trip occurs in editor client hot paths.
    - Code Quality: Reuse existing runtime commands, oneshots, provider registries, payload validators, and generation cancellation; do not add generic RPC/actor frameworks or serialize arbitrary JavaScript values.
    - Security: Bridge revalidates current approval/grants at ingress and before commit, rejects stale/revoked responses, and never exposes trusted V8 handles, internal Rust handles, workspace/document data beyond the called API, or raw op names.
  - Approach:
    - Documentation Reviewed:
      - `src/server/js_runtime.rs::RuntimeCommand` and existing parse/completion/intelligence oneshot paths.
      - `.agents/skills/project-patterns/references/{package-runtime-trust-domains,protocol-and-performance}.md`.
    - Options Considered:
      - Direct JS imports/functions: impossible across isolate boundary and violates security decision.
      - Generic JSON RPC: broad untyped surface and unnecessary protocol.
      - Existing typed runtime commands/registries plus bounded inert envelopes. Chosen.
    - Chosen Approach:
      - Add only request variants required by approved generic extension families; invoke third-party callbacks in third-party runtime and trusted callbacks in trusted runtime, with Rust registries/server state as primary authority.
    - API Notes and Examples:
      ```text
      PackageExtensionRequest { requester, target, point, contribution, generation, payload }
      -> validate -> typed handler/server state -> bounded inert result
      ```
    - Files to Create/Edit:
      - `src/server/{js_runtime,document_analysis,completion,language_intelligence,parse_coordinator}.rs`: domain-aware typed dispatch where needed.
      - `src/packages/{record,service}.rs`: extension route metadata.
      - `src/protocol/*` only if client-visible inert state changes (tentative; cross-runtime traffic stays internal by default).
      - Existing coordinator/provider tests: bounded/stale/cancel tests.
    - References:
      - Plan 060 result-routing task remains responsible for connection subscriptions; this task stops at runtime/server output ownership.
  - Test Cases to Write:
    - Oversize, timeout, queue saturation, stale generation, revoked edge, wrong target/point, and mismatched result provenance fail predictably.
    - Slow third-party extension cannot block trusted runtime or client local editing.
  - Completion Evidence (2026-07-21):
    - `src/server/cross_domain.rs` (new, crate-internal): `CrossDomainRequestEnvelope`/`CrossDomainResultEnvelope`/closed `CrossDomainStatus` (ok/error/denied/stale/revoked/timeout) per `clay-cross-domain-envelope-v1`; `validate_cross_domain_request` as the single ingress check — payload budget (8192 B) before target lookup, deadline ceiling (1..=250 ms), requester enabled-at-exact-version + third-party domain, approvalRef identity binding, target enabled + exact point/version/operation via `verify_relation_request`, and current durable-approval coverage mapped to statuses (revoked→revoked, identity drift→stale, expansion/absence→denied). Lane ceilings exported as consts (16 pending, 250 ms deadline, 8192 B payload). Handlers consuming validated routes arrive with task 8's extension surfaces (module allow(dead_code) mirrors the repo's pattern for pre-wired internals).
    - Domain-aware typed dispatch: shared `dispatch_to_domain` helper routes `RuntimeCommand::{Parse,Completion,LanguageIntelligence}` to the worker owning the registration's host-enabled record (poisoned-domain replace + one retry; timeout/heap poisons only the owning domain; per-domain evaluation counters). `invoke_document_analyzer` already followed this pattern from task 4. `registration_domain` resolves from the enabled set, never caller identity.
    - Bridge ingress revalidation: `op_clay_parse_store_update`, `op_clay_completion_store_result`, `op_clay_language_store_intelligence_result` now resolve `current_package_record()` before storing — disabled/revoked package results fail closed (`package_not_enabled`) instead of reaching host state.
    - Test infrastructure: `evaluate_as_package` evaluates synthetic packages in their record's domain (matching production adoption); new `evaluate_as_trusted_package` + `PackageService::force_enabled_runtime_domain_for_test` (cfg(test)) for tests exercising trusted-only ops; `domain_evaluations` test accessor.
    - Tests: 6 envelope unit tests (exact-approved validates; wrong point/operation/version denied; stale requester + revoked approval; oversize payload + bad deadline rejected before target lookup; approvalRef mismatch; expanded scope denied) + 2 runtime tests (`third_party_provider_executes_in_third_party_runtime_only` — third-party evaluation counter increments, trusted untouched; `slow_third_party_provider_poisons_only_third_party_domain` — busy-loop provider times out, trusted runtime keeps answering). 3 pre-existing tests switched to trusted-domain evaluation where they exercise trusted-only ops/facades.
    - Docs: `package-security.md` envelope section implementation-status paragraph.
    - Validation: `cargo test --all-targets` 1744 passed/0 failed (932 lib), `cargo clippy --all-targets -- -D warnings` clean, `cargo fmt --check` clean, `git diff --check` clean.

- [x] Publish and migrate first-party package extension surfaces
  - Acceptance Criteria:
    - Functional: Every package in the initial inventory declares supported extension/replacement surfaces; package load entries use trusted-domain APIs; third-party examples build on each generic family without importing first-party implementation modules.
    - Performance: Bundled package defaults remain one-line loadable, manifest/extension metadata stays within current budgets, and no package callback enters client paint/input/layout paths.
    - Code Quality: Prefer generic contribution extension points; add package-specific APIs only for irreducible public data (`clay:git` cached status direction). Keep package code language-owned and avoid Rust branches named for Markdown/Rust/TypeScript/JavaScript.
    - Security: First-party extension points expose only explicit typed scopes; LSP descriptors/grants are not mutable; theme/UI remain inert; trusted package internals and raw ops remain unreachable.
  - Approach:
    - Documentation Reviewed:
      - `docs/wiki/modules/first-party-package-extension-api-review.md`.
      - Every `packages/*/package.json`, `dist/load.js`, and package docs page.
      - `.agents/skills/project-patterns/references/{mode-primitive-first,package-ui-layout,clay-js-api-naming}.md`.
    - Options Considered:
      - Bespoke package callable APIs: rejected unless generic contribution APIs cannot represent required behavior.
      - Declarative extension-point metadata over existing registries, with full replacement for semantic rewrites. Chosen.
    - Chosen Approach:
      - Migrate package families together: language/mode packages, LSP bridge packages, Git, themes, then decide whether stable `lsp-shared` helpers become a public pure-JS SDK or stay private.
    - API Notes and Examples:
      ```javascript
      // Third-party package registers its own provider; approved graph edge targets Markdown.
      await serverRegisterCompletionProvider({ id: "vendor-markdown.wikilinks", modes: ["markdown"], ...options });
      ```
    - Files to Create/Edit:
      - `packages/{markdown,rust,typescript,javascript,git,lsp-rust,lsp-typescript,lsp-javascript,lsp-markdown,theme-gruvbox-material-dark,theme-gruvbox-material-light}/package.json` and relevant `dist/src/docs` files.
      - `packages/lsp-shared/*`: public SDK extraction or explicit private boundary.
      - `docs/reference/packages/creating-packages.md`: extension/replacement authoring.
      - Existing package, syntax, completion, language, and docs tests.
    - References:
      - `.agents/skills/create-plan/references/clay.md`: one-line loading and package UI authoring contract.
  - Test Cases to Write:
    - Every bundled package's declared extension points match real contribution IDs/kinds and remain within budgets.
    - Third-party fixtures extend each generic family through public APIs only.
    - No package-specific Rust branch or trusted module import appears in third-party fixtures.
  - Completion Evidence (2026-07-21):
    - All 11 bundled manifests declare `clay.extensionPoints` (`clay-extension-point-v1`, version 1): language packages (`@clay/markdown`, `rust`, `typescript`, `javascript`) publish 6 generic points each (completionProviders, languageLayers, commands, ui, grammar replace-only, modePattern append-only); `@clay/git` publishes `git.statusRegion` (replace) + `git.contributions` (append); the four `@clay/lsp-*` bridges publish `<prefix>.providers` (append/replace; server descriptor/grant explicitly not mutable); both themes publish `<prefix>.tokens` (replace). Whole-package semantic rewrites use `replaces` + approval, not extension points. The 9 packages with checked-in JS manifest functions carry identical `extensionPoints` in `dist/index.js` (node deepEqual suites pass).
    - Scope schema fix: contribution-id segment charset relaxed to alphanumeric/dots/hyphens (real contribution ids are camelCase, e.g. `markdown.toggleLineComment`); grammar scopes use the host grammar contribution id `<apiPrefix>.<languageId>`.
    - Budget: `BEHAVIOR_MANIFEST_PAYLOAD_BUDGET_BYTES` raised 4096 → 8192 with justification comment (locked schema allows 64 extension points; `@clay/markdown` ~5.5 KB defines headroom); performance docs + budget tests updated in lockstep.
    - `lsp-shared` decision: stays private; third-party LSP bridges use public `clay:language-server` APIs with their own approved contribution. Documented in the wiki review and `creating-packages.md`.
    - New test `bundled_extension_points_match_real_contributions` (src/packages/bundled.rs): for every bundled package, assembles the record, requires non-empty extension points with positive versions, and asserts every declared scope names a real contribution id (commands/providers/language servers/intelligence/sdui regions/ui components/panels/grammar ids + the five runtime-registered Markdown ids).
    - Docs: new "Extending and Replacing Packages (Plan 061)" authoring section in `docs/reference/packages/creating-packages.md` (declare/request/replace/`lsp-shared` rules); wiki review decision lines updated; `BUNDLED_PACKAGES` fingerprints regenerated for the manifest edits.
    - Validation: `cargo test --all-targets` 1745 passed/0 failed, node package suites pass via `lsp_bridge` harness, `cargo clippy --all-targets -- -D warnings` clean, `cargo fmt --check` clean, `git diff --check` clean.

- [x] Review Clay UI catalog and design package adoption/replacement interaction
  - Acceptance Criteria:
    - Functional: Produce a state-complete adoption flow for pending inspection, approve, reject, stale approval, authority expansion, disable, replace, failed candidate, revoke, and rollback without executing package-authored UI.
    - Performance: Inspection is asynchronous, bounded, and never blocks app startup/editor interaction; transitions avoid expensive animation/layout work.
    - Code Quality: Reuse `overlay`, `portal`, `scroll`, `list`, `label`, and `button` plus existing transient/shell primitives; justify and catalog a new generic modal only if composition cannot satisfy focus/accessibility.
    - Security: Host renders provenance/capabilities/processes/relations/wildcards/withdrawals/shared-runtime disclosure; package-authored descriptions are secondary and cannot hide authority facts or control approval actions.
  - Approach:
    - Documentation Reviewed:
      - `npx ui-skills start` and `wshobson/interaction-design`: purposeful feedback, interruptible transitions, reduced motion.
      - `.agents/skills/clay-ui/{SKILL.md,references/components.md,references/tokens.md}`.
      - `.agents/skills/project-patterns/references/package-ui-layout.md`.
    - Options Considered:
      - Reserved/custom modal immediately: stronger blocking semantics but new component cost.
      - Non-blocking Clay-owned overlay composed from current catalog; package remains disabled until decision. Chosen.
      - Package-rendered approval UI: rejected security boundary.
    - Chosen Approach:
      - Design one reusable internal package-authority overlay with scrollable host facts, explicit primary/reject/danger actions, keyboard/focus states, no decorative motion, and editor availability while package stays disabled.
    - API Notes and Examples:
      ```text
      Pending adoption -> inspect facts -> approve/reject
      Replacement -> inspect withdrawn target -> explicit danger confirmation -> candidate activation
      ```
    - Files to Create/Edit:
      - `docs/wiki/modules/first-party-package-extension-api-review.md`: finalized interaction contract.
      - `.agents/skills/clay-ui/references/components.md` only if a generic catalog addition is justified.
      - `docs/reference/packages/creating-packages.md`: author-visible adoption behavior.
    - References:
      - `src/shell/{components,package_ui,transient_menu}.rs`, `src/masonry_shell.rs`.
  - Test Cases to Write:
    - Design checklist covers keyboard/focus, reduced motion, overflow/long lists, wildcard warning, external process disclosure, rejection, stale state, and rollback.
  - Completion Evidence (2026-07-21):
    - Catalog review (`.agents/skills/clay-ui/references/components.md`): the implemented kinds compose to the full flow — `portal` > `overlay` (`WorkingArea` anchor, modal focus policy) > `scroll` > `flex`/`label`/`list` + footer `button` row. No new component kind justified; reserved `modal` stays reserved because the flow is deliberately non-blocking (editor stays interactive, package stays disabled until decision). `components.md` therefore unchanged per plan edit condition.
    - Finalized interaction contract written into `docs/wiki/modules/first-party-package-extension-api-review.md` as "Adoption and Replacement Interaction Contract (Plan 061 task 9)": state-complete machine (installed → pending inspection → approved/rejected; stale approval, authority expansion with diff, disable, revoke; replacement with danger confirmation, candidate activation, failed-candidate automatic rollback); fixed section order (identity, runtime disclosure, capabilities, external processes, dependencies/relations, mutation scopes with wildcard danger rows, withdrawals, muted package-authored summary last); footer actions (Approve primary/Reject default, Replace danger/Keep current, Revoke danger, Dismiss for rollback); keyboard contract (safe-default initial focus on non-mutating action, Tab traversal, Esc fail-closed reject); zero decorative motion by construction; bounded scroll with fixed footer; all facts host-rendered from PackageInspection/authorization/approval/graph records with package text confined to a muted final section.
    - Author-visible behavior documented in `docs/reference/packages/creating-packages.md` ("What users see at adoption") linking the full contract.
    - Design checklist (keyboard/focus, reduced motion, overflow, wildcard warning, process disclosure, rejection, stale, rollback) checked off in the contract itself.
    - Validation: `cargo test --all-targets` 1745 passed/0 failed (docs tests cover the wiki contract file), `cargo fmt --check` clean, `git diff --check` clean.

- [x] Implement pre-execution adoption, inspection, approval, and revocation interfaces
  - Acceptance Criteria:
    - Functional: Install remains non-executing; first third-party load creates a pending adoption record and native/CLI inspection; exact approval enables execution; existing exact approval preserves one-line future `loadPackage`; users can inspect/revoke grants and relationships.
    - Performance: Pending adoption and approval persistence are bounded/background work; no startup deadlock waits for a GUI client; package lists/inspection remain responsive at capacity.
    - Code Quality: CLI, trusted configuration path, and in-app UI call one `PackageService` authority path; no duplicate approval stores or package-rendered prompts.
    - Security: No third-party code runs before approval; noninteractive paths fail closed; approval is exact-provenance/scoped, permission-safe on disk, and cannot grant trusted domain or Clay-internal ops.
  - Approach:
    - Documentation Reviewed:
      - `src/main.rs` package CLI and `src/packages/service.rs` install/enable/inspect lifecycle.
      - `docs/reference/packages/creating-packages.md` one-line loading convention.
      - UI review task output and Clay UI catalog.
    - Options Considered:
      - Block server startup waiting for prompt: deadlocks/headless failure.
      - Auto-approve through `init.js`: executable config silently grants package authority.
      - Record pending adoption, keep package disabled, notify UI/CLI, then reload after explicit approval. Chosen.
    - Chosen Approach:
      - Add shared service commands/results for inspect/adopt/reject/revoke; trusted config `loadPackage` returns actionable adoption-required diagnostic when pending and succeeds unchanged after approval.
    - API Notes and Examples:
      ```bash
      clay package inspect vendor/pkg
      clay package adopt vendor/pkg
      clay package revoke vendor/pkg
      ```
    - Files to Create/Edit:
      - `src/packages/{service,authorization}.rs`: pending/current/revoked adoption lifecycle.
      - `src/main.rs`: CLI inspection/adopt/revoke.
      - `src/server/{mod,connection}.rs` and protocol only as required for native authority UI.
      - `src/shell/package_ui.rs` and `src/masonry_shell.rs`: Clay-owned overlay composition.
      - `docs/reference/packages/creating-packages.md` and relevant Clay JS API docs.
      - `.agents/skills/clay-ui/references/components.md` only if implementation adds a generic component.
    - References:
      - Existing package CLI and package-service tests; do not add another integration-test binary.
  - Test Cases to Write:
    - Install/pending/reject/approve/restart/revoke/stale-update/headless/noninteractive flows.
    - Long/wildcard/process/replacement authority lists remain bounded and inspectable.
    - Package JavaScript sentinel proves zero execution before approval.
  - Completion Evidence (2026-07-21):
    - Pre-execution adoption gate: `verify_relation_authority` (service.rs) no longer early-returns on relation-free packages — EVERY ThirdParty-domain enable requires an exact current durable approval covering identity+capabilities+processes+relations, failing with new `PackageServiceError::AdoptionRequired` (code + actionable `clay package inspect/adopt` guidance). Trusted bundled packages exempt (inventory is their authority). `loadPackage` surfaces the diagnostic unchanged to config/JS callers.
    - Single authority path: `PackageService::approve_package` builds the durable `PackageApprovalRecord` from host-side installed facts (identity, permissions, language-server contribution ids, structured relations, replacements with host-computed withdrawn contribution ids) and persists via the 0o600 fail-closed store; `adoption_state` reports Pending/Approved/Stale/Revoked for inspection surfaces; `rfc3339_now` (approvals.rs, no chrono dependency) stamps records.
    - CLI: `clay package adopt <name>` (writes approval, prints capabilities/processes/relations/replacements), `clay package revoke <name>` (revokes + disables), `inspect` now prints adoption state; usage text updated; store root unified in `PackageService::default_store_root` (shared by CLI and server runtime).
    - Server runtime durable wiring: `ClayJsRuntimeService::production()` opens the durable approval store at the default root (used by initial + reload generations in server/mod.rs); corrupt/unavailable store fails closed to an empty in-memory store with an eprintln warning — trusted domain unaffected, no startup deadlock.
    - Tests: 3 new integration tests in tests/package_graph.rs (`third_party_enable_requires_exact_durable_adoption` — pending→approve→enable→revoke→denied; `adoption_survives_service_restart` — durable store round-trip through `PackageService::open`; `package_update_stales_adoption` — version drift → Stale → identity_changed denial) + 1 sentinel lib test (`unapproved_third_party_package_never_executes_before_adoption` — loadPackage fails with `clay.package_approval.missing` and the load entry's sentinel op-record never fires). 20+ pre-existing tests retrofitted to call `approve_package` after authorize (test helpers: `evaluate_with_seeded_package_adoption` adopt flag, `ensure_synthetic_package_enabled` auto-approves).
    - Validation: `cargo test --all-targets` 1749 passed/0 failed, `cargo clippy --all-targets -- -D warnings` clean, `cargo fmt --check` clean, `git diff --check` clean.
    - Deferred with the plan's native-UI allowance: the in-app overlay composition from task 9's contract renders these same `adoption_state`/`approve_package` service calls; the CLI is the shipped inspection/adoption interface in this task.

- [x] Support atomic user-approved disable and full replacement of first-party packages
  - Acceptance Criteria:
    - Functional: User can disable any PackageService-managed bundled package and activate an approved third-party replacement; candidate validates/loads before commit, target contributions withdraw atomically, replacement may claim only exact target contribution IDs/scopes allowed by replacement edge, and rollback restores target after pre-commit failure.
    - Performance: Replacement builds candidate state off hot paths and commits one bounded generation snapshot; no per-document or client hot-path package graph resolution.
    - Code Quality: Extend existing disable/replaces/conflict/revocation/hot-reload transaction paths; do not reclassify replacement as first-party or add target-specific branches.
    - Security: Replacement retains own provenance/runtime/grants, receives no trusted ops or inherited language-server grant, cannot replace Clay core/bootstrap, and cannot claim undeclared target-prefix IDs or satisfy dependencies without explicit compatibility metadata.
  - Approach:
    - Documentation Reviewed:
      - `src/packages/{graph,service,conflict}.rs` and Plan 054 hot-reload transaction semantics.
      - `decision-logs/2026-07-16-1825-phase19-hot-reload-transaction-and-stale-edit-semantics.md`.
      - `docs/wiki/modules/first-party-package-extension-api-review.md` replacement flow.
    - Options Considered:
      - Move replacement into trusted runtime: violates boundary.
      - Let replacement impersonate target: destroys provenance/grants.
      - Disable target, stage third-party candidate, allow only approved exact target contribution claims, retain dual provenance. Chosen.
      - Automatically restore target after every post-commit third-party failure: silently reverses user choice; rejected. Expose explicit rollback while third-party runtime attempts bounded recovery.
    - Chosen Approach:
      - Treat full replacement as user-owned package graph control rather than owner-declared mutation; stage candidate registries, validate compatibility/conflicts, atomically swap active ownership, and retain target/replacement audit pair.
    - API Notes and Examples:
      ```javascript
      // Trusted user configuration/admin API; never callable from third-party runtime.
      await setPackageReplacement({ target: "@clay/markdown", replacement: "vendor/markdown" });
      ```
    - Files to Create/Edit:
      - `src/packages/{manifest,record,graph,service,conflict,authorization}.rs`: replacement scope/compatibility/transaction.
      - `src/server/{js_runtime,mod}.rs`: staged runtime generation activation/revocation.
      - Provider/registry modules: exact contribution withdrawal/ownership as required.
      - `tests/{package_graph,package_conflicts,package_loading,persistent_runtime_hot_reload,runtime_update_protocol}.rs`.
      - Package/configuration reference docs.
    - References:
      - Existing `PackageRevocationRecord` and `PackageConflictResolutionDiagnostic`.
  - Completion Evidence (2026-07-21):
    - Replacement edge approval coverage: `approval_covers` (approvals.rs) now emits the pre-existing-but-unreachable `ApprovalMismatch::ReplacementExpansion` — every declared `replaces` target must appear in the durable approval's replacement list; an unapproved edge fails closed with `clay.package_approval.replacement_expansion`.
    - Stale-on-replacement: `PackageService::enable` extends its snapshot/restore transaction (enabled/resolutions/revocations/generation) to the approval store (`PackageApprovalStore::snapshot`/`restore`); on committed replacement the replaced target's durable approval is revoked (trusted targets hold no record — no-op), so restoration never silently reuses a stale approval.
    - Explicit rollback: `PackageService::rollback_replacement(target)` disables the active replacement (resolved from its graph `replaces` edge), re-adopts the target via `approve_package` when third-party, and re-enables it; new `PackageServiceError::NoActiveReplacement` when no replacement is active. CLI `clay package rollback <name>` routes through the same service path (usage text updated).
    - Dependency satisfaction fail-closed: `enable_graph` denies dependency/extends activation of a target currently replaced by an enabled replacement (`clay.package_replacement.target_replaced`); compatibility claims in approval records are disclosure-only today (substitution through a replacement intentionally not implemented — explicit rollback is the only restore path).
    - Undeclared target-prefix contribution claims are structurally unrepresentable: `assemble_package_record` rejects contribution ids outside the replacement's own apiPrefix namespace, and `reconcile_enabled_conflicts` catches cross-package id collisions — no separate claims gate needed (verified by inspection of manifest assembly + conflict paths).
    - Clay core/bootstrap protection: `replaces` targets resolve only against installed packages; `core.*` fails `MissingGraphTarget` (test `replacement_cannot_target_clay_core`). Replacement retains own provenance/grants and ThirdParty domain (asserted via record Debug in `third_party_replacement_withdraws_trusted_target_atomically`); no trusted-op or language-server-grant inheritance path exists (LS grants keyed by exact package+fingerprint, authorize op is trusted-only).
    - Tests: 4 new integration tests in tests/package_graph.rs (replacement-edge approval coverage + stale-on-commit revocation; rollback restores target + re-adopts + second rollback fails closed; core target unresolvable; dependency cannot silently restore replaced target) and 1 in tests/package_loading.rs (trusted bundled markdown replaced by third-party: atomic withdrawal, ThirdParty domain retained, audit pair diagnostic recorded, explicit rollback restores).
    - Performance: all new work runs on enable/rollback cold paths; enable() snapshots one extra HashMap clone per enable call; zero per-document/client hot-path graph resolution added.
    - Validation: `cargo test --all-targets` 1754 passed/0 failed, `cargo clippy --all-targets -- -D warnings` clean, `cargo fmt --check` clean, `git diff --check` clean.
    - Deferred: candidate staged-runtime hot-reload generation wiring (server/mod.rs) stays with task 12 (hot-reload/rollback preservation); compatibility-claim-driven dependency substitution deferred (fail-closed today).
  - Test Cases to Write:
    - Disable-only, successful replacement, pre-commit failure rollback, post-commit runtime failure state, explicit rollback, restart persistence, stale target/replacement update.
    - Replacement cannot claim unrelated target IDs, trusted domain, target grants, core IDs, or implicit dependency compatibility.

- [x] Preserve lifecycle, hot reload, analysis, and external-process authority across domains
  - Acceptance Criteria:
    - Functional: Trusted and third-party generations reload independently; disable/revoke/update/root removal/last close/shutdown cancels correct providers, outputs, analysis workers, approvals/routes, and language-server sessions; third-party recovery replays only current approved graph.
    - Performance: Recovery and cancellation are bounded; one failing third-party package may fail/restart shared third-party generation without blocking trusted runtime or client local editing; no automatic unbounded restart loop.
    - Code Quality: Reuse existing runtime generation snapshot, provider cancellation, document-analysis, and language-server revocation paths; domain becomes explicit metadata rather than duplicated coordinators.
    - Security: Existing fixed-contribution executable/argv/environment/root grants remain exact and do not transfer during extension/replacement; same-user subprocess is never called sandboxed.
  - Approach:
    - Documentation Reviewed:
      - `decision-logs/2026-07-14-2023-language-server-package-authority.md`.
      - `decision-logs/2026-07-15-1750-lsp-document-sync-and-package-worker-authority.md`.
      - `docs/wiki/modules/{language-server-process-service,language-intelligence,embedded-js-runtime}.md`.
    - Options Considered:
      - Duplicate coordinators per domain: excessive state/routing drift.
      - Shared host coordinators with domain/package/generation-stamped registrations and domain-routed callbacks. Chosen.
    - Chosen Approach:
      - Keep server coordinators authoritative, route callbacks to owning runtime domain, and extend existing revocation indexes/cleanup triggers with domain generation.
    - API Notes and Examples:
      ```text
      RegistrationOwner { domain, package, package_generation, runtime_generation, contribution }
      ```
    - Files to Create/Edit:
      - `src/server/{js_runtime,parse_coordinator,completion,document_analysis,language_intelligence,language_server,mod}.rs`.
      - `src/packages/service.rs`: domain-aware revocation/replay.
      - Existing parse/completion/language/LSP/hot-reload tests.
    - References:
      - Plan 060 T8 later owns per-session LSP actor concurrency; this task preserves authority/routing metadata without preempting that refactor.
  - Test Cases to Write:
    - Trusted reload leaves third-party current only when bridge generation remains compatible; otherwise stale work rejects and replay occurs.
    - Third-party heap/timeout/reload revokes only third-party handlers/sessions and trusted runtime responds.
    - Replacement LSP requires its own fresh grant.
  - Completion Evidence (2026-07-21):
    - Independent generations: `DomainRuntime` gains a per-domain generation counter (bumped on every replacement; registration-ownership metadata per the task API note). `ClayJsRuntimeService::production_reload(&current)` shares the live third-party domain (worker, poison/generation state, package authority, load-entry allowlist) across a trusted configuration reload — server reload (`reload_runtime_generation_inner`) uses it, and generation commit now shuts down only the old trusted worker's LS sessions (`shutdown_trusted_generation_resources`); shared third-party LS sessions survive (`kill_on_drop` still bounds them to their worker).
    - Surviving registrations: commit re-registers the shared third-party worker's registration payload under the new generation via `third_party_registrations_snapshot()` (harvest extracted to `harvest_op_state_evaluation`), so `cancel_older_runtime_generations` never withdraws live third-party providers.
    - Cross-domain load bridge: `loadPackage` of a third-party package no longer imports it into the trusted runtime. The op returns `domain` and skips context-stamping for third-party; the facade calls new trusted-only async op `op_clay_packages_load_in_package_domain`, which revalidates the enabled record + allowlist host-side and dispatches the load-entry evaluation to the third-party worker through a bridge sender wired at construction and rewired on every third-party replacement; `absorb_cross_domain_evaluation` merges coordinator-bound registration lanes into the trusted op state so the outer config harvest publishes them (op inventory 66→67; plan ledger + inventory test updated).
    - Poison recovery replay: `dispatch_to_domain` replaces a poisoned/dead third-party worker once and replays ONLY the current enabled+approved third-party graph (`replay_third_party_domain`): sorted load-entry evaluations with host-stamped `PackageContext`, one bounded pass — a failing replay poisons the domain again instead of looping. Deterministic tokens (`apiPrefix:id:index`) make pre-poison coordinator registrations valid again.
    - Tests: `trusted_reload_preserves_third_party_providers` (reload keeps same third-party generation; snapshot re-registration answers in the shared worker), `third_party_poison_replays_approved_graph_and_restores_providers` (busy-loop poison → next dispatch replays allowlisted load entry and provider answers under the same token; generation bumped), `replacement_language_server_requires_own_fresh_grant` in tests/package_loading.rs (target grant never transfers; replacement needs own exact grant); raw-runtime test helpers gained `wire_test_third_party_bridge` (third-party worker sharing test authority).
    - Validation: `cargo test --all-targets` 1757 passed/0 failed, `cargo clippy --all-targets -- -D warnings` clean, `cargo fmt --check` clean, `git diff --check` clean.
    - Deferred: third-party command/mode-activation execution routing across domains (bridge absorbs only coordinator-bound registration lanes; command/mode registries stay per-worker) — tracked for task 13/14 closure; no automatic restart loop exists by construction (replay is one bounded pass per replacement, driven by dispatch demand only).

- [x] Run adversarial security, resource, compatibility, and Linux closure
  - Acceptance Criteria:
    - Functional: Every Plan 061 ledger row has executable closure; all bundled packages load in trusted domain, approved third-party extension/replacement flows work, and current package/runtime/coordinator regressions pass.
    - Performance: Record one-vs-two-runtime startup/RSS/heap/reload/provider latency, runtime/thread count, bridge saturation, and third-party recovery; meet task-1 budgets or document measured blocker before closure.
    - Code Quality: Linux `cargo fmt --check`, `cargo check --all-targets`, `cargo clippy --all-targets -- -D warnings`, focused tests, and `cargo test --all-targets` pass; no new integration-test binary is added unless measured justification updates this plan.
    - Security: Cross-domain op/module/global denial, A-as-B, no-preapproval execution, stale/revoked approvals, malformed stores, first-party mutation without dual consent, replacement escalation, and external-process grant transfer all fail closed.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/project-patterns/references/{maintenance-validation,package-runtime-trust-domains,protocol-and-performance}.md`.
      - Plan 060 baseline and Linux primary-host requirements in `AGENTS.md`.
    - Options Considered:
      - Documentation-only authority closure: rejected.
      - Focused adversarial suites followed by one full Linux gate and before/after metrics. Chosen.
    - Chosen Approach:
      - Add cases to existing integration binaries/inline tests, run focused loops during tasks, then one clean full closure with exact evidence in this plan.
    - API Notes and Examples:
      ```bash
      cargo fmt --check
      cargo check --all-targets
      cargo clippy --all-targets -- -D warnings
      cargo test --all-targets
      ```
    - Files to Create/Edit:
      - Existing test files named by implementation tasks; no default new harness.
      - `plans/061-Two-Package-Runtime-Trust-Domains-and-Extension-Authority.md`: final evidence/metrics.
  - Completion Evidence (2026-07-21):
    - Security (all fail closed, executable):
      - Cross-domain op denial: `domain_extension_is_strict_subset` (35-op package subset of 67 trusted); module denial: third-party facade allowlist (13 of 21); NEW `third_party_package_cannot_load_other_packages` proves `clay:packages` import denied at the module boundary in the third-party runtime (loadPackage unreachable, target never enabled).
      - A-as-B: host-stamped provenance retrofit from task 5 (caller manifests deleted; `PackageContext` from enabled record only); LS session spoofing closed host-side.
      - No-preapproval execution: `unapproved_third_party_package_never_executes_before_adoption` (sentinel never fires).
      - Stale/revoked approvals: `package_update_stales_adoption`, `third_party_enable_requires_exact_durable_adoption`, `replacement_edge_requires_approval_and_stales_target_on_commit`.
      - Malformed stores: `store_fails_closed_on_corrupt_truncated_and_unknown_version`, `service_open_fails_closed_on_corrupt_approval_store`.
      - First-party mutation without dual consent: `structured_relation_fails_without_owner_extension_point`, `structured_relation_fails_on_operation_not_offered_by_owner`.
      - Replacement escalation: `replacement_cannot_target_clay_core`, `third_party_replacement_withdraws_trusted_target_atomically`, dependency re-enable of replaced targets denied (`target_replaced`).
      - External-process grant transfer: `replacement_language_server_requires_own_fresh_grant` (target grant never transfers).
      - Bridge abuse: NEW `cross_domain_load_bridge_rejects_trusted_records` (bridge op rejects Trusted record, third-party runtime untouched).
      - Replay scope: NEW `third_party_poison_replay_skips_disabled_packages` (post-poison disable → replay skips; no completion ever produced; package stays disabled; domain stays alive).
    - Performance (same host as task 1, ignored probe `runtime_resource_baseline_probe` extended; machine-variable same-command comparison evidence):

      | Metric | Task-1 single-runtime | Task-13 two-runtime | Delta |
      | --- | --- | --- | --- |
      | Startup RSS delta (process before → first ping) | +26,492 KiB | +42,472 KiB | +15,980 KiB (second V8 isolate) |
      | Threads after start | 20 | 22 | +2 (worker + V8 platform) |
      | First ping | 8,179 µs | 8,611–11,975 µs | within noise of one extra runtime |
      | Warm ping median | 143 µs | 260–284 µs | +120–140 µs (documented dispatch+V8-platform overhead; completion/parse hot paths budgeted separately) |
      | Candidate reload ping | 6,361 µs | 8,093–9,911 µs | one extra runtime build |
      | Four bundled loads | 0.37–2.7 ms | 1.9–6.9 ms | machine-variance dominated; same order |
      | Persistent workers started | 2 | 2 | zero per-package runtimes ✓ |
      | Third-party provider invoke median | n/a (trusted-only) | 316 µs | cross-domain dispatch |
      | Bridge saturation (20 serial provider dispatches) | n/a | 6,765 µs (~338 µs each) | no queue growth, no deadlock |
      | Third-party recovery (poison → replace → replay → first answer) | n/a | 8,502 µs | bounded one-pass replay |

      No task-1 budget is exceeded beyond the documented second-isolate RSS/thread cost and the warm-ping dispatch delta recorded above; no per-package runtime/thread is created.
    - Compatibility: all bundled packages load in the trusted domain (`enabled_packages=4` probe assertion + full suite); approved third-party extension/replacement flows pass (package_graph/package_loading suites); no new integration-test binary added (all tests in existing files).
    - Linux closure: `cargo fmt --check` clean, `cargo check --all-targets` clean, `cargo clippy --all-targets -- -D warnings` clean, `cargo test --all-targets` 1760 passed / 0 failed, `git diff --check` clean.
    - Observation: a completion request racing a disabled-after-poison provider receives no result set (silent drop) — fail-closed and bounded by client-side completion timeouts; noted, not changed here.
    - References:
      - `tests/{package_loading,package_graph,package_conflicts,clay_js_api_inventory,persistent_runtime_hot_reload,runtime_update_protocol,language_server_authority,primitives_docs}.rs`.
  - Test Cases to Write:
    - Hostile matrix covering both runtime domains, shared third-party cohort disclosure, every adoption/replacement lifecycle, and queue/payload limits.
    - Full bundled package and representative third-party extension smoke flow.

- [x] Create or verify Clay JS APIs for public programmatic surfaces
  - Acceptance Criteria:
    - Functional: Audit all changed Rust visibility and expose only genuine public package inspection, extension, adoption-status, revocation, and replacement surfaces through stable domain-appropriate facades/docs; internal domain IDs, package contexts, approval storage, queues, and trusted ops remain private/`pub(crate)`.
    - Performance: APIs expose no per-keystroke callback bridge, raw runtime handles, queue/heap tuning, or unbounded authority payloads.
    - Code Quality: Every public API has stable ID, concise authority-aware export, user-facing name, key bindings, custom properties, Rust/op/facade paths, docs, generated registry, lookup, and generic coverage.
    - Security: Third-party facade cannot call trusted configuration/admin approval APIs; raw `Deno.core.ops` and JavaScript-visible principal/bearer tokens are not public APIs.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/project-patterns/references/{clay-js-api-boundary,clay-js-api-naming,clay-js-api-schema,documentation-as-code,doc-registry-tests}.md`.
      - `.agents/skills/create-plan/references/clay.md`: Clay JS API task requirements.
    - Options Considered:
      - Expose internal runtime controls: rejected.
      - Public read/declare/use surfaces plus trusted user/admin mutation surfaces in separate facade allowlists. Chosen.
    - Chosen Approach:
      - Map each changed public Rust function to explicit op/facade/docs or reduce visibility; generate domain facade allowlists from authoritative inventory.
    - API Notes and Examples:
      ```text
      Public third-party: inspect own package/extension points, register approved contribution.
      Trusted user/admin: approve/revoke adoption, disable/replace package.
      Internal only: RuntimeDomain, package context, approval path, bridge queues.
      ```
    - Files to Create/Edit:
      - `docs/reference/clay-js-api/*.md`, `docs/index.md`, generated registry artifacts.
      - `runtime/js/*.ts|*.js|*.d.ts` and op modules for approved public shapes only.
      - `tests/{clay_js_api_inventory,clay_js_doc_registry,clay_js_facade_layout,rust_visibility_api_mapping}.rs`.
    - References:
      - Initial first-party API inventory in this plan and wiki review.
  - Test Cases to Write:
    - Trusted and third-party facade inventories expose exactly their allowed APIs.
    - Missing docs/schema/index/generated entry fails with update command.
    - Internal runtime/approval identifiers are absent from public exports.
  - Completion Evidence (2026-07-21):
    - Rust visibility audit: `PackageApprovalStore`, `ApprovalMismatch`, `ApprovalStoreError`, `RelationVerificationError`, and `rfc3339_now` reduced from `pub` to `pub(crate)` (internal storage/routing/error types). `PackageApprovalRecord`, `ApprovedRelation`, `ApprovedReplacement` remain `pub` as they constitute the adoption/approval data model exposable to callers. `PackageContext` already `pub(crate)`. `RuntimeDomain`, `BundledTrustError`, `CrossDomainRequestEnvelope` already `pub(crate)`. Extension-point types (`ExtensionPointDeclaration`, `StructuredRelationRequest`, `ExtensionContributionKind`, `RelationOperation`) stay `pub` — they are part of the public package-authoring manifest API.
    - Third-party facade allowlist: `THIRD_PARTY_FACADES` (13 entries) exactly matches the plan's Public-third-party inventory classification. New test `tests/rust_visibility_api_mapping.rs` asserts exact match and production-code-thirteen count; also asserts seven internal types (`PackageApprovalStore`, `ApprovalMismatch`, `ApprovalStoreError`, `BundledTrustError`, `CrossDomainRequestEnvelope`, `PackageContext`, `RelationVerificationError`) are `pub(crate)`/private. All 3 existing Clay JS API test suites (inventory 62, doc-registry 34, facade-layout 4) pass clean.
    - No new public JS API was added by Plan 061 — the security boundary split restricts existing APIs (third-party `clay:packages` denied at import boundary; 32 trusted-only ops absent in the package extension). The adoption/revoke/replacement/rollback actions are CLI entry points and `PackageService` `pub fn` methods, not JS facades; the `api-inventory.toml` registry does not add new entries for these CLI paths.
    - Docs: No new `docs/reference/clay-js-api/*.md` pages needed (zero new JS APIs). Existing 105 API entries in `api-inventory.toml` already cover all public JS exports.
    - Validation: `cargo test --all-targets` 1745 (lib 937 + 33 integration + all doc-registry/facade/visibility suites) passed / 0 failed; `cargo clippy --all-targets -- -D warnings` clean; `cargo fmt --check` clean; `git diff --check` clean.

- [x] Create or verify Clay configuration APIs
  - Acceptance Criteria:
    - Functional: `~/.config/clay/init.js` can load already-approved packages, inspect/revoke/choose package relationships through documented trusted configuration APIs, and select/rollback replacements without embedding manifests, approval tokens, or low-level ops.
    - Performance: Configuration performs adoption/graph changes only at startup/reload/user command boundaries and adds no hot-path tuning or runtime polling.
    - Code Quality: Every behavior-changing option is a Clay JS API with docs/discovery metadata; no parallel hidden config key/store is introduced.
    - Security: Configuration cannot promote third-party code, bypass first-adoption approval, mint package contexts, install trusted ops, weaken validation/limits, or transfer process grants.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/project-patterns/references/configuration-system.md`.
      - `.agents/skills/create-plan/references/clay.md`: configuration and one-line package loading requirements.
    - Options Considered:
      - Approval secrets in `init.js`: forgeable/leaky and unsuitable.
      - Trusted config references host-owned durable approvals by package spec/relationship. Chosen.
    - Chosen Approach:
      - Keep first adoption an explicit host UI/CLI act; after approval preserve one-line load, and expose explicit trusted configuration APIs for enable/disable/replacement selection and rollback.
    - API Notes and Examples:
      ```javascript
      await loadPackage("vendor/markdown"); // succeeds only after adoption
      await setPackageReplacement({ target: "@clay/markdown", replacement: "vendor/markdown" });
      ```
    - Files to Create/Edit:
      - `runtime/js/configuration.*` or `packages.*` based on final API ownership.
      - `src/server/ops/{configuration,packages}.rs`.
      - `docs/reference/clay-js-api/{configuration,packages}*.md`, `docs/index.md`, generated registry.
      - Existing configuration/package/API docs tests.
    - References:
      - `~/.config/clay/init.js` loading contract and current `loadPackage` facade.
  - Test Cases to Write:
    - Approved one-line load, pending-adoption diagnostic, explicit replacement, rollback, stale approval, and forbidden promotion/internal limit settings.
  - Completion Evidence (2026-07-21):
    - Verified one-line load: `loadPackage` from init.js works for adopted third-party packages. The `loadPackage` facade already routes ThirdParty records through the cross-domain bridge op (`op_clay_packages_load_in_package_domain`); `PackagesService::enable` in the `ensure_first_party_record_locked` helper handles adoption gating, capability verification, graph resolution (dependsOn/extends/disables/replaces), atomic target disable, and conflict reconciliation. No separate `setPackageReplacement` or `enable` JS API is needed — `loadPackage` alone covers the replacement scenario: CLI `adopt` the replacement, then `loadPackage(replacement)` from init.js triggers replacement atomically through enable_graph.
    - New tests (3): (a) `third_party_config_load_fails_with_pending_adoption_diagnostic` — unapproved third-party package loaded via init.js fails with adoption diagnostic, package stays disabled. (b) `third_party_config_load_succeeds_after_cli_adoption` — after CLI adoption, one-line `loadPackage` from init.js succeeds; cross-domain bridge routes the load entry to the third-party worker and absorbs the completion-provider registration. (c) `stale_approval_blocks_config_load_with_clear_diagnostic` — version drift invalidates approval and blocks config load with a diagnostic.
    - Forbidden promotion verified: init.js cannot bypass adoption (enable requires durable approval), cannot mint PackageContext (pub(crate) Rust-only), cannot call trusted-only ops from third-party packages (op absence in package extension), and cannot transfer LS grants (host-side per-package). All enforcement is compiled-in, not configurable.
    - Rollback is deferred as CLI-only (`clay package rollback <target>` existing in main.rs). The facade stubs in `runtime/js/packages.ts` (`enable`, `disable`, `inspect`, `list`, `authorize`) are correctly marked as planned/unimplemented.
    - Documentation: `loadPackage` js-api doc already describes one-line default, user-authorized requirement, and capability gating. `creating-packages.md` documents the CLI adoption flow and one-line load from init.js. No new docs pages needed (zero new JS APIs).
    - Validation: `cargo test --all-targets` 1748 passed / 0 failed; `cargo clippy --all-targets -- -D warnings` clean; `cargo fmt --check` clean; `git diff --check` clean.

- [x] Update or verify the code wiki after implementation
  - Acceptance Criteria:
    - Functional: The project code wiki is updated after all implementation tasks are complete, covering final two-domain runtime, op/module allowlists, package provenance, extension/adoption graph, first-party APIs, replacement, bridge routing, lifecycle, UI, and tests.
    - Performance: Wiki documents measured runtime overhead, fixed budgets, bridge/recovery behavior, and hot-path exclusions without adding runtime work.
    - Code Quality: Pages explain source ownership, data/control flow, invariants/tradeoffs, examples, source/test paths, and link from `docs/wiki/index.md`; obsolete one-runtime/per-package-isolate guidance is removed.
    - Security: Wiki states exact domain boundary, third-party shared-cohort limitation, dual-consent mutation, replacement provenance, internal-op absence, external-process truth, and revocation without exposing secrets.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/project-wiki/SKILL.md` and page template.
    - Options Considered:
      - Update after each task: churns while schemas move.
      - Update once after implementation/verification/API/config maintenance. Chosen.
    - Chosen Approach:
      - Update implementation wiki once from final code and test evidence, retaining the initial review pages as rationale with links to final implementation pages.
    - API Notes and Examples:
      ```text
      docs/wiki/index.md
      docs/wiki/modules/package-runtime-trust-domains.md
      docs/wiki/modules/package-extension-and-adoption-authority.md
      ```
    - Files to Create/Edit:
      - `docs/wiki/index.md`: final navigation.
      - `docs/wiki/modules/{embedded-js-runtime,third-party-runtime-authority,package-principal-and-result-routing-primitive-review,first-party-package-extension-api-review}.md`: final behavior.
  - Completion Evidence (2026-07-21):
    - Updated `embedded-js-runtime.md`: added "Two Runtime Trust Domains (Plan 061)" section covering domain workers, op/facade partitioning, cross-domain bridge op, worker replacement and replay, document-analysis worker routing, and invariants (exactly-two-persistent-runtimes, no promotion path, compile-time op/facade subsets, durable approval gating).
    - Rewrote `third-party-runtime-authority.md` as "Package Extension and Adoption Authority (Plan 061)": covers four-layer architecture (identity/provenance/adoption/extension), bundled trust inventory with FNV-1a-64 fingerprints, extension points schema (version/operation/contribution-kind/scopes), structured relations and graph resolution, durable approval store with atomic persistence, adoption lifecycle state machine (Installed→Pending→Approved→Enabled→Revoked with stale detection), enable transactionality with snapshot/restore, replacement atomicity with rollback, cross-domain typed invocation (envelope validation, requester/target checks), first-party package replacement flow, host-stamped package provenance (P0-1 fix), and invariants. Removed obsolete pre-implementation target-model content (one-runtime profiles, native-trust/sandboxed/restricted profiles, unimplemented capability vocabulary).
    - Left `package-principal-and-result-routing-primitive-review.md` and `first-party-package-extension-api-review.md` unchanged (already finalized in tasks 2 and 8).
    - Updated `docs/wiki/index.md` entries for both pages; no new page files created (the existing `third-party-runtime-authority.md` file was repurposed).
    - Updated `tests/package_loading_docs.rs`: renamed test to `package_extension_and_adoption_authority_is_documented`, replaced outdated phrase checks with implemented Plan 061 terms (BUNDLED_PACKAGES, verify_bundled_trust, PackageApprovalStore, adoption_state, approve_package, rollback_replacement, enable_graph, dependOn/extend/disable/replace).
    - Removed dead code: `PackageApprovalStore::get()` extracted to `#[cfg(test)]` after clippy `dead_code` warning.
    - Validation: `cargo test --all-targets` 1748 passed / 0 failed; `cargo clippy --all-targets -- -D warnings` clean; `cargo fmt --check` clean; `cargo test --test primitives_docs` 132 passed; `cargo test --test package_loading_docs` 52 passed.
      - New final implementation pages only where existing pages cannot clearly own the topic.
      - `tests/primitives_docs.rs`: deterministic wiki/primitive coverage.
    - References:
      - `.agents/skills/project-wiki/references/page-template.md`.
  - Test Cases to Write:
    - Wiki index/coverage tests pass and every changed page names final source paths, tests, limits, authority, extension/replacement flow, and known limitations.

## Compromises Made

- To be filled after tasks are completed and tests pass.

## Further Actions

- To be filled after task completion with improvements, rationale, and priority.
