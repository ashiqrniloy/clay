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

- [x] Baseline: inventory npm/node usage in tooling
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
  - Evidence (2026-09-24):
    - Grep pass on `migration/bun` (HEAD `73d84cd`, plan-137 changes
      uncommitted in the working tree) over `.github/workflows/ci.yml`,
      `frontend/package.json`, `clay-agent/package.json`, `scripts/*`,
      `frontend/scripts/*`, `tools/*`, `tests/*.rs`, `mise.toml`,
      `README.md`, `clay-agent/README.md`, `docs/development/**`,
      `docs/wiki/**`, `.claude/`. Legend: **MOVE** = becomes a bun
      invocation (owning task in parentheses); **STAY** = unchanged, with
      the reason; "checker" marks tsc/`--check` steps, which are
      verification, not runtimes (per task AC, `tsc` stays).
    - CI — `.github/workflows/ci.yml`:
      - `:35` comment "Keep npm steps below until plan 138 swaps
        installs/tests to bun" — **MOVE** (task 3 removes the comment).
      - `:42` `npm ci --prefix frontend` — **MOVE** to `bun install
        --frozen-lockfile` (task 2).
      - `:43-47` `npm run lint|format:check|build|check:budget`, `npm
        test --prefix frontend` — **MOVE** invocation to `bun run` (task
        3); `check:budget` additionally moves off `node` inside the
        script (below).
      - `:50` `npm ci --prefix clay-agent` — **MOVE** (task 2).
      - `:51` `npm test --prefix clay-agent` — **MOVE** to `bun test`
        (task 4).
      - Root `mise.toml` node 24.21.0 stays on PATH transitionally, so
        no CI provisioning change is needed for these swaps.
    - `frontend/package.json` scripts — watch for the distinction
      between the package-manager invocation (moves to bun) and the
      script body (mostly unchanged):
      - `dev`, `preview`, `lint`, `format`, `format:check` — vite,
        eslint, prettier; runner-agnostic bodies — **MOVE** invocation to
        `bun run` (task 3).
      - `build` = `tsc -b && vite build`; `typecheck` = `tsc -b --pretty
        false` — `tsc` is a **checker: STAY** (executed via `bun run`/
        `bunx`); vite half is runner-agnostic.
      - `test` = `vitest run` — **MOVE** to run under bun (task 5);
        script text unchanged, fallback documented if workers
        misbehave.
      - `check:budget` = `node scripts/bundle-budget.mjs` — **MOVE** to
        `bun scripts/bundle-budget.mjs` (task 3).
    - `clay-agent/package.json`:
      - `build` = `tsc -p tsconfig.json` — **checker: STAY**.
      - `test` = `tsc -p tsconfig.json && node --test
        dist/__tests__/*.js` — **MOVE** to `tsc -p tsconfig.json && bun
        test` (task 4; the pre-compile `tsc` stays).
      - `start` = `node dist/main.js`; `engines.node >= 22` — **STAY**
        until plan 139 (daemon runtime).
      - `allowScripts` `better-sqlite3@13.0.3` — **MOVE** to bun
        `trustedDependencies: ["better-sqlite3"]` (task 2).
    - `scripts/` and runners (shebangs/invocation sites):
      - `scripts/generate-icon-packs.mjs` — no shebang; header `:5` says
        `node scripts/generate-icon-packs.mjs` — **MOVE** doc; its
        blocking invocation is `tests/icon_packages.rs:130`
        `Command::new("node")` inside `cargo test` — **MOVE** to bun
        (bun runs `.mjs`; task 3); docs
        `docs/wiki/modules/icon-pack-runtime.md:166` — **MOVE**.
      - `frontend/scripts/bundle-budget.mjs` — no shebang; sole
        invocation is `frontend/package.json check:budget` via `node` —
        **MOVE** (task 3); in-file hints `:1`, `:17` say `npm run
        build` — **MOVE** wording.
      - `tools/bench/markdown-parser.mjs` — documented as `node
        --check` / `node --expose-gc` (`docs/development/performance.md`
        `:77-79,588,606`; `docs/wiki/modules/first-party-markdown-package.md`
        `:85,88`) — **STAY** (benchmark methodology depends on V8
        `--expose-gc`; it measures Node-runtime memory); `:152` error
        hint `npm install --prefix packages/markdown --no-save ...` plus
        `docs/development/performance.md:76` and the same wiki page `:87`
        — **MOVE** to the bun equivalent (flag parity checked in task 3).
      - `scripts/build.sh:29` `npm run build` — **MOVE**; `:16` process
        match `node … clay-agent/dist/main.js` — **STAY** until 139
        (daemon still Node).
      - `scripts/check.sh:85` — comment-only `npm run build --prefix
        frontend` — **MOVE** comment (task 3).
      - `scripts/editor-performance-smoke.sh:67` `npm run build` —
        **MOVE**.
      - `scripts/package-smoke.sh` — `:29-35` `node --check
        examples/config/*.js` (guarded by `command -v node`) — **MOVE**
        to a bun parse check (exact command decided in task 3); `:46-48`
        `npm pack --dry-run` on `distribution/npm` — **STAY** (npm CLI
        is the reference packer for the published npm channel); `:81`
        `npm run check:budget --prefix frontend` — **MOVE**; `:86` `npm
        test --prefix clay-agent` — **MOVE** (task 4).
      - `scripts/security-audit.sh:14-17` advisory `npm audit --prefix
        frontend --omit=dev` — **MOVE** to `bun audit` (task 3; parity
        confirmed in task evidence; advisory either way).
      - `tests/lsp_bridge.rs:19`, `tests/lsp_real_servers.rs:36` —
        `Command::new("node") --import … --test` over
        `packages/lsp-*/adapter.test.mjs` and
        `tests/fixtures/lsp/**` — **STAY transitional** (fixture suites
        use `node:test`, whose bun compatibility is partial — the same
        reason task 4 rejects it for clay-agent; migration is a plan-139/
        follow-up tooling task, not this plan).
        `docs/development/launch-and-gui-smoke.md:809` repeats that
        command — **STAY** to match.
      - Runtime (not dev tooling): `src/server/agent.rs` `resolve_node()`
        (`$CLAY_NODE` else `node`), `clay-agent/src/main.ts` `MIN_NODE`
        guard — **STAY** until plan 139.
      - `.claude/helpers/graft-hooks.cjs` / `graft-statusline.cjs` —
        invoked from `.claude/settings.json` as `node …`, with an `npm
        root -g` fallback — **STAY** (agent-local graft tooling, outside
        the Clay dev/CI workflow; unaffected by bun install).
      - `design-artifacts/tools/*.mjs` — one-off capture scripts whose
        usage comments say `npm run dev` (`verify-component-conformance.mjs`
        even carries `#!/usr/bin/env node`) — **STAY** (historical
        design-artifact tooling, not the dev workflow).
    - Lockfiles and the root stub:
      - `frontend/package-lock.json` (175,757 B) and
        `clay-agent/package-lock.json` (29,281 B) — **MOVE** to
        `bun.lock` per package, npm locks deleted (task 2). No `bun.lock`
        exists yet in either package.
      - Root `package-lock.json` (83 B: `{"name":"clay",
        "lockfileVersion":3, "requires":true, "packages":{}}`) was added
        by commit `1bfe9c0` "Bug fixes and Prism version bump"
        (2026-09-08, Niloy Rahman) and **is the only file in that
        commit**. No root `package.json` has ever existed (`git log
        --all -- package.json` is empty), no CI step or script runs
        `npm ci` at the root (all use `--prefix` or `cd`), and no
        tracked file references the root lock. Origin: an accidental
        root-level npm invocation. **Deleted in this task**
        (`git rm package-lock.json`); task 2's deletion list is reduced
        by this one file.
    - Docs references (active docs to update in tasks 3-5):
      - `README.md:11` — `cd frontend && npm ci && npm run build` —
        **MOVE** (task 3).
      - `docs/development/build-and-test.md:109,123,133,137-142,194` —
        build-run examples, "mise-provided Node/npm", frontend gate
        block, `npm run build` timing baseline — **MOVE** (task 3).
      - `docs/development/windows.md:36-40` (manual node/npm fallback
        prose) and `:97-98` (`npm --prefix frontend ci|run build`) —
        **MOVE** to bun fallback (tasks 2-3).
      - `docs/development/ui-observability.md:60` — `npm --prefix
        frontend run test` — **MOVE**.
      - `docs/development/performance.md:76,1172` — markdown-it install
        hint and `npm --prefix frontend run check:budget` — **MOVE**
        (the node bench invocations above **STAY**).
      - `docs/wiki/flows/document-chunked-loading.md:198`,
        `editor-viewport-render-patch.md:147`,
        `frontend-edit-synchronization.md:103` — `npm test -- --run …`
        — **MOVE** to bun-run vitest (task 3).
      - `docs/wiki/modules/` — **MOVE**: `clay-agent.md:986,989` (test
        only; `:795` `node clay-agent/dist/main.js` **STAYS** until
        139), `dev-toolchain.md:11,41,44-47`,
        `icon-pack-runtime.md:166,185`, `configuration-runtime.md:256`
        (`node --check examples/…`), `decoration-transport.md:166`,
        `desktop-typed-bridge.md:208`, `desktop-release-hardening.md:53`
        (npm-audit wording), `folding-ranges.md:135`,
        `performance-fixtures.md:220`, `range-diagnostics.md:144`,
        `react-codemirror-editor.md:230`,
        `react-command-centre-desktop-workflows.md:141`,
        `react-sdui-package-ui.md:209`, `react-shell.md:227`,
        `react-tabs-and-splits.md:72`, `tauri-react-cutover.md:33-39`.
      - `clay-agent/README.md:50,137,139` — `npm rebuild
        better-sqlite3`, `npm install`, `npm test` — **MOVE** (task 3/4;
        `:9` `node dist/main.js` **STAYS** until 139).
      - `docs/reference/packages/creating-packages.md:1850` — `npx
        vitest … && npm run build && npm run check:budget` — **MOVE**
        (task 3).
    - Docs references that **STAY** (not the npm/node dev loop):
      - `docs/development/distribution.md:53,67,104,110` — npm
        registry publish/`npm install -g` consumer channel (product).
      - `docs/wiki/modules/package-management.md:39,127,144`,
        `docs/wiki/index.md:95` — Clay's product package-manager
        backend (npm/pnpm), not dev tooling.
      - `docs/development/client-local-parsing-spike-2026-08-26.md:9,16`
        and `docs/development/tauri-react-parity-ledger.md:100,112` —
        historical records; the spike's `tools/spikes/` path no longer
        exists.
      - `test-plan/**` (`npx vitest` run records), `VENT.md`,
        `code-reviews/**`, `roadmap.md:888` — historical/product-design
        records, never rewritten.
      - `npx skills` (`examples/config/README.md:46`,
        `packages/coding-agent/docs`), `npx tree-sitter`
        (`packages/*/grammars/PROVENANCE.md`), `npx ctx7` (`AGENTS.md`),
        `npx graft` (`.claude/settings.json`) — third-party CLIs.
    - Decision points handed to later tasks: (1) bun substitute for
      `node --check` in `package-smoke.sh` and
      `configuration-runtime.md`; (2) `bun audit` parity with
      `npm audit --omit=dev` on bun 1.4.2; (3) bun equivalent of `npm
      install --no-save --no-package-lock --ignore-scripts
      markdown-it@^14.1.0` for the bench; (4) LSP `node:test` fixture
      suites stay on node in plan 138 (documented transitional).

