# Plan 150 — Detached Agent Runtime Server

Source: decision
`decision-logs/2026-09-23-1908-detached-agent-runtime-server.md`
(user-decided, herdr-inspired). Closing Clay must not kill running agents;
launching Clay attaches to the running server and lists all running agents.
Clay keeps its own shape: one `clay-server` sidecar per user session
owning per-workspace sessions, the clay-agent daemon, and future external
runtimes; the GUI becomes a connect-or-spawn client of that server.

herdr reference points studied for this plan (implementation inspiration,
not obligation): `herdr` default launch probes the session socket and
attaches if listening (`src/server/autodetect.rs::auto_detect_launch`);
otherwise `spawn_server_daemon` spawns `herdr server` with stdio → null
and platform detach, then waits for socket readiness; the headless server
(`src/server/headless.rs::run_server`) owns panes until SIGINT / host
shutdown / `server stop`, saving state on exit.

Clay starting point: `Supervisor` (`src-tauri/src/server.rs`) already does
adopt-or-spawn with a typed protocol probe (Compatible → adopt,
Incompatible → fail-closed status, NotListening → spawn); workspaces
multiplex inside one server (`src-tauri/src/bridge/session.rs`); session
reclaim after webview reload exists (`connect_for_reclaim_or_new`). What
kills agents today: `spawn_locked` spawns a plain child, and
`RunEvent::Exit` in `src-tauri/src/lib.rs` calls `shutdown()` (kill+reap).

Depends on: nothing (independent of 149; can start immediately). Plans 151
(Agents navigator) and 150 (external runtimes as server children) consume
its outputs.

Binding prior decisions:

- `decision-logs/2026-09-23-1908-detached-agent-runtime-server.md`: source.
- `decision-logs/2026-09-14-1705-workspace-scoped-agent-sessions.md`:
  workspaces stay session-scoping units inside the one server.
- `decision-logs/2026-09-02-1440-direct-external-coding-agent-adapters.md`:
  external runtimes are server children — they inherit persistence here.
- `decision-logs/2026-09-23-1821-one-agent-registry-prism-rename-ari.md`:
  runtime inventory feeds the one-registry UI.

## Objectives

- Detach the server spawn (own session/process group, stdio to a rotating
  log file) and stop killing the server on app exit; agents keep running
  when Clay closes.
- Give the server an idle self-exit policy: exit after a configurable
  grace period with no connected clients and no active agent runs; expose
  explicit stop ("Quit Clay & stop agents" in the UI, plus a documented
  headless surface).
- Add a cross-workspace **running-agents inventory** on the
  server→frontend surface: per workspace and session — agent id, run
  status (`working` | `blocked` | `idle`), pending-approval flag, last
  activity — updating live, consumed by plan 151's Agents navigator.
- Keep adopt-or-spawn correctness under detachment: stale sockets,
  incompatible-version servers (typed status + user action), and
  multi-window attach must all behave deterministically.

## Expected Outcome

- Kill-test: start a run in Clay, quit the GUI (normal exit), observe via
  process list that `clay-server` and the agent daemon survive and the run
  completes; relaunch Clay, the transcript shows the completed run and
  the Agents navigator lists the session idle.
- Idle-test: a server with no clients and no active runs exits after the
  grace period (default 15 minutes, configurable), leaving no socket
  residue; a server with an active run never self-exits.
- The frontend can query `sessions inventory` on connect and receive live
  updates (new session, run start/end, approval raised/cleared, session
  close) for all workspaces.

## Tasks

- [ ] Baseline: lifecycle inventory and kill-path tests
  - Acceptance Criteria:
    - Functional: recorded inventory of every server lifecycle touchpoint:
      `Supervisor::{start, spawn_locked, shutdown, restart, Drop}`,
      `RunEvent::Exit` handler in `src-tauri/src/lib.rs`, the probe
      thread, `restart()` semantics, and existing lifecycle tests
      (`spawn_connect_shutdown_leaves_no_orphan` and siblings) — each
      marked keep / invert / extend under detachment.
    - Performance: none.
    - Code Quality: inventory names every test whose expectation
      **inverts** (no-orphan-on-exit becomes orphan-by-design), so no
      green test silently encodes the old behavior.
    - Security: inventory notes which paths hold child PIDs / socket
      ownership today.
  - Approach:
    - Documentation Reviewed: `src-tauri/src/server.rs`,
      `src-tauri/src/lib.rs`, `src-tauri/src/release.rs`
      (`resolve_server_binary`, `desktop_endpoint`).
    - Options Considered: none (mandatory inventory before touching
      lifecycle).
    - Chosen Approach: read + classify.
    - Files to Create/Edit: none.
    - References: decision 1908 (detachment).
  - Test Cases to Write: none.

