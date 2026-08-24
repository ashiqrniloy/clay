# Clay Tauri + React Migration Roadmap

Status: approved architecture migration. This roadmap replaces every prior
roadmap phase. Completed implementation remains repository history; unfinished
native-client phases are not carried forward unless required by the parity
ledger below.

Decision: `decision-logs/2026-08-23-0052-tauri-react-client-architecture.md`.
Executable master plan: `plans/097-Tauri-React-Architecture-Migration.md`.

## Target Architecture

- Tauri v2 desktop shell.
- React + strict TypeScript + Vite frontend.
- React Router Data Mode with an in-memory router for application surfaces.
- CodeMirror 6 primary text/code editor.
- Accessible headless React primitives, semantic HTML, CSS Modules, and Clay
  theme tokens projected to CSS custom properties.
- Separate Rust Clay server remains canonical for documents, workspaces,
  packages, language services, persistence, permissions, and `deno_core`.
- Existing length-prefixed `rkyv` transport remains between Tauri Rust and the
  Clay server.
- Typed Tauri commands/channels form the React boundary. TauRPC is optional
  replaceable glue, accepted only after an exact-version spike.
- Existing two persistent package-runtime trust domains remain unchanged.
- Existing Clay-owned Prism `clay-agent` daemon remains. AG-UI becomes the
  React-facing event/state protocol over a custom Tauri channel transport.
- Product surfaces remain replaceable first-party packages. Third-party UI is
  declarative by default or isolated in a sandboxed surface without direct
  Tauri IPC.
- Linux remains the blocking development and CI host.

## Non-Negotiable Migration Rules

1. Server authority does not move into React, CodeMirror, Zustand, or Tauri
   managed state.
2. Ordinary typing applies locally before any React render, Tauri IPC, server,
   package JavaScript, file IO, or agent work.
3. Main webview receives narrow Clay commands only; broad filesystem, shell,
   process, and network plugin capabilities stay denied.
4. Existing package provenance, generation, permission, extension-point,
   replacement, and revocation checks remain server-enforced.
5. Stable SDUI node IDs, bounded snapshots/updates, and inert command intents
   remain the package UI contract.
6. Current feature behavior is ported before native client deletion. Native
   and web clients are never permanent parallel products.
7. Every phase updates affected public docs, implementation wiki pages,
   architecture maps, manual test modules, examples, and generated registries
   in the same phase.
8. No new product feature may delay parity unless it is required by the new
   architecture. LaTeX, notebooks, terminal, broad VSIX execution, and later
   agent profiles remain post-parity work.

## Definition of Current Feature Parity

Migration cannot cut over while any implemented behavior below lacks an
accepted Tauri/React equivalent or an explicitly approved removal decision:

- Launch, server discovery/spawn, reconnect, diagnostics, editable/read-only
  access, and graceful shutdown.
- File/folder dialogs, workspace roots, file browser, open/save/reload,
  conflicts, dirty state, duplicate-open routing, and path browsing.
- Optimistic versioned editing, leases, resync, undo/redo, clipboard, IME,
  keymaps, sequence chords, movement, selection, multi-cursor, snippets, and
  accessibility editing semantics.
- Syntax themes, Tree-sitter decorations, diagnostics, completion, LSP
  intelligence, folding, links, inlay hints, Markdown editing/preview, and
  current large-file limits.
- Splits, panes, tabs, per-tab workspaces, independent client connections,
  focus policies, resize/reorder/close, restore, and layout persistence.
- Command Centre, fuzzy command mode, dired-style path mode, modal focus,
  package provenance, keybinding details, and keyboard-only operation.
- `init.js`, modular configuration, hot reload, package loading, two runtime
  trust domains, package replacement/extension, package UI, settings, themes,
  typography, appearance, and Git discovery.
- `@clay/chat`, provider/model/agent/setup/session flows, persisted transcripts,
  cancellation, streaming, credential secrecy, and package-owned landing.
- Existing Linux security, accessibility, performance, protocol compatibility,
  documentation-as-code, and maintenance validation gates.

## Phase 1: Freeze, Baseline, and Parity Ledger

### Scope

- Freeze native-client feature expansion except release/security fixes needed
  to establish a trustworthy baseline.
- Run all Linux blocking gates and current Node tests; record failures without
  weakening tests or budgets.
- Build a machine-readable parity ledger mapping every current manual-test
  step, public API, protocol family, package contribution, UI surface,
  accessibility contract, performance budget, and security invariant to its
  target owner and migration phase.
