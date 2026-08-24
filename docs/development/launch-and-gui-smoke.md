# Launch and GUI Smoke Validation

Use these command-first launch paths to validate Clay's current GUI and client/server behavior on every supported desktop platform. The normal workflow does not require copying a named pipe or Unix socket path.

## Quick Commands

For detailed platform matrices, capability tokens, conflict recovery menus, and save/reload internals, see [File Open, Save, and Reload Workflow](file-open-save-reload-workflow.md).

From the repository root:

```bash
# Start or reuse the default local server, then open the GUI client.
cargo run

# App-managed GUI smoke run with an isolated endpoint and managed child server.
cargo run -- smoke-gui

# Runtime-backed configuration smoke: evaluates tests/fixtures/configuration/runtime-sdui/init.js
# before the GUI connects, then renders the JavaScript-published SDUI tree.
cargo run -- smoke-gui --config-fixture runtime-sdui

# Markdown mode smoke: validates the first-party package SDUI preview/status workflow.
cargo run -- smoke-gui --config-fixture markdown-mode

# Windows Markdown open-dialog smoke: loads Markdown and binds Ctrl+O through init.js.
cargo run -- smoke-gui --config-fixture windows-markdown-open

# End-to-end file browser workflow smoke: loads Rust/TypeScript/JavaScript packages
# and binds folder picker, fuzzy open, browser toggle, save, copy/cut/paste, and
# open-documents switcher commands.
cargo run -- smoke-gui --config-fixture file-browser-workflow

# Stop and replace the default Linux server, wait for readiness, then exit.
cargo run -- restart

# Foreground default server, useful for watching server diagnostics.
cargo run -- server

# First default client: should receive the editable lease when available.
cargo run -- client

# Second default client: should attach as a read-only observer.
cargo run -- client
```

## Live AT-SPI accessibility regression check (plan 086 task 4)

On a Linux desktop session with a real AT-SPI bus, run the environment-gated
live accessibility smoke — the same check that caught the P0 startup crash:

```bash
CLAY_LIVE_A11Y_SMOKE=1 cargo test --test security live_atspi_smoke::live_atspi_accessibility_smoke -- --ignored --exact --test-threads=1
```

Prerequisites (missing ones print an explicit skip reason — never a false
pass):

- A desktop session with a live AT-SPI bus (GNOME/KDE; `org.a11y.Bus`
  reachable on the session bus).
- Python 3 with the AT-SPI GI bindings (`python3-gi`, `gir1.2-atspi-2.0`).
- The clay binaries built (`cargo build`); the test spawns the real
  `clay server` and `clay client`.

What it verifies against the real desktop accessibility stack:

- The window starts, restores a two-tab window from an isolated
  `layout.json`, and stays alive past startup (the pre-fix behavior was a
  panic on the first tree within ~2 s).
- The AT-SPI tree exposes the shell, the `Workspace tabs` TabList with both
  restored cards (one selected), the active pane, the connected status
  line, and the attached server-driven region.
- The active editor Entry advertises the real `EditableText` interface in
  addition to `Text`; this is checked separately from its editable state flag.
- Node identities stay stable across a second query (no per-pass virtual
  node churn).
- Every run uses a mode-700 temporary IPC/config home — never the ambient
  `~/.config/clay`, `~/.local/share/clay`, or default socket — and kills
  both child processes and removes the temp directory on every exit path.

The pre-cutover native accessibility unit tests (plan 086 task 3,
`tests/live_atspi_smoke.rs`) were removed with the native client; the
retained structural coverage for input-driven tab/menu/status updates lives in
the frontend component suites, and live desktop structure is proven by the
AT-SPI probe inside this harness.

## Plan 089 Linux multi-window, DPI, font-scale, and Wayland smoke

Run the environment-gated platform check on a real Wayland desktop:

```bash
CLAY_LIVE_WINDOW_SMOKE=1 cargo test --test security \
  live_atspi_smoke::live_multi_window_scale_smoke \
  -- --ignored --exact --test-threads=1
```

The check starts one isolated server and two real Clay clients, applies a
complete user-owned `theme.setTypography` profile (`ui: 24`, document roles
20/21), and verifies through AT-SPI that both windows expose distinct frames,
positive physical bounds within a bounded envelope derived from Clay's
900×600 logical window, and large bounded status bars. A compositor may
resize a mapped window's axes independently; logical/physical conversion and
typography-driven geometry are covered by the retained responsive-layout
guards (`responsive_layout_work_preserves_sidebar_and_editor_bounds`) plus the
UI-review harness wide/narrow captures. (The historical headless check sent `Rescale(2.0)` plus a physical 1800×1200 resize and asserted logical 900×600; it was removed with the native client at Plan 097 Phase 12.) Together these
checks cover scale change, typography-driven geometry, multi-window startup,
and Wayland delivery without blind input.

Prerequisites: Linux, `WAYLAND_DISPLAY`, a reachable `org.a11y.Bus`, Python
GI AT-SPI (`python3-gi` and `gir1.2-atspi-2.0`), and built Clay binaries.
Missing prerequisites return a skip reason; they never count as a pass. The
smoke uses mode-700 HOME/XDG/socket/workspace roots and synthetic documents
only.

Interactive Plan 088 completion, Command Centre, settings, file-browser,
multi-tab/multi-pane, narrow/wide, DPI, and native-dialog states additionally
require safe window targeting. Run `computer-use-linux doctor`; when
`can_query_windows` or `can_focus_windows` is false, run
`computer-use-linux setup-window-targeting` and log out/in if GNOME requests a
shell reload, then verify both readiness flags before targeted input. The
current host's unavailable-backend report is
`org.freedesktop.DBus.Error.ServiceUnknown` for the GNOME Shell window-control
API, so portal coordinates and unscoped chords must not be used as a
substitute; retain those states as `UNRESOLVED` until targeting or a semantic
no-focus harness action is available.

## Repeatable UI review harness (Plan 087 task 2)

The repeatable live-artifact harness launches one fixed-size review state, waits
for its Clay accessibility tree, captures a full-screen PNG through the native
`xdg-desktop-portal` Screenshot API, and writes a Clay-only AT-SPI dump:

```bash
scripts/capture-ui-review.sh --fixture ui-review-default \
  --output code-reviews/screenshots/plan087-ui-review/default
scripts/capture-ui-review.sh --fixture ui-review-completion \
  --output code-reviews/screenshots/plan087-ui-review/completion
```

`--fixture` accepts `ui-review-default`, `ui-review-loading`,
`ui-review-error`, `ui-review-recovery`, `ui-review-large-typography`,
`ui-review-completion`, `ui-review-command-centre`, and `ui-review-rust`. The
fixtures live under
`tests/fixtures/configuration/ui-review-*/`:

| Fixture | State and capture step |
|---|---|
| `ui-review-default` | Empty config, Clay-owned welcome/shell baseline. |
| `ui-review-loading` | Runtime SDUI loading-state fixture with label `Loading workspace…`; no timer or production-only hook is added. |
| `ui-review-error` | Invalid theme specifier; Clay stays alive and exposes a sanitized `Runtime packages.not_installed` diagnostic. |
| `ui-review-recovery` | Connects normally, stops only its private server, then captures `Disconnected`/recovery status. |
| `ui-review-large-typography` | Applies `theme.setTypography` with UI 24/document 20–21 before the client handshake; captures the bounded large-type shell. |
| `ui-review-completion` | Loads `@clay/rust` and binds `Ctrl+Space`; focus editor, trigger completion, then press Enter in the terminal to capture. |
| `ui-review-command-centre` | Binds global `Ctrl+Alt+P`; open the centered Command Centre, then press Enter in the terminal to capture. |
| `ui-review-rust` | Authorizes `@clay/lsp-rust`; make a no-op edit, capture visible inlays, toggle them off, and capture again. |

Each run copies its named `init.js` into a mode-700 temporary
`HOME/.config/clay`, uses a mode-700 temporary XDG config/data/socket root,
creates only bounded fixture documents, and removes the launch root and raw
process logs on exit. The script writes `instructions.md`, `metadata.txt`,
`screenshot.png`, `accessibility.txt`, and `review.status` under the caller's
output directory. A missing AT-SPI GI binding, live AT-SPI bus, interactive TTY,
or portal Screenshot capability writes `UNRESOLVED` and exits 2; it never counts
missing visual/accessibility tooling as a pass. `accessibility.txt` contains
only nodes from the Clay application. The normal `smoke-gui` path remains the
quick app-managed smoke; this wrapper uses explicit `clay server`/`clay client`
arguments because review capture needs a caller-selected artifact directory,
private socket, process cleanup, and an interaction checkpoint while the
window remains open. Both paths use the existing `WINDOW_WIDTH`/`WINDOW_HEIGHT`
logical-size constants (`900×600`).

These PNGs are review artifacts, not GPU goldens or CI pixel assertions.
Structural SDUI/accessibility tests remain the blocking automated layer; inspect
both the image and the AT-SPI dump before recording a visual result.

## Default End-User Configuration

The commands above launch the app or run dev-only smoke fixtures. The actual end-user product setup is a small `~/.config/clay/init.js` that loads Markdown defaults through the runtime-backed generic package loader and binds the Windows open-file command:

```js
import { bindKey } from "clay:keybindings";
import { loadPackage } from "clay:packages";

await loadPackage("@clay/markdown");
bindKey("Ctrl+O", "documents.clientOpenFileDialog", { scope: "editor" });
```

This is the Markdown product baseline. It is deliberately distinct from the smoke fixtures under `tests/fixtures/configuration/`:

- **Smoke-only (dev validation, never the product path):** the `markdown-mode` and `windows-markdown-open` fixtures inline a full `markdownPackage` manifest object and manually call `serverLoadPackage`, `serverRegisterModePattern`, `serverActivateMajorMode`, `serverRegisterCommand`, `serverRegisterParseHandler`, and `serverPublishDecorations`. That plumbing exists only to validate each facade deterministically. Pasting the smoke fixture manifest block into `~/.config/clay/init.js` is not supported and is not the documented setup.
- **End-user (product baseline):** the one-line `loadPackage("@clay/markdown")` plus the explicit `Ctrl+O` `bindKey`. No inline manifest object, no per-facade registration imports, no `publishTree` panel publication.

