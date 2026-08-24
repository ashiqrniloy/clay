# Phase 27: Package Data Flow Consolidation

Source: `roadmap.md` Phase 27 (added 2026-08-18 from the package-data-flow
review). This plan implements Phase 27.1–27.8.

Approved decisions (binding constraints):

- `decision-logs/2026-08-18-1758-single-manifest-package-loading.md` —
  `package.json` `clay.contributions` is the only package registration data
  path; first-party load entries execute code only; Tier 1 native grammars
  stay owned by `NativeGrammarDescriptor`.
- `decision-logs/2026-08-18-1758-package-capability-presets.md` — optional
  `clay.preset` (`code-mode`, `prose-mode`, `lsp-bridge`) expands at
  validation; explicit deviations win; expanded set is validated, budgeted,
  and shown on inspect.
- `decision-logs/2026-07-21-0001-two-package-runtime-trust-domains.md` —
  trusted vs shared third-party runtimes; compiled inventory plus exact
  provenance/integrity; no V8 objects across domains.
- `decision-logs/2026-06-09-0219-explicit-init-js-package-loading-with-one-line-defaults.md`
  — `loadPackage("@clay/markdown")` remains the one-line user path.
- `decision-logs/2026-07-14-2023-language-server-package-authority.md` —
  27.5 consolidates factory code only; no new process authority.

Confirmed-correct invariants (must not regress): two runtime trust domains;
deny-by-default `language-server` grants; no language-named Rust branches;
no package JavaScript on paint/layout/keypress; FNV inventory is the
bundled trust boundary; imperative register APIs stay public for `init.js`
and runtime contributions.

Task order note: 27.7 (inventory generation) is scheduled **before** the
package.json edits in 27.1–27.6. Roadmap listed it last; doing it first
removes the hand-edited fingerprint tax those edits would otherwise pay.
27.4 then extends the generated inventory with an exports map.

UI scope: this phase does **not** change Masonry/settings/shell UI.
"Package inspection UI" in the roadmap is the existing CLI surface
(`clay package inspect`) plus `PackageInspection` fields. No
the UI guidance current at execution time gate and no visual/a11y screenshot task.

## Objectives

- One package data path: `clay.contributions` in `package.json`. First-party
  load entries keep only executable wiring (parse-module import, LSP factory).
- One owner per native grammar style map (`FIRST_PARTY_NATIVE_GRAMMARS`).
- Capability presets collapse the copy-paste permission / apiDependency /
  extension-point / contribution-family boilerplate.
- Shared first-party modules resolve once (no vendored `dist/shared/` copies).
- Four LSP bridges share one factory; a new language server is data + config.
- First-party syntax producers emit the closed `TokenType`+`Modifiers`
  vocabulary; free-form `style_token` is frozen compat only.
- Bundled inventory is generated from a checked-in list; tests stay the
  trust gate.

## Expected Outcome

- A new code-language package is `preset: "code-mode"` plus deviations and a
  tiny execute-only load entry.
- `loadPackage("@clay/<pkg>")` still activates full default behavior with no
  user-side primitive plumbing.
- `clay package inspect` shows preset name, expanded permissions, and
  native-grammar ownership.
- First-party trees contain no `*PackageManifest()` literals, no empty-args
  register calls in load entries, no dead `syntaxGrammars` on native-owned
  languages, and no vendored `lsp-shared` copies.
- Adding a bundled package is a list entry + package tree, not an 11-struct
  hash edit.

## Tasks

- [x] Establish package-data-flow baseline and pattern compliance
  - Acceptance Criteria:
    - Functional: Record current duplication with file:line anchors: load-entry register calls, `*PackageManifest()` literals, native `syntaxGrammars` blocks, vendored `dist/shared/` copies, hand-maintained `BUNDLED_PACKAGES`, markdown `markup.*` tokens, inspect fields. Capture `clay package inspect @clay/markdown` and `@clay/rust` output.
    - Performance: Record current load/enable test names and any load-time budget constants; no implementation work.
    - Code Quality: List which later task owns each defect so each can be checked as fixed.
    - Security: Fixture/CLI output only; no host secrets.
  - Approach:
    - Documentation Reviewed:
      - `roadmap.md` Phase 27; decisions `2026-08-18-1758-single-manifest-package-loading.md`, `2026-08-18-1758-package-capability-presets.md`.
      - Patterns: `package-manifest-single-source.md`, `package-runtime-trust-domains.md`, `package-distribution.md`, `mode-primitive-first.md`, `language-capability-sequencing.md`, `authority-boundaries.md`, `clay-js-api-naming.md`, `planning-checklist.md`.
    - Options Considered:
      - Skip baseline: rejected — later tasks need a before-list to prove deletion.
      - Full visual screenshot matrix: rejected — no Masonry change.
    - Chosen Approach:
      - One written baseline in this plan's evidence; reuse existing package-load tests as the before suite.
    - API Notes and Examples:
      ```text
      clay package inspect @clay/markdown
      rg -n "PackageManifest|serverRegisterSyntaxGrammar|serverActivateMajorMode" packages/*/dist
      ```
    - Files to Create/Edit:
      - `plans/093-Phase27-Package-Data-Flow-Consolidation.md`: evidence appended to this task.
    - References:
      - `.agents/skills/project-patterns/references/package-manifest-single-source.md`
      - `packages/{markdown,rust,typescript,javascript,lsp-*}/dist/load.js`
      - `src/packages/bundled.rs`, `src/server/syntax.rs`
  - Test Cases to Write:
    - Baseline checklist: every planned deletion has a before anchor.

  - Completion Evidence (2026-08-19):
    - Patterns re-read: `package-manifest-single-source.md`, `package-distribution.md`, `authority-boundaries.md`. No implementation work.
    - Isolated-HOME `target/debug/clay package inspect @clay/markdown` and `@clay/rust` both failed at `launch.rs` `refresh_installed` → `PnpmBackend` spawn of `pnpm` (`ProcessSpawnFailed`). Inspect is store-only (`~/.config/clay/packages` via `default_store_root`); it does not list bundled `loadPackage` packages. Current printed fields (`src/launch.rs:237-268`): Package, Version, API prefix, Status, Modes, Permissions, Commands, Config keys, Docs, Adoption. `PackageInspection` (`src/packages/service.rs:260-273`) has no `preset`, no expanded-vs-declared permission split, no native-grammar ownership. Programmatic inspect covered by `tests/package_loading.rs` (`markdown_package_does_not_execute_on_install`, `granted_powerful_capabilities_parse_and_show_in_inspection`).
    - Load/enable before-suite (keep green): `src/server/js_runtime/mod.rs` `load_package_registers_first_party_syntax_grammars`, `load_package_resolves_and_activates_first_party_markdown_end_to_end`, `load_package_markdown_default_activates_full_mode_from_init_js`, `load_package_is_idempotent_per_persistent_runtime`, `load_package_remains_idempotent_inside_one_generation`, `op_clay_packages_load_package_by_specifier_resolves_and_enables_first_party_markdown`, `rust_package_expansion_registers_mode_command_completion_and_status`, `typescript_package_expansion_registers_mode_command_completion_and_status`, `javascript_package_expansion_registers_mode_command_completion_and_status`, `lsp_rust_package_loads_after_exact_grant_without_starting_child`, `lsp_typescript_and_javascript_packages_load_after_exact_grants_without_starting_children`, `lsp_markdown_package_loads_after_exact_grant_without_starting_child`; `tests/package_loading.rs` `package_service_install_records_without_executing_runtime`, `enable_rejects_duplicate_prefix_and_mode_and_command`; `tests/lsp_bridge.rs` `lsp_language_packages_fixture_grants_before_one_line_loads`; `src/packages/bundled.rs` `inventory_matches_source_tree`.
    - Load-time budgets: `JS_RUNTIME_EVALUATION_TIMEOUT_MS = 5000` (`src/perf/budgets.rs:305`, covers config + `loadEntry`); `RUNTIME_CONFIGURATION_EVAL_P95_BUDGET_MS = 25`; `BEHAVIOR_MANIFEST_PAYLOAD_BUDGET_BYTES = 8192` (estimatedManifestBytes gate); first-party `clay.performance.estimatedManifestBytes` 1700 rust / 1900 others. No paint/keypress work in this baseline.
    - Defect anchors → owning task:

      | Defect | Before anchor | Owner |
      | --- | --- | --- |
      | Load-entry register ceremony | `packages/rust/dist/load.js:39-70` (`serverRegisterSyntaxGrammar({})`, mode/command/completion/status); `packages/typescript/dist/load.js:137-152`; `packages/javascript/dist/load.js:124-146`; `packages/markdown/dist/load.js:95-190` (mode, **hardcoded `documentId ?? 1` / `sample.md` at 87-88**, commands, completion, status, parse handler, panel) | 27.1 |
      | `*PackageManifest()` literals | `packages/markdown/dist/index.js:186` + `load.js:15,63`; `packages/typescript/dist/index.js:72` + `load.js:67`; `packages/javascript/dist/index.js:59` + `load.js:62`; `packages/rust/dist/index.js:90` + `load.js:19` | 27.1 |
      | `editorRules` only in JS | rust `load.js:50`, typescript `load.js:143`, javascript `load.js:130`, markdown `load.js:101,114`. Absent from every first-party `package.json` `modePatterns` | 27.1 |
      | `modePatterns` not in record | `src/packages/record/mod.rs` `PackageContributions` (472-499) has no mode-pattern field; `parse_contributions` never reads `modePatterns` | 27.1 |
      | Dead native `syntaxGrammars` | `packages/{rust,typescript,javascript,markdown}/package.json` `clay.contributions.syntaxGrammars` (styleMap + queries). Native owner: `FIRST_PARTY_NATIVE_GRAMMARS` `src/server/syntax.rs:282` (rust, typescript, tsx, javascript, markdown). Silent skip: `syntax.rs:697` / `is_shadowed_by_native_first_party` `:827` | 27.2 |
      | No `clay.preset` | `rg` over `src/` + `packages/` is empty. Permissions / apiDependencies / extensionPoints copied across rust/ts/js/md + four `lsp-*` manifests | 27.3 |
      | Vendored `lsp-shared` | 4 × 6 files: `packages/lsp-{rust,markdown,typescript,javascript}/dist/shared/{client,framing,mapping,positions,typescript-language-server,utf8}.js`. Canonical: `packages/lsp-shared/`. Copy script: `scripts/update-first-party-lsp-shared.mjs`. Helper intentionally absent from `BUNDLED_PACKAGES` (`bundled.rs:46-48`) | 27.4 |
      | LSP load-entry + factory | Each `packages/lsp-*/dist/load.js:7` `serverRegisterDocumentAnalyzer`; each returns `*PackageManifest()`. rust/markdown `dist/server.js` ~330-line copies vs typescript/javascript factory wrappers | 27.5 (analyzer register stays as execute-only wiring) |
      | `markup.*` producer | `packages/markdown/dist/parser.js:20-33` (`STYLE_TOKENS` / heading map). Compat: `TokenType::classify_style_token` `src/protocol/decorations.rs:179`; `DecorationSpan::from_style_token` `:351` | 27.6 |
      | Hand inventory | `src/packages/bundled.rs:49-129` — 14 `BundledPackageEntry` literals with hand FNV-1a-64 hashes. Gate: `inventory_matches_source_tree` `:216` | 27.7 |
      | Inspect gaps | No preset / expanded-perms / native-owner fields; CLI inspect requires pnpm refresh and ignores bundled-only packages | 27.2 + 27.3 + 27.8 |
    - Out of Phase 27 language/LSP scope (do not treat as 27.1 deletions): `@clay/git` `gitPackageManifest` (`packages/git/dist/index.js:9`, `load.js:11,40`); `@clay/settings` `serverRegisterCommand` / `serverRegisterPanelContribution` (`packages/settings/dist/load.js:114,121`).
    - Security: isolated temp HOME only; no host store/paths/secrets retained.

