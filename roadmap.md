# Clay Implementation Roadmap

## Phase 22: Window Management with Splits and Tabs

Give Clay real multi-view window management, delivered incrementally: first
equal-area window splits, then keyboard-driven tabs where each tab behaves as
an independent client (own workspace, files, and modes), then composition of
tabs with splits. Confirmed architecture decisions: each tab owns its split
tree (one workspace per tab, panes inside a tab view files of that tab's
workspace); each tab is a real separate client connection with
server-authoritative tab state; panes are generic workspace-bound content
hosts (editor file views first, later workspace apps such as a terminal
emulator), never tied to files in the split model. Reuses the existing shell
layout machinery (`PaneSplitTree`/`PaneSlotLayout` in `src/shell/layout.rs`,
divider chrome in `src/shell/primitives.rs`, `layout.json` persistence, and
the JS keybinding system) instead of introducing a new layout stack.

### Phase 22.1: Equal-Area Window Splits

Focus areas:

- Split commands act only on the working area (the main zone, excluding the left/right/top/bottom fixed panel slots) and come in two families: (a) split the focused pane 50/50, with one horizontal and one vertical command (`SplitRatio` 0.5 on the existing `PaneSplitTree`); (b) an add-pane command that redivides the whole working area into N+1 equal areas, keeping the previously opened panes in reading order and leaving the new pane empty and free to load content.
- Maximum 4 panes per tab (up to 4 divisions along any single axis; 4 leaves total); equal-area geometry invariants and structural tests enforce the cap.
- Extend `ClayShellWidget` beyond `single_editor` so each pane leaf hosts a pane content widget with stable identity; retain Masonry reconciliation semantics for pane children. The pane host is content-type agnostic and workspace-bound — the first content type is the editor file view, but the split model must already accommodate later workspace apps (e.g. a terminal emulator package) in a pane.
- Clear visible pane boundaries using the existing `paint_divider` chrome with theme tokens; active pane indicated with existing focus-ring primitives.
- Pane resize on the focused split: mouse/trackpad divider drag (existing Phase 20.3 behavior) plus new keyboard resize commands.
- Pane movement: move/reorder the focused pane to a different position within the same tab.
- Pane focus behavior configurable with two options — focus follows cursor, or click-to-focus — with a defined default.
- Commands plus default key bindings for: split focused pane horizontally/vertically, add equal pane, close active pane, focus next/previous pane (building on `next_pane`/`prev_pane`), pane resize, pane move.

Expected outcome:

- Users can split the editor zone into up to 4 equal panes with visible boundaries, add panes that redivide the whole working area equally, resize and reposition panes, and move focus between panes entirely from the keyboard, while side/top/bottom panels stay untouched.

### Phase 22.2: Pane Document Views Within One Workspace

Focus areas:

- Each tab loads exactly one workspace; each pane is an independent view that can open and edit a file from that tab's workspace, so multiple files are edited simultaneously.
- Per-pane major mode activation: panes in the same tab showing different file types (e.g. `.rs` and `.md`) each activate their own major mode concurrently.
- Per-pane caret, selection, viewport, and shadow-sync state via the existing session/edit-queue machinery; pane focus decides keyboard routing and status context.
- Duplicate file opens are blocked within one workspace: opening a file already open in another pane of the tab focuses the existing pane instead of opening a second view.
- File-open flows (file browser, open-selected-file) target the focused pane.

Expected outcome:

- Panes behave as simultaneous file views of a single workspace with correct optimistic-editing sync and no cross-pane state bleed.

### Phase 22.3: Tabs as Independent Client Views with Tab Bar

Focus areas:

- Tab model where each tab is a real separate client: opening a tab establishes its own client connection to the server (within existing connection caps), with independent workspace roots, open documents, active modes, and retained sessions, isolated from other tabs.
- Tab state is server-authoritative: the server holds the tab registry (tab order, active tab, per-tab workspace and client binding) so tab structure survives client restarts and reconnects, consistent with the client-authority model, lease accounting, and connection caps.
- Tab bar chrome at the top of the window (below the top fixed panel slot, above the working area) shown whenever multiple tabs are open; each tab card shows the workspace name loaded in that tab and carries a close button; click switches tabs.
- With one tab open, behavior and chrome match today's layout.

Expected outcome:

- Multiple tabs run different workspaces and modes simultaneously, and the tab bar always shows which workspace each tab holds.

### Phase 22.4: Keyboard Tab Management

Focus areas:

- Default key bindings: view specific tab by number (1..9), next/previous tab, new tab, close tab, move tab left/right, move tab to a specific position.
- Every Phase 22 key binding (tab and split commands alike) is user-editable through `init.js` via the existing keybinding system.
- Commands registered with correct routing policies and listed in the command/help surfaces; tab-count bounds, numbered switch beyond 9, and wraparound policy for next/previous defined explicitly.
- Close-tab safety for dirty documents (confirm/save flow consistent with existing document-close behavior).

Expected outcome:

- Every tab operation is reachable from the keyboard and user-rebindable.

### Phase 22.5: Tab × Split Composition and Persistence

Focus areas:

- Each tab owns its split tree and per-pane state (one workspace per tab); split/close/focus-pane commands operate within the active tab only.
- Persist and restore tab order, active tab, per-tab workspace, per-tab split tree, and per-pane open documents across restarts and reconnects (extends `layout.json` persistence).
- Tab move/reorder keeps each tab's internal state intact.

Expected outcome:

- Tabs and splits compose cleanly, and a full window state survives restart.

### Phase 22.6: Hardening, Accessibility, and Documentation

Focus areas:

- Accessibility roles/names for tab bar, tab cards, panes, and focus movement; screen-reader announcements for tab switch and split changes.
- Performance budgets for per-pane paint, tab switch latency, and multi-pane decoration traffic; CI-guarded like existing Phase 14/16 budgets.
- Authority review: per-tab workspace grants and package scopes cannot leak across tabs.
- Protocol compatibility, tests, primitive reference docs, generated registry entries, and wiki updates.

Expected outcome:

- Window management is production-safe, documented, and performance-bounded.

## Phase 24: Command Centre

Give Clay a single keyboard-first command centre surface — one floating
Spotlight-style overlay with two modes: a command execution mode (all
registered commands, filterable, showing package provenance and key bindings)
and a dired-style filesystem browsing mode (editable path bar, drill into
directories, filter-as-you-type, open files into the active pane or open a
directory as the workspace of the current tab). Confirmed decisions: the
client keybinding router is extended to multi-stroke sequences so Emacs-style
chords are supported; filtering uses real fuzzy matching, not substring; the
backdrop is a translucent scrim only — no custom blur beyond what Masonry and
Vello provide upstream; a file selected in path mode opens in the active pane
of the current tab, respecting per-tab workspace isolation. The work reuses
the existing building blocks — `ControlCenter` (`src/server/control_center.rs`),
`TransientMenuSession` (`src/shell/transient_menu.rs`), `FileBrowserState`
(`src/shell/file_browser.rs`), the built-in command table
(`src/server/command_execution.rs`), and the per-tab workspace binding
(`src/server/tab_registry.rs`) — most of the effort is wiring, not new systems.

