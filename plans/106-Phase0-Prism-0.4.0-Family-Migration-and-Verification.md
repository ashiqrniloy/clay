# Phase 0: Prism 0.4.0 Family Migration and Verification

Source: `roadmap.md` Phase 0 (Prism 0.4.0 Family Migration and Verification) and
the Prism 0.4.0 draft resolution in that file.

This plan migrates the live `clay-agent` daemon from 22 exact `@arnilo/prism*`
0.3.0 pins to three 0.4.0 family packages plus a direct `better-sqlite3` pin.
It does **not** wire coding tools, Obscura, memory, MCP, workflows, supervisors,
or Antigravity.

Scope note: **no UI-surface tasks**. The mandatory UI skill stack applies only
if execution starts changing rendered UI — then stop, load `clay-ui` plus the
four mandatory design skills, and record it. No editor mode, language mode,
Clay JS package, package runtime capability, or extension point is added or
materially changed, so no primitive-review, package-runtime trust-domain,
`init.js` default-loading, grammar, external-process-authority, or package
UI/authoring task is required. No new user-visible editor/configuration
surface or public Clay JS API is introduced; the Clay JS API, configuration,
and manual-test tasks below are verification-only.

Confirmed architecture:

- `roadmap.md` Phase 0 and Prism 0.4 adoption map
- `decision-logs/2026-08-21-1758-native-prism-host-no-acp-cli-parity.md`
  (Clay-owned Node host; no ACP/AG-UI as first-party bus)
- `decision-logs/2026-08-21-2152-product-surfaces-are-replaceable-packages.md`
- Roadmap open decision 6: log the 0.4 pin set before implementation

Project patterns: `planning-checklist.md`, `agent-host.md`,
`authority-boundaries.md`, `extensions-and-ai.md`, `clay-js-api-naming.md`,
`clay-js-api-boundary.md`, `documentation-as-code.md`,
`doc-registry-tests.md`, `maintenance-validation.md`.

Library docs:

- Context7 has no `@arnilo/prism` (resolves syntax-highlighter / PHP / XAML
  Prism). Authoritative APIs: Prism `docs/migrate-to-0.4.md` (0.4.0),
  `packages/prism-core/package.json` exports, `packages/prism-providers`
  exports, and the family `index.ts` files under `/home/arn/Projects/prism`.
- Context7 `/wiselibs/better-sqlite3`: `npm install better-sqlite3`. Do **not**
  use the custom-amalgamation `preinstall` recipe. Pin exact `12.11.1` (already
  the 0.3 lockfile resolution; matches `@arnilo/prism-core` peer `^12.11.1`).

Not in scope: `@arnilo/prism-coding-tools`, `prism-web-tools`, `prism-memory`,
`prism-mcp`, `prism-antigravity-agent`, `prism-office`, `prism-acp-agent`,
`prism-ag-ui`, `playwright-core`, `/ai-sdk`, `/brave|/exa|/firecrawl`, D1
ask-user resume (Phase 1), Azure/Bedrock/Vertex factory wiring.

## Objectives

- Replace the 22 clay-agent 0.3.0 Prism dependencies with exact `0.4.0` pins
  for `@arnilo/prism`, `@arnilo/prism-core`, and `@arnilo/prism-providers`,
  plus direct `better-sqlite3@12.11.1`.
- Rewrite daemon imports to 0.4 family subpaths. Drop unused
  `@arnilo/prism-model-router`. Keep Azure/Bedrock/Vertex as host-config stubs
  under the new names.
- Prove chat/mock/session/credential behavior is unchanged: existing SQLite
  fixtures round-trip with no schema migration; initialize reports `0.4.0`;
  no retired 0.3 package names remain; ACP/AG-UI/MCP stay denied.
- Keep Linux cargo gates green. Document rollback as `npm ci` of the committed
  0.3 lockfile.

## Expected Outcome

- `clay-agent/package.json` depends only on `@arnilo/prism@0.4.0`,
  `@arnilo/prism-core@0.4.0`, `@arnilo/prism-providers@0.4.0`, and
  `better-sqlite3@12.11.1` from the Prism/native graph (plus existing
  `@types/node` / `typescript` devDeps).
