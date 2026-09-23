# Plan 138 — Bun Tooling: Install, Scripts, and Tests

## Objectives
- Move the dev workflow (dependency install, script execution, test
  running) from npm/node to bun while the daemon still runs on Node —
  zero product-runtime risk, immediate loop speedup.
- Establish bun-based CI as the default so plan 139 only flips the
  daemon spawn.

## Expected Outcome
- `bun install` replaces `npm ci` for frontend and clay-agent; bun
  lockfiles committed, npm lockfiles deleted.
- clay-agent tests run under `bun test` (`bun:test` imports); frontend
  vitest runs under bun with a verified fallback path.
- CI Linux job green with bun-only JS commands; install/test timings
  recorded before/after.

## Tasks

- [ ] Baseline: inventory npm/node usage in tooling
  - Acceptance Criteria:
    - Functional: recorded list of every npm/node invocation: CI steps,
      `package.json` scripts (frontend, clay-agent), `scripts/*.mjs`
      runners and their shebangs/invocation sites, docs references to
      npm commands, and the stray root `package-lock.json` (83-byte stub
      — identify origin, delete if unused).
    - Performance: none.
    - Code Quality: each site marked moves-to-bun / stays (tsc typecheck
      stays; it is a checker, not a runtime).
    - Security: none.
  - Approach:
    - Documentation Reviewed: `.github/workflows/ci.yml`, both
      `package.json` files, `scripts/` directory listing.
    - Options Considered:
      - Edit directly without inventory — rejected: node invocations
        hide in scripts and docs; a missed one keeps npm alive silently.
    - Chosen Approach: grep inventory first.
    - Files to Create/Edit: none (evidence in this plan).
    - References: decision `2026-09-23-1946-bun-runtime-nodejs-surfaces.md`.
  - Test Cases to Write: none.