- [x] Review existing editor primitives and plan generic primitive gaps before package work
  - Acceptance Criteria:
    - Functional: Inventory existing package/mode/syntax/LSP primitives and state what Phase 27 can achieve with them before any new Rust. Confirm `clay.contributions.modePatterns` is currently **not** parsed into `PackageContributions` and live mode/command/completion/UI install still happens only via load-entry ops.
    - Performance: Identify load-time work that must stay off paint/keypress (apply-record, preset expand, inventory gen, specifier resolve).
    - Code Quality: New Rust is generic/reusable (apply-record, preset expand, inventory exports, inspect fields). No language-named branches. Primitive docs/wiki/index/coverage tests updated in the implementing tasks.
    - Security: No new filesystem/network/shell/AI/WASM/raw-op authority; first-party export resolution stays inventory-bound and trusted-runtime-only.
  - Approach:
    - Documentation Reviewed:
      - `docs/reference/primitives/index.md`, `registry.md`, `package-loading.md`, `package-security.md`, `syntax-vocabulary.md`.
      - `docs/wiki/modules/primitive-architecture.md`, `package-loading.md`, `syntax-grammar-registry.md`, `first-party-language-packages.md`, `first-party-lsp-bridge-packages.md`.
      - Pattern `mode-primitive-first.md`. Decision `2026-06-04-1923-replace-markdown-parser-with-markdown-it-and-primitive-first-mode-planning.md`.
    - Options Considered:
      - Keep empty-args register ops as the only apply trigger: rejected — that is the ceremony the decision deletes.
      - Mode-specific Rust apply paths: rejected — one generic apply-record.
    - Chosen Approach:
      - Map each roadmap item to existing vs new generic primitive; table is the contract for later tasks.
    - API Notes and Examples:
      ```rust
      // Planned (27.1): after enable, apply PackageRecord contributions
      // into the same registries the empty-args ops already write.
      apply_package_record_contributions(&record, &mut registries)?;
      ```
    - Files to Create/Edit:
      - `plans/093-...md`: verified gap table recorded here.
      - Primitive refs updated by implementing tasks: `docs/reference/primitives/package-loading.md`, `syntax-vocabulary.md`, `docs/wiki/modules/package-loading.md`.
    - References:
      - `src/packages/record/mod.rs` (no `modePatterns` family today)
      - `src/server/ops/syntax.rs` `op_clay_syntax_register_syntax_grammar` (reads current package record)
      - `src/server/ops/packages.rs` `op_clay_packages_load_package_by_specifier` (enable + allowlist only)
  - Test Cases to Write:
    - Gap-table review: every planned Rust change maps to a named generic primitive.

  - Completion Evidence (2026-08-19):
    - Docs/wiki/pattern/decision read: `docs/reference/primitives/{index,registry,package-loading,package-security,syntax-vocabulary}.md`; `docs/wiki/modules/{primitive-architecture,package-loading,syntax-grammar-registry,first-party-language-packages,first-party-lsp-bridge-packages}.md`; `mode-primitive-first.md`; decision `2026-06-04-1923`. No implementation work. Coverage gate for later docs: `tests/primitives_docs.rs`.
    - Confirmed: `PackageContributions` (`src/packages/record/mod.rs:472-499`) has no mode-pattern field; `parse_contributions` never reads `modePatterns`. `ClayPackageMetadata` (`src/packages/manifest.rs:18-33`) has no `preset`. `PackageService::enable` / `enable_graph` assemble + grant + graph + conflict only — `rg` finds no `register_mode`/`register_command`/`register_syntax`/`register_completion`/`register_component` in `service.rs`.
    - Live install today is load-entry ops only, two styles:
      - Record-backed empty-args: `op_clay_syntax_register_syntax_grammar` (`src/server/ops/syntax.rs:39-61`) and `op_clay_completion_register_completion_provider` (`src/server/ops/completion.rs:134-145`) read `current` package `syntax_grammars` / `completion_providers`.
      - Caller-JSON: `op_clay_modes_register_pattern` (`src/server/ops/modes.rs:24-48`, `editorRules` at `:169-183`), `op_clay_commands_register_command` (`src/server/ops/commands.rs:23-38`), UI register ops (`src/server/ops/ui.rs:18-49`) parse the JS declaration. Markdown also calls `serverActivateMajorMode` with hardcoded `documentId ?? 1`.
      - Execute-only (must stay in load entry): `op_clay_parse_register_parse_handler` (`src/server/ops/parse.rs:16`) imports a module; `op_clay_language_register_document_analyzer` (`src/server/ops/document_analysis.rs:19`) wires the worker. `op_clay_packages_load_package_by_specifier` (`src/server/ops/packages.rs:537`) enable + allowlist + later JS `import(loadEntry)` only.
    - Existing primitives Phase 27 reuses as-is: `assemble_package_record`; `ModeRegistry` / `register_mode` (`ops/mod.rs:1184`); `register_command` (`:1293`); `register_completion_provider_metadata` (`:1589`); `register_syntax_grammar_package` (`:1702`); `FIRST_PARTY_NATIVE_GRAMMARS` + silent `is_shadowed_by_native_first_party` (`syntax.rs:282`, `:697`, `:827`); `TokenType`+`Modifiers` + `from_style_token` (`decorations.rs:351`); `PackageInspection` / CLI inspect; `ClayModuleLoader` resolve order (`source.rs:79-142`: facades → `markdown-it` → load-entry allowlist → relative-root confine → trusted config → deny); `BUNDLED_PACKAGES` + `fnv1a64_hex`; `LanguageServerSession` / analyzer / `lsp-shared` factory; `validate_manifest_value`. `build.rs` is Windows-manifest only.
    - What Phase 27 can do before new Rust: delete JS ceremony only after apply-record exists; 27.2 delete dead `syntaxGrammars` data; 27.5 JS factory; 27.6 markdown producer + docs. Cannot get one-line load parity, presets, shared imports, inspect fields, or generated inventory without the generic gaps below.
    - Verified gap table (implementation contract):

      | Roadmap item | Existing primitive reused | New generic primitive required | Hot-path? |
      | --- | --- | --- | --- |
      | 27.1 apply manifest data | `assemble_package_record`; `register_{mode,command,completion_provider_metadata,syntax_grammar_package,component_contribution}` | Parse `modePatterns` (+ `editorRules`) into the record; `apply_package_record_contributions` at `loadPackage` after enable, calling those existing register methods | No (load time) |
      | 27.1 execute-only load entries | `loadEntry` allowlist; parse-handler module import; analyzer register | none — delete JS register/`*PackageManifest` ceremony | No |
      | 27.2 native grammar owner | `FIRST_PARTY_NATIVE_GRAMMARS`, `is_shadowed_by_native_first_party` | Diagnostic + inspect note instead of silent `continue` at `syntax.rs:697` | No |
      | 27.3 presets | `validate_manifest_value`, permission / apiDependency / extension-point parsers | `clay.preset` expand-then-validate; store declared preset + expanded permissions | No |
      | 27.4 shared imports | `ClayModuleLoader` allowlist + relative-root confine; `BUNDLED_PACKAGES` | Inventory `exports` map; trusted-only specifier branch after facades, before deny | No (module resolve at load) |
      | 27.5 LSP factory | `lsp-shared` mapping/client, `createTypescriptLanguageServerBridge`; analyzer + `LanguageServerSession` | none — JS factory only | No |
      | 27.6 vocabulary | `TokenType`+`Modifiers`, `from_style_token` / `classify_style_token` compat | none — first-party producer data + deprecation docs | No (existing background parse) |
      | 27.7 inventory gen | `fnv1a64_hex`, `inventory_matches_source_tree` | `build.rs` emit from checked-in list (`include!` OUT_DIR) | No (compile time) |
      | 27.8 inspect | `PackageInspection`, `clay package inspect` | Extra fields: preset, expanded perms, native-grammar owner | No |
    - Security contract unchanged: no new filesystem/network/shell/AI/WASM/raw-op/`language-server` authority. 27.4 exports trusted-domain + inventory-listed paths only. Imperative register APIs stay public for `init.js`.

