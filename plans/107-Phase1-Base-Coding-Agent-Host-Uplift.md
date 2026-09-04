# Phase 1: Base Coding Agent Host Uplift (`clay-agent`)

Source: `roadmap.md` Phase 1 (Base Coding Agent Host Uplift) and the logged
resolutions that phase inherits.

This plan uplifts the Clay-owned Node `clay-agent` daemon from the Phase 0
chat host (three Prism 0.4 families) to a coding-capable host: coding-tools,
web-tools, memory compaction, and MCP as a package-declared bridge. It does
**not** ship `@clay/coding-agent`, agent UI chrome, wiki/graft, workflows,
supervisors, Antigravity, or computer-use-linux.

Status: daemon tasks (1-10) complete and verified 2026-09-02; remaining
non-daemon tasks (11-15: Clay JS APIs, configuration, example, manual test
plan, wiki refresh) follow.

Scope note: **no UI-surface tasks**. Approval, search, tree, and autonomy are
daemon RPC + server protocol this phase. Chat rendering stays prompt/response.
If execution must change React/SDUI/chat chrome to meet the exit gate, stop,
load `clay-ui` plus the four mandatory design skills, and add UI tasks — do
not silently restyle Chat. No editor mode, language mode, or first-party JS
package is added, so the editor primitive-review, `init.js` package-load, grammar,
and package UI/authoring tasks are not required. Document-operations inventory
(task 3), trust-domain/process-authority (task 2), Clay JS API, configuration,
example `init.js`, and manual-test tasks **are** required.

Confirmed architecture:

- `roadmap.md` Phase 1 and Prism 0.4 adoption map
- `decision-logs/2026-08-21-1758-native-prism-host-no-acp-cli-parity.md`
  (Clay-owned Node host; Clay document operations, not ACP `fs/*`)
- `decision-logs/2026-08-21-2152-product-surfaces-are-replaceable-packages.md`
- `decision-logs/2026-08-30-2157-default-acceptance-workspace-free-host-write-gated.md`
- `decision-logs/2026-08-30-2158-observational-memory-defaults-worker-models-80k-per-session.md`
- `decision-logs/2026-08-30-2159-obscura-web-browser-stack-and-computer-use-linux.md`
- `decision-logs/2026-08-30-2200-pi-tree-model-extended-to-workspace-checkpoints-and-discard.md`
- `decision-logs/2026-08-30-2201-workspace-scoped-session-search-shared-by-clay-and-st.md`
- `decision-logs/2026-09-02-0121-prism-0.4.0-clay-agent-family-pins.md`
- `decision-logs/2026-07-14-2023-language-server-package-authority.md`
  (process-authority template for host/package children)
- `decision-logs/2026-07-21-0001-two-package-runtime-trust-domains.md`

Project patterns: `planning-checklist.md`, `agent-host.md`,
`authority-boundaries.md`, `extensions-and-ai.md`,
`package-runtime-trust-domains.md`, `product-surfaces-are-packages.md`,
`clay-js-api-naming.md`, `clay-js-api-boundary.md`, `clay-js-api-schema.md`,
`configuration-system.md`, `documentation-as-code.md`,
`doc-registry-tests.md`, `protocol-and-performance.md`,
`maintenance-validation.md`.

Library docs:

- Context7 has no `@arnilo/prism`. Authoritative APIs: Prism
  `docs/migrate-to-0.4.md` (0.4.0), `docs/coding-agent-tools.md`,
  `docs/coding-security.md`, `docs/compaction-and-retry.md`,
  `docs/compaction-llm.md`, `docs/compaction-observational-memory.md`,
  `docs/context-and-skills.md`, `docs/extension-authoring.md` (CommandDrivers),
  `docs/agent-session-runtime.md` (durable runs, checkout/fork/clone),
  `docs/session-stores.md` (`searchSessions`), `docs/mcp-tools.md`,
  `docs/obscura.md`, and family `package.json` export maps under
  `/home/arn/Projects/prism/packages`.
- `playwright-core` exact pin `1.61.0` is the `@arnilo/prism-web-tools`
  optional peer. Use only with Obscura CDP composition
  (`connectObscuraCdp` → `chromium.connectOverCDP`). Do not add it merely
  because `prism-web-tools` is installed.
- `better-sqlite3@12.11.1` already pinned in Phase 0. Session FTS is Prism
  `prism_session_search_fts` (schema v4) plus
  `@arnilo/prism-core/sessions/codecs` field helpers — no second database.

Not in scope: `@clay/coding-agent` (Phase 2), wiki/graft (Phase 2),
`@arnilo/prism-core/runtime/workflows` (Phase 5), supervisor/model-router
(Phase 6), `@arnilo/prism-antigravity-agent` (Phase 6),
`prism-coding-tools/computer-use-linux` (Phase 5), `prism-office`,
`prism-acp-agent`, `prism-ag-ui` in the daemon, `/brave` `/exa` `/firecrawl`,
`/ai-sdk`, Git tool set, Docker/native sandbox composition, Prism HTTP
`createPrismHandler` listener, ACP `createAcpFilesystemOperations`.

## Objectives

- Pin exact `@arnilo/prism-coding-tools@0.4.0`, `prism-web-tools@0.4.0`,
  `prism-memory@0.4.0`, and `prism-mcp@0.4.0`. Narrow
  `phase25_dependencies_deny_acp_agui_mcp` so ACP/AG-UI and retired 0.3 names
  stay forbidden while `@arnilo/prism-mcp` / `@modelcontextprotocol/sdk` are
  allowed as the package-declared MCP bridge.
- Register the nine coding tools with Clay document `operations` for
  `read`/`write`/`edit`, workspace-confined `shell`/list/search/glob, and the
  logged default acceptance policy (workspace free; host writes gated; opt-in
  full autonomy off by default). Keep D1 omitted-`allowCustom` resume in
  daemon smoke.
- Add compaction (root default + LLM + observational-memory), skills,
  command dispatch with host `CommandDrivers`, durable
  `interruptBeforeTool` runs, workspace-scoped `session.search`, session
  tree checkout/fork/clone linked to document version checkpoints, allow-listed
  MCP, and fail-closed Obscura.
- Keep Chat/mock flows byte-compatible: Chat profiles still have no tools.

## Expected Outcome

- `clay-agent/package.json` pins the Phase 0 three families plus the four
  Phase 1 families at exact `0.4.0`, `better-sqlite3@12.11.1`, and
  `playwright-core@1.61.0` once Obscura CDP plumbing lands.
- A registered coding profile can run the nine tools against an open Clay
  document (dirty-buffer fidelity), suspend for host writes / ask-user, persist
  history, compact, attach OM, and search only the current workspace.
- Obscura/MCP/web capabilities are hidden when the binary or allow-list is
  absent. ACP/AG-UI stay out of the daemon. Chat remains no-tools.
- Linux gates pass. Wiki `docs/wiki/modules/clay-agent.md` matches the live
  host.

## Tasks

- [x] Pin Phase 1 Prism families and narrow the MCP deny test
  - Acceptance Criteria:
    - Functional: `clay-agent/package.json` has exact `"@arnilo/prism-coding-tools": "0.4.0"`,
      `"@arnilo/prism-web-tools": "0.4.0"`, `"@arnilo/prism-memory": "0.4.0"`,
      `"@arnilo/prism-mcp": "0.4.0"` beside the Phase 0 pins. `npm ci` and
      `npm ls --all` show those families; no retired 0.3 names; no
      `prism-acp-agent`, `prism-ag-ui`, `prism-office`,
      `prism-antigravity-agent`. `playwright-core` is **not** added in this
      task.
    - Performance: Install/lock only. No extra daemon process or RPC method.
    - Code Quality: Imports of the new families wait for later tasks. README
      pin list and Upgrade Prism steps include the four families and still
      say MCP is not a first-party agent bus.
    - Security: `phase25_dependencies_deny_acp_agui_mcp` still fails closed on
      `prism-acp`, `prism-ag-ui`, `agentclientprotocol`, retired
      `prism-coding-agent` / `@arnilo/prism-coding-agent` in Cargo.toml,
      clay-agent sources, and package.json. `@modelcontextprotocol` /
      `@arnilo/prism-mcp` are allowed **only** in `clay-agent/package.json`
      (and lockfile), never in `Cargo.toml` or Rust sources.
  - Approach:
    - Documentation Reviewed:
      - Prism `docs/migrate-to-0.4.md` — family mapping; `prism-web-tools`
        requires `prism-mcp` peer; optional peers stay uninstalled
      - `decision-logs/2026-09-02-0121-prism-0.4.0-clay-agent-family-pins.md`
      - `.agents/skills/project-patterns/references/agent-host.md`
      - `tests/agent_protocol.rs` `phase25_dependencies_deny_acp_agui_mcp`
      - `clay-agent/README.md` Pins / Upgrade Prism
    - Options Considered:
      - Pin families only when first imported: rejected. Roadmap exit and
        the deny-test change need the pins first so later tasks do not mix
        lockfile churn with behavior.
      - Also pin `playwright-core` now: rejected. Roadmap adds it when Obscura
        CDP plumbing lands (task 9).
    - Chosen Approach:
      - Add the four exact 0.4.0 dependencies, regenerate the lockfile, update
        the deny test and README. Do not import coding/web/memory/MCP symbols
        yet. `npm approve-scripts` only if a new native install script appears
        (none expected beyond existing `better-sqlite3`).
    - API Notes and Examples:
      ```json
      {
        "dependencies": {
          "@arnilo/prism": "0.4.0",
          "@arnilo/prism-core": "0.4.0",
          "@arnilo/prism-providers": "0.4.0",
          "@arnilo/prism-coding-tools": "0.4.0",
          "@arnilo/prism-web-tools": "0.4.0",
          "@arnilo/prism-memory": "0.4.0",
          "@arnilo/prism-mcp": "0.4.0",
          "better-sqlite3": "12.11.1"
        }
      }
      ```
      ```ts
      // Not imported this task. Later tasks use subpaths only:
      // "@arnilo/prism-coding-tools/agent"
      // "@arnilo/prism-coding-tools/security"
      // "@arnilo/prism-memory/compaction/llm"
      // "@arnilo/prism-memory/compaction/observational-memory"
      // "@arnilo/prism-web-tools"
      // "@arnilo/prism-web-tools/obscura"
      // "@arnilo/prism-web-tools/browser"
      // "@arnilo/prism-mcp"
      // "@arnilo/prism-core/sessions/codecs"
      ```
    - Files to Create/Edit:
      - `clay-agent/package.json`: four exact 0.4.0 deps
      - `clay-agent/package-lock.json`: regenerate via `npm install`
      - `clay-agent/README.md`: pin list; MCP-as-bridge wording; deny list
      - `tests/agent_protocol.rs`: allow prism-mcp / modelcontextprotocol in
        clay-agent package.json only; keep Cargo/ACP/AG-UI/retired denies
    - References:
      - `roadmap.md` Phase 1 first bullet
      - `.agents/skills/project-patterns/references/agent-host.md`
      - `plans/106-Phase0-Prism-0.4.0-Family-Migration-and-Verification.md`
  - Test Cases to Write:
    - `phase25_dependencies_deny_acp_agui_mcp`: package.json contains the four
      new exact pins; Cargo.toml still has no MCP/ACP/AG-UI; retired 0.3
      coding-agent names still denied everywhere.
    - Manual `cd clay-agent && npm ls --all`: four new families present;
      `prism-acp-agent` / `prism-ag-ui` / `prism-office` / `prism-antigravity-agent`
      absent; `playwright-core` unmet.

