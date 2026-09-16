# Adopt Prism 0.7.0 Pins, Node 22, and Event-Mapper Drop

Depends on: none (current live pin is exact Prism **0.5.5**). Unlocks plans
121–123.

This plan is **Cut 1** of the Prism 0.7.0 clay-agent adoption: lockstep pins,
Node `>=22`, `playwright-core@1.63.0`, version-literal tests, and the
`map_event` unknown-type footgun. It does **not** turn on 0.7 opt-ins
(attention, spawn, work-scopes, wiki ingest). Those are 121–123.

Scope note: **no UI-surface tasks**. No editor mode, language mode, Clay JS
package, package runtime, extension point, or `init.js` configuration surface
is added. Skip primitive-review, package-runtime trust-domain, package
default-loading, grammar, external-process-authority, package UI/authoring,
example-config maintenance, and live launch-test. Clay JS API and
configuration tasks below are verification-only. No HTML prototype.

Context7 has no `@arnilo/prism` (it resolves `/stoplightio/prism`).
Authoritative APIs are the local Prism **0.7.0** tree.

Confirmed architecture:

- `decision-logs/2026-09-07-2149-prism-0.5.1-clay-agent-family-pins.md`
  (exact-pin rule; later 0.5.x increments followed the same lockstep)
- `.agents/skills/clay-execution/references/packages.md` (Agent Host)
- `.agents/skills/clay-execution/references/planning-checklist.md`
- `.agents/skills/clay-execution/references/docs-as-code.md`
- `.agents/skills/clay-execution/references/js-api.md`
- `.agents/skills/clay-execution/references/protocol-perf.md`
- `roadmap.md` Prism 0.5.x lockstep section (live pins still name 0.5.5)
- `clay-agent/package.json` exact `0.5.5` seven-family pins +
  `playwright-core@1.61.0` + `better-sqlite3@13.0.3`

Library docs:

- `/home/arn/Projects/prism/docs/migrate-to-0.6.md` §1 (Node `>=22`), §2
  (`playwright-core` `1.63.0`), Upgrade steps
- `/home/arn/Projects/prism/docs/migrate-to-0.7.md` Upgrade steps 1, 5–6
  (lockstep `0.7.0`, no persisted-data migration; ACP/router refusals do not
  apply to clay-agent)
- `/home/arn/Projects/prism/CHANGELOG.md` `[0.6.0]` `[0.7.0]`
- `/home/arn/Projects/prism/docs/agent-events.md`

Clay never shipped 0.5.6 or 0.6.0. This pin jump absorbs both. Rollback for
Clay = exact **0.5.5** pins + `npm ci` in `clay-agent/` (not Prism’s
documented 0.6.0 rollback, which Clay never ran).

Not in scope: attention compiler, `RunOptions.toolNames`, wiki ingest, graft
init/build-deep, spawn/supervisor/worktrees, work-scopes, memory fabric,
ACP, AG-UI-in-daemon, office/OCR, E2B, realtime voice, Bedrock Converse,
model-router, `activateKernel`, Azure/Bedrock/Vertex factory wiring.

## Objectives

- Pin the seven adopted `@arnilo/prism*` families to exact `0.7.0`. Keep
  `better-sqlite3@13.0.3`. Move `playwright-core` `1.61.0` → exact `1.63.0`.
- Raise the clay-agent runtime floor from Node `>=20` to Node `>=22`
  (`MIN_NODE`, `engines.node`, README, wiki, Agent Host pattern).
- Stop mapping unknown Prism `AgentEvent` types to `AgentWireEvent::Started`.
  Drop them (`None`). Fixes the pre-existing fake-Started path that
  `agent_suspended` already hit, and keeps 0.7 `attention_compiled` /
  `subagent_*` / `delegation_*` from reopening runs.
- Prove `clay-agent` tests plus Linux cargo gates still pass. No persisted
  schema migration.

## Expected Outcome

- `clay-agent/package.json` pins seven families at exact `"0.7.0"` and
  `playwright-core` at exact `"1.63.0"`. `initialize` reports
  `{ prism: "0.7.0" }`.
- `MIN_NODE === 22`. Node 20 is refused at process start with a clear
  message. CI stays Node 24.
- `map_event` returns `None` for unknown `event.type` values, including
  `attention_compiled`, `subagent_started`, `subagent_stopped`, and
  `provider_turn_finished` without context tokens. Known arms unchanged.
- `tests/agent_protocol.rs` pin test asserts exact `0.7.0` and
  `playwright-core@1.63.0`.