- [x] Preserve the two package-runtime trust domains across consolidation
  - Acceptance Criteria:
    - Functional: Trusted classification still comes only from generated bundled inventory + exact provenance/integrity. Third-party runtime still lacks Clay-internal ops and trusted module roots. 27.4 export specifiers resolve only in the trusted domain, only to inventory-listed first-party paths, with no `../` escape and no third-party import of those specifiers.
    - Performance: Resolver remains allowlist lookup + file read; no extra IPC on typing/paint.
    - Code Quality: Cross-domain communication stays typed/bounded/inert Rust values with generation/payload/timeout/provenance/revocation checks.
    - Security: Tests prove internal-op/module denial, stale-generation rejection, adoption/revocation, replacement rollback, and disclosed lack of hostile isolation among third-party packages. 27.5 adds no process grant, no implicit `language-server` from load/bundled trust, and no runtime-selected executable/argv/cwd/env.
  - Approach:
    - Documentation Reviewed:
      - Pattern `package-runtime-trust-domains.md`, `authority-boundaries.md`.
      - Decision `2026-07-21-0001-two-package-runtime-trust-domains.md`.
      - `src/server/js_runtime/source.rs` loader branches; `src/packages/bundled.rs`.
    - Options Considered:
      - npm-style third-party imports of `lsp-shared`: rejected — trust-domain leak.
      - Copy shared files into each package (status quo): rejected by 27.4.
    - Chosen Approach:
      - Inventory-bound first-party exports only; third-party keep vendoring-out-of-scope or public facades. Process authority unchanged.
    - API Notes and Examples:
      ```js
      // trusted first-party loadEntry only
      import { LspClient } from "lsp-shared/client.js";
      // third-party: denied (runtime.invalid_import)
      ```
    - Files to Create/Edit:
      - Tests under `src/server/js_runtime/mod.rs` / `tests/package_loading.rs` (extend existing deny suites).
    - References:
      - `decision-logs/2026-07-21-0001-two-package-runtime-trust-domains.md`
      - `decision-logs/2026-07-14-2023-language-server-package-authority.md`
  - Test Cases to Write:
    - Trusted package resolves `lsp-shared/client.js` to the inventory file.
    - Third-party loadEntry importing `lsp-shared/client.js` is denied.
    - `../` or absolute path via an export specifier is denied.
    - Existing cross-domain internal-op deny / stale-generation / revoke tests stay green.

  - Completion Evidence (2026-08-19):
    - Patterns/decisions read: `package-runtime-trust-domains.md`, `authority-boundaries.md`, `2026-07-21-0001`, `2026-07-14-2023`. No implementation. 27.4/27.5 own the new deny tests and factory change.
    - Two domains only (`src/packages/bundled.rs:25-31`). Trusted = `verify_bundled_trust` (`:173-192`): exact name + `ClayShipped` + version + canonical `packages/<root>` + FNV-1a-64 of `package.json`. `runtime_domain` (`:202-207`) never trusts `@clay/*` name. Spoof/wrong-version/wrong-root/tamper → ThirdParty (`bundled.rs` tests `:264-331`). `packages/lsp-shared` intentionally absent from `BUNDLED_PACKAGES` (`:46-48`, skipped at `:232`) — helper, not `loadPackage`-able. 27.4 may add an **exports** map; must not add a loadable inventory entry or promote by name.
    - Distinct runtimes/ops: `ClayJsRuntimeService` starts Trusted + ThirdParty workers (`js_runtime/mod.rs:239-247`); `init_runtime_extension` (`ops/mod.rs:2038-2045`) installs `clay_runtime_trusted_extension` (82 ops, `:1866`) vs `clay_runtime_package_extension` (44 ops, `:1943-2016`). Third-party subset of trusted; admin ops absent including `load_package*`, `language_server_authorize`, config/documents/workspace/keybindings/theme/shell (`domain_extension_tests` `:2074-2123`). Runtime probe: `third_party_runtime_cannot_see_trusted_ops_or_admin_modules` (`js_runtime/mod.rs:2513`). Import of `clay:packages` denied at facade: `third_party_package_cannot_load_other_packages` (`:2479`).
    - Module roots today (`source.rs:79-142`): facades → `markdown-it` → load-entry allowlist → `resolve_relative` package-root confine (`ops/packages.rs:127-148`, `../` → `None`) → **Trusted-only** config resolve (`source.rs:133`) → deny `runtime.invalid_import`. Facades partitioned (`facades.rs:4-6, 105-110`): TrustedOnly = configuration/documents/workspace/keybindings/packages/application/editor/shell/theme; Public = 13 contribution facades. 27.4 insert point: **after facades, trusted-domain only, inventory-listed paths only**, then existing deny. No extra IPC; lookup + file read at load time.
    - Cross-domain: `cross_domain.rs` typed inert envelopes only — no V8 objects/functions/promises/globals/modules (`:1-12`). Status vocab includes stale/revoked (`:31-38`). Requester must be ThirdParty (`:185`). Existing isolation tests stay green: `third_party_provider_executes_in_third_party_runtime_only` (`:1941`), `slow_third_party_provider_poisons_only_third_party_domain` (`:2026`), `trusted_reload_preserves_third_party_providers` (`:2108`), `third_party_poison_replays_approved_graph_and_restores_providers` (`:2208`), `cross_domain_load_bridge_rejects_trusted_records` (`:2316`), `package_load_entry_allowlist_revokes_owned_entries` (`:10107`), `failed_replacement_keeps_previous_generation_active` (`tests/package_loading.rs:1295`). Third-party is one disclosed shared cohort — no sibling isolation planned.
    - Process authority unchanged: NativeTrust authorization **filters out** `LanguageServer` unless a current contribution/root grant exists (`service.rs:967-976`). `op_clay_language_server_authorize` is trusted-only and still grant-gated. Session start/send/read/stop exist in both extensions but select only an already-approved contribution/root — no runtime exe/argv/cwd/env. 27.5 is JS factory only; bundled trust / `loadPackage` never imply `language-server`.
    - Binding contract for later tasks:
      | Later task | May add | Must not |
      | --- | --- | --- |
      | 27.7 inventory gen | `build.rs` emit of current 14 loadable entries | Promote `lsp-shared` to `loadPackage`; classify by name |
      | 27.4 shared imports | Trusted-only inventory `exports` resolve; delete vendored `dist/shared/` | Third-party import of those specifiers; `../` or absolute escape; npm `file:` / node_modules |
      | 27.5 LSP factory | JS `createLspBridge` in `lsp-shared` | New process grant; implicit LS from trust/load; runtime-selected exe/argv/cwd/env |
    - 27.4 still owns the three new deny tests listed above. This task only locked the contract.

