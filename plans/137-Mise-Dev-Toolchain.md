# Plan 137 — mise Dev Toolchain

## Objectives
- Replace scattered dev-environment setup (CI-only Node pin, manual Bun,
  prose prerequisites) with one `mise.toml` pinning every dev-tool version.
- Make `mise install` the single fresh-clone setup command, with CI
  installing the same pinned tools so local and CI cannot drift.

## Expected Outcome
- Root `mise.toml` pins Bun (exact version) and Node (transitional exact
  version, removed by plan 139).
- CI provisions JS runtimes via the mise action instead of `setup-node`.
- `docs/development/build-and-test.md` describes mise-first setup;
  Rust stays on rustup/`rust-toolchain.toml` (plan 148 owns the pin
  policy there).

## Tasks

- [ ] Baseline: inventory current toolchain touchpoints
  - Acceptance Criteria:
    - Functional: recorded list of every place a JS runtime version is
      requested: `.github/workflows/ci.yml` (`setup-node` node-version,
      any PATH expectations), `clay-agent/package.json` engines,
      `frontend/package.json` engines (if any), dev docs, scripts that
      invoke `node`/`npm` by name, and existing `rust-toolchain.toml`
      (informational — not changed here).
    - Performance: none (inventory).
    - Code Quality: inventory committed as task evidence in the plan doc
      or test-plan artifact; each touchpoint marked keep/move/delete.
    - Security: none.
  - Approach:
    - Documentation Reviewed: `.github/workflows/ci.yml`,
      `docs/development/build-and-test.md`, `clay-agent/package.json`,
      `scripts/*` (node/npm invocations), `Cargo.toml`/rust-toolchain
      files (context only).
    - Options Considered:
      - Skip inventory, edit directly — rejected: cheap list prevents a
        missed pin that reintroduces drift.
    - Chosen Approach: grep-driven inventory, one pass.
    - Files to Create/Edit: none (evidence recorded in this plan).
    - References: decision `2026-09-23-1946-mise-dev-toolchain.md`.
  - Test Cases to Write: none.

- [ ] Add root mise.toml and local workflow
  - Acceptance Criteria:
    - Functional: `mise.toml` at repo root pins `bun = "<exact local
      verified version>"` and a transitional exact `node` version; a
      fresh scratch clone + `mise install` + `cargo check` succeeds.
    - Performance: `mise install` with warm cache completes without
      network after first run (tools cached under mise data dir).
    - Code Quality: exact pins only (no ranges), file commented with why
      node is present ("transitional until plan 139").
    - Security: no credentials or machine-specific paths in the file.
  - Approach:
    - Documentation Reviewed: mise docs — `mise.toml` `[tools]` config
      (https://mise.jdx.dev/configuration.html), tool installations
      (https://mise.jdx.dev/tools.html); local `mise --version` used to
      confirm availability.
    - Options Considered:
      - `.tool-versions` — rejected: toml is mise-native and carries
        comments.
      - Pin bun only, let node float — rejected: transitional node must
        be reproducible too.
    - Chosen Approach: exact pins in `[tools]`; nothing else (no mise
      tasks — `scripts/check.sh` remains the entry point; YAGNI).
    - API Notes and Examples:
      ```toml
      # mise.toml — one-command dev setup. Rust stays on rustup +
      # rust-toolchain.toml (plan 148 owns that pin policy).
      [tools]
      bun = "1.4.2"
      node = "24" # transitional; removed by plan 139 (bun daemon)
      ```
    - Files to Create/Edit: `mise.toml` (new).
    - References: decision `2026-09-23-1946-mise-dev-toolchain.md`;
      decision `2026-09-23-1946-bun-runtime-nodejs-surfaces.md`.
  - Test Cases to Write:
    - Scratch-clone setup: `mise install && mise exec -- bun --version`
      prints the pinned version (manual, recorded as evidence).

- [ ] CI: provision JS tools via mise
  - Acceptance Criteria:
    - Functional: `.github/workflows/ci.yml` replaces `actions/setup-node`
      with `jdx/mise-action@v2` (or current major); subsequent steps run
      under mise-provided bun/node without separate installs; full Linux
      job green.
    - Performance: CI setup step no slower than the setup-node baseline
      by more than ~30 s (cache miss tolerance); mise action caching
      enabled where supported.
    - Code Quality: one source of truth for tool versions (`mise.toml`);
      CI no longer hard-codes node/bun versions.
    - Security: action pinned by major tag like existing actions;
      no `latest` tool references.
  - Approach:
    - Documentation Reviewed: mise GitHub action README
      (https://github.com/jdx/mise-action) — install, `cache: true`,
      tool resolution from `mise.toml`.
    - Options Considered:
      - Keep setup-node + add setup-bun — rejected: two pins, drifts
        from local mise.
      - mise action — chosen: same file as local.
    - Chosen Approach: mise action with cache; steps switch from
      `npm ...` to bun commands only in plan 138 (this task only changes
      provisioning — npm still runs, on mise's node).
    - Files to Create/Edit: `.github/workflows/ci.yml`.
    - References: existing CI file; platform-validation rule (Linux job
      stays the blocking gate).
  - Test Cases to Write:
    - CI run green (recorded run link as evidence).

- [ ] Docs: mise-first setup
  - Acceptance Criteria:
    - Functional: `docs/development/build-and-test.md` setup section
      leads with `curl https://mise.run | sh` (or equivalent documented
      install), `mise install`, rustup; Node/Bun manual sections removed
      or reduced to "provided by mise".
    - Performance: none.
    - Code Quality: no stale npm/node install instructions contradicting
      mise; Windows dev notes mention mise-on-Windows is experimental and
      point at manual installs as fallback.
    - Security: install command matches mise's documented official URL.
  - Approach:
    - Documentation Reviewed: mise install docs
      (https://mise.jdx.dev/install.html); current build-and-test.md.
    - Options Considered:
      - Keep both sets of instructions — rejected: drift; one path.
    - Chosen Approach: rewrite setup section, keep rustup section.
    - Files to Create/Edit: `docs/development/build-and-test.md`.
    - References: decision `2026-09-23-1946-mise-dev-toolchain.md`.
  - Test Cases to Write: none.

- [ ] Final code-wiki task
  - Acceptance Criteria:
    - Functional: `docs/wiki/modules/` updated (new or amended node) for
      the dev toolchain change; wiki passes its maintenance validation.
    - Performance: none.
    - Code Quality: uses the standard code-wiki template per
      `.agents/skills/clay-execution/references/docs-as-code.md` and the
      task template in `.agents/skills/clay-execution/references/clay.md`.
    - Security: none.
  - Approach:
    - Documentation Reviewed:
      `.agents/skills/clay-execution/references/docs-as-code.md`,
      `.agents/skills/clay-execution/references/clay.md` (code-wiki task
      template), existing `docs/wiki/modules/` neighbors.
    - Options Considered:
      - Skip (docs-only change) — rejected: repo duty requires exactly
        one final code-wiki task per plan.
    - Chosen Approach: amend the relevant module page (or add
      `dev-toolchain.md`).
    - Files to Create/Edit: `docs/wiki/modules/**`.
    - References: clay-execution references above.
  - Test Cases to Write: none.

## Compromises Made
- To be filled after tasks are completed and tests pass.

## Further Actions
- To be filled after task completion. Known at planning time: Rust pin
  policy (rust-toolchain vs CI float) is decided in plan 148's toolchain
  task — do not duplicate it here.