### Phase 24.1: Transient Menu Interaction Round-Trip

Focus areas:

- Protocol messages for menu interactivity: client-to-server `MenuQueryUpdate`,
  selection movement, `MenuActivate`/`MenuCancel` intents keyed by
  `TransientMenuSessionId`, and a server-to-client transient menu snapshot
  carrying the bounded, filtered `TransientMenuSession`.
- Server session ownership in `src/server/connection.rs`: one active menu
  session per tab; intents mutate server-owned session state and push a fresh
  snapshot; sessions time out and cancel cleanly on tab switch or disconnect.
- Client keystroke routing: while a menu session with modal focus policy is
  active, key events feed the menu query/selection instead of the editor;
  rendering goes through the existing `set_active_menu`/overlay projection in
  `src/masonry_sdui.rs`.
- Bounded payloads: item counts, label/detail/query lengths stay within the
  existing `TRANSIENT_MENU_*` budget constants.

Expected outcome:

- Any `TransientMenuSession` is fully interactive end-to-end: open, type to
  filter, move selection, activate, cancel — all server-authoritative.

### Phase 24.2: Command Execution Mode

Focus areas:

- Default key binding for `controlCenter.open`; executing it opens a
  `ControlCenter` session server-side and pushes the menu snapshot through the
  Phase 24.1 round-trip instead of returning a bare `Accepted`.
- Include the `shell.client*` command family (splits, pane focus/resize/move,
  tab management) in the listing: activating one closes the menu and dispatches
  through the existing `ShellClientCommand::from_command_id` client path,
  since these require client UI authority.
- Item detail already surfaces key binding, routing policy, and package
  provenance (`built-in` or `name@version`); verify coverage for every
  built-in command, every `shell.client*` command, and package-registered
  commands (markdown, javascript/typescript comment toggles, settings, and
  runtime-registered contributions).
- Fuzzy matching replaces the current substring filter: a small Clay-owned
  subsequence-scoring matcher shared by all transient menus, with ranking
  (word-boundary and consecutive-match bonuses) and bounded candidate scans.

Expected outcome:

- One keybinding opens the command centre listing every executable command
  with its package and key binding shown; typing fuzzy-filters; Enter runs it
  through the shared command execution path.

### Phase 24.3: Path Mode — Dired-Style Filesystem Browsing

Focus areas:

- New `PathBrowserSession` state (sibling of `FileBrowserState`): editable
  path bar seeded with the active document's directory (fallback: tab
  workspace root, then cwd), a bounded depth-1 listing snapshot, and
  filter-as-you-type over the listing using the Phase 24.2 fuzzy matcher.
- Dired navigation semantics: activating a directory descends into it;
  Backspace on an empty query ascends; the path bar can be edited directly to
  jump to any path; listings stay bounded (`max_depth: 1`, entry caps) and are
  never read on the paint/layout path.
- User-authorized browse grant: navigation inside this built-in surface
  implicitly authorizes traversal outside granted workspace roots, consistent
  with the unified user-authorized authority decision; opening a file converts
  to an explicit `SingleFile` grant and opening a folder as workspace converts
  to a `Directory` root grant. Package code receives no equivalent authority.
- Activations: a file opens in the active pane of the current tab (duplicate
  open focuses the existing pane, per Phase 22.2); a directory offers descend
  (default) and open-as-workspace-for-this-tab (secondary key), the latter
  routed through the existing `TabRegistry::open_workspace` binding and
  per-tab snapshot push, preserving per-tab workspace isolation.
- Native folder dialog (`src/client/file_dialog.rs`) remains as fallback, not
  the primary flow.

Expected outcome:

- One keybinding opens the command centre in path mode with the current
  directory loaded; the user can change path, filter, drill into folders,
  open any file into the active pane, or load a folder as the tab's workspace
  — all without leaving the keyboard.

### Phase 24.4: Centered Floating Surface with Scrim Backdrop

Focus areas:

- New `TransientMenuOrigin::Centered` variant: the overlay host anchors the
  menu at window center with a token-driven width, following the existing
  origin-to-anchor/focus-policy pattern from Phase 20.5.
- New `scrim` theme tokens (color + opacity) in the theme catalog; the
  overlay host paints a translucent scrim over the shell behind the menu.
  No custom blur: only what Masonry 0.4/Vello provide upstream is used, and
  true backdrop blur is deferred unless upstream gains a filter pass.
- Both command and path modes adopt the centered surface; bottom-anchored
  origins remain for completion pickers and context menus.
- Accessibility: role/name for the centred dialog, focus trap while modal,
  screen-reader announcements for filtered result counts, and full keyboard
  operability already required by Phase 20.5/22.6 conventions.

Expected outcome:

- The command centre presents as a Spotlight-style floating panel that dims
  the Clay background, works identically for both modes, and meets the
  existing accessibility and performance budget conventions.

### Phase 24.5: Sequence Keybindings and Hardening

Focus areas:

- Extend the client keybinding router (`route_key` in
  `src/client/behavior.rs`) from single-stroke matching to multi-stroke
  sequences with a pending-chord state, timeout, and cancel-on-mismatch, so
  Emacs-style chords (e.g. a prefix chord for path mode) are bindable through
  the existing keybinding system and `init.js`.
- Default bindings for command mode and path mode assigned as chords;
  conflict/ambiguity validation extended to sequence prefixes.
- Performance budgets for menu open latency, per-keystroke filter updates,
  and listing snapshot sizes, CI-guarded like existing budgets; authority
  review confirming the browse grant cannot be reached by package code.
- Protocol compatibility tests, primitive reference docs, generated registry
  entries, and wiki updates.

Expected outcome:

- Multi-stroke keybindings work everywhere keybindings do, the command centre
  is performance-bounded and documented, and the new authority surface is
  review-clean.

## Phase 25: AI-Native Entry, Prism 0.3.0 Host, and Chat

Make Clay greet the user as an AI-native workspace, not a text editor waiting
for a folder. Product surfaces are first-party packages on Clay primitives:
`@clay/chat` owns the default landing and the Chat profile; Phase 29's
`@clay/coding-agent` owns the coding profile. Users load them with one-line
`loadPackage`. A third-party package may `replaces` the landing with a
completely different page, or `extends`/`replaces` the coding-agent package,
through the existing package graph and user approval. Clay core owns the
Prism host, credentials, IPC, catalog widgets, and Command Centre — not the
greeting copy.

With `@clay/chat` loaded, launch and every new tab open that package's entry
surface: greeting copy ("What do you want to do today?"), a focused composer,
agent/provider/model pickers, and Open File / Open Folder as secondary
actions. Chat works with no workspace. Without an entry-surface package, core
fallback is Open File / Open Folder only. This phase adopts `@arnilo/prism`
**0.3.0** as the agent runtime and ships generic host primitives later
special-purpose agent packages reuse. Coding-agent tools, diffs, and AI-safe
mutation move to Phase 29.