- [x] Dependency install moves to bun
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
  - Evidence (2026-09-24):
    - Documentation: bun v1.4.2 docs via ctx7 — `bun install` lockfile
      and `--frozen-lockfile`/`install.frozenLockfile` (fail when
      `package.json` and lock disagree); `trustedDependencies`
      "replaces Bun's default allowlist rather than extending it"
      (`docs/guides/install/trusted.mdx`), `bun pm trust`,
      `--ignore-scripts`.
    - Files changed:
      - `clay-agent/package.json` — `allowScripts` replaced with
        `"trustedDependencies": ["better-sqlite3"]`. Note: installed
        `better-sqlite3@13.0.3` declares no install/postinstall script
        and ships platform prebuilds (`prebuilds/linux-x64.node`), so
        the entry is the explicit opt-in record for a future version
        that adds one; it appears in the lockfile trust list
        (`clay-agent/bun.lock`).
      - `frontend/package.json` — no change needed: no frontend install
        script is required (bun's default allowlist covers the tree;
        `bun pm untrusted` reports `0 untrusted dependencies with
        scripts` in both packages).
      - `.github/workflows/ci.yml` — installs swapped to
        `(cd frontend && bun install --frozen-lockfile)` and
        `(cd clay-agent && bun install --frozen-lockfile)`; npm
        script/test steps stay for tasks 3/4. YAML parsed with
        `Bun.YAML.parse` (actionlint not installed on this host).
      - Deleted `frontend/package-lock.json` and
        `clay-agent/package-lock.json` (root stub already deleted in
        task 1).
      - Added `frontend/bun.lock` (83,014 B, lockfileVersion 2, 357
        `sha512-` integrity entries) and `clay-agent/bun.lock`
        (14,173 B, 60 `sha512-` entries), both untracked pending the
        branch commit. No `.gitignore` change: bun writes nothing
        in-repo outside `node_modules/`, already ignored.
    - Timings (mise node 24.21.0/npm 11.19.0, bun 1.4.2, Linux;
      `node_modules` removed before every run):
      | package | `npm ci` warm | `bun install --frozen-lockfile` warm | ratio | `npm ci` cold cache | bun frozen cold cache | ratio |
      |---|---|---|---|---|---|---|
      | frontend | 2.831 s | **0.297 s** | 10.5% | 4.681 s | 3.908 s | 83% |
      | clay-agent | 1.096 s | **0.338 s** | 30.8% | 3.802 s | 3.963 s | 104% |
      - Warm-cache (the steady-state local/CI comparison) clears the
        ≤50% target by a wide margin. Isolated-cache cold installs
        (`npm_config_cache` / `BUN_INSTALL_CACHE_DIR` fresh dirs) are
        network-bound and land at roughly parity; recorded in
        compromises.
    - Native module check: `better-sqlite3` loads and executes SQL under
      both runtimes after the bun install —
      `node -e "require('better-sqlite3') …"` OK and
      `bun -e "import Database from 'better-sqlite3' …"` OK.
    - Fresh-clone drill: local clone of HEAD `73d84cd` with the
      uncommitted `mise.toml`, both `bun.lock`s, and
      `clay-agent/package.json` copied in and npm locks removed; cold
      isolated bun caches; `--frozen-lockfile` exit 0 — frontend
      3.503 s, clay-agent 3.826 s. Gates then run from the clone via
      `bun run`:
      - frontend: lint PASS, format:check PASS, test PASS (vitest 51
        files / 497 tests, 10.60 s — already green under bun, formal
        comparison is task 5), build PASS (3.07 s; shell gzip 176.9/
        180 kB, total 399.9/404 kB), check:budget PASS.
      - clay-agent: `bun run build` (tsc) PASS; `bun run test` (current
        `tsc … && node --test`, migration is task 4) PASS 182/0.
    - Security: committed lockfiles carry sha512 integrity; CI uses
      frozen mode; the trust list is explicit in `package.json` and
      mirrored in `clay-agent/bun.lock`; nothing ran untrusted
      lifecycle scripts (`bun pm untrusted` clean in both packages).