- [x] Phase 27.7: Generate the bundled inventory at build time
  - Acceptance Criteria:
    - Functional: `BUNDLED_PACKAGES` (name, version, root, FNV-1a-64 fingerprint) is generated in `build.rs` from a checked-in package list plus each listed `packages/<root>/package.json`. Adding/editing a first-party package is a list entry + package tree; no hand-computed hashes in `src/packages/bundled.rs`.
    - Performance: Generation is compile-time only; runtime lookup unchanged (`bundled_entry`, domain classify).
    - Code Quality: Existing inventory-matches-source-tree test remains the trust gate and fails closed on drift. `packages/lsp-shared` stays non-loadable (no `loadEntry` / not in `loadPackage` inventory) unless listed as a helper in a later 27.4 edit.
    - Security: Trust root stays the checked-in tree + test; FNV is still drift detection, not a crypto hash. `@clay/*` naming still cannot promote into trusted.
  - Approach:
    - Documentation Reviewed:
      - Cargo book build scripts: https://doc.rust-lang.org/cargo/reference/build-scripts.html (`cargo::rerun-if-changed`, `OUT_DIR`).
      - https://doc.rust-lang.org/cargo/reference/build-script-examples.html (`include!(concat!(env!("OUT_DIR"), "/..."))`).
      - Local `build.rs` (Windows manifest only today); `src/packages/bundled.rs`.
    - Options Considered:
      - Auto-scan `packages/*`: rejected — a stray directory would become trusted.
      - Keep hand hashes: rejected — that is the tax this task deletes.
    - Chosen Approach:
      - Checked-in list (TOML or similar) of loadable package roots. `build.rs` reads each `package.json`, writes generated entries, reruns on list + those manifests.
    - API Notes and Examples:
      ```rust
      // build.rs
      println!("cargo::rerun-if-changed=src/packages/bundled-inventory.toml");
      println!("cargo::rerun-if-changed=packages/rust/package.json");
      let dest = Path::new(&env::var("OUT_DIR")?).join("bundled_packages.rs");
      fs::write(dest, generated)?;

      // src/packages/bundled.rs
      include!(concat!(env!("OUT_DIR"), "/bundled_packages.rs"));
      ```
    - Files to Create/Edit:
      - `src/packages/bundled-inventory.toml`: checked-in loadable package list (name optional; root required).
      - `build.rs`: generate inventory; keep existing Windows manifest link args.
      - `src/packages/bundled.rs`: replace the 14-entry literal with `include!`; keep `RuntimeDomain`, `fnv1a64_hex`, classify, tests.
    - References:
      - Cargo 1.96 `cargo::rerun-if-changed` (legacy `cargo:` form still works; prefer new form).
      - `src/packages/bundled.rs` inventory-matches-source-tree test.
  - Test Cases to Write:
    - Existing fingerprint drift test still fails if a listed `package.json` changes without rebuild.
    - Unlisted `packages/extra/` is not trusted even if it has a `package.json`.
    - `lsp-shared` remains absent from the loadable inventory after this task.

  - Completion Evidence (2026-08-19):
    - Docs: Cargo book build scripts (`cargo::rerun-if-changed`, `OUT_DIR`, `include!(concat!(env!("OUT_DIR"), "/bundled_packages.rs"))`) via ctx7 `/websites/doc_rust-lang_cargo`. Windows `build.rs` link args left as legacy `cargo:` form.
    - Checked-in allowlist: `src/packages/bundled-inventory.toml` — 14 `root = "..."` lines. Parser is line-based in `build.rs` (no `toml` crate). `packages/lsp-shared` unlisted.
    - `build.rs` reads each listed `packages/<root>/package.json` (requires `name`, `version`, `clay`), emits FNV-1a-64 entries to `$OUT_DIR/bundled_packages.rs`, `cargo::rerun-if-changed` on the list + each manifest. `serde_json` added as `[build-dependencies]` only (already a crate dep).
    - `src/packages/bundled.rs` replaced the 14-entry literal with `include!`. `RuntimeDomain`, `verify_bundled_trust`, `fnv1a64_hex`, classify unchanged.
    - Tests: `inventory_matches_source_tree` now binds listed roots to `bundled-inventory.toml` + live name/version/fingerprint. New `unlisted_package_dirs_are_not_trusted` asserts `lsp-shared` exists on disk, is absent from the list, and no unlisted `packages/*` name is trusted. Existing spoof/tamper/extension-point tests still pass.
    - Linux: `cargo fmt --check`, `cargo check --all-targets`, `cargo clippy --all-targets -- -D warnings`, `cargo test --lib packages::bundled` (6 passed).
    - Trust contract unchanged: classify by compiled inventory + exact provenance, never `@clay/*` name. 27.4 may add exports; must not add `lsp-shared` as a loadable entry.

- [x] Phase 27.1: Single-manifest loading and execute-only load entries
  - Acceptance Criteria:
    - Functional: `loadPackage("@clay/{markdown,rust,typescript,javascript}")` still registers modes (with `editorRules`), commands, completions, UI status items, and (for markdown) the Tier 3 parse-handler module — with **no** `serverRegister*` / `serverActivateMajorMode` ceremony in those load entries. Delete `*PackageManifest()` literals from `@clay/markdown` and `@clay/typescript` (`dist/index.js` / `load.js`). Delete markdown's hardcoded `documentId: 1` / `sample.md` activation. Imperative register APIs remain public for `init.js` and runtime contributions.
    - Performance: Apply-record runs once per `loadPackage` / generation; no paint/keypress work; existing load tests stay within current timeouts.
    - Code Quality: `clay.contributions.modePatterns` parsed into `PackageRecord` including `editorRules`. One generic `apply_package_record_contributions` used by `loadPackage`. First-party load entries contain only executable wiring (markdown: import `parser.js` + `serverRegisterParseHandler`; language packages: empty or a no-op default export if still required).
    - Security: Apply-record uses the host-enabled record only (same as empty-args ops today). No caller-supplied grammar/mode JSON. No new authority.
  - Approach:
    - Documentation Reviewed:
      - Decision `2026-08-18-1758-single-manifest-package-loading.md`.
      - `docs/reference/packages/creating-packages.md` load contract.
      - `src/server/ops/syntax.rs`, `src/server/ops/modes.rs`, `src/server/ops/completion.rs`.
    - Options Considered:
      - Keep empty-args calls as implicit apply: rejected by the decision.
      - Per-package Rust apply: rejected — one generic function.
    - Chosen Approach:
      - Extend record parsing for `modePatterns` + `editorRules`. Call apply-record from `op_clay_packages_load_package_by_specifier` after enable (trusted and, via the existing third-party bridge, adopted packages). Strip first-party load-entry register calls. Move `editorRules` that today live only in JS into `package.json` modePatterns (or equivalent contribution) so apply-record has data.
    - API Notes and Examples:
      ```js
      // packages/rust/dist/load.js after
      export default async function loadRustPackage() {}

      // packages/markdown/dist/load.js after (execute only)
      import { serverRegisterParseHandler } from "clay:parse";
      import * as parserModule from "./parser.js";
      export default async function loadMarkdownPackage() {
        await serverRegisterParseHandler({
          mode: "markdown",
          module: parserModule,
          exportName: "parseMarkdownDecorationUpdate",
          adapter: "./dist/parser.js",
          // budgets already in the manifest / record
        });
      }
      ```
    - Files to Create/Edit:
      - `src/packages/record/mod.rs` (+ language/behavior parser as needed): `modePatterns`.
      - `src/server/ops/packages.rs` or a new `src/packages/apply.rs`: apply-record.
      - `packages/{rust,typescript,javascript,markdown}/package.json`: add `editorRules` on mode patterns; keep other contributions.
      - `packages/{rust,typescript,javascript,markdown}/dist/load.js` and `dist/index.js`: delete duplicates / register ceremony.
      - Tests: `src/server/js_runtime/mod.rs` load-package suites; `tests/package_loading.rs`.
    - References:
      - `packages/markdown/dist/load.js` `markdownPackageContract` / `serverActivateMajorMode`.
      - `packages/typescript/dist/load.js` `typescriptPackageManifest`.
      - Pattern `package-manifest-single-source.md`.
  - Test Cases to Write:
    - `load_package_without_load_entry_register_calls_activates_rust_mode_and_completions`.
    - `load_package_markdown_does_not_activate_hardcoded_document_1`.
    - First-party load entries contain no `serverRegister(SyntaxGrammar|ModePattern|Command|CompletionProvider|ComponentContribution)` and no `*PackageManifest`.
    - `init.js` can still call `serverRegisterModePattern` after `loadPackage` (imperative path alive).
    - Existing `load_package_markdown_default_activates_full_mode_from_init_js` and grammar-register tests stay green.

  - Completion Evidence (2026-08-19):
    - `PackageContributions.mode_patterns` parsed from `clay.contributions.modePatterns` including `editorRules` JSON (`src/packages/record/behavior.rs`). Unit: `package_record_parses_mode_patterns_and_editor_rules`.
    - `apply_package_record_contributions` runs in `op_clay_packages_load_package_by_specifier` for **Trusted** packages only (modes, commands, syntax, completion, UI `{kind,id}`). Third-party load entries still register themselves — applying in the trusted worker duplicated providers.
    - `serverActivateMajorMode` falls back to host-record editorRules/commands/keymaps when the JS `activationRegistry` is empty. `serverActivateClassifiedMode` no longer requires a JS registerModePattern cache entry.
    - First-party `packages/{rust,typescript,javascript,markdown}/package.json` modePatterns now carry editorRules. Those four `dist/load.js` files are execute-only (markdown: parse-handler import only). Deleted `*PackageManifest()` from their `dist/index.js`.
    - Tests: `load_package_without_load_entry_register_calls_activates_rust_mode_and_completions`, `load_package_markdown_does_not_activate_hardcoded_document_1`, `init_js_can_still_register_mode_pattern_after_load_package`. Existing loadPackage / grammar / language-command tests green. Linux: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`.
    - Default-load tests that needed a behavior manifest now classify+activate a real path after `loadPackage` (no documentId 1).

- [x] Define and verify the package default init.js loading experience
  - Acceptance Criteria:
    - Functional: Preferred user setup remains one line per package (`loadPackage("@clay/markdown")` etc.). After 27.1, that line still yields default mode/commands/completion/syntax/parse without copied manifests or primitive-by-primitive plumbing. Customization via existing Clay/package JS APIs stays optional.
    - Performance: One-line load stays within existing configuration-eval timeout.
    - Code Quality: Authoring docs state execute-only load entries; any longer setup is documented as a fallback, not the convention.
    - Security: One-line load still does not grant filesystem/network/shell/`language-server`.
  - Approach:
    - Documentation Reviewed:
      - Decision `2026-06-09-0219-explicit-init-js-package-loading-with-one-line-defaults.md`.
      - Pattern `package-distribution.md`.
      - `examples/init.js`, `docs/reference/clay-js-api/packages/load-package.md`.
    - Options Considered:
      - Silent default-on packages: rejected — explicit load stays required.
    - Chosen Approach:
      - Reuse existing one-line path; prove it after apply-record lands.
    - API Notes and Examples:
      ```js
      import { loadPackage } from "clay:packages";
      await loadPackage("@clay/markdown");
      ```
    - Files to Create/Edit:
      - `docs/reference/packages/creating-packages.md` (load-entry contract).
      - Existing load-package tests / fixtures.
    - References:
      - `docs/reference/clay-js-api/packages/load-package.md`
  - Test Cases to Write:
    - Default-load fixture with only `loadPackage` lines (no extra register calls) activates markdown + rust.

  - Completion Evidence (2026-08-19):
    - `examples/packages/first-party.js` stays one `loadPackage` line per package. Authoring now says host applies `package.json`; `loadEntry` is execute-only (`docs/reference/packages/creating-packages.md`, `docs/reference/clay-js-api/packages/load-package.md`, first-party rust/ts/js/markdown package pages).
    - Test `default_init_js_load_package_lines_activate_markdown_and_rust`: init.js has only two `loadPackage` lines; asserts markdown parse handler + `markdown.keywords` + `rust.keywords`; elapsed < `JS_RUNTIME_EVALUATION_TIMEOUT_MS`; `document_analyzers` empty (no language-server / analyzer grant).
    - `package_author_guide_uses_public_facades_not_raw_ops` now requires the `execute-only` marker. Linux: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`.