- [ ] Detached spawn (survives GUI exit)
  - Acceptance Criteria:
    - Functional: `spawn_locked` spawns `clay-server` in a new session
      and process group (Linux `setsid` semantics via `pre_exec`/
      `process_group(0)`), stdio → null except stderr → rotating log file
      under the Clay log dir (release builds too — a detached server with
      discarded stderr is undebuggable); the GUI no longer holds a
      killable `Child` for the adopted/spawned server; `Drop` and
      `shutdown()` stop killing the server unless an explicit stop was
      requested.
    - Performance: spawn cost unchanged to first order; log rotation
      bounded (size-capped file set).
    - Code Quality: platform detach helper isolated for the Windows
      long-term path (documented no-op there — Windows remains a
      non-required target; never weaken Linux behavior for it).
    - Security: socket permission discipline unchanged; the detached
      server runs as the same user with the same endpoint ownership
      checks.
  - Approach:
    - Documentation Reviewed: herdr
      `src/server/autodetect.rs::{spawn_server_daemon, build_server_daemon_command}`
      (stdio null + detach pattern), herdr `src/platform/*`
      detach helpers.
    - Options Considered:
      - Keep child handle, kill only on explicit stop — rejected: a child
        of the GUI dies with the GUI's process group on some exit paths;
        detachment must be real.
      - True daemonization (fork) — rejected: Rust std/tokio favors
        spawn-detached over fork; platform detach on the child side is
        sufficient and testable.
    - Chosen Approach: `Command` + `process_group(0)` (Linux), stdio→log,
      drop the `Child` after readiness handshake; readiness still via
      probe thread.
    - API Notes and Examples:
      ```rust
      // spawn_locked: detach
      use std::os::unix::process::CommandExt;
      command.process_group(0); // new process group; survives parent exit
      // stderr → CLAY_LOG_DIR/server.log (rotated, size-capped)
      ```
    - Files to Create/Edit: `src-tauri/src/server.rs`,
      `src-tauri/src/release.rs` (log-path helper if none exists).
    - References: decision 1908; herdr autodetect.
  - Test Cases to Write:
    - Spawned server survives supervisor shutdown (process alive after
      `shutdown()` without explicit stop).
    - Release-mode stderr lands in the log file (test with CLAY_LOG_DIR
      override).