Markdown end-user baseline invariants:

- **Editor-only Markdown main slot.** The Markdown package occupies the mandatory `main` slot of `PaneSlotLayout` and does not publish a package-owned default `PanelContribution` (side panel, preview panel, or status panel) on load. This invariant is about package-published Markdown panels only; it does not forbid the Clay-owned Workspace file browser used by the app-level file-browser workflow.
- **Fixed panels resize the editor.** Any accepted visible fixed panel in `left`, `right`, `top`, or `bottom` consumes `PaneSlotLayout` geometry and reduces/clips the editor `main` rect; transient overlays may cover content by design and do not consume fixed slot geometry.
- **Optional preview only on demand.** An optional Markdown preview/status panel is a `clay:ui` `PanelContribution` targeting a slot such as `right` with `defaultVisibility: "hidden"`; it appears only through `setPackageOption`, `serverSetLayoutOverride`, or `markdown.togglePreview`.
- **Selected-file open supports save/conflict UX.** `Ctrl+O` opens a selected file and activates Markdown behavior/decorations through generic `MajorModeActivation` + `DocumentClassification`. Dirty edits surface in status/accessibility; bind `Ctrl+S` to `documents.serverSaveDocument` for server-first save. Stale on-disk metadata and dirty-reload conflicts open a recovery menu (reload / keep edits / compare later) instead of silently overwriting.

Timing and authority boundaries for the baseline:

- **Configuration/open time only.** Markdown loading, contribution-descriptor validation, and selected-file activation run at configuration load or document-open time. Ordinary typing, paint, scroll, layout, and text-event handling stay client-local/non-blocking and read only already-installed inert shell/contribution state; they never run package JavaScript, parser work, IPC, file IO, or full-document serialization.
- **No authority broadened.** Simplifying `init.js` to the one-line loader plus `bindKey` does not grant package installation, filesystem access beyond the selected file and the config root, workspace expansion, shell, network, AI mutation, WASM, raw Deno op, native-widget handle, raw CSS, renderer-callback, or client-side JavaScript authority. Package loading remains constrained to first-party `@clay/*` specifiers and deny-by-default for arbitrary external imports.

## Expected GUI Status

The GUI status line and accessibility label should make the connection state visible without reading stderr:

- `Connected — Editable`: the client has the editable document lease.
- `Connected — Read-only Observer`: the client is attached but cannot edit because another client owns the editable lease.
- `Local Fallback`: no server was reachable for `cargo run -- client`, so the GUI opened with local-only state.
- `Disconnected`: the connection was lost after a connected session. Status/accessibility include reconnect guidance (restart Clay); a Dismiss recovery menu is available. Raw transport strings/host paths are not shown.
- Version text such as `v12`: the latest known server-confirmed document version after a snapshot, resync, or edit acknowledgement.
- `Pending edits: N`: outbound optimistic edits not yet acknowledged; increments on local enqueue and decrements after ack/rejection/resync.
- `Recovery: …`: sanitized recovery summary from an active recovery menu or actionable sync/file diagnostic (edit rejection, disconnect, save conflict).
- `Runtime runtime.<code>: <safe message>`: server-side JavaScript configuration/runtime diagnostics reached the GUI status path. The message should be actionable but must not include absolute local paths, source snippets, secrets, tokens, or environment dumps.

Typing remains local and optimistic. Editor input must not wait for IPC acknowledgements, server work, runtime diagnostics, or full-document synchronization; acknowledgements, resyncs, and runtime diagnostic status updates arrive asynchronously and update status when available.

Bare `cargo run` should not show a package-published Markdown preview/status side panel by default. Clay-owned workspace chrome is separate: when a workspace root is available, the app-level Workspace file browser may appear or be toggled through documented workspace commands. Server-driven package/UI fixture behavior remains covered by the explicit `runtime-sdui` smoke fixture; that fixture should show more than one native region when connected: a server-generated workspace/sidebar panel with status/list/button content plus the document-bound editor view. Visible fixed panels should reduce the editor `main` rect instead of covering text/caret hit targets; transient overlays may cover content because they are overlay UI. Updating or interacting with side-panel controls must not replace the editor text, caret, document version, editable/read-only status, or runtime diagnostic status text.

SDUI payload costs are validated by unit tests rather than default GUI smoke output. The representative explicit SDUI snapshot is expected to stay under 4 KiB, and a simple side-panel update is expected to stay under 1 KiB and below the equivalent snapshot size.

## Smoke Modes

### Bare `cargo run`

Bare `cargo run` tries the platform default local endpoint. If no server is reachable, Clay starts the current executable directly as a background `clay server <endpoint>` process, retries the client handshake for a bounded readiness window, and opens the GUI when connected. The Markdown package still publishes no default preview/status panel; Clay-owned Workspace file-browser chrome is controlled by workspace state and documented workspace commands, not by Markdown package loading.

### `cargo run -- restart`

On Linux, `restart` finds only server processes running the current Clay executable against the default endpoint, sends `SIGTERM`, escalates to `SIGKILL` after a two-second bound, starts a fresh background server through the existing shell-free child command, waits for the normal client handshake, then exits without opening another GUI. It does not stop Clay clients or isolated `smoke-gui` servers. Other platforms currently return an unsupported-command error.

### `cargo run -- smoke-gui`

`smoke-gui` is the isolated app-managed GUI smoke path. It creates a unique local endpoint, starts a managed child `clay server <endpoint>` process with direct arguments, waits for readiness, opens the GUI client, and terminates the child server when the GUI exits.

### `cargo run -- smoke-gui --config-fixture runtime-sdui`

The runtime-backed smoke path uses the same managed local IPC lifecycle, but passes `--config-fixture runtime-sdui` to the child server. The server evaluates `tests/fixtures/configuration/runtime-sdui/init.js`, imports `clay:sdui`, publishes a validated SDUI tree, and then sends that tree through the normal bootstrap `SduiSnapshot` path. The GUI should show the `Runtime Smoke Workspace` panel, list/button/status content, and the document-bound editor view while retaining editable/read-only connection status and asynchronous edit acknowledgements.

### `cargo run -- smoke-gui --config-fixture markdown-mode`

The Markdown smoke path uses `tests/fixtures/configuration/markdown-mode/init.js`. The fixture validates and loads `@clay/markdown` metadata, activates the `markdown` mode for `sample.md`/document `1`, registers package commands and parse/decorations providers, and publishes representative decorations. If no workspace root is configured, the fixture still uses document `1` so the GUI smoke remains deterministic and does not expand filesystem authority.

The fixture does not publish a default side panel; the optional preview is a `PanelContribution` the host opts into. Ordinary typing remains local; parse/decoration publication is configuration/load-time work, not keypress, paint, or scroll work.

### `cargo run -- smoke-gui --config-fixture windows-markdown-open`

The Windows Markdown open-dialog smoke path uses `tests/fixtures/configuration/windows-markdown-open/init.js`. The fixture loads `@clay/markdown`, registers the Markdown mode/parser/decorations workflow, and binds `Ctrl+O` to `documents.clientOpenFileDialog` through the normal `bindKey` configuration API. It does not add a Rust shortcut, install packages, fetch network resources, execute shell commands, or broaden workspace authority.

Manual Windows 11 verification:

1. Run `cargo run -- smoke-gui --config-fixture windows-markdown-open`.
2. Press `Ctrl+O`, select a regular UTF-8 `.md`, `.markdown`, or `.mdown` file in the native Windows file browser, and confirm the selected file replaces the editor buffer.
3. Confirm Markdown decorations are visible for the opened file. Decoration refresh may arrive asynchronously; ordinary typing should remain responsive and local.
4. Type a small edit in the opened document, then close/discard it. Do not test save in Phase 19.

### Phase 18.8 Control Center manual smoke

Phase 18.8 adds the server-owned `CommandExecutor` validation boundary, the generic `TransientMenuSession` state model, and the built-in `controlCenter.open` command-palette workflow. The Control Center has no default Rust key binding and no dedicated smoke fixture; it is reached only by binding a key to the built-in command through the existing `keybindings.bindKey` configuration API. Because pixel snapshots are unavailable, manual validation is required.

Manual Control Center smoke:

1. Create or extend `~/.config/clay/init.js` to bind a key to the built-in command:

   ```js
   import { bindKey } from "clay:keybindings";

   bindKey("Ctrl+Shift+P", "controlCenter.open", { scope: "editor" });
   ```

2. Launch Clay with the normal command-first GUI path (`cargo run` or `cargo run -- smoke-gui`).
3. Press the configured `Ctrl+Shift+P` (or chosen chord). The bottom-pane Control Center transient overlay should appear with a bounded list of executable commands: built-in server commands such as `controlCenter.open` and `workspace.refresh`, registered package commands, and the `shell.client*` pane/tab commands (which are listed and, on activation, bridged back to the client shell driver through the server-approved `ShellClientCommandRequest` frame). Only client-first edit commands (`text.insert*`) are excluded from the listing.
4. Type a filter query and confirm the list narrows by label, command id, key binding, or package provenance; item count stays within `MAX_ITEMS` (256) and the query is truncated at `MAX_QUERY_CHARS` (256).
5. Move the selection with `Up`/`Down` (wraps at boundaries) and confirm the selected item stays within bounds.
6. Press `Enter` on a selected safe command (for example `workspace.refresh` or a registered `server-first` command). The activation should enqueue an inert command intent that the server-owned `CommandExecutor` re-validates (command id, routing policy, package provenance, declared permissions, argument budget, target context) before any side effect; the menu should close on successful activation.
7. Press `Escape` to cancel the Control Center without executing a command and confirm focus returns to the editor.
8. Type in the editor and confirm ordinary typing remains responsive, local, and optimistic; the transient menu / command execution path must not run on the keypress-to-paint, layout, scroll, text-event, edit acknowledgement, parse-result publication, or decoration rendering hot paths.
9. Open the Control Center again with an empty registry-only filter (no package commands registered in a bare install) and confirm the built-in commands still appear and the menu handles the empty/no-match state without panicking.