- [x] Confirm process-authority and trust-domain coverage before host children
  - Acceptance Criteria:
    - Functional: A short in-plan record (this task’s completion evidence)
      maps Obscura, package-declared MCP, skills, and commands onto existing
      decision logs. No new decision log unless a real gap needs user
      approval — then stop.
    - Performance: No runtime work.
    - Code Quality: Later tasks implement the mapped rules; they do not
      invent a second process-grant model or run package JS in the daemon.
    - Security: Daemon never executes package JavaScript. MCP/skills/commands
      are inert data until the daemon activates them under allow-list /
      provenance / user approval. Obscura is host-owned (not
      package-triggered). Same-user children are not called sandboxed.
      Third-party MCP requires a declared extension point **and** explicit
      user approval. Trusted vs third-party runtimes stay two domains.
  - Approach:
    - Documentation Reviewed:
      - `decision-logs/2026-08-21-1758-native-prism-host-no-acp-cli-parity.md`
        (MCP allow-list; no ACP; package JS cannot spawn daemon)
      - `decision-logs/2026-08-30-2159-obscura-web-browser-stack-and-computer-use-linux.md`
      - `decision-logs/2026-07-14-2023-language-server-package-authority.md`
      - `decision-logs/2026-07-21-0001-two-package-runtime-trust-domains.md`
      - `.agents/skills/create-plan/references/clay.md` External Process
        Authority + Package Runtime Trust-Domain tasks
      - `.agents/skills/project-patterns/references/authority-boundaries.md`
      - `.agents/skills/project-patterns/references/package-runtime-trust-domains.md`
      - `.agents/skills/project-patterns/references/agent-host.md`
      - Prism `docs/mcp-tools.md`, `docs/obscura.md`, `docs/host-security.md`
    - Options Considered:
      - New MCP/Obscura decision log now: rejected unless a gap is found.
        1758 + 2159 + 0714 already decide allow-list, host-owned Obscura,
        and deny-by-default children.
      - Treat Obscura as package-triggered: rejected. Host-owned binary,
        daemon lifecycle, hidden when absent (2159).
      - Confirm coverage, implement against those logs: selected.
    - Chosen Approach:
      - Read the four logs and write completion evidence: capability → log →
        grant shape → truthful containment language. If package MCP argv
        cannot be bound to a fixed contribution + canonical executable as
        0714 requires, stop and ask for a decision. Do not start spawn code
        in this task.
    - API Notes and Examples:
      ```ts
      // Host-owned Obscura (2159). Absolute command, shell-free, fail-closed.
      import { spawnObscuraProcess } from "@arnilo/prism-web-tools/obscura";
      const obscura = spawnObscuraProcess({
        command: "/usr/local/bin/obscura",
        args: ["serve", "--host", "127.0.0.1", "--port", "9222"],
      });

      // Package MCP: inert allow-list entry, not package-spawned argv.
      import { connectMcpTools } from "@arnilo/prism-mcp";
      const bridge = await connectMcpTools({
        serverId: "fs",
        transport: { type: "stdio", command: "/usr/bin/fixed-server", args: ["--stdio"] },
      });
      ```
    - Files to Create/Edit:
      - None expected. New `decision-logs/YYYY-MM-DD-HHMM-*.md` only if a
        gap is found and the user approves logging.
    - References:
      - `.agents/skills/create-plan/references/clay.md`
      - `roadmap.md` Phase 1 MCP + Obscura bullets
  - Test Cases to Write:
    - None this task. Later MCP/Obscura tasks must include deny/revocation/
      absent-binary/absent-allow-list tests named in the evidence map.
  - Completion Evidence (2026-09-02):
    - Coverage map — capability → log → grant shape → containment language:
      - **Obscura** → 2159 (+ 0714 lifecycle pattern): host-installed binary,
        fail-closed `spawnObscuraProcess`, complete advertised `obscura_*`
        MCP surface, CDP + Playwright composition, all hidden when binary
        absent. Prism `validateObscuraCommand` already enforces absolute
        canonical command, NUL-free bounded literal argv, env-key pattern,
        `INSECURE_FLAGS` deny (`--allow-private-network`,
        `--allow-file-access`) unless explicit, loopback-only CDP hosts —
        matches the log. Language: **host-owned trusted subprocess, never
        sandboxed**; same-user child can read other paths/network under OS
        permissions.
      - **Package-declared MCP** → 0714 contract mapped onto Phase 1
        daemon: server builds the allow-list entry (fixed `serverId`,
        canonical executable, literal argv, explicit env, known roots) and
        the daemon only `connectMcpTools`es what the server sent. Package
        JS never passes executable/argv/cwd/env at any phase — Phase 2/4
        packages declare inert contributions; Clay resolves + canonicalizes
        + user-approves (0714 flow), then forwards the fixed entry. Prism
        `connectMcpTools` matches: host explicitly chooses command/args/env/
        cwd, no PATH search, empty default exposure, MCP output is untrusted
        and dispatches through PermissionPolicy + redactor. 0714 disclosure
        applies verbatim: **trusted same-user subprocess authority, not an
        OS sandbox**. Gap check: no gap — the fixed-contribution binding is
        satisfiable because argv never originates from runtime JS; no new
        decision needed for Phase 1. Phase 2/4 must bind package provenance
        + contribution digest into the server-side allow-list record before
        any third-party MCP is user-approved (third-party MCP needs declared
        extension point **and** explicit user approval per trust-domains
        0001).
      - **Skills/commands** → 1758 + 0001: inert data over RPC into kernel
        registries; progressive disclosure via `load_skill`; host-only
        `CommandDrivers` injected in-process, never from RPC params; no
        package JS executes in the daemon (two-runtime rule); registering a
        skill grants no filesystem/network/shell. Language: **inert
        declarative data activated by the host**, not executed package code.
      - **computer-use-linux** → 2159: explicitly `st`-only, Phase 5; base
        clay agent stays autonomy-free. Out of scope here.
    - Deny obligations carried into later tasks: MCP tools register only
      from server entries; empty allow-list → zero MCP tools; revocation /
      shutdown kills owned children; absent Obscura binary hides web/
      browser tools without error; Cargo.toml never gains MCP; no
      `createAcpFilesystemOperations`, no ACP/AG-UI bus (1758).
    - No new decision log required.

- [x] Review existing document and agent primitives before coding-tool operations
  - Acceptance Criteria:
    - Functional: Written inventory (this task’s evidence) lists existing
      document registry/lease/version/dirty APIs, `AgentHost` stdio RPC,
      `AgentClientCommand` / `AgentWireEvent` (including unused tool/
      permission variants), and Prism `ReadOperations`/`WriteOperations`/
      `EditOperations` seams. It states what Phase 1 can do with those
      before new Rust, and names only generic gaps (document checkpoint
      join, daemon→server reverse RPC for tool backends).
    - Performance: Inventory adds no runtime work. Reverse RPC and
      checkpoints stay off the CodeMirror typing/paint path.
    - Code Quality: No mode-specific Rust. No ACP filesystem adapter. New
      primitives, if any, are reusable by later agent packages (`st`), not
      named around coding-agent UI.
    - Security: AI mutation stays server-authoritative (version, lease,
      range). Daemon does not open workspace files behind the document
      registry for `read`/`write`/`edit`.
  - Approach:
    - Documentation Reviewed:
      - Prism `docs/coding-agent-tools.md` — operations seams; reject
        `createAcpFilesystemOperations`
      - `decision-logs/2026-08-21-1758-native-prism-host-no-acp-cli-parity.md`
      - `docs/wiki/modules/clay-agent.md`
      - `docs/wiki/modules/phase25-agent-protocol.md`
      - `docs/wiki/modules/phase25-agent-host-primitive-review.md` (historical)
      - `src/server/agent.rs`, `src/protocol/agent.rs`, `src/server/document.rs`
      - `.agents/skills/project-patterns/references/authority-boundaries.md`
      - `.agents/skills/project-patterns/references/protocol-and-performance.md`
      - `.agents/skills/project-patterns/references/mode-primitive-first.md`
        (why this is **not** an editor-mode primitive-review)
    - Options Considered:
      - `createAcpFilesystemOperations`: rejected (1758).
      - Daemon reads/writes disk for open documents: rejected. Dirty buffers
        live in the server registry.
      - Reverse-RPC: daemon tool `operations` call parent JSON-RPC
        (`document.read` / `document.write` / `document.edit`); server
        applies through the existing document/lease path: selected.
      - Skip checkpoints until Phase 2 `/tree` UI: rejected. Roadmap Phase 1
        stores the join; UI is Phase 2.
    - Chosen Approach:
      - Record the reverse-RPC shape and checkpoint join. Implement in
        task 4 and task 8. Do not add `createPrismHandler` HTTP. Do not add
        editor primitives.
    - API Notes and Examples:
      ```ts
      import { createCodingTools } from "@arnilo/prism-coding-tools/agent";
      const tools = createCodingTools(workspaceRoot, {
        read: { operations: clayDocumentOps.read },
        write: { operations: clayDocumentOps.write },
        edit: { operations: clayDocumentOps.edit },
        executionPolicy,
      });
      // repo_list / repo_search / glob / shell stay workspace-cwd disk backends.
      ```
    - Files to Create/Edit:
      - None. Evidence lives in this task’s completion notes.
    - References:
      - `roadmap.md` Phase 1 coding-tools bullet
      - `src/server/agent.rs` `rpc` / `map_event`
      - `src/protocol/agent.rs` `AgentClientCommand`, `AgentToolPhase`
  - Test Cases to Write:
    - None this task. Task 4 tests dirty-buffer read/write/edit and lease
      rejection.
  - Completion Evidence — primitive inventory (2026-09-02):
    - **Document registry / mutation authority** (`src/server/document.rs`,
      `src/server/ops/documents.rs`): `DocumentState` owns `version`
      (`DocumentVersion`, starts at 1), `active_lease: Option<EditableLease>`
      (single editable lease per document, `client_id` + `lease_id`), `dirty`,
      `created_at_version`; `apply_edit` / `apply_edit_with_parse_input` are
      `pub(crate)` single mutation path. Clay JS ops already expose
      `open_document` / `save_document` / `reload_document` /
      `get_document_status` / `list_documents` returning `{version, leaseId,
      dirty}`, and save takes a caller `knownVersion` for CAS (runtime 0
      bypasses an editable lease but cannot claim newer state than server).
      Phase 1 needs **no new mutation path**: `document.read`/`write`/`edit`
      reverse-RPC handlers call the same `apply_edit` + CAS surfaces, so AI
      mutation stays version/lease/range authoritative.
    - **AgentHost stdio RPC** (`src/server/agent.rs`):
      `AgentHost::run(AgentClientCommand) -> AgentServerMessage` with 30s
      `RPC_TIMEOUT`, 1 MiB frames, `env_clear` + owner-only passphrase spawn;
      `HostCommand::Rpc` queue; stdout pump already parses daemon→server
      messages via `map_event` and **daemon-initiated requests already get
      replies** (line ~848–851 `reply.send(Ok(result))`) — the reverse-RPC
      frame plumbing exists; what is missing is only method routing to
      document ops (generic gap, task 4).
    - **Wire protocol** (`src/protocol/agent.rs`): `AgentClientCommand`
      (Prompt/Cancel/Steer/NewSession/Load/Resume/Delete/ListSessions/Picker/
      Credential/RegisterProfile) and `AgentWireEvent` already carry unused
      `Tool { phase: AgentToolPhase, tool_call_id }` and `Permission {
      request_id, tool_name, allowed }` variants — no IPC rewrite needed;
      Phase 1 fills the existing union. `AgentTranscriptKind` has no tool
      kind yet (add when tools land, task 4).
    - **Prism seams** (`docs/coding-agent-tools.md`): every tool accepts an
      `operations` seam — `ReadOperations` (bounded `readText` + `statFile`),
      `WriteOperations` (temp+rename durability), `EditOperations`
      (`statFile`), plus workspace-cwd `BashOperations`/`RepositoryOperations`/
      `DeleteOperations`/`MoveOperations` with containment caps.
      `createAcpFilesystemOperations` rejected (1758).
    - **What Phase 1 can do before new Rust**: register tools against Prism
      factories; back `read`/`write`/`edit` with Clay ops through the
      existing reply path; run `shell`/list/search/glob/delete/move against
      workspace root with Prism defaults; emit `Tool`/`Permission` events
      already in the union. **Generic gaps only**: (1) document checkpoint
      join — no server-side `(sessionId, entryId) → document version` map
      yet (task 8); (2) daemon→server `document.*` request routing (task 4);
      (3) `run.resume` / compact / search client commands (tasks 5–8). No
      mode-specific primitives, no ACP adapter, no HTTP handler; new
      primitives stay generic for `st` reuse.
    - Security note carried forward: the daemon never opens workspace files
      behind the registry for open documents; disk backends apply only to
      workspace-cwd tools (`shell`/`glob`/search), never `read`/`write`/`edit`
      of registry documents.

