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

- [x] Baseline: inventory current toolchain touchpoints
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
  - Evidence (2026-09-23):
    - Grep pass over `.github/workflows/`, `scripts/*.sh`, `package.json`
      files, `docs/development/*.md`, `clay-agent/src/main.ts`,
      `src/server/agent.rs`. Local versions observed on verification host:
      Node v26.9.0, npm 12.0.2, Bun 1.4.2, mise 2026.9.9. No
      `.nvmrc`/`.node-version`/`.tool-versions`/volta config anywhere; no
      `rust-toolchain.toml` anywhere.
    - Touchpoints (keep / move / delete):
      - `.github/workflows/ci.yml:34-37` — `actions/setup-node@v4`,
        `node-version: 24`, npm cache keyed only on
        `frontend/package-lock.json` (clay-agent cache miss): **move** to
        mise action (task 3). PATH expectation: `node`/`npm` on PATH for
        frontend gates, `clay-agent` unit tests, and
        `scripts/package-smoke.sh` (`node --check`, `npm pack`).
      - `clay-agent/package.json:13-15` — `engines.node >= 22`:
        **keep** (transitional runtime floor; plan 139 owns removal).
        Scripts `test` (`node --test`) and `start` (`node dist/main.js`):
        **move** to bun in plan 138.
      - `clay-agent/src/main.ts:7,57` — `MIN_NODE = 22` runtime guard:
        **keep** until plan 139 flips the runtime.
      - `src/server/agent.rs:1480-1496` — `resolve_node()`: `$CLAY_NODE`
        override else first `node` on `PATH`; tests that start the daemon
        therefore depend on PATH: **keep** (plan 138/139 territory).
      - `frontend/package.json` — no `engines` field; `check:budget` runs
        `node scripts/bundle-budget.mjs`; `bundle-budget.mjs` has no
        shebang, node invoked by package script: **move** to bun in plan
        138.
      - `distribution/npm/package.json:14-16` — `engines.node >= 18` for
        the published npm wrapper (`bin/clay.js`) and its install docs
        (`docs/development/distribution.md:67,104`): **keep** — consumer
        runtime constraint, outside mise.
      - `docs/development/build-and-test.md:66-119` — setup prose assumes
        PATH `node`/`npm`, no pinned version stated (drift point);
        `README.md:5-9` — `npm ci && npm run build` source-checkout
        instructions: **move** to mise-first (task 4).
      - Scripts invoking `node`/`npm` by name: `scripts/build.sh:29`,
        `scripts/editor-performance-smoke.sh:67`,
        `scripts/package-smoke.sh:29-50,81,86`,
        `scripts/security-audit.sh:15-17` (advisory `npm audit`),
        `scripts/generate-icon-packs.mjs:5` (doc comment), manual command
        in `docs/development/launch-and-gui-smoke.md:809`:
        **move** to bun commands in plan 138 (consumer `npm install -g`
        docs stay).
      - `scripts/check.sh:85` — comment only, no invocation: no change.
      - Rust toolchain: CI uses `dtolnay/rust-toolchain@stable`
        (`.github/workflows/ci.yml:14-16`); no `rust-toolchain.toml`
        exists. Informational only — plan 148 owns the pin policy.
    - Decision point for task 2: local Node is v26.9.0 while CI pins 24;
      pin the transitional `node` at **24** (matches CI/LTS) or re-verify
      at 26 deliberately. Bun pin: exact 1.4.2 (verified local).

