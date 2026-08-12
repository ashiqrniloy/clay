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

## ACP/AG-UI

## Coding agent
- Basics like pi

### Loop
- Roadmap
- Spec/Requirements -> Acceptance Criteria
- Plan 
- Execute
- Review (ponytail)
- Test
- Document
- User to-do

General details:
- After every turn plan/task, OM compaction
- If user decision required, choose the simplest and document in user to-do
- Loop can be started at any stage
- Can be run manually. Differentiate between auto and manual
- Each step follows acceptance criteria separately as context injection
- Context inspection
- Caveman
- Ponytail
- Providers: Alibaba cloud, Ollama-cloud, Opencode-go, Kimi, OpenAI Oauth, Cursor SDK, Gemini SDK

## User Package and Config segregation with defined ~/.config/clay structure

## Command Centre

## File browser with dynamic root selection

## Coding agent



### Evaluation of Prism capability against requirements

#### Requirements list

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

## Agentic AI with Prism
- Prism upgrade with Web agent for search with Exa, Firecrawl, Brave search
- Agentic web action
- Web bridge

## JSON

## YAML

## TOML

## Terminal Emulator package

## Python

## Jupyter and IPYNB

## Latex

## PDF mode with links to md files

## Personal Assistant Agent
- Extends markdown mode for personal knowledge management
- To do lists
- Schedule management
- Automation for daily tasks

## Work Agent
- Extends markdown mode for work management
- Office CLI with GUI

## Research Agent
- Reference management
- Show reference from source

## Finance Agent

## Clay agent
- Update wiki for AI agents and access in user device
- Extension writing methodology and knowledge system for AI agents

## UI update for managing agents

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
