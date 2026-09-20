# Plan 138 — Local Package Sources: `path:` Specs Without npm

Source: the 2026-09-20 configurability review, deviation D2. Making a
personal theme/mode today requires build → npm publish →
`clay install npm:…` → adopt → `loadPackage`. `PackageSpec::parse`
(`src/packages/manager.rs:502`) accepts only the v1 `npm:` form and rejects
every other source family with a typed error, even though
`PackageSourceKind` (`src/packages/manager.rs:30`) already models
`GitHub | GitUrl | Tarball | LocalPath` and the distribution decision
explicitly lists valid sources as "npm registry, GitHub/git URL, tarball,
local path — routed through the shared package manager/source resolver".

Binding prior decisions:

- `decision-logs/2026-05-08-1958-clay-js-api-naming-and-package-distribution.md`:
  local path is an approved source family; Clay delegates mechanics to the
  npm-compatible manager and owns the contract.
- `decision-logs/2026-06-27-2014-unified-user-authorized-package-authority.md`
  and `2026-07-21-0001-two-package-runtime-trust-domains.md`: local packages
  are adopted third-party trust — never promoted by naming or origin
  familiarity.
- `decision-logs/2026-06-09-0219-explicit-init-js-package-loading-with-one-line-defaults.md`:
  one-line `loadPackage` remains the canonical user path.
- `decision-logs/2026-09-08-2141-binary-provisioning-deny-by-default-table.md`:
  lifecycle scripts stay off absent an explicit per-invocation flag (the same
  posture `PnpmBackend::add` already carries via `--ignore-scripts`).

Roadmap position: Phase 3 package-platform follow-through; unblocks plan 140's
personal-theme workflow and plan 139's tier-2 agent changes (agent authors a
local package instead of publishing).

## Objectives

- Extend the install CLI and spec parser with a `path:` source family
  resolving to local directories (`path:/abs/dir`, `path:./rel`,
  `path:~/…`), installed through the existing `PackageManagerBackend`
  (`pnpm add <dir> --ignore-scripts`) into the Clay-owned store so
  enable/adopt/load stay byte-for-byte the existing verbs.
- Provenance records `source_kind = local-path` with the absolute resolved
  root; the spec string round-trips.
- A local package is adopted third-party by default: full manifest
  validation, capability grants required, no lifecycle scripts, no trusted
  classification — identical to an npm package of the same manifest.
- Ship a documented local-package template under `examples/` so the
  personal-theme/mode loop is copy → edit → install → load.
- Keep `npm:` the documented default; this plan adds one source family
  (`path:`), not git/tarball (recorded as further actions, not silently
  skipped).

## Expected Outcome

- `clay install path:~/clay-themes/my-theme && clay package adopt … &&
  loadPackage("<name>")` works end to end from `init.js`; the store shows the
  package with local-path provenance; conflicts, budgets, validation, and
  revocation behave exactly as for npm-sourced packages.
- Editing the source directory and reinstalling refreshes the store record;
  removal works with the existing verb.
- All gates green; tests cover parse, containment (no `path:` escape outside
  an explicit user-given directory — the path is user-supplied CLI input, not
  package-controlled), script denial, and provenance.

## Tasks

- [ ] Baseline gates on the unmodified tree
  - Acceptance Criteria:
    - Functional: commit + seven-stage gate results recorded under
      `code-reviews/<date>-plan138-baseline/logs/`; the current rejection
      reproduced (`clay install ./fixtures/theme` → `MissingNpmPrefix`).
    - Performance: n/a.
    - Code Quality: baseline before edits.
    - Security: fixture package used for baseline is inert (no
      `loadEntry` side effects).
  - Approach:
    - Documentation Reviewed: AGENTS.md platform validation;
      planning-checklist.md.
    - Options Considered: reuse plan-136 baseline — rejected: different
      code paths.
    - Chosen Approach: standard gate run + typed-error reproduction.
    - API Notes and Examples:
      ```bash
      cargo test -p clay --test package_loading 2>/dev/null || cargo test --lib package
      ```
    - Files to Create/Edit: `code-reviews/<date>-plan138-baseline/README.md`.
    - References: `src/packages/manager.rs:502`.
  - Test Cases to Write: none (evidence capture).