Automated coverage (no manual execution needed): `CommandExecutor` validation (unknown command, invalid routing policy, invalid provenance, undeclared permission, malformed/oversize arguments, unauthorized target) is covered by `tests/command_execution.rs` and the `command_execution` module unit tests; Control Center open/filter/execute/empty-reject/client-first-exclusion/item-detail are covered by the `control_center` module unit tests; transient menu session bounds/selection/cancel/stale-rejection are covered by the `transient_menu` module unit tests; built-in command membership is covered by `builtin_server_command`/`builtin_server_command_ids`; internal-vs-public API boundary (no public `serverExecuteCommand`/`serverOpenTransientMenu`/`controlCenter.open` facade) is covered by `tests/rust_visibility_api_mapping.rs::phase18_8_command_execution_and_transient_menu_surfaces_are_internal`; configuration-via-`bindKey` and no-hidden-keys contracts are covered by `tests/clay_js_api_inventory.rs` and `tests/primitives_docs.rs`.

What the manual smoke adds on top of automated tests: the rendered bottom-pane overlay geometry, real keyboard focus restore after `Escape`, and perceptual confirmation that typing stays responsive while the Control Center is open — none of which pixel-free automated tests can assert.

### Phase 18.9 built-in fallback mode smoke (no `init.js`)

Phase 18.9 ships always-on built-in Clay-owned fallback modes `core.text` and `core.code` (registered at server startup through `ModeRegistry::new()`), so any file opens into a predictable, editable mode even when no language package is installed, disabled, or invalid — and first open needs no JavaScript round trip for fallback editing because the built-in modes are registered before any configuration/package evaluation runs. No `~/.config/clay/init.js` line and no `loadPackage` step are required for fallback editing.

Manual fallback smoke:

1. Launch Clay with **no `~/.config/clay/init.js`** (or an empty one) using the normal command-first GUI path (`cargo run` or `cargo run -- smoke-gui`). No language package is loaded.
2. Open a plain-text file such as a `README.txt` (or any file whose extension no package claims). Confirm the document opens editable with generic Tab/Enter/backspace behavior — its active major mode is the built-in `core.text` universal fallback (`modes.explainActiveMode` reports `fallbackUsed: true`).
3. Open a code-like file such as `main.rs` (or any file with one of the curated built-in `core.code` extensions). Confirm the document opens editable with code-oriented behavior — its active major mode is the built-in `core.code` fallback, and closing braces/brackets/parens reflow via electric outdent rules shipped by the `core_code_editing` manifest.
4. Confirm ordinary typing stays local and optimistic and that no synchronous JavaScript round trip occurs before local paint (built-in mode manifests are inert `ClientFirstPredictable` data executed by Rust-known engines).
5. (Optional) Add the one-line default loader to `~/.config/clay/init.js` and relaunch:
   ```js
   import { loadPackage } from "clay:packages";
   await loadPackage("@clay/markdown");
   ```
   Open a `README.md` and confirm the Markdown package mode activates (package-declared pattern wins precedence over `core.code`); open `main.rs` again and confirm it still resolves to `core.code` — language packages *extend* `core.code`, they do not replace it. Remove the line and `.md` falls back to `core.text` (still editable, just without Markdown-specific behavior/decorations).

What the manual smoke adds on top of automated tests: perceptual confirmation that documents remain editable with no `init.js` present and that the package opt-in extends rather than shadows the built-in fallback for unrelated extensions.

Automated coverage (no manual execution needed): built-in `core.text`/`core.code` classification and activation with zero packages (absent `init.js`) and editable manifest composition (`minimal_text_editing`/`core_code_editing`) are covered by `tests/package_primitive_gate.rs::empty_init_js_opens_txt_and_rs_into_core_fallbacks_and_remains_editable`; `loadPackage("@clay/markdown")` package-wins-over-`core.code` coexistence is covered by `tests/package_primitive_gate.rs::load_package_markdown_extends_core_code_for_md_while_rs_still_uses_core_code`; the always-on registration at `ModeRegistry::new()` is covered by `builtin_core_modes_are_present_and_classify_with_zero_packages` and `builtin_core_modes_activate_and_remain_editable_without_packages`; fallback manifest payload budget is covered by `fallback_activation_manifest_fits_payload_budget`.

### Phase 18.10 syntax grammar package smoke

Phase 18.10 adds first-party grammar-only syntax packages. They are explicit opt-in packages, not auto-loaded core behavior:

```js
import { loadPackage } from "clay:packages";

await loadPackage("@clay/rust");
await loadPackage("@clay/typescript");
await loadPackage("@clay/javascript");
await loadPackage("@clay/markdown");
```

Manual syntax smoke:

1. Put the four `loadPackage` lines above in `~/.config/clay/init.js`, or use the equivalent checked-in fixture with `cargo run -- smoke-gui --config-fixture syntax-grammars`.
2. Launch Clay with `cargo run`, `cargo run -- smoke-gui`, or the fixture command above.
3. Open small `.rs`, `.ts`, `.tsx`, `.js`, and `.md` files similar to `tests/fixtures/syntax/rust.rs`, `tests/fixtures/syntax/typescript.ts`, `tests/fixtures/syntax/typescript.tsx`, `tests/fixtures/syntax/javascript.js`, and `tests/fixtures/syntax/markdown.md`.
4. Confirm each file renders text immediately and remains editable under its active `core.code`/`core.text` fallback behavior while syntax decorations arrive asynchronously from the background parse/decor path.
5. Type a small edit and scroll; local editing and paint must remain responsive while highlighting may refresh later.
6. Remove the language package load lines and relaunch. The same files should still open editable, but with no active syntax grammar and no syntax highlights.

Automated coverage (no manual execution needed): `tests/syntax_grammar.rs::manual_syntax_smoke_contract_is_covered_by_deterministic_fixture_flow` runs the documented smoke contract deterministically by loading grammar packages, selecting active syntax grammars for `.rs`, `.ts`, and `.js` fixture paths while preserving `core.code`, producing decorations before and after a small edit, and verifying unloaded no-highlight fallback editability. `tests/syntax_grammar.rs::first_party_language_fixtures_produce_themed_vocabulary_decorations` parses Rust, TypeScript, TSX, JavaScript, and Markdown fixture files with package highlight queries and verifies bounded vocabulary `DecorationSet` output; `tests/syntax_grammar.rs::first_party_artifact_provenance_is_recorded` checks reproducible artifact provenance; `syntax_provider_selection_falls_back_to_no_highlighting_without_changing_mode` covers unloaded fallback editability; `tree_sitter_handler_publishes_through_parse_coordinator_and_rejects_stale_results`, `tests/parse_coordinator.rs`, `tests/decoration_transport.rs`, and `tests/editor_performance_invariants.rs` cover background scheduling, stale-result rejection, payload/cache budgets, and hot-path source guards.

### Phase 18.16 tiered syntax engine smoke

Phase 18.16 keeps first-party syntax explicit while selecting among native, WASM, and package-JavaScript engines through one generic pipeline. Captures map to `TokenType` + `Modifiers` vocabulary spans. Normal setup uses native Tier 1 automatically after package load:

```js
import { loadPackage } from "clay:packages";

await loadPackage("@clay/rust");
await loadPackage("@clay/typescript");
await loadPackage("@clay/javascript");
await loadPackage("@clay/markdown");
```

To exercise explicit engine selection from `~/.config/clay/init.js`, set the preference before loading the package:

```js
import { setSyntaxEnginePreference } from "clay:syntax";
import { loadPackage } from "clay:packages";

setSyntaxEnginePreference("rust", "wasm");       // Tier 2, explicit override
// setSyntaxEnginePreference("markdown", "javascript"); // Tier 3 package parser
await loadPackage("@clay/rust");
await loadPackage("@clay/markdown");
```

Manual verification:

1. Run `cargo run -- smoke-gui --config-fixture syntax-grammars` with the four package loads. Open `.rs`, `.ts`, `.tsx`, `.js`, and `.md` fixtures and confirm Tier 1 vocabulary highlighting arrives after text renders.
2. Confirm syntax selection does not replace the active `core.code`/`core.text` or package major mode, and small edits remain local while background decorations refresh.
3. Select `wasm` for one language. If no `*.wasm` binary is committed, confirm `packages/*/grammars/PROVENANCE.md` documents the reproducible build and SHA-256 step; do not build or fetch artifacts during the smoke run.
4. Select `javascript` for Markdown and confirm the existing package parser remains usable as Tier 3 fallback. Remove the preference and return to the native default.
5. If a parse handler fails, confirm a sanitized `parse.open_failed` runtime diagnostic appears asynchronously while the document remains editable. No absolute path, source text, or token data should appear in the diagnostic.

Representative checks:

```bash
cargo test --test runtime syntax_grammar::
cargo test --test runtime parse_coordinator::
cargo test --test protocol manual_smoke_docs::
cargo test --test editor editor_performance_invariants::
```

Security/performance contract: engine selection, package loading, query compilation, artifact validation, and parse work happen at init/package-load/open/reclassification or background time. No network fetch, shell/package-manager execution, native-library load, client-side JavaScript, parser work, configuration evaluation, or blocking IPC is allowed in keypress, paint, layout, scroll, pointer, or text-event handlers. Tier 2 package assets remain first-party, resolver-validated, and package-root-confined; third-party grammar trust is deferred to Phase 23 and a separate security decision.

Automated coverage: `first_party_language_fixtures_produce_themed_vocabulary_decorations`, `first_party_artifact_provenance_is_recorded`, `user_forced_tier_is_honored_and_recorded_in_provenance`, `tier2_wasm_override_suppresses_tier1_when_user_selected`, `js_parser_fallback_still_runs_without_tree_sitter_grammar`, `package_cannot_silently_override_native_tier`, `finish_task_publishes_runtime_diagnostic_for_handler_error`, and `open_document_renders_before_background_parse_completes` cover the tier choices, provenance, fallback, diagnostics, and non-blocking open contract.

### Phase 18.16.5 typography smoke

Use one complete `setTypography` call in `~/.config/clay/init.js`, then launch with `cargo run`. Repeat with Gruvbox Material dark and light themes and UI/document sizes 6 px, defaults, and 40 px.