- [ ] Exit policy: GUI exit, idle self-exit, explicit stop
  - Acceptance Criteria:
    - Functional: `RunEvent::Exit` no longer kills the server. The server
      self-exits after a grace period (default 15 min, configurable,
      documented) with zero connected clients and zero active agent runs
      (active run = any session with an in-flight run or raised approval);
      a client connecting during grace cancels the timer. Explicit stop
      surface: UI action "Quit Clay & stop agents" (confirm dialog when
      active runs exist) and the same reachability via the existing
      server management path; stop kills agent daemons/children, closes
      sessions, cleans the socket, and persists session state first.
    - Performance: idle-check is a cheap periodic tick (≥ 1s interval),
      not a busy loop.
    - Code Quality: policy table (clients × runs → keep/exit) unit-tested
      exhaustively.
    - Security: explicit stop requires the same local-socket authority as
      every other command; no new remote surface.
  - Approach:
    - Documentation Reviewed: herdr shutdown paths
      (`headless.rs`: SIGINT → `initiate_shutdown` → drain → save),
      clay-agent daemon teardown (server.rs agent child actor) for child
      cleanup ordering.
    - Options Considered:
      - No idle exit (server resident forever after first launch) —
        rejected: orphaned idle daemons leak resources and confuse
        upgrades; herdr accepts explicit-stop-only because PTY panes are
        always "work", Clay sessions can be truly idle.
      - Grace-period idle exit — chosen.
    - Chosen Approach: server-side idle monitor actor + client-count from
      the connection registry + active-run registry; explicit stop flows
      through existing shutdown machinery with save-first ordering.
    - Files to Create/Edit: server-side idle monitor (clay server
      crate), `src-tauri/src/server.rs` (stop paths), frontend quit menu
      action (minimal wiring; full UI belongs to 148's shell).
    - References: decision 1908 consequences.
  - Test Cases to Write:
    - Policy table: no clients + no runs → exit after grace; run active →
      never; approval pending → never; client connects mid-grace → timer
      cancels.
    - Explicit stop kills daemon children and cleans the socket file.

- [ ] Stale-endpoint and version-mismatch handling under detachment
  - Acceptance Criteria:
    - Functional: stale socket file (dead server) → probe NotListening →
      clean re-spawn (existing path verified under detach); incompatible
      running server → typed Disconnected status with user action (stop
      the old server), and the explicit-stop surface can target it;
      zombie/duplicated server detection (PID file or socket-credential
      check) documented and tested.
    - Performance: probe timeouts unchanged.
    - Code Quality: failure modes enumerated as typed statuses, not
      strings.
    - Security: socket-credential check (SO_PEERCRED / named-pipe
      equivalent) ensures the adoptable server belongs to this user.
  - Approach:
    - Documentation Reviewed: herdr
      `validate_running_server_compatibility` + version-refusal UX;
      existing `probe_protocol` and `PROTOCOL_VERSION` handshake.
    - Options Considered:
      - PID file only — rejected: racy after crashes; credential check on
        the live socket is authoritative.
    - Chosen Approach: keep probe; add credential check + typed
      mismatch UX (status line + action).
    - Files to Create/Edit: `src-tauri/src/server.rs`, status DTO if a
      new state is needed.
    - References: herdr protocol_guard.rs.
  - Test Cases to Write:
    - Incompatible-server adopt attempt → typed status, no spawn loop.
    - Stale socket → respawn works.

- [ ] Running-agents inventory surface
  - Acceptance Criteria:
    - Functional: a new bridge command (frontend-facing, existing
      session-management surface pattern) returns the cross-workspace
      inventory: workspace root + display name, session id, agent id,
      run status (`working` | `blocked` | `idle`), pendingApproval flag,
      last-activity timestamp; a subscription delivers live deltas
      (session open/close, run start/end/steer, approval raised/cleared,
      status transitions). Blocked = approval pending or run waiting;
      working = run in flight; idle = otherwise.
    - Performance: inventory payload bounded (< 64 KB for realistic
      session counts); deltas are per-transition events, not snapshots.
    - Code Quality: reuses existing event-pump/envelope machinery
      (`src-tauri/src/bridge/`), one mapping site; DTOs through the ts-rs
      layer.
    - Security: no prompt content or tool output in inventory rows —
      metadata only (consistent with redaction posture).
  - Approach:
    - Documentation Reviewed: `src-tauri/src/bridge/session.rs` (event
      pump, envelopes), daemon `session.list`/run state events, snapshot
      status fields already projected.
    - Options Considered:
      - Frontend polls each workspace session — rejected: cross-workspace
      view must not scale with open tabs; server is the single owner of
      truth.
      - Server-side inventory + subscription — chosen.
    - Chosen Approach: aggregate in the clay server (it owns sessions);
      bridge command + subscription; status derivation from existing run
      state + pendingApproval (this is the "state rollup" previously
      deferred — now required).
    - Files to Create/Edit: clay server session-inventory module, bridge
      command + DTO, ts-rs regen.
    - References: decision 1908; plan 151 (consumer); herdr sidebar
      agent-state model (working/blocked/done at a glance).
  - Test Cases to Write:
    - Derivation unit tests: run active → working; approval raised →
      blocked; cleared → working; run end → idle.
    - Subscription delivers deltas in order for a scripted session
      lifecycle.

- [ ] Adopt-on-launch UX: relaunch restores running agents
  - Acceptance Criteria:
    - Functional: on GUI launch with a running server, the app connects
      (existing adopt path), requests the inventory, and existing
      frontend session-reclaim machinery (`connect_for_reclaim_or_new`)
      is extended so previously-open agent sessions reattach with live
      transcripts (no data loss, no duplicate sessions); sessions the
      user had not open remain visible via the inventory (surfaced by
      plan 151's Agents navigator; until 151 lands, a minimal list in the
      existing UI proves the capability).
    - Performance: relaunch to interactive inventory < existing bootstrap
      budget.
    - Code Quality: reclaim keys (tab/session) unchanged in shape.
    - Security: reclaim still refuses sessions not owned by this
      endpoint/workspace binding.
  - Approach:
    - Documentation Reviewed: `connect_for_reclaim_or_new`,
      `session_bootstrap`/`session_subscribe` commands.
    - Options Considered: none beyond reuse.
    - Chosen Approach: extend reclaim with inventory-driven bootstrapping;
      minimal UI proof now, full navigator in 148.
    - Files to Create/Edit: `src-tauri/src/bridge/session.rs`, frontend
      bootstrap glue, minimal list surface.
    - References: plan 151.
  - Test Cases to Write:
    - Integration: run active → GUI "restart" (reconnect) → transcript
      intact, run continues streaming.

- [ ] Configuration APIs + documentation for new knobs
  - Acceptance Criteria:
    - Functional: idle grace period and log-rotation cap are documented
      configuration keys following existing config-key conventions;
      `CLAY_ENDPOINT`/`CLAY_SERVER_BIN` behavior unchanged and documented
      for the detached model.
    - Code Quality: coverage gates for undocumented config APIs green.
    - Security: no key can weaken socket permission checks.
  - Approach:
    - Documentation Reviewed: `references/clay.md` Clay Configuration
      Task; existing config-key documentation set.
    - Chosen Approach: verification + the two new keys.
    - Files to Create/Edit: config docs, coverage tests.
  - Test Cases to Write: config coverage tests updated.

- [ ] Create or verify Clay JS APIs for public programmatic surfaces
  - Acceptance Criteria:
    - Functional: any new JS-reachable surface (inventory command, stop
      action) gets an op + facade + docs per the dotted-ID convention or
      stays internal; explicit-stop is deliberately **not** exposed to
      package JS in this plan (shell/UI action only) — recorded decision.
    - Code Quality: doc-registry gates green.
    - Security: packages cannot stop the server or enumerate cross-workspace
      inventory without the documented permission surface.
  - Approach:
    - Documentation Reviewed: `.agents/skills/clay-execution/references/js-api.md`.
    - Chosen Approach: verification-first.
    - Files to Create/Edit: as found.
  - Test Cases to Write: as found.

- [ ] Live launch-test (scratch profile, kill/idle drills)
  - Acceptance Criteria:
    - Functional: scratch-config GUI run: (1) prompt a session with the
      mock provider, quit the GUI, verify server+daemon alive and run
      completes; (2) relaunch, verify inventory lists the session idle
      and reclaim reattaches; (3) leave idle → server self-exits after
      the (test-shortened) grace; (4) explicit stop kills everything
      cleanly. Evidence recorded.
    - Performance: startup within existing gates.
    - Code Quality: drill log in task evidence.
    - Security: scratch HOME only.
  - Approach:
    - Documentation Reviewed: `references/clay.md` Live Launch-Test Task.
    - Chosen Approach: manual drills mirroring Expected Outcome.
    - Files to Create/Edit: none.
  - Test Cases to Write: none.

- [ ] Code-wiki and documentation truth update
  - Acceptance Criteria:
    - Functional: wiki lifecycle pages document the detached model
      (adopt-or-spawn, idle policy, explicit stop, inventory), inverted
      test expectations noted; truth checks pass.
    - Code Quality: docs-as-code workflow.
    - Security: none.
  - Approach:
    - Documentation Reviewed:
      `.agents/skills/clay-execution/references/docs-as-code.md`.
    - Chosen Approach: final wiki task.
    - Files to Create/Edit: `docs/wiki/` affected pages.
  - Test Cases to Write: existing truth checks.

## Compromises Made

- To be filled after tasks are completed and tests pass.

## Further Actions

- To be filled after task completion. Known at planning time: Windows
  detach parity (non-required target — helper isolated for later); a
  headless "clay server" CLI surface for power users/service wrappers;
  inventory enrichments (vendor runtime kind once plan 152/153 land).