- [x] Script and runner execution via bun
  - Acceptance Criteria:
    - Functional: `frontend/scripts/bundle-budget.mjs` and any other
      node-invoked dev scripts run under `bun` (CI steps and docs
      updated, including removing the "npm steps stay until plan 138"
      comment above `jdx/mise-action` in `.github/workflows/ci.yml`);
      package.json scripts invoke bun-compatible commands;
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
  - Evidence (2026-09-24):
    - CI (`.github/workflows/ci.yml`): the "Keep npm steps below until
      plan 138" comment is gone; the frontend step is now
      `cd frontend` + `bun install --frozen-lockfile` + `bun run
      lint`/`format:check`/`test`/`build`/`check:budget`, and the
      clay-agent step is `cd clay-agent` + `bun install --frozen-lockfile`
      + `bun run test`. The clay-agent package script still invokes
      `node --test` until task 4; the CI/script surface itself no longer
      calls npm. YAML re-parsed with `Bun.YAML.parse`.
    - Invocations changed, scripts not rewritten:
      - `frontend/package.json`: `check:budget` → `bun
        scripts/bundle-budget.mjs`; `bundle-budget.mjs` header and
        missing-dist error now say `bun run build`.
      - `scripts/build.sh` / `scripts/editor-performance-smoke.sh`:
        frontend builds via `bun run build`. build.sh's `node …daemon`
        pkill pattern gains a comment marking it transitional until
        plan 139.
      - `scripts/package-smoke.sh`: `node --check` → `bun build
        --no-bundle` (parse/transpile only — no execution or import
        resolution) gated on `command -v bun`; budget and clay-agent
        tests now `(cd … && bun run …)`.
      - `scripts/security-audit.sh`: `npm audit --prefix frontend
        --omit=dev` → `(cd frontend && bun audit)` gated on
        `frontend/bun.lock`. The old block keyed on the deleted
        `package-lock.json` and had been silently skipped since task 2.
        Header notes bun 1.4.2 audit has no `--omit=dev`, so
        devDependencies are included and it stays advisory.
      - `scripts/check.sh`: stale `npm run build` comment → `cd frontend
        && bun run build`.
      - `tests/icon_packages.rs`: drift test spawns `bun
        scripts/generate-icon-packs.mjs --check` (expect message updated
        to bun/CI-provides-via-mise).
      - `scripts/generate-icon-packs.mjs` header and
        `tools/bench/markdown-parser.mjs` missing-dependency hint moved
        to bun commands.
    - Docs: `docs/development/build-and-test.md` (mise-exec toolchain
      wording, local-build equivalent, Vite hot-reload, Frontend gates
      block all bun; the historical Phase 2/4 baseline lines keep the
      npm command they were measured with), `docs/development/windows.md`
      (manual fallback needs `bun`; PowerShell block uses `bun --cwd`),
      `docs/development/ui-observability.md`,
      `docs/development/performance.md` (bench install is now `bun add
      --cwd packages/markdown --no-save --ignore-scripts
      markdown-it@^14.1.0`; budget gate `bun --cwd frontend run
      check:budget`), root `README.md`, `clay-agent/README.md`.
    - Performance (bundle-budget script, 3 runs each on the same built
      tree): node baseline 0.060/0.063/0.061 s; bun
      0.038/0.040/0.040 s — bun is faster, comfortably inside the
      "within noise" bar.
    - Validation:
      - Frontend gates under bun: lint PASS, format:check PASS, test
        PASS, build PASS, check:budget PASS (shell gzip 178.6/180 kB,
        total 403.5/404 kB).
      - clay-agent `bun run test` PASS: 182 pass / 1 skip / 0 fail,
        10.83 s (script body still `tsc && node --test`).
      - `cargo test --test presentation
        icon_pack_generation_is_deterministic_and_drift_free` PASS —
        the bun-spawned drift check is exercised.
      - `scripts/package-smoke.sh` PASSED end to end; log shows the bun
        syntax-check branch, the npm-wrapper `npm pack --dry-run`
        (stays: product distribution), the bun budget gate, and the
        clay-agent tests.
      - `scripts/security-audit.sh` PASSED: cargo audit clean, bun-audit
        branch taken (3 advisories = 1 high / 2 moderate, dev-inclusive,
        advisory only).
      - `bun add --cwd packages/markdown --no-save --ignore-scripts
        markdown-it@^14.1.0` verified: installs only into
        `packages/markdown/node_modules` (removed again), no
        `bun.lock`/`package.json` change; the node-based bench then ran
        a 64 KiB markdown-it sample.
      - `bash -n` clean on every touched shell script.
    - Documented stays: `node --expose-gc` markdown bench (V8 GC
      methodology), LSP `node --test` fixture suites, daemon
      spawn/pkill/usage (plan 139), `npm pack` for the `@arnilo/clay`
      wrapper, `.claude` hooks, and `node:*` builtin imports (bun
      implements them).

