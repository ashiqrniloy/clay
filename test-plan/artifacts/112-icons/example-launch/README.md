# Plan 112 Task 15 — example configuration live launch-test

Source revision: plan 112 working tree at task 15 (canonical example config from task 14).
All runs use isolated mode-700 scratch roots (config/data/home/workspace/tmp), a private
Unix socket, and synthetic scratch documents only — never the developer profile.

## Launch recipe (recorded per metadata.txt in each variant directory)

```bash
# scratch=$(mktemp -d /tmp/clay-example-launch.XXXXXX)   # mode 700
# config root: $scratch/home/.config/clay                # server resolves $HOME/.config/clay
# copy: examples/config/init.js + examples/config/packages/  (verbatim, then variant sed)
# workspace: $scratch/workspace (main.rs, notes.md)
# layout: XDG_CONFIG_HOME/clay/layout.json opens main.rs (window-state restore)
#
# server: (cd $workspace && HOME=$scratch/home XDG_CONFIG_HOME=$scratch/config \
#           XDG_DATA_HOME=$scratch/data TMPDIR=$scratch/tmp \
#           target/debug/clay server $scratch/example.sock) > server.log 2>&1 &
# client: (cd repo && same env target/debug/clay client $sock) > client.log 2>&1 &
#
# Evidence: AT-SPI tree dump (python3-gi, Clay app nodes only) + portal Screenshot
# cropped to the Clay window (same tooling as scripts/capture-ui-review.sh).
```

Note: the server resolves its configuration root at `$HOME/.config/clay`
(`ConfigurationRuntime::default_config_root`), not `XDG_CONFIG_HOME`;
`XDG_CONFIG_HOME` only governs `layout.json` window state. The plan sketch's
`XDG_CONFIG_HOME`-only copy silently no-ops — corrected here.

## Results

| Variant | Config state | Outcome |
|---|---|---|
| `default` | verbatim copy (bundled Regular selection active) | PASS — Connected ~1.3s, example config evaluated (file-browser kind icons, icon editor action row Save/Reload/Close, rust mode, gruvbox dark), zero failure diagnostics, no reload loops (25-line server log after settle) |
| `duotone` | selection swapped to `@clay/icons-phosphor-duotone` | PASS — same shell; rendered pixels differ from default (36 rows), theme/design-system unchanged |
| `unloaded` | selection swapped to `@vendor/unloaded-icons` | PASS — bounded sanitized diagnostic at startup AND reload: `clay server configuration failed [theme.load_failed]: JavaScript runtime evaluation failed.`; app stays up, editor/document usable, bundled Regular fallback renders |
| `recovery` | running app: good → broken → restored | PASS — broken selection reload rejected fail-closed (`clay server runtime reload failed [theme.load_failed]`, previous generation preserved, UI usable); restore reloads cleanly (no new failures), shell + document recover |

Per-variant evidence: `review.status`, `metadata.txt` (socket/connected latency),
`config-verify.txt` (the launched selection lines), `accessibility.txt` +
`dump-*.txt` (AT-SPI), `server.log`/`client.log`, `screenshot.png`
(`screenshot.absent` when the portal refused; never counts as a pass/fail).

## Performance observations

- Socket ready ≈ 0.11s, client workspace shell + document ≈ 1.3s on this host.
- Server log stays bounded (13–25 lines) after touch + settle — no reload loops,
  no unbounded diagnostics.

## Blockers / ceilings (environmental, not defects)

- No input-synthesis backend on this host (no wtype/xdotool/ydotool; Wayland):
  interactive keyboard/pointer legs (tab navigation, tooltip-on-focus, opening the
  coding-agent panel by click) are UNRESOLVED here; covered structurally by
  jsdom suites (tasks 7–8) and the task 11 captures.
- `pnpm` absent on this host: the third-party fixture adoption leg
  (`clay package add`/`adopt` + load) could not run live; the unloaded-specifier
  variant covers the same fail-closed selection path end to end.
- Transient observed once (1 of 4 runs, recovery variant): the first no-op
  configuration reload recorded a transient `theme.load_failed` evaluation
  failure despite an unchanged valid selection; the next watcher event
  re-evaluated cleanly and the app stayed usable. Not reproduced in the other
  three runs or by the isolated production-path probe. Flagged for the task 16
  manual test plan follow-up.