- [x] Add root mise.toml and local workflow
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
  - Evidence (2026-09-23):
    - Added `mise.toml` (untracked; not committed):
      ```toml
      [tools]
      bun = "1.4.2"
      node = "24.21.0" # transitional: removed once plan 139 makes the daemon run on Bun
      ```
      File header comments state mise is the one-command dev setup and that
      Rust stays on rustup (plan 148 owns that pin policy).
    - Task-1 decision point resolved: node pinned at **24.21.0** (mise's
      latest 24.x, matching CI's `node-version: 24`), not the host's
      26.9.0; bun pinned at 1.4.2 (matches host `/usr/bin/bun` and mise's
      latest).
    - Working checkout, first `mise install`: `bun@1.4.2` downloaded and
      extracted in 6.2 s; `node@24.21.0` already installed via the shared
      mise cache (host global `~/.config/mise/config.toml` pins `node =
      "24"`). `mise exec -- bun --version` → `1.4.2`; `mise exec -- node
      --version` → `v24.21.0`. `mise trust` was run once on first sight of
      the new config.
    - Warm cache (acceptance perf check): second `mise install` took
      0.016 s, reported `installed 0 tools · 2 already installed`, no
      download.
    - Fresh scratch clone `/tmp/clay-mise-scratch` (`git clone` of HEAD;
      the untracked `mise.toml` copied in), `mise install` exit 0 with no
      trust prompt and both tools already installed from the shared cache;
      `mise exec -- bun --version` → `1.4.2`, `mise exec -- node --version`
      → `v24.21.0`, resolving to
      `~/.local/share/mise/installs/{bun,node}/...` paths.
    - Scratch-clone `CARGO_TARGET_DIR=<main checkout>/target mise exec --
      cargo check` exit 0 in 16.09 s. Compromise recorded: the shared
      target dir was reused to avoid a ~cold full dependency compile; the
      clone, mise install, and tool resolution were genuinely fresh, and
      `cargo check` at the workspace root checks the root `clay` package
      (repo convention — `scripts/check.sh` stages are root-scoped and
      desktop crates are checked explicitly).
    - Scratch clone removed after the drill.

- [x] CI: provision JS tools via mise
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
  - Evidence (2026-09-23):
    - `.github/workflows/ci.yml:34-40`: `actions/setup-node@v4` block
      (node-version 24 + npm cache) removed; `jdx/mise-action@v4` (current
      major — the plan's `@v2` is superseded) added with `cache: true`.
      npm gate steps unchanged (plan 138 swaps them to bun). Comment notes
      mise.toml is the single version source and why npm steps remain.
    - Security: action pinned by major tag matching the existing pattern
      (`actions/checkout@v4`, `taiki-e/install-action@v2`); tool versions
      stay exact in `mise.toml`, no `latest` references.
    - Code Quality: CI no longer hard-codes any JS version; `node-version:
      24` and the setup-node npm cache block are gone.
    - Known tradeoff: setup-node's `~/.npm` cache went away (mise-action
      caches tool installs instead), so `npm ci` runs cold; accepted within
      the ~30 s cache-miss tolerance and moot once plan 138 moves installs
      to bun.
    - Local validation: `actionlint` v1.7.12 → exit 0 for
      `.github/workflows/ci.yml`.
    - Local CI-step simulation under `mise exec --` (mise-provided PATH:
      node v24.21.0, npm 11.19.0, bun 1.4.2): ran the workflow's exact JS
      steps — frontend `npm ci`, lint, format:check, test, build,
      check:budget (22 s) and clay-agent `npm ci` + `npm test` (17 s, 182
      pass / 1 skip / 0 fail) — exit 0 throughout, confirming downstream
      steps work on mise-provided node/npm with no separate installs.
    - CI-run evidence still pending: `migration/bun` is not on origin,
      CI triggers only on `main` push or a PR, and this host has no `gh` CLI
      or GitHub token to open a PR or read run status. Push/PR the branch
      (commit `mise.toml` + `.github/workflows/ci.yml` together) and record
      the run link here to close the acceptance test case.

- [x] Docs: mise-first setup
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
  - Evidence (2026-09-23):
    - `docs/development/build-and-test.md`: new `### Dev toolchain (mise +
      rustup)` section before `### Linux prerequisites` — official mise
      installer `curl -fsSL https://mise.run | sh`, then `mise install`,
      conditional `mise trust`, and the rustup installer; activation or
      `mise exec -- ` explained; states no separate Linux Node/Bun install
      is needed and CI uses the same file via `jdx/mise-action`. The
      `### Frontend gates` section now notes commands run under the
      mise-provided Node/npm.
    - No manual Node/Bun install sections existed in the docs, so nothing
      was removed; npm steps remain intentionally until plan 138 swaps
      them to bun.
    - `README.md` source-checkout block now requires rustup + mise and
      runs `mise install` first (was bare `npm ci`).
    - `docs/development/windows.md`: new `### JavaScript runtimes`
      subsection under Install Prerequisites — pins named (Bun 1.4.2,
      Node 24.21.0), mise's native Windows support marked experimental for
      Clay's purposes, manual exact-version fallback (nodejs.org or
      `winget` LTS, bun.sh) with `node`/`npm` on PATH.
    - Security: commands use the official documented URLs (`mise.run`,
      `sh.rustup.rs`); no credentials or machine-specific paths added.
    - No stale contradictory install instructions remain (grep pass over
      `docs/development/`, `README.md`); distribution docs keep consumer
      `npm install -g` guidance by design.
    - Verification: `cargo test --test protocol manual_smoke_docs::` → 26
      passed, 0 failed, confirming the docs markers those suites pin are
      intact.