- Record current screenshot/accessibility fixtures for representative editor,
  tabs/splits, Command Centre, package UI, theme, error/recovery, and Chat
  states.
- Classify source into keep, adapt, port, and delete sets. Preserve behavior
  tests even when implementation-specific native tests will later be replaced.

### Exit Gate

- Linux baseline is reproducible.
- Every implemented user-visible behavior has one target phase and test owner.
- Known failing checks are either fixed or explicitly blocking later cutover.
- No undocumented feature can disappear during migration.

## Phase 2: Tauri Workspace and Secure Desktop Skeleton

### Scope

- Add a Tauri v2 crate and React/Vite/TypeScript frontend without moving the
  existing server crate or server entry point.
- Add deterministic frontend dependency locking, formatting, linting, type
  checking, unit testing, and production build commands.
- Implement one main webview with strict CSP and minimal capability files.
- Implement local server discovery/spawn/connect/shutdown using Clay-owned Rust
  code. Package code receives no process handle or spawn capability.
- Provide launch, loading, server-error, reconnect, and unsupported-platform
  surfaces using semantic accessible HTML and Clay token placeholders.
- Establish Linux WebKitGTK prerequisites and CI build packaging smoke.

### Exit Gate

- Tauri opens on Linux, connects to a real Clay server, reports connection
  state, and shuts down without orphaning server/client resources.
- Main webview has no broad Tauri filesystem/shell/process authority.
- Existing native client remains runnable only as a temporary parity oracle.

## Phase 3: Typed Frontend Bridge and Session Bootstrap

### Scope

- Keep the server protocol and `rkyv` codec intact behind Tauri Rust.
- Define bounded JSON-compatible frontend DTOs for bootstrap, tab, document,
  runtime, menu, package UI, theme, diagnostics, language, and agent families.
- Use strings for identifiers that may exceed JavaScript safe integer range.
- Add typed Tauri commands for request/response operations and channels for
  ordered streams, cancellation, backpressure, reconnect, and resync.
- Run the TauRPC compatibility spike. Pin exact Rust/npm/Specta/Tauri versions
  if retained; otherwise use native Tauri commands with generated TypeScript
  DTOs. Keep either choice behind one frontend bridge module.
- Implement React bootstrap stores for connection/session state without making
  them canonical domain state.

### Exit Gate

- Real server bootstrap reaches React through validated typed messages.
- Malformed/oversized/stale frames fail closed before frontend installation.
- Reconnect and latest-state recovery work without full-document traffic for
  ordinary edits.
- Bridge choice is documented and replaceable.

## Phase 4: React UI, Design-System, Theme, and Accessibility Foundation

### Scope

- Establish application routes, shell composition, error boundaries, loading
  states, and narrow/wide desktop layout behavior.
- Port Clay component semantics to accessible React primitives, preferring
  React Aria Components and native HTML over custom ARIA widgets.
- Implement one frontend theme runtime. Validate existing theme package data in
  Rust, emit a resolved snapshot, map tokens to CSS custom properties, and
  adapt syntax/editor tokens to CodeMirror.
- Preserve user-owned `ui`, `monospace`, and `proportional` font roles plus the
  semantic typography hierarchy.
- Keep light/dark/system appearance, package themes, contrast rejection, live
  theme reload, density, spacing, radius, and state-token behavior.
- Create a deterministic UI fixture/review route for visual, responsive,
  interaction-state, and accessibility testing; do not add a second product UI.

### Exit Gate

- Core controls are keyboard-operable, named, focus-visible, themeable, and
  tested in light/dark plus large typography.
- No raw package CSS or concrete package font/size authority reaches host UI.
- Token/theme changes do not trigger package JavaScript or server work per
  animation frame.

## Phase 5: CodeMirror Editing and Versioned Synchronization Foundation

### Scope

- Mount CodeMirror directly through a focused React lifecycle adapter; editor
  state does not live in ordinary React component state.
- Port document open, local shadow state, transaction IDs, pending edit queue,
  optimistic application, acknowledgements, stale rejection, correction, and
  full resync.
- Define one reviewed UTF-16 line/column ↔ canonical UTF-8 byte conversion
  boundary and test Unicode, emoji, combining marks, CRLF, and malformed input.
- Batch ordinary edit transmission without delaying local paint.
- Port caret/selection/viewport retention, dirty/read-only status, save/reload,
  conflict recovery, and document close.
- Establish CodeMirror extension compartments for behavior manifest, language,
  theme, typography, keymap, read-only state, and decorations.

### Exit Gate

