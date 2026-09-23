# Plan 136 task 12 — live launch of the canonical example configuration

A real Linux GUI build (debug `clay server` + `clay client`, Tauri desktop)
launched against a **copy** of `examples/config/` in a scratch config root.

## Launch

```bash
run-live.sh start example-config      # harness mode added by this task
drive-example-config.sh               # focus, menu, command, pane, file browser
run-live.sh stop                      # writes perf/clay-server-perf-summary.json
```

- Harness mode `example-config` (`test-plan/artifacts/127-lane-scheduling/run-live.sh`)
  copies `examples/config/.` verbatim into the scratch `HOME/.clay/` — the same
  `cp -r examples/config/. ~/.clay/` the guide documents.
- Isolation: private mode-700 root `/tmp/clay-plan136-example` holding `HOME`,
  `XDG_CONFIG_HOME`, `XDG_DATA_HOME`, `TMPDIR`, the package store, the workspace,
  and a private socket `clay.sock`. The developer's `~/.clay` is never opened, no
  credentials are involved (the canonical config sets no provider), and the store
  starts empty — the third-party module is a commented template, so nothing is
  installed, adopted, or granted.
- Driver (`drive-example-config.sh`) focuses the window through the mango
  compositor IPC and refuses to synthesize input unless `clay-desktop` owns
  keyboard focus; screenshots use the portal capture path, cropped to the window.
  `probe.py` gained a `click <name>` command (AT-SPI action press) for the
  header button; the rest of the interaction is real key input.

## Observed

| Check | Evidence |
| --- | --- |
| Client reaches **Connected** | `window-initial.png` status bar `workspace notes.md v1 clean Connected`; `client.log` has only GTK locale warnings |
| Configuration evaluation commits, no `configuration failed` diagnostics | `config-failure-count.txt` = 0 lines matching `configuration failed\|configuration.module_failed\|load_failed\|MissingCapabilityGrant\|panicked`; `server.log` has the `[agent-reg]` lines that only run once config JavaScript evaluates |
| Config effects are live (positive proof, not just absence of errors) | theme `@clay/theme-gruvbox-material-dark`, `@clay/markdown` prose mode with a 200-entry outline, coding-agent lane (`Ask, or type / or @`), file browser, and the example's chord hints — all in `window-initial.png` |
| **Open the menu** | header "Control Center" button pressed via AT-SPI (`click-control-center.txt`) → palette with the command list (`window-menu.png`, `tree-menu-hits.txt`: `dialog|Commands` + list box) |
| **Run a command / open a pane** | palette filtered by typing `equal` to the selected row `Add Equal Pane client — built-in Ctrl+Shift+\` (`window-menu-filtered.png`, `tree-menu-filtered-hits.txt`); the row's own chord ran it → pane split (editor extents `1322x14553` → `763x14553`, pane nodes 5 → 8; `window-pane.png`, `tree-after-pane.txt`) |
| Example config's own binding | `Ctrl+B` → `workspace.toggleFileBrowser` toggled the browser (node `Filter files` 1 → 0; `window-file-browser.png`, `tree-after-files.txt`) |
| Shell edit path round-trips to disk | typing ` ok` (chars 243 → 246) + toolbar `Save` (AT-SPI press, `click-save.txt`) → `notes.md` on disk begins ` ok# heading 0` with a fresh mtime (`window-after-save.png`) |

## Performance

`startup-timing.txt` (fresh-launch wrapper, window on the active compositor tag):

```
server.socket_ready_ms=429
config.first_effect_ms=478   ([agent-reg] line: config JavaScript ran)
client.editor_visible_ms=1301
```

`perf-summary-example-config.txt` (server perf report on SIGTERM):

| Metric | Value |
| --- | --- |
| `runtime.load_configuration_with_workspace` | 1 call, p50 67.9 ms — the canonical config's whole evaluation, bundled package loads included |
| `runtime.evaluate_controlled_module` | 1 call, 1.9 ms |
| `server.edit_ack` | 3 edits, p50 194.7 µs / p95 342.5 µs (plan 136 task 1 baseline: p50 231.8 / p95 282.1 µs) → no regression |
| retained / dropped events | 282 / 4096, 0 dropped |

No baseline *startup* number was ever recorded for this plan (task 1 recorded the
edit-ack envelope), so the copy's startup is recorded fresh above; its config
evaluation is the same order as the plan-127 live runs, which also load bundled
packages.

Caveat: the same-harness `markdown` control mode did not expose its window to
AT-SPI inside the poll window because its client window was not on the active
compositor tag — a harness artifact (the driver switches tags, the timing wrapper
does not), not a startup cost. Startup numbers here are therefore the
example-config run's own, measured with the window visible.

## Findings (pre-existing; no plan 136 code is involved)

Both were observed in the live session and are recorded for a follow-up rather
than fixed here (plan 136 changes no menu or chord code):

1. **Two-stroke chord double-dispatch.** With editor focus, `Ctrl+X Ctrl+O`
   resolves the palette *and* lets the single-stroke `Ctrl+O` binding fire as
   well (`documents.clientOpenFileDialog` → a file chooser opened on top; status
   bar `File dialog could not open` after the portal dialog was killed).
   Reproduction: focus the editor, `wtype -M ctrl -k x -m ctrl`, `sleep 0.3`,
   `wtype -M ctrl -k o -m ctrl`.
2. **Palette row activation.** Enter on the selected palette row (footer says
   `↵ run`) left the palette open and ran nothing; the status bar reported
   `no active menu session for id 9223372036854775809 (client 2)`. The row's own
   chord works, which is what this task used.