- [x] clay-agent tests: node:test → bun:test
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
  - Evidence (2026-09-24):
    - Import migration (mechanical, sed-level): all 28
      `src/__tests__/*.test.ts` now `import { test } from "bun:test"`;
      the two default imports (`context.test.ts`, `om.test.ts`) became
      named imports (bun:test has no default export).
      `node:assert/strict` stays in all 28 files (bun implements it).
      Diff check: 25 files are 1-line import swaps; only the three below
      differ.
    - Three API differences surfaced by tsc/the runner, all minimal:
      - `obscura-web-surface.test.ts`: node:test options-object
        `test(name, { skip: !env }, fn)` → `test.skipIf(!env)(name, fn)`
        (bun:test has no options-object overload).
      - `mcp-obscura.test.ts`: explicit `20_000` timeout on the "Obscura
        harness fails to spawn" test. bun:test defaults to a 5 s
        per-test timeout, node:test has none. The test is green but takes
        ~10.0 s because the CDP readiness path fail-closes on a dead
        binary only after its own timeout.
      - `resume.test.ts`: 5 ms yield before the prompt that must bump
        `updated_at`. Prism orders by `updated_at DESC, id DESC` at
        millisecond resolution; under bun the creations and the prompt
        landed in the same millisecond and the random-id tie-break won.
        Node had passed only because it was slower — a latent flake. No
        assertion semantics changed.
    - Types: devDependency `@types/bun@^1.4.2` added (resolves 1.4.2,
      pulls `bun-types@1.4.2`); `clay-agent/tsconfig.json` types is now
      `["node", "bun"]`. `bun-types` itself depends on `@types/node: *`,
      so bun:test API types come from one source while the existing
      `node:*` builtin typings stay backed by `@types/node@^24.3.0`.
      `tsc -p tsconfig.json` still compiles every test file (kept in the
      test script).
    - Script: `"test": "tsc -p tsconfig.json && bun test src --parallel"`.
      `src` scoping prevents bun test from also discovering the compiled
      `dist/__tests__/*.js` copies (56 files instead of 28 otherwise).
      `--parallel` (worker processes, implies per-file isolation) matches
      `node --test`'s file parallelism; bun's sequential default ran the
      suite in ~29.9 s. CI already invokes `bun run test` (task 3), so no
      separate `ci.yml` edit was needed.
    - Performance (clay-agent/):
      | run | result | wall |
      |---|---|---|
      | old script (HEAD worktree: `tsc && node --test dist`) | 182/1/0 | 12.489 / 12.496 s |
      | `node --test dist` suite only (pre-migration) | 182/1/0 | 10.978 / 11.444 s |
      | `bun test src --parallel` suite only | 182/1/0 | 10.583 / 10.959 s |
      | new script (`tsc && bun test src --parallel`) | 182/1/0 | 13.395 / 12.139 s |
      Suite wall time is ≤ the node baseline; the full script is within
      noise of the old one (tsc dominates the spread).
    - Validation: `bun run test` green twice — 183 tests across 28 files,
      182 pass / 1 skip / 0 fail; the skip is the opt-in `CLAY_WEB_E2E`
      CDP test. `scripts/package-smoke.sh` PASSED end to end (log shows
      `$ tsc -p tsconfig.json && bun test src --parallel` and
      `bun test v1.4.2 … 16x PARALLEL`).
    - Note: tests that spawn the daemon or the MCP fixture use
      `process.execPath`, so those paths exercised the daemon on Bun as a
      side effect of the migration.