- [ ] Dependency install moves to bun
  - Acceptance Criteria:
    - Functional: `bun install` succeeds for frontend and clay-agent
      (better-sqlite3 native build included); `bun.lock` committed per
      package (or workspace-wide if restructured — do not restructure);
      `package-lock.json` files deleted (root stub included);
      CI uses `bun install --frozen-lockfile` (or equivalent frozen
      mode).
    - Performance: cold install ≤ ~50% of the recorded `npm ci` baseline
      for the same package (record both numbers as evidence).
    - Code Quality: no npm artifacts left; `.gitignore` updated for bun
      cache artifacts if any land in-repo.
    - Security: lockfile committed (reproducible installs);
      `allowScripts` semantics preserved — bun's trustedDependencies
      lists `better-sqlite3` explicitly so postinstall scripts stay
      opt-in.
  - Approach:
    - Documentation Reviewed: bun install docs
      (https://bun.com/docs/cli/install) — lockfile, frozen mode,
      `trustedDependencies`; clay-agent `package.json` (`allowScripts`
      for `better-sqlite3@13.0.3`).
    - Options Considered:
      - Keep npm install, bun only as runner — rejected: two installers,
        two lockfiles, drift.
      - bun workspaces merge for frontend+clay-agent — rejected:
        restructures two independent packages; YAGNI now.
    - Chosen Approach: per-package `bun install`; pin-compatible
      trustedDependencies mapping for the native module.
    - API Notes and Examples:
      ```jsonc
      // clay-agent/package.json (addition)
      "trustedDependencies": ["better-sqlite3"]
      ```
      ```yaml
      # CI
      - run: bun install --frozen-lockfile
        working-directory: clay-agent
      ```
    - Files to Create/Edit: `clay-agent/package.json`,
      `clay-agent/bun.lock`, `frontend/package.json`, `frontend/bun.lock`,
      `.github/workflows/ci.yml`; delete `package-lock.json` (root),
      `clay-agent/package-lock.json`, `frontend/package-lock.json`.
    - References: bun install docs; CI file.
  - Test Cases to Write:
    - Fresh-clone drill: `bun install` in both packages from clean
      cache, then `cargo check`-equivalent JS gates pass.

- [ ] Script and runner execution via bun
  - Acceptance Criteria:
    - Functional: `frontend/scripts/bundle-budget.mjs` and any other
      node-invoked dev scripts run under `bun` (CI steps and docs
      updated); package.json scripts invoke bun-compatible commands;
      `tsc -p` remains the typecheck step (executed via `bunx tsc` or
      `bun run` — no behavior change).
    - Performance: bundle-budget check runtime within noise of node
      baseline (it is a small script; record once).
    - Code Quality: no `node ` invocations left in scripts/CI outside
      documented transitional comments.
    - Security: none.
  - Approach:
    - Documentation Reviewed: bun CLI docs (https://bun.com/docs/cli/bun
      — `bun run`, `bunx`); `scripts/` contents.
    - Options Considered:
      - Rewrite .mjs scripts as .ts — rejected: no benefit; bun runs mjs.
    - Chosen Approach: change invocations, not scripts.
    - Files to Create/Edit: `.github/workflows/ci.yml`,
      `frontend/package.json` (scripts), `docs/development/**` where
      commands are documented.
    - References: bun CLI docs.
  - Test Cases to Write: none (covered by CI green).

- [ ] clay-agent tests: node:test → bun:test
  - Acceptance Criteria:
    - Functional: all `node:test`/`node:assert/strict` imports in
      `clay-agent/src/__tests__/` (and any inline tests) map to
      `bun:test`; `bun test` runs the full suite green from `clay-agent/`;
      CI step updated; the tsc typecheck step keeps compiling tests
      (types come from bun-types or @types/node — pick one source,
      record which).
    - Performance: suite wall time ≤ node baseline (record both).
    - Code Quality: import map is mechanical (sed-level); no test logic
      changes beyond API-name differences (`assert` stays
      `node:assert/strict`-compatible via bun).
    - Security: none.
  - Approach:
    - Documentation Reviewed: bun test docs
      (https://bun.com/docs/cli/test) — `bun:test` API (test, describe,
      before/after), node compatibility notes; current
      `clay-agent/package.json` test script.
    - Options Considered:
      - Keep node:test under `bun --test` — rejected: bun's node:test
        compatibility is partial; first-party `bun:test` imports are the
        supported path.
      - Vitest for clay-agent — rejected: second framework for one
        package; bun:test matches the migration decision.
    - Chosen Approach: mechanical import migration to `bun:test`,
      keep `node:assert/strict` (bun implements it).
    - API Notes and Examples:
      ```ts
      import { describe, test, beforeEach } from "bun:test";
      import assert from "node:assert/strict";
      ```
    - Files to Create/Edit: `clay-agent/src/__tests__/**`,
      `clay-agent/package.json` (test script), `.github/workflows/ci.yml`.
    - References: bun test docs; existing suite layout.
  - Test Cases to Write:
    - Full `bun test` green locally + CI (evidence: run outputs).

- [ ] Frontend vitest under bun + fallback verification
  - Acceptance Criteria:
    - Functional: `bun run test` (vitest) executes the frontend suite
      green under the bun-provided Node API; any worker-pool
      incompatibility surfaces here, and if hit, the documented fallback
      (one CI line running vitest on mise's node) is applied and
      recorded; default path stays bun.
    - Performance: suite wall time ≤ node-vitest baseline or within 10%
      (record).
    - Code Quality: decision (bun default, fallback Y/N) recorded in
      `docs/development/build-and-test.md`.
    - Security: none.
  - Approach:
    - Documentation Reviewed: vitest docs — runtime requirements
      (https://vitest.dev/guide/), bun compatibility notes
      (https://bun.com/docs/runtime/test); current `frontend/package.json`.
    - Options Considered:
      - Migrate frontend to bun:test — rejected: RTL/vitest ecosystem
        integration risk; vitest under bun first.
    - Chosen Approach: run vitest via bun, verify, keep fallback path
      documented.
    - Files to Create/Edit: `frontend/package.json` (if script tweaks
      needed), `.github/workflows/ci.yml`, `docs/development/build-and-test.md`.
    - References: vitest + bun docs.
  - Test Cases to Write: none (suite is the check).

- [ ] Final code-wiki task
  - Acceptance Criteria:
    - Functional: `docs/wiki/modules/` node added/amended covering the
      bun-based dev workflow (install/test/scripts, lockfiles, trusted
      dependencies); maintenance validation passes.
    - Performance: none.
    - Code Quality: standard code-wiki template per
      `.agents/skills/clay-execution/references/docs-as-code.md`.
    - Security: none.
  - Approach:
    - Documentation Reviewed:
      `.agents/skills/clay-execution/references/docs-as-code.md`,
      `.agents/skills/clay-execution/references/clay.md` (template),
      existing module pages (`docs/wiki/modules/clay-agent.md` likely
      needs the test-command update).
    - Options Considered:
      - Skip — rejected: one final code-wiki task per plan is a repo duty.
    - Chosen Approach: amend `clay-agent.md` + add/adjust a dev-workflow
      module page.
    - Files to Create/Edit: `docs/wiki/modules/**`.
    - References: clay-execution references above.
  - Test Cases to Write: none.

## Compromises Made
- To be filled after tasks are completed and tests pass.

## Further Actions
- To be filled after task completion. Known at planning time: the daemon
  still runs on Node until plan 139; `clay-agent/package.json` `engines`
  switch belongs to plan 139, not here.
