# Plan 118 — launch test with the canonical example config

Real Linux GUI build (`target/debug/clay server` + `... client`), launched
against a **copy of `examples/config/`** in an isolated scratch root. Evidence
for the plan task "Launch-test the app with the canonical example config".

## Command

```bash
scripts/capture-ui-review.sh --fixture ui-review-launcher --example-config \
  --size 1500x950 --output test-plan/artifacts/.../launch-test/<state>
```

`--example-config` is the leg added for this task: instead of copying
`tests/fixtures/configuration/<fixture>/init.js`, the harness copies the whole
canonical tree (`cp -r examples/config/. <root>/home/.clay/`) — init.js plus
`packages/first-party.js` and `packages/third-party.js`, i.e. exactly what the
documented `cp -r examples/config/. ~/.clay/` does. Each capture records
`config_source=examples/config (canonical tree, cp -r parity)` in
`metadata.txt`. The flag is refused (exit 2) for fixtures whose checks assert
their own panel content, so a mismatched pair can never be captured as a pass.

Scratch root: `mktemp -d` mode 700, with `HOME`, `XDG_CONFIG_HOME`,
`XDG_DATA_HOME`, `TMPDIR` and a private unix socket all inside it
(`scripts/capture-ui-review.sh:843`, `:856`) — see `isolation.txt`.

## Results

| Capture | Status | What it proves |
|---|---|---|
| `landing-1500x950/` | PASS | The copied example boots healthy and lands on the launcher: panel `Start`, `Workspaces` pane (1 row: the scratch workspace) with `Open folder…` foot, `Agents` pane (1 row: `Coding Agent ~/.clay/agents/coding-agent`, `0 skills`) with `One folder per agent in ~/.clay/agents/` foot, action row `Tab panes · ↑↓ move · ⏎ pick · ⏎ open both · esc clear` and a disabled `Open`. Shell chrome present: titlebar (`CLAY`, tab `Workspace`, `Palette`/`Files`/`Hide outline`), sidebar, outline rail (`OUTLINE`/`FILE`/`REVISION`/`STATE`/`ENTRIES`/`WORDS`), status bar. |
| `theme-modus-operandi/` | PASS | Theme switch 1/4 (light). Seeded `preferences.json` wins over the example's `setTheme("@clay/theme-gruvbox-material-dark")`, as documented. Palette applied to every surface, `Connected` in the status line. |
| `theme-modus-vivendi/` | PASS | Theme switch 2/4 (dark). |
| `theme-gruvbox-material-dark/` | PASS | Theme switch 3/4. |
| `theme-gruvbox-material-light/` | PASS | Theme switch 4/4. |
| `palette-open/` | PASS | Command Centre opens over the landing: scrim + overlay (12px radius, soft shadow), 94 list items in the AT-SPI tree — **0 hits** for `chat`, `neobrutal`, `glass`. |
| `launcher-to-agent/` | UNRESOLVED | Interactive landing→agent handoff attempt (recorded, see below). |
| `palette-type-drive-unresolved/` | UNRESOLVED | AT-SPI typing into the Control Center filter attempt (recorded, see below). |

Startup contract (`server.diagnostics.txt` in every PASS capture, bounded and
root-redacted): `configuration_failed_lines=0`, `agent_registration_lines=22`
(the canonical config's `@clay/coding-agent` load entry ran and registered its
profile/commands), plus the expected `package discovery failed … store packages
stay unloaded` note (an isolated root has no `node_modules`; store packages are
third-party installs and are not part of the example).

## Design-system geometry (Quiet Instrument)

`@clay/design-instrument` is what the copied example selects, so the captured
chrome is the shipped language. Measured from the captures (crops at
`/tmp/pane-operandi.png` during the run; `screenshot.png` in each directory):

- hairline zones: 1px pane/zone separators (sidebar right edge, pane borders,
  header divider under `Workspaces`/`Agents`, rail divider) — no heavy borders;
- radius ladder: 12px panes/palette overlay, 8px filter wells and `Open
  folder…` button, 5px count badges and `kbd` chips, pill `Open` action;
- no hard offset shadows anywhere; the only elevation is the Command Centre
  overlay's soft shadow over the scrim;