- [x] Frontend vitest under bun + fallback verification
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
  - Evidence (2026-09-24):
    - Runtime: vitest 3.2.7 (frontend, default `forks` pool) executed by
      bun 1.4.2 from mise. `bun run test` is green twice: 51 files /
      497 tests passed, no worker-pool incompatibility, no Bun-specific
      warnings (the `environmentMatchGlobs` and react-aria `<Section>`
      deprecations are pre-existing and appear under Node too).
    - Performance:
      | run | wall |
      |---|---|
      | `node node_modules/vitest/vitest.mjs run` (mise Node 24.21.0) | 9.579 / 9.778 s |
      | `bun run test` (bun 1.4.2) | 8.986 / 9.270 s |
      Bun is at or under the Node baseline, so the ≤ +10% allowance was
      not needed.
    - Decision recorded in `docs/development/build-and-test.md`
      (Frontend gates): Bun stays the default runner, fallback NOT
      applied; the fallback command
      `node node_modules/vitest/vitest.mjs run` is documented for a
      future pool breakage and would be a one-line CI swap of
      `bun run test`.
    - Files: only `docs/development/build-and-test.md` changed.
      `frontend/package.json` (`"test": "vitest run"`) and `ci.yml`
      (already `bun run test`) need no edit — the script is
      runtime-agnostic.