- [x] Register coding tools with Clay document operations, acceptance policy, and D1 smoke
  - Acceptance Criteria:
    - Functional: Host registers the nine tools from
      `@arnilo/prism-coding-tools/agent`. Chat profiles with omitted tools
      still get none. A coding profile’s `read`/`write`/`edit` go through
      Clay document reverse-RPC (open dirty buffer wins over disk).
      `shell` / `repo_list` / `repo_search` / `glob` / `delete` / `move`
      stay workspace-root confined. Default policy: inside workspace, no
      approval; outside-workspace reads free; outside-workspace writes
      suspend for permission; full-autonomy flag (off by default) lifts
      that write gate for the session/run. Egress stays
      `ExecutionPolicy`/sandbox rules (no new unrestricted network). D1:
      `ask_user_decision` suspend/resume without explicit `allowCustom`
      persists `allowCustom: false` and resumes.
    - Performance: Tool reverse-RPC uses the existing 30s `RPC_TIMEOUT`
      and 1 MiB frame cap. Tool work is off the editor hot path. Event
      queue stays `maxQueuedEvents: 256`, `overflow: "drop_oldest"`.
    - Code Quality: Subpath imports only. No ACP types. Mock provider
      tests do not hit the network. Linux `cargo fmt`/`check`/`clippy -D
      warnings` stay green for touched Rust.
    - Security: Secrets stay redacted. Paths symlink-contained via
      `createCodingApprovalPolicy` roots. Daemon cannot be spawned by
      package JS. Full autonomy is explicit per session/run, never a
      process-wide default.
  - Approach:
    - Documentation Reviewed:
      - Prism `docs/coding-agent-tools.md` (`createCodingTools`, operations)
      - Prism `docs/coding-security.md` (`createCodingApprovalPolicy`)
      - Prism `docs/coding-agent-tools.md` / ask-user
        `toAskUserDecisionSuspendData` (D1: omitted `allowCustom` → false)
      - `decision-logs/2026-08-30-2157-default-acceptance-workspace-free-host-write-gated.md`
      - Task 3 inventory
    - Options Considered:
      - Per-tool host wrappers instead of `operations` seams: rejected.
        Prism already has the seams.
      - Approve every workspace write (pi-style): rejected (2157).
      - Docker/native sandbox this phase: rejected. Policy + roots first;
        sandbox composition is not in the Phase 1 exit gate.
    - Chosen Approach:
      - `createCodingTools(cwd, { read/write/edit operations, executionPolicy })`.
        Policy `roots` = workspace roots; `approve` allows in-root mutating
        actions, asks the user for out-of-root writes unless
        `fullAutonomy`, always applies command/egress rules. Register tools
        on the kernel; resolve only when the profile lists them. Wire
        `createAskUserDecisionTool` for D1 smoke (not Chat).
    - API Notes and Examples:
      ```ts
      import { createCodingTools, createAskUserDecisionTool } from "@arnilo/prism-coding-tools/agent";
      import { createCodingApprovalPolicy } from "@arnilo/prism-coding-tools/security";

      const executionPolicy = createCodingApprovalPolicy({
        roots: workspaceRoots,
        approvalCacheScope: "run",
        approve: async ({ action, path }) => {
          if (isInsideWorkspace(path)) return true;
          if (action === "read") return true;
          if (fullAutonomy) return true;
          return host.confirm(action, path);
        },
      });
      const tools = createCodingTools(workspaceRoot, {
        executionPolicy,
        read: { operations: clayOps.read },
        write: { operations: clayOps.write },
        edit: { operations: clayOps.edit },
      });
      ```
      ```ts
      import { toAskUserDecisionSuspendData, createAskUserDecisionTool } from "@arnilo/prism-coding-tools/agent";
      // D1: omit allowCustom → persisted false; resume must not drop the field.
      const data = toAskUserDecisionSuspendData({ question, options, selectionMode: "single" });
      // data.allowCustom === false
      ```
    - Files to Create/Edit:
      - `clay-agent/src/coding-tools.ts`: tool factories + policy
      - `clay-agent/src/document-ops.ts`: reverse-RPC Read/Write/EditOperations
      - `clay-agent/src/host.ts`: register tools; `session.prompt` passes
        cwd/roots/fullAutonomy; new `session.setAutonomy` if needed
      - `clay-agent/src/main.ts`: daemon-initiated document RPC frames
      - `src/server/agent.rs`: handle `document.read`/`write`/`edit`;
        forward autonomy
      - `src/protocol/agent.rs`: permission/tool events already exist;
        add command only if autonomy needs a client op
      - `clay-agent/src/__tests__/coding-tools.test.ts`: nine tools + dirty
        buffer + policy + D1
      - `clay-agent/README.md`: coding profile vs Chat honesty
      - Tentative: `src/server/workspace/` only if checkpoint/version APIs
        need a thin helper (prefer existing document apply/save)
    - References:
      - `roadmap.md` Phase 1 coding tools + acceptance policy + D1
      - `.agents/skills/project-patterns/references/extensions-and-ai.md`
  - Test Cases to Write:
    - Chat profile: omitted tools → no tool dispatch (existing unknown-tools
      fail-closed still holds).
    - Coding profile mock: `read` of an unsaved open document returns buffer
      text, not on-disk bytes.
    - `write`/`edit` of that document bumps server version and dirty flag.
    - `edit` without a current lease is rejected; disk unchanged.
    - In-workspace `write` does not emit an approval prompt.
    - Out-of-workspace `write` suspends; denied → no write; approved → write;
      `fullAutonomy: true` skips the prompt.
    - `shell`/`glob` path escape outside workspace roots fails closed.
    - D1: suspend `ask_user_decision` without `allowCustom`; stored data has
      `allowCustom === false`; resume with `selectedId` succeeds.

- [x] Add compaction strategies, observational-memory attach, and compact RPC
  - Acceptance Criteria:
    - Functional: Daemon can compact with root
      `createDefaultCompactionStrategy`,
      `@arnilo/prism-memory/compaction/llm` (`createCodingCompactionStrategy`
      or `createLlmCompactionStrategy`), and
      `@arnilo/prism-memory/compaction/observational-memory`.
      `session.compact` RPC runs manual compact. Per-run override selects
      strategy. Auto-compact uses configured threshold. OM `attach()`
      against the daemon SQLite store; recall tool available when OM is
      attached. `session.compact()` while a run is active fails closed
      (Prism behavior; do not swallow).
    - Performance: Compaction is explicit/threshold, not per-token on the
      hot path. OM worker limits stay at package defaults; token threshold
      default **80000** (`compactAfterTokens`), per session (2158).
    - Code Quality: Import only `/compaction/llm` and
      `/compaction/observational-memory`. Do not import `/wiki`, `/graft`,
      `/rag`, or root working/vector memory. Do not initialize PostgreSQL.
    - Security: Compaction/OM get the host redactor + known secrets.
      Recall is exact-id only. Search still does not auto-inject OM text
      into the current run.
  - Approach:
    - Documentation Reviewed:
      - Prism `docs/compaction-and-retry.md`
      - Prism `docs/compaction-llm.md`
      - Prism `docs/compaction-observational-memory.md`
      - `decision-logs/2026-08-30-2158-observational-memory-defaults-worker-models-80k-per-session.md`
    - Options Considered:
      - LLM compaction as the host default: rejected. Root default stays
        local; LLM/OM are selectable.
      - Attach OM to Chat always: rejected. Attach when the profile/session
        opts in (coding tests + later packages).
      - Three named strategies + RPC override: selected.
    - Chosen Approach:
      - Register strategies on the kernel. `session.compact` /
        `RunOptions.compaction` pick by name. OM `attach` on coding
        sessions; worker models from host config, not the session model.
    - API Notes and Examples:
      ```ts
      import { createDefaultCompactionStrategy } from "@arnilo/prism";
      import { createCodingCompactionStrategy } from "@arnilo/prism-memory/compaction/llm";
      import {
        createObservationalMemory,
        createObservationalMemoryCompactionStrategy,
        createRecallMemoryTool,
      } from "@arnilo/prism-memory/compaction/observational-memory";

      await session.compact({ strategy: createDefaultCompactionStrategy() });
      const om = createObservationalMemory({ /* worker models, compactAfterTokens: 80_000 */ });
      await om.attach({ sessionId, appendEntry, getEntries });
      ```
    - Files to Create/Edit:
      - `clay-agent/src/compaction.ts`: strategy registry
      - `clay-agent/src/host.ts`: `session.compact`; run override; OM attach
      - `src/server/agent.rs`: forward compact RPC
      - `src/protocol/agent.rs`: `AgentClientCommand` compact variant
      - `clay-agent/src/__tests__/compaction.test.ts`
    - References:
      - `roadmap.md` Phase 1 compaction bullet + exit gate OM attach
      - `.agents/skills/project-patterns/references/agent-host.md`
  - Test Cases to Write:
    - Manual compact on a mock session appends a compaction entry; raw
      entries remain.
    - Active-run compact fails closed.
    - Strategy override `llm` with mock summary provider writes an llm
      compaction entry (no live network).
    - OM attach records an observation after a turn; `createRecallMemoryTool`
      round-trips a known id; invalid id fails closed.
    - Chat session without OM attach has no recall tool.

