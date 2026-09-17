# Plan 124 — agent lane + composer palette, manual-test-plan live evidence

Live Linux GUI pass for the plan 124 shell change (persistent agent lane,
composer-anchored `/` palette, retired centered command sheet). This directory
holds the raw evidence behind the "Plan 124 manual-test-plan execution record"
in `test-plan/index.md`; the step definitions live in modules
[10](../10-keybindings-and-commands.md) (K92–K99), [13](../13-window-splits.md)
(S47–S49), [14](../14-tabs.md) (T79–T82), [16](../16-agent-host.md) (A21–A22)
and [17](../17-coding-agent-parity.md) (C57–C64).

## How the run was set up

```bash
npm run build                                  # frontend/dist (embedded by the desktop binary)
cargo build -p clay -p clay-desktop
bash /tmp/plan124-final-launch.sh /tmp/plan124-mtp-root   # isolated server + client
bash /tmp/plan124-mtp-live.sh                             # the drive sequence recorded in drive.txt
```

Isolation (plan-097 privacy rule): `HOME`, `XDG_CONFIG_HOME`, `XDG_DATA_HOME`
and `TMPDIR` all point inside a mode-700 scratch root, `~/.clay` there is a
verbatim `cp -r examples/config/.`, the server's cwd is the scratch workspace
`/tmp/plan124-mtp-root/workspace`, and captures are cropped to the Clay window.
The developer's real profile was never opened.

Host ceilings on this machine (unchanged since module 10's earlier records, no
keyboard or pointer synthesis reaches the app): actual `Ctrl+X Ctrl+P` /
`Ctrl+X Ctrl+O` keystrokes, free-text typing into the composer, `Enter`,
`Escape` and `Shift+Tab` are covered by the automated suites; the live pass
drove the same commands through their on-screen affordances (the status-bar
`hide lane` / `palette` hints call the very commands the chords dispatch) and
read state from AT-SPI. WebKitGTK also exposes palette rows as list items with
no actions and the composer textarea as `Text` only (no `EditableText`), so row
activation and typing are not drivable through the accessibility bus here.

## Files

| File | Contents |
|---|---|
| `drive.txt` | The action sequence with the observed result of each step |
| `accessibility.txt` | AT-SPI extracts (roles, names, states, text) per captured state |
| `geometry.txt` | Lane/rail extents and the veil pixel measurements |
| `server.diagnostics.txt` | Server log for the run (config load, package registration) |
| `client.log` | Desktop client log (empty on this build unless a bridge error occurs) |
| `screenshots/` | Window-cropped captures for the states named in `drive.txt` |