- [x] Final code-wiki task
  - Acceptance Criteria:
    - Functional: `docs/wiki/modules/dev-toolchain.md` updated to the
      bun-based dev workflow (install/test/script commands, lockfiles,
      trusted dependencies; the setup-node npm-cache tradeoff note and
      the "npm stays until plan 138" framing drop out), and
      `clay-agent.md` amended for its bun-side test command;
      maintenance validation passes.
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
    - Chosen Approach: amend `docs/wiki/modules/dev-toolchain.md` +
      `clay-agent.md` (the dev-workflow page already exists from plan
      137).
    - Files to Create/Edit: `docs/wiki/modules/**`.
    - References: clay-execution references above.
  - Test Cases to Write: none.
  - Evidence (2026-09-24):
    - `docs/wiki/modules/dev-toolchain.md` rewritten to the Bun workflow:
      `bun install`/`bun run` as the only install/script entry points, one
      `bun.lock` per package with CI `--frozen-lockfile`, the
      `trustedDependencies` install-script policy (better-sqlite3 prebuild
      note), `bun test src --parallel` + `bun:test` for clay-agent, vitest
      on Bun with the documented Node fallback, and the deliberate
      remaining Node invocations (daemon spawn, LSP fixture suites,
      `node --expose-gc` bench, npm wrapper pack/publish). The
      `actions/setup-node` npm-cache tradeoff note and the "npm stays until
      plan 138" framing are gone. Source list now names both
      `package.json`/`bun.lock` pairs and the shell scripts that drive the
      toolchain.
    - `docs/wiki/modules/clay-agent.md` test section: command is now
      `cd clay-agent && bun run test`, with a short explanation
      (`tsc -p tsconfig.json` typecheck first, then
      `bun test src --parallel`; `src` scope skips compiled `dist/` copies).
      The historical Plan 119 verification sentence keeps its `npm test`
      wording as a record of that run.
    - Sweep of stale dev-command examples in the wiki (classified MOVE in
      task 1, still on npm after task 3's docs/development edits):
      flows `document-chunked-loading`,
      `editor-viewport-render-patch`, `frontend-edit-synchronization`; module
      pages `decoration-transport`, `desktop-typed-bridge`, `folding-ranges`,
      `icon-pack-runtime` (plus its `node scripts/generate-icon-packs.mjs`),
      `performance-fixtures`, `range-diagnostics`, `react-codemirror-editor`,
      `react-command-centre-desktop-workflows`, `react-sdui-package-ui`,
      `react-shell`, `react-tabs-and-splits`, `tauri-react-cutover` (gate
      list), `first-party-markdown-package` (bench install),
      `desktop-release-hardening` (`bun audit`). Patterns:
      `npm test -- --run X` → `bun run test X`,
      `npm --prefix <pkg>` → `bun --cwd <pkg> run`. Product npm references
      (`package-management.md`, `package-loading.md`,
      `package-primitive-gate.md`) are untouched. Argument forwarding was
      verified once (`bun run test src/editor/sync/session.test.ts` → 1 file,
      16 tests).
    - `docs/wiki/index.md`: the Dev Toolchain blurb now covers plan 138's
      Bun workflow; clay-agent's blurb is unchanged because the daemon
      runtime is still Node until plan 139.
    - Maintenance validation:
      `mise exec -- cargo test --test protocol -- documentation_coverage:: manual_smoke_docs::`
      → 42 passed, 0 failed (50.32 s), including
      `wiki_navigation_is_complete_and_current_page_paths_resolve` and the
      manual-smoke marker checks.
    - No new decision log: the bun-tooling decision is already recorded in
      `decision-logs/2026-09-23-1946-bun-runtime-nodejs-surfaces.md`, which
      names plan 138.

## Compromises Made
- Cold-cache installs only reach npm parity: with an isolated empty cache,
  `bun install --frozen-lockfile` took 3.908 s (frontend, 83% of `npm ci`'s
  4.681 s) and 3.963 s (clay-agent, 104% of 3.802 s). Warm-cache frozen
  installs easily beat the ≤50% target (0.297 s / 0.338 s), but a
  GitHub-hosted runner starts cold, so CI install time should be expected at
  roughly the old npm level, not halved.
- The LSP fixture suites (`tests/fixtures/lsp/*.test.mjs`, driven by
  `tests/lsp_bridge.rs` / `tests/lsp_real_servers.rs` with `node --import …
  --test`) stay on `node:test` in this plan: bun's `node:test`
  compatibility is partial and the same reason excluded it for clay-agent
  in task 4. They are documented transitional, not migrated.
- The markdown bench keeps `node --expose-gc` because its numbers are V8
  methodology; the invocation site moved to bun only where the runtime is
  irrelevant (icon-pack generation).
- Vitest's Node fallback is documented but untested in CI, since Bun was at
  or below the Node-vitest baseline and no pool incompatibility appeared.

## Further Actions
- Plan 139 flips the daemon runtime to Bun: remove the `node` pin from
  `mise.toml`, switch `clay-agent/package.json` `engines`, update
  `build.sh`'s `pkill` daemon pattern, `src/server/agent.rs`
  `resolve_node()`, and clay-agent's `MIN_NODE` guard, and revisit the LSP
  fixture suites.
- Prerequisite still open from plan 137: push `migration/bun`, open the PR,
  and record the green CI run link in plan 137's task-3 evidence (this host
  has no `gh` CLI or token).
- `npm pack`/publish for the `@arnilo/clay` distribution wrapper stays on
  npm by design (product distribution, not dev tooling).