- [ ] Review package-source and trust-boundary primitives before implementation
  - Acceptance Criteria:
    - Functional: inventory with paths — `PackageSpec` /
      `PackageSpecError` (`src/packages/manager.rs`), `PackageSourceKind`
      (`:30`), `PnpmBackend::add/remove/list` (`src/packages/manager.rs`
      backend impl), `PackageService::install/enable`
      (`src/packages/service.rs`), provenance plumbing
      (`PackageProvenance::from_package_json`), adoption/runtime-domain
      assignment, CLI verb parsing (`src/cli.rs`). State exactly where a
      `path:` spec changes behavior (parse, display, install delegation) and
      where nothing changes (enable validation, grants, conflict pass).
    - Performance: install-time only work; no load-path additions.
    - Code Quality: one source family implemented generically so
      git/tarball later reuse the same spec enum + provenance.
    - Security: review restates the trust rule — local-path never yields
      trusted classification; scripts stay off; the resolved root is recorded
      verbatim (symlinks resolved) in provenance.
  - Approach:
    - Documentation Reviewed:
      `docs/reference/primitives/index.md`,
      `docs/wiki/modules/package-loading.md`,
      `docs/wiki/modules/package-management.md`,
      `.agents/skills/clay-execution/references/packages.md`
      (Package Distribution, Trust Domains, Binary Provisioning).
    - Options Considered:
      - A side channel outside PackageSpec (`clay package adopt-local`) —
        rejected: second install path duplicates validation/lifecycle.
      - Auto-adopted `~/.clay/packages/local/` directory convention —
        deferred: implicit activation contradicts explicit-load; revisit
        after `path:` proves out.
    - Chosen Approach: extend `PackageSpec` to an enum
      (`Npm { name, version } | Path { resolved_root, display }`) behind the
      existing parse entry point; install delegates `pnpm add <abs-path>`.
    - API Notes and Examples:
      ```rust
      pub enum PackageSpec {
          Npm { name: String, version: Option<String> },
          Path { root: PathBuf }, // canonicalized, user-supplied
      }
      ```
    - Files to Create/Edit:
      `code-reviews/<date>-plan138-baseline/primitive-review.md` (new).
    - References: `plans/115-Phase3-Package-Installation-Update-and-Clay-Distribution.md`,
      `plans/136-…md` (primitive-review task shape).
  - Test Cases to Write: none (review verified by later tasks).

- [ ] Implement `path:` spec parsing and CLI acceptance
  - Acceptance Criteria:
    - Functional: `clay install path:./my-theme` (cwd-relative),
      `path:/abs`, `path:~/…` parse; `~` expands via the config-root home;
      relative paths resolve against the CLI working directory; the
      directory must exist and contain `package.json` (typed error
      otherwise); `path:` + npm-style name mixing is rejected; `clay list` /
      `inspect` display the original spec and resolved root.
    - Performance: parse is O(1) string + one stat; no hot-path contact.
    - Code Quality: `PackageSpecError` gains typed variants
      (`MissingDirectory`, `MissingManifest`) with the established Display
      style; no stringly-typed source sniffing on this path (explicit
      prefix, unlike legacy `from_spec` heuristics).
    - Security: the path is user CLI input (same trust as the shell the user
      typed it in); validation never executes package code; `--ignore-scripts`
      remains the backend default; enabling still requires grants.
  - Approach:
    - Documentation Reviewed: primitive-review output; `src/cli.rs` verb
      parser; `PnpmBackend::add` options.
    - Options Considered: accept bare `./dir` without prefix (npm parity) —
      rejected for v2 of the spec: explicit families keep provenance honest
      and the error messages teachable.
    - Chosen Approach: prefix-gated enum; `to_spec()` round-trips the
      canonical form.
    - API Notes and Examples:
      ```bash
      clay install path:~/Projects/clay-themes/my-theme
      clay package inspect my-theme   # provenance: local-path (root: …)
      ```
    - Files to Create/Edit: `src/packages/manager.rs` (spec + backend),
      `src/cli.rs` (verb wiring), `src/packages/service.rs` (install entry).
    - References: decision `2026-05-08-1958`.
  - Test Cases to Write:
    - `parse_path_spec_variants_and_errors` (abs/rel/tilde/missing
      dir/missing manifest/mixed prefix).
    - `install_path_package_records_local_provenance`.
    - `path_install_denies_lifecycle_scripts_by_default`.