- no retired patterns: no leading accent bars on rows, no underlines on tabs,
  no film grain; accent appears only as state (active tab, cursor/focus).

## Deviations from the task's acceptance criteria

1. **"The window lands in the Coding Agent" — superseded.** The approved
   2026-09-11 target IA makes the launcher the empty-tab landing surface and the
   coding agent the agent *view* of a tab
   (`decision-logs/2026-09-11-2331-workspace-agent-tab-model-and-launcher-landing.md`,
   `DESIGN.md` §12/§16). The launch therefore lands on the launcher; the coding
   agent is loaded and reachable (the agent pane lists it, and the command
   catalogue carries its 94 entries). Recorded as a stale AC, not a defect.
2. **Flicker/"atomic swap" cannot be observed in a still capture.** What is
   observable and recorded: each capture is a fully settled state with no stale
   chrome under four different themes, and the server installs one
   `RuntimeStateSnapshot` generation (the boot parity test asserts the shipped
   system in it). The transition itself is covered by the frontend DOM-continuity
   test (`frontend/src/test/design-system-conformance.test.tsx`) rather than by
   this harness.

## Unresolved legs (honest reasons)

- **Landing → agent handoff** (`launcher-to-agent/`): AT-SPI `click` on the
  `Coding Agent …` list item performs no selection, so the primary button never
  becomes `Open Coding Agent`. This host has no input-synthesis backend, which is
  exactly the case `test-plan/01-launch-and-connection.md` L14a/L14b mark
  UNRESOLVED, with the wiring pinned by `frontend/src/launcher/LauncherPanel.test.tsx`
  and `frontend/src/shell/WorkspacePanes.test.tsx`.
- **Filtering the Command Centre** (`palette-type-drive-unresolved/`): the
  palette's `entry` exposes no `org.a11y.atspi.EditableText` interface, so AT-SPI
  cannot type into it. The negative check is therefore done on the opened
  palette's full 94-entry tree instead (0 removed-surface hits).

## Observed defects (reported, not fixed here)

1. **Stale error status on a fresh landing.** The status bar shows
   `unknown workspace document 1 — Hint: Open the document through the server
   before saving, reloading, or querying it.` on a landing where no document was
   ever opened. It is the bootstrap placeholder diagnostic
   (`src/server/workspace/mod.rs:2853`, cleared on `documentOpened` —
   `frontend/src/shell/workspace-envelope.ts:251`). Pre-existing and
   environment-wide: the same line is in every capture since plan 109
   (`test-plan/artifacts/109-coding-agent/agent-surface/accessibility.txt`), and
   it is not caused by the canonical example config. It should not be visible on
   a surface that never opened a document.
2. **Control Center entry is not AT-SPI editable** (see unresolved legs). Either
   the widget is not exposed as an editable text control, or WebKitGTK does not
   implement `EditableText` for it; both are accessibility-relevant and need a
   decision before the harness can drive palette filtering.

## Harness changes made by this task

- `--example-config` (copy the canonical tree; refused for mismatched fixtures)
  + `config_source` in `metadata.txt` + usage text.
- PASS captures now keep bounded, root-redacted `server.diagnostics.txt`
  (diagnostic/configuration/generation lines, plus the
  `agent_registration_lines` / `configuration_failed_lines` counters) so the
  startup contract is evidenced without retaining full logs.
- Drive-step failures now keep the whole probe transcript
  (`drive.failed.txt`) and name the first `FAIL` line instead of the last line of
  a mixed stream — the earlier `tail -n 1` reported an `OK` line as the reason.
- `tests/manual_smoke_docs.rs` pins the new contract: `--example-config` is
  refused for a fixture with its own panel content, and `--help` documents it.

## Gates run

`bash -n scripts/capture-ui-review.sh`; `cargo fmt --check`; `cargo clippy
--all-targets -- -D warnings`; `cargo test --lib` (1336); `--test protocol`
(216); `--test presentation` (61); `--test runtime` (75); `--test security`
(154) — all pass, including
`manual_smoke_docs::plan118_ui_review_harness_captures_the_shipped_system_and_rejects_removed_states`.
