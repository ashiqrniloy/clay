---
date: 2026-09-23 19:08
status: approved
decision_about: "Agent runtime detachment: clay-server survives Clay app exit; agents keep running; relaunch attaches and lists all running agents"
proposed_by: user
explicitly_approved_by_user: true
---

# Decision: Detached Agent Runtime Server (herdr-style persistence)

## Decision

The Clay agent runtime runs as a **detached server**: closing the Clay app
must not kill a running agent, and launching Clay attaches to the running
server and shows all running agents. Clay adopts the herdr model at the
capability level — background server owns agent processes, GUI is a client
— while keeping Clay's own implementation shape (one `clay-server` sidecar
per user session owning per-workspace sessions, the clay-agent daemon, and
future external runtimes). Implementation approach is ours to choose; the
capability is mandatory.

## Context

- Inspired by herdr (`github.com/herdrdev/herdr`): its default launch
  probes the session socket; if a server is listening it attaches as a thin
  client, otherwise it spawns `herdr server` **detached** (stdio → null,
  platform detach), waits for socket readiness, then attaches. The
  headless server owns PTYs/panes/workspaces and runs until explicit stop
  or host shutdown; closing all clients leaves work running.
- Clay today: `Supervisor` (`src-tauri/src/server.rs`) already does
  adopt-or-spawn with a protocol probe (Compatible → adopt,
  Incompatible → fail-closed typed status, NotListening → spawn
  `clay-server` sidecar). But the sidecar is a regular child: `Drop`,
  `shutdown()`, and `RunEvent::Exit` in `src-tauri/src/lib.rs` kill+reap it,
  so GUI exit kills the server, the clay-agent daemon, and every running
  agent. Workspaces already multiplex inside one server
  (`bridge/session.rs` connects per workspace root over one endpoint), and
  session reclaim after webview reloads already exists
  (`connect_for_reclaim_or_new`).

## Approval

- Proposed by: user.
- Approved by user: Yes
- Approval evidence: "the clay agent runtime must be running as a server.
  Meaning that closing clay should not kill a running agent and launching
  clay I should be able to see all the running agents in the server. Check
  the implementation of herdr to understand what it is doing. We do not
  necessarily have to do the implementation in the same way, but the
  mentioned capability has to be there." (2026-09-23)

## Alternatives Considered

1. **Keep server as GUI child; persistence via resume-from-disk** —
   rejected: an interrupted agent run is not a running agent; the user
   requirement is continuous execution.
2. **One user-level systemd-style service always running** — rejected for
   now: launch-on-demand (connect-or-spawn) matches herdr, avoids
   install-time service registration, and keeps dev/CI simple. A system
   service can wrap the same binary later if headless-box usage appears.
3. **One detached server per workspace** — rejected: Clay already
   multiplexes workspaces in one server; per-workspace processes multiply
   lifecycle surface for no isolation gain (workspace isolation is
   enforced inside the server today).

## Rationale and Evidence

- herdr proves the model at scale (PTY persistence, multi-client attach,
  version-mismatch refusal); Clay's Supervisor already implements
  adopt-or-spawn, so the delta is bounded: detach the spawn, stop killing
  on exit, add an idle self-exit policy, and expose a running-agents
  inventory.
- One-server-per-user matches the existing single endpoint
  (`desktop_endpoint()`, `CLAY_ENDPOINT`) and the existing
  one-daemon-per-server clay-agent ownership; external runtimes (plan 153)
  become server children and inherit persistence for free.

## Consequences

- `Supervisor` spawn becomes detached (new session/process group, stdio to
  a rotating log file, no kill-on-GUI-exit); the server gains an idle
  policy: self-exit after a configurable grace period with **no connected
  clients and no active agent runs**; explicit stop via UI ("Quit Clay &
  stop agents") and CLI-equivalent surface.
- The server exposes a running-agents inventory (per workspace: session,
  agent, run status working/blocked/idle, pending-approval flag) consumed
  by the Agents navigator (apps/splits shell decision, same date) and
  future state rollups.
- Stale sockets and incompatible-version servers get typed handling
  (probe already distinguishes; detach changes failure modes — plan 150
  owns the details).
- Multi-window/multi-instance Clay attaches to the same server; the server
  is the single owner of agent processes and truth.
- Security posture unchanged: same socket permission discipline, same
  protocol handshake; a detached process must never weaken the
  deny-by-default child-spawn rules.
- Plans: `plans/150-Detached-Agent-Runtime-Server.md` implements;
  `plans/151-…` consumes the inventory; plans 149–153 sequencing shifts to
  149 → 150 → 151 → 152 → 153.
- Revisit when: remote/headless boxes justify a system service, or agent
  resource contention demands per-workspace process isolation.
