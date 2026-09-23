# Plan 139 — Bun Daemon Runtime: clay-agent Off Node

## Objectives
- Switch the clay-agent daemon from the Node.js runtime to Bun: clay-server
  spawns `bun` running `src/main.ts` directly (no tsc build on the runtime
  path).
- Make Bun the only production JS process runtime; remove the transitional
  Node pin from mise.

## Expected Outcome
- `resolve_launch` resolves `bun` (PATH, `CLAY_BUN` override); Node is no
  longer referenced by the server, spawn errors, docs, or mise.
- Daemon behavior identical from the frontend's perspective: same NDJSON
  RPC surface, same session features, same event stream; prism 0.7.0 suite
  green under Bun.
- Startup faster (no dist build requirement; measured handshake time
  recorded vs the Node baseline).

## Tasks

- [ ] Baseline: daemon spawn and runtime-assumption inventory
  - Acceptance Criteria:
    - Functional: recorded inventory of every Node-specific assumption in
      the spawn path and daemon: `src/server/agent.rs` (`resolve_node`,
      `CLAY_NODE`, `NodeMissing`, `"agent.node_missing"` string and its
      consumers), daemon Node-API usage (`node:process` stdio, fs,
      child_process, worker_threads via prism), build-path assumptions
      (`dist/main.js`, `CLAY_AGENT_MAIN`), CI/docs mentions of Node ≥ 22,
      and which Rust tests pin node behavior.
    - Performance: none.
    - Code Quality: inventory separates mechanical renames from real
      compatibility risks (each risk gets a verification task below).
    - Security: spawn trust boundaries restated (env_clear, argv literal,
      no shell) — unchanged by the runtime swap.
  - Approach:
    - Documentation Reviewed: `src/server/agent.rs` (spawn, errors,
      tests), `clay-agent/src/main.ts`, bun runtime docs — Node.js
      compatibility (https://bun.com/docs/runtime/nodejs), worker_threads
      status.
    - Options Considered:
      - Skip inventory — rejected: `"agent.node_missing"` has consumers
        that must rename atomically with the variant.
    - Chosen Approach: graft + grep inventory over `src/`, `clay-agent/`.
    - Files to Create/Edit: none (evidence in this plan).
    - References: decision `2026-09-23-1946-bun-runtime-nodejs-surfaces.md`;
      plan 138 (tooling already bun).
  - Test Cases to Write: none.

- [ ] Spawn path: resolve bun, run TypeScript directly
  - Acceptance Criteria:
    - Functional: `resolve_launch` resolves `bun` from `CLAY_BUN`
      (is-file check) then PATH; runs `src/main.ts` (or `CLAY_AGENT_MAIN`
      override) with the same argv (`--data-dir`, config args,
      `CLAY_AGENT_MOCK`); `CLAY_NODE` removed (grep-clean);
      `AgentError::NodeMissing` → `AgentError::RuntimeMissing` with
      message "bun >= 1.2 is required for clay-agent but was not found";
      serialized id `"agent.node_missing"` → `"agent.runtime_missing"`
      updated at every consumer (Rust mapping + frontend status handling)
      in the same change.
    - Performance: daemon first-RPC handshake time measured (test or
      manual timing evidence) and not slower than the Node baseline;
      no `dist/` build required to run.
    - Code Quality: no dead Node resolution code; error variant and id
      renamed atomically (no window where frontend matches the old id);
      `engines` in `clay-agent/package.json` becomes `bun >= 1.2`.
    - Security: spawn hardening unchanged: env_clear, literal argv, no
      shell, frame-size limits, RPC timeouts, respawn actor; `CLAY_BUN`
      is an explicit-file override exactly like `CLAY_NODE` was
      (no PATH search expansion, no relative paths).
  - Approach:
    - Documentation Reviewed: bun CLI — running files
      (https://bun.com/docs/cli/bun), env/flags; current
      `resolve_node`/`resolve_script` in `src/server/agent.rs`.
    - Options Considered:
      - Keep node fallback (try bun, then node) — rejected: two runtimes
        forever, drift between tested and shipped behavior; decision log
        records Bun-only.
      - Keep dist/main.js built path under bun — rejected: bun runs TS
        natively; build step leaves the runtime path (tsc stays as a
        typecheck gate from plan 138).
    - Chosen Approach: bun-only, source-direct; keep `config.program`
      explicit override semantics unchanged (mock harness depends on it).
    - API Notes and Examples:
      ```rust
      // resolve_launch (shape unchanged)
      let bun = resolve_bun()?;               // CLAY_BUN then PATH "bun"
      let script = resolve_script()?;         // CLAY_AGENT_MAIN then src/main.ts
      args = [script, "--data-dir", …];       // bun passes argv like node
      ```
    - Files to Create/Edit: `src/server/agent.rs`,
      `clay-agent/package.json`, frontend status-string consumer (found
      in baseline), `.github/workflows/ci.yml` if it sets `CLAY_NODE`,
      `docs/development/**`.
    - References: decision `2026-09-23-1946-bun-runtime-nodejs-surfaces.md`;
      `src/server/agent.rs` spawn/tests.
  - Test Cases to Write:
    - Rust: spawn test with `CLAY_BUN` pointed at a fixture script via
      `config.program` mock path still passes; missing-bun path yields
      the renamed typed error; stale `CLAY_NODE` env is ignored
      (grep-assert no read).

- [ ] Prism 0.7.0 under Bun: compatibility verification
  - Acceptance Criteria:
    - Functional: full clay-agent test suite (`bun test`, migrated in
      plan 138) green under the bun-spawned daemon, exercising: sqlite
      persistence (better-sqlite3 native), MCP stdio bridges, coding
      tools (fs/child_process paths), compaction, checkpoints; prism's
      `worker_threads` image-processing path smoke-tested (oversize-image
      rejection test or equivalent).
    - Performance: daemon ready-to-first-session time recorded; tool-heavy
      test (coding-tools suite) not slower than Node baseline beyond
      noise.
    - Code Quality: any bun-vs-node behavioral difference found is fixed
      in the daemon shim layer (not by forking prism), or recorded as a
      Further Action with a pin-bump path.
    - Security: no dependency version changes to work around bun gaps;
      if one is unavoidable it is a named decision, not a drive-by.
  - Approach:
    - Documentation Reviewed: bun Node.js compatibility pages (fs,
      child_process, worker_threads, streams), better-sqlite3 under bun
      (known-good per bun docs registry); prism 0.7.0 pin decision
      (`2026-09-16-0026-prism-0.7.0-clay-agent-family-pins.md`).
    - Options Considered:
      - Bump prism if incompatibility found — rejected by default: pins
        are deliberate; escalate instead.
    - Chosen Approach: verification pass over existing suites + one
      worker_threads-targeted smoke; record evidence.
    - Files to Create/Edit: possibly `clay-agent/src/**` shims; test
      evidence under `test-plan/artifacts/` if a manual matrix is run.
    - References: prism pin decision log; bun compatibility docs.
  - Test Cases to Write:
    - Worker-threads image path smoke under bun (fails if bun threads
      regress).

- [ ] Toolchain closeout: drop transitional Node
  - Acceptance Criteria:
    - Functional: `node` entry removed from `mise.toml` (bun remains the
      only pinned JS runtime); CI no longer provisions Node;
      `docs/development/build-and-test.md` states bun as the daemon
      runtime; `CLAY_NODE` gone from docs; the transitional-Node
      rationale removed from `README.md`, `docs/development/windows.md`,
      and `docs/wiki/modules/dev-toolchain.md`.
    - Performance: none.
    - Code Quality: single JS runtime across dev, CI, and the shipped
      daemon; no stale Node references (`grep -rn
      "node = \"24.21.0\"\|24\.21\.0\|CLAY_NODE\|setup-node"` clean
      outside immutable plan/decision history).
    - Security: none.
  - Approach:
    - Documentation Reviewed: `mise.toml` (from plan 137), CI workflow.
    - Options Considered:
      - Keep node pinned "just in case" — rejected: the whole point;
        rollback is git revert.
    - Chosen Approach: delete the pin, update docs.
    - Files to Create/Edit: `mise.toml`, `.github/workflows/ci.yml`,
      `docs/development/build-and-test.md`, `README.md`,
      `docs/development/windows.md`, `docs/wiki/modules/dev-toolchain.md`.
    - References: decisions 2026-09-23-1946 (both).
  - Test Cases to Write: none.

- [ ] Live launch-test: daemon on bun end-to-end
  - Acceptance Criteria:
    - Functional: recorded manual launch test (scratch HOME per the
      launch-test duty): Clay starts, clay-server spawns the bun daemon,
      a real agent session runs one authenticated provider turn
      (streaming + one tool execution + sqlite session reload after
      restart), no Node process in the tree (`ps` evidence).
    - Performance: daemon spawn→ready latency recorded in the evidence.
    - Code Quality: evidence file under `test-plan/artifacts/` following
      the existing launch-test format.
    - Security: no secrets in the recorded evidence.
  - Approach:
    - Documentation Reviewed:
      `.agents/skills/clay-execution/references/clay.md` (live
      launch-test duty), `docs/development/launch-and-gui-smoke.md`.
    - Options Considered:
      - CI-only verification — rejected: real provider turn + process
        tree can't be faked in CI; this is the recorded manual gate.
    - Chosen Approach: one scripted launch-test run, evidence committed.
    - Files to Create/Edit:
      `test-plan/artifacts/139-bun-daemon/launch-test.md` (tentative
      path).
    - References: clay.md duties.
  - Test Cases to Write: none (this is the manual check).

- [ ] Final code-wiki task
  - Acceptance Criteria:
    - Functional: `docs/wiki/modules/clay-agent.md` (and the spawn
      module page if separate) updated: bun runtime, `CLAY_BUN`,
      `RuntimeMissing`, no dist build on the runtime path; and
      `docs/wiki/modules/dev-toolchain.md` updated to the bun-only
      toolchain story (transitional Node pin and rationale removed);
      wiki maintenance validation passes.
    - Performance: none.
    - Code Quality: standard code-wiki template per
      `.agents/skills/clay-execution/references/docs-as-code.md`.
    - Security: trust-boundary description (env_clear, argv, frame
      limits) restated unchanged.
  - Approach:
    - Documentation Reviewed:
      `.agents/skills/clay-execution/references/docs-as-code.md`,
      `docs/wiki/modules/clay-agent.md` (current), this plan's diffs.
    - Options Considered:
      - Skip — rejected: one final code-wiki task per plan.
    - Chosen Approach: amend existing module pages.
    - Files to Create/Edit: `docs/wiki/modules/**`.
    - References: clay-execution references.
  - Test Cases to Write: none.

## Compromises Made
- To be filled after tasks are completed and tests pass.

## Further Actions
- To be filled after task completion. Known at planning time: plan 150
  (detached agent runtime server) later rebuilds spawn supervision for
  detachment — it must spawn the **bun** daemon via this plan's
  resolution path, not reintroduce node assumptions; note this in plan
  150's baseline when it executes.
