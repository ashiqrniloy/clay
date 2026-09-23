# Plan 153 — pi Coding Agent: First External Runtime Adapter

Source: decision `decision-logs/2026-09-23-1821-one-agent-registry-prism-rename-ari.md`
(option A′). Implements the first `ExternalAgentRuntime` (plan 152's trait)
around the pi coding agent's documented RPC mode: `pi --mode rpc` as a
core-spawned child, JSONL commands/responses/events bridged onto the ARI
lifecycle and the existing `AgentWireEvent` projection, extension-UI dialogs
mapped to the agent-lane approval flow, and a pi capability declaration that
gates the UI (plan 152's gate). pi appears in the agent registry beside
Prism and can be a tab's primary agent. Hidden entirely when the `pi`
binary is absent (fail-closed).

Depends on plans 149 (registry descriptors) and 152 (ARI trait, capability

types, approved `agent-runtime-capability-states` artifact). Execution
order 146 → 147 → 148 → 149 → 150. The pi runtime is spawned by the
**detached clay-server** (plan 150): pi children survive GUI close
alongside the native daemon, and the plan-151 Agents navigator surfaces
them via the inventory. No overlap with plan 136/142.

Binding prior decisions:

- `decision-logs/2026-09-23-1821-one-agent-registry-prism-rename-ari.md`:
  pi is the first external runtime; primary-agent framing.
- `decision-logs/2026-09-02-1440-direct-external-coding-agent-adapters.md`:
  direct vendor-surface integration, policy bundles, truthful capability
  boundaries, no Prism intermediary.
- `decision-logs/2026-07-14-2023-language-server-package-authority.md`
  pattern + `decision-logs/2026-09-08-2141-binary-provisioning-deny-by-default-table.md`:
  deny-by-default external-binary discipline this plan follows (authority
  task below records the pi-specific log).
- `decision-logs/2026-08-21-1758-native-prism-host-no-acp-cli-parity.md`:
  the AG-UI projection is the single frontend wire — pi events join it,
  never a second frontend path.

pi documentation (authoritative, local install:
`/home/arn/.bun/install/global/node_modules/@earendil-works/pi-coding-agent/docs/`):
`rpc.md` (protocol, framing, lifecycle, shutdown), `rpc-commands.md`
(command set: `prompt`, `steer`, `follow_up`, `abort`, `new_session`,
`switch_session`, `fork`, `get_state`, `get_messages`, `set_model`,
thinking-level commands, `get_commands`, bash commands),
`rpc-extension-ui.md` (dialog + fire-and-forget subprotocol),
`json.md` (session-event shapes: `agent_start/end/settled`, `turn_*`,
`message_*`, tool events), `session-format.md` (JSONL entries, trees),
`cli.md`. Re-verify against the installed `pi --help` at execution time; the
docs pages, not this plan's prose, win on drift.

## Objectives

- Record the pi external-process authority decision (deny-by-default,
  canonical executable, literal argv, env allow-list, workspace cwd,
  truthful containment language) with explicit user approval **before any
  process-spawning code lands**.
- Ship `PiRuntime`: an `ExternalAgentRuntime` implementation spawning
  `pi --mode rpc --no-session` with cwd = the session's workspace root —
  lifecycle mapping, strict JSONL framing (LF-only reader, backpressure
  honored), id-correlated commands, bounded frames, fail-closed on
  malformed output, exit, and auth/startup failure.
- Bridge pi session events onto `AgentWireEvent` (text deltas, tool
  phases, run lifecycle) through the existing redaction discipline, and
  pi extension-UI dialog requests onto the approval flow
  (`extension_ui_request` → pendingApproval → `extension_ui_response`),
  fire-and-forget UI records onto the state strip.
- Declare pi's capabilities (streaming, dialog approvals, steering,
  session trees, model switch, effort switch, slash commands, compaction
  events; **not** memory activity or full context inspection in v1) and
  register `pi` in the agent registry (kind `external`, hidden when the
  binary is absent).
- Prove the adapter with a recorded/mock JSONL harness in CI plus one
  recorded authenticated manual run with the real `pi` binary.

## Expected Outcome

- With `pi` installed: the picker lists pi beside Prism; a tab bound to pi
  prompts, streams a transcript, surfaces extension-UI approvals in the
  agent lane, steers, cancels, resumes vendor sessions (fork/switch), and
  switches model/thinking level — with Memory/Context tabs absent per the
  capability gate. Without `pi`: no registry entry, no residue.
- CI proves every path against a mock pi child (scripted JSONL) including
  malformed frames, oversized records, unknown record types (tolerated and
  logged, per pi's forward-compat posture), child exit, and blocked
  dialogs.

## Tasks

- [ ] Record the pi external-process authority decision (user approval gate)
  - Acceptance Criteria:
    - Functional: decision log `decision-logs/<date>-pi-external-runtime-process-authority.md`
      exists, explicitly approved by the user, recording: fixed contribution
      (the `pi` runtime adapter), canonical executable resolution
      (`pi` from PATH or explicit user override under
      `~/.clay/agents/pi/pi.json`), literal argv
      (`--mode rpc --no-session`), explicit inherited-environment allow-list,
      cwd = session workspace root, no shell, bounded I/O with timeouts,
      cleanup on revocation/session close, and truthful containment
      language (same-user authority, cwd constrains Clay's API and audit
      record, not OS confinement — never call it sandboxed).
    - Performance: none.
    - Code Quality: log follows the create-decision-log template; this task
      blocks every spawn-code task below.
    - Security: deny-by-default — no spawn before the log exists.
  - Approach:
    - Documentation Reviewed:
      `decision-logs/2026-07-14-2023-language-server-package-authority.md`
      (authority-task duty), `2026-09-08-2141-…` (provisioning table),
      pi `rpc.md` (startup, shutdown, stderr-not-protocol rule).
    - Options Considered:
      - Reuse decision 1440 as blanket authority — rejected: the authority
        duty requires a per-process log with the concrete argv/env facts.
      - pi-specific log — chosen.
    - Chosen Approach: draft from this plan's objective bullets; user
      approves; then implementation.
    - Files to Create/Edit: the decision log above.
    - References: pi `rpc.md` "Shutdown"; `security.md` if shipped locally.
  - Test Cases to Write: none.

- [ ] Binary resolution + config root (fail-closed, hidden when absent)
  - Acceptance Criteria:
    - Functional: `PiRuntime::available()` resolves the executable once per
      server run (PATH `pi` or the `pi.json` override; `~`-expanded or
      absolute only, relative rejected); absent/unreadable ⇒ registry entry
      hidden, zero residue. Config root `~/.clay/agents/pi/` created on
      first enable with a documented `pi.json` (executable override,
      defaults) following the tool-caps.json starter pattern; starter
      shipped under `examples/config/agents/pi/`.
    - Performance: resolution cached; no per-prompt stat storm.
    - Code Quality: mirrors the existing fail-closed resolution patterns
      (Obscura/graft resolution in the daemon; binary provisioning table).
    - Security: override path never evaluated relative; no PATH search for
      the override case.
  - Approach:
    - Documentation Reviewed: `clay-agent/src/resolve-obscura.ts` (pattern),
      `decision-logs/2026-09-08-2141-…`, `examples/config/agents/`
      starter conventions.
    - Options Considered:
      - Bundle pi via the provisioning table — rejected for v1: pi is a
        user-installed npm package with its own update channel; add to the
        table only if user demand appears (Further Actions).
      - User-resolved binary — chosen.
    - Chosen Approach: Rust-side resolution (adapter lives in core), cached
      with invalidation on config reload.
    - Files to Create/Edit: `src/server/agent/runtime/pi.rs`
      (resolution), `examples/config/agents/pi/pi.json`.
    - References: pi `cli.md` (install/flags).
  - Test Cases to Write:
    - Absent binary ⇒ hidden registry entry.
    - Relative override path rejected fail-closed.

- [ ] Child process lifecycle: spawn, framing, correlation, shutdown
  - Acceptance Criteria:
    - Functional: spawn per session (one pi child per agent session, cwd =
      workspace root, argv literal, env allow-list per the authority log);
      strict JSONL framing — one JSON object per LF-terminated line, byte
      reader that does not split on Unicode line/paragraph separators
      (Node `readline` is explicitly unsuitable per pi docs — Rust
      implementation must split on LF only); command ids correlated by
      response `id`, not order; `agent_settled`-aware idle tracking;
      graceful shutdown by stdin close + timeout + process-group kill;
      stderr drained as diagnostics, never parsed as protocol.
    - Performance: bounded frame size with a documented ceiling;
      backpressure honored both directions (slow UI never stalls the child
      indefinitely — bounded queues with fail-closed overflow).
    - Code Quality: actor pattern mirroring the daemon supervisor; typed
      serde records for every consumed record kind.
    - Security: env allow-list exactly as logged in the authority
      decision; no credential passing; secrets redactor applied to all
      outbound event payloads.
  - Approach:
    - Documentation Reviewed: pi `rpc.md` (Framing, Run lifecycle, Errors,
      Shutdown, Minimal client), `json.md` (Framing and process I/O),
      plan 150 (detached clay-server is the parent process).
    - Options Considered:
      - Long-lived one-child-for-all-sessions — rejected: sessions are
        independent conversations; pi sessions are switchable but a shared
        child couples failure domains.
      - Child per session — chosen (server-owned; persistence across GUI
        restarts comes from plan 150's detached server, not from this
        plan).
    - Chosen Approach: tokio actor, `BufReader` + manual LF split, framed
      writer.
    - API Notes and Examples:
      ```text
      spawn: pi --mode rpc --no-session   (cwd = workspace root)
      cmd:  {"id":"req-1","type":"prompt","message":"..."}
      evt:  {"type":"message_update","assistantMessageEvent":{"type":"text_delta","delta":"..."}}
      ui:   {"type":"extension_ui_request","id":"uuid-1","method":"confirm","title":"..."}
      ```
    - Files to Create/Edit: `src/server/agent/runtime/pi.rs` (+ `mod`).
    - References: authority decision log (this plan, task 1).
  - Test Cases to Write:
    - LF-only framing (U+2028 inside a JSON string survives).
    - Oversized/malformed line fails closed with typed error.
    - stdin-close shutdown completes; timeout path kills the process group.

- [ ] ARI lifecycle mapping (start/prompt/cancel/resume/respond)
  - Acceptance Criteria:
    - Functional: `start` = spawn (+ `new_session` when the tab starts
      fresh); `prompt` = pi `prompt` (response acceptance ≠ completion —
      run state driven by `agent_start`/`agent_end`/`agent_settled`);
      `cancel` = pi `abort`; `steer` = pi `steer` (mid-run) vs `follow_up`
      (idle/queued) chosen by run state; `resume` = pi
      `switch_session`/`fork` with the vendor session id round-tripped
      through the snapshot; `respond` routes approval answers (next task).
      Slash commands typed in the composer pass through as prompts
      (pi expands its own `/` commands; `get_commands` populates the
      palette's command list when `SlashCommands` is declared).
    - Performance: no added hop beyond the child pipe; prompt round-trip
      latency within the daemon's existing acceptance window.
    - Code Quality: mapping table documented in the wiki task; unknown pi
      response ⇒ typed error, session marked degraded, never a silent
      drop of the user's prompt.
    - Security: user prompts are data, never evaluated as config.
  - Approach:
    - Documentation Reviewed: pi `rpc-commands.md` (all command shapes,
      `steer`/`follow_up` semantics, `get_commands`),
      `rpc.md` (Correlate commands and responses).
    - Options Considered:
      - Only prompt/cancel in v1 — rejected: steering and trees are pi's
        differentiators; the contract should be exercised.
      - Full mapping — chosen.
    - Chosen Approach: implement against `rpc-commands.md` shapes; pin the
      command list in tests via the mock harness.
    - Files to Create/Edit: `src/server/agent/runtime/pi.rs`.
    - References: plan 152 trait signature.
  - Test Cases to Write:
    - Prompt accepted → `agent_settled` drives the run state machine.
    - Steer during run routes to `steer`; after settle routes to
      `follow_up`.
    - Resume restores the vendor session id into the snapshot.

- [ ] Event bridge onto the AG-UI projection
  - Acceptance Criteria:
    - Functional: pi `message_start/update/end` → transcript entries with
      text deltas; tool events → `AgentWireEvent::Tool` phases (started/
      finished + output digest); `agent_start/end/settled` → run status;
      compaction events → state strip note; **unknown session-event types
      are tolerated** (logged, not fatal — pi's forward-compat posture)
      while **unknown extension-UI methods fail the dialog closed** with a
      visible note (never a hang); all payloads pass the existing secrets
      redactor before projection; raw vendor tool output stays out of FTS
      and logs by default (decision 1440).
    - Performance: delta coalescing consistent with the daemon path's
      stream-token discipline; no per-delta snapshot rebuilds.
    - Code Quality: one mapping module, table-driven, unit-tested per
      record kind; reuses `map_event`-adjacent projection code, not a
      parallel implementation.
    - Security: redaction coverage tests with secret-bearing fixtures.
  - Approach:
    - Documentation Reviewed: pi `json.md` (event tables, delta-only wire,
      reconstruct-streaming rules), `src/server/agent_agui.rs`.
    - Options Considered:
      - Reconstruct full messages client-side from deltas — rejected: pi
        already sends authoritative `message_end`; use deltas for paint,
        `message_end` for state.
      - Chosen: that split.
    - Chosen Approach: mapping module + redaction + projection reuse.
    - Files to Create/Edit: `src/server/agent/runtime/pi/events.rs` (or
      inline module).
    - References: 0.7 unknown-event-drop precedent (relaxed here for pi per
      its documented compat posture — recorded in the wiki task).
  - Test Cases to Write:
    - Per-record-kind mapping units (fixture JSONL from pi docs).
    - Secret-bearing delta is redacted in the projected event.
    - Unknown event type logged and survived; unknown dialog method fails
      closed.

- [ ] Approval bridge: extension-UI dialogs ↔ agent lane
  - Acceptance Criteria:
    - Functional: `extension_ui_request` dialog methods (`select`,
      `confirm`, `input`, `editor`) surface as the session's
      `pendingApproval` (reusing the existing approval wire + UI, rendered
      per the approved agent-lane artifact); the user's answer sends
      `extension_ui_response` with the matching id (`value` or
      `cancelled: true`); pi-side timeouts auto-resolve without client
      tracking (pi owns the default); fire-and-forget methods (`notify`,
      `setStatus`, `setWidget`, `setTitle`) map to state-strip/transcript
      notes; a dialog raised while another is pending queues visibly, and
      a cancelled tab/child fails the dialog closed.
    - Performance: dialog round-trip does not block the event pump.
    - Code Quality: one outstanding-dialog table keyed by pi request id.
    - Security: dialog payloads redacted; options list bounded.
  - Approach:
    - Documentation Reviewed: pi `rpc-extension-ui.md` (all methods,
      timeout semantics, RPC-mode limitations), approved artifacts
      `agent-lane-palette` + `agent-runtime-capability-states` (approval
      strip states).
    - Options Considered:
      - New approval UI for external runtimes — rejected: one approval
        surface for all runtimes is the A′ point.
      - Reuse pendingApproval — chosen.
    - Chosen Approach: capability declaration `Approvals { modify_input:
      false }` for pi v1 (dialog-level; no tool-input editing round-trip
      documented by pi RPC).
    - Files to Create/Edit: pi runtime module + capability constant.
    - References: plan 152 capability enum (`Approvals`).
  - Test Cases to Write:
    - Confirm-dialog round-trip (mock child): request → pendingApproval →
      response → run continues.
    - Cancelled dialog sends `cancelled: true`.
    - Two overlapping dialogs queue without id clobbering.

- [ ] Capability declaration, registry entry, and picker integration
  - Acceptance Criteria:
    - Functional: pi registry entry (id `pi`, kind `external`,
      `runtimeId: pi`, capabilities: `StreamingEvents`, `Approvals
      {modify_input: false}`, `Steering`, `SessionTrees`, `Compaction`,
      `ModelSwitch`, `EffortSwitch`, `SlashCommands`, `FileHistory`
      (derived from tool events); **not** `MemoryActivity`, **not**
      `ContextInspection` in v1); picker lists it only when the binary
      resolves; gating verified against plan 152's approved artifact
      (Memory/Context tabs absent, steer/effort/model present, files
      history populated from tool events when edit/write tools appear).
    - Performance: registry probe cached.
    - Code Quality: capability constant lives beside the adapter, not in
      the picker.
    - Security: declaring `FileHistory` never grants filesystem authority —
      display-only derivation from events the runtime already sent.
  - Approach:
    - Documentation Reviewed: plan 152 capability enum + gate; pi
      `rpc-commands.md` (`get_available_models`, thinking-level commands)
      backing `ModelSwitch`/`EffortSwitch`.
    - Options Considered:
      - Derive capabilities dynamically from a probe — deferred: v1 ships a
        static declaration matching the documented RPC surface; probing is
        a Further Action once pi's RPC has a version handshake.
      - Static declaration — chosen.
    - Chosen Approach: constant + registry wiring + gated-UI conformance
      test.
    - Files to Create/Edit: `src/server/agent/runtime/pi.rs`,
      `src/server/agent_picker.rs` registration.
    - References: decision 1440 ("never add controls the runtime has not
      declared").
  - Test Cases to Write:
    - Gated-UI conformance: pi session renders no Memory/Context tab, no
      undeclared control.
    - FileHistory derivation from a fixture edit-tool event.

- [ ] Mock-harness CI tests (no real pi required)
  - Acceptance Criteria:
    - Functional: a scripted mock pi child (JSONL fixture files, or a tiny
      Rust test double reading a script) exercises: startup handshake-free
      RPC, prompt round-trip with streaming events, approval dialog
      round-trip, steer/follow_up routing, cancel, resume via
      `switch_session`, model switch, malformed/oversized line, unknown
      record tolerance, child exit mid-run, stderr noise, stdin-close
      shutdown.
    - Performance: suite runtime bounded (no real sleeps beyond timeouts
      under test).
    - Code Quality: fixtures recorded in-repo under
      `tests/` fixtures following the repo's fixture conventions; the
      harness is reusable for future external-runtime adapters.
    - Security: fixtures contain no real credentials; redaction tests use
      synthetic secrets.
  - Approach:
    - Documentation Reviewed: repo fixture patterns
      (`src-tauri/tests/bridge_session.rs`, `tests/agent_session_isolation.rs`
      style); pi `json.md` record examples (fixture source).
    - Options Considered:
      - Spin a real `pi` in CI — rejected: adds a Node + npm dependency to
        the Linux gate; the manual task covers the real path.
      - Mock child — chosen.
    - Chosen Approach: stdio pipe to a fixture-driven process.
    - Files to Create/Edit: `tests/pi_runtime/*.rs` + fixtures.
    - References: authority decision log (env/argv facts under test).
  - Test Cases to Write: the list above.

- [ ] Recorded authenticated manual run (real pi binary)
  - Acceptance Criteria:
    - Functional: with the user's real `pi` install, one recorded session:
      pick pi in a tab, prompt a real task in a scratch workspace, observe
      streaming transcript, answer one real extension-UI dialog, steer
      mid-run, cancel, resume the vendor session after tab reopen, switch
      model/thinking level; evidence (command log, screenshots or transcript
      export) recorded in task notes.
    - Performance: subjective responsiveness noted.
    - Code Quality: deviations from docs become adapter fixes or filed
      drift notes.
    - Security: scratch workspace only; no real project paths in evidence.
  - Approach:
    - Documentation Reviewed: pi `cli.md`, `rpc.md` (verification duty).
    - Chosen Approach: manual drill, mirrors Phase 9's authenticated-run
      exit-gate style.
    - Files to Create/Edit: none (evidence).
  - Test Cases to Write: none.

- [ ] Update the canonical example configuration (examples/config/init.js)
  - Acceptance Criteria:
    - Functional: the new `~/.clay/agents/pi/` surface appears exactly once
      in the example config tree (`examples/config/agents/pi/pi.json`
      starter referenced/commented per the established pattern); init.js
      itself unchanged unless a loadable surface exists (none expected —
      external runtimes register via the server, not `loadPackage`).
    - Code Quality: `node --check examples/config/init.js` passes.
    - Security: starter documents the executable-override rules, not a
      blanket trust grant.
  - Approach:
    - Documentation Reviewed: `references/clay.md` Example Configuration
      Maintenance Task.
    - Chosen Approach: starter file + comment sweep.
    - Files to Create/Edit: `examples/config/agents/pi/pi.json`, init.js
      only if needed.
  - Test Cases to Write: starter parses against the server-side schema
      test.

- [ ] Example configuration live launch-test
  - Acceptance Criteria:
    - Functional: scratch-config GUI launch per the duty; picker lists pi
      when a stub `pi` executable is placed on the scratch PATH (proving
      resolution + registration without the real binary), and hides it
      when removed; evidence recorded.
    - Performance: within startup gates.
    - Code Quality: launch command + results in evidence.
    - Security: scratch home only.
  - Approach:
    - Documentation Reviewed: `references/clay.md` Live Launch-Test Task.
    - Chosen Approach: two runs (with/without stub binary).
    - Files to Create/Edit: none.
  - Test Cases to Write: none.

- [ ] Create or verify Clay JS APIs for public programmatic surfaces
  - Acceptance Criteria:
    - Functional: the adapter stays `pub(crate)`; no new JS-reachable op
      exposes spawn/control of external runtimes (future `agent.*` package
      APIs are Phase 4 scope); any leaked public fn gets the op + facade +
      docs treatment or is made private.
    - Code Quality: doc-registry gates green.
    - Security: JS cannot spawn `pi` or mutate capabilities.
  - Approach:
    - Documentation Reviewed: `.agents/skills/clay-execution/references/js-api.md`.
    - Chosen Approach: verification.
    - Files to Create/Edit: as found.
  - Test Cases to Write: as found.

- [ ] Code-wiki and documentation truth update
  - Acceptance Criteria:
    - Functional: wiki documents the pi adapter (lifecycle/event/approval
      mapping tables, capability declaration, fail-closed behaviors, the
      unknown-event tolerance note and its rationale), the ARI reference
      gains its first concrete implementer, truth checks pass.
    - Code Quality: docs-as-code workflow.
    - Security: none.
  - Approach:
    - Documentation Reviewed:
      `.agents/skills/clay-execution/references/docs-as-code.md`.
    - Chosen Approach: final wiki task.
    - Files to Create/Edit: `docs/wiki/`, `docs/reference/`.
  - Test Cases to Write: existing truth checks.

## Compromises Made

- To be filled after tasks are completed and tests pass.

## Further Actions

- To be filled after task completion. Known at planning time: dynamic
  capability probing (if pi grows an RPC version handshake); adding pi to
  the binary-provisioning table; `ContextInspection` via pi
  `get_entries`/`get_messages` as a later capability bump; PTY fallback
  lane for TUI-only controls; agent state rollup sidebar.