- Real files can be opened, edited, saved, rejected, corrected, and resynced
  against the existing server.
- Local typing remains responsive with a slow or absent IPC consumer.
- Unicode positions and server versions remain exact.

## Phase 6: Pane, Split, Tab, Workspace, and Persistence Parity

### Scope

- Port shell working area, generic pane-content host, split tree, fixed slots,
  divider drag, keyboard resize/move, focus policies, and four-pane cap.
- Port per-pane independent editor views and duplicate-open focus routing.
- Port tabs as independent server client connections with per-tab workspaces,
  active modes, documents, split trees, dirty-close protection, and tab order.
- Port keyboard tab/pane commands, numbered navigation, sequence chords, and
  user keybinding overrides.
- Port versioned layout persistence and hostile/corrupt-file fallback.
- Preserve focus, selection, viewport, and transient state by stable IDs during
  React reconciliation and tab/pane moves.

### Exit Gate

- Every current split/tab manual-test scenario passes in React.
- Cross-tab workspace/document/grant isolation remains server-enforced.
- Restore/reconnect does not duplicate clients or leak leases.

## Phase 7: Complete Editor, Rendering, and Language-Intelligence Parity

### Scope

- Port movement, selection, multi-cursor, cursor undo, text objects, smart
  select, snippets, comment/list/heading transforms, bracket behavior, and
  multi-stroke key routing.
- Port clipboard, IME composition, accessible editable-text semantics, caret
  shape/blink, ligatures, wrapping, horizontal scroll, prose column, gutters,
  active line, indent guides, and bracket matching.
- Adapt server-issued syntax, semantic, diagnostic, search, link, inlay, fold,
  and background decorations to CodeMirror extensions without adding a second
  parser or language authority in the frontend.
- Port completion ordering, exclusive providers, snippets, caret-anchored popup,
  stale/error dismissal, and accepted-item application.
- Port hover, definition, signature help, code actions, LSP diagnostics,
  folding, link activation, and inlay toggles through existing server services.
- Port Markdown source editing and current preview behavior using sanitized web
  rendering while retaining package/mode ownership.

### Exit Gate

- Editor and language parity ledger is complete.
- Current large-file, typing, scrolling, decoration, completion, and language
  service budgets pass or have stricter measured replacements.
- No package/mode-specific Rust branch is added for frontend convenience.

## Phase 8: React SDUI, Package UI, and Trust-Boundary Parity

### Scope

- Implement stable-ID React reconciliation for existing SDUI snapshots and
  updates. Preserve local focus, scroll, input, collapse, menu, and transient
  state when node identity survives.
- Port the package component catalog, fixed slots, pane-content contribution,
  overlays, modal containment, disabled/state semantics, action intents, and
  payload limits.
- Preserve package manifests as the single contribution source, explicit
  one-line `loadPackage`, extension points, `extends`/`replaces`, provenance,
  generation, revocation, rollback, and two persistent Deno trust domains.
- Define trusted first-party compiled UI registration without giving package
  logic Tauri authority.
- Keep third-party UI declarative by default. Add isolated custom surfaces only
  if a current parity requirement needs them; otherwise defer the mechanism.
- Port package settings, Git status, file-browser, and Chat entry composition
  onto the same component registry.

### Exit Gate

- Existing first-party packages load unchanged or through a documented schema
  migration and retain behavior/UI parity.
- Third-party runtime cannot import internal modules, call internal ops, access
  Tauri, inject host CSS, or impersonate first-party provenance.
- Package disable/reload/replacement cleans up UI and executable authority.

## Phase 9: Command Centre, Configuration, Settings, and Desktop Workflow Parity

### Scope

- Port the centered Command Centre, scrim, fuzzy command list, path browser,
  provider/model/agent/session pickers, modal focus containment, result-count
  announcements, and keyboard-only operation.
- Port native file/folder dialogs through narrow Tauri commands while keeping
  server-issued grants and browse authority unchanged.
- Port `init.js` configuration diagnostics, modular imports, watcher-triggered
  hot reload, runtime generation replacement, and atomic frontend state install.
- Port settings UI for appearance, themes, typography, package state, and
  documented preferences without inventing a second configuration store.
- Port status/recovery/error surfaces, workspace/file browser, Git discovery,
  clipboard commands, and desktop shortcuts.

### Exit Gate

- Existing configuration, Command Centre, path, settings, and file workflows
  pass automated and manual parity checks.
- Secrets and absolute paths remain absent from logs, menu snapshots, rendered
  errors, and accessible names.