- [x] Add skills registry plumbing, command dispatch, and host CommandDrivers
  - Acceptance Criteria:
    - Functional: Daemon accepts inert `Skill` / `CommandDefinition` data
      over RPC, stores them on the kernel, and progressive-discloses skill
      bodies via Prism `load_skill`. `command.dispatch` runs a registered
      `/verb`. Host injects `CommandDrivers` `{ startRun, startWorkflow,
      steer }` (`startWorkflow` may return a typed “not in this phase”
      error). Absent drivers omit the key. Packages never supply drivers.
    - Performance: Registry ops are in-process maps. Dispatch uses existing
      RPC timeout.
    - Code Quality: Skills are text. Commands are data + host-side execute
      in the daemon, not Deno package closures. Public package JS
      `agent.registerCommand` waits for Phase 4; this phase is daemon RPC
      the Rust server can call.
    - Security: Unknown skill/command names fail closed. Duplicate skill
      names use `{ duplicate: "error" }`. Drivers cannot be passed in from
      RPC params. No new filesystem/network grant from registering a skill.
  - Approach:
    - Documentation Reviewed:
      - Prism `docs/context-and-skills.md` (`createSkillRegistry`,
        `resolveActiveSkills`, `load_skill`)
      - Prism `docs/extension-authoring.md` CommandDrivers
      - `decision-logs/2026-07-21-0001-two-package-runtime-trust-domains.md`
      - Task 2 evidence map
    - Options Considered:
      - Public Clay JS package API now: rejected (Phase 4).
      - Daemon RPC + kernel registries: selected.
      - Skip `startWorkflow` driver key: rejected. Inject the key; fail
        closed with a clear error until Phase 5 so command authors see the
        same shape.
    - Chosen Approach:
      - `skill.register` / `command.register` / `command.dispatch` JSON-RPC.
        Server will later feed package contribution data; tests register
        directly. `runRpcServer` is not adopted (Clay already has stdio
        JSON-RPC); copy the driver injection pattern onto
        `CommandExecutionContext`.
    - API Notes and Examples:
      ```ts
      import { createSkillRegistry, resolveActiveSkills } from "@arnilo/prism";
      const registry = createSkillRegistry(skills, { duplicate: "error" });
      const active = resolveActiveSkills({ registry, names: ["brief"], tools: activeTools });

      // Host-only drivers (never from package RPC params):
      const drivers = {
        startRun: (input, options) => session.run(input, options),
        startWorkflow: async () => {
          throw rpcError(-32000, "workflows not enabled");
        },
        steer: (runId, input) => session.steer(runId, input),
      };
      ```
    - Files to Create/Edit:
      - `clay-agent/src/host.ts`: skill/command RPC + drivers
      - `src/server/agent.rs`: forward register/dispatch
      - `src/protocol/agent.rs`: dispatch command if user-facing
      - `clay-agent/src/__tests__/skills-commands.test.ts`
    - References:
      - `roadmap.md` Phase 1 skills + commands bullets
      - `.agents/skills/project-patterns/references/product-surfaces-are-packages.md`
  - Test Cases to Write:
    - Register skill text; catalog shows name+description only until
      `load_skill`; then instructions appear.
    - Skill referencing an inactive tool throws before a provider turn.
    - Duplicate skill name with `duplicate: "error"` throws.
    - Dispatch `/steer`-like command calls host `drivers.steer`.
    - Dispatch a command that wants `startWorkflow` gets the Phase 5 error,
      not a thrown missing-key crash.
    - RPC cannot inject `drivers`.

- [x] Add durable run lifecycle, interruptBeforeTool, and approval resume RPC
  - Acceptance Criteria:
    - Functional: Coding runs set `runState: { checkpoints, definitionRevision,
      interruptBeforeTool: true }` on a host-owned checkpoint store (SQLite
      persistence). Suspended runs emit redacted `pendingDecisions` and
      persist. `run.resume` accepts Prism’s `decision` or `decisions` batch
      with `expectedVersion` CAS. Chat runs stay non-durable (no tool
      interrupt).
    - Performance: Checkpoints bounded by Prism `maxStateBytes` (default
      256 KB). Resume uses existing RPC timeout. No extra listener.
    - Code Quality: Use `@arnilo/prism` `resumeAgentRun` /
      `createAgentRunLifecycle` and SQLite persistence. Do **not** import
      `@arnilo/prism-core/runtime/server` `createPrismHandler` or bind HTTP.
      Import `@arnilo/prism-core/sessions/codecs` only if resume/search
      codecs are needed.
    - Security: Resume validates before checkpoint I/O (Prism 0.2.0
      no-side-effect guarantee). Pending decision payloads never include
      raw tool arguments. Ownership/version/fingerprint mismatch fails
      closed. Crash after dispatch is not auto-replayed.
  - Approach:
    - Documentation Reviewed:
      - Prism `docs/agent-session-runtime.md` Durable interruption
      - Prism `docs/server.md` — read to **reject** HTTP handler
      - `@arnilo/prism-core/sessions/codecs` ownership/search helpers
    - Options Considered:
      - Prism HTTP server as the Clay agent bus: rejected (stdio JSON-RPC
        already exists; no localhost listener).
      - Durable interrupt for Chat: rejected (no tools).
      - Host checkpoint store + `interruptBeforeTool` on coding runs:
        selected.
    - Chosen Approach:
      - Wire `runState` on coding `session.prompt`. Add `run.resume` RPC
        mapping to `resumeAgentRun`. Surface permission events already in
        `AgentWireEvent::Permission`.
    - API Notes and Examples:
      ```ts
      import { resumeAgentRun } from "@arnilo/prism";
      const result = await session.run(input, {
        runState: { checkpoints, definitionRevision: "clay-agent.1", interruptBeforeTool: true },
      });
      if (result.status === "suspended") {
        await resumeAgentRun(agent, { runId: result.runId, sessionId: result.sessionId }, {
          decision: "approve",
          expectedVersion: result.runState!.version!,
        }, { checkpoints, definitionRevision: "clay-agent.1" });
      }
      ```
    - Files to Create/Edit:
      - `clay-agent/src/host.ts`: runState on prompt; `run.resume`
      - `src/server/agent.rs`: forward resume
      - `src/protocol/agent.rs`: resume/approval client command
      - `clay-agent/src/__tests__/durable-run.test.ts`
    - References:
      - `roadmap.md` Phase 1 durable run bullet
      - `.agents/skills/project-patterns/references/agent-host.md`
  - Test Cases to Write:
    - Coding mock tool gated by policy suspends with `pendingDecisions`;
      resume approve executes once; expectedVersion stale fails with no
      side effect.
    - Chat prompt has no `interruptBeforeTool`.
    - Malformed resume (`decision: "sideways"`) fails closed, no checkpoint
      write.

- [x] Add workspace-scoped session search, session tree, and document checkpoints
  - Acceptance Criteria:
    - Functional: Every session record stamps a stable workspace identity
      (`metadata.workspaceRoot` / `SESSION_SEARCH_WORKSPACE_METADATA_KEY`).
      `session.search` wraps Prism `searchSessions` and returns only the
      current workspace: session, branch/leaf, snippet, timestamp, status.
      Hits are transcript data, never auto-injected into agent context.
      Raw tool output is not searchable by default. `session.checkout` /
      `fork` / `clone` match Prism/pi. Server records a document version
      checkpoint at task-boundary and user-checkpoint RPCs, keyed by
      `(sessionId, entryId)`. Checkout of entry E restores those document
      versions together with the conversation leaf. Abandoned sides are
      kept, not deleted. `st` git-layer checkpoints wait for Phase 5.
    - Performance: Reuse `prism_session_search_fts`. Page caps stay Prism
      defaults (20/100). Search is not on the typing path.
    - Code Quality: Prefer `persistence.searchSessions` +
      `@arnilo/prism-core/sessions/codecs` (`entrySearchFields`,
      `clipSearchSnippet`) over a second SQLite file. Filter tool-result
      hits in the host if Prism dual-write indexed them.
    - Security: Cross-workspace search returns empty, not an error that
      enumerates other workspaces. Snippets redacted. Checkpoint restore
      requires current leases / server authority; no silent disk rewrite
      of dirty foreign leases.
  - Approach:
    - Documentation Reviewed:
      - Prism `docs/session-stores.md` Session search + checkout/fork/clone
      - Prism `docs/agent-session-runtime.md` (`session.checkout`/`fork`/`clone`)
      - `decision-logs/2026-08-30-2201-workspace-scoped-session-search-shared-by-clay-and-st.md`
      - `decision-logs/2026-08-30-2200-pi-tree-model-extended-to-workspace-checkpoints-and-discard.md`
      - `@arnilo/prism-core/sessions/sqlite/ddl.ts` `MIGRATION_004_SESSION_SEARCH`
    - Options Considered:
      - Second Clay FTS database: rejected. Prism FTS already exists.
      - Cross-workspace search: rejected (2201).
      - Index tool output: rejected by default.
      - Conversation-only branch (pi as-is): rejected (2200).
      - Stamp workspace id + reuse `searchSessions` + server checkpoint
        map: selected.
    - Chosen Approach:
      - Pass workspace identity on `session.new`. Search RPC always
        includes that identity. Checkpoint table on the Rust server (document
        authority). Daemon checkout calls server to restore versions, then
        `session.checkout(leafId)`.
    - API Notes and Examples:
      ```ts
      import { SESSION_SEARCH_WORKSPACE_METADATA_KEY } from "@arnilo/prism";
      await persistence.appendSession({
        id, tenantId, agentDefinitionId, createdAt, updatedAt,
        metadata: { profile, provider, model, [SESSION_SEARCH_WORKSPACE_METADATA_KEY]: workspaceId },
      });
      const page = await persistence.searchSessions!({
        workspaceRoot: workspaceId,
        query,
        limit: 20,
      });
      await session.checkout(leafId);
      const forked = session.fork();
      const cloned = await session.clone({ id: newId });
      ```
    - Files to Create/Edit:
      - `clay-agent/src/host.ts`: workspace metadata; `session.search` /
        `checkout` / `fork` / `clone` / `checkpoint`
      - `src/server/agent.rs`: pass workspace id; checkpoint restore
      - `src/server/agent_checkpoints.rs` (new, tentative): version map
      - `src/protocol/agent.rs`: search/tree/checkpoint commands
      - `clay-agent/src/__tests__/session-search-tree.test.ts`
      - `tests/agent_protocol.rs` or a focused Rust test for checkpoint
        restore
    - References:
      - `roadmap.md` Phase 1 session index + tree bullets
      - `.agents/skills/project-patterns/references/agent-host.md`
  - Test Cases to Write:
    - Two workspaces, identical fixture text: search from A never returns B.
    - Tool-result-only text does not appear as a hit.
    - Search hit includes sessionId + leafId; calling checkout moves the
      leaf; it does not append hit text into the current run.
    - Fork/clone produce independent session ids/entries.
    - Checkpoint at entry E, dirty edit, checkout E: documents return to
      E’s versions; later branch still listable.