- [x] Phase 27.2: Native grammar ownership cleanup
  - Acceptance Criteria:
    - Functional: First-party packages whose grammars are in `FIRST_PARTY_NATIVE_GRAMMARS` (`rust`, `typescript`/`tsx`, `javascript`, `markdown`) drop `clay.contributions.syntaxGrammars` (including dead `styleMap` / `queries` paths). Registering a shadowed native grammar is a diagnostic ("owned by native descriptor"), not a silent skip. `clay package inspect` / `PackageInspection` surfaces that ownership.
    - Performance: Native pre-register path unchanged; no extra parse work.
    - Code Quality: Query files stay `include_str!`-sourced from the package tree. Long-term inversion (style maps from trusted package records) is documented as a future decision, not implemented.
    - Security: No new grammar-loading authority; Tier 2/3 packages still declare grammars in the manifest. No language-specific Rust branches.
  - Approach:
    - Documentation Reviewed:
      - Decision `2026-08-18-1758-single-manifest-package-loading.md` (alternative 2 deferred).
      - Pattern `language-capability-sequencing.md`.
      - `src/server/syntax.rs` `is_shadowed_by_native_first_party`; `src/server/ops/syntax.rs`.
    - Options Considered:
      - Invert ownership now: rejected — deferred by the decision until third-party grammars.
      - Keep dead package.json copies: rejected — silent drift.
    - Chosen Approach:
      - Delete the inert blocks; make the skip visible; document inversion as follow-up.
    - API Notes and Examples:
      ```text
      clay package inspect @clay/rust
      # Syntax: rust — owned by native descriptor (FIRST_PARTY_NATIVE_GRAMMARS)
      ```
    - Files to Create/Edit:
      - `packages/{rust,typescript,javascript,markdown}/package.json`: drop `syntaxGrammars`.
      - `src/server/syntax.rs` / `src/server/ops/syntax.rs`: diagnostic on shadow.
      - `src/packages/service.rs`, `src/launch.rs`: inspect field.
      - `docs/reference/packages/creating-packages.md`, `docs/wiki/modules/syntax-grammar-registry.md`.
    - References:
      - `src/server/syntax.rs:282` `FIRST_PARTY_NATIVE_GRAMMARS`.
      - Phase 26.2 note that package-side style maps for native grammars are dead.
  - Test Cases to Write:
    - Native-owned package.json has no `syntaxGrammars` (guard).
    - `serverRegisterSyntaxGrammar` on a native-owned package returns a diagnostic mentioning native ownership; live grammar stays the descriptor.
    - Inspect output includes the ownership note.
    - Existing query-contract / fixture highlight tests stay green.

  - Completion Evidence (2026-08-19):
    - Dropped `clay.contributions.syntaxGrammars` from `@clay/{rust,typescript,javascript,markdown}` `package.json`. Queries still `include_str!` from the package tree via `FIRST_PARTY_NATIVE_GRAMMARS`.
    - Shadowed register is `SyntaxGrammarRegistryError::OwnedByNativeDescriptor` (not silent skip). `serverRegisterSyntaxGrammar` on a native-owned prefix returns `syntax.owned_by_native_descriptor`. Live grammar stays `builtin` / Native.
    - `PackageInspection.native_syntax_languages` + `clay package inspect` print `Syntax: … — owned by native descriptor (FIRST_PARTY_NATIVE_GRAMMARS)`.
    - Tests: `native_owned_first_party_package_json_omits_syntax_grammars`, `registering_shadowed_native_grammar_returns_ownership_diagnostic`, `server_register_syntax_grammar_on_native_owned_package_returns_ownership_diagnostic`, inspect assertion on `@clay/markdown`. Fixture highlight / smoke tests now use `with_first_party_native()`. Linux: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`.
    - Docs: creating-packages + syntax-grammar-registry record inversion as future decision, not implemented.

- [x] Phase 27.3: Capability presets
  - Acceptance Criteria:
    - Functional: Optional `clay.preset` of `code-mode` | `prose-mode` | `lsp-bridge` expands at `validate_manifest_value` into the standard permission, `apiDependencies`, extension-point, and contribution-family sets. Explicit keys win. Expanded set is what is validated, budgeted, stored on the record, and shown by inspect. First-party `@clay/rust`, `@clay/typescript`, `@clay/javascript`, `@clay/markdown`, `@clay/lsp-*` migrate to a preset plus deviations only.
    - Performance: Expansion is load/validate time; payload budget still applies to the expanded JSON.
    - Code Quality: Additive schema (`clay.preset` optional). No `manifestSchemaVersion` field (YAGNI). Preset tables live in Rust + authoring docs. Unknown preset is a validation error.
    - Security: Presets never grant extra authority beyond the expanded permission set; `language-server` is still deny-by-default and not implied by `lsp-bridge` load.
  - Approach:
    - Documentation Reviewed:
      - Decision `2026-08-18-1758-package-capability-presets.md`.
      - `src/packages/manifest.rs` `validate_manifest_value`.
      - Current near-identical blocks in `packages/{rust,typescript,javascript,markdown,lsp-*}/package.json`.
    - Options Considered:
      - Per-family granules: rejected by the decision (reintroduces combinatorics).
      - User-facing schema version field: rejected — additive optional key is enough.
    - Chosen Approach:
      - Expand in `validate_manifest_value` before permission/graph/extension-point parse. Store both declared preset and expanded permissions on the record for inspect.
    - API Notes and Examples:
      ```json
      {
        "name": "@clay/rust",
        "clay": {
          "apiPrefix": "rust",
          "preset": "code-mode",
          "modes": ["rust"],
          "entry": "./dist/index.js",
          "loadEntry": "./dist/load.js",
          "docs": "./docs/index.md",
          "contributions": {
            "modePatterns": [{ "mode": "rust", "extensions": ["rs"], "fileNames": ["Cargo.toml"] }],
            "commands": [{ "id": "rust.toggleLineComment", "displayName": "Toggle Rust Line Comment", "routingPolicy": "server-first" }]
          }
        }
      }
      ```
    - Files to Create/Edit:
      - `src/packages/manifest.rs`: preset parse/expand.
      - `src/packages/service.rs`, `src/launch.rs`: inspect `preset` + expanded permissions.
      - `packages/{rust,typescript,javascript,markdown,lsp-*}/package.json`: migrate.
      - `docs/reference/packages/creating-packages.md`: preset table, override rules, migration.
      - Generated registry / `api-inventory.toml` if a documented API/option is added.
    - References:
      - Pattern `package-manifest-single-source.md`.
  - Test Cases to Write:
    - `code-mode` without explicit permissions expands to the documented set.
    - Explicit permission list intersecting a preset: union with explicit wins / documented override rule.
    - Unknown preset fails validation.
    - `lsp-bridge` expand does not auto-approve `language-server`.
    - Inspect of `@clay/rust` shows `preset: code-mode` and the expanded permission list.
    - Payload-too-large still measured on expanded content.

  - Completion Evidence (2026-08-19):
    - Optional `clay.preset` (`code-mode` / `prose-mode` / `lsp-bridge`) expands in `expand_capability_preset` before validate/assemble. Override rule: missing `permissions` get the preset list; explicit `permissions` replace it; `apiDependencies` union; explicit `extensionPoints` replace generated points. No `manifestSchemaVersion`.
    - `language-server` never injected. `lsp-bridge` expands parse/completion/render only. First-party `@clay/lsp-*` keep `capabilities: ["language-server"]` plus `language-server.startLanguageServerSession` as the explicit deviation.
    - Migrated `@clay/{rust,typescript,javascript}` to `code-mode`, `@clay/markdown` to `prose-mode`, four `lsp-*` to `lsp-bridge`. Dropped copied permission / apiDependency / extension-point blocks.
    - `PackageInspection.preset` + `clay package inspect` print `Preset:`. Inspect of installed `@clay/rust` shows `code-mode` and expanded `parse-document`.
    - Tests: `code_mode_preset_expands_permissions_and_extension_points`, `explicit_permissions_replace_preset_defaults`, `unknown_preset_fails_validation`, `lsp_bridge_preset_does_not_request_language_server`, `preset_expansion_counts_toward_payload_budget`, `assemble_expands_code_mode_api_dependencies`, `assemble_lsp_bridge_without_capability_does_not_grant_language_server`, `rust_package_inspect_shows_code_mode_preset_and_expanded_permissions`. Linux: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`.
    - Docs: creating-packages preset table + override rules; package-loading wiki/primitive; rust/ts/js/md contract lines.