## Phase 10: AG-UI Agent and Chat Parity

### Scope

- Keep one Clay-owned Prism `clay-agent` daemon and existing Rust server process
  manager, credential vault, provider kernel, and persisted session store.
- Add a server adapter from bounded Prism/Clay events to AG-UI lifecycle,
  message, tool, state snapshot/delta, raw/custom, cancellation, and error
  events.
- Implement a custom AG-UI `AbstractAgent` transport over Tauri commands and
  channels; do not open a localhost HTTP/SSE listener.
- Port `@clay/chat` landing, composer, transcript, provider/model/agent/setup,
  session list/resume/delete, streaming, cancellation, thinking, usage, and
  unconfigured-provider states.
- Preserve package ownership: replacing/disabling `@clay/chat` changes product
  presentation/profile registration but cannot access daemon/process/secrets.
- Keep ACP and coding-agent-only tools, diffs, MCP, PTY, and AI-safe mutation out
  unless they are already implemented when the Phase 1 parity ledger freezes.

### Exit Gate

- Current Chat workflows pass through AG-UI with no duplicate event model in
  React.
- Credentials never enter AG-UI state, logs, snapshots, DOM attributes, or
  accessibility names.
- Agent streaming never blocks editor input or unbounds transcript memory.

## Phase 11: Remote, Platform, Packaging, and Operational Hardening

### Scope

- Verify local, remote, container, and multi-client server connections through
  the same frontend bridge without moving remote authority into the webview.
- Test Linux WebKitGTK first; retain practical Windows/macOS support without
  weakening Linux behavior.
- Add Tauri packaging, application identity, icons, desktop integration,
  updater signing/configuration, crash diagnostics, and server/agent artifact
  bundling.
- Add CSP/capability regression tests, dependency audit policy, SBOM/license
  checks, and packaged-install smoke tests.
- Establish frontend bundle, startup, memory, tab switch, editor, Command
  Centre, SDUI, and agent-stream budgets.

### Exit Gate

- A packaged Linux build installs, launches, edits, reconnects, updates through
  a safe test channel, and uninstalls cleanly.
- Remote/container scenarios retain current server authority and security.
- Blocking Rust, TypeScript, frontend test, audit, and package smoke gates pass.

## Phase 12: Parity Certification, Native Client Removal, and Documentation Cutover

### Scope

- Execute every parity-ledger automated and manual check on the Tauri/React
  build. Retain screenshot, accessibility-tree, performance, security, and
  packaged-build evidence.
- Resolve every gap; do not classify missing behavior as parity by deleting or
  weakening its test. Product removals require separate user approval and a
  superseding decision log.
- Make Tauri the default `clay` desktop launch path while keeping `clay server`
  independently runnable.
- Delete native client, Masonry/Vello/Parley/winit dependencies, local
  Masonry/AccessKit patches, native-only UI modules, obsolete benchmarks,
  obsolete fixtures, and native-only launch code.
- Rewrite architecture, development, security, performance, accessibility,
  package authoring, primitive reference, Clay JS API, manual-test, code-wiki,
  examples, build, platform, and contribution documentation to describe only
  the implemented target architecture. Preserve historical plans and decision
  logs as history; mark superseded decisions rather than rewriting them.
- Add deterministic searches/tests preventing stale claims such as "native
  Masonry client", "no AG-UI", direct package widget authority, or obsolete
  source paths in current documentation.

### Exit Gate

- Parity ledger has zero unresolved rows.
- Tauri/React is the only production desktop client.
- Linux format/check/clippy/tests, frontend lint/typecheck/tests/build, Node
  daemon tests, audits, documentation registry checks, wiki navigation checks,
  manual test plan, visual/a11y review, and packaged smoke all pass.
- Repository current-state documentation is internally consistent; historical
  decision logs and completed plans remain clearly historical.

## Post-Parity Work (Not Part of This Migration)

After Phase 12, create separate approved plans for:

- Terminal emulator and PTY support.
- JSON/YAML/TOML structured editing beyond current text-mode parity.
- Python, Jupyter kernels, and `.ipynb` cell UI.
- LaTeX/TexLab/Tectonic/PDF.js workflows.
- Rich PDF/Markdown cross-document links.
- Coding Agent, AI-safe mutation, diffs, approvals, tools, MCP, and sandboxing
  if not already present in the Phase 1 parity ledger.
- Personal, Work, Research, Finance, and Clay meta-agent packages.
- Declarative VSIX asset import; browser extension-host subset only after a
  separate compatibility/security decision.