- [ ] Preserve the two-runtime-trust-domain boundary for local packages
  - Acceptance Criteria:
    - Functional: a `path:`-installed package with `@clay/`-prefixed name
      fails enable (`is_reserved_prefix` path unchanged); the third-party
      runtime installs only documented public ops; grants still required for
      capabilities; replace/extend of first-party surfaces still needs the
      extension point + approval.
    - Performance: unchanged enable path.
    - Code Quality: no new classification logic — local path is provenance,
      never trust.
    - Security: cross-domain deny tests pass for the fixture; revocation and
      generation replacement behave as for npm packages.
  - Approach:
    - Documentation Reviewed:
      `.agents/skills/clay-execution/references/packages.md` (Trust Domains);
      `plans/034-Persistent-Runtime-Hardening-Before-Third-Party-Package-Authority.md`.
    - Options Considered: trust packages under `~/.clay/` by location —
      rejected: violates compiled-inventory classification rule outright.
    - Chosen Approach: fixture package exercises every deny assertion.
    - API Notes and Examples: n/a (test-heavy task).
    - Files to Create/Edit: `tests/package_loading.rs` (or successor suite),
      fixture under `tests/fixtures/local-package/`.
    - References: decision `2026-07-21-0001`.
  - Test Cases to Write:
    - `local_path_clay_prefix_rejected`.
    - `local_path_needs_capability_grants` (MissingCapabilityGrant).
    - `local_path_revocation_withdraws_contributions`.

- [ ] Verify the package default init.js loading experience
  - Acceptance Criteria:
    - Functional: after install+adopt, one `loadPackage("<name>")` line in
      `init.js` activates a local theme package's contributions with zero
      manifest copying or facade plumbing; customization via
      `setPackageOption` works as for npm packages.
    - Performance: no additional load-time cost vs npm source.
    - Code Quality: no source-family branches inside `loadPackage`.
    - Security: load-time capability checks unchanged.
  - Approach:
    - Documentation Reviewed: decision `2026-06-09-0219`;
      `create-plan/references/clay.md` → Package Default Loading Task.
    - Options Considered: document a longer local-dev loop as acceptable —
      rejected: one-line load is the recorded convention.
    - Chosen Approach: prove it with the template package from the next task.
    - API Notes and Examples:
      ```js
      loadPackage("my-theme"); // local-path provenance, ordinary load
      ```
    - Files to Create/Edit: none (verification; test additions live in the
      template task).
    - References: `runtime/js/packages.js`.
  - Test Cases to Write: `local_package_one_line_load_activates_contributions`.

- [ ] Ship a local-package template and authoring docs
  - Acceptance Criteria:
    - Functional: `examples/packages/local-template/` contains a minimal
      theme-flavored package (manifest with `clay.contributions` designTokens
      sample, `loadEntry`, README) that installs, adopts, and loads cleanly;
      `docs/reference/packages/creating-packages.md` gains a "Local packages"
      section (install, edit, reinstall loop, trust posture, script denial).
    - Performance: n/a.
    - Code Quality: template uses `preset` where applicable, minimal
      declarations, no copied boilerplate (single-manifest decision).
    - Security: template declares no capabilities it doesn't use; docs state
      adopted-third-party trust explicitly (not "sandboxed").
  - Approach:
    - Documentation Reviewed: decision `2026-08-18-1758` (single manifest,
      presets); `create-plan/references/clay.md` → Package UI/Layout and
      Authoring Documentation Task (docs duty).
    - Options Considered: wiki-only docs — rejected:
      `creating-packages.md` is the contracted authoring surface.
    - Chosen Approach: smallest honest template (theme tokens + one command)
      exercising the full loop.
    - API Notes and Examples:
      ```bash
      cp -r examples/packages/local-template ~/.clay-themes/mine
      clay install path:~/.clay-themes/mine
      ```
    - Files to Create/Edit: `examples/packages/local-template/**` (new),
      `docs/reference/packages/creating-packages.md`.
    - References: `examples/config/packages/first-party.js` (load examples).
  - Test Cases to Write:
    - `template_package_installs_and_loads` (CI-friendly: install into temp
      store, enable, assert one token contribution active).

- [ ] Create or verify Clay JS APIs for public programmatic surfaces
  - Acceptance Criteria:
    - Functional: no new JS facades expected (CLI + existing
      `loadPackage`/`setPackageOption` cover the surface); task verifies and
      documents that stance; if install/adopt gained server-side Rust public
      functions, they get ops/facades or stay `pub(crate)`.
    - Performance: n/a.
    - Code Quality: inventory of touched Rust pub fns recorded in the task
      evidence with their disposition.
    - Security: docs note `path:` specs are CLI-user input, not reachable
      from package JS.
  - Approach:
    - Documentation Reviewed: `references/js-api.md` (boundary + schema).
    - Options Considered: expose `packages.install` as a JS API — rejected
      this phase: install stays user-initiated CLI per the distribution
      decision; revisit with the in-app package UI.
    - Chosen Approach: verification + `loadPackage` doc update noting
      source-family transparency.
    - API Notes and Examples: n/a.
    - Files to Create/Edit:
      `docs/reference/clay-js-api/packages/load-package.md` (provenance note).
    - References: decision `2026-05-08-1509`.
  - Test Cases to Write: existing registry gates stay green.