- [x] Phase 27.4: First-party package-to-package dependency resolution
  - Acceptance Criteria:
    - Functional: Trusted module loader resolves inventory `exports` specifiers (e.g. `lsp-shared/client.js`) to files under that helper's canonical root. `@clay/lsp-*` import shared modules instead of `./shared/...`. Delete the four `packages/lsp-*/dist/shared/` trees and `scripts/update-first-party-lsp-shared.mjs`.
    - Performance: Resolve is allowlist lookup; no extra stat on hot path.
    - Code Quality: `lsp-shared` is a bundled **helper** (fingerprinted, exported, not `loadPackage`-able). Guard test: first-party packages contain no vendored copy of another first-party package's modules.
    - Security: Trusted domain only; no third-party import; no path escape; export map is the allowlist (not the whole helper directory).
  - Approach:
    - Documentation Reviewed:
      - `src/server/js_runtime/source.rs` resolve/load.
      - `src/packages/bundled.rs` (lsp-shared intentionally absent today).
      - `scripts/update-first-party-lsp-shared.mjs`.
    - Options Considered:
      - Keep the copy script: rejected — four copies is the bug.
      - npm `file:` deps through the package manager: rejected — runtime cannot reach node_modules; inventory exports stay the trust gate.
    - Chosen Approach:
      - Extend the 27.7 list with a helper entry + export paths. Loader branch after `clay:*` / before deny: trusted + exact export specifier → recorded allowlist path under the helper root.
    - API Notes and Examples:
      ```js
      import { LspClient } from "lsp-shared/client.js";
      import { completionToClay } from "lsp-shared/mapping.js";
      ```
    - Files to Create/Edit:
      - `src/packages/bundled-inventory.toml`: helper + exports.
      - `build.rs` / generated inventory: export map.
      - `src/server/js_runtime/source.rs`: resolve/load.
      - `packages/lsp-{rust,markdown,typescript,javascript}/dist/**`: switch imports; delete `dist/shared/`.
      - `scripts/update-first-party-lsp-shared.mjs`: delete; drop any CI `--check` hook.
      - `packages/lsp-shared/package.json`: optional identity/exports documentation (still private / not loadable).
    - References:
      - Trust-domain task; `ClayModuleLoader` relative-root confine as the escape model to copy.
  - Test Cases to Write:
    - First-party tree has no `dist/shared/` (guard).
    - Trusted resolve of each exported `lsp-shared/*.js` file succeeds.
    - Third-party and path-escape cases from the trust-domain task.
    - Existing LSP load tests (`lsp_rust_package_loads_after_exact_grant_without_starting_child`, `tests/lsp_bridge.rs`) stay green.

  - Completion Evidence (2026-08-19):
    - `bundled-inventory.toml` adds helper `lsp-shared` plus 6 exact exports. `build.rs` emits `BUNDLED_HELPERS` + `BUNDLED_HELPER_EXPORTS`. Helper is fingerprinted, not in `BUNDLED_PACKAGES`, not `loadPackage`-able.
    - Trusted `ClayModuleLoader` resolves exact `lsp-shared/*.js` to `clay://packages/lsp-shared/...` and records the helper root on the allowlist. Relatives stay confined. Third-party / unexported / `../` escape deny.
    - `@clay/lsp-*` `server.js` imports inventory specifiers. Deleted four `dist/shared/` trees and `scripts/update-first-party-lsp-shared.mjs`.
    - Node package tests resolve those specifiers via `--import tests/fixtures/lsp/register-lsp-shared.mjs` (Node has no Clay loader).
    - Tests: `clay_module_loader_resolves_trusted_helper_exports`, `clay_module_loader_denies_helper_exports_outside_trusted_domain`, `clay_module_loader_denies_unexported_and_escaping_helper_specifiers`, `first_party_packages_do_not_vendor_helper_modules`, existing `lsp_*` load + `lsp_bridge` suites. Linux: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`.
    - Docs: creating-packages, first-party-lsp-bridge-packages, package index pages.

- [x] Phase 27.5: LSP bridge factory consolidation
  - Acceptance Criteria:
    - Functional: One `createLspBridge({ server, languageId, diagnostics: "push"|"pull", features })` in `lsp-shared` absorbs the shared body of `@clay/lsp-rust` and `@clay/lsp-markdown` (capabilities, `TOKEN_TYPES`/`TOKEN_MODIFIERS`, document tracking, refresh/completion/intelligence). Those two packages become config + manifest + thin load entry like `lsp-typescript` / `lsp-javascript`. New language-server adoption is one `languageServers` contribution + one factory config object.
    - Performance: No extra child processes; session start still lazy on document open; existing worker/frame ceilings unchanged.
    - Code Quality: No language-named Rust. Factory is JS in `lsp-shared`.
    - Security: No new grant surface. Fixed-contribution executable/argv/env/root rules unchanged. Bundled trust still does not auto-grant `language-server`. Replacement still does not inherit the target's grant.
  - Approach:
    - Documentation Reviewed:
      - `packages/lsp-typescript/dist/server.js` (40-line factory wrapper — the target shape).
      - `packages/lsp-rust/dist/server.js` / `packages/lsp-markdown/dist/server.js` (~330 lines each).
      - `packages/lsp-shared/typescript-language-server.js` (existing TS factory).
      - Decision `2026-07-14-2023-language-server-package-authority.md`.
    - Options Considered:
      - Separate factories per diagnostic mode: rejected — one factory, one option.
      - Move factory into Rust: rejected — protocol framing stays package-side.
    - Chosen Approach:
      - Generalize the TS factory (or add `createLspBridge` next to it and make the TS helper a wrapper). Point rust/markdown at it. Keep per-package `languageServers` contribution and grant checks.
    - API Notes and Examples:
      ```js
      import { createLspBridge } from "lsp-shared/bridge.js";
      export function handleDocumentAnalysis(event) {
        return defaultBridge.handle(event);
      }
      ```
    - Files to Create/Edit:
      - `packages/lsp-shared/bridge.js` (or extend `typescript-language-server.js` if it is already generic enough — prefer one file).
      - `packages/lsp-{rust,markdown}/dist/server.js`, `dist/load.js`, `package.json`.
      - `docs/reference/packages/creating-packages.md` LSP factory section.
      - `docs/wiki/modules/first-party-lsp-bridge-packages.md`.
    - References:
      - Pattern `language-capability-sequencing.md`.
      - `tests/lsp_bridge.rs`, `tests/lsp_real_servers.rs`.
  - Test Cases to Write:
    - Rust and markdown bridges produce the same capability/token tables as today (snapshot or equality with the factory defaults).
    - Load still registers analyzer metadata only (no child until open + grant).
    - Missing/revoked grant still degrades; no auto-start.

  - Completion Evidence (2026-08-19):
    - `packages/lsp-shared/bridge.js` adds `createLspBridge({ languageId | languageIds+languageIdsByExtension, diagnostics: "push"|"pull", features })`. Shared body: capabilities, `DEFAULT_TOKEN_TYPES`/`DEFAULT_TOKEN_MODIFIERS`, document tracking, semantic refresh (full + advertised delta), pull diagnostics, completion, intelligence.
    - `@clay/lsp-rust` / `@clay/lsp-markdown` `server.js` are thin wrappers (`diagnostics: "pull"` + `languageId: "rust"`; push + markdown extension map + no signatureHelp). TS helper wraps the same factory.
    - Inventory export `lsp-shared/bridge.js`. No new grant surface. Existing grant-before-load tests still prove no child until open + grant.
    - Tests: adapter factory token/capability tables + identity reject; `lsp_bridge` rust/markdown/ts/js suites + fake-server matrix; `lsp_*` load-without-child. Linux: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`.
    - Docs: creating-packages factory section, first-party-lsp-bridge-packages wiki.