Manual matrix:

1. Open plain prose, Rust/TypeScript/JavaScript code, and Markdown containing inline code, fenced code, Unicode (`Hé`, `漢字`), and emoji (`🦀`). Prose/Markdown body should use proportional; code documents/ranges should use monospace.
2. Set each profile stack to an unavailable name followed by its generic fallback (`monospace`, `sans-serif`, or `system-ui`). Text, caret, selection, wrapping, hit testing, and scrolling must remain usable; Clay must not fetch or open font files/URLs.
3. Compare minimum, default, and large sizes. Confirm wrapped lines do not clip, caret/selection stay aligned, scrollbar remains bounded, and a live typography reload resets/clamps stale scroll geometry once.
4. Check status text, Workspace file browser, runtime SDUI, package status items, buttons, and lists. Text, row hit regions, and accessibility bounds must scale together without overlap.
5. Keep focus in editor while typing, selecting, clicking, scrolling, and opening/closing UI. Input remains local; typography changes arrive asynchronously without package JavaScript, IPC, font discovery, filesystem, network, or shell work in paint/input/layout.
6. Remove typography configuration and reconnect. Defaults must return. Try an invalid partial profile or invalid size and confirm one sanitized runtime diagnostic while previous complete typography remains active.

Automated coverage: `unicode_and_emoji_shape_with_unavailable_named_font_fallback`, `live_typography_update_requests_layout_render_and_accessibility`, `ui_size_change_scales_row_hit_and_accessibility_bounds_together`, `custom_typography_keeps_scrollbar_and_viewport_geometry_bounded`, `typography_updates_do_not_enter_editor_hot_paths`, protocol/configuration rejection tests, viewport-bounded editor benchmarks, and large-file Markdown decoration budgets.

### Phase 18.17 range diagnostic transport smoke

Phase 18.17 adds viewport-bounded `DiagnosticSet` transport for explicit analyzer packages through `clay:diagnostics.serverPublishDiagnostics`. Native Tree-sitter highlighting does not emit diagnostics: bounded-fragment recovery nodes are not correctness authority. Status-level `RuntimeDiagnostic` stays in the chrome; analyzer-owned range diagnostics stay paint-only.

Manual matrix:

1. Run `cargo run -- smoke-gui --config-fixture syntax-grammars` (or load `@clay/rust`, `@clay/typescript`, `@clay/javascript`, and `@clay/markdown` in `~/.config/clay/init.js`).
2. Open valid and incomplete `.rs`, `.ts`, `.tsx`, `.js`, and `.md` snippets. Confirm syntax highlighting appears without red squiggles from Tree-sitter.
3. Type and scroll while a slow reparse is outstanding. Local typing/scroll remain responsive and no parser-recovery diagnostics appear.
4. If testing an explicit analyzer package, confirm only its validated `DiagnosticSet` produces themed squiggles and that a status-level runtime diagnostic does not become an inline mark.
5. Unload language packages and reopen. Documents stay editable with no syntax highlights or grammar-produced squiggles.

Automated coverage (no manual execution needed): `tree_sitter_highlighting_does_not_emit_range_diagnostics`, `first_party_invalid_fixtures_do_not_masquerade_as_analyzer_diagnostics`, `runtime_diagnostics_remain_status_level_and_range_diagnostics_remain_inline`, and `range_diagnostics_do_not_enter_editor_hot_paths`, plus generic transport coverage in `tests/range_diagnostics.rs`, `tests/parse_coordinator.rs`, and `tests/performance_protocol.rs`.

### Phase 18.18 first-party language package smoke

First-party language packages are explicit opt-in. Run the checked-in end-user-shaped fixture:

```bash
cargo run -- smoke-gui --config-fixture language-packages
```

It calls only `loadPackage("@clay/rust")`, `loadPackage("@clay/typescript")`, `loadPackage("@clay/javascript")`, and `loadPackage("@clay/markdown")`; no raw ops or per-facade setup. Open these deterministic fixtures in order, then repeat one small edit and scroll while decoration/diagnostic work is pending:

| Language | Valid fixture(s) | Invalid fixture | Expected mode and behavior |
| --- | --- | --- | --- |
| Rust | `tests/fixtures/syntax/rust.rs` | `tests/fixtures/syntax/rust-invalid.rs` | `rust`; 4-space indent, pairs, `//` comment toggle, `.`/`:` keyword completion |
| TypeScript | `tests/fixtures/syntax/typescript.ts`, `tests/fixtures/syntax/typescript.tsx` | `tests/fixtures/syntax/typescript-invalid.ts` | `typescript`; 2-space indent, pairs, `//` comment toggle, `.` keyword completion |
| JavaScript | `tests/fixtures/syntax/javascript.js`, `tests/fixtures/syntax/javascript.jsx`, `tests/fixtures/syntax/javascript.mjs`, `tests/fixtures/syntax/javascript.cjs` | `tests/fixtures/syntax/javascript-invalid.js` | `javascript`; 2-space indent, pairs, `//` comment toggle, `.` keyword completion |
| Markdown | `tests/fixtures/syntax/markdown.md` | `tests/fixtures/syntax/markdown-invalid.md` | `markdown`; list continuation/prose pairs, Markdown construct completion, optional package-JS preview/status separate from native decorations |

Manual matrix:

1. Launch the command above. Verify all eight valid fixture extensions classify into their package-declared major modes and remain editable.
2. Confirm Gruvbox-themed native vocabulary highlighting appears after text paints: code keywords/strings/comments/functions/types for Rust, TypeScript, and JavaScript; headings, list markers, code, links, quotes, strong, and emphasis for Markdown. Markdown decoration is Tier 1 native; its optional preview/status remains independent package-JS SDUI and does not open a default panel.
3. Trigger keyword completion with the listed one-character trigger. Confirm bounded priority-0 static text items from `rust.keywords`, `typescript.keywords`, `javascript.keywords`, or `markdown.keywords`; snippets are not expected in Phase 18.18.
4. Exercise indent, pairs, and comment behavior in each code fixture. Confirm Rust uses four spaces; TypeScript/JavaScript use two; Markdown continues list markers. Command/status metadata is package-prefixed (`rust.status.mode`, `typescript.status.mode`, `javascript.status.mode`, `markdown.status.mode`).
5. Open each invalid fixture. Confirm text remains editable and highlighted without a grammar-produced squiggle; syntax correctness diagnostics wait for an explicit analyzer such as a future LSP package.
6. Type and scroll during background parse. Typing/scroll remain responsive; package JavaScript, parser work, IPC, or full-document serialization must not enter input/paint/scroll hot paths.
7. Remove one package `loadPackage` line, relaunch, and open its fixture. Confirm graceful fallback to `core.code` (Rust/TypeScript/JavaScript) or `core.text` (Markdown), with no package status/completion/highlighting error. Restore the line before testing the next package.

Fixtures contain only short synthetic source text—no secrets, real paths, or executable authority.

Automated coverage (no manual execution needed): `tests/manual_smoke_docs.rs::phase18_18_manual_smoke_documents_first_party_language_matrix` and `first_party_syntax_fixtures_exist_per_language` lock this matrix and fixture set. `src/server/js_runtime/mod.rs::language_packages_config_fixture_loads_and_registers_all_contributions`, `first_party_language_packages_are_not_silent_defaults`, `rust_package_expansion_registers_mode_command_completion_and_status`, `typescript_package_expansion_registers_mode_command_completion_and_status`, `javascript_package_expansion_registers_mode_command_completion_and_status`, `language_packages_classify_with_core_fallbacks_and_no_conflicts`, and `language_package_classification_is_deterministic_across_load_orders` cover registration, package classification, and fallback deterministically. `tests/syntax_grammar.rs`, `tests/range_diagnostics.rs`, and `tests/editor_performance_invariants.rs` cover full-window vocabulary decoration chunks, analyzer-only range diagnostics, and no-hot-path behavior.

### Plan 056 low-latency syntax Linux smoke (2026-07-19)

Run the real GUI with first-party package loading and developer metric collection:

```bash
cargo run -- smoke-gui --config-fixture language-packages --profile-perf
```

On the Linux verification host this command started its managed local server, connected the client, installed the `language-packages` runtime fixture, and created a native window. The bounded smoke session was then stopped intentionally; managed smoke cleanup left no `clay-smoke-gui` server process. This boot check confirms the actual `cargo run` IPC/window path; the deterministic matrix below supplies repeatable edit assertions without relying on GUI automation.

For each Rust, TypeScript, TSX, JavaScript, and Markdown fixture, verify immediate editable text, then make rapid keyword/identifier/punctuation/comment/string/prose/code edits and scroll while refresh is pending. Confirm complete current token captures after authoritative refresh, provisional broad-span continuity until replacement, syntax-plus-semantic layering, and that stale versions do not publish. Exercise save, undo/redo, and document switching through the existing Phase 20 file-browser workflow after opening more than one fixture.