- [ ] Update the canonical example configuration (examples/config/init.js)
  - Acceptance Criteria:
    - Functional: commented section shows installing + loading a local
      package (`loadPackage("local-template")` style) with the path-source
      trust note; file stays `node --check`-clean; every option annotated
      once.
    - Performance: n/a.
    - Code Quality: section ordering follows existing file structure.
    - Security: active section stays copy-safe (local load stays commented
      since the directory won't exist on a new machine — annotated
      accordingly).
  - Approach:
    - Documentation Reviewed: `create-plan/references/clay.md` → Example
      Configuration Maintenance Task.
    - Options Considered: active local load — rejected: not copy-safe.
    - Chosen Approach: commented, clearly instructed.
    - API Notes and Examples:
      ```js
      // Local packages (installed via `clay install path:…`, then adopt):
      // loadPackage("my-theme");
      ```
    - Files to Create/Edit: `examples/config/init.js`.
    - References: user instruction 2026-08-03.
  - Test Cases to Write: `node --check` gate.

- [ ] Launch-test the app with the canonical example config
  - Acceptance Criteria:
    - Functional: scratch HOME; copied example + the template package
      installed from its fixture path via `clay install path:`; GUI launch;
      Connected; generation commits clean; the loaded local package's
      contribution visibly applies (e.g., token visible in Settings).
    - Performance: startup budgets unchanged.
    - Code Quality: commands + results in evidence.
    - Security: scratch profile only.
  - Approach:
    - Documentation Reviewed: `create-plan/references/clay.md` → Live
      Launch-Test Task.
    - Options Considered: headless-only — fallback on GUI blocker with
      headless generation-commit assertion.
    - Chosen Approach: full GUI launch.
    - API Notes and Examples: see plan 137 sibling task for the launch
      pattern.
    - Files to Create/Edit: evidence in plan file.
    - References: `plans/109-…md` review lesson (config never
      launch-tested).
  - Test Cases to Write: manual evidence.

- [ ] Execute and update the manual test plan (test-plan/)
  - Acceptance Criteria:
    - Functional: package-management module steps for path install,
      reinstall-after-edit, provenance display, remove; results recorded on
      Linux.
    - Performance: n/a.
    - Code Quality: no weakened steps; ceilings documented if pnpm missing.
    - Security: negative step: scripts never ran during install.
  - Approach:
    - Documentation Reviewed: `test-plan/index.md`.
    - Options Considered: automated-only — rejected: user-visible verb
      behavior.
    - Chosen Approach: extend the package module.
    - API Notes and Examples: n/a.
    - Files to Create/Edit: `test-plan/packages.md` (or nearest module),
      `test-plan/index.md`.
    - References: user instruction 2026-08-04.
  - Test Cases to Write: the added steps.

- [ ] Update or verify the code wiki after implementation
  - Acceptance Criteria:
    - Functional: `docs/wiki/modules/package-management.md` +
      `package-loading.md` document the `path:` family, provenance, and trust
      posture; index current.
    - Performance: none added.
    - Code Quality: docs-as-code bar.
    - Security: trust-domain wording truthful (adopted third-party; scripts
      denied; not OS-confined).
  - Approach:
    - Documentation Reviewed: `references/docs-as-code.md`.
    - Options Considered: per-task updates — rejected (churn).
    - Chosen Approach: one post-pass.
    - API Notes and Examples: n/a.
    - Files to Create/Edit: `docs/wiki/modules/package-management.md`,
      `docs/wiki/modules/package-loading.md`, `docs/wiki/index.md`.
    - References: docs-as-code reference.
  - Test Cases to Write: manual wiki review.

## Compromises Made
- To be filled after execution.

## Further Actions
- To be filled after execution. Known candidates: `github:`/`git+`/tarball
  spec families (same enum, backend delegation already modeled); an in-app
  package-manager UI (own prototype→approval loop per the UI gate).
