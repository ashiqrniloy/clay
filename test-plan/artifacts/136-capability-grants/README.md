# Plan 136 task 6 artifacts: third-party grant → provider → latency lane

Evidence that a *third-party* package reaches the plan-127 lane path through the
plan-136 grant surface, and a record of the one part of that path the live client
still cannot reach.

The fixture package is plan 127's `@fixture/lane`
(`test-plan/artifacts/127-lane-scheduling/fixture-package/`, reused per this
task's approach note): a slow parse handler plus a `moduleSpecifier` completion
provider. The plan-136 artifact directory adds the granted/ungranted live configs
and the one driver script that produces the live evidence.

## Fixture changes made by this task

| File | Change | Why |
| --- | --- | --- |
| `fixture-package/load.js` | registers its own mode pattern with `serverRegisterModePattern` | the host applies package manifest contributions only for trusted/bundled records (`src/server/ops/packages.rs:736-738`), so a third-party `modePatterns` contribution alone never registers the `lane` mode |
| `fixture-package/load.js` | `serverRegisterCompletionProvider` now passes `module` **and** `moduleSpecifier` | specifier-only registers nothing: the facade only creates a JS registration when `module !== undefined` (`runtime/js/completion.js:26`), and only a specifier-marked provider is materialized by the latency lane |
| `fixture-package/package.json` | declares the `.` trigger (`modePatterns[].editorRules.autocompleteTriggers` + `completionProviders[].triggerCharacters`) | the client classifies trigger characters from its manifest layer (`frontend/src/editor/extensions/completion.ts:73`), and the server selects providers by trigger character (`src/server/completion.rs:627`) |

## Modes

```bash
run-live.sh start granted-lane     # install → adopt → authorize(3 caps) → enable → server + client
run-live.sh start ungranted-lane   # install → adopt → enable must fail closed (plan 127 negative check)
drive-granted-lane.sh              # focus, type, trigger, capture (granted-lane only)
run-live.sh stop                   # SIGTERM writes perf/clay-server-perf-summary.json
```

`drive-granted-lane.sh` drives the live step in one shot: it focuses the Clay
window through the compositor's own IPC, refuses to synthesize input unless the
compositor reports `clay-desktop` as the focused client, types an edit, types the
fixture's `.` trigger, and captures the AT-SPI popup tree plus a portal screenshot
cropped to the window. Store seeding and the `pnpm add` shape come from
`run-live.sh` unchanged.

## Evidence index

| Artifact | Shows |
| --- | --- |
| `automated-lane/lane-measurement.txt` | `PLAN136_GRANTED_LANE busy_ms=500 idle_median_us=2281 busy_completion_us=3572 workers_started=4` against the plan-127 envelope (`idle 1567 / busy 2594`), plus `granted_third_party_provider_serves_from_latency_lane_when_general_lane_busy` and `ungranted_third_party_provider_still_fails_closed` passing (15/15 lane tests) — completion keeps answering in ~3.6 ms while the package parse handler holds the general lane for 500 ms, and the answer carries the fixture's package provenance |
| `live-granted-lane/authorize.log`, `enable.log`, `inspect.log` | the host CLI grant surface on the real package: `completion-provider, mode-registration, parse-document (native-trust)`, `Granted by: cli`, `Approved by: cli`, then `Enabled @fixture/lane` |
| `live-granted-lane/perf-summary.json` | the granted server really loaded the package: `runtime.load_in_package_domain` = 1 (third-party package domain) with the config evaluation separate, and `server.edit_ack` = 6 for the typing the driver synthesized |
| `live-granted-lane/editor-before.txt`, `editor-after.txt` | typing landed in the granted document: `chars=29` → `chars=35` (` lane.`) in `demo.lane` |
| `live-granted-lane/focus.json`, `focus.txt` | compositor focus was `clay-desktop` (id 101, pid matches the harness client) before any input was synthesized |
| `live-granted-lane/window-popup.png`, `tree-popup.txt` | the negative half of the live step: no completion popup appeared (`no completion surface or status text`); see the ceiling below |
| `live-ungranted-lane/enable.log`, `inspect.log`, `server-load-failed.txt` | without the grant the same fixture still fails closed — `MissingCapabilityGrant { capability: CompletionProvider }`, `Ungranted: completion-provider, mode-registration, parse-document`, server `packages.load_failed` |
| `lane-occupancy/live-server-only-summary.json` | plan 136 task 7: a real `CLAY_PERF_REPORT_DIR` summary from a granted-lane server run with **no** client — `js_runtime.lane.trusted.general.dispatched` 1, `trusted.latency` 0, `third_party.general` 5, `third_party.latency` 3 (load-time registrations, the same 3 as the client run, so no client-driven completion dispatch reaches either lane) |
| `lane-occupancy/live-with-client-summary.json` | the same config with the client connected: only `trusted.general` moved (1 → 2, the connect-time config reload) |
| `option-surface/README.md` | plan 136 task 8: the option-surface guard (236 declared keys / 59 APIs), the delete-vs-document audit, and the two mutation checks |
| `option-surface/mutation-1-drop-documented-key.txt`, `mutation-2-redeclare-ignored-key.txt`, `guard-pass.txt` | the guard failing on both mutations (dropped documented key, re-declared ignored key) and passing after the reverts |
| `option-surface/gate-check-full.log` | `scripts/check.sh full` PASSED (exit 0, nine stages; protocol 228 including the new guard) |
| `option-surface/gate-check-full-task9.log` | the same gate after task 9's page hot-path paragraph and the two `pub(crate)` tightenings (protocol 228, security 162, lib 1435) |
| `config-surface/README.md` | plan 136 task 10: the grant as a configuration surface (guide section, ordering, self-grant refusal) |
| `config-surface/mutation-self-grant-gate-removed.txt` | deleting the op's activation gate makes the self-grant test fail with `self-granted:true` (reverted) |
| `config-surface/gate-check-full-task10.log` | `scripts/check.sh full` PASSED (exit 0, nine stages; protocol 229, lib 1436) |
| `example-config/README.md` | plan 136 task 11: the canonical example tree's commented capability-grant section |
| `example-config/node-check-and-active-lines.txt` | `node --check` for `init.js` + the third-party template, and the template's only uncommented line |
| `example-config/gate-check-full-task11.log` | `scripts/check.sh full` PASSED (exit 0, nine stages; protocol 229 incl. the example-config boot test) |
| `live-example-config/README.md` | plan 136 task 12: live GUI launch on a copy of `examples/config/` — Connected, config committed, menu/pane/command/file-browser interactions, startup + perf numbers, and the two pre-existing input findings |
| `live-example-config/startup-timing.txt` | socket 429 ms, first config effect 478 ms, editor visible 1.30 s |
| `live-example-config/perf-summary-example-config.txt` | `load_configuration_with_workspace` p50 67.9 ms, `edit_ack` p50 194.7 / p95 342.5 µs over 3 edits |
| `live-example-config/window-*.png` + `tree-*.txt` | screenshots and AT-SPI trees for the palette, the filtered row, the pane split, the file-browser toggle, and the saved edit |
| `manual-plan/README.md` | plan 136 task 13: manual-test-plan execution — module 09 P56/P57/P58 and module 11 Q44, with the store JSONs, per-verb logs, lane measurement, and the recorded ceilings |
| `code-reviews/2026-09-20-plan136-baseline/logs/gate-check-full-task7.log` | `scripts/check.sh full` PASSED (exit 0, nine stages; lib 1435 including the two lane-counter tests) |
| `code-reviews/2026-09-20-plan136-baseline/logs/gate-check-full-task6.log` | `scripts/check.sh full` PASSED (exit 0, nine stages; lib 1433 including the two plan-named lane cases, 9 allowed audit warnings unchanged) |

Performance note: the live measurement is the automated one above (the same code
path the client drives); the live run adds the real grant + package load. The
driver's AT-SPI/portal timings are not latency evidence — input synthesis,
capture, and AT-SPI polling are all slower than the completion round trip.

## Recorded ceiling: a package-owned mode never activates live

The live completion popup could not be produced, and the cause is a host gap, not
a harness problem:

- Third-party package manifest contributions are not applied — the host takes
  that path only for trusted/bundled records
  (`src/server/ops/packages.rs:736-738`), which is why the fixture now registers
  its own mode (`serverRegisterModePattern`) instead.
- Nothing in the host activates a registered mode for an open document: the only
  caller of `modes.activate_major_mode` is the `clay:modes` JS op
  (`src/server/ops/mod.rs:1104`), which requires an already-open `documentId`.
- There is no JS hook for document open (`runtime/js/documents.js` exposes only
  open/save/reload/list/snapshot) and no client/protocol message that activates a
  mode, so configuration code cannot activate the mode later either — it runs
  before any document exists (`runtime.load_configuration_with_workspace`).

Consequence: `demo.lane` never activates mode `lane`, so the package's parse
handler is never dispatched for an edit and the client never receives the
fixture's editor rules or trigger characters. What a third-party package *can*
do live today is load, run in its own domain, and register mode/metadata —
everything plan 136 task 5 made possible. The lane routing, provenance, and
latency criteria are therefore covered by the automated test (which drives the
same runtime path in CI), and the live step records the grant/load half plus
this ceiling. Plan 136 task 7's per-lane counters confirm the client half: a
live session with the client connected adds no dispatch to either latency lane
over a client-free run of the same config (`lane-occupancy/`), so no completion
request from the client ever reaches the fixture's provider.

## Environment deviations on this host

- `corepack` is not installed: `run-live.sh`'s `$root/bin/pnpm` shim now falls
  back to `/usr/bin/pnpm`, and the harness's own `pnpm add` / `pnpm list` calls
  use the shim instead of `corepack pnpm` directly (the old shape re-entered the
  shim and forked recursively).
- The compositor is mango (dwl), not GNOME: the GNOME Shell extension the
  plan-126/127 capture scripts assume is absent, so the driver focuses with
  `mmsg dispatch view/<tag>` + `focusstack,next` and captures with the portal
  `Screenshot` path (cropped by the compositor geometry, full-desktop capture
  deleted).
- Portal captures do not contain the editor's text layer (accelerated rendering),
  so `probe.py editor`'s `chars=` count is the readable signal for "the edit
  landed"; the document editor also exposes no `EditableText`, so the plan-127
  `wait-ready` probe never goes ready on this host.
- The fixture's own default hold (`parser.js`: `globalThis.clayLaneBusyMs ?? 1000`)
  applies live: configuration code cannot reach the package isolate's global. The
  automated test passes 500 ms explicitly, as the task's criterion states.