- [x] Wire allow-listed MCP and host-owned Obscura (fail-closed)
  - Acceptance Criteria:
    - Functional: Daemon connects MCP only for server-supplied allow-list
      entries (fixed `serverId`, canonical executable, literal argv, explicit
      env names, known roots). Tools become `ToolDefinition`s. Empty
      allow-list → no MCP tools. Obscura: resolve host binary; if absent,
      hide web/browser/Obscura tools. If present, daemon owns
      `spawnObscuraProcess` lifecycle (waitReady, group close), exposes
      `createObscuraMcpTools` (complete advertised surface),
      `connectObscuraCdp`, and Playwright `connectOverCDP` composition.
      Exact `playwright-core@1.61.0` added in this task. Skip
      `/brave` `/exa` `/firecrawl`.
    - Performance: No spawn on daemon import or Chat initialize. Spawn on
      first coding session that needs the capability, or explicit enable.
      Close on shutdown. Bounded stderr.
    - Code Quality: Import `@arnilo/prism-mcp`, `@arnilo/prism-web-tools`,
      `/obscura`, `/browser` only. No ACP. Isolation: installing families
      does not start Postgres, browsers, or extra adapters until configured.
    - Security: Matches task 2 map. Insecure Obscura flags rejected unless
      explicit. Loopback-only CDP unless `allowRemoteEndpoint`. Untrusted
      web content labeled. Package JS cannot pass a runtime-selected
      executable. Missing binary ≠ error on Chat. Revocation/shutdown kills
      owned processes. Do not call Obscura/MCP “sandboxed”.
  - Approach:
    - Documentation Reviewed:
      - Prism `docs/mcp-tools.md` (`connectMcpTools`)
      - Prism `docs/obscura.md` (`spawnObscuraProcess`,
        `createObscuraMcpTools`, `connectObscuraCdp`)
      - `decision-logs/2026-08-30-2159-obscura-web-browser-stack-and-computer-use-linux.md`
      - Task 2 evidence map
      - `@arnilo/prism-web-tools` `peerDependencies.playwright-core` `1.61.0`
    - Options Considered:
      - Always-on web tools: rejected (2159 hide when absent).
      - Direct Brave/Exa/Firecrawl providers: rejected.
      - Pin `playwright-core` in task 1: rejected; add here with CDP.
      - Host-owned Obscura + allow-listed MCP: selected.
    - Chosen Approach:
      - Server sends allow-list; daemon validates and `connectMcpTools`.
        Obscura binary from host path (PATH or documented absolute default);
        fail closed. `playwright-core` exact pin for CDP composition only.
    - API Notes and Examples:
      ```ts
      import { connectMcpTools } from "@arnilo/prism-mcp";
      import { spawnObscuraProcess, createObscuraMcpTools, connectObscuraCdp } from "@arnilo/prism-web-tools/obscura";
      import { createBrowserTools } from "@arnilo/prism-web-tools/browser";
      import { createWebTools } from "@arnilo/prism-web-tools";

      const bridge = await connectMcpTools({
        serverId: "allowlisted",
        transport: { type: "stdio", command: canonicalBin, args: literalArgv },
      });
      const obscura = await createObscuraMcpTools({
        transport: { type: "stdio", command: obscuraBin, args: ["mcp"] },
      });
      const session = await connectObscuraCdp({
        command: obscuraBin,
        args: ["serve", "--host", "127.0.0.1", "--port", "9222"],
      });
      const browserTools = createBrowserTools({ browser: session.browser, networkPolicy });
      ```
    - Files to Create/Edit:
      - `clay-agent/package.json`: `"playwright-core": "1.61.0"`
      - `clay-agent/src/mcp.ts`: allow-list connect/close
      - `clay-agent/src/obscura.ts`: resolve/spawn/hide
      - `clay-agent/src/host.ts`: register tools when present
      - `clay-agent/src/main.ts`: shutdown closes children
      - `src/server/agent.rs`: pass allow-list; never package argv
      - `clay-agent/README.md`: Obscura/MCP honesty
      - `clay-agent/src/__tests__/mcp-obscura.test.ts`
      - Tentative: package contribution schema later consumed in Phase 4;
        this phase accepts server-built allow-list entries
    - References:
      - `roadmap.md` Phase 1 MCP + Obscura bullets
      - `.agents/skills/project-patterns/references/authority-boundaries.md`
  - Test Cases to Write:
    - Empty allow-list: no MCP tools; Chat unchanged.
    - Allow-list entry with non-canonical command rejected.
    - Missing Obscura binary: no `obscura_*` / browser tools; initialize
      still `{ ok: true, prism: "0.4.0" }`.
    - Present stub binary: spawn + close on shutdown (group kill); tools
      registered with `obscura_` prefix.
    - Source has no `@arnilo/prism-web-tools/brave` (or exa/firecrawl)
      imports.
    - Cargo.toml still has no `@modelcontextprotocol`.

- [x] Verify Linux gates, Chat/mock byte-compatibility, and Phase 1 exit gate
  - Acceptance Criteria:
    - Functional: `cd clay-agent && npm ci && npm run build && npm test`
      pass. Coding fixture: nine tools against an open document, dirty
      buffer, approval, persisted history, D1 resume. Compaction manual +
      threshold. OM attach against daemon store. Chat mock prompt/resume/
      cancel still pass.
    - Performance: `cargo fmt --check`, `cargo check --all-targets`,
      `cargo clippy --all-targets -- -D warnings` pass on Linux.
      `cargo test --test agent_protocol` pass.
    - Code Quality: Chat events still have no tool/permission payloads in
      the mock Chat path. README Chat honesty sentence remains.
    - Security: Deny test still blocks ACP/AG-UI/retired names. MCP only
      in clay-agent graph.
  - Approach:
    - Documentation Reviewed:
      - `AGENTS.md` Linux gates
      - `roadmap.md` Phase 1 Exit Gate
      - `clay-agent/README.md`
    - Options Considered:
      - Weaken Chat tests to allow tools: rejected.
      - Run listed suites on Linux: selected.
    - Chosen Approach:
      - Execute the commands. Fix only regressions found. Record coding
        fixture command in evidence.
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
      - `.agents/skills/project-patterns/references/maintenance-validation.md`
  - Test Cases to Write:
    - None new. This task runs the suites already listed.

