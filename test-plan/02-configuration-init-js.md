# 02 — Configuration (init.js)

Verify the server-side configuration evaluation path. Canonical example:
`examples/init.js` (repository root). API reference:
`docs/reference/clay-js-api/configuration.md`. Repeatable server-side fixture:
`tests/fixtures/configuration/plan080-manual/` (a copy of the `examples/`
tree); run it with `target/debug/clay server /tmp/clay-ipc/plan080.sock
--config-fixture plan080-manual` and watch server stderr for diagnostics.

## Setup

Back up your real config first, then copy the whole example tree (base
config + package modules):

```bash
cp ~/.config/clay/init.js ~/.config/clay/init.js.bak 2>/dev/null || true
cp -r ~/.config/clay/packages ~/.config/clay/packages.bak 2>/dev/null || true
cp -r examples/. ~/.config/clay/   # init.js + packages/first-party.js + packages/third-party.js
```

## Evaluation

| # | Action | Expected |
|---|--------|----------|
| C1 | Launch with the copied `examples/` tree | All sections evaluate; theme set (gruvbox-material-dark), packages loaded (markdown/rust/typescript/javascript/settings), caret bar+blink, custom bindings active |
| C2 | Watch launching terminal | No `runtime.*` diagnostics; each bad line would print one |
| C3 | Introduce `bindKey("Ctrl+X", "nonexistent.command", …)` | Diagnostic naming rejected command ID; editor otherwise unaffected (deny-by-default) |
| C4 | Introduce `setPackageOption(...)` (planned stub) | Diagnostic: planned API not callable; no crash |
| C5 | Introduce a raw `Deno.core.ops.op_clay_...` call | Rejected/diagnostic — raw ops are not a user surface |
| C6 | Delete init.js entirely, launch | Built-in fallback modes (`core.code`/`core.text`) still edit files (Phase 18.9 behavior) |

## Modular configuration

| # | Action | Expected |
|---|--------|----------|
| C7 | Split: move the `bindKey` calls into `~/.config/clay/keys.js`; in init.js: `await loadConfigurationModule({ path: "./keys.js" })` | Bindings from the module work identically |
| C8 | Point `path` at a file with a syntax error | Diagnostic from the module evaluation; rest of config still applied as documented |
| C9 | Break `~/.config/clay/packages/first-party.js` (syntax error), reload | `configuration.module_failed` diagnostic; base config (theme/typography/bindings) still active; packages inactive |
| C10 | Fix `packages/first-party.js`, reload (Ctrl+Shift+R or save-triggered auto-reload) | Diagnostic clears; packages load again; grants-before-loadPackage ordering intact |
| C11 | Delete `packages/third-party.js` entirely, reload | No fatal failure: the missing optional module records a `configuration.module_failed` warning, then base config and first-party packages stay active |
| C12 | Relaunch with `packages/first-party.js` already broken (boot-time isolation) | Server starts; `configuration.module_failed` for the module; base config (theme/typography) active; no launch failure |

## Live reload (no restart)

| # | Action | Expected |
|---|--------|----------|
| C13 | With settings package loaded, open settings and switch appearance | Runtime generation reloads WHILE the client stays connected; theme/appearance change confirms init.js reran |
| C14 | Before the switch, add a new `bindKey` to init.js; after reload press the chord | New binding active without restart |
| C15 | Change `clientSetCursorStyle` shape, reload via appearance | Caret shape changes live |
| C16 | Edit `init.js` on disk (e.g. append a valid `bindKey` line) and save; do NOT press any reload key | Within ~2 s the watcher auto-reloads: server stderr shows a fresh reload with no diagnostics, and the new binding is active |
| C17 | Rapidly save the same file 5× in under a second (e.g. `for i in 1 2 3 4 5; do echo ... > init.js; done`) | Debounce collapses the burst into ONE reload (count `configuration.module_failed` diagnostics in server stderr: one reload, not five) |
| C18 | Break a watched file with an invalid `bindKey` (e.g. `bindKey("Ctrl+X", "nonexistent.command", { scope: "global" })`) and save | Watcher reloads; server stderr shows `runtime reload failed [keybindings.unknown_command]`; previous generation stays active; fix the file and the next reload is clean |
| C19 | GUI: with no custom binding, press `Ctrl+Shift+R` | `runtime.reloadConfiguration` executes (reload succeeded, server stderr clean if config is valid); the Control Center command list shows `runtime.reloadConfiguration` with the `Ctrl+Shift+R` chord |