This recasts the previous ACP-first Phase 25 draft. ACP v1 remains optional
later interop (Zed-class editor protocol), not Clay’s native agent bus.
Chat, provider setup, model selection, and multi-agent routing use Prism’s
own `createAgent` / `createAgentSession` / `AgentEvent` / `AgentDefinition`
surfaces. One Clay-owned Node daemon hosts every agent profile.

Working architecture (supersedes the 0.2.6 ACP-client draft):

- Prism runs in a spawned Node >= 20 child (`clay-agent`), never inside
  `deno_core`. The process boundary is the trust boundary; package JS cannot
  spawn or speak to the daemon.
- Clay server owns the agent protocol over existing GUI IPC. The daemon
  speaks a Clay-owned, bounded, stdio JSON-RPC wrapping Prism `AgentEvent`.
  The event union is closed and already includes tool and permission
  variants so Phase 29 does not rewrite IPC; Chat never emits them. Do not
  pull AG-UI or ACP into the Rust client or `clay-agent`. Do not use
  `prism --mode rpc` as the product transport.
- Clay owns credentials and feeds them through Prism credential resolvers
  (`@arnilo/prism-credentials-node`). Prism never reads `process.env`.
  Secrets never appear in events, transcripts, menu snapshots, logs, or a11y
  names.
- Every first-party `@arnilo/prism-provider-*` package shipped in 0.3.0 is
  loaded through the extension kernel. Auth UI is data-driven from each
  package’s `registerAuthMethod` descriptors (`api_key` vs `oauth`), not a
  hand-written per-provider screen. OpenAI-compatible custom endpoints cover
  unknown vendors. Cursor SDK is not a Prism 0.3.0 package and is out of
  scope.
- Agent profiles are Prism `AgentDefinition` values registered by packages
  (and later on-disk `AGENT.md` bundles). This phase's `@clay/chat` registers
  **Chat** (no tools). **Coding Agent** is registered by `@clay/coding-agent`
  in Phase 29, not a core disabled stub. Work / PA / Research / Finance are
  later first-party packages.
- User-selected agent for now. Task-based auto-routing is deferred.
- Provider and model pickers are Command Centre session kinds, not native
  dropdown widgets. Entry-surface buttons open the same sessions.
- Empty pane hosts the loaded entry-surface package (default `@clay/chat`),
  not an empty editor document with “Ready to edit” copy. Open File still
  opens an editor in the pane; Open Folder binds a workspace to the tab and
  leaves the entry surface in place. Core without that package is file/folder
  fallback only.
- Pin `@arnilo/prism` and first-party packages to exact **0.3.0** for the
  first cut; upgrades are reviewed events. Prism 0.3.0’s independent
  patch/minor line does not mean Clay floats versions.

### Phase 25.1: `clay-agent` Daemon (Prism 0.3.0 Host)

Focus areas:

- First-party TypeScript package `clay-agent` embedding `@arnilo/prism@0.3.0`,
  `@arnilo/prism-providers` plus the enterprise adapters Prism umbrellas omit
  (`@arnilo/prism-provider-azure`, `-bedrock`, `-vertex`),
  `@arnilo/prism-credentials-node`, `@arnilo/prism-session-store-sqlite`,
  `@arnilo/prism-model-router`, and `@arnilo/prism-tool-validator-json-schema`.
  Do not load coding-agent, coding-security, ACP, MCP, browser, or web-tools
  packages in this phase.
- Extension kernel loads every first-party provider package with host-supplied
  credential resolvers. Model catalogs come from each package’s static
  featured list; caller-gated `list*Models` runs only on explicit user refresh,
  never at setup.
- `createAgent` / `createAgentSession` per Clay session. Profiles are
  `AgentDefinition`s registered by Clay packages against the same kernel,
  registries, store, and credential resolver. `@clay/chat` registers Chat:
  no tools, no skills, package-owned system prompt. The daemon does not
  hard-code Chat as the only profile.
- SQLite session/run store under Clay’s per-user data directory. Sessions
  survive daemon restart. Encrypted credential vault (and OS keychain when
  available) in the same data dir; no silent plaintext fallback.
- Stdio JSON-RPC: session new/list/load/resume/delete, prompt, cancel, steer,
  provider list/status, model list/search, credential put/oauth-start/oauth-
  poll/delete, agent-profile list. Bounded payloads, redacted errors, graceful
  shutdown, structured logs with secret scrubbing.
- Node >= 20 detection with a clear failure message. Daemon is Clay-core
  owned, not a package-triggered process grant.

Expected outcome:

- `clay-agent` is spawnable standalone. A prompt against a configured mock or
  live provider streams `AgentEvent`s over stdio and persists the session
  without any editor, ACP, or tool involvement.

### Phase 25.2: Clay Server Agent Protocol and Process Manager

Focus areas:

- Agent process manager next to existing server connection machinery:
  spawn/restart/health-check/log-capture for `clay-agent`, one daemon per
  Clay server (not per tab). Tabs multiplex sessions through it.
- Clay IPC in `src/protocol`: agent session ops, streaming event snapshots,
  provider/model inventory, credential setup intents that never echo secrets,
  agent-profile selection. Same compatibility-test gate as other ops.
- Server is the authority for session identity, selected profile/provider/
  model, and transcript snapshots. Client renders and forwards composer
  input. Typing in the editor hot path never waits on the daemon.
- New reserved core API domain `agent` (`RESERVED_CORE_API_DOMAINS`).
  Commands such as `agent.serverPrompt`, `agent.serverCancel`,
  `agent.serverRegisterProfile`, `agent.clientOpenProviderPicker`,
  `agent.clientOpenModelPicker`, `agent.clientOpenAgentPicker`,
  `agent.clientOpenProviderSetup`.
- Last-used provider/model are documented `init.js` configuration APIs.
  The live default profile is whichever loaded package registered one
  (`loadPackage("@clay/chat")` in the canonical example), not a silent
  compiled Chat surface.

Expected outcome:

- GUI can create a chat session, stream a reply, cancel, and resume after
  restart, all through typed protocol messages with CI-guarded compatibility
  tests. No ACP crate on the Rust side.

### Phase 25.3: Empty-Tab Pane Content and `@clay/chat`

Focus areas:

- Open the Phase 22 pane-content contribution path (today not public). Empty
  / new-tab `main` hosts at most one validated package SDUI tree through the
  existing `PackageRegionWidget`. No Chat-named pane kind. Later terminal
  stays a distinct kind (PTY ≠ SDUI).
- Core fallback when no contribution is loaded: keep a slim `WelcomeWidget`
  with Open File / Open Folder only. No fake welcome text document.
- First-party `@clay/chat`: bundled, explicit `loadPackage("@clay/chat")`.
  Registers the Chat profile and the default entry surface (greeting, agent/
  provider/model buttons, Open File, Open Folder, focused composer). Chat
  works with no workspace. Greeting copy lives in the package.