- [x] Phase 27.6: One syntax vocabulary — Tier 3 migration and compat demotion
  - Acceptance Criteria:
    - Functional: `@clay/markdown/dist/parser.js` emits closed `TokenType`+`Modifiers` (headings, emphasis, link parts, fences) instead of `markup.*` style tokens. `style_token` / `from_style_token` / `classify_style_token` / `scope` escape remain for old packages but are documented as frozen/deprecated.
    - Performance: Same parse window/budgets; no payload growth beyond existing decoration budget tests.
    - Code Quality: First-party producers emit no free-form style tokens (guard). Themes still key one table.
    - Security: No new parse authority; handler still first-party-only token-backed.
  - Approach:
    - Documentation Reviewed:
      - `docs/reference/primitives/syntax-vocabulary.md`.
      - `src/protocol/decorations.rs` `classify_style_token` / `from_style_token`.
      - `packages/markdown/dist/parser.js` `STYLE_TOKENS` / `syntaxSpan`.
    - Options Considered:
      - Delete compat now: rejected — old packages must still render.
      - Keep first-party on `markup.*`: rejected — two vocabularies is the bug.
    - Chosen Approach:
      - Change the markdown producer; mark compat APIs deprecated in docs + rustdoc; keep baseline-locked color tests for the compat mapper.
    - API Notes and Examples:
      ```js
      // before
      syntaxSpan(source, start, end, "markup.heading.1", priority)
      // after
      syntaxSpan(source, start, end, { tokenType: "Heading1", modifiers: [] }, priority)
      ```
    - Files to Create/Edit:
      - `packages/markdown/dist/parser.js` (+ tests/fixtures that assert span shape).
      - `src/protocol/decorations.rs`: rustdoc deprecation on compat fns (keep `#[allow]` if needed; do not `#[deprecated]` if that breaks `-D warnings` without a tracked allow — prefer rustdoc + docs).
      - `docs/reference/primitives/syntax-vocabulary.md`, `creating-packages.md`.
    - References:
      - Pattern `language-capability-sequencing.md`.
  - Test Cases to Write:
    - Markdown Tier 3 fixture spans use `Heading1`… / `Modifiers::BOLD` etc., not `markup.*`.
    - Guard: no first-party `packages/**` producer emits `markup.` style tokens.
    - Compat mapper tests (`free_form_style_token_decoration_colors_baseline_locked`) still pass.

  - Completion Evidence (2026-08-19):
    - `@clay/markdown/dist/parser.js` emits `tokenType` + `modifiers` (`Heading1`…, `Paragraph`+Bold/Italic, `CodeSpan`, `CodeBlock`, `ListItem`, `Link`). No first-party `markup.*` / `styleToken` producer.
    - `classify_style_token` / `from_style_token` / `scope` stay as frozen compat for old packages (rustdoc + syntax-vocabulary + creating-packages). No `#[deprecated]`.
    - Tests: adapter fixtures assert closed names; `first_party_js_producers_emit_no_free_form_markup_style_tokens`; `free_form_style_token_decoration_colors_baseline_locked` still passes. Linux: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`.

- [x] Phase 27.8: Consolidation hardening, inspect fields, and authoring contract
  - Acceptance Criteria:
    - Functional: Behavior parity for activation, keymaps, commands, completion, and syntax across all first-party language + LSP packages vs the 27.1–27.6 end state. Load-order and hot-reload still honor execute-only load entries. `clay package inspect` shows preset + expanded permissions + native-grammar ownership.
    - Performance: Existing load/reload/eval budgets non-regressed; no new hot-path JS.
    - Code Quality: `docs/reference/packages/creating-packages.md` rewritten for single-manifest, presets, shared imports, LSP factory, execute-only load entries, migration notes, permissions, tests. Registry freshness holds.
    - Security: Trust-domain and process-authority tests from earlier tasks still pass. Authoring guide does not describe children as sandboxed.
  - Approach:
    - Documentation Reviewed:
      - Pattern `package-ui-layout.md` (authoring contract).
      - Decision `2026-06-09-1431-clay-owned-shell-layout-and-package-ui-contribution-model.md`.
      - `docs/reference/packages/creating-packages.md`.
    - Options Considered:
      - New settings-panel inspect UI: rejected — CLI + `PackageInspection` is the surface; JS `packages.inspect` stays planned.
    - Chosen Approach:
      - Parity tests + docs/inspect in one hardening pass after the behavioral tasks.
    - API Notes and Examples:
      ```text
      clay package inspect @clay/markdown
      clay package inspect @clay/lsp-rust
      ```
    - Files to Create/Edit:
      - `src/launch.rs`, `src/packages/service.rs` (inspect formatting if not finished in 27.2/27.3).
      - `docs/reference/packages/creating-packages.md`.
      - `docs/index.md` links if new pages.
      - Parity tests in `src/server/js_runtime/mod.rs` / `tests/package_loading.rs`.
    - References:
      - Pattern `documentation-as-code.md`, `doc-registry-tests.md`.
  - Test Cases to Write:
    - Reload generation: execute-only markdown load entry re-registers the parse handler; no double-register panic.
    - Inspect snapshot for rust + markdown + one lsp package.
    - Authoring-doc coverage: creating-packages mentions preset names and execute-only load entries (`tests/package_loading_docs.rs` or successor).

  - Completion Evidence (2026-08-19):
    - `PackageService::inspect_bundled_inventory` + CLI fallback: isolated-HOME `clay package inspect` works without pnpm. Store inspect still preferred when present. Adopt/revoke stay store-only.
    - Inspect prints preset, expanded permissions, native syntax ownership. Snapshots: rust `code-mode` + `rust`, markdown `prose-mode` + `markdown`, lsp-rust `lsp-bridge` + `language-server` (no native syntax). `lsp-shared` not inspectable.
    - Reload: `trusted_reload_reruns_markdown_execute_only_load_entry` re-registers parse handler on generation 2 without panic. Same-generation idempotency already covered.
    - Authoring contract section in `creating-packages.md`; docs tests require execute-only, presets, `createLspBridge`, inspect.
    - Linux: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`.