## Plan 088 configuration compatibility steps

| # | Action | Expected |
|---|--------|----------|
| C20 | Copy `examples/.` into an isolated config root, run `node --check examples/init.js`, and launch the configuration fixture | Explicit Gruvbox dark theme, all three typography profiles, complete seven-field hierarchy, caret settings, bindings, and optional modules apply once; no runtime diagnostics |
| C21 | Review the commented `setAppearance("light")`, `setAppearance("dark")`, and `setAppearance("system")` alternatives; enable one only after commenting out explicit `setTheme` | Appearance values are limited to the documented enum; canonical defaults resolve without a second configuration surface, and explicit `setTheme` precedence is clear |
| C22 | Reload with valid large UI typography (`ui.size: 24`) and a complete hierarchy | Reload is atomic; shell/editor geometry reflows once, remains bounded, and the active generation retains all three profiles |
| C23 | Remove one hierarchy field or set a scale to `0`, `NaN`, or `>4`; reload | Validation rejects the whole update; the prior valid theme/typography generation remains active and no partial hierarchy reaches layout |
| C24 | Add an unknown config key or raw `Deno.core.ops.op_clay_*` call | Deny-by-default diagnostic; no undocumented authority, native widget, filesystem, network, shell, or raw-op access is granted |
| C25 | Confirm `init.js` has no `setPackagePreset` / `clay.preset` knob; presets live in package.json | One-line `loadPackage` still enough; no new configuration key |

## Plan 097 Phase 9 Tauri/React configuration steps

| # | Action | Expected |
|---|--------|----------|
| C26 | Open the React Command Centre, select `runtime.reloadConfiguration` | Existing server reload transaction runs; success diagnostic appears in shell status; no frontend configuration evaluator/store is created |
| C27 | Break a watched configuration module, trigger reload, then fix it | Sanitized failure diagnostic reaches the active tab; old runtime generation/theme/package UI remains installed; fixed reload replaces it atomically |
| C28 | Open React settings, change theme/appearance, then reload/relaunch | `preferences.json` retains the choice with `ui-session` precedence; React receives one resolved theme/runtime snapshot and does not parse raw theme data |
| C29 | Change all typography profiles/ratios, then submit an invalid size or partial hierarchy | Valid complete transaction persists and applies once; invalid transaction is disabled client-side and rejected server-side without changing the prior preference/generation |

## Plan 097 Phase 9 execution record (2026-08-23)

| Checks | Result | Evidence |
|---|---|---|
| C26–C27 | PASS automated / live failure edit not rerun | Existing reload/watcher atomicity suites pass; `workspace-controller` projects live diagnostics and `command-centre` tests route activation; shell footer uses latest per-tab diagnostic |
| C28–C29 | PASS fixture + automated | Settings wide/narrow/expanded/error artifacts under `code-reviews/screenshots/2026-08-23-tauri-react-phase9/`; 83 frontend tests and Rust settings/configuration tests pass; no secret/raw theme values appear in accessibility snapshots |

## Negative checks

- Configuration JavaScript runs ONLY at startup/reload — typing, scrolling,
  paint must never trigger config evaluation (the watcher is bounded polling
  server work; it never runs on keypress, paint, or parse paths).
- No configuration path grants filesystem/network/shell/package-install
  authority beyond its documented scope; watch/reload/isolation add no
  authority.

## Recorded results (Linux, 2026-08-11, debug build)

Executed against `target/debug/clay server /tmp/clay-ipc/clay-plan080.sock
--config-fixture plan080-manual` with the fixture tree in
`tests/fixtures/configuration/plan080-manual/`; server stderr captured to a
log.

