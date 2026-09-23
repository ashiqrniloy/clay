# Plan 127 lane-scheduling manual artifacts (2026-09-19, task 7)

Evidence and harness for the plan 127 manual steps: module **04 E40** (typing and
completion responsiveness while a bundled package parse handler works a large
document), module **09 P56** (an adopted third-party package whose declared
capability has no grant surface never executes), module **11 Q42** (perceived
typing latency with packages active, measured).

## Conventions and sources

- AT-SPI probe `probe.py` (`dump`, `editor`, `focus`, `click`, `insert`, `completion`, `wait-ready`)
  and `portal-shot.py` (portal screenshot cropped to the `clay-desktop` window)
  are copied from the plan 126 artifact set: `test-plan/artifacts/126-access-paths/`.
  Crop bounds come from the computer-use-linux GNOME Shell extension; activate
  the Clay window before capturing, otherwise the crop can contain another
  window (recorded ceiling from plan 126).
- Input synthesis uses the xdg-desktop-portal remote-desktop keyboard session
  through `computer-use-linux` (`activate_window`, `type_text`, `press_key`).
  `Ctrl+Space` is consumed by GNOME input-source switching on this host, so the
  fixtures also bind `Ctrl+J` to `completion.trigger`; the `.` autocomplete
  trigger of `@clay/rust` works unmodified. Plan 136 update: a *package-owned*
  mode does not route the built-in `completion.trigger` command (the client
  rejects it as an unknown command there), so `@fixture/lane` declares its own
  `.` trigger in its manifest instead of relying on a bound chord.
- `run-live.sh` isolation mirrors `scripts/capture-ui-review.sh`: private
  mode-700 root for `HOME`/`XDG_CONFIG_HOME`/`XDG_DATA_HOME`/`TMPDIR`, private
  server socket, fixture workspace, tree-kill teardown. `CLAY_PERF_PROFILE=1`
  and `CLAY_PERF_REPORT_DIR` are wired so `stop` (SIGTERM) writes
  `clay-server-perf-summary.json`.

## Modes

```bash
test-plan/artifacts/127-lane-scheduling/run-live.sh start completion   # @clay/rust, 64 KiB review.rs
test-plan/artifacts/127-lane-scheduling/run-live.sh start markdown     # @clay/markdown, 1 MiB notes.md
test-plan/artifacts/127-lane-scheduling/run-live.sh start fixture      # @fixture/lane, grant-gate check
test-plan/artifacts/127-lane-scheduling/run-live.sh status | stop
```

## Fixture packages

`fixture-package/` (`@fixture/lane`) registers a deliberately slow parse handler
for its own `lane` mode (`parser.js`, `clayLaneBusyMs`, default 1000 ms hold on
the third-party **general** lane) plus a **module-backed** completion provider
(`provider.js`, `moduleSpecifier: import.meta.resolve("./provider.js")`), which is
the shape plan 127 routes to the **latency** lane.
`fixture-package-inline/` (`@fixture/laneblocked`) is the A/B half: same slow
parse handler, but the provider is registered as a module object, so it lives in
the general lane and must wait for the handler to release it.

Plan 136 update (2026-09-23): plan 136 task 5 added the `clay package authorize`
surface and task 6 drove both fixtures through it. `fixture-package/load.js` now
registers its own `lane` mode pattern (`serverRegisterModePattern`, the
`mode-registration` grant) because the host applies package manifest
contributions only for trusted/bundled records, passes `module` **and**
`moduleSpecifier` to the completion registration (a specifier-only call
registers nothing), and `package.json` declares the `.` trigger
(`modePatterns[].editorRules.autocompleteTriggers` +
`completionProviders[].triggerCharacters`). The granted/ungranted live modes,
the driver script, and all of that evidence are in
`test-plan/artifacts/136-capability-grants/`.

These fixtures are ready to use the moment a capability-grant surface exists;
they cannot be *enabled* today (see the ceiling below), which is why the live
lane A/B is not part of the execution record.

Store note: Clay v1 `clay install` accepts only `npm:` specs
(`PackageSpec::parse` rejects every other source family), so `run-live.sh` seeds
the store with the same command shape the pnpm backend runs — `pnpm add <path>`
inside `~/.clay/packages` — and then uses Clay's own authority verbs
(`clay package inspect`/`adopt`/`enable`). The `loadPackage` line in
`init-fixture.js` mirrors the self-removing block `clay install` appends. The
host's `corepack`-cached pnpm is exposed to the isolated `HOME` through a small
shim (`$root/bin/pnpm`) with `COREPACK_HOME` pointed at the host cache.

## Reachability ceiling recorded by this task

Plan 136 update (2026-09-23): the grant half of this ceiling is closed —
`clay package authorize` (plan 136 task 5) records `parse-document`,
`completion-provider`, and `mode-registration` for `@fixture/lane`, and the
package then enables and loads in its own third-party domain. What remains
unreachable live is mode *activation*: third-party manifest contributions are
applied only for trusted/bundled records, and nothing in the host activates a
registered mode for an open document (the only caller of
`modes.activate_major_mode` is the `clay:modes` JS op, which needs an already-open
`documentId`, and there is no document-open hook). The plan-136 artifact README
records that ceiling and its evidence.

Capability grants for non-bundled packages are recorded by
`PackageService::authorize_package`, which has no CLI, desktop, or JS surface in
this build (bundled packages use `authorize_bundled_defaults`; language servers
use `authorizeLanguageServer`). Consequences:

- A live third-party parse handler or completion provider cannot be registered,
  so the plan's "completions remain responsive while a package parse handler
  runs" step is live for the typing half (bundled `@clay/markdown` handler) and
  automated-only for the provider-lane half
  (`latency_lane_unblocked_by_busy_general_lane` and the rest of the plan 127
  scheduling suites; baselines in `code-reviews/2026-09-18-plan127-baseline/`).
- No bundled package registers a JS completion provider either; the completion
  items seen live come from the built-in host-side Rust provider.

## Evidence directories

| Directory | Contents |
|---|---|
| `grant-gate-live/` | `enable.log` (`MissingCapabilityGrant`), `server.log` (sanitized `packages.load_failed` diagnostic), `tree.txt`, `window.png` (`demo.lane` open, 9 typed characters, no fixture mode/handler/provider). |
| `live-completion/` | `window-popup.png` (popup open over the Rust document), `tree-popup.txt` (`Completions` list box with the `rust` group rows), `server.log`, `client.log`, `clay-server-perf-summary.json`. |
| `live-markdown-parse-handler/` | `window-typed.png` (1 MiB `notes.md` after typing), `tree.txt`, `server.log`, `clay-server-perf-summary.json` (measured `server.edit_ack` p50 0.33 ms / p95 0.52 ms; `syntax.*` background parse counters). |