- Open File still routes `documents.clientOpenFileDialog` and replaces the
  pane with the editor on `DocumentOpened`. Open Folder still routes
  `workspace.clientOpenFolderDialog` and binds the tab workspace without
  dismissing the entry surface.
- Catalog: reuse `button`, `list`, `scroll`; add generic multiline `textArea`
  if `textInput` cannot host the composer. No `agentChat` one-off. Command
  Centre remains host-owned (not a package dropdown).
- `@clay/chat` declares extension points (`entrySurface`, `chromeActions`).
  A third-party package may `replaces` `@clay/chat` (user approval) and ship a
  different landing. Replacement stays in the third-party runtime.
- Connection/runtime diagnostics stay visible. Global keybindings keep
  working while the composer has focus.

Expected outcome:

- With `@clay/chat` loaded, opening Clay or a new tab shows the greeting and
  a ready composer. File and folder remain one click away. Chat does not
  require a workspace. Without the package, only the core file/folder
  fallback appears. A replacement package can own the landing instead.

### Phase 25.4: Command Centre Provider, Model, Agent, and Setup

Focus areas:

- New `TransientMenuSession` kinds on the existing centered Command Centre
  (no second overlay system, shared fuzzy matcher): agent picker, provider
  picker, model picker, provider setup.
- Entry-surface buttons invoke the same commands as the Command Centre.
  There is no parallel dropdown widget.
- Provider picker lists every loaded Prism provider with configured/
  unconfigured state. Last item is “Configure provider…”, which opens setup.
- Provider setup is data-driven from Prism auth-method descriptors: API-key
  secret field (masked, never snapshotted) or OAuth device-code (user code +
  poll, then store). Custom OpenAI-compatible provider: base URL + key.
  Successful setup makes that provider selectable immediately.
- Model picker lists models from configured providers only, fuzzy-searchable,
  grouped by provider. Explicit catalog refresh is a command, not a
  background fetch on every keystroke.
- Agent picker lists registered profiles only (Chat when `@clay/chat` is
  loaded). Coding Agent appears when `@clay/coding-agent` loads in Phase 29.
  Work / PA / Research / Finance appear when those packages load.
- All four flows are also reachable as ordinary Command Centre commands so
  keyboard-only use never needs landing buttons.

Expected outcome:

- User configures a provider (key or OAuth), picks a model, picks Chat, and
  starts typing — from the `@clay/chat` entry surface or entirely from
  Command Centre.

### Phase 25.5: Chat Transcript and Session UX

Focus areas:

- Server-authoritative transcript: user/assistant/thinking/error/usage.
  `@clay/chat` projects it through catalog list/scroll. No client-side model
  calls. A replacement landing may project the same snapshots differently.
- Composer: Enter sends, a documented chord inserts newline, Escape cancels
  an in-flight run. Empty submit is a no-op.
- Session list/resume/delete in Command Centre. New tab starts a new session;
  restoring a session reopens the entry surface with history from the SQLite
  store (redacted, bounded).
- Unconfigured-provider empty state tells the user to configure a provider;
  it does not fail as a generic server error.
- No tools, no approvals, no diffs, no MCP, no slash-command runtime in this
  phase. Thinking/usage render when Prism events carry them.

Expected outcome:

- A configured user with `@clay/chat` loaded can have a multi-turn LLM
  conversation in Clay, cancel it, and resume it after restart.

### Phase 25.6: Hardening, Budgets, and Documentation

Focus areas:

- Budgets: daemon spawn, prompt-to-first-delta, per-delta IPC, transcript
  snapshot size, menu open/filter. CI-guarded like existing budgets. Deltas
  never block keypress-to-local-paint.
- Security review: child-process privileges, credential vault permissions,
  OAuth redirect/device-code honesty, no secret leakage in protocol/logs/
  a11y, package-code denial of daemon access, truthful “no sandbox / no
  tools” language for Chat.
- Protocol compatibility tests for every new IPC message; Clay JS API docs
  and `examples/init.js` / `examples/packages/first-party.js` for `agent.*`
  commands, `loadPackage("@clay/chat")`, and last-used model options;
  generated registry; wiki; manual test plan; `clay-ui` catalog update for
  pane-content contribution and `textArea` if added.
- Dependency policy: exact 0.3.0 pin, upgrade checklist. First-party Clay
  never speaks ACP or AG-UI. Revisit only if a later product goal is
  third-party ACP agents or a web front-end sharing this daemon.

Expected outcome:

- Chat-first Prism 0.3.0 host is performance-bounded, review-clean, and the
  stable base `@clay/coding-agent` (Phase 29) and later agent packages extend
  without a second daemon or a second credential store.


## Phase 26: Editor Rendering Quality Foundation

Fix the defects and missing paint primitives found in the 2026-08-18 editor
implementation review so rendered text looks right in every mode before new
formats are added. Confirmed decisions: decorations gain a theme-owned
background color axis and foreground colors become opaque
(`decision-logs/2026-08-18-1758-decoration-background-axis.md`), and document
typography gains a bounded theme-owned per-token size ladder
(`decision-logs/2026-08-18-1758-document-typography-size-ladder.md`). The
two-axis vocabulary (TokenType + Modifiers), theme single-source-of-color,
and optimistic decoration interpolation are confirmed correct and must not be
regressed.

### Phase 26.1: Default Theme Opacity and StyleSpec Contract Repair

Focus areas:

- Replace every `0x55`/`0x2f`-alpha entry in `StyleRegistry::clay_default()`
  (`src/editor/theme.rs`) with opaque foreground text colors; same fix for
  the semantic fallback tint. Verified visually in light and dark contexts.
- Rewrite the `StyleSpec` doc comment ("background tint") to the actual
  contract: opaque foreground color plus optional background axis.
- Theme token coverage: give `theme-modus-operandi`, gruvbox, and the default
  distinct colors for currently-dormant vocabulary entries (`Macro`,
  `Property`, `Method`, `Parameter`, `EnumMember`, `Operator`, …) so richer
  queries (26.2) light up immediately.

Expected outcome:

- Default-theme code renders at full opacity with a legible, distinct token
  palette; no theme maps two token types to visually identical output.

### Phase 26.2: Capture-Rich Highlight Queries and Style Maps

Focus areas:

- Rewrite `packages/{rust,typescript,javascript}/queries/highlights.scm` from
  the current ~9-capture POC set to nvim-treesitter-class capture sets:
  boolean/null literals, macro invocations (`println!`), operators, fields,
  constants, lifetimes, attributes (`#[derive]`), method calls, parameters,
  type parameters, punctuation tiers for Rust; analogues for TS/JS
  (properties, optional chains, regex, JSX tags) and Markdown (emphasis
  levels, link text vs URL, fence info strings).