| # | Result | Evidence |
|---|--------|----------|
| C1 | PASS | Server starts on the example tree; no `runtime.*` diagnostics on stderr |
| C2 | PASS | Startup log clean (0 diagnostics) |
| C3 | PASS (watcher variant) | Appending `bindKey("Ctrl+X", "nonexistent.command", …)` to `init.js` produced `runtime reload failed [keybindings.unknown_command]`; reload rejected, previous generation preserved |
| C9 | PASS | Breaking `packages/first-party.js` produced `configuration.module_failed: … Unexpected token '='`; server kept running, base config active |
| C10 | PASS | Fixing the module produced a clean reload (no new diagnostics) |
| C11 | PASS | Deleting `packages/third-party.js` produced a `configuration.module_failed` warning for the missing module; no fatal failure, base + first-party stayed active |
| C12 | PASS (boot isolation) | Launching with the broken module present: server started, warning recorded |
| C16 | PASS | Appending a valid `bindKey` line to `init.js` triggered an auto-reload within ~2 s with no diagnostics |
| C17 | PASS | 5 rapid writes to `packages/first-party.js` collapsed into exactly ONE reload (one `configuration.module_failed` pair in the log, not five) |
| C18 | PASS | Invalid `bindKey` → `keybindings.unknown_command`; fix → clean reload |
| C4–C8, C13–C15, C19 | NOT RUN headless | GUI/client-interaction steps; covered by automated integration tests (`example_configuration_*`, `configuration_watcher_*`, `configuration_default_reload_binding_is_present_and_overridable`, `control_center_includes_built_in_commands`, `typography_update_reaches_connected_clients_once`) — run on a desktop session |

## Plan 088 task 12 Linux execution record (2026-08-15)

| Checks | Result | Evidence |
|---|---|---|
| C20 | PASS | `node --check` passed for the canonical tree; `cargo test --lib example_configuration_loads_cleanly_and_applies_effects -- --test-threads=1` passed and asserted explicit dark theme, all profiles, families, and default hierarchy; `cargo test --test protocol clay_js_doc_registry` passed (41) |
| C21 | PASS | Existing theme-package tests cover all four bundled themes and appearance resolution; light/dark visual artifacts are retained under `code-reviews/screenshots/2026-08-14-plan088-modernization/`; no new API or registry entry was needed |
| C22 | PASS — strongest available evidence | Large-typography capture at `code-reviews/screenshots/2026-08-14-plan088-modernization/large-typography/` and typography/layout structural tests pass; direct reload/key delivery was not re-run because window targeting is unavailable |
| C23 | PASS automated / NOT RUN manually | `invalid_init_typography_reports_actionable_validation_error` and existing atomic-install tests pass; targeted GUI reload is blocked by the host input backend |
| C24 | PASS automated / NOT RUN manually | Existing raw-op/authority-denial and configuration registry tests pass; no manual raw-op execution was attempted |

## Cleanup

```bash
mv ~/.config/clay/init.js.bak ~/.config/clay/init.js 2>/dev/null || rm ~/.config/clay/init.js
rm -rf ~/.config/clay/packages
mv ~/.config/clay/packages.bak ~/.config/clay/packages 2>/dev/null || true
```

## Plan 097 Phase 12 Tauri/React visual and accessibility review (2026-08-24)

| Check | Result | Evidence |
|---|---|---|
| Theme/settings surface | PASS static | `code-reviews/screenshots/2026-08-24-tauri-react-parity/settings/fixture-{wide,narrow}.png` and AX snapshots show labelled Theme/Appearance controls and disclosure state |
| Large typography | PASS automated/static coverage | Current typography validation and bundle checks pass; static state coverage remains in `states/` and the existing typography captures |
| Live reload/theme switching | UNRESOLVED interaction | Host cannot safely focus Clay or deliver keyboard chords. No live reload pass inferred from fixture screenshots |

No configuration API or hidden authority was added by this review.