- Linux: `cargo fmt --check`, `cargo check --all-targets`,
  `cargo clippy --all-targets -- -D warnings`,
  `cargo test --test protocol agent_protocol` (Cargo.toml folds
  `tests/agent_protocol.rs` into the `protocol` suite target).
- `cd clay-agent && npm ci && npm test` pass.
- Wiki + `packages.md` Agent Host + `roadmap.md` say live pins are Prism
  0.7.0 and Node `>=22`.

## Tasks

- [x] Record the Prism 0.7.0 clay-agent pin set in `decision-logs/`
  - Acceptance Criteria:
    - Functional: A new decision log states the 0.7.0 pin set (seven
      families at exact `0.7.0` + `better-sqlite3@13.0.3` +
      `playwright-core@1.63.0`), Node floor 22, unknown-event drop, the
      four-cut adoption sequence (this plan, then 121–123), and restates
      office/ACP/AG-UI-as-bus stay out of the daemon.
    - Performance: Logging adds no runtime work.
    - Code Quality: Filename `YYYY-MM-DD-HHMM-*.md`; follows
      `create-decision-log` template; existing logs stay immutable.
    - Security: Log records no secrets; restates no-ACP/AG-UI-as-bus and
      that session/cache keys are correlation ids, never secrets.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/create-decision-log/SKILL.md`
      - `.agents/skills/clay-execution/references/packages.md` (Agent Host)
      - `decision-logs/2026-09-07-2149-prism-0.5.1-clay-agent-family-pins.md`
      - `/home/arn/Projects/prism/docs/migrate-to-0.6.md` §1–§2
      - `/home/arn/Projects/prism/docs/migrate-to-0.7.md` Upgrade steps
    - Options Considered:
      - Skip the log and treat migrate-to-0.7 as enough: faster, but every
        prior Prism pin cut logged first.
      - One log covering pins + Cuts 2–4 sequence, written only after
        explicit user approval: selected.
    - Chosen Approach:
      - Stop if the user has not explicitly approved logging. After
        approval, write one log. Fold durable “live pins 0.7.0 / Node 22 /
        unknown events drop” into `packages.md` Agent Host. Point roadmap
        live pins at 0.7.0.
    - API Notes and Examples:
      ```text
      decision-logs/YYYY-MM-DD-HHMM-prism-0.7.0-clay-agent-family-pins.md
      ```
    - Files to Create/Edit:
      - `decision-logs/YYYY-MM-DD-HHMM-prism-0.7.0-clay-agent-family-pins.md`: new log
      - `.agents/skills/clay-execution/references/packages.md`: live pins
        `0.5.5` (text currently still says 0.5.2 in one stanza) → `0.7.0`;
        Node ≥ 20 → ≥ 22
      - `roadmap.md`: live-pin / 0.5.5 increment section → 0.7.0 cut
    - References:
      - `.agents/skills/create-decision-log/SKILL.md`
      - `plans/113-Adopt-Prism-0.5.1.md` (prior pin-cut shape)
  - Test Cases to Write:
    - None. Decision logs are not executed.
  - Completion (2026-09-16):
    - Log written: `decision-logs/2026-09-16-0026-prism-0.7.0-clay-agent-family-pins.md`
      (status approved; records exact `0.7.0` seven-family pins +
      `better-sqlite3@13.0.3` + `playwright-core@1.63.0`, Node ≥ 22
      floor, unknown-`AgentEvent` drop instead of fake `Started`, the
      four-cut sequence 120–123, office/ACP/AG-UI/E2B/voice/Bedrock/
      router exclusions, and rollback to exact `0.5.5`).
    - `.agents/skills/clay-execution/references/packages.md` (Agent Host):
      live pins → Prism `0.7.0` (+ `playwright-core@1.63.0`); Node ≥ 20 →
      ≥ 22; sources list adds the 0.7.0 log; new bullet: `map_event` maps
      named types only and must never resurrect the fake-`Started`
      catch-all.
    - `roadmap.md`: governing-decisions pin history extended with the
      0.5.5 → 0.7.0 jump; Product Shape daemon line now Node ≥ 22 /
      Prism 0.7.0; capability-review heading + live-pin paragraph updated;
      new “Prism 0.7.0 lockstep (live pins)” section; 0.5.4/0.5.5
      headings marked superseded; pin-summary list gains the 0.7.0 entry.

- [x] Bump clay-agent family pins to exact 0.7.0 and Node 22
  - Acceptance Criteria:
    - Functional: All seven `@arnilo/prism*` dependencies are exact
      `"0.7.0"`. `playwright-core` is exact `"1.63.0"`. `better-sqlite3`
      stays `"13.0.3"`. `initialize` reports `prism: "0.7.0"`.
      `engines.node` is `>=22`. `MIN_NODE` is `22`. Lockfile resolves
      registry 0.7.0 tarballs. No `0.5.5` pin remains in
      `clay-agent/package.json`.
    - Performance: No new process, RPC method, or event-queue change.
    - Code Quality: Lockstep only — do not mix 0.5.5 and 0.7.0. Subpath
      imports unchanged. `@types/node` stays `^24` (already above floor).
    - Security: No ACP/AG-UI/office/antigravity/SDK-module pins.
      `npm ls --all` still shows MCP SDK only under `@arnilo/prism-mcp`.
  - Approach:
    - Documentation Reviewed:
      - `/home/arn/Projects/prism/docs/migrate-to-0.6.md` Upgrade steps 1–3
      - `/home/arn/Projects/prism/docs/migrate-to-0.7.md` Upgrade step 1
        (Clay uses exact pins, not `^0.7.0`)
      - `clay-agent/package.json`, `clay-agent/README.md`, `clay-agent/src/main.ts`
      - `tests/agent_protocol.rs` `phase25_dependencies_deny_acp_agui_mcp`
      - `.agents/skills/clay-execution/references/packages.md` (Agent Host pins)
    - Options Considered:
      - Range pin `^0.7.0`: rejected. Pattern is exact reviewed pins.
      - Partial bump (`@arnilo/prism` only): rejected. 0.7.0 is lockstep.
      - Stay on Node 20 and ignore `engines`: rejected. 0.6+ is unsupported
        on 20; CI already runs Node 24.
    - Chosen Approach:
      - Exact seven-family `0.7.0` + `playwright-core@1.63.0` + `MIN_NODE=22`.
      - Confirm `npm view @arnilo/prism version` is `0.7.0` before `npm ci`.
    - API Notes and Examples:
      ```json
      {
        "engines": { "node": ">=22" },
        "dependencies": {
          "@arnilo/prism": "0.7.0",
          "@arnilo/prism-coding-tools": "0.7.0",
          "@arnilo/prism-core": "0.7.0",
          "@arnilo/prism-mcp": "0.7.0",
          "@arnilo/prism-memory": "0.7.0",
          "@arnilo/prism-providers": "0.7.0",
          "@arnilo/prism-web-tools": "0.7.0",
          "better-sqlite3": "13.0.3",
          "playwright-core": "1.63.0"
        }
      }
      ```
      ```ts
      const MIN_NODE = 22;
      // initialize result:
      { ok: true, mock, prism: "0.7.0", mcpServers }
      ```
    - Files to Create/Edit:
      - `clay-agent/package.json`: pins, engines, description
      - `clay-agent/package-lock.json`: regenerate via `npm ci` in `clay-agent/`
      - `clay-agent/src/main.ts`: `MIN_NODE`, `prism: "0.7.0"`
      - `clay-agent/src/__tests__/host.test.ts`: initialize assertion `0.7.0`
      - `clay-agent/README.md`: Node >= 22, Pins section 0.7.0 / playwright 1.63.0
      - `tests/agent_protocol.rs`: README + seven-family + playwright pin literals
    - References:
      - `plans/113-Adopt-Prism-0.5.1.md` bump task
      - `.github/workflows/ci.yml` (`node-version: 24` — unchanged)
  - Test Cases to Write:
    - `initialize reports prism 0.7.0` (replace 0.5.5 assertion).
    - `phase25_dependencies_deny_acp_agui_mcp`: exact `"0.7.0"` pins,
      `"playwright-core": "1.63.0"`, README contains `0.7.0`, no `0.5.5`
      family pins remain in `clay-agent/package.json`.
  - Completion (2026-09-16):
    - Pins: seven families exact `0.7.0`, `playwright-core` `1.63.0`,
      `better-sqlite3` stays `13.0.3`, `engines.node` `>=22` (description
      literal updated). Registry check before install: all seven families
      publish `0.7.0`; `@arnilo/prism-memory@0.7.0` peer moves to
      `@nanonets/graft ^0.16.0 || ^0.18.0` (not a direct dep here).
    - Floor: `MIN_NODE = 22` and `initialize` literal `prism: "0.7.0"` in
      `clay-agent/src/main.ts`; `host.test.ts` initialize test renamed and
      asserts `0.7.0`; README heading/spawn/Pins/Upgrade-Prism sections
      moved to Node >= 22 / Prism 0.7.0 / `playwright-core@1.63.0`.
    - Lockfile: regenerated with `npm install` (a `npm ci` cannot rewrite a
      stale lock); `npm ci` then reinstalled clean from it and `npm ls --all`
      shows MCP only transitively under `@arnilo/prism-mcp`.
    - Gate literals: `tests/agent_protocol.rs`
      `phase25_dependencies_deny_acp_agui_mcp` now asserts the seven
      `"<family>": "0.7.0"` pins, `"playwright-core": "1.63.0"`, README
      `0.7.0` + `Node >= 22`. The `agent.node_missing` diagnostic text
      (`src/server/agent.rs`, test fixture) moved `>= 20` → `>= 22` to
      match the floor it names.
    - 0.7 behavior surfaced by the bump (not in the planned file list):
      Prism 0.7 resolves `wikiRoot` against `workspaceRoot` and
      `/wiki-init` reports the absolute path, so two
      `wiki-knowledge.test.ts` assertions moved to `join(root, ".wiki")`
      and the stale `enableWiki` comment claiming an absolute `wikiRoot`
      breaks tool resolution was corrected (0.7 tools also
      `resolve(workspaceRoot, wikiRoot)`).
    - Evidence: `cd clay-agent && npm test` → 150 tests, 149 pass, 0 fail;
      `cargo fmt --check`, `cargo check --all-targets`,
      `cargo clippy --all-targets -- -D warnings`,
      `cargo test --test protocol agent_protocol` (22 passed) all clean.
      No persisted-schema migration ran; Linux host, Windows not a pass
      condition.

- [x] Drop unknown Prism events in `map_event` instead of faking Started
  - Acceptance Criteria:
    - Functional: Unknown `event.type` returns `None` (daemon event is not
      forwarded). `attention_compiled`, `subagent_started`,
      `subagent_stopped`, and `provider_turn_finished` without context
      tokens do not produce `AgentWireEvent::Started`. Existing named arms
      (`agent_started`, `agent_finished`, `agent_suspended`, tools,
      permissions, overflow, errors, thinking/message deltas,
      `provider_turn_finished` *with* tokens) keep their mapping.
    - Performance: Mapper stays O(1) per event; dropping is cheaper than
      emitting a fake Started into the book.
    - Code Quality: No new `AgentWireEvent` variant in this plan. Plans
      121–122 may add arms later; until then drop is the contract.
    - Security: Still redacts secrets on mapped arms. Dropped events never
      leak raw payloads to AG-UI.
  - Approach:
    - Documentation Reviewed:
      - `src/server/agent/run.rs` `map_event` (catch-all at unknown → Started)
      - `src/server/agent.rs` unreported-usage test (expects fake Started)
      - `/home/arn/Projects/prism/docs/agent-events.md`
      - `/home/arn/Projects/prism/docs/migrate-to-0.7.md` §18 / §21
        (`attention_compiled`, `subagent_started` / `subagent_stopped`)
      - `.agents/skills/clay-execution/references/protocol-perf.md`
        (agent mapping is Background vs editor hot path; typing unaffected)
    - Options Considered:
      - Keep catch-all Started: smallest diff, but already caused
        forever-streaming on `agent_suspended` and will reopen runs on 0.7
        lifecycle events.
      - New `AgentWireEvent::Ignored` forwarded to the client: extra wire
        noise for no UI.
      - Return `None` for the default arm: selected.
    - Chosen Approach:
      ```rust
      _ => return None,
      ```
      Update the unreported-usage unit test: `provider_turn_finished`
      without tokens → `None`, not Started.
    - API Notes and Examples:
      ```rust
      let mapped = match event_type {
          "agent_started" => AgentWireEvent::Started { session_id: session_id.clone(), run_id },
          // …existing arms…
          "error" => AgentWireEvent::Error { session_id: session_id.clone(), message: redact_text(detail, secrets) },
          _ => return None,
      };
      Some(AgentServerMessage::Event { session_id, event: mapped })
      ```
    - Files to Create/Edit:
      - `src/server/agent/run.rs`: default arm
      - `src/server/agent.rs`: unreported-usage test + new unknown-type test
    - References:
      - `src/server/agent/run.rs` `agent_suspended` comment (same class of bug)
  - Test Cases to Write:
    - `map_event_drops_unknown_event_types`: `attention_compiled`,
      `subagent_started`, `not_a_real_event` → `None`.
    - `map_event` unreported `provider_turn_finished` → `None`.
    - Existing tool/digest/suspension tests still pass.
  - Completion (2026-09-16):
    - `src/server/agent/run.rs`: default arm is now `_ => return None,`
      with a comment naming the `agent_suspended` incident, the 0.7
      telemetry it would reopen (`attention_compiled`, `subagent_*`,
      `delegation_*`), and the rule that a lifecycle type Clay renders gets
      an explicit arm (subagent lifecycle → existing `Tool` events).
    - `src/server/agent.rs`:
      `provider_turn_finished_books_context_occupancy` now asserts
      `mapped.is_none()` for unreported usage; new
      `map_event_drops_unknown_event_types` covers `attention_compiled`,
      `subagent_started`, `subagent_stopped`, `delegation_started`,
      `delegation_finished`, and `not_a_real_event` → `None`.
    - Drop path confirmed: `route_daemon_line` maps `map_event(..) == None`
      to `DaemonLine::Ignore`, so a dropped event is never forwarded and
      never reaches `apply_book_event` (unknown events can no longer
      un-cancel a session via a fake `Started`).
    - Evidence: `cargo fmt --check`; `cargo clippy --all-targets -- -D
      warnings`; `cargo test --lib agent` → 128 passed;
      `cargo test --test protocol` → 216 passed; targeted
      `cargo test --lib map_event` / `provider_turn_finished` pass.

- [x] Verify clay-agent suite and Linux cargo gates on 0.7.0
  - Acceptance Criteria:
    - Functional: `cd clay-agent && npm ci && npm run build && npm test`
      pass. `cargo test --test protocol agent_protocol` pass. No sqlite
      schema migration ran or was needed.
    - Performance: Daemon start and `initialize` stay within existing
      host-owned boot floor (plan 119 C41). No new RPC round-trips.
    - Code Quality: `cargo fmt --check`, `cargo check --all-targets`,
      `cargo clippy --all-targets -- -D warnings` pass on Linux.
    - Security: `npm ls --all` in `clay-agent/` still has no direct
      `@modelcontextprotocol/*`, `prism-acp`, `prism-ag-ui`, or office pins.
  - Approach:
    - Documentation Reviewed:
      - `/home/arn/Projects/prism/docs/migrate-to-0.7.md` Upgrade step 5
      - `clay-agent/package.json` scripts
      - AGENTS.md platform-validation (Linux cargo gates blocking)
    - Options Considered:
      - Skip Linux cargo because this is a Node pin: rejected. Pin literals
        live in `tests/agent_protocol.rs`.
      - Run the listed gates after pin + mapper land: selected.
    - Chosen Approach:
      - `npm ci` inside `clay-agent/` (not repo root). Then the Linux
        cargo set. Windows cross is not a pass condition.
    - API Notes and Examples:
      ```bash
      cd clay-agent && npm ci && npm run build && npm test
      cargo fmt --check
      cargo check --all-targets
      cargo clippy --all-targets -- -D warnings
      cargo test --test protocol agent_protocol
      ```
    - Files to Create/Edit:
      - None unless a 0.7 typecheck forces a one-line import tweak
        (record the exact symbol if so; do not “clean up” host.ts).
    - References:
      - `plans/113-Adopt-Prism-0.5.1.md` verification task
  - Test Cases to Write:
    - None new. Existing suites are the gate.
  - Completion (2026-09-16):
    - clay-agent: `npm ci` (49 packages, clean from the regenerated
      lock) → `npm run build` (`tsc -p tsconfig.json`) → `npm test` =
      **150 tests, 149 pass, 1 skipped, 0 fail** (the skip is the
      pre-existing non-Linux path). Plan 119 C41 slow-boot MCP cases still
      pass (`mcp-v2.test.ts` connect-handshake, bounded-call, and
      cancellation cases).
    - Dependency authority unchanged: `npm explain
      @modelcontextprotocol/{client,server,core}` resolves through a single
      path, `@arnilo/prism-mcp@0.7.0` (no direct pin), and `npm ls --all`
      has zero `prism-acp` / `prism-ag-ui` / `prism-office` entries.
    - Linux gates: `cargo fmt --check` clean; `cargo check --all-targets`
      clean; `cargo clippy --all-targets -- -D warnings` clean;
      `cargo test --test protocol agent_protocol` → 22 passed;
      `cargo test --test protocol` → 216 passed;
      `cargo test --lib` → 1368 passed, 1 ignored.
    - No sqlite schema migration ran or was needed: the pin bump touched no
      store, and Prism 0.7's only new versioned record
      (`LINEAGE_SCHEMA_VERSION = 1`, `dist/lineage.js`) belongs to the
      opt-in lineage feature Clay does not call.
    - Boot floor: no new RPC round-trips — `git diff` over `src/server/` is
      2 files / 50 lines, all inside the event mapper and its unit tests
      (`agent.rs` diff is test-module only); no protocol or handshake
      change, so daemon start and `initialize` keep the plan 119 sequence.
    - Gate-command erratum: `cargo test --test agent_protocol` does not
      exist (Cargo.toml folds four suite targets; Cargo rejects the
      per-file name). Correct form: `cargo test --test protocol
      agent_protocol`, matching plan 107's existing note; this plan's
      references were corrected above.
    - Windows not exercised (Linux is the blocking host per AGENTS.md);
      nothing Windows-only was touched.

- [x] Create or verify Clay JS APIs for public programmatic surfaces
  - Acceptance Criteria:
    - Functional: No new Clay JS API, op, or `clay:` export is required.
      `map_event` stays `pub(super)`. `MIN_NODE` stays private in
      `clay-agent/src/main.ts`. Inventory of touched Rust publics confirms
      they remain crate-private.
    - Performance: Verification adds no runtime work.
    - Code Quality: Follows `js-api.md` dotted-ID rules; no accidental
      `clay.<domain>.*` IDs.
    - Security: No new filesystem, network, shell, or AI-mutation API.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-execution/references/js-api.md`
      - `.agents/skills/create-plan/references/clay.md` (Clay JS API Task)
    - Options Considered:
      - Expose prism version as a Clay JS API: rejected. `initialize` already
        reports it on the daemon wire; packages cannot speak to the daemon.
      - Verification-only task: selected.
    - Chosen Approach:
      - Walk the diff. If a Rust `pub` slipped out, make it `pub(crate)` or
        document the JS API. Expected result: no new APIs.
    - API Notes and Examples:
      ```ts
      // none — clay-agent is not a Clay JS package
      ```
    - Files to Create/Edit:
      - None expected.
    - References:
      - `docs/index.md` (do not add a pin-version page)
  - Test Cases to Write:
    - Existing `cargo test` doc-registry tests still pass.
  - Completion (2026-09-16):
    - No new Clay JS API, op, or `clay:` export: `git diff` over `src/`,
      `examples/`, and `frontend/src/` contains no added `clay:` binding,
      and `docs/index.md` gained no pin-version page (no new docs files in
      this plan's diff).
    - Rust visibility unchanged: `map_event` is still
      `pub(super) fn map_event(...)` (`src/server/agent/run.rs:40`); the
      diff's only `pub` lines are inside existing test/signature contexts —
      an added-`pub`/`export` scan over the whole diff returns nothing.
      `src/server/agent.rs`'s change is confined to the test module.
    - clay-agent is not a Clay JS package: `MIN_NODE = 22` stays a private
      const in `clay-agent/src/main.ts` (used only by the version guard);
      the Prism version is reported on the daemon wire by `initialize`, not
      as a Clay binding.
    - Evidence: `cargo test --test protocol` → 216 passed, of which 70 are
      `clay_js_*` inventory / doc-registry / facade-layout assertions
      (including `inventory_rust_paths_name_existing_source_files` and
      `syntax_engine_api_docs_registry_are_fresh`).

- [x] Create or verify Clay configuration APIs
  - Acceptance Criteria:
    - Functional: No new `init.js` option, custom property, or bindable
      command. Node 22 is a process floor, not a user setting.
    - Performance: No config reload path change.
    - Code Quality: Configuration remains documented Clay JS APIs only.
    - Security: Configuration still grants no new authority.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-execution/references/config.md` (as needed)
      - `.agents/skills/create-plan/references/clay.md` (Clay Configuration Task)
    - Options Considered:
      - `run.setOptions({ nodeVersion })`: nonsense; rejected.
      - Verification-only: selected.
    - Chosen Approach:
      - Confirm `examples/config/init.js` (the canonical example; no
        top-level `examples/init.js` exists) needs no new config API or option.
    - API Notes and Examples:
      ```js
      // no new init.js surface
      ```
    - Files to Create/Edit:
      - None expected.
    - References:
      - `examples/config/init.js` section 12 (agent host) — pin version is
        not a user option
  - Test Cases to Write:
    - None.
  - Completion (2026-09-16):
    - No new `init.js` option, custom property, bindable command, or
      `clay:` export resulted from the Prism 0.7.0 / Node 22 migration.
      `MIN_NODE = 22` remains a private startup guard, not configuration;
      `clay-agent/package.json` changes are pins, description, and engine
      floor only (scripts and configuration fields unchanged).
    - Existing config API contract remains intact: `agent.setRunOptions` is
      still the documented `clay:agent` facade with the same eight
      `custom_properties`, no default key binding, generated inventory entry,
      security notes, and server validation. `runtime/js/configuration.d.ts`,
      `runtime/js/agent.d.ts`, `docs/reference/clay-js-api/`, and
      `docs/reference/clay-js-api/api-inventory.toml` gained no new surface.
      No new filesystem, network, shell, package, extension, workspace, or
      AI authority is granted.
    - Canonical example verification: `node --check
      examples/config/init.js` passed. `examples/init.js` is absent by
      design; `examples/config/init.js` is the documented copy tree and
      remains the sole JS-level configuration entry point.
    - Configuration coverage: `cargo test --test protocol configuration`
      → 23 passed, including `phase28_configuration_apis_have_documented_
      bindings_and_closed_options` and the configuration inventory gates;
      the full protocol suite already passed 216/216 in task 4.
    - No config reload path changed: no files under `runtime/js/`,
      `src/server/ops/`, `src/packages/`, `docs/reference/clay-js-api/`, or
      `examples/config/` are part of the migration diff. The only related
      plan correction is the stale reference from nonexistent
      `examples/init.js` to canonical `examples/config/init.js`.

- [x] Execute and update the manual test plan (`test-plan/`)
  - Acceptance Criteria:
    - Functional: Record this cut as automated-only in `test-plan/index.md`
      / `test-plan/16-agent-host.md`: pin + Node floor + event drop have no
      new interactive chrome. Do not weaken existing 16/17 steps.
    - Performance: No new live-launch gate.
    - Code Quality: Coverage matrix notes plan 120.
    - Security: No change to credential/MCP/Obscura manual steps.
  - Approach:
    - Documentation Reviewed:
      - `test-plan/index.md` modules 16 and 17
      - `.agents/skills/create-plan/references/clay.md` (Manual Test Plan Task)
    - Options Considered:
      - Full GUI launch to “see” 0.7.0 in initialize: not user-visible.
      - Record automated-only with reason: selected.
    - Chosen Approach:
      - Add a short plan-120 row to the 16-agent-host coverage notes.
        Existing C-steps stay. If GUI launch is blocked, say so; do not
        claim a live pass.
    - API Notes and Examples:
      ```text
      test-plan/16-agent-host.md — note live pins Prism 0.7.0 / Node >= 22
      ```
    - Files to Create/Edit:
      - `test-plan/16-agent-host.md`: pin note
      - `test-plan/index.md`: plan 120 row if the matrix lists pin cuts
        (113 is listed — add 120 the same way)
    - References:
      - `test-plan/index.md` plan 113 row
  - Test Cases to Write:
    - None beyond the matrix note.
  - Completion (2026-09-16):
    - `test-plan/index.md`: new "Plan 120 manual-test-plan execution record"
      section (automated-only, no live claim) plus a coverage-matrix row
      "Plan 120 Prism 0.7.0 pins + Node >= 22 floor + unknown `AgentEvent`
      drop" pointing at module 16 (pin/event record) and module 17 (existing
      steps unchanged).
    - `test-plan/16-agent-host.md`: new "Plan 120 pin record" section with
      the pin-lockstep, Node-floor, unknown-event-drop, and daemon-suite
      results, all marked automated, plus a NOT RUN live-GUI row. No
      existing A-step or recorded result was edited, deleted, or re-scoped;
      the section explicitly leaves them as the standing procedure.
    - Fresh evidence in the record: `cargo test --test protocol
      phase25_dependencies_deny_acp_agui_mcp` 1 pass, `cargo test --lib
      map_event` 2 pass (`map_event_drops_unknown_event_types`), `clay-agent
      npm test` 149 pass / 0 fail / 1 skip, host Node v24.19.0.
    - Doc-guard verification: `cargo test --test protocol documentation`
      → 17 passed, including `parity_ledger_...` (no new step IDs added, so
      no ledger rows required) and the wiki-navigation guard.
    - Follow-up recorded, not silently fixed: the standing module 16 prose
      still names the historical `examples/init.js`; the canonical file is
      `examples/config/init.js`. Module 17 was not touched. Security steps
      (credentials, MCP allow-list, Obscura) are unchanged.

- [x] Update or verify the code wiki after implementation
  - Acceptance Criteria:
    - Functional: The project code wiki is updated after all implementation
      tasks are complete, or explicitly verified as unchanged for non-code
      work.
    - Performance: Wiki updates add no runtime work and document
      performance-relevant implementation details changed by the plan.
    - Code Quality: Wiki pages explain what changed code does, how it
      works, invariants/tradeoffs, source/test paths, examples where
      useful, and links from the master wiki index.
    - Security: Wiki pages document touched security boundaries,
      permissions, validation, secrets handling, or external authority
      without exposing secrets.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-execution/references/docs-as-code.md`: wiki
        workflow, quality bar, and archive policy.
    - Options Considered:
      - Update after each task: more granular, but noisy and likely to churn.
      - Update once after tests pass: keeps docs aligned with final code. (Chosen.)
    - Chosen Approach:
      - `docs/wiki/modules/clay-agent.md` Node >= 22 and Prism 0.7.0.
        `docs/wiki/modules/agent-protocol.md` if it describes event mapping:
        unknown types dropped, not Started.
    - Files to Create/Edit:
      - `docs/wiki/index.md`: navigation links for changed implementation areas.
      - `docs/wiki/modules/clay-agent.md`: pins, Node floor.
      - `docs/wiki/modules/agent-protocol.md`: mapper default arm (if present).
  - Test Cases to Write:
    - Manual wiki review: the master index links relevant pages and updated
      pages explain what changed implementation does and how it works.
  - Completion (2026-09-16):
    - `docs/wiki/modules/clay-agent.md`: Overview now states Node >= 22 and
      the event-mapping rule (unknown Prism `AgentEvent` types are dropped,
      never forwarded, with the old catch-all failure mode explained and
      links to the protocol/process-manager pages); Responsibilities bump
      every current-behavior stamp to Prism 0.7.0 / `playwright-core@1.63.0`
      (seven-package exact family + `better-sqlite3@13.0.3`), the MCP
      `callTimeoutMs` note to 0.7.0 (re-verified in
      `@arnilo/prism-mcp@0.7.0` — single knob still applied to
      initialize/tools-list, 30 min hard ceiling unchanged), the bootstrap
      step to "refuses Node < 22", and the Obscura CDP peer to 1.63.0.
    - `docs/wiki/modules/agent-protocol.md`: the `AgentWireEvent` paragraph
      now documents the drop rule (`map_event` → `None` →
      `DaemonLine::Ignore`), names the `agent_suspended` incident, lists the
      Prism 0.7 telemetry that exercises it, states that renderable lifecycle
      types need an explicit arm, and adds
      `map_event_drops_unknown_event_types` to the test list.
    - `docs/wiki/modules/agent-process-manager.md`: the mapping
      responsibility now says unknown event types are dropped, never
      forwarded.
    - `docs/wiki/index.md` line 52: link description is now "Node >= 22 Prism
      0.7.0 host — exact 0.7.0 family pins + direct `better-sqlite3@13.0.3`".
    - Same-change stamp sweep (current-behavior comments/docs, no code
      change): `clay-agent/src/providers.ts`, `clay-agent/src/mcp.ts`,
      `clay-agent/src/host.ts` (three stamps), `examples/config/init.js`
      (Prism 0.7.0 caps + Node >= 22), and
      `docs/reference/clay-js-api/agent/set-run-options.md` (policy-axis
      version). Historical provenance notes that name the version a behavior
      landed in (`Prism 0.5.1 kernel`, "0.5.5 fixed cumulative charging") and
      `docs/wiki/archive/` were intentionally left alone.
    - Security boundary unchanged and still documented: vault/keychain-only
      credentials, `env_clear` spawn, MCP allow-list validation, Obscura
      fail-closed absence — no secrets or absolute host paths added.
    - Verification: `cargo test --test protocol documentation` 17 pass (incl.
      wiki-navigation and parity-ledger guards), `cargo test --test protocol
      clay_js` 70 pass, `cargo test --test protocol
      phase25_dependencies_deny_acp_agui_mcp` 1 pass, `node --check
      examples/config/init.js` clean, `git diff --check` clean. No wiki
      navigation change (no page added/renamed); archive untouched.

## Compromises Made

- Jump is 0.5.5 → 0.7.0 in one Clay cut (0.5.6 and 0.6.0 never lived in
  clay-agent). Rollback target is 0.5.5, not 0.6.0.
- Unknown 0.7 lifecycle events are dropped, not rendered. Plans 121–122
  may map specific types later.
- `activateKernel`, attention, spawn, work-scopes, wiki ingest, and fabric
  stay off.

## Further Actions

- Plan 121: attention compiler, wiki ingest, graft init/build-deep,
  `RunOptions.toolNames` RPC seam.
- Plan 122: host-owned `spawn_agent` (Phase 6 primitive).
- Plan 123: work-scope OM (Phase 7 shape, no fabric).