- Extend the compiled-in `DEFAULT_NATIVE_STYLE_MAP`/
  `MARKDOWN_NATIVE_STYLE_MAP` (`src/server/syntax.rs`) so every new capture
  maps onto the closed `TokenType`+`Modifiers` vocabulary — data changes
  only, no engine changes.
- Add query-contract tests: every capture name in each `.scm` resolves to a
  vocabulary entry or is explicitly inert.

Expected outcome:

- A Rust/TS/JS/Markdown file renders with full token differentiation under
  any theme, using the dormant half of the existing vocabulary.

### Phase 26.3: Decoration Background Axis and Layered Fills

Focus areas:

- Add the optional background axis to `StyleSpec` through decoration
  chunking (`src/protocol/decorations.rs`), budget accounting, rkyv
  serialization, and `VisibleTextStyleRun` normalization; paint fills run
  backgrounds before text in the parley/vello path.
- Implement client-side `DecorationKind::SearchMatch` painting on the new
  axis (layer-rank plumbing already exists).
- Markdown mode paints fenced code blocks and block quotes as background
  panels (tinted blocks) driven by existing `CodeBlock`/`Quote` tokens —
  package data only, no Rust markdown branches.
- LSP bridges map "unused symbol" fades/dead-code dims to background-axis
  decorations where the server reports them.

Expected outcome:

- Backgrounds, search highlights, code-fence panels, and quote tints render
  through one axis owned by the theme; no new `DecorationKind`s for what is
  a paint property.

### Phase 26.4: Document Typography Size Ladder

Focus areas:

- `StyleRegistry` gains a bounded per-`TokenType` scale ladder (heading
  1.0/0.87/0.75…, small/code 0.9) mirroring `UiTypographyHierarchy`
  (`src/editor/typography.rs`); applied per-run in `rebuild()`
  (`src/editor/layout.rs`) next to the font-role override; themes override
  the ladder like any style.
- Reconcile line metrics: the single `document_line_height =
  max(mono, prop) × 1.4` approximation breaks with mixed-size lines; adopt
  per-line metrics (or a recalibrated uniform height with a documented
  ceiling) and keep logical viewport/scroll math consistent with painted
  lines.
- Prose rendering validation: heading hierarchy, mixed mono/prose lines, and
  wrapped headings checked in manual test plan screenshots.

Expected outcome:

- Markdown headings render as a real typographic hierarchy in every theme;
  scroll/viewport math stays correct on mixed-size documents.

### Phase 26.5: Editor Chrome — Gutter, Active Line, Bracket Match, Indent Guides

Focus areas:

- Line-number gutter as a generic client chrome surface: token-styled
  (theme-owned colors), configurable width/visibility, correct alignment
  under mixed line heights, never on the hot layout path.
- Active-line highlight and indent guides as theme-token-driven chrome with
  per-mode configuration defaults (on for code modes, off for prose unless
  configured).
- Bracket-match highlight: reuse the existing matching-pair scan in
  `src/editor/buffer.rs` (currently only used for electric indent) to paint
  matched-pair ranges when the caret is adjacent to a bracket declared in
  the active behavior manifest.

Expected outcome:

- Code modes get the affordances of a real editor; all chrome is generic,
  token-driven, and available to every mode with zero package code.

### Phase 26.6: Layout Geometry — Insets, Wrap Policy, Prose Column

Focus areas:

- Replace the uniform 48px `TEXT_INSET` with asymmetric, token-driven insets;
  define a prose column cap (bounded max line width) for proportional modes
  while code modes keep full-width behavior.
- Introduce a `WrapPolicy` primitive (`none | viewport | column`): `none`
  enables horizontal scrolling (scroll plumbing + caret visibility beyond
  width), `viewport` is today's soft wrap, `column` caps wrap width; declared
  per mode in behavior manifests with user override via `init.js`.
- Address the viewport simplification noted in the review: the visible
  snapshot uses a logical-line window (`viewport.visible_range`) that breaks
  with wrapped lines and proportional fonts; derive the visible range from
  painted visual lines.

Expected outcome:

- Long-line code files scroll horizontally instead of wrapping; prose reads
  at a sane column; insets and wrap behavior are mode- and user-configurable.

### Phase 26.7: Rendering Hardening, Accessibility, and Documentation

Focus areas:

