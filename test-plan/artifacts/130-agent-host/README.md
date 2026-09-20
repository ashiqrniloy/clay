# Plan 130 agent-host manual-test-plan artifacts (2026-09-20)

Evidence behind the "Plan 130 agent-host decomposition execution record" in
`test-plan/index.md` and the A21/A22 rows of module
[16](../../16-agent-host.md) / the C43 extension in module
[17](../../17-coding-agent-parity.md).

Plan 130 was a pure ownership/decomposition change (agent host authority
injected into runtime lanes, `ClayAgentHost` split into
`clay-agent/src/host/*.ts`, no function above the 80-line budget) plus one
behavioral fix: a resumed coding session re-activates its workspace's
capabilities and graft binding (A2). Only that fix is user-visible, so the pass
is a regression pass plus the resume-binding step.

## Build

| Artifact | Command | Stamp |
|---|---|---|
| `target/debug/clay` | `cargo build --bins` | 2026-09-20 19:36 |
| `target/debug/clay-desktop` | `cargo build --bins -p clay-desktop` | 2026-09-20 19:37 |
| `clay-agent/dist` | `npx tsc -p clay-agent/tsconfig.json` | 2026-09-20 19:37 |
| `frontend/dist` | unchanged (no frontend source newer than the 2026-09-18 build; `git status frontend/src` clean) | 2026-09-18 19:12 |

Host: Linux, GNOME/Wayland, Node v24.19.0, `graft` peer `@nanonets/graft`
installed in the Node global prefix (`resolveGraftCli` reaches it through the
module graph, which is why the daemon probe sets `NODE_PATH`).

## Live daemon protocol drive — `live-daemon.mjs`

Two real `clay-agent` processes over one isolated data dir (`/tmp/clay-plan130-live`),
driven through the daemon's own ndjson JSON-RPC — no test harness, no mock host
object. The first runs in `cwd-a` and creates a coding session rooted in this
repository (a real graft workspace) plus one session with **no** root (the
documented `process.cwd()` fallback, which is what makes the second process's
root assertion meaningful). The second process starts in an unrelated `cwd-b`
and resumes the recorded session.

| File | Contents |
|---|---|
| `live-daemon.log` / `live-daemon.json` | the 24 legs, all PASS (raw log + structured summary) |
| `live-daemon-falsify.log` / `live-daemon-falsify.json` | same probe with the A2 activation removed from `ensureLive`: the three re-activation legs fail, everything else still passes |

Covered live: initialize (+ server-style MCP allow-list), profile
registration, lazy capability state before any session, session create with a
recorded root, graft skill + graft tool + MCP tool in the created session's tool
list, prompt to `agent_finished`, workspace-scoped `session.search` (hit and
no-hit), `session.resumable` (own root vs another root), `session.clone`,
`session.fork`, `session.setAutonomy` both ways, `session.compact` +
persisted compaction entry, the cwd fallback, and after the restart:
`session.resume` reporting the **recorded** root, the graft skill, the graft
extension in `environment.list`, the re-connected MCP server
(`{"serverId":"live","connected":true,"tools":2}`), system-prompt layers, the
autonomy toggle on the resumed session, and a follow-up prompt.

The pass found two autonomy inconsistencies (see the module 16 record):
creation is **opt-out** (`fullAutonomy !== false`) while the docs/inventory then
documented `default:boolean=false`, and a resumed session came up
non-autonomous because `ensureLive` wrote its live record with
`fullAutonomy: false` even though the session itself was created from the
recorded value.

**Resolved 2026-09-20** — user decision
`decision-logs/2026-09-20-2049-agent-autonomy-default-on-and-resume-restores-recorded-autonomy.md`:
autonomy stays the default and the docs moved to it
(`default:boolean=true`), and `ensureLive` now restores the autonomy recorded
with the session, so a block survives a daemon restart. The inventory gate pins
both the documented default and the daemon expressions, and the resume suite
carries `resumed session keeps a recorded autonomy block (approvals stay
armed)`. Raw transcripts below (`live-daemon.log`, `config-legs.txt`) are the
recordings of the original pass and keep their original text.

## Live GUI pass — `gui-live.sh` + `probe.py`

Isolated launch (mode-700 root `/tmp/clay-plan130-gui`, canonical
`examples/config` as `~/.clay`, scratch workspace, private socket, desktop
client) with `CLAY_AGENT_MOCK=1`; every step driven by AT-SPI actions, no input
synthesis.

| File | State | Observation |
|---|---|---|
| `gui/tree-01-rest.txt` | lane at rest | 1 `Agent lane` footer, `Message` entry, `Coding Agent Agent type` combo box, 0 palette rows |
| `gui/tree-02-palette.txt`, `gui/rows-palette.txt` | palette open | 97 rows including all daemon slash commands (`/branch /clone /compact /discard /fork /n /new /open-session /open-session-as-fork /resume /tree`, each `server-first — @clay/coding-agent@0.1.0`) |
| `gui/rows-session-scope.txt` | `Session` scope chip pressed | 14 daemon rows, no shell/file rows |
| `gui/tree-04-lane-hidden.txt` | `hide lane` pressed | 0 `Agent lane` nodes, 0 agent rows |
| `gui/tree-05-lane-restored.txt` | lane toggled back | `Agent lane` and `Message` back |
| `gui/actions.txt` | — | each AT-SPI `action press … -> True` with the verified tree consequence |
| `gui/server.log` | — | `[agent-reg] … host=live -> Ok({"queued": true})` per registration, then `[daemon] agentProfile.register 'coding' applied` — the lane's injected host (plan 130 A1 ownership) answering, with the daemon applying the declaration once up — and `[agent] ensure_tab_session(1): empty selection (provider='' model='')` |

### No screenshots in this record (host ceiling + privacy rule)

The portal screenshot surface on this host exposes only the **currently
visible** workspace/monitor, and the isolated client opens on another
workspace. The first crop written by the plan-126/129 `portal-shot.py` copy
therefore contained an unrelated desktop window (the developer's browser). It
was deleted before anything entered the repo; no image from that run is
retained. `portal-shot.py` in this directory is the hardened version: it crops
from Clay's live AT-SPI frame, trusts a compositor rect only when it lies
inside that frame, and **fails closed (exit 2) without writing a PNG** when the
geometry does not agree or the crop escapes the screenshot — it refused every
capture attempt in the final run, recorded in `gui/actions.txt`.

Follow-up recorded on the plan: the copies in
`test-plan/artifacts/126-*/`, `127-*/` and `129-*/` still trust the compositor
listing alone and can retain another window's pixels; this directory's version
is the one to copy forward.

Interactive steps that need a keyboard/pointer or a configured provider stay
**UNRESOLVED** on this host (standing ceilings: no `/dev/uinput`, no
`xdg-desktop-portal` key delivery without the crash loop recorded by plan 129;
no provider credentials in an isolated profile, so the lane sits in the
`no provider configured` state): typing a composer prompt, sending it,
approval/interrupt drill, creating/resuming a session from the lane, and
activating a `/resume` row.

## Fresh automated regression on the crafted tree

`automated-legs.txt` — the daemon and Rust suites cited by the manual steps, on
the same built tree.