- [x] Create or verify Clay JS APIs for public programmatic surfaces
  - Acceptance Criteria:
    - Functional: Inventory Rust `pub` functions this phase adds/changes. Public programmatic ones get `deno_core` op + `clay:*` facade + Markdown docs + index link + registry. Internal ones become `pub(crate)`. No new user-facing `Deno.core.ops.op_*`. Dotted IDs follow `clay-js-api-naming.md` (core `packages.*` / `syntax.*`; package IDs keep `apiPrefix`).
    - Performance: Docs/registry generation is offline; no runtime lookup cost on hot path.
    - Code Quality: `cargo test` fails on missing API/doc/index/registry/keybinding/custom-property/lookup fields. `packages.inspect` remains planned unless a new public JS surface is truly required (CLI inspect is enough).
    - Security: Facades do not expose inventory internals, fingerprints, or trusted module roots to third-party callers.
  - Approach:
    - Documentation Reviewed:
      - Patterns `clay-js-api-boundary.md`, `clay-js-api-naming.md`, `clay-js-api-schema.md`, `documentation-as-code.md`, `doc-registry-tests.md`.
      - Decisions `2026-05-08-1509`, `2026-05-08-1840`.
    - Options Considered:
      - Implement `packages.inspect` now: rejected unless a JS caller needs preset fields; CLI already prints them.
      - New `packages.applyContributions` JS API: rejected — apply-record is internal to `loadPackage`.
    - Chosen Approach:
      - Likely no new callable APIs. Verify existing `loadPackage` / register / validate docs mention execute-only load entries and presets. Demote any accidental new `pub` to `pub(crate)`.
    - API Notes and Examples:
      ```js
      import { loadPackage } from "clay:packages";
      await loadPackage("@clay/rust");
      ```
    - Files to Create/Edit (tentative — finalize from the inventory):
      - `docs/reference/clay-js-api/packages/load-package.md`
      - `docs/reference/clay-js-api/syntax/server-register-syntax-grammar.md` (if present)
      - `docs/index.md`, `docs/reference/clay-js-api/api-inventory.toml`
      - `cargo run --bin update-doc-registry` when docs change
    - References:
      - `RESERVED_CORE_API_DOMAINS` in `src/packages/manifest.rs`.
  - Test Cases to Write:
    - Existing API-doc / registry freshness tests stay green.
    - No new undocumented `pub` server functions.

  - Completion Evidence (2026-08-19):
    - No new public JS API. `inspect_bundled_inventory` is Rust/CLI only. `packages.inspect` stays planned. Existing `loadPackage` / register docs already mention execute-only + presets.

- [x] Create or verify Clay configuration APIs
  - Acceptance Criteria:
    - Functional: Review the phase for new user-visible options. Expected result: **none** — presets and apply-record are package-author surfaces, not `init.js` knobs. If any option did land, it is a documented Clay JS API with custom properties, index link, registry entry, and no implicit authority.
    - Performance: No extra configuration-eval work.
    - Code Quality: Configuration docs state that `clay.preset` is package.json, not `init.js`.
    - Security: Configuration still does not grant filesystem/network/shell/extension/AI/workspace authority.
  - Approach:
    - Documentation Reviewed:
      - Pattern `configuration-system.md`.
      - Decision `2026-05-08-1841-configuration-through-init-js-and-clay-js-apis.md`.
    - Options Considered:
      - `setPackagePreset` in init.js: rejected — YAGNI; authors set `clay.preset`.
    - Chosen Approach:
      - Verify-only unless implementation accidentally adds a knob.
    - API Notes and Examples:
      ```js
      // still the only user load configuration
      await loadPackage("@clay/markdown");
      ```
    - Files to Create/Edit:
      - `docs/reference/clay-js-api/configuration.md` (note: no new keys) if the review section would otherwise go stale.
    - References:
      - `examples/init.js`
  - Test Cases to Write:
    - Existing undocumented-config / custom-property gates stay green.

  - Completion Evidence (2026-08-19):
    - No new `init.js` knobs. `clay.preset` stays package.json. `node --check examples/init.js` passes.

- [x] Update the canonical example configuration (examples/init.js)
  - Acceptance Criteria:
    - Functional: `examples/init.js` still shows one-line `loadPackage` for first-party packages users should load. No new uncommented heavy setup. `node --check examples/init.js` passes. Ordering constraints (`authorizeLanguageServer` before first `loadPackage`) preserved.
    - Performance: Example eval still within configuration timeout tests.
    - Code Quality: Cross-check touched APIs against `api-inventory.toml` custom properties.
    - Security: Active example still grants no filesystem/network/shell/package-install authority.
  - Approach:
    - Documentation Reviewed:
      - Decision source: user instruction 2026-08-03 (canonical example duty).
    - Options Considered:
      - Add preset commentary to init.js: only if it helps users; prefer package-authoring docs.
    - Chosen Approach:
      - Verify + comment tweak only if the load-entry contract needs a one-line note.
    - API Notes and Examples:
      ```js
      await loadPackage("@clay/markdown");
      await loadPackage("@clay/rust");
      ```
    - Files to Create/Edit:
      - `examples/init.js` (and modular example files if they mention load ceremony).
      - `tests/fixtures/configuration/plan080-manual/` if it must stay a verbatim copy.
    - References:
      - `docs/reference/clay-js-api/packages/load-package.md`
  - Test Cases to Write:
    - `node --check examples/init.js`.
    - Existing canonical-example coverage tests.

  - Completion Evidence (2026-08-19):
    - `examples/init.js` unchanged (already one-line loads + grant-before-load). `node --check` clean.

- [x] Execute and update the manual test plan (test-plan/)
  - Acceptance Criteria:
    - Functional: Run `test-plan/09-packages-and-modes.md` and `02-configuration-init-js.md` (plus `08-syntax-and-textobjects.md` for 27.6) on a real Linux build; record pass/fail. Add numbered steps for: execute-only load (no user register calls), inspect preset/expanded perms/native ownership, one-line load still enough, LSP packages still grant-gated. Update `test-plan/index.md` coverage matrix.
    - Performance: Note load feel; no new paint regression expected.
    - Code Quality: Do not weaken/delete existing steps; failures are defects or documented ceilings.
    - Security: No host paths/secrets in evidence.
  - Approach:
    - Documentation Reviewed:
      - `test-plan/index.md` module map.
    - Options Considered:
      - Automated-only: rejected — load/inspect/classification are user-visible.
    - Chosen Approach:
      - Extend 09/02/08; do not rewrite.
    - API Notes and Examples:
      ```text
      test-plan/09-packages-and-modes.md — new Pxx steps
      ```
    - Files to Create/Edit:
      - `test-plan/09-packages-and-modes.md`, `test-plan/02-configuration-init-js.md`, `test-plan/08-syntax-and-textobjects.md`, `test-plan/index.md`.
    - References:
      - User instruction 2026-08-04 (test-plan duty).
  - Test Cases to Write:
    - Manual steps listed above with expected results and negative checks.

  - Completion Evidence (2026-08-19):
    - Added P25–P28 (inspect/one-line/LSP grant-gate), C25 (no preset knob), S20 (closed markdown vocabulary). Isolated-HOME CLI inspect recorded PASS. GUI load feel not re-run (existing automated load tests).

- [x] Update or verify the code wiki after implementation
  - Acceptance Criteria:
    - Functional: Wiki updated after all implementation tasks, or explicitly verified unchanged for non-code work. Master index links every touched page.
    - Performance: Wiki adds no runtime work; documents load-time vs hot-path for apply-record, preset expand, export resolve, inventory gen.
    - Code Quality: Pages explain what/how/invariants/tradeoffs, source/test paths, examples. Public API usage links to `docs/reference/` instead of duplicating it. New/changed primitives recorded in reference docs, wiki, index, and deterministic coverage tests.
    - Security: Documents trust-domain export resolve, native-grammar ownership, and that presets do not grant `language-server`.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/project-wiki/SKILL.md`
    - Options Considered:
      - Update after each task: rejected — churn.
    - Chosen Approach:
      - One wiki pass after tests pass.
    - API Notes and Examples:
      ```text
      docs/wiki/index.md
      docs/wiki/modules/package-loading.md
      docs/wiki/modules/syntax-grammar-registry.md
      docs/wiki/modules/first-party-language-packages.md
      docs/wiki/modules/first-party-lsp-bridge-packages.md
      ```
    - Files to Create/Edit:
      - `docs/wiki/index.md`
      - `docs/wiki/modules/package-loading.md`
      - `docs/wiki/modules/syntax-grammar-registry.md`
      - `docs/wiki/modules/first-party-language-packages.md`
      - `docs/wiki/modules/first-party-lsp-bridge-packages.md`
      - `docs/wiki/modules/first-party-markdown-package.md`
      - New page only if apply-record / presets need a dedicated module (prefer folding into `package-loading.md`).
    - References:
      - `.agents/skills/create-plan/references/wiki-task.md`
  - Test Cases to Write:
    - Manual wiki review: index links; pages explain apply-record, presets, exports, factory, vocabulary deprecation.
    - `tests/primitives_docs.rs` (or successor) still passes.

  - Completion Evidence (2026-08-19):
    - `docs/wiki/modules/package-loading.md` documents inspect bundled fallback + no process start. Preset/apply-record/exports already on that page from 27.1–27.4. `primitives_docs::wiki_index_links_every_wiki_page` passes. No new wiki page.

## Compromises Made

- CLI inspect of user-store packages still needs pnpm refresh; only bundled inventory names work without a package manager.
- `packages.inspect` JS API still planned. CLI is the surface.
- Did not rewrite all of `creating-packages.md`; added a single-manifest contract section on top of existing Phase 18 prose.

## Further Actions

- Low: `packages.inspect` JS facade if an in-app UI needs preset fields.
- Low: `list` could show bundled inventory without pnpm; skipped (YAGNI).