- `clay-agent` source imports those families' subpaths only. Startup
  `initialize` result is `{ ok: true, mock, prism: "0.4.0" }`.
- `npm ci && npm run build && npm test` pass in `clay-agent`.
- `tests/agent_protocol.rs` `phase25_dependencies_deny_acp_agui_mcp` still
  forbids ACP/AG-UI/MCP/retired `prism-coding-agent` and asserts README `0.4.0`.
- `cargo fmt --check`, `cargo check --all-targets`,
  `cargo clippy --all-targets -- -D warnings`, and `cargo test --test agent_protocol`
  pass on Linux.
- Wiki `docs/wiki/modules/clay-agent.md` and its index blurb say Prism 0.4.0.

## Tasks

- [x] Record the Prism 0.4.0 clay-agent pin set in `decision-logs/`
  (DONE 2026-09-02 01:21: log written at
  `decision-logs/2026-09-02-0121-prism-0.4.0-clay-agent-family-pins.md` with
  user approval to complete this task; roadmap open decision 6 marked resolved
  with the log reference; `agent-host.md` pin line extended with the 0.4
  family/subpath rule and the new decision link. No `agent-host.md` duplicate
  pin-line cleanup — out of task scope.)
  - Acceptance Criteria:
    - Functional: A new decision log states the Phase 0 pin set (three 0.4
      families + `better-sqlite3@12.11.1`; drop `prism-model-router`; defer
      coding/web/memory/MCP/Antigravity; never adopt office/ACP/AG-UI in the
      daemon) and marks roadmap open decision 6 resolved.
    - Performance: Logging adds no runtime work.
    - Code Quality: Filename `YYYY-MM-DD-HHMM-*.md`; follows
      `create-decision-log` template; existing logs stay immutable.
    - Security: Log records no secrets and restates no-ACP/AG-UI/MCP-as-bus.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/create-decision-log/SKILL.md`
      - `.agents/skills/project-patterns/references/agent-host.md`
      - `roadmap.md` Phase 0 and open decision 6
      - `decision-logs/2026-08-21-1758-native-prism-host-no-acp-cli-parity.md`
    - Options Considered:
      - Skip the log and treat the roadmap draft as enough: faster, but
        `roadmap.md` requires a log before the phase is implemented.
      - Log after user approval, then implement: selected.
    - Chosen Approach:
      - Stop if the user has not explicitly approved logging. After approval,
        write one log and fold the durable pin/subpath rule into
        `agent-host.md` only if it is not already covered by “pin exact
        reviewed `@arnilo/prism*` versions”.
    - API Notes and Examples:
      ```text
      decision-logs/YYYY-MM-DD-HHMM-prism-0.4.0-clay-agent-family-pins.md
      ```
    - Files to Create/Edit:
      - `decision-logs/YYYY-MM-DD-HHMM-prism-0.4.0-clay-agent-family-pins.md`: new log
      - `.agents/skills/project-patterns/references/agent-host.md`: one-line
        0.4 family/subpath pin note if missing
      - `roadmap.md`: mark open decision 6 resolved after the log exists
    - References:
      - `roadmap.md` Phase 0
      - `.agents/skills/project-patterns/SKILL.md`
  - Test Cases to Write:
    - None. Decision logs are not executed.

- [x] Rewrite `clay-agent` to Prism 0.4.0 family pins and imports
  (DONE 2026-09-02 01:47: `package.json` now pins exactly `@arnilo/prism@0.4.0`,
  `@arnilo/prism-core@0.4.0`, `@arnilo/prism-providers@0.4.0`,
  `better-sqlite3@12.11.1`; 19 retired names removed, `prism-model-router`
  dropped without a subpath. `host.ts` imports
  `@arnilo/prism-core/{credentials/node,sessions/sqlite,validation/json-schema}`;
  `providers.ts` uses 13 `@arnilo/prism-providers/<adapter>` factories and
  `@arnilo/prism-providers/{azure,bedrock,vertex}` stubs; `main.ts` reports
  `prism: "0.4.0"`. README version/pins/upgrade steps updated. `npm install`
  regenerated the lock (registry 0.4.0 artifacts; `better-sqlite3` install
  script approved via `npm approve-scripts better-sqlite3` then rebuilt).
  `npm test` 8/8 pass incl. SQLite persist/resume on the existing fixture;
  manual stdio smoke: initialize → `{ ok: true, mock: false, prism: "0.4.0" }`,
  13 provider factories + 3 stubs load. `npm ls --all`: only the three families
  + `better-sqlite3`; optional core peers (pg/nats/memory) correctly unmet.
  One-line out-of-plan fix: `tests/agent_protocol.rs` README assertion
  `0.3.0` → `0.4.0` to keep `cargo test --test protocol agent_protocol` green
  (15/15 pass; `cargo fmt --check` clean). Full deny-list/isolation work stays
  in task 3.)
  - Acceptance Criteria:
    - Functional: Daemon still opens vault + SQLite, loads 13 provider
      factories + 3 host-config stubs, mock prompt/persist/resume/cancel, and
      reports `prism: "0.4.0"` on `initialize`.
    - Performance: No new process, RPC method, or event-queue change.
      `MAX_QUEUED_EVENTS` stays 256. SQLite path remains
      `--data-dir/sessions.sqlite`.
    - Code Quality: Exact `0.4.0` pins (not `^0.4.0`). No mixed 0.3/0.4
      imports. Unused `prism-model-router` gone. `tsc` clean.
    - Security: No `process.env` secrets. Vault/SQLite file modes unchanged.
      No ACP, AG-UI, MCP, coding-tools, web-tools, memory, office, or
      Antigravity dependencies. Family install does not import `/sessions/postgres`,
      `/sessions/nats`, `/ai-sdk`, or other unused subpaths.
  - Approach:
    - Documentation Reviewed:
      - Prism `docs/migrate-to-0.4.md` (package/import mapping; “not a
        database/data migration”; rollback = restore 0.3 lockfile + `npm ci`)
      - `@arnilo/prism-core@0.4.0` exports: `./credentials/node`,
        `./sessions/sqlite`, `./validation/json-schema`
      - `@arnilo/prism-providers@0.4.0` exports: 16 adapter subpaths Clay uses
        (not `./ai-sdk`)
      - `packages/prism-core/src/sessions/sqlite/index.ts`
        (`createSqlitePersistence`, `SqlitePersistence`)
      - `packages/prism-core/src/credentials/node/index.ts`
        (`openEncryptedCredentialStore`, `createKeychainCredentialStore`,
        `createStoredCredentialResolver`)
      - `packages/prism-core/src/validation/json-schema/index.ts`
        (`createJsonSchemaToolArgumentValidator`)
      - `packages/prism-providers/src/openai/index.ts`
        (`createOpenAIProviderPackage`; package name
        `@arnilo/prism-providers/openai`)
      - Context7 `/wiselibs/better-sqlite3`: `npm install better-sqlite3`
      - `clay-agent/src/host.ts`, `providers.ts`, `main.ts`, `package.json`
      - `.agents/skills/project-patterns/references/agent-host.md`
    - Options Considered:
      - Pin Phase 1 families now (`coding-tools`/`web-tools`/`memory`/`mcp`):
        rejected. `prism-web-tools` requires `prism-mcp`; that fights
        `phase25_dependencies_deny_acp_agui_mcp`. Roadmap Phase 0 is the live
        daemon graph only.
      - Keep 16 `prism-provider-*` packages beside the family: rejected.
        0.4 has no wrappers; one family + subpath imports.
      - Leave unused `prism-model-router` as `prism-core/governance/model-router`:
        rejected. It is not imported. Phase 6 can add the subpath when wired.
    - Chosen Approach:
      - Atomic `package.json` + import rewrite. Keep existing factory option
        bags; if `tsc` fails on 0.4 signatures, match the 0.4 types in this
        same task. Stub names become `@arnilo/prism-providers/{azure,bedrock,vertex}`.
    - API Notes and Examples:
      ```ts
      import {
        createKeychainCredentialStore,
        createStoredCredentialResolver,
        openEncryptedCredentialStore,
        type EncryptedCredentialStore,
        type KeychainCredentialStore,
      } from "@arnilo/prism-core/credentials/node";
      import {
        createSqlitePersistence,
        type SqlitePersistence,
      } from "@arnilo/prism-core/sessions/sqlite";
      import { createJsonSchemaToolArgumentValidator } from "@arnilo/prism-core/validation/json-schema";
      import { createOpenAIProviderPackage } from "@arnilo/prism-providers/openai";

      const persistence = createSqlitePersistence({
        filename: join(dataDir, "sessions.sqlite"),
        fileMode: 0o600,
      });
      ```

      ```json
      {
        "dependencies": {
          "@arnilo/prism": "0.4.0",
          "@arnilo/prism-core": "0.4.0",
          "@arnilo/prism-providers": "0.4.0",
          "better-sqlite3": "12.11.1"
        }
      }
      ```
    - Files to Create/Edit:
      - `clay-agent/package.json`: 0.4 pins; drop 19 retired/unused names;
        description Prism 0.4.0
      - `clay-agent/package-lock.json`: regenerate via `npm install` in
        `clay-agent/`
      - `clay-agent/src/host.ts`: core subpath imports
      - `clay-agent/src/providers.ts`: `prism-providers/<adapter>` imports and
        stub names; comment 0.4.0
      - `clay-agent/src/main.ts`: `prism: "0.4.0"`
      - `clay-agent/README.md`: version, sqlite peer, pin/upgrade steps
    - References:
      - `roadmap.md` Phase 0
      - Prism `docs/migrate-to-0.4.md`
      - `plans/096-Phase25-AI-Native-Prism-Host-and-Chat.md` (original 0.3 host)
  - Test Cases to Write:
    - Existing `clay-agent/src/__tests__/host.test.ts` persist/resume must pass
      against a 0.4 store (same fixture path, no migration API).
    - Existing unreadable-vault process-exit test must still exit 1.

- [x] Add isolation / retired-name checks and update the Phase 25 deny test
  (DONE 2026-09-02 02:20: `phase25_dependencies_deny_acp_agui_mcp` now asserts
  the four exact 0.4.0 pins, denies retired 0.3 names
  (`prism-credentials-node`, `-session-store-sqlite`, `-session-store-codecs`,
  `-tool-validator-json-schema`, `-model-router`, quoted `-provider-` prefix)
  across `clay-agent/package.json` + the five production `clay-agent/src/*.ts`
  files, and asserts family subpath usage (`credentials/node`, `sessions/sqlite`,
  `validation/json-schema`, `providers/openai`). Existing ACP/AG-UI/MCP/
  coding-agent denies and README `0.4.0` assertion retained. New Node test
  `initialize reports prism 0.4.0` spawns `main.js --mock`, sends initialize,
  asserts `{ ok, mock: true, prism: "0.4.0" }` (first cut forgot to send the
  initialize frame and hung on stdout — fixed). `npm test` 9/9;
  `cargo test --test protocol agent_protocol` 15/15; `cargo fmt --check` clean;
  manual `npm ls --all` scan: only unmet optional peer `@arnilo/prism-memory`
  (not installed, expected), zero retired names.)
  - Acceptance Criteria:
    - Functional: `npm ls --all` in `clay-agent` contains only the three
      selected Prism families plus `better-sqlite3` from that graph, and none
      of the retired 0.3 names. Source has no `@arnilo/prism-*` imports except
      `@arnilo/prism`, `@arnilo/prism-core/...`, `@arnilo/prism-providers/...`.
    - Performance: Checks are `npm ls` / string scans, not extra daemon
      processes.
    - Code Quality: `phase25_dependencies_deny_acp_agui_mcp` still denies
      `prism-acp`, `prism-ag-ui`, `agentclientprotocol`,
      `@modelcontextprotocol`, `prism-coding-agent`,
      `@arnilo/prism-coding-agent`. README assertion is `0.4.0`.
    - Security: Installing the three families does not load Postgres/NATS
      drivers, browsers, or unused adapters. MCP remains denied until Phase 1.
  - Approach:
    - Documentation Reviewed:
      - Prism `docs/migrate-to-0.4.md` isolation rule: family install is inert
      - `tests/agent_protocol.rs` `phase25_dependencies_deny_acp_agui_mcp`
      - `clay-agent/README.md` pin checklist
    - Options Considered:
      - New `prism-0.4.test.ts` covering every later-phase seam: rejected
        (Phase 1+).
      - Extend deny test + one small Node assertion on initialize version:
        selected.
    - Chosen Approach:
      - Update the existing Rust deny test and README. Add one Node test that
        `initialize` returns `prism: "0.4.0"` by spawning `main.js` like the
        unreadable-vault test. Optional: `host.test.ts` asserts
        `createSqlitePersistence` reopen of the first-run `sessions.sqlite`.
        Do not import unused core subpaths “to prove they exist”.
    - API Notes and Examples:
      ```js
      const child = spawn(process.execPath, [main, "--data-dir", dataDir, "--mock"], {
        stdio: ["pipe", "pipe", "pipe"],
      });
      child.stdin.write(
        `${JSON.stringify({ jsonrpc: "2.0", id: 1, method: "initialize", params: { passphrase: "pass-phrase-ok" } })}\n`,
      );
      // stdout line includes result.prism === "0.4.0"
      ```

      ```bash
      cd clay-agent && npm ls --all
      # fail if output contains prism-credentials-node, prism-session-store-sqlite,
      # prism-tool-validator-json-schema, prism-model-router, prism-provider-,
      # prism-coding-agent, prism-mcp, prism-web-tools, prism-memory, prism-acp, prism-ag-ui
      ```
    - Files to Create/Edit:
      - `tests/agent_protocol.rs`: README `0.4.0`; keep ACP/AG-UI/MCP/coding-agent denies
      - `clay-agent/src/__tests__/host.test.ts`: initialize reports `0.4.0`;
        persist/resume already covers SQLite round-trip
      - `clay-agent/README.md`: already edited in previous task; keep deny list
    - References:
      - `roadmap.md` Phase 0 exit gate
      - `.agents/skills/project-patterns/references/maintenance-validation.md`
  - Test Cases to Write:
    - `initialize reports prism 0.4.0` in `host.test.ts` (or adjacent Node test).
    - `phase25_dependencies_deny_acp_agui_mcp` still fails closed on MCP/ACP/AG-UI
      and retired coding-agent names.
    - Manual `npm ls --all` scan recorded in task evidence (no extra test
      framework).

- [x] Verify Linux gates, daemon tests, and 0.3 rollback drill
  (DONE 2026-09-02 02:45: 0.4 state — `cargo fmt --check`,
  `cargo check --all-targets`, `cargo clippy --all-targets -- -D warnings`
  clean; `cargo test --test protocol agent_protocol` 15/15;
  `clay-agent` `npm ci` + `npm rebuild better-sqlite3` + `npm test` 9/9;
  lockfile resolves `@arnilo/prism 0.4.0` + `better-sqlite3 12.11.1`.
  Rollback drill (changes uncommitted, so a partial `git stash push --
  clay-agent tests/agent_protocol.rs` replaced the planned `git checkout`):
  0.3 state → `npm ci` → `npm test` 8/8 green; `git stash pop` restored 0.4
  with no conflicts; reinstall + 9/9 again. Vault/SQLite rollback = no data
  migration needed (persist/resume test reopens 0.4 store on both passes).
  No compatibility shim, no second committed lockfile.)
  - Acceptance Criteria:
    - Functional: `cd clay-agent && npm ci && npm run build && npm test` pass.
      `cargo test --test agent_protocol` pass.
    - Performance: Linux blocking gates pass:
      `cargo fmt --check`, `cargo check --all-targets`,
      `cargo clippy --all-targets -- -D warnings`. No new latency budget.
    - Code Quality: Chat/mock flows stay byte-compatible aside from the
      version string.
    - Security: Rollback restores 0.3 pins with `git checkout` of
      `clay-agent/package.json` + lockfile then `npm ci`; no database
      rollback tool; vault/SQLite files from 0.4 reopen after re-upgrade
      without a migration command.
  - Approach:
    - Documentation Reviewed:
      - `AGENTS.md` Linux gates
      - Prism `docs/migrate-to-0.4.md` Rollback section
      - `clay-agent/README.md` Upgrade Prism
    - Options Considered:
      - Compatibility shim packages: rejected (Prism ships none).
      - Keep 0.3 lockfile as a second committed file: rejected. Git history
        is the rollback.
    - Chosen Approach:
      - Run the listed commands on Linux. Record rollback as a documented
        drill (`git stash` or checkout those two files, `npm ci`, `npm test`,
        then restore 0.4). Do not unpublish anything.
    - API Notes and Examples:
      ```bash
      cd clay-agent && npm ci && npm run build && npm test
      cargo fmt --check
      cargo check --all-targets
      cargo clippy --all-targets -- -D warnings
      cargo test --test agent_protocol
      ```
    - Files to Create/Edit:
      - None unless a gate failure forces a fix in files from prior tasks.
    - References:
      - `roadmap.md` Phase 0 exit gate
      - `.agents/skills/project-patterns/references/planning-checklist.md`
  - Test Cases to Write:
    - None new. This task runs the suites already listed.

- [x] Create or verify Clay JS APIs for public programmatic surfaces
  (DONE 2026-09-02 02:52: verified no-op. Full diff inventory: 10 modified
  files + 2 untracked docs — all in `clay-agent/` (Node child, not a Clay JS
  package), `tests/agent_protocol.rs` (test-only), roadmap, agent-host
  pattern, plan, decision log. Zero changes under `src/`, `frontend/`,
  `packages/`, or `runtime/js`; diff contains no `deno_core`, `op_*`, or new
  `pub fn`. Daemon `initialize.prism: "0.4.0"` is internal clay-agent
  JSON-RPC, not a Clay JS facade. `cargo test --test protocol
  clay_js_doc_registry` 50/50 green — registry not stale, `update-doc-registry`
  not needed. No files changed in this task.)
  - Acceptance Criteria:
    - Functional: No new Clay JS API, `deno_core` op, or Rust `pub` function
      is required. Daemon JSON-RPC `initialize.prism` is not a Clay JS API.
    - Performance: No new JS/Rust surface on the editor hot path.
    - Code Quality: No `clay.<domain>.*` IDs introduced. No `examples/init.js`
      change (no configuration surface).
    - Security: No new filesystem/network/shell/AI authority.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/create-plan/references/clay.md` Clay JS API +
        Configuration + Example Configuration tasks
      - `.agents/skills/project-patterns/references/clay-js-api-naming.md`
      - `.agents/skills/project-patterns/references/clay-js-api-boundary.md`
      - `.agents/skills/project-patterns/references/documentation-as-code.md`
      - `.agents/skills/project-patterns/references/doc-registry-tests.md`
    - Options Considered:
      - Expose Prism version through a new `agent.*` Clay JS API: rejected.
        Host already returns it on daemon `initialize`; React/server do not
        need a new facade for a pin bump.
      - Verify no public API change: selected.
    - Chosen Approach:
      - Inventory this plan’s Rust/JS edits. Confirm only `clay-agent` Node
        sources, README, deny-test strings, and wiki. Do not run
        `update-doc-registry` unless an accidental public-API edit appears.
    - API Notes and Examples:
      ```ts
      // Not added. Existing daemon handshake only:
      // initialize -> { ok: true, mock: boolean, prism: "0.4.0" }
      ```
    - Files to Create/Edit:
      - None expected.
    - References:
      - `decision-logs/2026-05-08-1509-clay-js-api-facade-for-rust-functions.md`
  - Test Cases to Write:
    - None. `cargo test` doc-registry suite remains green without edits.

- [x] Execute and update the manual test plan (`test-plan/`)
  (DONE 2026-09-02 02:56: automated-only confirmed. `test-plan/index.md`
  contains no agent/prism/daemon manual steps (rg scan: zero matches); no
  editor/UI/config/keybinding/package behavior changed in this phase, so there
  are no manual steps to add or execute. `test-plan/` left byte-identical
  (`git diff --stat test-plan/` empty). Existing Chat steps under plans 09/10/11
  remain untouched and were not exercised by Phase 0. Daemon behavior is
  covered by the automated suites recorded in task 4. No files changed in
  this task.)
  - Acceptance Criteria:
    - Functional: Record that this phase is automated-only: no new
      editor/UI/config/keybinding/package behavior. Existing Chat steps in
      `test-plan/` are unchanged.
    - Performance: No new manual latency step.
    - Code Quality: Do not weaken or delete existing steps.
    - Security: No new trust-boundary step.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/create-plan/references/clay.md` Manual Test Plan Task
      - `test-plan/index.md` (no clay-agent module; Chat noted under 09/10/11)
    - Options Considered:
      - Add `test-plan/12-clay-agent.md` for the version string: rejected.
        Covered by Node + `agent_protocol` tests.
      - Explicit automated-only record: selected.
    - Chosen Approach:
      - Leave `test-plan/` files unchanged. Write the reason in this task’s
        completion evidence when executing.
    - API Notes and Examples:
      ```text
      test-plan/index.md  # no module add
      ```
    - Files to Create/Edit:
      - None expected.
    - References:
      - `test-plan/11-performance.md` existing clay-agent structural note
  - Test Cases to Write:
    - None.

- [x] Update or verify the code wiki after implementation
  (DONE 2026-09-02 03:05: `docs/wiki/modules/clay-agent.md` — overview says
  Prism 0.4.0; responsibilities now record the three exact family pins +
  `better-sqlite3@12.11.1`, subpath-only imports, unmet optional peers, the
  dropped `prism-model-router`, and the deny test enforcing retired names;
  invariants list the full Phase 0 deny set incl. coding-tools/web-tools/
  memory/Antigravity. `docs/wiki/index.md` blurb rewritten to Prism 0.4.0.
  `phase25-agent-host-primitive-review.md` left as-is — its "Prism 0.3.0"
  heading is the historical review basis, not a live-pin claim. Verified:
  zero `0.3.0` mentions remain in the two live pages; page structure and
  master-index link unchanged.)
  - Acceptance Criteria:
    - Functional: `docs/wiki/modules/clay-agent.md` and the index blurb say
      Prism 0.4.0 and the three family packages / subpath imports.
    - Performance: Wiki adds no runtime work. Note SQLite schema is unchanged.
    - Code Quality: Page still explains spawn, vault, providers, tests, and
      invariants. Master index links it.
    - Security: Still documents no ACP/AG-UI/MCP, no `process.env` secrets,
      package JS cannot spawn the daemon.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/project-wiki/SKILL.md`
      - `.agents/skills/project-wiki/references/page-template.md`
      - `.agents/skills/create-plan/references/wiki-task.md`
      - `docs/wiki/modules/clay-agent.md`
      - `docs/wiki/index.md`
    - Options Considered:
      - Update after each task: noisy.
      - Update once after tests pass: selected.
    - Chosen Approach:
      - After verification, update `clay-agent.md` (version, import table,
        `better-sqlite3` direct pin, dropped model-router). Update index
        one-liner. Historical Plan 096 / phase25 primitive-review pages stay
        0.3.0-era unless a sentence claims the *current* pin.
    - API Notes and Examples:
      ```text
      docs/wiki/index.md
      docs/wiki/modules/clay-agent.md
      ```
    - Files to Create/Edit:
      - `docs/wiki/modules/clay-agent.md`: current 0.4 host facts
      - `docs/wiki/index.md`: blurb Prism 0.4.0
      - `docs/wiki/modules/phase25-agent-host-primitive-review.md`: only if it
        claims the live pin is still 0.3.0
    - References:
      - `.agents/skills/project-wiki/SKILL.md`
  - Test Cases to Write:
    - Manual wiki review: index link + page matches `clay-agent/package.json`
      and `host.ts` imports.

## Compromises Made

- Known before execution: Phase 0 does not pin Phase 1 families. That keeps
  the MCP deny test honest and matches the live 22-dep graph.
- Context7 cannot document `@arnilo/prism`; local Prism 0.4 docs are used.

## Further Actions

- All seven tasks complete (2026-09-02). Expected follow-up is Phase 1
  (`prism-coding-tools`, `prism-web-tools`, `prism-memory`, `prism-mcp`) in a
  later numbered plan, including D1 smoke, narrowing the MCP deny test, and
  adding exact pins for those families.
