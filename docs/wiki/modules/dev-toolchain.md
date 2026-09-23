# Dev Toolchain

## Source
- `mise.toml`, `.github/workflows/ci.yml`
- `frontend/package.json` + `frontend/bun.lock`,
  `clay-agent/package.json` + `clay-agent/bun.lock`
- `docs/development/build-and-test.md`, `README.md`, `docs/development/windows.md`
- `scripts/check.sh`, `scripts/build.sh`, `scripts/package-smoke.sh`,
  `scripts/security-audit.sh`

## Overview

One root `mise.toml` (plan 137) pins the JavaScript runtimes exactly for both
local development and CI. Plan 138 then moved the dev workflow — dependency
installs, package scripts, and test runners — onto Bun while the daemon keeps
running on Node until plan 139, so the runtime change carries no
product-runtime risk. Rust deliberately stays outside mise: `rustup` plus the
plan-148 pin policy own the Rust toolchain.

## Responsibilities

- One version source for Bun and the transitional Node used by the daemon
  runtime, the LSP fixture suites, and the `node --expose-gc` markdown bench.
- `bun install` is the only dependency install and `bun run <script>` the only
  script entry point in CI and `scripts/`; `tsc` typechecks stay as they are.
- CI provisions the same `mise.toml` through `jdx/mise-action`, so local and CI
  cannot drift.
- Not responsible for: Rust pinning (plan 148), the published npm wrapper's
  consumer `engines` (`distribution/npm/package.json`), or the daemon runtime
  migration itself (plan 139).

## How It Works

- `mise.toml` is `[tools]`-only with exact pins:

  ```toml
  [tools]
  bun = "1.4.2"
  node = "24.21.0" # transitional: removed once plan 139 makes the daemon run on Bun
  ```

  No tasks, no ranges, no `latest`, no machine-specific paths. `mise install`
  reads the file (with a one-time trust of the project config) and installs
  both tools into mise's shared data directory; a second run is a no-op with
  no network access.
- Locally, contributors either activate mise in their shell or prefix
  commands with `mise exec -- `. `bun` and `node` on `PATH` then resolve to
  the pinned installs instead of system versions.
- Lockfiles: `frontend/` and `clay-agent/` each commit a text `bun.lock`; the
  npm `package-lock.json` files were deleted by plan 138 (the stray root stub
  had never been referenced by any tracked file). CI installs with
  `bun install --frozen-lockfile` inside each package directory, so a drifted
  lock fails the step instead of silently updating. No lockfile is ignored by
  `.gitignore`.
- Install scripts: Bun blocks package lifecycle scripts unless they are
  allowed, so `clay-agent/package.json` carries
  `"trustedDependencies": ["better-sqlite3"]`. The pinned 13.0.3 release
  ships platform prebuilds and declares no install script at all, which makes
  the entry an explicit record that the native driver is trusted should a
  future release add one. The frontend needs no entry — `bun pm untrusted`
  reports zero packages with blocked scripts.
- Runners: CI steps and `scripts/*.sh` call `bun run <script>` (or
  `bun --cwd <pkg> run <script>` from another directory). `tsc -p` and
  `tsc -b` remain the typechecks, so type errors are unchanged; only the
  installs and test runtimes moved.
  - clay-agent tests: `tsc -p tsconfig.json && bun test src --parallel` runs
    the `bun:test` suite. Scoping to `src` avoids re-running the compiled
    `dist/__tests__` copies, and `--parallel` matches the file parallelism
    the old `node --test` invocation had.
  - frontend tests: vitest 3 runs on the Bun runtime by default; the Node
    fallback (`node node_modules/vitest/vitest.mjs run`) is documented in
    `docs/development/build-and-test.md` for a future worker-pool breakage.
  - bundle budget: `check:budget` invokes `bun scripts/bundle-budget.mjs`.
- CI: `jdx/mise-action@v4` with `cache: true` replaces the former
  `actions/setup-node@v4` step; the action runs `mise install` and exports the
  tool `PATH` to later steps. Frontend and clay-agent steps then `cd` into
  their package, run `bun install --frozen-lockfile`, and the gates
  (`bun run lint|format:check|test|build|check:budget`, `bun run test`). The
  setup-node npm download cache went away by design — mise-action caches tool
  installs instead — and no npm install remains in CI.
- Remaining Node invocations, all deliberate: the daemon spawn
  (`node clay-agent/dist/main.js`, plan 139), the LSP fixture suites
  (`node --import ./tests/fixtures/lsp/register-lsp-shared.mjs --test` in
  `tests/lsp_bridge.rs` / `tests/lsp_real_servers.rs`), the markdown bench
  (`node --expose-gc tools/bench/markdown-parser.mjs`, V8 methodology), and
  `npm pack`/publish for the `@arnilo/clay` distribution wrapper.
- Windows: mise's native Windows support is newer and less exercised than
  Linux, so `docs/development/windows.md` treats it as experimental and
  documents manual installs of the same pinned versions as the supported
  fallback (`bun --cwd frontend install` and friends).

## Code Examples

```bash
curl -fsSL https://mise.run | sh   # official installer
mise install                       # pinned Bun + Node from mise.toml
mise exec -- bun --version         # 1.4.2
mise exec -- node --version        # v24.21.0

cd frontend && bun install         # from bun.lock
bun run test                       # vitest on the Bun runtime
cd ../clay-agent && bun run test   # tsc typecheck + bun:test
```

## Invariants and Constraints

- Exact pins only; version changes land in `mise.toml` and CI inherits them.
- CI must not hard-code JS versions, and tool entries never use `latest`.
- `bun.lock` is the committed lockfile per JS package; there is no
  `package-lock.json`, and CI always installs with `--frozen-lockfile`.
- Install-script trust is explicit through `trustedDependencies`; no package
  that needs a build is installed with `--ignore-scripts`.
- The Node pin exists only for the transitional daemon runtime and the
  Node-specific checks listed above; it disappears with plan 139.
- The file stays credential-free and free of machine-specific paths.
- `cargo check` at the workspace root checks the root `clay` package only
  (repo convention); desktop crates are covered by explicit CI stages.

## Tests

- `cargo test --test protocol manual_smoke_docs::` — keeps the build/test
  documentation markers intact.
- `cargo test --test protocol documentation_coverage::wiki_navigation_is_complete_and_current_page_paths_resolve`
  — keeps every wiki page indexed and every intra-wiki link resolving.
- No automated test parses `mise.toml` or the lockfiles; the CI job is the
  integration check (`jdx/mise-action`, `bun install --frozen-lockfile`, then
  the gates). Plans 137/138 record the local drills: `mise install`,
  scratch-clone installs with isolated caches, frozen-lock installs, and full
  `bun run` gate runs.

## Related

- [Maintenance Validation](maintenance-validation.md) — repository gates and
  documentation validators that this toolchain feeds.
- [clay-agent Daemon](clay-agent.md) — the Node runtime kept alive by the
  transitional pin until plan 139; its suite runs on `bun:test`.
- `docs/development/build-and-test.md`, `docs/development/windows.md`,
  `README.md`
- `plans/137-Mise-Dev-Toolchain.md`, `plans/138-Bun-Tooling-Install-Scripts-Tests.md`,
  `plans/139-Bun-Daemon-Runtime.md`,
  `plans/148-Gate-Hygiene-Flake-Determinism-and-Toolchain-Pinning.md`
- `decision-logs/2026-09-23-1946-mise-dev-toolchain.md`,
  `decision-logs/2026-09-23-1946-bun-runtime-nodejs-surfaces.md`
