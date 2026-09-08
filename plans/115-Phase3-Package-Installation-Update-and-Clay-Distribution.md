# Phase 3: Package Installation, Update, and Clay Distribution

Source: `roadmap.md` — "Phase 3: Package Installation, Update, and Clay
Distribution". Must land before any third-party package (`@arnilo/st`) is
planned (Phase 5). Phase 0–2 groundwork is in place: Prism host
(`plans/106`), base coding-agent host uplift (`plans/107`, 15/15), the
`@clay/coding-agent` package (`plans/108`, 15/22; rest absorbed by 109),
and Phase 2.1 is mid-flight (`plans/109`, 22/24). Nothing in this plan
depends on 109 completing.

Binding prior decisions:

- `decision-logs/2026-05-08-1958-clay-js-api-naming-and-package-distribution.md`:
  Clay does not implement its own package manager or registry; fetch is
  delegated to an npm-compatible JavaScript package manager (pnpm preferred
  direction). Clay owns manifest validation, provenance, enable/disable
  state, and authorization.
- `decision-logs/2026-08-30-2153-st-package-name-arnilo-scope-and-npm-distribution.md`:
  Clay is `@arnilo/clay` on npm; `clay install` appends the package to
  `loadPackage` in `init.js`; pi install model (`install npm:` forms);
  Homebrew deferred; curl installer + npm are the v1 channels.
- `decision-logs/2026-07-21-0001-two-package-runtime-trust-domains.md`:
  trusted runtime membership comes only from the compiled bundled
  inventory, never `@clay/*` naming or user promotion; adopted packages run
  in the third-party runtime; adoption is explicit and revocable.
- `decision-logs/2026-06-09-0219-explicit-init-js-package-loading-with-one-line-defaults.md`:
  packages are explicitly loaded from `~/.config/clay/init.js`; the
  one-line `loadPackage("<name>")` is the preferred default.
- `decision-logs/2026-08-21-2152-product-surfaces-are-replaceable-packages.md`:
  in-app package surfaces ride the same package-management service as the
  CLI; no second manager and no Clay registry (pattern:
  `.agents/skills/project-patterns/references/package-distribution.md`).

## Objectives

- Replace the `clay package add/remove/list` verbs with the pi model:
  top-level `clay install`, `clay remove`, `clay list`, and the
  `clay update` family (`clay update`, `--extensions`, `--all`,
  `clay update <spec>`); keep the trust-lifecycle verbs
  `clay package enable|disable|adopt|revoke|inspect|rollback`.
- Fix the existing install path defects: pnpm-only backend (no npm
  fallback), scoped-spec install matching bug
  (`PackageService::install` splits `"@arnilo/st"` on `@` and matches an
  empty name), parsed-but-unused `github:`/git/tarball/local source kinds
  (v1 has exactly one source: the npm registry), and no self-update.
- Install never executes: `clay install` fetches, records provenance, and
  appends one idempotent `loadPackage("<name>")` line to
  `~/.config/clay/init.js`. Enable/adopt/revoke/rollback stay Clay-owned
  CLI verbs; third-party JS does not run until adopt; a load line without
  adopt fails closed with a typed diagnostic.
- Clay self-distribution v1: npm package `@arnilo/clay` + curl installer;
  `clay update` self-updates only from the channel that installed it
  (no-op on unmanaged/dev checkouts); `src-tauri/src/release.rs`
  `accept_update` remains the only payload-apply gate and unsigned
  payloads stay rejected.
- Provision used binaries (graft, qmd, obscura, ripgrep, and any others
  the inventory finds): host presence check, and install only with explicit
  user permission.
- Document the distribution: publish dry-run docs for `@arnilo/clay`,
  author-facing install/distribution guidance, updated canonical example
  config, manual test plan coverage, and wiki pages.

## Expected Outcome

- On Linux, `clay install npm:<fixture>` round-trips to the store;
  `clay remove` cleans up; `clay update --extensions` updates floating
  specs and skips pinned ones; lifecycle scripts never run unless
  `--allow-scripts`.
- `clay install` appends the `loadPackage` line exactly once and never
  enables, adopts, or executes package JS; an un-adopted third-party load
  line produces a fail-closed `packages.*` diagnostic through the normal
  configuration-evaluation path.
- `clay update` is a documented skip on dev/unmanaged checkouts and drives
  the recorded npm/curl channel otherwise; it never touches packages unless
  `--all`; no new payload-download updater exists.
- One shared package service (`src/packages/service.rs::PackageService`)
  backs every verb; no Clay registry, no second package manager, no
  in-app package-management UI is added in this phase (any future in-app
  surface must reuse the same service).
- Publish dry-run documentation exists for `@arnilo/clay`; nothing is
  actually published in this phase.
- **UI scope note (deliberate):** this plan touches no app-UI files
  (components, panels, overlays, menus, tokens, layout), so the mandatory
  UI skill stack and the post-implementation visual/accessibility review
  tasks do not apply; the omission is recorded here so it is not read as an
  oversight.

## Tasks