- [x] Create or verify Clay JS APIs for public programmatic surfaces
  - Acceptance Criteria:
    - Functional: Inventory every new/changed Rust `pub` function. Expose
      user-facing host controls through `clay:agent` facades with stable
      IDs under reserved domain `agent` (already in
      `RESERVED_CORE_API_DOMAINS`): compact, compaction strategy, full
      autonomy, session search, run resume/approval, checkout/fork/clone,
      checkpoint. Daemon JSON-RPC stays internal. Package
      registerSkill/registerCommand/registerMcp wait for Phase 4 unless a
      `pub` server function would otherwise leak — then `pub(crate)`.
    - Performance: Facades are server-first and off the editor hot path.
    - Code Quality: Naming per `clay-js-api-naming.md` (no `clay.agent.*`).
      Markdown docs + `docs/index.md` + registry. `cargo test` doc-registry
      suite green.
    - Security: APIs do not grant filesystem/network/shell by existing.
      Search results are not implicit context. Autonomy default false.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/create-plan/references/clay.md` Clay JS API Task
      - `.agents/skills/project-patterns/references/clay-js-api-naming.md`
      - `.agents/skills/project-patterns/references/clay-js-api-boundary.md`
      - `.agents/skills/project-patterns/references/clay-js-api-schema.md`
      - `.agents/skills/project-patterns/references/documentation-as-code.md`
      - `.agents/skills/project-patterns/references/doc-registry-tests.md`
      - `examples/init.js` section 12 (currently no `agent*` export)
    - Options Considered:
      - Keep all new RPCs private and skip facades: rejected if `pub`
        server methods exist; clay.md requires JS APIs or `pub(crate)`.
      - Full package authoring API: rejected (Phase 4).
      - Thin `clay:agent` user/host facades for shipped RPCs: selected.
    - Chosen Approach:
      - After implementation, inventory `src/server/agent.rs` and protocol
        commands. Add facades + Markdown only for intended public behavior.
        Internal reverse-RPC stays `pub(crate)`.
    - API Notes and Examples:
      ```ts
      import { compact, searchSessions, setFullAutonomy, resumeRun } from "clay:agent";
      await setFullAutonomy({ enabled: false });
      await compact({ sessionId, strategy: "default" });
      const hits = await searchSessions({ query: "flake" });
      await resumeRun({ sessionId, runId, decision: "approve", expectedVersion });
      ```
    - Files to Create/Edit:
      - `runtime/js/agent.js` (tentative): facade
      - `src/server/js_runtime/` ops (tentative)
      - `docs/reference/clay-js-api/agent/*.md`
      - `docs/index.md`
      - generated registry via `cargo run --bin update-doc-registry`
      - `src/packages/manifest.rs`: only if a new core domain were needed
        (`agent` already reserved)
    - References:
      - `decision-logs/2026-05-08-1509-clay-js-api-facade-for-rust-functions.md`
  - Test Cases to Write:
    - Registry/docs tests fail if a new public API lacks Markdown/index/
      custom_properties/key_bindings.
    - No `clay.agent.*` IDs.

- [x] Create or verify Clay configuration APIs
  - Acceptance Criteria:
    - Functional: Full autonomy default, compaction strategy name, and OM
      `compactAfterTokens` (default 80000) are documented Clay JS API
      custom properties, not hidden keys. `~/.config/clay/init.js` remains
      the entry. Configuration does not put credentials in init.js.
    - Performance: Config read at runtime generation, not per keystroke.
    - Code Quality: Each option is on a Clay JS API. Defaults match 2157
      and 2158.
    - Security: Setting strategy/autonomy does not grant MCP/Obscura/
      filesystem. Obscura binary path is host-resolved, not a free
      init.js executable string (0714).
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/create-plan/references/clay.md` Configuration Task
      - `.agents/skills/project-patterns/references/configuration-system.md`
      - `decision-logs/2026-08-30-2157-default-acceptance-workspace-free-host-write-gated.md`
      - `decision-logs/2026-08-30-2158-observational-memory-defaults-worker-models-80k-per-session.md`
    - Options Considered:
      - Undocumented daemon env vars: rejected.
      - Custom properties on `clay:agent` APIs: selected.
    - Chosen Approach:
      - Attach custom_properties to the facades from the previous task.
        No credential or binary-path options in init.js.
    - API Notes and Examples:
      ```ts
      import { setFullAutonomy, setCompactionStrategy } from "clay:agent";
      await setCompactionStrategy({ name: "default", compactAfterTokens: 80_000 });
      await setFullAutonomy({ enabled: false });
      ```
    - Files to Create/Edit:
      - Same Markdown/facade files as the JS API task
      - `docs/index.md` if new pages
    - References:
      - `decision-logs/2026-05-08-1841-configuration-through-init-js-and-clay-js-apis.md`
  - Test Cases to Write:
    - Coverage gate fails if autonomy/strategy/threshold lack
      `custom_properties`.
    - Default autonomy is false; default compactAfterTokens is 80000.
- Task 12 complete (2026-09-02). Configuration options are documented Clay
  JS API custom properties on the `clay:agent` facades (no hidden keys,
  init.js untouched, credentials vault-only).
  - `agent.setFullAutonomy`: custom property `default:boolean=false`
    (decision 2157) documents the autonomy default; docs state there is no
    settable "default autonomy" init.js key — autonomy is session state
    changed only by the host-side facade.
  - `agent.compact`: custom property `compactAfterTokens:number=80000`
    (decision 2158; Prism package default is 81000). The strategy name is
    the documented `strategy` option of `agent.compact` (`default`, `llm`,
    `om`) — deliberately no separate strategy-configuration API.
  - Made the threshold real, not just documented: `session.compact` accepts
    a positive-finite `compactAfterTokens`, validated fail-closed
    (-32602); the daemon stores it per session and feeds it to the OM
    runtime through a `SettingsProvider` (`{"context":{"compactAfterTokens":N}}`),
    so post-run auto-compaction honors the override, not only the manual
    call. Non-OM sessions ignore it. `op_clay_agent_compact_session`
    forwards a positive-integer override.
  - Coverage gate:
    `agent_configuration_options_are_documented_custom_properties_with_decision_defaults`
    in `tests/clay_js_api_inventory.rs` asserts the two custom properties
    with decision defaults, the docs carry `compactAfterTokens`, no
    separate strategy API exists, and the daemon source keeps
    `DEFAULT_COMPACT_AFTER_TOKENS = 80_000`.
  - Registry regenerated; compact/set-full-autonomy doc pages updated
    (frontmatter, Options, Custom properties, Lookup metadata); README +
    wiki updated. Security unchanged: strategy/autonomy/threshold grant no
    MCP/Obscura/filesystem authority; Obscura binary stays host-resolved
    (0714).
  - Tests: daemon 49/49 (new validation + override test); clay_js_api
    inventory suite 14/14; protocol 202/202.

- [x] Update the canonical example configuration (`examples/init.js`)
  - Acceptance Criteria:
    - Functional: Section 12 documents new `clay:agent` APIs (commented
      safe examples). Active uncommented part stays safe to copy. No
      secrets. `node --check examples/init.js` passes.
    - Performance: No extra runtime.
    - Code Quality: Option names/enums/defaults match docs and inventory.
    - Security: Examples do not enable full autonomy uncommented, do not
      paste API keys, do not set a free Obscura/MCP executable.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/create-plan/references/clay.md` Example
        Configuration Maintenance Task
      - `examples/init.js` section 12
      - JS API Markdown from the previous tasks
    - Options Considered:
      - Leave “no agent* export” sentence: rejected if facades shipped.
      - Commented examples + keep credentials out: selected.
    - Chosen Approach:
      - Replace the “deliberately no agent* export” claim with documented
        host controls; keep provider credentials vault-only.
    - API Notes and Examples:
      ```js
      // import { setFullAutonomy, setCompactionStrategy } from "clay:agent";
      // await setFullAutonomy({ enabled: false });
      // await setCompactionStrategy({ name: "default", compactAfterTokens: 80_000 });
      ```
    - Files to Create/Edit:
      - `examples/init.js`: section 12
    - References:
      - user instruction 2026-08-03 (canonical `examples/init.js`)
  - Test Cases to Write:
    - `node --check examples/init.js`
    - Cross-check option names against API docs.
- Task 13 complete (2026-09-02). Section 12 of `examples/init.js` now
  documents the shipped `clay:agent` host controls.
  - Removed the stale "deliberately no `agent*`/`chat*`/`provider*` export"
    claim; replaced with a summary of the trusted-only facade and its five
    exports (`agent.compact`, `agent.searchSessions`, `agent.setFullAutonomy`,
    `agent.resumeRun`, `agent.sessionTree`) — names/enums/defaults match the
    task-11/12 doc pages (`strategy`: default/llm/om; autonomy default
    false, 2157; `compactAfterTokens` default 80000, 2158).
  - Commented safe examples only (the active/uncommented part is untouched,
    still copy-safe): `setFullAutonomy({ enabled: false })`, manual compaction
    incl. the OM threshold override, and a workspace-scoped
    `searchSessions` query labeled metadata-only.
  - Security notes restated: credentials vault-only (no API keys in init.js),
    MCP servers server-allow-listed (`data_dir/mcp.json`), Obscura binary
    host-resolved (`CLAY_OBSCURA_BIN`/PATH), never config strings; no
    uncommented autonomy enablement.
  - `node --check examples/init.js` passes; canonical-example doc-registry
    tests (Ctrl+B marker, import/coverage pins) still green; full protocol
    suite 203/203.

- [x] Execute and update the manual test plan (`test-plan/`)
  - Acceptance Criteria:
    - Functional: Add numbered steps for new user-visible configuration
      (autonomy, compaction) under `test-plan/02-configuration-init-js.md`
      (or a new agent module linked from `test-plan/index.md`). Coding-tool
      dirty-buffer / approval / search isolation covered by automated
      suites; record that Chat UI chrome is unchanged. Do not weaken
      existing Chat steps.
    - Performance: No new manual latency claim without a measurement step.
    - Code Quality: Coverage matrix updated if a module is added.
    - Security: Steps include: autonomy off by default; missing Obscura
      hides tools; search does not inject context.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/create-plan/references/clay.md` Manual Test Plan Task
      - `test-plan/index.md`
      - `test-plan/02-configuration-init-js.md`
    - Options Considered:
      - Automated-only like Phase 0: rejected if init.js agent APIs ship.
      - Full pi TUI manual pass: rejected (Phase 2 UI).
      - Config module steps + automated host evidence: selected.
    - Chosen Approach:
      - Update `test-plan/` for configuration. Point host exit-gate drills
        at `clay-agent` tests. Run new config steps on a Linux build if
        the facades are callable; otherwise record the blocker.
    - API Notes and Examples:
      ```text
      test-plan/index.md
      test-plan/02-configuration-init-js.md
      ```
    - Files to Create/Edit:
      - `test-plan/02-configuration-init-js.md` and/or `test-plan/16-agent-host.md`
      - `test-plan/index.md` if a module is added
    - References:
      - `test-plan/11-performance.md` existing clay-agent note
  - Test Cases to Write:
    - Manual steps as added. Automated host tests already in prior tasks.
- Task 14 complete (2026-09-02). New module `test-plan/16-agent-host.md`
  (steps A1–A19) + index wiring + parity-ledger row.
  - Module 16 covers: config/docs cross-check of init.js section 12 (A1–A4);
    autonomy default-off + per-session enable (A5–A7, decision 2157);
    compaction strategies + OM `compactAfterTokens` override (A8–A10,
    decision 2158); OM recall never auto-injects (A11); workspace-scoped
    search metadata-only, tool output not indexed (A12–A13); session tree
    checkout/fork/clone with buffer-only checkpoint restore (A14–A15); MCP
    empty/non-canonical allow-list fail-closed + missing Obscura hidden,
    not an error (A16–A19). Each step cites its automated suite; Chat UI
    chrome unchanged recorded in A1; no latency claim added (no module-11
    entry needed).
  - Coding-tool dirty-buffer/approval/search isolation stays automated
    (clay-agent suites 49/49); live GUI steps are the standing procedure —
    recorded NOT RUN under the documented host input ceiling, consistent
    with Plans 097/099/105 records. No existing step weakened.
  - `test-plan/index.md`: module-map row 16, coverage-matrix row (agent
    config surfaces → 16 + 02 C24), dated Plan 107 execution record.
  - Ledger (`docs/development/tauri-react-parity-ledger.json`): new
    `agent.host.phase1` row (phase 2, five agent.* public APIs, A1–A19
    manual steps, AgentRpc/Snapshot/Event/Diagnostic agent messages);
    removed the five APIs accidentally duplicated on
    `launch.connection.lifecycle`; added the row to
    `tauri-react-primitive-migration.md`. `documentation_coverage` 11/11.
  - Tests: protocol 203/203 (incl. coverage gate), fmt + clippy clean.

- [x] Update or verify the code wiki after implementation
  - Acceptance Criteria:
    - Functional: `docs/wiki/modules/clay-agent.md` and index blurb describe
      Phase 1 families, coding tools + document reverse-RPC, acceptance
      policy, compaction/OM, skills/commands/drivers, durable runs, session
      search/tree/checkpoints, MCP allow-list, Obscura fail-closed, and
      Chat still no-tools. Master index links any new pages.
    - Performance: Wiki adds no runtime work. Note FTS is Prism schema v4
      plus Clay indexing policy, not a second DB.
    - Code Quality: Pages explain how it works, invariants, tests, and
      phase boundaries (2/4/5/6).
    - Security: Documents no ACP/AG-UI bus, no package spawn, no
      `process.env` secrets, truthful same-user subprocess language for
      Obscura/MCP, workspace vs host write gate.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/project-wiki/SKILL.md`
      - `.agents/skills/create-plan/references/wiki-task.md`
      - `docs/wiki/modules/clay-agent.md`
      - `docs/wiki/index.md`
    - Options Considered:
      - Update after each task: noisy.
      - Update once after tests pass: selected.
    - Chosen Approach:
      - After verification, rewrite live host facts. Keep Plan 096
        phase25 pages historical unless they claim the live pin set.
    - API Notes and Examples:
      ```text
      docs/wiki/index.md
      docs/wiki/modules/clay-agent.md
      ```
    - Files to Create/Edit:
      - `docs/wiki/modules/clay-agent.md`
      - `docs/wiki/index.md`
      - Tentative: `docs/wiki/modules/clay-agent-coding-tools.md` only if
        `clay-agent.md` would exceed a readable page
    - References:
      - `.agents/skills/project-wiki/SKILL.md`
  - Test Cases to Write:
    - Manual wiki review: index link + page matches `package.json` imports
      and host RPC list.
- Task 15 complete (2026-09-02). `docs/wiki/modules/clay-agent.md` verified
  against `package.json` and the live host RPC list; gaps closed:
  - New "Coding tools and document reverse-RPC (Phase 1)" section: nine tool
    factories + `ask_user_decision`, document.read/write/edit/stat reverse
    RPC, server-side lease/CAS via `apply_edit` (client 0 identity),
    user-lease fail-closed, acceptance policy (2157) + autonomy default.
  - Dependencies list corrected to the real seven-family exact pin set +
    `playwright-core@1.61.0`; dropped the stale "prism-memory unmet" note
    (memory is now imported for compaction/OM).
  - Security section added: no ACP/AG-UI bus, no package spawn, no
    `process.env` secrets, truthful same-user subprocess language for
    Obscura/MCP, workspace-vs-host write gate, redacted search metadata.
  - FTS noted as Prism SQLite schema v4 + Clay indexing policy (no second
    DB). Phase boundaries stated (2 UI, 4 contribution feeding, 5 workflows,
    6 supervisors). Tests section now enumerates all nine daemon suites
    (49 tests) plus the Rust-side gates. Chat-no-tools recorded.
  - Related links: authoritative `docs/reference/clay-js-api/agent/` docs
    (not duplicated) and `test-plan/16-agent-host.md`.
  - `docs/wiki/index.md` blurb rewritten: full Phase 1 family list, no
    stale "no MCP" claim, Chat tool-free/Phase 2 UI noted.
  - Verification: protocol 203/203 (incl. doc-coverage + primitives-docs
    gates), fmt clean. Wiki adds no runtime work.

## Compromises Made

- Known before execution: Phase 1 does not ship `@clay/coding-agent` UI,
  wiki/graft, workflows, supervisors, Antigravity, or computer-use-linux.
- Known: reuse Prism `searchSessions` FTS instead of a second database;
  Clay owns workspace stamping and tool-output exclusion policy.
- Known: do not adopt `@arnilo/prism-core/runtime/server` HTTP
  `createPrismHandler`; Clay keeps stdio JSON-RPC.
- Known: Context7 cannot document `@arnilo/prism`; local Prism 0.4 docs
  are used. `playwright-core@1.61.0` comes from the web-tools peer pin.

## Further Actions

- Task 11 complete (2026-09-02). Clay JS APIs for the shipped agent host
  controls. Inventory of new/changed server surfaces: daemon RPCs
  (`session.setAutonomy` new; `session.compact`, `session.search`,
  `run.resume`, `session.{checkout,fork,clone,checkpoint}` shipped in tasks
  5-8), Rust protocol (`AgentClientCommand::{SetAutonomy, SearchSessions,
  RunResume}` — decision carried as a JSON string for the rkyv wire;
  `AgentServerMessage::AgentRpc` result projection; AG-UI adapter extended),
  and five new ops in `src/server/ops/agent.rs`.
  - Facades: new `runtime/js/agent.js` + `agent.d.ts` (trusted-only),
    registered in `src/server/facades.rs` and `runtime/js/mod.ts`. Exports:
    `compact`, `searchSessions`, `setFullAutonomy`, `resumeRun`,
    `sessionTree` — stable IDs `agent.*` under the reserved `agent` domain;
    no `clay.agent.*`; naming per clay-js-api-naming.md.
  - Authority wiring: process-global `AgentHostHandle` (one clay-agent child
    per server) installed in `IpcServer::try_new`; ops fail closed with
    `agent.unavailable` when unset. Package (third-party) extension has
    none of the five ops; `clay:agent` is trusted-only — updated
    `package_runtime_cannot_import_a_daemon_handle` to assert the real
    boundary (package op-set absence + facade trusted-only) instead of the
    file-level name absence the Phase-1 intermediate guard used.
  - Docs: 5 pages under `docs/reference/clay-js-api/agent/` with full
    schema frontmatter, `api-inventory.toml` entries, `docs/index.md`
    links, generated registry via `cargo run --bin update-doc-registry`,
    Plan 061 rebaseline tables (ops 86→91 trusted-only 34→39; facades
    23→24 admin-only 6→7), runtime/js/README.md, parity ledger rows.
  - Daemon: `session.setAutonomy` RPC validates `enabled` is boolean and
    the session is live; autonomy default false, host-set only.
  - Tests: facade parity/allowlist suites green; new JS-runtime test
    proves the facade fails closed without a host; agent_protocol wire
    roundtrip covers the new command/message variants; daemon 48/48 (new
    setAutonomy toggle/validation test); full Rust suite 1171 lib + 202
    protocol + 134 presentation + 40 security + 73 runtime; fmt + clippy
    clean.
  - Skipped (per plan): package registerSkill/registerCommand/registerMcp
    (Phase 4); compaction-strategy facade folded into `compact({strategy})`
    — a separate strategy-registry API would expose kernel internals with
    no user-facing need.
- Task 10 complete (2026-09-02). Phase 1 exit-gate verification, no code
  changes needed. Evidence:
  - `npm ci` clean, `npm run build` clean, `npm test` 47/47 (compaction,
    durable-run, coding-tools, host, mcp-obscura, rpc, session-search-tree,
    skills-commands).
  - `cargo fmt --check` clean; `cargo check --all-targets` clean;
    `cargo clippy --all-targets -- -D warnings` clean;
    `cargo test --test protocol agent_protocol` 16/16 (includes
    `phase25_dependencies_deny_acp_agui_mcp`);
    `cargo test --test runtime` 73/73. Full Rust suite: one transient
    large-document perf-budget flake (open->head 888ms vs 500ms budget under
    parallel load) that passed in isolation and stayed green for 7
    consecutive full-suite reruns (1170 lib + 73 runtime + 202 protocol +
    134 presentation + 40 security) — no code change.
  - Dependency graph: `npm ls --all` shows only `@arnilo/prism`,
    `@arnilo/prism-core`, `@arnilo/prism-providers` plus the four Phase 1
    family pins (`@arnilo/prism-coding-tools`, `-web-tools`, `-memory`,
    `-mcp`), `better-sqlite3`, and `playwright-core`; zero retired 0.3
    names. Cargo.toml has no `@modelcontextprotocol`; MCP stays only in the
    clay-agent npm graph.
  - Chat/mock byte-compatibility probe (live host): mock Chat session emits
    exactly 9 lifecycle events (agent_started…agent_finished) with no
    tool/permission/tool_result payloads anywhere in the emitted JSON,
    `session.new` returns `tools: []`, prompt result shape unchanged
    (`lastEvent,runId,sessionId,status`). Existing tests enforce the same:
    "chat host session with no coding tools stays tool-free", "chat runs
    stay non-durable", "chat session without OM attach has no recall
    tool". README Chat honesty sentence intact ("Chat never emits
    tool/permission variants").
  - Coding fixture (covered by the passing suites, daemon store): nine tools
    registered with `ask_user_decision`; read returns dirty buffer of an
    open document; write goes through server authority and bumps version;
    edit on a user-held lease fails closed; acceptance policy gates
    out-root writes and shell metacharacters; durable run suspends before
    tool side effect and resume-approve executes once; D1 omitted-`allowCustom`
    resume accepted; manual compaction appends a compaction entry on the
    daemon SQLite store; threshold strategy override writes an llm
    compaction entry; OM attach records an observation and recall
    round-trips a known id against the daemon store; Chat mock
    prompt/resume/cancel all pass.
  - Startup reports Prism `0.4.0` (host test "initialize reports prism
    0.4.0").
- Task 9 complete (2026-09-02). Files: `clay-agent/src/mcp.ts` (new:
  `connectAllowListedMcpServers` — synchronous fail-closed validation of the
  server-built allow-list (canonical absolute executable, literal bounded
  argv, explicit env names, known fields, serverId charset, 32-server cap)
  BEFORE any spawn; one bad entry fails the whole connect closed;
  `stderr: "pipe"` bounded capture), `clay-agent/src/obscura.ts` (new:
  `spawnObscuraHarness` — managed-mode CDP via `connectObscuraCdp` on
  127.0.0.1 only (no `allowRemoteEndpoint`, no insecure flags),
  `createObscuraMcpTools` complete advertised surface with `obscura_` prefix,
  `createBrowserTools` composition; half-started serve process cleaned up on
  failure), `clay-agent/src/resolve-obscura.ts` (new: `CLAY_OBSCURA_BIN` →
  PATH → `/usr/local/bin/obscura`, absolute results only, undefined when
  absent = hidden capability), `clay-agent/src/host.ts` (`ensureCapabilities`
  lazily on first coding session — never on import or Chat initialize;
  validation errors (-32602) fail closed, connection failures hide the
  capability; owned children killed in `close()`; capability tools ride with
  coding sessions; `mcpAllowList` + `resolveObscuraBinary` host options),
  `clay-agent/src/main.ts` (`initialize` accepts server `mcpAllowList`;
  shutdown closes children; initialize result reports `mcpServers` count),
  `src/server/agent.rs` (`AgentMcpAllowListEntry` typed config +
  `mcp_allow_list` on `AgentHostConfig`, sent as `mcpAllowList` in
  `initialize` — server-built, never package argv),
  `clay-agent/package.json` (`playwright-core@1.61.0` exact added),
  `clay-agent/src/__tests__/mcp-obscura.test.ts`. Evidence: empty allow-list
  → zero tools and no-op close; relative/Windows/bare/NUL/oversized argv,
  non-identifier env names, unknown fields, and bad serverId all rejected
  before any connection; missing Obscura binary resolves undefined and a
  coding session spawns with no `obscura_*` tools while Chat initialize is
  unaffected; stub binary exercises the harness cleanup path (owned serve
  process killed, CDP session closed on failure); coding session still
  created when the harness fails to spawn (capability reduction, not error);
  no `/brave` `/exa` `/firecrawl` imports in daemon source (test enforced);
  Cargo.toml has no `@modelcontextprotocol` (deny test green). Tests:
  daemon 47/47, full Rust suite green (one unrelated large-document perf
  budget flake passed in isolation and suite rerun), fmt + clippy clean.
  Ceilings: MCP tools unclassified by Prism effect policy stay
  `external_mutation`/`unsupported` until a Clay effect policy exists;
  Obscura `browser_search` (in-page) is not public-web search and
  `/brave`-style providers remain out of scope; Playwright CDP attach not
  exercised against a real browser in unit tests (stubbed seam).
- Task 8 complete (2026-09-02). Files: `clay-agent/src/host.ts`
  (`session.new` stamps `SESSION_SEARCH_WORKSPACE_METADATA_KEY` in persisted
  metadata; `session.search` wraps `persistence.searchSessions` with
  `tenantId` + `workspaceRoot` + optional FTS `query`, page cap
  `MAX_SESSION_SEARCH_LIMIT = 100`, Prism default 20, snippets redacted;
  `session.checkout` calls server `checkpoint.restore` before
  `session.checkout(entryId)` — restore failure fails the checkout closed;
  `session.fork` (same id, new branch) / `session.clone` (pre-creates the
  tenanted session record before Prism's entry copy so the clone row is not
  tenant-less; new id, `workspaceRoot` metadata stamped),
  `session.checkpoint` → reverse `checkpoint.capture`),
  `src/server/agent_checkpoints.rs` (new: in-memory per-document snapshots
  keyed `(sessionId, entryId)`; capture snapshots every open document buffer
  via canonical paths; restore routes snapshot text through
  `open_existing_file_unlocked` + `apply_edit` — same lease/CAS path as all
  mutations — buffer-only, no disk rewrite (decision 2200); foreign-lease
  documents fail closed; missing checkpoint restores as no-op so
  pure-conversation checkout works), `src/server/agent_documents.rs`
  (`checkpoint.capture` / `checkpoint.restore` reverse methods),
  `src/server/mod.rs` (store wiring), `src/server/workspace/mod.rs`
  (`open_document_canonical_paths`), `src/protocol/agent.rs` +
  `src/server/agent.rs` (`SessionTree` client command forwarding
  checkout/fork/clone/checkpoint), `tests/agent_protocol.rs` (command
  coverage), `clay-agent/src/__tests__/session-search-tree.test.ts`.
  Evidence: two-workspace search returns only own-workspace sessions
  (cross-workspace hits absent, empty result not an error); search hit
  includes `sessionId` + `leafId`; checkout moves the leaf without appending
  entries (entry count unchanged); fork keeps sessionId at the requested
  leaf while the original branch stays listable; clone gets a new session id
  with copied branch entries; checkpoint capture round-trips through the
  reverse RPC and a failed restore (lease held elsewhere) rejects checkout
  without moving the leaf. Rust unit test: capture→dirty edit→restore
  reverts text to checkpoint. Findings: Prism's clone entry-append
  auto-creates the session row with `tenant_id: NULL` and `appendSession`
  cannot backfill tenant — fixed by pre-creating the record;
  searchSessions with no FTS query returns sessions with zero entries
  (fine for resume UX). Tests: daemon 39/39, full Rust suite green
  (incl. 1170 lib), fmt + clippy clean. Ceilings: checkpoint store is
  in-memory (daemon restart drops snapshots — restore becomes no-op,
  marked with `ponytail:` in source); search is transcript-only by design
  (tool output not indexed); no auto-injection of hits into agent context.
- Task 7 complete (2026-09-02). Files: `clay-agent/src/host.ts`
  (`runState` on coding prompts — `checkpoints` from SQLite persistence,
  `definitionRevision: "clay-agent.1"`, `interruptBeforeTool: true`; explicit
  subscribe+run event pump capturing `AgentRunResult`; `run.resume` RPC with
  decision-shape validation before any checkpoint I/O),
  `clay-agent/src/__tests__/durable-run.test.ts`. Evidence: coding prompt
  whose mock provider calls `read` suspends with `status: "suspended"`,
  `version`, and one redacted `pendingDecisions` entry (scope
  `toolName: "read"`, no raw arguments); tool did not execute before
  approval (`reads === 0`); `run.resume` with `decisions:
  [{ approvalId, outcome: "allow_once" }]` + current `expectedVersion`
  executes the tool exactly once (`reads === 1`); stale `expectedVersion`
  resume rejects (`Stale or non-suspended`) with no side effect; malformed
  `decision: "sideways"` and unknown batch `outcome` reject without
  checkpoint mutation (valid resume still succeeds after); Chat prompt stays
  non-durable (`run.resume` rejects `No durable agent run`). Agents now
  carry `id: profile` (Prism requires AgentConfig.id for durable runs).
  Tests: daemon 35/35. Deferred: `src/server/agent.rs` /
  `src/protocol/agent.rs` forwarding of `run.resume` — no UI/consumer path
  exists yet; add a Rust client command when the shell resumes approvals
  (task 9+), same posture as task 6.
- Task 6 complete (2026-09-02). Files: `clay-agent/src/host.ts` (skill +
  command RPC, `createLoadSkillTool` on skill-holding profiles, host
  `CommandDrivers` injected at dispatch), `clay-agent/src/__tests__/
  skills-commands.test.ts`. Evidence: `skill.list` returns catalog-only
  (name+description, no instructions); `load_skill` pulls the body into the
  session `LoadedSkillSet`; duplicate skill name fails closed
  (`duplicate: "error"`); a skill with an inactive `toolNames` entry throws
  before a provider turn; `/steer`-like command dispatch calls host
  `drivers.steer` on a live run; `startWorkflow` command returns the Phase 5
  error (not a missing-key crash); `drivers` in register params is inert
  (ignored, never stored); dispatch without a live session omits the
  `drivers` key and fails closed; unknown command names fail closed. Tests:
  daemon 32/32. Deferred: `src/server/agent.rs`/`src/protocol/agent.rs`
  forwarding (plan's chosen approach already defers package-fed contribution
  data to Phase 4 — tests register directly against daemon RPC; add a Rust
  command when a UI/consumer path needs it).
- Task 5 complete (2026-09-02). Files: `clay-agent/src/compaction.ts`
  (named `default`/`llm`/`om` factories + kernel register),
  `clay-agent/src/host.ts` (`session.compact`, prompt `compaction` override,
  OM attach + recall tool on opt-in), `src/protocol/agent.rs` (`Compact`),
  `src/server/agent.rs` (forward `session.compact`). Evidence: manual compact
  appends compaction entry while raw messages remain; active-run compact
  fails closed (`already has an active run`); `llm` override writes
  `data.strategy === "llm"` with mock provider (no network); OM attach
  records an observation, recall round-trips a known 12-hex id, invalid id
  fails closed; Chat without OM has no recall tool. Defaults: worker models
  from host config (`requireExplicitModel: true`), `compactAfterTokens`
  80000. Tests: daemon 24/24, protocol 202/202, fmt + clippy clean.
  Ceilings: OM context blocks not wired into `AgentConfig.context` (plan
  forbids auto-inject); llm summary uses session provider until a dedicated
  summary model exists; no PostgreSQL / wiki / graft / rag imports.
- Task 4 complete (2026-09-02). Files: `clay-agent/src/coding-tools.ts`
  (per-session tool build + 2157 acceptance policy),
  `clay-agent/src/document-ops.ts` (reverse-RPC Read/Write/EditOperations),
  `clay-agent/src/host.ts` (reverse `request()` + per-session coding tools +
  `session.new` `workspaceRoot`/`fullAutonomy`), `clay-agent/src/main.ts`
  (reverse-response routing), `src/server/agent_documents.rs` (new:
  `document.read`/`write`/`mkdir`/`stat` + fail-closed `approval.request`),
  `src/server/agent.rs` (reverse-RPC routing + stdin writer task +
  `set_reverse_handler`), `src/server/workspace/mod.rs` (containment +
  by-path lookup + release helpers), `src/server/mod.rs` (handler wiring).
  Evidence: dirty-buffer read (open doc wins over disk), write bumps server
  version through `apply_edit` + CAS save + lease release, user-held lease
  fails closed, out-of-root mutations gated (deny / approve / fullAutonomy),
  shell metachars gated, D1 suspend data + resume. Tests: daemon 18/18,
  lib `agent_documents` 4/4, protocol 202/202 (new reverse round-trip test),
  fmt + clippy clean. Ceilings recorded: single bootstrap workspace (multi-tab
  routing -> Phase 2), no approval cache (-> task 7), `approval.request` /
  `ask_user_decision` server handlers fail closed (UI -> Phase 2),
  `session.new` autonomy/root params daemon-side only (Rust
  `NewSession` passthrough deferred to task 7/8 protocol work).
- Task 1 complete (2026-09-02). Evidence: `clay-agent/package.json` pins the
  four exact 0.4.0 families beside the Phase 0 pins; lockfile regenerated via
  `npm install` + verified with `npm ci`; `npm ls --all` shows
  `@modelcontextprotocol/sdk@1.30.0` only as a transitive dep of
  `@arnilo/prism-mcp`, `playwright-core@1.61.0` UNMET OPTIONAL, and no
  `prism-acp` / `prism-ag-ui` / `prism-office` / `prism-antigravity-agent`.
  `tests/agent_protocol.rs::phase25_dependencies_deny_acp_agui_mcp` now denies
  `@modelcontextprotocol` / `@arnilo/prism-mcp` in Cargo.toml only, keeps
  ACP/AG-UI/retired denies universal, and asserts all eight exact pins.
  Gates: `npm ci && npm run build && npm test` (9 pass), `cargo fmt --check`,
  `cargo check --all-targets`, `cargo clippy --all-targets -- -D warnings`,
  `cargo test --test protocol` (201 pass). Note for later tasks: the deny
  test target runs as `cargo test --test protocol` (suite
  `tests/suites/protocol.rs`), not `--test agent_protocol`.
- Task 2 complete (2026-09-02). Coverage map written into the task (see
  Completion Evidence): Obscura → 2159, package MCP → 0714 (no gap — argv
  never originates from package JS; Phase 2/4 must add provenance/digest
  binding to the server-side allow-list record), skills/commands → 1758 +
  0001, computer-use-linux → 2159 (st-only, Phase 5). Verified against Prism
  source: `validateObscuraCommand` enforces canonical absolute command,
  bounded literal argv, env-key pattern, insecure-flag deny, loopback CDP;
  `connectMcpTools` gives the host explicit command/args/env/cwd control with
  empty default exposure. No new decision log.
- Task 3 complete (2026-09-02). Inventory in task evidence: `DocumentState`
  version/lease/dirty + CAS save ops exist (`apply_edit` is the single
  mutation path — reuse, don't fork); `AgentHost` reply path for
  daemon-initiated requests already exists (only method routing missing);
  `AgentWireEvent` Tool/Permission variants pre-exist — no IPC rewrite.
  Generic gaps: document.* routing (task 4), checkpoint join (task 8),
  resume/compact/search commands (tasks 5–8).
- To be filled after remaining tasks with improvements, rationale, and
  priority.
