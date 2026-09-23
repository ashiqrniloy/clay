---
date: 2026-09-23 19:46
status: approved
decision_about: "JavaScript runtime migration: all Node.js runtime surfaces (clay-agent daemon, dev tooling, tests) move to Bun; deno_core embedded package sandbox in the Rust server stays unchanged"
proposed_by: user
explicitly_approved_by_user: true
---

# Decision: Bun Runtime for Node.js Surfaces, deno_core Sandbox Unchanged

## Decision

Everything Clay currently runs on a Node.js runtime moves to **Bun**:
the clay-agent daemon (spawned by clay-server) and the development
workflow (dependency install, script execution, test running, package
tooling). Bun becomes the single required JS process runtime, pinned via
the mise dev toolchain. The **deno_core sandbox embedded in the Rust
server stays exactly as it is** — it is not a Node alternative but the
embedded V8 isolation boundary for Clay packages (per-domain isolates,
capability ops, heap ceilings, load-entry allowlists), and Bun exposes no
Rust embedding API that could replace it.

## Context

- clay-agent daemon today: clay-server resolves PATH `node`
  (`resolve_node`, `CLAY_NODE` override, `AgentError::NodeMissing`,
  Node >= 22 floor) and spawns `dist/main.js` built by `tsc`. Daemon Node
  API surface is small and fully Bun-implemented (`node:process` stdio,
  `fs/promises`, `child_process`, `crypto`, `path/os/url`, custom NDJSON
  framing).
- Dependency compatibility verified: `better-sqlite3` 13.x (native NAPI —
  prism-core, prism-channels, daemon persistence) works under Bun;
  `playwright-core` (pure JS CDP driver) works; prism's
  `worker_threads` image path is implemented in Bun and needs one
  verification pass.
- Dev workflow today: `npm ci` per package, `node --test` for clay-agent
  (`node:test` imports), vitest for frontend, `node
  scripts/bundle-budget.mjs`, `setup-node` in CI.
- deno_core 0.400 in the Rust server (`src/server/js_runtime/`) runs
  Clay packages (`packages/javascript`, `lsp-*`, `git`, `rust`, markdown,
  themes…) in per-lane isolates with ops for shell/git/parse/workspace/
  language-server/config — a security and latency boundary inside the
  Rust process, unreachable by a Bun migration.

## Approval

- Proposed by: user.
- Approved by user: Yes
- Approval evidence: "I want to migrate everything in clay from a nodejs
  runtime to bun runtime. This is to increase the performance of the app
  as much as possible. I think we are still using deno_core inside the
  rust backend. I believe that should stay as it is. But anything that we
  are doing on the nodejs runtime should be shifted to bun. This includes
  the dev workflow of testing and creating packages." (2026-09-23)

## Alternatives Considered

- Keep Node everywhere — rejected: slower install/test/dev loop, build
  step (tsc) on the daemon runtime path, no upside beyond familiarity.
- Hybrid (bun for tooling, node for the daemon) — rejected: two runtimes
  to test forever, drift between dev and shipped daemon behavior.
- Replace deno_core with Bun-as-sandbox — rejected (and infeasible): Bun
  has no Rust embedding/isolate API; per-domain isolation, heap budgets,
  and synchronous op bridging (completion latency lane) have no Bun
  equivalent. deno_core is a trust boundary, not a convenience runtime.
- Bun single-file compile of the daemon — deferred: install-size win,
  release-pipeline complexity; revisit when distribution reports show
  users lack Bun.

## Consequences

- Daemon runtime switch happens in plan 139 (Bun daemon runtime) after
  plan 137 (mise toolchain pins Bun) and plan 138 (bun tooling/tests);
  sequential plan numbers are execution order.
- Production spawn is Bun-only (no node fallback) once plan 139 lands;
  `CLAY_NODE` is replaced by `CLAY_BUN`; the transient Node pin in mise
  is removed in plan 139.
- `AgentError::NodeMissing` and its `"agent.node_missing"` status id are
  renamed atomically (variant + serialized id + frontend mapping, both
  sides in-repo).
- Prism pins remain `0.7.0`; its full test suite must pass under Bun
  before the switch; Bun version is pinned by mise (local and CI agree).
- deno_core and the packages' op surface are untouched by this decision;
  the `clay` CLI's npm-only package source (Phase 3) continues to target
  the npm registry — only the local install/execution tooling changes.
- Windows: Bun supports Windows; daemon spawn paths keep the Windows
  branch compiling, Linux gates remain blocking (platform-validation
  rule).