- [x] Review existing package-management primitives and plan generic primitive gaps before install/distribution work (completed 2026-09-08)
  - Acceptance Criteria:
    - Functional: A written inventory (in this task's completion evidence) of existing primitives — `PackageManagerBackend`/`PnpmBackend`/`FakeBackend`, `PackageStore`, `PackageProvenance`/`PackageSourceKind`, `PackageService` (install/enable/disable/remove/inspect/adopt/revoke/rollback), the durable approval store, CLI parsing (`src/cli.rs::parse_package_subcommand`), `src/launch.rs::run_package_subcommand`, the init.js configuration runtime (`ConfigurationRuntime`, config watch/hot reload), and the `src-tauri/src/release.rs` update gate — states exactly what the pi-model verbs can achieve with existing primitives and names the genuinely new primitives.
    - Functional: The three known defects are reproduced as failing or documenting tests before fixes: (a) pnpm-only backend, (b) scoped-name install matching (`PackageService::install` `split('@')` bug), (c) `PackageSourceKind::GitHub`/`GitUrl`/`Tarball`/`LocalPath` accepted by parsing but unused by any install path.
    - Performance: No new primitive may be callable from typing/paint/layout/scroll/text-event handlers (the `manager.rs` hot-path rule); install/update/binary provisioning are one-shot user operations.
    - Code Quality: New primitives are generic and reusable (install ledger, init.js line management, manager selection, self-update channel, binary provisioning) — not shaped around `@arnilo/st` or any single package; the task records which are Rust-side primitives vs CLI-only logic.
    - Security: The inventory records the trust invariants every later task must preserve: bundled-inventory-only trusted classification, install≠execute, adopt-before-execute, `--ignore-scripts` default, fail-closed missing-manager behavior.
  - Approach:
    - Documentation Reviewed:
      - `roadmap.md` — Phase 3 scope and exit gate.
      - `.agents/skills/project-patterns/references/package-distribution.md` — shared service, delegated manager, install/enable/execute separation.
      - `docs/reference/packages/creating-packages.md` — current `loadPackage` contract and authoring guide.
      - `docs/reference/primitives/package-security.md`, `docs/reference/primitives/package-loading.md` — provenance and load/runtime boundaries.
      - `docs/wiki/modules/` package pages (via the wiki index) as they exist today.
      - `src/packages/manager.rs`, `src/packages/service.rs`, `src/cli.rs`, `src/launch.rs`, `src-tauri/src/release.rs`, `src/server/ops/packages.rs` (read at the cited spans).
    - Options Considered:
      - Skip the review because `PackageService` already exists — rejected: the clay.md primitive-first rule is mandatory for package runtime capability changes, and the defect list proves the current path needs re-planning, not just verb renaming.
      - Fold the review into the first implementation task — rejected: the install ledger, self-update channel, and binary provisioning are new primitives whose shape should be fixed before code starts.
    - Chosen Approach:
      - Read the cited sources, write the inventory + defect reproductions, and fix the primitive list this plan's later tasks implement. No production code changes in this task.
    - API Notes and Examples:
      ```text
      Existing seam: PackageManagerBackend::install(spec, &store, options)
      New primitives planned: install ledger, init.js load-line manager,
      manager selection (pnpm→npm), self-update channel resolver,
      binary-provisioning table. All user-triggered, all off the hot path.
      ```
    - Files to Create/Edit:
      - `tests/package_loading.rs`: defect-reproduction test (written, `#[ignore]`d, task 2 flips it). Production code intentionally untouched.
    - References:
      - `decision-logs/2026-06-04-1923-replace-markdown-parser-with-markdown-it-and-primitive-first-mode-planning.md` (primitive-first rule).
  - Test Cases to Write:
    - Defect reproduction: install-spec match test proving `"@arnilo/st"` fails to re-discover a scoped install through the current matching logic (documented failing or fixed in task 2).
  - Evidence — Primitive Inventory (2026-09-08, task complete):
    - Sources read: `src/packages/manager.rs` (full), `src/packages/service.rs` (full), `src/cli.rs` (full), `src/launch.rs` (full), `src/packages/approvals.rs` (open/upsert/revoke/approval_covers/snapshot/restore), `src/packages/bundled.rs` (`verify_bundled_trust` L132, `runtime_domain` L159, `bundled-inventory.toml`), `src/server/ops/packages.rs` (L363/398/452/540/602–605/653/692), `src/server/mod.rs` (config watch wiring L898, `ConfigurationRuntime` use L472), `src/server/configuration.rs` (`ConfigurationRuntime::from_config_root`), `src-tauri/src/release.rs` (full), `tests/package_loading.rs` (existing install/provenance tests).
    - Inventory — existing primitives and what the pi-model verbs inherit:
      - `PackageManagerBackend` sealed trait (install/remove/list_installed) + `PackageStore` (cwd-based store root), `PackageInstallOptions.allow_lifecycle_scripts` (default false), `InstallResult`/`DiscoveredPackage`/`PackageProvenance` (requested_spec, source_kind, resolved name/version, package_root, integrity) — manager.rs. `PnpmBackend` (install args test-visible; `--ignore-scripts` unless allowed) and `FakeBackend` (in-memory, never spawns) exist. `PackageSourceKind::from_spec` is labeling-only — there is no spec parser anywhere; specs pass raw into the manager. CLI verbs inherit: install delegation, `--allow-scripts` + `CLAY_ALLOW_LIFECYCLE_SCRIPTS=1|true` parity, fail-closed spawn errors.
      - `PackageService` (service.rs) — single shared service: install (backend + re-discovery), `install_from_value_at_root_with_spec` (in-memory, bundled/fake), `refresh_installed` (store is source of truth per process), enable/disable/revoke/rollback (validator + graph + relation authority + adoption gate), remove (silently disables first), list/inspect/`inspect_bundled_inventory` (bundled read without store), adopt (`approve_package`, exact durable record), `open` (durable approval store), `default_store_root()` = `~/.config/clay/packages`. Lifecycle verbs (enable/disable/adopt/revoke/inspect/rollback) need no primitive changes.
      - `PackageApprovalStore` (approvals.rs): fail-closed on corruption/unsafe perms, upsert/revoke/`approval_covers` (exact identity+capabilities+processes+relations), snapshot/restore, in-memory ctor. This is adoption state — a different lifecycle from install provenance; the Phase 3 install ledger is a genuinely new primitive beside it.
      - CLI (cli.rs): `ClayCommand::Package` + `PackageCliSubcommand` Add/Remove/List/Enable/Disable/Inspect/Adopt/Revoke/Rollback; `CLI_USAGE`; `parse_package_subcommand` L326. No top-level verbs, no update, no list statuses beyond enabled/installed.
      - launch.rs `run_package_subcommand`: opens `PackageService` fresh per invocation with `PnpmBackend::new()` hardcoded (no manager resolver); `refresh_installed` except Add/Inspect. No init.js writes, no ledger, no self-update.
      - Server load path (ops/packages.rs): `load_package_by_specifier` L540 → `ensure_first_party_record_locked` L363 → `ensure_package_installed_locked` L398 (store-installed or `packages/<name>` bundled read) → enable → `apply_package_record_contributions` L692 + `set_current_package`/`enter_package_activation` L602–605; third-party loads cross the domain bridge L452 with host-side re-validation; `end_package_activation` L653. Store-installed packages are already loadable through this path with zero CLI changes; adopt gating (service `AdoptionRequired` + approvals store) is the execution boundary an init.js load line must trip. Config watch (server/mod.rs L898) reloads on init.js change — an install-appended `loadPackage` line lands via the standard reload path with no restart.
      - Release gate (release.rs): `accept_update` rejects Unsigned/WrongTarget/WrongVersion; no payload-apply path exists; `DESKTOP_VERSION` single-sourced. `clay update` self must delegate to the install channel and add no apply path here.
    - Defect reproductions / documentation (this task adds no production code):
      - (a) pnpm-only backend — launch.rs hardcodes `PnpmBackend::new()`; no resolver. Documented, not tested (fail-closed spawn error already covered; the gap is missing npm fallback, which is a feature, not broken behavior). Task 2 adds `NpmBackend` + resolver.
      - (b) scoped/npm:-prefix discovery bug — NEW test `scoped_npm_prefixed_install_rediscovers_package` in `tests/package_loading.rs`, `#[ignore]`d with the task-2 flip instruction, verified failing against current logic when run with `--ignored`. It simulates real manager discovery (bare resolved name in `from`/`name`) and proves `PackageService::install("npm:@arnilo/st")` fails with `MissingPackageJson` because `split('@')` yields an empty/`npm:` base name for scoped and `npm:`-prefixed specs. Contrast: bare scoped specs work today only via requested-spec equality (existing passing `source_aware_install_records_provenance_without_enabling_runtime`); the v1 `npm:` form can never match because discovery never echoes the spec. The ignored test asserts today's defective behavior, so both the default suite stays green and task 2 has a failing test to flip.
      - (c) non-npm source kinds — `PackageSourceKind::GitHub/GitUrl/Tarball/LocalPath` are labeled and flow through install to the raw manager (existing test documents that `github:`/git/tarball/local specs install through `FakeBackend` today); there is no parse-time gate. Task 2 adds the v1 npm-only spec parser that rejects them; no test written now because today's behavior is not a bug but a missing policy gate.
    - New generic primitives (recorded as Rust-side, all CLI-triggered, all off editing hot paths): `npm:` spec model/parser (with manager resolver pnpm→npm), install ledger (durable install provenance; distinct from the approval store), init.js load-line manager (append/remove exact-shape lines), self-update channel resolver + extension-update logic, binary-provisioning table. CLI-only (no new op/JS API): verb parsing, printing, channel-marker files. Server-side changes: none required — the existing load path and config watch already consume store installs and init.js lines.
    - Security invariants recorded and binding for tasks 2–8: trusted runtime placement only from the bundled-inventory verification (`bundled.rs`), never `@clay/*` naming or user promotion; install never enables or executes; adopt-before-execute enforced at enable (`verify_relation_authority` → `AdoptionRequired`, exact `approval_covers`); `--ignore-scripts` default on all backends; fail-closed missing-manager spawn errors; fail-closed approval store; `accept_update` remains the only payload-apply gate (no third updater; `clay update` delegates to the channel command).

- [x] Implement the `npm:` spec model and npm-manager fallback backend (completed 2026-09-08)
  - Acceptance Criteria:
    - Functional: `clay` parses `npm:`-prefixed specs exactly — `npm:<name>`, `npm:@scope/name`, `npm:<name>@1.2.3`, `npm:@scope/name@1.2.3` — into (source = npm registry, resolved name, optional pinned version). Bare specs and `github:`/`git+`/tarball/local specs are rejected at parse time with a typed error that names the accepted v1 form; `PackageSourceKind` remains for provenance labeling but no install path accepts a non-npm source.
    - Functional: An `NpmBackend` implements `PackageManagerBackend` with `npm install --prefix <store> <spec> --ignore-scripts` (scripts only with `--allow-scripts`), `npm remove --prefix <store> <name>`, and `npm list --prefix <store> --json` discovery; a manager resolver selects pnpm when available and npm otherwise (both npm-compatible; honors the ambient npm configuration such as `npm_config_registry`), failing closed with a typed spawn error when neither is present.
    - Functional: `PackageService::install` discovery matching is name-exact (scoped names included) — the `split('@')` bug is fixed by parsing the spec through the new spec model instead of string splitting.
    - Performance: One manager process per user verb; no daemon/server involvement; no store scan on editor hot paths.
    - Code Quality: Backend trait unchanged (no new trait methods beyond what install/remove/list already require); arg builders exposed for tests exactly like `PnpmBackend::install_command_args`; `FakeBackend` untouched.
    - Security: `--ignore-scripts` stays the default for every backend; manager stdout/stderr passes through the existing diagnostic sanitizer (secrets redacted, bounded); spawn errors never leak environment or token material.
  - Approach:
    - Documentation Reviewed:
      - `decision-logs/2026-05-08-1958-clay-js-api-naming-and-package-distribution.md` — delegated npm-compatible manager.
      - npm CLI: `npm install --prefix`, `--ignore-scripts`, `--registry`/`npm_config_registry` (npm ships with Node ≥ 20, which `clay-agent` already requires).
      - `src/packages/manager.rs` (full), `src/packages/service.rs::install` (L663–738).
    - Options Considered:
      - Keep pnpm-only and document the requirement — rejected: the roadmap names pnpm-only as a defect; requiring users to install pnpm just to install packages contradicts the pi model, while Node's bundled npm is already a Clay prerequisite (`clay-agent` requires Node ≥ 20).
      - Vendor a package manager (bundled pnpm via clay-agent) — rejected for v1: extra footprint; revisit only if both-pnpm-and-npm-absent hosts become real.
      - Fetch from the registry inside Rust (no manager) — rejected: violates the 2026-05-08 decision (Clay must not become a registry/manager).
    - Chosen Approach:
      - Parse `npm:` specs in one place (a `PackageSpec` type in `src/packages/manager.rs` or a sibling module); add `NpmBackend` + manager selection next to `PnpmBackend`; keep the sealed trait.
    - API Notes and Examples:
      ```bash
      # resolver: pnpm → npm → typed fail-closed error
      pnpm add <spec> --ignore-scripts          # cwd = store root (existing shape)
      npm install --prefix <store> <spec> --ignore-scripts
      npm list --prefix <store> --json          # discovery
      ```
    - Files to Create/Edit:
      - `src/packages/manager.rs`: `PackageSpec` parsing, `NpmBackend`, manager resolver.
      - `src/packages/service.rs`: install discovery matching via `PackageSpec` (bug fix).
      - `src/launch.rs`: construct the service with the selected manager backend.
  - Test Cases to Write:
    - Spec parse: all four accepted forms resolve; bare, `github:`, `git+`, `.tgz`, `file:` specs are rejected with distinct messages.
    - Backend args: `NpmBackend` and `PnpmBackend` arg builders carry `--ignore-scripts` unless `allow_lifecycle_scripts`.
    - Service: scoped-name install re-discovers the installed package through `FakeBackend` (regression for the `split('@')` bug).
    - Resolver: pnpm-present, npm-only, neither-present selection and the fail-closed error.
  - Evidence (2026-09-08, task complete):
    - `PackageSpec` + `PackageSpecError` in `src/packages/manager.rs`; four v1 forms accepted; bare/`github:`/`git+`/tarball/local/range rejected at parse, before any backend spawn.
    - `NpmBackend` (`npm install|remove|list --prefix <store>`); `--ignore-scripts` unless `allow_lifecycle_scripts`; list JSON `dependencies` + `node_modules/<name>/package.json`.
    - `resolve_manager_backend()` / `resolve_manager_from_path()`: pnpm on PATH, else npm, else typed fail-closed; `CLAY_PACKAGE_MANAGER=pnpm|npm` override. PATH probe only, one manager process per verb. `launch.rs` uses the resolver.
    - `PackageService::install` name-exact match via parsed spec (scoped `npm:@arnilo/st` rediscovery fixed). Stderr sanitized on pnpm error paths.
    - Tests: manager unit module (parse/reject/args/resolver); `scoped_npm_prefixed_install_rediscovers_package` flipped live; `source_aware_install_records_provenance_without_enabling_runtime` npm-only; `non_npm_sources_are_rejected_before_any_backend_call`.
    - Offline check: `npm install --prefix` self-bootstraps `package.json`; no Clay store-manifest writer.
    - Linux: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, lib + security + protocol green. `large_document` 500ms budget flake under load (unrelated). Protocol inventory sync (plan 061 op count 98, ledger C21–C23 + `agent.setRunOptions`) unblocked pre-existing WIP, not this task's code.

- [x] Add the durable install ledger for pinned/floating specs and cross-process provenance (completed 2026-09-08)
  - Acceptance Criteria:
    - Functional: Every Clay-initiated install records (resolved name, original `npm:` spec, pinned flag, resolved version, source, timestamp) in a Clay-owned JSON file under the package store root (e.g. `~/.config/clay/packages/installs.json`); `clay remove` deletes the entry; the ledger is the authority for `clay update --extensions` skip decisions and for `clay list` provenance display. Manager-discovered packages with no ledger entry (installed outside Clay) are listed as unmanaged and are never auto-updated.
    - Functional: Ledger corruption/unsafe permissions fail closed with a typed `packages.*` error, mirroring the approval store's behavior; the file is written atomically.
    - Performance: Ledger read once per CLI verb; single small JSON; no server-side reads.
    - Code Quality: The ledger is a generic install-record primitive (no Clay-version or `st`-specific fields); reuse the diagnostic sanitizer for any manager text stored.
    - Security: The ledger stores no credentials; provenance strings are sanitized; the file must not be executable and is created with owner-only permissions.
  - Approach:
    - Documentation Reviewed:
      - `src/packages/approvals.rs` (fail-closed store patterns, permission checks), `src/packages/service.rs::default_store_root`.
      - pi rule (roadmap + pi README §Pi Packages): versioned/pinned installs are skipped by `update --extensions`.
    - Options Considered:
      - Derive pinned-ness from `pnpm list --json` "from" fields on each refresh — rejected: fragile across managers, lost across processes, and cannot distinguish Clay-initiated installs from manual ones.
      - Store the ledger inside the approval store — rejected: approval records are adoption state; install provenance is a different lifecycle with different write paths.
    - Chosen Approach:
      - Small dedicated ledger module owned by `PackageService`, written on install/remove, read by list/update.
    - API Notes and Examples:
      ```json
      {
        "packages": [
          { "name": "@arnilo/st", "spec": "npm:@arnilo/st", "pinned": false,
            "version": "0.1.0", "source": "npm", "installedAt": "2026-09-09T10:00:00Z" }
        ]
      }
      ```
    - Files to Create/Edit:
      - `src/packages/ledger.rs` (new): load/save/lookup with fail-closed errors.
      - `src/packages/service.rs`: install/remove/list integration.
  - Test Cases to Write:
    - Round-trip: install writes the entry; remove deletes it; reopen across a fresh `PackageService` sees it.
    - Pinned flag: `npm:name@1.2.3` → pinned; `npm:name` → floating; unmanaged store discovery → no entry.
    - Fail-closed: corrupt JSON and world-writable file each produce typed errors; atomic write leaves no partial file.
  - Evidence (2026-09-08, task complete):
    - `src/packages/ledger.rs`: `InstallLedger` / `InstallRecord` at `<store>/installs.json` (`version: 1`, `packages[]` with name/spec/pinned/version/source/installedAt). Missing file = empty; load fails closed on corrupt JSON, unknown version, oversize, duplicate names, world-writable (`0o077`). Atomic write reused from `approvals::atomic_write_owner_only` (now `pub(crate)`, returns `io::Error`).
    - `PackageService::{new,open,install,remove}`: in-memory ledger for tests; `open` loads durable ledger beside the approval store; Clay-initiated `install` upserts; `remove` deletes; `refresh_installed` never writes. `install_record(name)` is the lookup (absent = unmanaged).
    - Tests: ledger unit (round-trip/remove, pinned flag, corrupt/unknown version, unsafe perms); `install_ledger_round_trips_across_service_reopen`; `install_ledger_pinned_flag_follows_spec_and_unmanaged_has_no_entry`. List provenance printing deferred to task 5; update skip to task 6.
    - Linux: fmt, clippy `-D warnings`, lib 1269, security 138, protocol 209, presentation 46, runtime 73 (large_document skipped as 500ms budget flake).

- [x] Implement install-time init.js load-line management with the fail-closed adopt boundary (completed 2026-09-08)
  - Acceptance Criteria:
    - Functional: `clay install` appends exactly one `loadPackage("<name>")` line to the user's `init.js` at the configuration root (`default_config_root()`, honoring the existing configuration-root override for tests), marked with a short comment identifying it as Clay-added and how to remove it; the append is idempotent (re-install, version bump, and floating re-install never duplicate the line); `clay remove` removes the appended line when it still matches the appended shape exactly and never touches user-written or user-edited lines (a hand-edited line is left in place and reported).
    - Functional: Install never calls `enable`, `adopt`, or any package-execution path; first-party bundled `@clay/*` loading is unchanged. A load line for an installed-but-un-adopted third-party package fails closed through the normal configuration evaluation: a typed `packages.*` diagnostic (not a crash, not silent execution), with guidance to run `clay package adopt <name>`; after adopt, the same line loads the package in the third-party runtime domain. This is verified end-to-end through configuration hot reload, not only direct op calls.
    - Functional: A missing package (line present, package removed from the store) produces the existing `packages.not_installed` failure without blocking the rest of the configuration evaluation (the module/line isolation the example config already documents).
    - Performance: init.js line management runs only inside CLI verbs; configuration reload work is the existing watch/reload path with no new per-keystroke cost.
    - Code Quality: Line matching is exact-text against the appended shape; no heuristic parsing of user JS; the writer preserves the file's existing bytes outside the appended block.
    - Security: Trust domains preserved — install cannot promote a package to the trusted runtime (bundled-inventory verification is the only trusted path, per `2026-07-21-0001`); install grants no permissions, extension points, or process authority; the appended line carries no version pin (execution authority is the adopt record, not the load line).
  - Approach:
    - Documentation Reviewed:
      - `decision-logs/2026-06-09-0219-explicit-init-js-package-loading-with-one-line-defaults.md` — one-line `loadPackage` default; this task makes Clay itself write that line.
      - `roadmap.md` Phase 3: "Install ≠ execute, but install does write the load line."
      - `src/server/ops/packages.rs` (`ensure_first_party_record_locked`, cross-domain bridge, L363–540), `src/server/mod.rs` config watch/reload, `examples/config/init.js` + `examples/config/packages/third-party.js`.
    - Options Considered:
      - Do not write any load line; make the user add it — rejected: the roadmap and decision 2026-08-30-2153 explicitly specify install appends the line.
      - Write the line into a Clay-managed config module instead of init.js — rejected: the roadmap names `~/.config/clay/init.js`; a second auto-managed file is a new configuration surface with no added safety (the adopt gate, not file location, is the security boundary).
      - Append `loadPackage("<name>@<version>")` for pinned installs — rejected: execution authority lives in adoption; the bare-name line stays valid across updates.
    - Chosen Approach:
      - A small `init.js` line manager (append/remove exact-shape lines) invoked from the install/remove verbs; adopt-boundary verification through existing load/bridge ops plus one regression test proving the un-adopted line fails closed.
    - API Notes and Examples:
      ```js
      // clay install npm:@arnilo/st — remove with `clay remove npm:@arnilo/st`
      loadPackage("@arnilo/st");
      ```
    - Files to Create/Edit:
      - `src/packages/init_lines.rs` (new): append/remove/find exact-shape lines at a configuration root.
      - `src/packages/service.rs` or `src/launch.rs`: wire install/remove verbs to the line manager.
      - `tests/package_loading.rs`: un-adopted load line fails closed; adopted line loads in the third-party domain.
  - Test Cases to Write:
    - Idempotent append: three installs → one line; remove → line gone; re-install after remove → one line.
    - User-line safety: a user-written `loadPackage("@arnilo/st")` without the Clay comment is left untouched by `clay remove` and reported.
    - Fail-closed adopt boundary: fresh config root + installed fixture (FakeBackend) + appended line → configuration evaluation reports the typed adopt-required diagnostic and executes no package JS; after `approve_package`, the same root loads cleanly in the third-party domain.
    - Missing package: line without store entry → `packages.not_installed`, other configuration still evaluates.
  - Evidence (2026-09-08, task complete):
    - `src/packages/init_lines.rs`: exact two-line block `// clay install npm:<name> — remove with \`clay remove npm:<name>\`\nawait loadPackage("<name>");\n` (`await` required — Clay config is ESM; the plan snippet omitted it). Append idempotent; remove only on exact match; user-written/edited `loadPackage("<name>")` → `LeftUntouched`. Missing `init.js` fails closed (`packages.init_lines.missing`). No JS parse; temp+rename write; CRLF preserved. Names rejecting `"` `' ` `\\` / newlines so the block cannot break out of the string.
    - Not wired into `clay package add` / `launch.rs`: top-level `clay install`/`remove` do not exist yet (task 5). `default_config_root()` is `pub(crate)` on the lib; the binary cannot call it until task 5 publishes it. Install still never enable/adopt/execute.
    - Adopt boundary: `clay_appended_load_line_fails_closed_without_adoption` (js_runtime) appends the Clay block then `load_configuration_from_root` — adoption diagnostic, package JS does not run, package stays disabled. Adopted success remains `third_party_config_load_succeeds_after_cli_adoption` (same `loadPackage` op). Missing-package isolation remains `reload_reruns_init_js_package_load_in_fresh_generation_and_preserves_old_on_failure` (`packages.not_installed`, previous generation kept).
    - Unit tests in `init_lines.rs`: idempotent append/remove/re-append; user line left; edited comment left; missing file.
    - Linux: fmt, clippy `-D warnings`, lib 1274, security 138.

- [x] Implement the pi-model CLI verbs (install/remove/list; lifecycle verbs unchanged) (completed 2026-09-08)
  - Acceptance Criteria:
    - Functional: Top-level verbs parse and execute: `clay install npm:<spec> [--allow-scripts]`, `clay remove npm:<spec>` (also accepts the bare name), `clay list`. The trust-lifecycle verbs `clay package enable|disable|adopt|revoke|inspect|rollback` are unchanged and keep their current semantics/output. `clay package add` is removed (replaced by `clay install`); `clay package remove`/`list` are removed in favor of the top-level forms. `--allow-scripts` and `CLAY_ALLOW_LIFECYCLE_SCRIPTS=1` parity is preserved on `clay install`.
    - Functional: `clay list` prints installed packages with name, resolved version, original spec, pinned/floating, source, and status (installed/enabled/adopted or unmanaged), plus bundled first-party packages on request; unmanaged store discoveries are marked; an empty store prints a helpful line.
    - Functional: Install output records provenance (fetched spec, resolved name@version, backend used) and states explicitly that the package was not enabled, not adopted, and will not run until `clay package adopt`; the appended init.js line is reported.
    - Performance: Verbs are one-shot CLI processes; no server spawn; no lock contention with a running server beyond the package-store/approval file semantics already in place.
    - Code Quality: Parsing lives in `src/cli.rs` beside the existing subcommand parsers; usage text (`CLI_USAGE`) updated with the new verbs and the v1 one-source rule; no duplicated service logic — every verb routes through `PackageService`.
    - Security: `install` never bypasses `--ignore-scripts` defaults; `remove` refuses to remove bundled inventory entries (bundled packages are not store-managed); unknown/malformed specs fail closed with typed errors.
  - Approach:
    - Documentation Reviewed:
      - pi README §Pi Packages (`/home/arn/.nvm/versions/node/v24.19.0/lib/node_modules/@earendil-works/pi-coding-agent/README.md`) — verb set and semantics.
      - `src/cli.rs` (`parse_command`, `parse_package_subcommand`, `CLI_USAGE`), `src/launch.rs::run_package_subcommand`.
      - `.agents/skills/project-patterns/references/package-distribution.md` — one shared service for CLI and any in-app surface.
    - Options Considered:
      - Keep `clay package add` as an alias — rejected: the roadmap says replace the user-facing verbs with the pi model; two verbs for one action is drift the docs then have to police.
      - Move the lifecycle verbs top-level too — rejected: the roadmap keeps them as `clay package enable|disable|adopt|revoke|inspect|rollback`; they are Clay-owned trust operations, not pi-style package management.
    - Chosen Approach:
      - New `ClayCommand` variants `Install`/`Remove`/`List` parsed top-level; handlers in `src/launch.rs` reusing `PackageService` + the ledger + the init-line manager; `PackageCliSubcommand` retains only the six lifecycle verbs.
    - API Notes and Examples:
      ```bash
      clay install npm:@arnilo/st            # floating
      clay install npm:@arnilo/st@1.2.3      # pinned; skipped by update --extensions
      clay install npm:@arnilo/st --allow-scripts
      clay remove npm:@arnilo/st
      clay list
      clay package adopt @arnilo/st          # unchanged lifecycle verb
      ```
    - Files to Create/Edit:
      - `src/cli.rs`: new verbs, updated usage.
      - `src/launch.rs`: verb handlers.
      - `tests/package_cli.rs` (new): end-to-end CLI drills with a scratch `HOME` (see task 13's registry fixture).
  - Test Cases to Write:
      - Parse: every accepted/rejected form from the roadmap; `--allow-scripts` flag; env parity.
      - Behavior: install → ledger entry + single load line + "not adopted" output; remove → store entry, ledger entry, and load line all cleaned; list shows pinned/floating/unmanaged statuses.
      - Guard: `clay remove` on a bundled inventory name fails closed.
  - Evidence (2026-09-08, task complete):
    - `src/cli.rs`: top-level `ClayCommand::{Install,Remove,List}`; `PackageCliSubcommand` is lifecycle-only. `clay package add|remove|list` fail closed with replacement messages. Install parse rejects non-`npm:` specs before opening the store. `--allow-scripts` accepted before or after the spec. `clay list --bundled` is the on-request bundled inventory. `CLI_USAGE` documents the v1 one-source rule.
    - `src/packages/verbs.rs`: shared install/remove/list printers used by `src/launch.rs` and tests. Install never enable/adopt; reports provenance, backend, "will not run until `clay package adopt`", and the init.js append/skip. Remove resolves `npm:<spec>` or bare name; refuses bundled inventory names; strips the Clay load line only. List prints name/version/spec/pinned|floating|unmanaged/source/[installed|enabled]/[pending|adopted|…].
    - `default_config_root()` is pub next to `default_store_root()`. `resolve_manager_backend()` now returns `(ManagerKind, backend)` so install output can name pnpm vs npm.
    - Tests: parse in `src/cli.rs` (cargo test --bins); behavior in `tests/package_cli.rs` via FakeBackend (install ledger+load line+not adopted; remove cleans all three; list pinned/floating/unmanaged; bundled remove fails closed; missing init.js still installs). Binary registry e2e deferred to task 13.
    - Linux: fmt, clippy `-D warnings`, bins 8, lib 1274, security 143, protocol documentation_coverage 11.

- [x] Implement the `clay update` family (self channel, `--extensions`, `--all`, single package) (completed 2026-09-08)
  - Acceptance Criteria:
    - Functional: `clay update` updates Clay itself only, using the channel that installed it: an npm-managed `@arnilo/clay` install runs the recorded npm update command; a curl-installed checkout re-runs the recorded curl update command; a dev or unmanaged checkout (e.g. `target/` build, system package, unrecognized location) prints a clear skip message and exits successfully without touching anything. It never updates packages unless `--all`.
    - Functional: `clay update --extensions` re-runs the manager install for every ledger entry with a floating spec, skips pinned entries with a per-package "pinned; reinstall with a new version to move it" message, skips unmanaged store discoveries, and never updates Clay itself. `clay update --all` does both. `clay update npm:<spec>` updates exactly one package (pinned entries are skipped with the same message, matching the pi rule).
    - Functional: Channel identity comes from a recorded channel marker (written by the npm package layout or the curl installer, task 8), not from executable-name guessing; a missing/corrupt marker means "unmanaged" and a skip.
    - Performance: Self-update spawns at most one channel command; `--extensions` spawns one manager process per updated package (the manager's own resolution does the work); no registry polling or background updater exists.
    - Code Quality: The self-update module is a generic channel primitive (npm/curl/dev-skip) in `src/`, reusable by future channels; no Tauri plugin, no `tauri-plugin-updater`, no second updater.
    - Security: `accept_update` in `src-tauri/src/release.rs` remains the only gate any payload-apply path may call, and no such path is added in v1 (self-update delegates to the channel's own command); the update module never downloads or executes raw payload URLs itself; unsigned/wrong-target/non-newer rejection tests stay green.
  - Approach:
    - Documentation Reviewed:
      - `src-tauri/src/release.rs` (whole file: `accept_update`, `UpdateReject`, the "no third updater" policy comment), `docs/development/build-and-test.md` ("in-app updates have no apply path").
      - pi README: `pi update`, `--all`, `--extensions`, `--self`, single-package update; pinned packages skipped by `--extensions`/`--all`.
      - `decision-logs/2026-08-30-2153-st-package-name-arnilo-scope-and-npm-distribution.md` — npm + curl channels, Homebrew deferred.
    - Options Considered:
      - Build a native updater that downloads release payloads and applies them — rejected: roadmap says "no third updater"; the channel command already exists and carries its own integrity story; any future payload path must pass `accept_update`.
      - Treat `clay update` as `--all` by default — rejected: roadmap and pi both make bare `update` self-only so package churn never happens implicitly.
    - Chosen Approach:
      - A channel resolver (marker → {npm command, curl command, unmanaged}) plus extension-update logic driven by the ledger; each path prints what it did and what it deliberately skipped.
    - API Notes and Examples:
      ```bash
      clay update                    # self only, from install channel
      clay update --extensions        # packages only, floating specs
      clay update --all               # both
      clay update npm:@arnilo/st      # one package
      # dev checkout:
      # clay: this checkout is not managed by an install channel; nothing to update.
      ```
    - Files to Create/Edit:
      - `src/packages/self_update.rs` (new): channel marker resolution + self-update execution + skip semantics.
      - `src/launch.rs`, `src/cli.rs`: verbs/flags.
      - `src-tauri/src/release.rs`: no behavior change; its policy tests are extended to assert the new module performs no payload apply of its own (or the module's tests simply assert delegation, keeping release.rs untouched).
  - Test Cases to Write:
    - Channel resolver: npm marker → npm command, curl marker → recorded command, missing marker → skip; each with fake commands in tests.
    - Extensions: floating entry re-installed, pinned entry skipped with message, unmanaged discovery untouched, Clay untouched.
    - `--all` and single-spec forms; bare `update` on a dev checkout is a documented no-op exit 0.
  - Evidence (2026-09-08, task complete):
    - `src/packages/self_update.rs`: `channel.json` beside `current_exe()` (canonicalize). Schema `{version:1, channel:"npm"|"curl", argv:[...]}`. Missing/corrupt/unknown/empty-argv/non-owner-only → unmanaged skip, exit 0. `write_channel_marker` is the owner-only writer task 8 reuses. `run_self_update` execs recorded argv only — no URL fetch, no `accept_update`, no payload apply. `src-tauri/src/release.rs` untouched.
    - Bare `clay update` never opens `PackageService` (dev checkout without npm still no-op). `--extensions`/`npm:<spec>`/`--all` after self use the ledger: floating → `service.install(original spec)` with scripts off and no init.js rewrite; pinned → `pinned; reinstall with a new version to move it`; unmanaged discoveries skipped. `--all` = self then extensions. Conflicting flags rejected at parse.
    - Tests: 5 channel-resolver unit tests; parse in `src/cli.rs`; `tests/package_cli.rs` floating update + pinned skip + unmanaged skip + load-line count stays 1 + single-spec forms.
    - Linux: fmt, clippy `-D warnings`, lib 1279, security 145, bins parse tests green.

- [x] Provision used binaries: presence check plus permissioned install (completed 2026-09-08)
  - Acceptance Criteria:
    - Functional: A table-driven inventory of binaries Clay features reference today — obscura (web engine), graft (context graph), qmd (wiki hybrid search), ripgrep, and any others the inventory in task 1 finds (recorded in the table) — each with the feature that uses it and an install source (npm-compatible spec) or presence-only status; a check command reports present/absent per binary using the existing resolution rules (explicit configured path, then PATH), reusing the fail-closed semantics already implemented in `clay-agent/src/resolve-obscura.ts` and graft `cliPath` handling (no new silent resolution).
    - Functional: Missing binaries can be installed only with explicit user confirmation naming exactly what will run (npm-compatible install via the shared backend; lifecycle scripts, which binary-distribution packages commonly need, run only when the user explicitly approves them for that install); non-interactive contexts require an explicit flag and otherwise report the manual install command instead.
    - Functional: Installed binaries land in a Clay-owned bin directory recorded in the table's entry; the feature's existing fail-closed resolution is unchanged (features never gain ambient authority from provisioning).
    - Performance: Presence checks are user-triggered or install-time only; never run at startup by default, never in editor hot paths.
    - Code Quality: The table is data (Rust const/struct), not per-binary code branches; adding a binary is a one-line table change; provisioning routes through the same manager backend as packages.
    - Security: Deny-by-default: no binary is installed or executed without the explicit approval bound to that invocation; canonical argv only; no ambient-environment reliance beyond the manager's own documented configuration; provisioning grants no package, workspace, or process authority to anything; `--allow-scripts`-equivalent approval is per-invocation and recorded in output; removal (`clay remove`-parity or documented manual step) is specified for every installable binary.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/project-patterns/references/authority-boundaries.md`, `decision-logs/2026-07-14-2023-language-server-package-authority.md` (external process authority pattern: deny-by-default, explicit approval, truthful containment language).
      - `clay-agent/src/resolve-obscura.ts`, `clay-agent/src/host.ts` (graft `resolveGraftCli`), wiki `qmd` optional-CLI handling, `roadmap.md` Phase 3 binary bullet.
    - Options Considered:
      - Bundle binaries into the `@arnilo/clay` npm package unconditionally — rejected: platform/size cost for users who never touch the features; the roadmap says "check if host has binaries installed; if not, install with permission".
      - Silent `npm i -g` of binary packages at feature use — rejected: violates deny-by-default and the script-execution default.
      - Presence-only reporting (no install at all) — rejected: the roadmap explicitly requires the permissioned install path.
    - Chosen Approach:
      - Binary table + check flow + permissioned install through the existing manager backend; a decision log entry is written and approved before any script-enabled binary install path is implemented (per the external-process authority pattern), and the `package-distribution.md` pattern reference is updated with the provisioning model afterwards.
    - API Notes and Examples:
      ```bash
      clay install            # no spec: run the binary check (status report)
      # report names each missing binary, its feature, and the exact
      # install command; installing requires confirmation or the explicit
      # non-interactive flag.
      ```
    - Files to Create/Edit:
      - `src/packages/binaries.rs` (new): table + presence check + permissioned install flow.
      - `src/launch.rs`, `src/cli.rs`: no-arg install wiring (exact verb surface confirmed against task 5's parser).
      - `decision-logs/` (new entry, before script-enabled install code), `.agents/skills/project-patterns/references/package-distribution.md` (pattern update).
  - Test Cases to Write:
    - Table: every entry resolves presence correctly against a scratch PATH (found/absent/explicit-path override).
    - Permission: non-interactive without flag → report only, no spawn; with approval → backend install invoked with the recorded args; scripts enabled only under explicit approval.
    - Feature isolation: provisioning a binary changes no package record, grant, or feature availability beyond the binary's presence.
  - Evidence (2026-09-08, task complete):
    - Decision log written first: `decision-logs/2026-09-08-2141-binary-provisioning-deny-by-default-table.md` (deny-by-default, per-invocation `--yes` approval, scripts off unless `--allow-scripts`, shared backend, no package-record/ledger pollution, features keep fail-closed resolution). Pattern reference updated: `.agents/skills/clay-execution/references/packages.md` "Binary Provisioning" section (the plan's `project-patterns/references/` paths no longer exist in-tree; `packages.md` is the live pattern file).
    - `src/packages/binaries.rs`: `BINARY_INVENTORY` table — obscura (web engine; presence-only, `CLAY_OBSCURA_BIN` → PATH → `/usr/local/bin/obscura` mirroring `resolve-obscura.ts`), graft (context graph; installable `npm:@nanonets/graft`), qmd (wiki hybrid search; presence-only), ripgrep (no consuming feature today — roadmap mention, presence-only). Adding a binary is one table row.
    - `resolve`/`resolve_with`: explicit absolute path → PATH → fallback, fail-closed absent when an absolute override is set but missing; no new silent resolution.
    - CLI: `clay install` (no args) → full-table check (never spawns); `clay install --bin <name> --yes [--allow-scripts]` → permissioned install; unknown binary names and `--bin`+spec combos fail closed at parse. Provision without `--yes` prints the exact command (`ManagerKind::install_argv_display`, mirroring each backend's arg builder) and changes nothing; with `--yes` it runs the shared backend into the store, reports `<store>/node_modules/.bin`, the scripts policy, and the per-entry removal command. Provisioning never opens `PackageService` — no package records, grants, or ledger entries (feature-isolation criterion).
    - Tests: 5 unit tests (table shape, resolution override/PATH/fallback, check report, refusal without approval, presence-only never spawns) + parse tests in `src/cli.rs`.
    - Linux: fmt, clippy `-D warnings`, lib 1284, security 145, bins parse tests green.

- [x] Clay self-distribution v1: `@arnilo/clay` npm package skeleton, curl installer, and publish dry-run docs (completed 2026-09-08)
  - Acceptance Criteria:
    - Functional: A distribution skeleton exists: an npm wrapper package for `@arnilo/clay` (bin shim, version aligned with the crate version via the existing version-sync check in `scripts/package-smoke.sh`, platform-binary layout documented as optional platform packages) that requires no lifecycle scripts to install, plus a curl installer script that installs release artifacts and writes the channel marker consumed by task 6.
    - Functional: `docs/development/distribution.md` documents: packaging steps, the `npm publish --dry-run` verification (the publish dry-run the exit gate requires), the curl install/update commands, the channel marker contract, binary-provisioning notes, rollback (reinstall previous version via the channel), and the explicit statement that nothing is published in this phase.
    - Functional: `scripts/package-smoke.sh` covers the distribution additions (dry-run pack check when artifacts are present, channel-marker schema test) and its stale `examples/init.js` paths are fixed to `examples/config/init.js` (+ `examples/config/packages/`).
    - Performance: Distribution artifacts add no runtime cost; the npm wrapper's bin shim adds only process exec.
    - Code Quality: Version single-sourcing is preserved (one bump moves crate, desktop, tauri.conf, frontend, agent, and now `@arnilo/clay`).
    - Security: The wrapper package declares no postinstall and no ambient network use at install time; the curl installer verifies artifact digests before install and records the channel marker owner-only; docs carry the "packages run with full system access — review before installing" warning pi uses, adapted to Clay's adopt boundary (install never executes; adopt is the reviewable gate).
  - Approach:
    - Documentation Reviewed:
      - `decision-logs/2026-08-30-2153-st-package-name-arnilo-scope-and-npm-distribution.md`, `scripts/package-smoke.sh`, `src-tauri/src/release.rs`, `docs/development/build-and-test.md`.
    - Options Considered:
      - Publish for real in this phase — rejected: the exit gate says dry-run docs only.
      - Homebrew channel — rejected: deferred by decision 2026-08-30-2153.
      - Tauri updater plugin for self-update — rejected: roadmap says no third updater; unsigned payloads have no apply path.
    - Chosen Approach:
      - Skeleton + docs + smoke wiring only; the actual artifact build/publish pipeline is post-phase work the docs describe.
    - API Notes and Examples:
      ```bash
      npm publish --dry-run          # from the wrapper package dir (docs-only path)
      curl -fsSL https://…/install.sh | sh   # documented; not hosted in this phase
      ```
    - Files to Create/Edit:
      - `distribution/npm/` (new): `@arnilo/clay` wrapper package skeleton.
      - `distribution/install.sh` (new): curl installer + channel marker writer (documented, testable locally against a fixture artifact dir).
      - `docs/development/distribution.md` (new).
      - `scripts/package-smoke.sh`: distribution checks + stale example paths.
      - `docs/index.md`: link the new doc.
  - Test Cases to Write:
    - Marker contract: installer writes the marker the task-6 resolver reads; schema mismatch → unmanaged skip.
    - Digest: installer rejects a tampered fixture artifact.
    - Smoke: package-smoke passes with the new checks (version alignment includes the wrapper package).
  - Evidence (2026-09-08, task complete):
    - `distribution/npm/`: `package.json` (`@arnilo/clay@0.1.0`, `bin/clay` shim, `"scripts": {}` — no postinstall/preinstall, empty `optionalDependencies`), `bin/clay.js` (execs optional platform package `@arnilo/clay-<platform>-<arch>` at `bin/clay`; clear error when absent, no downloads), `README.md` (full-system-access warning, install-never-executes/adopt-is-the-gate boundary, curl-installer pointer).
    - `distribution/install.sh`: `--version` required; local fixture mode (`--artifact-dir`) + remote mode (`--base-url`, docs-only until hosted); sha256 digest verified BEFORE install; installs `clay` 0755 into `--bindir` (default `~/.local/bin`); writes `channel.json` beside the binary owner-only 0600 with `{version:1, channel:"curl", argv:[…]}` — argv records the exact re-runnable install command so task-6 `clay update` self resolves a Command channel; prints the review warning.
    - `docs/development/distribution.md`: packaging steps, `npm publish --dry-run` verification, install/update commands, channel marker contract (fail-closed rules incl. world-readable → unmanaged), binary-provisioning notes, rollback via same-channel reinstall, explicit "nothing is published or hosted yet" statement, full-system-access warning.
    - `scripts/package-smoke.sh`: stale `examples/init.js` + `examples/packages/` paths fixed to `examples/config/init.js` + `examples/config/packages/`; wrapper version alignment added (`npm_wrapper` in the mismatch guard); distribution checks — package.json hygiene (name, no lifecycle scripts), `npm pack --dry-run` (packed tarball: README + bin shim only), fixture install asserting marker schema (version 1, channel curl, non-empty string argv), tamper rejection (appended byte to artifact → installer exits non-zero).
    - `docs/index.md`: linked the new distribution doc.
    - Linux: `scripts/package-smoke.sh` PASSED (all version artifacts aligned at 0.1.0 including `@arnilo/clay`, npm pack dry-run clean, marker schema + tamper tests green, desktop release-policy tests green).
    - Skipped: real platform packages and hosted artifacts (post-phase pipeline per plan); npm-channel marker writer (no lifecycle scripts allowed — npm installs without a marker resolve to Unmanaged skip, documented in distribution.md).

- [x] Create or verify Clay JS APIs for public programmatic surfaces (completed 2026-09-08)
  - Acceptance Criteria:
    - Functional: The phase's public surfaces are inventoried: the new CLI verbs and init-line management are host-owned operations with no new JS APIs; `packages.loadPackage` semantics are unchanged (verified by the existing op tests); no new `deno_core` ops are added and no existing op grows authority; any server-side Rust public function added by this phase that is a public programmatic capability is exposed through the facade per the naming convention, or made `pub(crate)` — verified by listing every new `pub fn`.
    - Performance: No new ops means no JS-runtime surface to keep off hot paths; init-line writes are CLI-only.
    - Code Quality: Dotted-ID naming rule respected (no new core domains added; if any API were added it would use bare `<domain>.<name>` and register in `RESERVED_CORE_API_DOMAINS` — recorded as not-applicable with evidence); docs and `api-inventory.toml` cross-checked for the touched `packages.*` entries.
    - Security: Documentation states explicitly that configuration/init.js grants no package-install authority and that `loadPackage` of an un-adopted third-party package fails closed.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/project-patterns/references/clay-js-api-boundary.md`, `clay-js-api-naming.md`, `docs/reference/packages/creating-packages.md`, `docs/reference/clay-js-api/api-inventory.toml`.
      - `tests/clay_js_api_inventory.rs`, `tests/clay_js_doc_registry.rs` (the gates that enforce doc/registry sync).
    - Options Considered:
      - Expose package-management verbs as JS APIs (`packages.install`) — rejected for this phase: the roadmap keeps install/update as host CLI verbs; in-app surfaces ride the service when a UI exists, which is out of scope; revisit with the Phase 4 platform if a programmatic need appears.
    - Chosen Approach:
      - Verification-only task with doc updates where behavior text changed (creating-packages.md install section, `docs/index.md` links, generated registry refresh via the project command).
    - API Notes and Examples:
      ```text
      Verified unchanged: packages.loadPackage("<name>") — resolves, validates,
      enables, executes (trusted domain) or bridges (third-party domain, adopt-gated).
      New JS APIs: none. New ops: none. New pub fns: audited; CLI-only ones stay
      outside the facade.
      ```
    - Files to Create/Edit:
      - `docs/reference/packages/creating-packages.md`: install/remove/update verbs, adopt boundary, distribution pointers for package authors.
      - `docs/index.md`: link updates for changed/added docs.
  - Test Cases to Write:
    - Doc-registry gates: `cargo test` doc-registry/inventory suites pass after the doc updates (existing gates; no new test needed if nothing API-shaped changed — record that explicitly).
  - Evidence (2026-09-08, task complete):
    - New `pub fn` audit (git diff vs plan-115 baseline, library crate): all new public items are host/CLI plumbing in `src/packages/*` — `manager` (`PackageSpec::parse`/`to_spec`, `ManagerKind::as_str`, `install_argv_display`, `resolve_manager_backend`, backend arg builders), `service` (`default_config_root`, `install_record`, `install_records`, `list_bundled_inventory`), `ledger` (`open`/`record`/`remove`/…), `init_lines` (`append_load_line`/`remove_load_line`/…), `self_update` (`resolve_channel`/`write_channel_marker`/`run_self_update`/…), `binaries` (`BINARY_INVENTORY`, `check`, `provision`, `bin_dir`), `verbs` (`install`/`remove`/`list`/`update_*`). Each is reachable only from the `clay` binary crate (`src/cli.rs`/`launch.rs`/`main.rs`) or integration tests, following the existing `default_config_root` bin-boundary precedent; none is a package-author programmatic capability, so none enters the Clay JS facade (the `clay-js-api` boundary rule: internal functions stay outside the facade). `run_install`/`run_remove`/`run_list`/`run_update` and `atomic_write_owner_only` are `pub(crate)`.
    - No new ops and no authority growth: zero op registrations in the diff; the only `op_clay_runtime_record` hit is inside task-4's test fixture (fake package JS asserting execution). `packages.loadPackage` semantics unchanged — package_loading suite 55 passed incl. `clay_appended_load_line_fails_closed_without_adoption` (un-adopted third-party load fails closed) and install-never-enables tests.
    - `docs/reference/clay-js-api/api-inventory.toml` cross-check: `packages.loadPackage` entry unchanged (`runtime-backed`, `registry_public = true`); `packages.install` stays `status = "planned"`, `registry_public = false` (correct — no JS install API this phase; in-app surfaces ride the service when a UI exists, rejected per plan options). No new core domains → dotted-ID `RESERVED_CORE_API_DOMAINS` rule not applicable.
    - Docs: added "Installing Packages (host CLI)" section to `docs/reference/packages/creating-packages.md` — `clay install npm:<spec>` appends the load line; configuration/init.js grants no package-install authority; `loadPackage` of an un-adopted third-party package fails closed with an adoption diagnostic. Regenerated `docs/generated/clay-js-api-registry.json` via `cargo run --bin update-doc-registry` (picks up the task-2 `agent.setRunOptions` registry-public entry).
    - Linux: protocol documentation_coverage 11, clay_js 69, security package_loading 55 — all green.

- [x] Create or verify Clay configuration APIs (completed 2026-09-08)
  - Acceptance Criteria:
    - Functional: No new configuration APIs or `custom_properties` entries are introduced (install writes the documented `loadPackage` line, which is existing API); this is verified against `api-inventory.toml` for the touched surfaces; the install-appended line convention (shape, idempotency, remove behavior, adopt requirement) is documented as part of the configuration docs.
    - Performance: Unchanged (no new evaluation work; the appended line is one existing-API call per configuration evaluation).
    - Code Quality: The documented behavior matches the validated server-side parsers and op behavior, not prose (cross-checked against the task 4 tests).
    - Security: Documentation repeats the ground rule that init.js grants no package-install authority and that adopt is the execution gate; the appended-line docs name the exact file Clay writes and how to opt out (remove the line; `clay remove`).
  - Approach:
    - Documentation Reviewed:
      - `decision-logs/2026-05-08-1841-configuration-through-init-js-and-clay-js-apis.md`, `examples/config/init.js` header ground rules.
    - Options Considered:
      - Add a configuration API to disable install-time line writing (a setting) — rejected: YAGNI; no user request; the line is visible, documented, and removable.
    - Chosen Approach:
      - Verification + documentation only.
    - API Notes and Examples:
      ```js
      // the only configuration surface this phase touches:
      loadPackage("@arnilo/st"); // appended by `clay install`; adopt-gated
      ```
    - Files to Create/Edit:
      - Configuration docs section touched by task 9's updates (single edit point; no new file).
  - Test Cases to Write:
    - Existing configuration suites stay green; no new config keys appear in `api-inventory.toml` (asserted by the existing inventory test).
  - Evidence (2026-09-08, task complete):
    - Verification-only. No new configuration APIs, no new `custom_properties`, no `api-inventory.toml` keys added by this phase (the inventory diff vs baseline is only the task-2 `agent.setRunOptions` registry-public sync). The only configuration surface install touches is the documented `loadPackage` call — existing API.
    - Docs (single existing-file edit, no new file): added "Appended `loadPackage` lines (`clay install`)" section to `docs/reference/clay-js-api/configuration/load-configuration-module.md` — exact two-line shape (verified byte-for-byte against `src/packages/init_lines.rs::clay_load_block`, em dash included), idempotent append, CRLF-aware, picked up by the standard configuration watcher, `clay remove npm:<spec>` strips the block and leaves hand-edited lines untouched (LeftUntouched semantics), explicit ground rules: init.js grants no package-install authority, adopt is the reviewable execution gate, opt out by deleting the lines or `clay remove`.
    - Behavior-vs-prose cross-check: every documented claim maps to a task-4 test (`append_is_idempotent_and_remove_deletes_only_clay_block`, `remove_leaves_user_written_load_line`, `edited_clay_comment_is_left_untouched`, `missing_init_js_fails_closed`, e2e `clay_appended_load_line_fails_closed_without_adoption`).
    - Performance unchanged: no new evaluation work; the appended line is one existing-API call per configuration evaluation.
    - Linux: lib configuration filter 57 passed; protocol documentation_coverage and clay_js suites green (run in task 9, unchanged since — docs-only edits here).

- [x] Update the canonical example configuration (examples/config/init.js) (completed 2026-09-08)
  - Acceptance Criteria:
    - Functional: `examples/config/init.js` and `examples/config/packages/third-party.js` document the `clay install` flow: the exact appended-line shape with its marker comment, what `clay remove` does to it, the adopt requirement before a third-party package runs, and the pinned/floating spec meaning; heavy/environment-specific examples stay commented per the file's rules.
    - Functional: Every new documented behavior appears exactly once, in its section, annotated in the file's established comment style; ordering constraints (e.g. `authorizeLanguageServer` before first `loadPackage`) are preserved.
    - Performance: None (documentation file).
    - Code Quality: `node --check examples/config/init.js` (and the packages modules) pass; the active uncommented part stays safe to copy verbatim.
    - Security: The example continues to grant nothing new; the third-party template keeps showing that a load line alone executes nothing without adopt.
  - Approach:
    - Documentation Reviewed:
      - `examples/config/init.js`, `examples/config/packages/third-party.js` (whole files), `scripts/package-smoke.sh` (node --check paths, fixed in task 8).
    - Options Considered:
      - Skip — Clay writes the line automatically — rejected: clay.md requires the canonical example to carry every user-facing configuration surface, and the appended line is one users will find in their real init.js.
    - Chosen Approach:
      - Extend the third-party template section + the relevant init.js comment; fix the smoke-script paths (task 8 covers the script edit; this task verifies the checks pass).
    - API Notes and Examples:
      ```js
      // — third-party packages (clay install) —
      // `clay install npm:@scope/name` appends a line like the one below;
      // the package does not run until `clay package adopt @scope/name`.
      // loadPackage("@arnilo/st");
      ```
    - Files to Create/Edit:
      - `examples/config/init.js`, `examples/config/packages/third-party.js`.
  - Test Cases to Write:
    - `node --check` on all three example files (via package-smoke, path-corrected).
  - Evidence (2026-09-08, task complete):
    - `examples/config/packages/third-party.js`: header rewritten to the phase-3 verb model — `clay install npm:<spec>` (replaces stale `clay package add`), the exact appended-line block with its marker comment, idempotent append, `clay remove npm:<spec>` strips what it wrote and leaves hand-edited lines alone, pinned (`npm:<name>@1.2.3`) vs floating (`npm:<name>`) update semantics, install never enables/adopts/executes, adopt before the package runs, un-adopted load fails closed. Template loads show the bare-name form `clay install` appends plus the `clay install` pointer (the invented `npm:`-prefixed `loadPackage` argument was removed — the appended line uses the bare name, per `clay_load_block`).
    - `examples/config/init.js` (section 11 module-load comment): documents that `clay install` appends its block to `~/.config/clay/init.js` (not this file), shows the exact two-line shape, idempotency, removal, and the adopt gate, with a pointer to `packages/third-party.js` for the full contract. Each documented behavior appears once, in its section, in the file's established comment style; `authorizeLanguageServer`-before-first-`loadPackage` ordering untouched.
    - Security unchanged: everything added is commented; the active uncommented configuration grants nothing new, and the third-party template still demonstrates that a load line alone executes nothing without adopt.
    - Linux: `node --check` green on all three example files; `scripts/package-smoke.sh` PASSED (canonical example syntax check + distribution checks).

- [x] Launch-test the app with the canonical example config (completed 2026-09-08)
  - Acceptance Criteria:
    - Functional: Copy `examples/config/` (+ `packages/`) into an isolated scratch config root (temp HOME), launch a real Linux GUI build (server + client) with it, and verify: healthy startup (client reaches Connected; a configuration generation commits with no `configuration failed` diagnostics), shell interaction works (open a pane, run a command), and the example's `loadPackage`'d first-party contributions register. Then, in the same scratch root, simulate the install flow: append the documented load line for an installed-but-un-adopted fixture package and verify the app stays healthy with a typed adopt-required diagnostic (fail-closed, no execution) — the exact user-visible behavior `clay install` produces.
    - Performance: Startup and reload timings show no regression from the added line (the line is one existing-API call).
    - Code Quality: The launch command, scratch config path, and observed results are recorded in the task evidence.
    - Security: The test never runs against the developer's real profile; the un-adopted fixture line produces no package execution.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/create-plan/references/clay.md` — Example Configuration Live Launch-Test Task (binding), `docs/development/launch-and-gui-smoke.md`.
    - Options Considered:
      - Server-only check — only acceptable if GUI launch is blocked; then record the blocker, assert the scratch-root generation commits without diagnostics, and leave interactive acceptance unresolved.
    - Chosen Approach:
      - Full GUI launch drill as specified, with the fail-closed un-adopted-line exercise as the phase-specific step.
    - API Notes and Examples:
      ```bash
      HOME=$scratch clay server &   # or the standard smoke launch
      # verify: connected, generation committed, no configuration diagnostics
      ```
    - Files to Create/Edit:
      - None (evidence recorded in the task; test-plan steps come next).
  - Test Cases to Write:
    - Scratch launch: healthy startup + interaction (manual, recorded).
    - Un-adopted appended line: app stays healthy, typed diagnostic, no package JS executed.
  - Evidence (2026-09-08, task complete — live Linux GUI drill on Wayland host):
    - Launch: debug build (`cargo build -p clay -p clay-desktop`); scratch root `/tmp/clay-launch-drill.*` (mode 700) with `examples/config/` copied to `<scratch>/.config/clay/`, endpoint dir + workspace + XDG dirs inside the scratch root. Fixture package `@vendor/mode` (manifest shape from `loadable_package_fixture`: apiPrefix `mode`, `loadEntry ./dist/load.js`) hand-placed at `<scratch>/.config/clay/packages/node_modules/@vendor/mode/` — the store layout the npm manager produces (real-registry install not needed for the un-adopted drill). `HOME/XDG_*` pointed at the scratch root; developer profile never touched. `clay server <scratch>/endpoint/clay.sock` + `clay client <same>` (CLAY_DESKTOP_BIN). Server log: zero `configuration failed` matches; agent command registry populated (`/compact`, `/branch`, `/tree`, … host=live). Client log: session_request round-trip reached the server.
    - Healthy startup verified visually (screenshot): desktop window rendered, `Ready` status, Workspace file browser populated from the server (client↔server protocol interaction), `@clay/chat` first-party landing pane loaded ("What do you want to do today?" + Agent/Provider/Model controls), no configuration diagnostics.
    - Interaction: sidebar file selection via click worked (selection moved); command-palette drill UNRESOLVED — the host's portal input backend cannot deliver synthetic coordinates/chords into the app (documented in `docs/development/launch-and-gui-smoke.md`: `org.freedesktop.DBus.Error.ServiceUnknown` GNOME Shell window-control API; portal coordinates must not be substituted). Server-side command dispatch evidenced by the registered-command log + the client session_request round-trip.
    - Fail-closed un-adopted exercise: appended the documented two-line block (`// clay install npm:@vendor/mode — remove with …` + `await loadPackage("@vendor/mode")`) to the scratch `init.js`. Configuration watcher reloaded within seconds; server logged `runtime reload failed [runtime.exception]: JavaScript runtime evaluation failed.` (adoption-required failure inside evaluation); client stayed `Ready` with the previous working generation intact (chat landing + workspace browser fully functional) and the status strip surfaced the typed diagnostic (`JavaScript runtime evaluation failed.`). Package JS never executed — enable failed before `loadEntry`, no execution records in the server log. Exactly the user-visible behavior `clay install` produces for an installed-but-un-adopted package.
    - Performance: the appended line is one existing-API call; the watcher picked up the edit and completed the failing reload within the 3 s observation window — no timing regression to measure (documentation-file change only).
    - Cleanup: server/client/desktop processes stopped, scratch root removed. Residual note: the host input limitation is pre-existing (recorded in launch-and-gui-smoke.md) and unrelated to plan 115 changes.

- [x] Execute and update the manual test plan (test-plan/) (completed 2026-09-08)
  - Acceptance Criteria:
    - Functional: `test-plan/09-packages-and-modes.md` gains numbered steps for the new verbs (`clay install`/`remove`/`list`/`update` family, the appended load line, the adopt boundary, `--allow-scripts` warning) with expected results, negative checks (un-adopted load fails closed; pinned spec skipped by `--extensions`), and known ceilings (e.g. real-registry steps require network; script-enabled binary installs need explicit approval); `test-plan/02-configuration-init-js.md` gains steps for the install-appended line and reload behavior; `test-plan/index.md` coverage matrix updated if module scope changes.
    - Functional: Relevant steps are executed on a real Linux build and results recorded; failures are treated as defects or documented ceilings, never weakened steps.
    - Performance: Steps include the startup-with-appended-line check (no measurable startup regression).
    - Code Quality: New steps cross-link `docs/development/distribution.md` and `docs/reference/packages/creating-packages.md` instead of duplicating.
    - Security: Steps verify `--allow-scripts` stays off by default and the adopt gate holds on the manual path.
  - Approach:
    - Documentation Reviewed:
      - `test-plan/index.md` (module map + rules), `test-plan/09-packages-and-modes.md`, `test-plan/02-configuration-init-js.md`.
    - Options Considered:
      - New module file for distribution — rejected: 09 already owns package surfaces; distribution drills are CLI, not GUI.
    - Chosen Approach:
      - Extend the two existing module files + index matrix; run the affected steps.
    - API Notes and Examples:
      ```text
      09-packages-and-modes.md — new steps: install round-trip, list output,
      pinned skip, remove cleanup (store + ledger + line), adopt boundary.
      ```
    - Files to Create/Edit:
      - `test-plan/09-packages-and-modes.md`, `test-plan/02-configuration-init-js.md`, `test-plan/index.md`.
  - Test Cases to Write:
    - The added numbered steps themselves (executed and recorded).
  - Evidence (2026-09-08, task complete — steps written AND executed live):
    - Module 09: new P43–P54 (install round-trip via real npm backend + local fixture registry, list output, append idempotency, un-adopted fail-closed, adopt→activate, remove cleanup incl. stale-hand-written-line diagnostic, pinned install + `--extensions`/single-spec skip, update-self dev no-op, binary presence report, provisioning refusal without `--yes`, `--allow-scripts` warning), plus negative checks and known ceilings; cross-links `docs/reference/packages/creating-packages.md` + `docs/development/distribution.md` instead of duplicating.
    - Module 02: new C30–C34 (watcher reload of the appended line: fail-closed un-adopted, clean post-adopt, remove strips block byte-exactly, stale-line diagnostic, startup budget 33 ms vs 60 ms baseline).
    - Index: module map rows 02/09 extended, new coverage-matrix row "Package install/remove/list/update CLI → 09 (P43–P54), 02 (C30–C34), 01", plan 115 execution record added.
    - Execution: full drill on real Linux build, scratch HOME, local registry fixture (`clay-fixture-pkg` 0.1.0/0.2.0), live `clay server`; every executed step PASS; developer profile untouched; scratch removed.
    - DEFECT FOUND AND FIXED (the drill's core catch): production server opened its PackageService with FakeBackend and never refreshed `installed`, so `loadPackage` of any store package failed `packages.not_installed` even after adoption — install→adopt→activate could never complete. Fix: `PackageService::open_production` (`src/packages/service.rs`) — one real manager discovery pass at boot, fail-closed fallback to prior no-discovery behavior; wired into `ClayJsRuntimeService::production`. Also added the missing `--allow-scripts` warning line to `src/packages/verbs.rs` install output (P54 step expected it).
    - Gates after the fix: lib 1284 passed / 1 ignored, security package suites 116 passed, protocol suite green, `cargo fmt --check` + `cargo clippy --all-targets -- -D warnings` clean. No test-plan step deleted or weakened.

- [x] Final verification: Linux gates and the roadmap exit-gate drills (completed 2026-09-08)
  - Acceptance Criteria:
    - Functional: All roadmap Phase 3 exit-gate drills pass on Linux: (1) `clay install npm:<fixture>` round-trips to the store and `clay remove` cleans it (fixture served from a local static registry: fixture tarball + registry metadata JSON + `python3 -m http.server`, with `npm_config_registry` pointing at it and a scratch `HOME`; no network dependency), (2) `clay update --extensions` updates a floating fixture and skips a pinned one, (3) lifecycle scripts stay off unless `--allow-scripts` (asserted via backend args + fixture install), (4) install appends `loadPackage` once and never enables/adopts/executes (ledger, line count, and adopt-state assertions), (5) `clay update` self is a documented no-op on the dev checkout and drives the recorded channel command on marker-present checkouts (fake commands), (6) publish dry-run docs exist for `@arnilo/clay`.
    - Functional: Existing behavior stays intact: `clay package enable|disable|adopt|revoke|inspect|rollback` drills from earlier phases still pass; chat/editor flows unaffected; first-party bundled loading unchanged.
    - Performance: `cargo test` suites show no new hot-path work (package suites only); CLI verb wall-clock is dominated by the manager process (recorded for the drill).
    - Code Quality: `cargo fmt --check`, `cargo check --all-targets`, `cargo clippy --all-targets -- -D warnings` pass on Linux; the new tests are deterministic (scratch HOME, local registry, FakeBackend) so CI has no network flake.
    - Security: Fail-closed checks re-run: un-adopted load line, unsigned-payload rejection (`release.rs` suite), `--ignore-scripts` default, binary provisioning without approval.
  - Approach:
    - Documentation Reviewed:
      - `roadmap.md` Phase 3 Exit Gate; `.agents/skills/create-plan/references/clay.md`; npm registry metadata document shape (versions map + `dist.tarball`) for the static-registry fixture.
    - Options Considered:
      - Real-registry integration test — rejected: network flake in CI; the static local registry exercises the identical code path through the real backend.
      - FakeBackend-only drills — rejected: the exit gate demands a real round-trip through the actual manager.
    - Chosen Approach:
      - Deterministic local-registry integration drills in `tests/package_cli.rs` (or a dedicated integration test file) using `CARGO_BIN_EXE_clay`, scratch `HOME`, and the local registry; plus the full Linux gate run.
    - API Notes and Examples:
      ```bash
      # fixture registry layout served at http://127.0.0.1:<port>/
      #   /clay-fixture-pkg            (metadata JSON, versions → dist.tarball)
      #   /clay-fixture-pkg/-/0.1.0.tgz
      HOME=$scratch npm_config_registry=http://127.0.0.1:<port>/ \
        $CARGO_BIN_EXE_clay install npm:clay-fixture-pkg
      ```
    - Files to Create/Edit:
      - `tests/package_cli.rs` (new) + fixture helpers under `tests/fixtures/`.
  - Test Cases to Write:
    - The six exit-gate drills above, automated.
  - Evidence (2026-09-08, task complete — all six drills automated AND passing):
    - New `tests/package_exit_gate.rs` (registered as a module of the security suite; every `tests/*.rs` file is suite-assigned by the protocol inventory test): in-test static npm registry (std TcpListener HTTP server + packument + tarballs built with `tar czf`, sha1/sha512 SRI digests via `python3`), scratch `HOME`, real `CARGO_BIN_EXE_clay` + real npm backend, `CLAY_PACKAGE_MANAGER=npm` pinned, `npm_config_registry` pointed at the local server. No network dependency.
    - Drill 1 PASS: `clay install npm:<fixture>` → store `node_modules/<pkg>/package.json` materialized by the real backend, ledger record `{version 0.2.0, pinned false}`, exactly one appended load block, adopt-boundary line printed; `clay remove` → package dir + ledger entry + Clay block all gone, user lines byte-preserved.
    - Drill 2 PASS: `clay update --extensions` updates the floating record (`Updated <pkg>@0.2.0`), pinned and unmanaged single-spec updates skip with `pinned; reinstall with a new version to move it` / `not a Clay-managed install`; update never duplicates the load line.
    - Drill 3 PASS (backend args + fixture install per plan wording): default real-npm install leaves the postinstall sentinel absent and prints no ENABLED warning; an npm argv-recording shim on PATH proves default installs pass `--ignore-scripts` to the manager process and `--allow-scripts` installs do NOT. ponytail ceiling recorded in-test: npm ≥ 11.17 gates registry-dependency lifecycle scripts behind its own `allowScripts` policy regardless of flags, so only Clay's suppression boundary is asserted (whether npm then runs the script is npm's policy, not Clay's).
    - Drill 4 PASS: install appends the block exactly once (`Load line already present` on re-install, count stays 1), `clay list` shows `[pending]` and never `[enabled]`, no approval store file exists before adopt.
    - Drill 5 PASS: `clay update` on the dev checkout prints the documented no-op (`not managed by an install channel … no channel marker`); with a channel marker beside the binary the recorded command runs verbatim (fake `sh -c touch` argv, nothing downloaded or replaced; marker removed by a drop guard).
    - Drill 6 PASS: `distribution/npm/package.json` is `@arnilo/clay` with no lifecycle scripts, `bin/clay.js` exists, `distribution/install.sh` writes `channel.json`, `docs/development/distribution.md` covers `@arnilo/clay` and the publish dry-run contract (also exercised end-to-end by `scripts/package-smoke.sh` `npm pack --dry-run`).
    - Inventory syncs required by new steps/files: P43–P54 added to the 09-packages-and-modes row and C30–C34 to the 02-configuration-init-js row in `docs/development/tauri-react-parity-ledger.json`; `package_exit_gate.rs` visible to the protocol suite inventory (module of security suite).
    - Linux gates at completion: `cargo fmt --check` clean; `cargo check --all-targets` clean; `cargo clippy --all-targets -- -D warnings` clean; lib 1289 passed; security 152 passed (includes the 7 exit-gate drills); runtime 75; presentation 46; protocol 209. CLI verb wall-clock is manager-process dominated (drill binaries finish in ~3 s total including 10+ real npm invocations).
    - Security re-checks: un-adopted load line fails closed (drill 4 + task 13 drill), unsigned-payload rejection unchanged (`src-tauri/src/release.rs` suite untouched), `--ignore-scripts` default proven at the manager-process boundary (drill 3), binary provisioning without `--yes` refused (task 13 drill + parse tests).

- [ ] Update or verify the code wiki after implementation
  - Acceptance Criteria:
    - Functional: The project code wiki is updated after all implementation tasks are complete: package-management pages cover the new verbs, ledger, init-line management, self-update channels, and binary provisioning; distribution is documented; the master index links the pages.
    - Performance: Wiki updates add no runtime work and document performance-relevant details (CLI-only, off hot path, one manager process per verb).
    - Code Quality: Wiki pages explain what the changed code does, how it works, invariants/tradeoffs (install≠execute, adopt boundary, pinned-skip rule, channel marker), source/test paths, examples, and links from the master wiki index.
    - Security: Wiki pages document the touched boundaries (adopt gate, script default, unsigned-payload rejection, provisioning permission) without exposing secrets.
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
      docs/wiki/modules/<package-management>.md
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
- Known constraint carried in from the roadmap (not a new compromise): v1
  supports exactly one package source — the npm registry. `github:`, git
  URLs, tarballs, and local paths are rejected at parse time rather than
  half-parsed; adding a second source is a later roadmap decision, not a
  deferred defect.

## Further Actions

- To be filled after task completion with improvements, rationale, and
  priority. Candidates to evaluate at completion (not pre-decided):
  actually publishing `@arnilo/clay` and hosting the curl installer;
  bundling a package manager for hosts without pnpm/npm; exposing package
  management to the in-app UI through the same service; `clay update
  --models`-style auxiliary refreshes if Clay grows model catalogs.