- Fix the client panic in `accesskit_consumer` ("Focused ID #4 is not in the
  node list") when closing a dirty pane via Ctrl+Alt+W — accessibility tree
  must drop focus references before the pane widget is removed.
- Performance budgets for gutter/active-line/bracket-match paint and
  background-fill paths, CI-guarded like existing Phase 14/16 budgets;
  keypress-to-local-paint budget must not regress.
- Theme catalog documentation for the two new axes (background, size ladder);
  generated registry entries, primitive reference updates, manual test plan
  screenshots (light+dark, code+prose), and wiki updates.

Expected outcome:

- Rendering foundation is production-safe, budgeted, documented, and ready
  for new formats to adopt visually with data-only contributions.

## Phase 27: Package Data Flow Consolidation

Remove the duplication identified in the 2026-08-18 review so a new file
format is one package, not four copies of the same declarations. Confirmed
decisions: manifest contributions are the sole package data path
(`decision-logs/2026-08-18-1758-single-manifest-package-loading.md`) and
manifests gain capability presets
(`decision-logs/2026-08-18-1758-package-capability-presets.md`).

### Phase 27.1: Single-Manifest Loading and Load-Entry Cleanup

Focus areas:

- Delete the `*PackageManifest()` literal duplicates (`@clay/markdown`,
  `@clay/typescript` `dist/index.js`/`load.js`); `package.json`
  `clay.contributions` is the only manifest source.
- Remove imperative registration calls from first-party load entries
  (`serverRegisterSyntaxGrammar({})`, `serverRegisterCompletionProvider({})`,
  explicit mode-pattern/command/component re-registrations); load entries
  keep only executing code (parse module imports, bridge factories).
- Remove stale ceremony: `@clay/markdown` load-time `serverActivateMajorMode`
  with hardcoded `documentId: 1`/`sample.md`, and similar legacy triggers.
- Keep the imperative APIs public for `init.js` and runtime contributions;
  document the execute-only load-entry contract in package authoring docs.

Expected outcome:

- Every first-party package declares data exactly once; load entries contain
  only executable wiring; ~80–120 lines deleted per package.

### Phase 27.2: Native Grammar Ownership Cleanup

Focus areas:

- Drop the inert `syntaxGrammars` blocks (including dead `styleMap` and
  `queries` paths) from first-party `package.json` files whose grammars are
  owned by `FIRST_PARTY_NATIVE_GRAMMARS`; the Rust descriptor is the Tier 1
  source of truth (decision: no drift between two copies).
- Simplify `op_clay_syntax_register_syntax_grammar` to a diagnostic or no-op
  for shadowed native grammars instead of silently skipping package
  contributions; surface "owned by native descriptor" in package inspection.
- Decide and document the long-term inversion (Rust statics carry only
  grammar/query functions; style maps read from trusted package records) as
  a future decision to take when third-party grammars arrive.

Expected outcome:

- One owner per grammar's style map; editing a first-party package.json can
  no longer silently do nothing for syntax.

### Phase 27.3: Capability Presets

Focus areas:

- Manifest `preset` field (`code-mode`, `prose-mode`, `lsp-bridge`) expanded
  at validation (`src/packages/manifest.rs`) into the standard permission,
  `apiDependencies`, extension-point, and contribution-family sets; explicit
  deviating declarations win; expanded set is what is validated, budgeted,
  and shown in package inspection UI.
- Migrate `@clay/rust`, `@clay/typescript`, `@clay/javascript`,
  `@clay/markdown`, and `@clay/lsp-*` manifests to presets with only
  deviating declarations; version the manifest schema change.
- Package authoring docs: preset tables, override rules, and migration notes;
  generated registry and API inventory updated.

Expected outcome:

- A new code-language package is a preset line plus its deviations; the
  copy-paste boilerplate class of divergence bugs is gone.

### Phase 27.4: Package-to-Package Dependency Resolution

Focus areas:

- Extend the bundled inventory (`src/packages/bundled.rs`) with an exports
  map so workspace-local specifiers (e.g. `lsp-shared/client.js`) resolve in
  the package module loader — first-party packages only, fingerprinted and
  trust-boundary-preserving (no third-party imports, no path escapes).
- Delete the four vendored `dist/shared/` copies of `lsp-shared` and
  `scripts/update-first-party-lsp-shared.mjs` once packages import the
  shared source.
- Guard test: first-party packages contain no vendored duplicate of another
  first-party package's modules.

Expected outcome:

- Shared first-party code lives once; new `lsp-*` packages import shared
  bridge utilities instead of copying them.

### Phase 27.5: LSP Bridge Factory Consolidation

Focus areas:

- One `createLspBridge({ server, languageId, diagnostics: "push"|"pull",
  features })` factory in `lsp-shared` absorbing the ~85% shared body of
  `@clay/lsp-rust` and `@clay/lsp-markdown` inline bridges (capabilities
  objects, `TOKEN_TYPES`/`TOKEN_MODIFIERS` tables, document tracking,
  refresh/completion/intelligence plumbing).
- `@clay/lsp-rust` and `@clay/lsp-markdown` become config + manifest
  packages like the existing `lsp-typescript`/`lsp-javascript` shells.
- New-language-server adoption becomes: one manifest `languageServers`
  contribution + one factory config object; documented in package authoring
  docs.

Expected outcome:

- Four LSP bridge packages share one implementation; adding a language
  server is data plus configuration.

### Phase 27.6: One Syntax Vocabulary — Tier 3 Migration and Compat Demotion

Focus areas:

- Migrate `@clay/markdown/dist/parser.js` from legacy free-form `markup.*`
  style tokens to the closed `TokenType`+`Modifiers` vocabulary (heading
  levels, emphasis, link parts, fences are all already modeled).
- Demote the `style_token`/`from_style_token`/`classify_style_token` compat
  path and the `scope` escape hatch to explicitly deprecated; keep them
  rendering old packages but documented as frozen.
- Vocabulary guard test: no first-party producer emits free-form style
  tokens.

Expected outcome:

- One syntax vocabulary end to end; themes key one table; compat path is
  frozen rather than first-class.

### Phase 27.7: Bundled Inventory Generation

Focus areas:

- Generate the `BUNDLED_PACKAGES` FNV fingerprint inventory in
  `src/packages/bundled.rs` at build time (`build.rs`) from a checked-in
  package list, so adding/editing a first-party package stops requiring an
  11-struct Rust edit with hand-computed hashes; test-enforced inventory
  stays the trust boundary.

Expected outcome:

- First-party package adoption touches the package tree and a list entry,
  not hand-maintained hash literals.

### Phase 27.8: Consolidation Hardening and Documentation

Focus areas:

- Behavior parity tests: activation, keymaps, commands, completion, and
  syntax identical before/after the manifest and preset migration for all
  first-party packages.
- Load-order and hot-reload regression tests for the execute-only load-entry
  contract; package inspection UI shows preset-expanded permissions.
- Docs: package authoring guide rewrite (single-manifest, presets, shared
  imports, LSP factory), generated registry freshness, wiki updates.

Expected outcome:

- Package consolidation is behavior-preserving, review-clean, and
  documented; new-format packages are materially smaller to write.

## Phase 28: Editor Command and Intelligence Primitives

Complete the generic command/edit/intelligence primitives whose data already
exists, and fix the defects found in the 2026-08-18 review in the command and
provider layers. No language-specific Rust logic: everything is driven by
behavior-manifest data and the closed decoration/completion vocabularies.

### Phase 28.1: Package Keymap Parsing Fix

Focus areas:

- `parse_keymap` (`src/server/ops/modes.rs`) currently stuffs the whole chord
  string ("Ctrl+Shift+M") into `KeyCode::Character` with no modifiers, so
  every package-declared keymap is dead; reuse
  `src/server/ops/keybindings.rs::parse_key_sequence` instead and delete the
  local divergent copy; same treatment for the duplicated routing-policy
  string parser.
- Regression tests: package keymaps (`Ctrl+Shift+M`-style chords and
  sequences) parse to real `KeyStroke`s with modifiers and match key events;
  markdown's default keymaps (`togglePreview`, `insertHeading`,
  `toggleList`) become functional.

Expected outcome:

- Package-declared keymaps work; one chord parser in the codebase.

### Phase 28.2: Generic Comment Toggle and Prose Line Transforms

Focus areas:

- Client `editor.toggleComment` primitive driven by the active behavior
  manifest's `comments` rule: per-line prefix toggle with indent awareness,
  multi-caret, selection-line handling — one implementation serving every
  code mode.
- Declarative line-transform primitives for prose: toggle list marker,
  insert/rotate heading level — same shape as
  `EnterRule::ContinueLineMarkers`, declared as manifest data.
- Wire the currently-inert registered commands (`rust.toggleLineComment`,
  `markdown.toggleComment`/`toggleList`/`insertHeading`) to these
  primitives; policy: packages may not register commands they cannot back.

Expected outcome:

- Comment toggle works in every code mode; markdown list/heading commands
  execute for real; no metadata-only commands in the palette.

### Phase 28.3: Folding Ranges

Focus areas:

- Implement the stubbed `folding.serverPublishFoldingRanges` API surface
  (currently documented as planned/unavailable in
  `docs/reference/clay-js-api/api-inventory.toml`): protocol messages,
  budget-validated range sets, provenance.
- Server-side fold computation from tree-sitter indents/multi-line nodes
  (nearly free on the existing parsed trees); client gutter fold UI composes
  with Phase 26.5 chrome.

Expected outcome:

- Code modes get working folding with provider provenance; the API stub
  becomes real.

### Phase 28.4: Link Decorations and Hover Intent

Focus areas:

- Add `DecorationKind::Link` carrying target provenance; paint styling
  (underline/color) from theme tokens.