The Linux full run passed `cargo test --all-targets`, including `syntax_grammar` (58 tests), `parse_coordinator` (29), `decoration_transport` (15), `performance_protocol` (19), `editor_performance_invariants` (22), and `language_intelligence` (31). Those suites cover exact token/capture transitions, stale and superseded result rejection, one parse per version/window, bounded fan-out, malformed/oversize/provenance failures, non-blocking local edits, syntax/semantic composition, and source-safe metric retention. See [Performance Fixtures and Baseline Workflow](performance.md#plan-056-low-latency-syntax-linux-verification-2026-07-19) for benchmark distributions and metric evidence.

### Plan 057 syntax-continuity Linux smoke (2026-07-19)

Run the first-party package fixture with profiling enabled:

```bash
cargo run -- smoke-gui --config-fixture language-packages --profile-perf
```

The Linux verification launched the native Wayland window successfully. A second X11-backend run (`WAYLAND_DISPLAY=''`, forcing the toolkit's X11 path) provided agent-observable framebuffer checkpoints while exercising the same managed server, workspace, package loading, editor, parse, transport, and paint paths. Both runs used the checked-in `language-packages` fixture and left package authority unchanged.

Manual visual checkpoints used actual workspace files under the synthetic `tmp/src` smoke corpus. Rust opened with visible keyword/function/type/string/number/operator layers. In TypeScript, appending `x` to the already classified `greet` declaration kept the complete `greetx` run in the function color; pressing Enter after that declaration retained the earlier interface/type decorations and the later return/template-string/const decorations. The reported per-letter base-color flash and all-white newline regression were not observed. Screenshots were temporary local verification artifacts and were not committed.

The repeatable matrix is `plan057_first_party_languages_keep_continuity_across_edit_boundaries`: real Rust, TypeScript, TSX, JavaScript, and Markdown grammar output flows through optimistic edit, acknowledgement, incremental parse, every authoritative member, and visible paint-range inspection. It covers declaration and code-string growth, code-comment and Markdown-prose newline, paragraph/code-span growth, punctuation, and deletion. `plan057_authoritative_queries_correct_inherited_code_keywords` covers server correction in all code grammars; `rapid_local_versions_reject_stale_authority_without_losing_provisional_geometry` covers superseded authority while provisional geometry remains valid. The UTF-8/128-byte replacement-boundary regression remains covered by `plan057_utf8_scalar_at_nominal_chunk_boundary_is_never_split`.

Representative commands:

```bash
cargo test --test runtime syntax_grammar::plan057
cargo test --test editor decoration_transport::rapid_local_versions
cargo bench --bench first_party_language_baselines first_party_incremental_edit -- --sample-size 10 --warm-up-time 1 --measurement-time 2
```

See [Performance Fixtures and Baseline Workflow](performance.md#plan-057-syntax-continuity-linux-verification-2026-07-19) for measured parser/query/member counts, `syntax.edit_to_publish` instrumentation, and advisory five-language timings.

### Plan 058 exact-range replacement Linux smoke (2026-07-20)

Run the same first-party fixture with profiling enabled:

```bash
cargo run -- smoke-gui --config-fixture language-packages --profile-perf
```

The Linux verification used the X11 backend (`WAYLAND_DISPLAY=''` plus the then-native toolkit's X11 backend variable) so real editor keyboard/pointer events and framebuffer checkpoints could be observed. From the synthetic `tmp/src` workspace it opened `main.rs` containing a 150-byte Rust line comment before decorated functions/types/strings/numbers. Eight letters were typed one at a time inside that comment across the 128-byte replacement boundary. Screenshots after one, four, and eight letters retained decoration on the complete comment suffix and every downstream code block; no one-byte-per-keypress white gap or downstream color peeling appeared. The client log recorded edit acknowledgements and current `[0,128)` Rust syntax authority through document version 10.

The same session then sent Backspace and Enter at the comment caret. Both produced acknowledgements (versions 11 and 12). Backspace retained downstream decoration; Enter correctly ended the line-comment capture at the newline while all later Rust keyword/function/type/string/number/operator decoration stayed visible. Temporary fixture text was restored after shutdown, screenshots/logs stayed under `/tmp`, managed smoke cleanup removed the isolated server, and package/protocol authority was unchanged.

Repeatable coverage is `plan058_first_party_languages_preserve_shifted_boundary_continuity` for Rust, TypeScript, TSX, JavaScript, and Markdown; `plan058_repeated_insert_delete_authority_cycles_preserve_boundary_geometry` for 128 insertion/deletion pairs; `plan058_repeated_comment_edits_do_not_grow_a_shifted_chunk_boundary_gap` for every Rust state transition; and `repeated_authority_keeps_local_residual_cache_bounded` for 512 bounded coalescing/cache cycles. Existing Plan 057 punctuation, newline, empty-authority, stale-version, and authoritative-correction matrices remain green.

See [Performance Fixtures and Baseline Workflow](performance.md#plan-058-exact-range-replacement-linux-verification-2026-07-20) for unchanged parser/query/member counts, client apply cost, cache bounds, and advisory first-party timings.

### End-to-end file browser workflow smoke

This smoke validates the six-step app workflow on Linux, Clay's primary development and CI host. The Plan 044 real-`cargo run` regressions are locked separately in [Manual File Browser Workflow Bug Contract](manual-file-browser-workflow-bug-contract.md).


1. Open the Clay app.
2. See the Clay-owned Workspace file browser. It starts hidden; press `Ctrl+B` to show it, confirm the header contains the workspace folder name and current relative directory (never the absolute host location), then press `Ctrl+B` again to hide it.
3. Select a folder from the system.
4. Navigate different folders and files.
5. See file contents when the selected file is Rust, TypeScript, or JavaScript.
6. Copy text snippets from opened files to the OS clipboard.

Use the checked-in fixture:

```bash
cargo run -- smoke-gui --config-fixture file-browser-workflow
```

The fixture at `tests/fixtures/configuration/file-browser-workflow/init.js` is equivalent to this end-user configuration shape:

```js
import { bindKey } from "clay:keybindings";
import {
  clientCopySelection,
  clientCutSelection,
  clientPasteClipboard,
  clientShowOpenDocuments,
} from "clay:editor";
import { loadPackage } from "clay:packages";
import { clientOpenFolderDialog } from "clay:workspace";

await loadPackage("@clay/rust");
await loadPackage("@clay/typescript");
await loadPackage("@clay/javascript");

bindKey("Ctrl+Shift+O", clientOpenFolderDialog(), { scope: "editor" });
bindKey("Ctrl+P", "workspace.openFuzzyFile", { scope: "editor" });
bindKey("Ctrl+B", "workspace.toggleFileBrowser", { scope: "editor" });
bindKey("Ctrl+S", "documents.serverSaveDocument", { scope: "editor" });
bindKey("Ctrl+Shift+C", clientCopySelection(), { scope: "editor" });
bindKey("Ctrl+Shift+X", clientCutSelection(), { scope: "editor" });
bindKey("Ctrl+Shift+V", clientPasteClipboard(), { scope: "editor" });
bindKey("Ctrl+Shift+E", clientShowOpenDocuments(), { scope: "editor" });
```

#### Product `cargo run` configuration path

The smoke fixture above is the checked-in equivalent of the real end-user configuration path. To exercise the actual product workflow without a fixture, place the same shape in `~/.config/clay/init.js` and run a bare `cargo run`:

```bash
cargo run
# with ~/.config/clay/init.js binding Ctrl+Shift+O to clientOpenFolderDialog(),
# Ctrl+B to workspace.toggleFileBrowser, and native Ctrl+C / Ctrl+Shift+C to copy
```

This is the regression-checked product path on Linux/GNOME. The shifted-character binding fix means `Ctrl+Shift+O` (manifest chord stored lowercase as `o`) now routes when the GNOME key event reports uppercase `O`. Manual verification mirrors the steps below but also covers the Plan 044 regressions:

- `Ctrl+Shift+O` opens the native folder picker (shifted-character case-insensitive routing).
- Selecting a folder adds it as a server-validated workspace root and refreshes the file browser.
- Clicking nested files such as `src/main.rs` (`.rs`), `main.ts` (`.ts`), `main.js` (`.js`), and `.md` opens them through the generic open-document path; the row identity and action source now match for nested paths.
- Opening a second file retains the prior document session; use `editor.clientShowOpenDocuments` to switch with dirty/active markers.
- Dirty edits show a `Dirty` marker in status/accessibility; configured `Ctrl+S` (`documents.serverSaveDocument`) saves through server file IO and clears dirty on success.
- Stale on-disk save conflicts and dirty-reload conflicts open a recovery menu (reload / keep edits / compare later or save first) rather than silently overwriting.
- The file browser keeps working after opening a Markdown file and after any `parse.open_activation_timeout` diagnostic (Clay-owned workspace browser state is no longer replaced by open-time runtime SDUI).
- The file browser scrolls when there are many rows and the scroll direction matches wheel/trackpad intent (positive deltas reveal later rows; no inversion).
- The editor shows a slim vertical scrollbar thumb for long files, scrolls through them without snapping back to the caret after each wheel event, and stays non-overlapping with the file browser.
- Rust/TypeScript/JavaScript/Markdown files show visible syntax highlighting as distinct token-family background tints (keyword, string, comment, punctuation, markup).
- Selecting text and pressing `Ctrl+C` (or `Ctrl+Shift+C`) copies only the selected UTF-8 text to the OS clipboard; a collapsed selection is a no-op.
- Copy selection is write-only for the current native editor selection; cut (`Ctrl+X` / `Ctrl+Shift+X`) and paste (`Ctrl+V` / `Ctrl+Shift+V`) are separate explicit user commands.

Typing, paint, layout, pointer, and scroll stay client-local/non-blocking throughout; directory listing, folder dialogs, file opens, language parsing/decorations, clipboard writes, and save/reload IO happen only after explicit user action or background scheduling. Security and authority are unchanged from the fixture path: selected-folder grants are server-validated, file opens are root-relative or selected-file validated, and clipboard copy is write-only. Cut and paste are additional explicit user-mediated client commands.

Manual Linux verification:

1. Run `cargo run -- smoke-gui --config-fixture file-browser-workflow` from the repository root.
2. Confirm the GUI connects with the workspace pane hidden. Press `Ctrl+B` to show it; the header must contain the workspace folder name and full location. Press `Ctrl+B` again to hide it and confirm the editor reclaims the left slot. The file browser is SDUI composed by Clay, not a package widget.
3. Press `Ctrl+O` (or the configured open-file binding) and choose a Markdown file in the native file picker. On Linux this uses xdg-desktop-portal `OpenFile` with Markdown/all-files filters; cancellation is a non-error no-op. Confirm the selected file opens through the existing selected-file grant path without expanding sibling-directory authority.
4. Press `Ctrl+Shift+O` and choose a regular folder in the native folder picker. On Linux this uses xdg-desktop-portal with `directory=true`; cancellation is a non-error no-op.
5. Confirm Clay adds only the selected folder as a server-validated workspace root and refreshes the Workspace browser. The selected-folder path is protected by the same selected-path capability family as selected-file opens.
6. Click directory rows such as `workspace/` or `src/` and confirm the browser navigates with `workspace.openDirectory`, shows a `../` parent row for non-root directories, and stays inside the selected workspace root.
7. Open `tests/fixtures/configuration/file-browser-workflow/workspace/main.rs`, `main.ts`, and `main.js` (or equivalent files under the selected folder). The file opens through the generic open-document path, activates the Rust/TypeScript/JavaScript language package when matched, and decorations/status/completions may arrive asynchronously. Confirm visible syntax highlighting appears as distinct token-family background tints.
8. Select text in an opened file and press the native copy shortcut (`Ctrl+C` on Linux/Windows, `Cmd+C` on macOS) or the configured `Ctrl+Shift+C` route. Confirm only the selected UTF-8 text is copied to the OS clipboard; a collapsed selection is a no-op. Then verify cut (`Ctrl+X`) deletes after copying and paste (`Ctrl+V`) inserts or replaces as an ordinary local edit. Where an input method is available (for example ibus/fcitx on Linux), confirm IME preedit paints as an underlined overlay without changing saved document text until commit, and that Escape/focus loss cancels unfinished composition.
9. Type a small edit and confirm the status chrome shows `Dirty`. While the edit is still outbound, status may also show `Pending edits: N` until acknowledgement. Press configured `Ctrl+S` (`documents.serverSaveDocument`) and confirm dirty clears after a successful save. To exercise conflict UX, change the on-disk file externally while the buffer is dirty, then save again: Clay must keep the dirty buffer and open a recovery menu with reload / keep edits / compare later (no silent overwrite). A dirty reload without force must offer save-first / discard-and-reload / keep edits. Sync recovery is GUI-visible: edit rejections and disconnects update status/accessibility (not stderr-only); bindable `editor.clientRequestResync` / `editor.clientDismissRecovery` cover explicit resync and dismiss.
10. Type another small edit and scroll. File-browser and editor scroll directions must match wheel/trackpad intent, long files must scroll without snapping back to the caret, and ordinary typing, paint, layout, pointer, and scroll must remain client-local/non-blocking; directory listing, folder dialogs, file opens, language parsing/decorations, clipboard writes, and save/reload IO happen only after explicit user action or background scheduling.

Security and authority contract: folder selection grants only the selected directory after server validation; file opens remain root-relative or selected-file validated; packages cannot scan arbitrary paths, add root markers, override ignore/listing budgets, call raw `Deno.core.ops`, run shell commands, fetch network/package-manager resources, access AI/WASM/native widgets, execute client-side JavaScript, or invent package clipboard-contents APIs. Copy selection is write-only and limited to the current native editor selection; cut/paste are separate Clay-owned user-mediated client commands.

Automated coverage (no manual execution needed): `tests/manual_smoke_docs.rs::end_to_end_file_browser_workflow_smoke_has_runnable_fixture_contract` verifies this docs/fixture contract; `src/server/js_runtime/mod.rs::file_browser_workflow_config_fixture_loads_packages_and_bindings` loads the fixture and confirms package contributions plus folder/copy/cut/paste/file-browser/open-documents bindings; `src/server/connection/mod.rs::connection_add_selected_workspace_root_sends_file_browser_snapshot`, `connection_add_selected_workspace_root_rejects_stale_capability`, `workspace_directory_action_sends_refreshed_file_browser_snapshot`, and `file_browser_open_uses_generic_open_document_followups` cover selected-folder grants, directory navigation, SDUI refresh, and generic language activation; `copy_selection_writes_selected_text_without_edit_event`, `copy_selection_is_noop_when_selection_is_collapsed`, `copy_selection_failure_reports_runtime_diagnostic`, plus cut/paste unit tests cover clipboard behavior.

### Phase 20 daily-editing platform matrix and Linux verification

Phase 20 daily-editing verification is Linux-primary (CI and agent-run). Windows and macOS rows document dialogs/shortcuts for host validation even when those hosts are not exercised in the current run.

#### Platform capability matrix

| Capability | Linux | Windows | macOS | Other |
|---|---|---|---|---|
| Native file-open dialog | Tauri dialog command (GTK portal on Linux) | Tauri dialog command (Win32) | Tauri dialog command (NSOpenPanel) | selected paths return to the existing server grant paths |
| Native folder dialog | same Tauri command, directory mode | same | same | selected paths return to the existing server grant paths |
| Clipboard copy/cut/paste | webview/CodeMirror clipboard via explicit commands (`Ctrl+C`/`X`/`V`) | same (`Ctrl`) | same (`Cmd`) | no polling or hot-path reads; no clipboard plugin granted to the webview |
| Undo / redo | native `Ctrl+Z` / `Ctrl+Shift+Z` or `Ctrl+Y` | same as Linux | native `Cmd+Z` / `Cmd+Shift+Z` | client inverse-edit stack |
| IME preedit / commit | WebKitGTK IME (ibus/fcitx when available) → CodeMirror composition | WebView2 IME → CodeMirror | macOS IME → CodeMirror | composition renders locally; commits sync through the ordinary edit path |
| Save / reload / conflict menus | server-first; bind `Ctrl+S` in `init.js` | same | same (`Cmd` bindings via `init.js` if desired) | no client filesystem write |
| Multi-document retain/switch | `DocumentSessionStore` + `clientShowOpenDocuments` | same | same | bound to 64 sessions |
| Pending-edit / disconnect / resync recovery | status + recovery menus | same | same | no stderr-only recovery |
| Pixel/GPU snapshots | re-deferred (CPU `TestHarness` ≠ production GPU) | same | same | structural observability stays mandatory |

#### Shortcut matrix (native editor chords)

| Action | Linux / Windows | macOS | Optional `init.js` bindKey |
|---|---|---|---|
| Copy selection | `Ctrl+C` | `Cmd+C` | `Ctrl+Shift+C` → `clientCopySelection` |
| Cut selection | `Ctrl+X` | `Cmd+X` | `Ctrl+Shift+X` → `clientCutSelection` |
| Paste clipboard | `Ctrl+V` | `Cmd+V` | `Ctrl+Shift+V` → `clientPasteClipboard` |
| Undo | `Ctrl+Z` | `Cmd+Z` | `clientUndo` |
| Redo | `Ctrl+Shift+Z` or `Ctrl+Y` | `Cmd+Shift+Z` | `clientRedo` |
| Save active document | none hardcoded | none hardcoded | `Ctrl+S` → `serverSaveDocument` |
| Open file dialog | none hardcoded | none hardcoded | `Ctrl+O` → `clientOpenFileDialog` |
| Open folder dialog | none hardcoded | none hardcoded | `Ctrl+Shift+O` → `clientOpenFolderDialog` |
| Show open documents | none hardcoded | none hardcoded | `Ctrl+Shift+E` → `clientShowOpenDocuments` |
| Request resync / dismiss recovery | none hardcoded | none hardcoded | `clientRequestResync` / `clientDismissRecovery` |

Copy/cut/paste/undo/redo use the primary modifier (`Ctrl` on Linux/Windows, `Cmd` on macOS) through the CodeMirror default keymap projected from behavior manifests (`frontend/src/editor/extensions/keymaps.ts`). Save, open dialogs, open-documents switcher, and recovery actions require explicit `bindKey` configuration (see the file-browser-workflow fixture).

#### Linux verification evidence (Plan 055 Task 17)

Host: Linux/GNOME (`XDG_CURRENT_DESKTOP=GNOME`), Wayland + X11 display, `xdg-desktop-portal` active, `ibus` present for IME smoke when an input method is configured.

Automated Linux gates (required):

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test --all-targets
```

Recorded result for this task: all three commands pass on Linux. Automated coverage includes clipboard cut/paste, undo/redo inverse edits, IME preedit unit paths, multi-document retain/switch, dirty/save/conflict recovery menus, pending-edit/disconnect/resync recovery, capability-token/workspace authorization, and hot-path guards that keep clipboard/save/JS work off ordinary paint.

#### Plan 060 dialog and clipboard backend validation (2026-07-22) — historical native-client record

The pre-cutover native client reserved independent file/folder dialog
generations in its driver; that machinery was replaced at Plan 097 Phase 12 by
narrow Tauri dialog commands feeding the existing server grant paths
(`docs/wiki/modules/react-command-centre-desktop-workflows.md`). The dated
record below describes the removed implementation.

Clipboard handling in the removed native client used `arboard` because the toolkit kept copypasta private; that entire backend was deleted at cutover. The current client performs copy/paste through CodeMirror/webview clipboard APIs inside explicit user commands only — no polling or hot-path reads.

This Linux host was GNOME Wayland with XWayland (`WAYLAND_DISPLAY=wayland-0`, `DISPLAY=:0`). The ignored live round-trip passed both with the normal Wayland-session environment and with `WAYLAND_DISPLAY` removed (explicit X11 path):

```bash
cargo test --lib client::clipboard::tests::live_system_clipboard_round_trip -- --ignored --exact --test-threads=1
env -u WAYLAND_DISPLAY cargo test --lib client::clipboard::tests::live_system_clipboard_round_trip -- --ignored --exact --test-threads=1
```

macOS and Windows parity was reviewed historically but those hosts were not executed; re-run their checklist before claiming cross-platform clipboard behavior for the Tauri shell.

Live boot check: `cargo run -- smoke-gui --config-fixture file-browser-workflow` opens the GUI on Linux and observes `Ime::Enabled` / empty `Ime::Preedit` / `Ime::Disabled` plus ordinary `EditAck` sync while typing. Interactive portal file-open, save-conflict, multi-document switcher, and full CJK composition remain on the manual checklist below (not fully automatable from this agent session).

Manual Linux checklist (interactive; extends the end-to-end file browser workflow above):

1. `cargo run -- smoke-gui --config-fixture file-browser-workflow`
2. `Ctrl+O` opens the portal file picker; while it is open, repeat `Ctrl+O` and confirm no second picker appears. Cancel is a non-error no-op; trigger it again to confirm state reset, then select a Markdown file through the selected-file grant path. Repeat the same duplicate/cancel/reopen check for `Ctrl+Shift+O`; file and folder generations are independent.
3. Cut/paste/undo/redo via native chords; optional fixture binds (`Ctrl+Shift+X`/`V`) route the same client UI commands.
4. With ibus/fcitx available, compose CJK/accented text: preedit underlines without changing canonical text; commit inserts once; Escape/focus loss cancels composition.
5. Dirty → `Ctrl+S` save clears dirty; external on-disk change while dirty opens the stale-save recovery menu; dirty reload without force offers save-first.
6. Open a second file, confirm the prior session is retained, then `Ctrl+Shift+E` (`clientShowOpenDocuments`) switches with dirty/active markers.
7. Confirm pending-edit count and disconnect/resync recovery appear in status/accessibility (not stderr-only).

Windows/macOS host checklists remain the dialog/shortcut rows above plus the Phase 19 Windows Markdown open-dialog smoke and [File Open, Save, and Reload Workflow](file-open-save-reload-workflow.md) platform steps. Agent-run validation does not claim those hosts were executed in this Linux-primary run.

### Phase 19 Windows Markdown open-dialog smoke contract

The Phase 19 Windows Markdown open-dialog smoke still documents the Windows native dialog backend matrix for Windows 11 validation.

Phase 19 starts from this baseline:

- Working today: command-first launch, `smoke-gui`, foreground server/client validation, local optimistic typing, server-owned workspace/file opens for configured roots, the `markdown-mode` fixture that loads `@clay/markdown`, activates `sample.md`/document `1`, publishes representative Markdown decorations, shows inert Markdown status SDUI, the bindable `documents.clientOpenFileDialog` client UI command, native file-open backends on Windows (Shell COM), Linux (xdg-desktop-portal), and macOS (`NSOpenPanel`) that filter for `.md`, `.markdown`, and `.mdown` plus an all-files fallback, explicit selected-file IPC, server single-file grants for files outside configured workspace roots, buffer replacement from the selected-file open response, and live selected-file Markdown activation/decorations/status when `@clay/markdown` is loaded.
- Save exists for Phase 9 workspace documents. Phase 20 selected-file save/conflict UX (dirty chrome, `Ctrl+S` → `documents.serverSaveDocument`, recovery menus) is covered in the end-to-end file-browser workflow smoke above; this Phase 19 Windows matrix remains focused on dialog open + edit.

The in-scope manual Windows 11 smoke scenario is edit-only:

1. Load the first-party Markdown package and configure the key binding through `~/.config/clay/init.js`, or use the repository fixture with `cargo run -- smoke-gui --config-fixture windows-markdown-open`:

   ```js
   import { bindKey } from "clay:keybindings";

   bindKey("Ctrl+O", "documents.clientOpenFileDialog", { scope: "editor" });
   ```

2. Launch Clay with the normal command-first GUI path or the fixture command above.
3. Press the configured `Ctrl+O` binding. On Windows 11, Clay should open the OS file browser with Markdown filters for `.md`, `.markdown`, and `.mdown` plus an all-files fallback.
4. Select a regular UTF-8 Markdown file. Cancellation should be a non-error no-op.
5. Clay should send the selected path to the server as an explicit user-selected open request. The server validates and grants only that file, opens it as a Clay document, replaces the current buffer snapshot, activates Markdown mode when `@clay/markdown` is loaded, and publishes viewport-bounded Markdown decorations/status.
6. Type in the opened document and confirm local editing remains responsive while decoration refresh may arrive asynchronously.
7. Do not test save for this phase; close or discard the smoke document after editing.

Out of scope for the Phase 19 Windows Markdown open-dialog smoke only: saving the selected file, full HTML preview or browser/webview rendering, Windows Explorer file associations, double-click-to-open behavior, drag-and-drop, recent-files lists, package installation, network fetches, shell execution, workspace expansion to the selected file's parent directory, and client-side package JavaScript. Linux folder selection, directory navigation, and workspace-root expansion are covered separately by the end-to-end file browser workflow smoke.

Performance and security contract: the explicit open-dialog command may perform modal native UI and server file-open work. Ordinary typing, paint, scroll, layout, and text-event paths must remain client-local/non-blocking and must not wait on JavaScript, IPC, file IO, parser work, or full-document serialization. A selected path is an explicit user-mediated open request only; it is not unrestricted client filesystem authority and must not broaden workspace access beyond the selected regular UTF-8 file.

The Phase 19 Windows Markdown file-dialog smoke remains the Windows matrix. On Linux and macOS, `documents.clientOpenFileDialog` opens the native file picker (xdg-desktop-portal / `NSOpenPanel`) and still routes through selected-file capability grants; unsupported platforms report a diagnostic/status without panics. Linux native folder selection remains validated by the `workspace.clientOpenFolderDialog` workflow smoke; macOS folder selection uses the same `NSOpenPanel` backend in directory mode.

### Phase 18.11 completion provider smoke

Phase 18.11 adds the `CompletionTriggerAndResult` primitive, the server-side completion provider framework, the built-in `core.bufferWords` provider, and `TransientMenuSession`-based completion display/acceptance. The built-in buffer-word provider is always available; package providers are metadata-only opt-ins registered through `completion.serverRegisterCompletionProvider` and loaded with one explicit `loadPackage` call. No default manual completion key binding exists in Rust, so manual completion is only reachable when `init.js` binds a key.

Manual completion smoke:

1. Configure the manual completion trigger key binding through `~/.config/clay/init.js`:

   ```js
   import { bindKey } from "clay:keybindings";

   bindKey("Ctrl+Space", "completion.trigger", { scope: "editor" });
   ```

2. Launch Clay with `cargo run` or `cargo run -- smoke-gui` and open or create an editable document containing repeated words (for example `fn hello hello_world helper`).
3. Type a prefix such as `hel`. In a completion-enabled mode, a bottom transient completion menu should appear while the word is being typed, with unique matching buffer words (for example `hello`, `hello_world`, `helper`), the selected item highlighted, and provider provenance/detail text.
4. Press the configured `Ctrl+Space` binding to request or refresh completion manually at any caret position.
5. Use `ArrowUp`/`ArrowDown` to move the selection locally. Confirm the menu re-renders without server round trips.
6. Press `Enter` or `Tab` to accept the selected completion. Clay should commit a validated text replacement in the active document only (replacing the current word prefix range with the selected `insertText`) and dismiss the menu. Confirm no command, raw op, or provider code runs on accept.
7. Type a completion item's commit character (if the result item advertises `commitCharacters`) while the menu is open. Clay should accept the completion with that character and insert the commit character through the local edit path only.
8. Press `Escape` while the menu is open. Clay should dismiss the menu without mutating text and clear the active completion request.
9. Type an autocomplete trigger character declared by the active behavior manifest (for example `.`). Local text should mutate first, then a completion request is enqueued asynchronously; typing must remain responsive even if the server result arrives later.
10. Continue typing while a slow completion result is pending. Local edits must remain non-blocking; if a newer edit/cursor movement/mode change supersedes the request, the stale result is dropped and the menu is not installed.
11. Disable/reload a package provider (or remove its `loadPackage` line and relaunch). The package provider's results should disappear, but the built-in `core.bufferWords` provider should still produce completions.

Performance and security contract: trigger classification is local manifest lookup; typing an identifier or declared trigger character edits locally first (`ClientFirstPredictable`) and then enqueues a typed `CompletionRequest` through a bounded non-blocking channel. Provider execution runs server-side on a cancellable `UiReactivePriority` lane that aborts or stale-drops older in-flight requests and validates results against the current document/behavior version and provider generation before publication. Ordinary typing, local text mutation, paint, layout, scroll, pointer, and text-event paths must not execute configuration/provider JavaScript, wait on IPC, run provider code, or recompute provider metadata. Completion grants no filesystem, network, shell, AI mutation, extension loading, workspace mutation, package enable/disable, WASM, raw-op, native-widget, client-JS, or provider execution authority; result items are inert text-replacement data only.

Automated coverage (no manual execution needed): `tests/completion_provider.rs` covers buffer-word unique sorted prefix matches, empty-match status, result payload caps, bounded-window rejection, package cancellation preserving the built-in provider, registry budget validation, request validation, superseded request abort, generation replacement, priority ordering, non-blocking scheduling, unregistered provider rejection, stale document-version/provider-generation result rejection, duplicate provider-ID conflict diagnostics, disabled package provider fallback, and oversized result rejection. `tests/editor_performance_invariants.rs::completion_hot_paths_use_inert_state_and_nonblocking_enqueue_only` statically guards that completion hot paths use inert state and non-blocking enqueue only. `tests/performance_protocol.rs::representative_completion_result_payload_stays_bounded` checks the completion result payload budget. `tests/package_primitive_gate.rs` covers completion-provider contribution permission/conflict/oversize-metadata rejection. `tests/clay_js_api_inventory.rs`, `tests/clay_js_doc_registry.rs`, `tests/clay_js_facade_layout.rs`, and `tests/rust_visibility_api_mapping.rs` cover the public `clay:completion` facade, registry/docs entry, and internal-status mapping. `tests/package_loading_docs.rs` and `tests/primitives_docs.rs` cover the package authoring contract and primitive review documentation.

### Phase 18.20 language intelligence / Phase 18.21 LSP bridge smoke markers

Phase 18.20 ships engine-neutral hover, go-to-definition, code-action, and signature-help primitives plus explicit `language-server` authority. First-party LSP bridge packages (`@clay/lsp-*`) and live rust-analyzer / typescript-language-server / marksman smoke are Phase 18.21.

Phase 18.20 discoverable commands (empty default key bindings):

- `language.hover`
- `language.goToDefinition`
- `language.codeActions`
- `language.signatureHelp`

Manual Phase 18.20 smoke (fake analyzer / no language-server required):

1. Bind one command in `~/.config/clay/init.js`, for example `bindKey("Alt+H", "language.hover", { scope: "editor" })`. Runtime keybindings accept single strokes and space-separated multi-stroke chords (for example `bindKey("Ctrl+X Ctrl+P", "controlCenter.open", { scope: "global" })`); function keys remain unsupported.
2. Launch Clay and place the caret in an editable document.
3. Invoke the binding. A bottom `TransientMenuSession` should show bounded plain-text hover/signature content or a selectable definitions/code-actions list. Raw HTML must not render as native markup.
4. For multiple definitions, select a current-document target and confirm caret navigation. Workspace-file targets open through `workspace.openFile` after root/relative-path revalidation; external/traversing targets are not navigable.
5. For code actions, command-backed items reuse `CommandExecution`; direct edit previews display only and must not mutate text in Phase 18.20.
6. Edit or move the caret before a late result arrives. Stale results must not install a menu.

Phase 18.21 compatibility markers: `authorizeLanguageServer`, `@clay/lsp-rust`, `@clay/lsp-typescript`, `@clay/lsp-javascript`, `@clay/lsp-markdown`, `rust-analyzer`, `typescript-language-server`, `marksman`, LSP 3.17 position-encoding conversion at the package boundary, and no Clay-core `lsp-types`/JSON-RPC dependency.

Manual Phase 18.21 bridge smoke (host tools required):

1. Install the host language servers you want to exercise (`rust-analyzer` via `rustup`, `typescript-language-server` + compatible `typescript@5.9.x`, and/or `marksman`).
2. For bare `cargo run`, create the directory root before authorization in `~/.config/clay/init.js`; `workspaceRootIds` must identify roots that already exist when configuration evaluates:
   ```js
   import { serverAddWorkspaceRoot } from "clay:workspace";
   import { authorizeLanguageServer } from "clay:language-server";
   import { loadPackage } from "clay:packages";

   const root = await serverAddWorkspaceRoot("/absolute/path/to/project");
   await authorizeLanguageServer({
     package: "@clay/lsp-rust",
     contribution: "lsp-rust.server",
     workspaceRootIds: [root],
   });
   await loadPackage("@clay/rust");
   await loadPackage("@clay/lsp-rust");
   ```
   Repeat the object-form authorization and base/bridge loads for other languages. Positional `authorizeLanguageServer(package, contribution, options)` calls are invalid. Empty `init.js` must load no bridge and start no child.
3. Open a matching workspace file. Local typing/paint must stay responsive; semantic/diagnostic refinement arrives asynchronously.
4. Confirm overlapping Tree-sitter recovery noise yields to LSP error/warning squiggles while unrelated diagnostics remain, and that LSP completion merges at priority 100 unless you call `serverDisableCompletion`.
5. Remove one bridge `loadPackage` line or revoke its grant and relaunch: base language package behavior must remain; that bridge's semantic/diagnostic/completion outputs must not linger as authority.
6. Repeat once without the matching `@clay/lsp-*` package loaded to confirm baseline Tree-sitter/base completion still works alone.

Automated fake/real coverage:

```bash
# Deterministic (no host language servers required)
cargo test --test runtime lsp_bridge::
cargo test --test security language_server_authority::
cargo test --test protocol performance_protocol::phase18_21
cargo test --test editor editor_performance_invariants::document_analysis
node --test tests/fixtures/lsp/fake-server/fake-server.test.mjs tests/fixtures/lsp/fake-server/matrix.test.mjs

# Opt-in real servers (skip with explicit reason when a binary is missing)
CLAY_LSP_REAL_SMOKE=1 cargo test --test runtime lsp_real_servers:: -- --nocapture
CLAY_LSP_REAL_SMOKE=1 cargo test --test runtime lsp_bridge:: -- --nocapture

# Advisory baselines remain Tree-sitter/first-party language benches; LSP child
# timings stay environment-gated rather than Criterion CI gates.
cargo bench --bench first_party_language_baselines -- --save-baseline pre-lsp
cargo bench --bench first_party_language_baselines -- --baseline-lenient pre-lsp
```

The generic fake LSP fixture lives at `tests/fixtures/lsp/fake-server/` (profiles + in-process session + spawnable stdio child). Language sample workspaces remain under `tests/fixtures/lsp/{rust,typescript,javascript,markdown}/` and are indexed by `tests/fixtures/lsp/workspaces/README.md`.

Automated coverage (no manual execution needed): `tests/language_intelligence.rs` covers protocol validation, provider registry/coordinator cancellation/timeouts, discoverable command mapping, and inert preview/navigation contracts. Editor/client unit tests cover non-blocking request enqueue, bottom transient UI projection, stale-result drop, current-document definition jump, and edit-preview non-mutation. `tests/editor_performance_invariants.rs::language_intelligence_provider_work_is_absent_from_editor_hot_paths` keeps provider/process work off editor hot paths. `tests/lsp_bridge.rs` owns shared adapter freshness, package manifests, and the fake-server matrix. `tests/lsp_real_servers.rs` owns environment-gated real smoke. `tests/language_server_authority.rs` covers lossless bytes, session cap/revoke, and generic fake-server initialize/exit through the host process service.

### Foreground server plus clients

Use the default server/client commands to validate second-client observer behavior without endpoint arguments:

```bash
cargo run -- server
cargo run -- client
cargo run -- client
```

The first client should show `Connected — Editable`; the second should show `Connected — Read-only Observer`.

## Runtime Diagnostic Smoke Expectations

To manually validate runtime diagnostics, temporarily use an invalid local configuration such as a syntax error in `~/.config/clay/init.js` or an unauthorized import. Start the foreground server and GUI client:

```bash
cargo run -- server
cargo run -- client
```

Expected behavior:

- The server logs a `runtime.*` or `configuration.*` diagnostic code with safe detail.
- The GUI status line includes `Runtime <code>: <message>` after connection.
- The status message omits raw absolute paths and source snippets.
- Typing and native rendering remain responsive; diagnostics are status events, not synchronous input/rendering work.

### Phase 20.1 UI design language, token catalog, and typography hierarchy smoke

Phase 20.1 expanded the typed token catalog, typography hierarchy, and token-backed geometry defaults. No new settings API was added; theme/typography configuration reuses existing `setTheme` and `setTypography`.

Manual matrix (no pixel goldens):

1. Launch with no `~/.config/clay/init.js` (empty defaults). Confirm editor renders with core fallback tokens (sidebar 240px, spacing scale 1.0). Open a file and type; status/shared-ui chrome must render without re-resolution artifacts.
2. Set Gruvbox Material dark theme plus default typography:
   ```js
   import { setTheme, setTypography } from "clay:theme";
   setTheme("@clay/theme-gruvbox-material-dark");
   setTypography({ monospace: { families: ["monospace"], size: 20 }, proportional: { families: ["sans-serif"], size: 20 }, ui: { families: ["system-ui"], size: 12 } });
   ```
   Launch and confirm dark editor/status chrome + default hierarchy scales. Type, scroll, and switch tabs; editable/read-only status and runtime diagnostics must remain functional.
3. Repeat step 2 with Gruvbox light (`@clay/theme-gruvbox-material-light`). Confirm light chrome.
4. Add a custom hierarchy to `setTypography`:
   ```js
   setTypography({
     monospace: { families: ["monospace"], size: 20 },
     proportional: { families: ["sans-serif"], size: 18 },
     ui: { families: ["system-ui"], size: 14 },
     hierarchy: { display: 2.0, title: 1.3, section: 1.1, body: 1.0, status: 1.0, detail: 0.8, caption: 0.7 },
   });
   ```
   Confirm status text, panel titles, and UI chrome scale proportionally. Row hit regions and accessibility bounds must scale with text. Scrollbar, viewport, and panel geometry must remain bounded.
5. Narrow window to ~600px wide, then maximize. Sidebar/panel defaults (240px) should not clip text or overflow.
6. Keyboard focus: tab through editor and Workspace file browser. Focus ring, text selection, and caret must remain visible with the active theme.
7. Remove all configuration and relaunch. Defaults must return (core fallback token catalog + Clay default typography).

What NOT to check manually (smoke-only, deferred):

- Modus Operandi/Vivendi theme selection — deferred to Phase 20.6.
- Live appearance-mode switching (system light/dark autodetection) — deferred to Phase 20.6.
- Density compact/spacious scaling — Phase 20.4 component uplift consumes these.
- Elevation/motion/z-level rendering — Phase 20.4/20.5 consume these.
- Resize/collapse drag persistence — Phase 20.3.

Automated coverage (no manual execution needed): `core_design_token_catalog_has_unique_names_and_typed_fallbacks`, `theme_install_is_atomic_across_editor_and_ui_tokens`, `gruvbox_and_default_themes_cover_new_tokens_via_fallback`, `theme_switch_does_not_parse_or_execute_package_code_in_paint_paths`, `unchanged_hierarchy_does_not_invalidate_layout`, `ui_typography_hierarchy_defaults_preserve_existing_variant_metrics`, `custom_hierarchy_updates_layout_hit_and_accessibility_geometry_together`, `legacy_sidebar_and_package_left_panel_share_default_dimension_token`, `ui_design_tokens_resolve_without_package_javascript_in_paint_layout_or_input_hot_paths`, plus existing Gruvbox theme contract tests, typography protocol round-trip, SDUI layout, and editor performance invariants.

## Security and Endpoint Boundaries

Default and smoke launch paths use only local IPC transports:

- Windows: local named pipes.
- Unix: Unix domain sockets.

Normal GUI smoke validation does not open a remote TCP listener, does not use shell-mediated IPC, and does not require user-managed endpoints. Child servers are launched with `std::process::Command`-style direct executable arguments rather than through a shell. The `--config-fixture runtime-sdui`, `--config-fixture markdown-mode`, and `--config-fixture windows-markdown-open` development options resolve only named repository fixtures under `tests/fixtures/configuration/`; they do not enable package installs, network fetches, shell execution, arbitrary client JavaScript, WASM, AI mutation, or direct client filesystem authority. The Markdown fixtures register only the package commands they use before publishing SDUI actions; the Windows Markdown open fixture also binds the native dialog command through inert keybinding configuration.

Advanced endpoint arguments are optional debugging aids only, for example when reproducing a specific bind/listen problem or inspecting a custom endpoint. They are not part of normal GUI smoke validation.

## Implementation Details

For code-level behavior, see the [Client/Server Edit Acknowledgement Flow](../wiki/flows/client-server-edit-ack.md), [Client Snapshot Bootstrap](../wiki/modules/client-snapshot-bootstrap.md), and [Server IPC Skeleton](../wiki/modules/server-ipc-skeleton.md).