- [x] Final code-wiki task
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
  - Evidence (2026-09-23):
    - Added `docs/wiki/modules/dev-toolchain.md` following the page template
      in `.agents/skills/clay-execution/references/docs-as-code.md`
      (Source / Overview / Responsibilities / How It Works / Code Examples /
      Invariants and Constraints / Tests / Related). It covers the exact
      `mise.toml` pins and why Node is transitional, local activation vs
      `mise exec --`, the CI `jdx/mise-action@v4` provisioning and the lost
      setup-node npm cache, the Windows manual-install fallback, ownership
      boundaries (Rust = plan 148, npm consumer `engines`, daemon migration =
      plans 138/139), and the repo-convention root-package scope of
      `cargo check`.
    - `docs/wiki/index.md`: linked next to Maintenance Validation with a
      one-line scope description.
    - Verification: `cargo test --test protocol
      documentation_coverage::wiki_navigation_is_complete_and_current_page_paths_resolve`
      and `cargo test --test protocol
      primitives_docs::wiki_index_links_every_wiki_page` — both pass (wiki
      indexing and intra-wiki link resolution intact).

## Compromises Made
- CI-run evidence for task 3 is still pending: `migration/bun` is not on
  origin, the workflow triggers only on `main` push or a PR, and this host
  has no `gh` CLI or GitHub token. Local equivalents (actionlint, exact JS
  steps under `mise exec --`, scratch-clone `mise install`) are green and
  recorded in the task evidence; a push/PR must supply the run link.
- The task-2 scratch-clone `cargo check` reused the main checkout's
  `target/` to avoid a cold full dependency compile; the clone, `mise
  install`, and tool resolution were genuinely fresh, and the workspace-root
  check scope matches repo convention.
- Replacing `actions/setup-node` dropped its `~/.npm` cache, so `npm ci`
  runs cold in CI until plan 138 moves installs to bun; accepted within the
  ~30 s cache-miss tolerance.
- `mise trust` remains a conditional documented step; the scratch-clone
  drill needed no trust prompt.

## Further Actions
- Push `migration/bun` (commit `mise.toml`, `.github/workflows/ci.yml`,
  docs/README/wiki, and this plan together) and record the CI run link in
  task 3 evidence; open a PR because CI runs on `main` pushes and PRs only.
  Priority: high — closes the one open acceptance test case in this plan.
- Plan 138: swap npm installs/tests to bun, then update
  `docs/wiki/modules/dev-toolchain.md`, the build-and-test docs, and the
  workflow comment to drop the npm-cache tradeoff note. Priority: next plan
  in sequence.
- Plan 139: delete the transitional `node` pin from `mise.toml` and remove
  the wiki/docs/README references that explain why it exists. Priority:
  with the daemon migration.
- Plan 148: Rust pin policy (rust-toolchain.toml vs CI float) stays outside
  `mise.toml` as planned. Priority: plan-148 scope.
- If mise-action's floating default ever flakes CI, pin its `version:`
  input; no evidence it is needed today. Priority: low.