- Hover/click intent protocol for decorated ranges: markdown links and
  footnotes open targets; groundwork for LSP go-to-definition affordances
  using the same generic intent (no language branches in core).

Expected outcome:

- Links in prose are visually distinct and activatable; one generic hover
  intent primitive serves markdown and future LSP features.

### Phase 28.5: Inlay Hints

Focus areas:

- New decoration kind for inlay hints (type annotations, parameter names)
  with the existing vocabulary-alignment to LSP; bounded payloads, gutter-
  adjacent paint, toggle command and per-mode default.
- LSP bridge mapping (`lsp-rust` first) through the factory from Phase 27.5.

Expected outcome:

- rust-analyzer inlay hints render as decorations through the same pipeline
  as syntax/diagnostics.

### Phase 28.6: Completion Ranking and Provider Polish

Focus areas:

- Replace the alphabetical-prefix-only buffer-word ranking with a scoring
  function (exact-prefix, case-match, length, recency-of-use) inside the
  existing budgeted candidate scan; shared by all providers for tie-breaks.
- Ranking tests over representative candidate sets; no ranking work on the
  keypress-to-local-paint path.

Expected outcome:

- Buffer-word completions rank sensibly; the scoring function is the single
  shared tie-breaker as more providers land.

### Phase 28.7: Hardening and Documentation

Focus areas:

- Behavior parity and protocol compatibility tests for every new command,
  intent, and decoration kind; budgets CI-guarded; authority review for the
  new intents (link activation, hover) under existing package permissions.
- `src/server/js_runtime/mod.rs` test mass (~9.7k inline test lines) moves to
  a sibling integration module to keep the runtime file reviewable — move
  only, no behavior change.
- Generated registry entries, API inventory, primitive reference docs, and
  wiki updates for all new primitives; manual test plan coverage.

Expected outcome:

- The command/intelligence primitive set is complete, budgeted, and
documented; new formats consume it declaratively.

Sequencing note: Phase 26 (rendering) first — new formats cannot be judged
visually until paint is fixed; then 27 (data flow) before any new package is
authored; 28 can proceed in parallel with 27. Phase 25 (AI-native chat host)
does not wait on 26–28; it is a different surface. Phase 29 (coding agent)
requires Phase 25 plus AI-Safe Mutation. Tier 2 WASM execution stays
scheduled with Phase 23 ecosystem work as planned; the adoption target once
built is server-side capture→vocabulary mapping so third-party grammars are
data-only packages.

## Phase 29: Coding Agent, CLI-Parity, Clay UI (no ACP)

Ship first-party `@clay/coding-agent` on the Phase 25 host. Same daemon,
credentials, provider/model UI, composer/transcript primitives, and `agent.*`
APIs. No ACP, no AG-UI, no second process. The package registers the Coding
Agent profile, tool UX, diffs, and approvals using Clay UI primitives — the
same pattern as `@clay/chat`. Third-party packages may `extends` declared
extension points (tools, skills, approvals, MCP allow-list, prompt) or
`replaces` the whole package with user approval; replacement stays in the
third-party runtime. Clay is the UI a CLI agent would have used a TTY for.

Bar: everything `@arnilo/prism-coding-agent` gives a CLI host, plus the
editor-aware pieces a CLI cannot see. Do not ship a chat-with-tools toy.

Dirty buffers do **not** go through ACP `fs/read_text_file`. Prism tools
already take pluggable `ReadOperations` / `WriteOperations` / `EditOperations`.
Clay implements those seams over the server document registry (dirty snapshot
first, disk fallback). `createAcpFilesystemOperations` is unused.

### CLI-parity contract

| CLI agent capability | Clay surface |
| --- | --- |
| Multi-turn prompt, stream, cancel, steer | Phase 25 composer primitive + protocol |
| Session persist / resume / list / delete | Phase 25 SQLite store |
| Thinking + usage | Transcript rows from `AgentEvent` |
| Compaction when context fills | `@arnilo/prism-compaction` / observational memory |
| Nine tools: `shell` `read` `write` `edit` `repo_list` `repo_search` `glob` `delete` `move` | Same factories; `read`/`write`/`edit` use Clay document operations |
| Opt-in Git tools | `createGitTools` registered on the coding profile |
| `ask_user_decision` | Existing Clay confirmation / Command Centre session |
| One-shot `shell` + output | Tool card in transcript; cwd = tab workspace |
| Long-running processes | `createProcessSessions` streamed into tool cards; `pty: true` fail-closed until the terminal package |
| Execution policy / approvals | `createCodingApprovalPolicy`; UI for allow-once / allow-for-run / reject-once / reject-for-run |
| Linux sandbox | `@arnilo/prism-coding-security` native adapter; Windows `shell` deny-by-default |
| Plan / todo markdown | `writeCodingPlanFile` / transcript plan block |
| MCP | `@arnilo/prism-mcp` behind an explicit allow-list; off until configured |
| Images in `read` | Supported; binary never silently falls back to a second path |
| AGENT.md / skills | Prism host skills on the coding profile when wired |
| Open-file / selection context | Injected by Clay server (CLI has no editor) |
| Unsaved buffers | Clay document operations (CLI only sees disk) |
| Apply edits with undo/lease/version | AI-Safe Mutation (below), not raw disk write of open docs |
| Diff review | Editor diff surface + jump-to-line; not TTY patches only |
| Interactive PTY | Deferred; terminal emulator package |

### Phase 29.1: Clay document operations and AI-safe mutation

- Implement Prism `ReadOperations` / `WriteOperations` / `EditOperations` in
  `clay-agent` as reverse-RPC to the Clay server. Prefer the open document
  snapshot (including dirty). Disk only if the path is not open.
- `delete` / `move` / `glob` / `repo_*` stay disk-backed. After those
  mutate a path, server reloads or invalidates any open document on that path.
- Writes and edits of open documents go through AI-Safe Mutation: explicit
  document version, range, permission scope, preview/apply/reject, conflict
  explanation. Agent edits never bypass package-edit authority.
- No ACP filesystem client. No `createAcpFilesystemOperations`.

### Phase 29.2: Coding profile, tools, sandbox, approvals

- First-party `@clay/coding-agent` (`loadPackage("@clay/coding-agent")`)
  registers the Coding Agent profile and its extension points. Load
  `@arnilo/prism-coding-agent` and `@arnilo/prism-coding-security` in the
  existing daemon when that package is enabled. Picker row appears because
  the package registered, not because core un-stubs a reserved name.
- Register `createCodingTools` + `createGitTools` + `createAskUserDecisionTool`
  + `createProcessSessions` (no PTY backend).
- Workspace required for Coding Agent. Chat still works with none. Open
  Folder on the agent view is the grant.
- Approvals use the Phase 25 permission event variants. Mutating tools wait.
- Linux sandbox adapter; truthful “not sandboxed” language on Windows.

### Phase 29.3: Tool UX, diffs, loop, MCP

- Tool-call tree in the transcript (start/progress/finish/error). Shell and
  process output are bounded cards, not a hidden log.
- Diff review, jump-to-line, activity markers on open documents.
- Agent loop UX: plan / execute / review / test / document / user to-do.
  Observational-memory compaction. Caveman/Ponytail as opt-in Prism behavior
  packages. Auto vs manual loop. Context inspection.
- MCP allow-list configuration. Slash commands only if Prism host seams are
  already wired — do not invent a second command language.

Expected outcome:

- User loads `@clay/coding-agent`, picks Coding Agent, grants a folder, and
  gets a CLI-class coding agent inside Clay: tools, sandbox, approvals,
  dirty-buffer awareness, diffs, sessions. No ACP on either side of the
  process boundary. A third-party package can extend or replace this without
  forking Clay core.

## ACP / AG-UI interop (later, not first-party)

Out of the first-party coding-agent path. Revisit ACP only if Clay must host
third-party ACP agents or expose this agent to other ACP editors. Revisit
AG-UI only if a web front-end must share the daemon. Do not add either crate
to the Rust client or to `clay-agent` for Chat or Coding Agent.

## AI-Safe Mutation and Region Locks

Support AI-generated edits without corrupting user state.

Focus areas:

- Make region locks first-class.
- Require AI edit sessions to carry explicit document versions, behavior versions, mode/package primitive versions, ranges, and permission scopes.
- Add preview/apply/reject flows.
- Add conflict explanations.
- Consider transaction logs and richer correction transactions.
- Separate extension/agent permissions from direct user input.
- Lock only the needed scope: range, document, behavior, mode, rendering primitive, or workspace.

Expected outcome:

- AI agents can propose or apply changes safely.
- User edits and agent edits have explicit conflict boundaries.
- AI-visible tools and mutation capabilities are documented and inspectable.

## Markdown mode preview implementation with capabilities required for personal and work agent

## Handling config, key binding, theme, font from UI with config file override

## Agentic AI with Prism (later, on the Phase 25 host)

After `@clay/chat` (Phase 25) and `@clay/coding-agent` (Phase 29), the same
`clay-agent` loads optional Prism capability packages — no new runtime:

- Web agent: `@arnilo/prism-web-tools` (Brave / Exa / Firecrawl) behind
  explicit user-configured credentials and allow-lists
- Agentic web action / browser: `@arnilo/prism-browser` with host-owned
  Playwright lifecycle
- Web bridge only if a remote UI must share the daemon (AG-UI server
  package), not as the native Clay path

## JSON

## YAML

## TOML

## Terminal Emulator package

## Python

## Jupyter and IPYNB

## Latex

## PDF mode with links to md files

## Personal Assistant Agent (later, Phase 25 host)

First-party package registering a Prism `AgentDefinition` + picker row.
Extends markdown mode for personal knowledge management, to-do lists,
schedule, daily-task automation. No new daemon. Third-party extend/replace
via declared extension points + user approval.

## Work Agent (later, Phase 25 host)

First-party package. Work management; `@arnilo/prism-work-tools`
(M365 / GWS) only with explicit OAuth connectors. No new daemon.

## Research Agent (later, Phase 25 host)

First-party package. Reference management, citations from source, web-tools
when that phase has landed. No new daemon.

## Finance Agent (later, Phase 25 host)

First-party package when a dedicated phase lands; not a core reserved stub.

## Clay Agent (later)

Meta-agent: wiki updates, extension-writing methodology, on-device knowledge
for AI agents. Same host, same credential store.

## UI for managing agents

Shipped in Phase 25.4 (Command Centre agent/provider/model/setup). Later
agent packages add picker rows and setup descriptors by registering
profiles; they do not invent a new settings surface.

## Phase 21: Remote, Container, and Multi-Client Hardening

Make the server/client split useful beyond local IPC.

Focus areas:

- Remote server connection over secure transport.
- Container/toolbox/distrobox server startup and discovery.
- Live workspace-root discovery for UI/help surfaces, including a dedicated root-list protocol/server method if this is still needed before or beyond general Clay JS runtime wiring.
- SSL/TLS or SSH/tunnel strategy.
- Multiple clients connected to one server.
- Multiple documents open concurrently at scale.
- Read-only observer behavior for duplicate opens.
- Server concurrency and per-document actor scaling.
- CI coverage for `cargo fmt --check`, native `cargo test --all-targets`, Windows MSVC checks, generated registry freshness, package docs, and wiki navigation.
- Add `cargo bench --no-run` to CI to verify all Criterion benchmark targets compile on every push without running machine-variant timing loops.
- Promote Phase 14 advisory latency budget constants (`KEYPRESS_TO_LOCAL_PAINT_P95_BUDGET_MS`, `EDIT_ACK_P95_BUDGET_MS`, `SCROLL_LAYOUT_RENDER_ADJACENT_P95_BUDGET_MS`, `RUNTIME_CONFIGURATION_EVAL_P95_BUDGET_MS`) and Phase 16 primitive/package budget constants (`DECORATION_PAYLOAD_BUDGET_BYTES`, `INCREMENTAL_PARSE_UPDATE_BUDGET_BYTES`, `MODE_ACTIVATION_P95_BUDGET_MS`, completion/folding payload budgets, and package/mode validation payload ceilings) to hard CI thresholds only after verifying stability across at least one consistent CI runner and representative Phase 17/18 fixtures; document the promoted values and remove the advisory-only qualifier from `docs/development/performance.md` and primitive reference docs.
- If developer-only profiling hooks have been promoted to a stable user-facing feature by this phase, verify the `clay:diagnostics` Clay JS API exists with Markdown docs, inventory entry, generated registry entry, and lookup coverage; otherwise confirm the `no_public_configuration_needed_for_internal_perf_hooks` guard test remains active.

Expected outcome:

- A host client can connect to a server running in a target development environment.
- Clay can support local, container, and remote editing without changing the client authority model.



## Phase 23: Ecosystem and Repository Hardening

Prepare Clay packages and primitive APIs for a broader ecosystem after first-party package/mode proof points exist.

Focus areas:

- Package repository policy, package publishing workflow, trust, signatures or integrity checks beyond delegated package-manager integrity, offline/local packages, registry metadata, upgrades, removal, compatibility policy, package-manager environment diagnostics, and persistent shared package enable/disable state across CLI, in-app UI, and server runtime processes.
- Documentation coverage gates for Clay JS APIs, packages, generated registries, code wiki navigation, package-provided user-facing features, primitive contributions, and mode behavior.
- User/developer package UI for install, enable, disable, upgrade, remove, inspect permissions, inspect primitive contributions, and diagnose conflicts.
- Additional first-party package/mode examples beyond Markdown, using the primitive registry to expose missing capabilities iteratively.

Expected outcome:

- Clay has a sustainable package ecosystem path after proving package-controlled editing/rendering locally.
- The primitive registry grows through real modes while remaining inspectable and performance-safe.
