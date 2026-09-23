# Launcher Landing Surface (Plan 118 Part D)

**Files:** `frontend/src/launcher/LauncherPanel.tsx`, `frontend/src/launcher/launcher.module.css`, `packages/launcher/package.json`, `packages/launcher/dist/load.js`, `packages/launcher/docs/index.md`, `src/server/launcher.rs`, `src/server/ui.rs` (`empty_tab`, `wire_snapshot`), `src/server/connection/mod.rs` (launcher messages), `src/server/connection/tabs.rs` / `workspace.rs` / `src/server/mod.rs` (recents writes), `src/protocol/mod.rs` (`LauncherEntries`, `ListLauncherEntries`, `RemoveLauncherRecent`), `frontend/src/editor/sync/messages.ts` / `session.ts`, `frontend/src/shell/PaneTree.tsx`, `frontend/src/shell/WorkspacePanes.tsx`, `frontend/src/shell/workspace-controller.ts`, `examples/config/packages/first-party.js`
**Tests:** `src/server/launcher.rs` (unit tests), `frontend/src/launcher/LauncherPanel.test.tsx`, `frontend/src/shell/WorkspacePanes.test.tsx`, `frontend/src/shell/workspace-controller.test.ts`, `tests/manual_smoke_docs.rs` (harness contract), `tests/package_ui_conformance.rs` (source independence)
**Reference Docs:** `DESIGN.md` §12/§16, `design-artifacts/approved/quiet-instrument-migration/start.html`, `design-artifacts/approved/quiet-instrument-migration/README.md`, `docs/reference/packages/creating-packages.md`

---

## 1. What it is

The launcher is the content of a fresh window and of every new empty tab. It
replaces the `@clay/chat` landing surface that was removed in plan 118, and it
carries no product identity of its own beyond the panel: a tab is one workspace
plus one agent, and the landing asks which side to start with.

Three layers cooperate, and none of them is a special case in the shell:

1. **Election (server).** A package contributes pane content with
   `activation: "empty-tab"`. Exactly one may do so; the winner becomes
   `PackageUiSnapshot.empty_tab`.
2. **Rendering (client).** `PaneTree` renders the winner as the host's compiled
   panel **only** for exact trusted provenance (`hostRenderedSurface`); any
   other package's empty-tab contribution renders through the generic SDUI view.
3. **Data (server).** Rows are server-resolved display data — recent workspace
   roots and configured agent directories — requested by the panel with typed
   feature messages.

With no contribution loaded, the core fallback (`Open file` / `Open folder`)
renders instead. No product-named landing lives in core.

## 2. Election and trust

`src/server/ui.rs`:

- `RegisteredPaneContentContribution` entries are grouped by `activation`
  (`"pane"` vs `"empty-tab"`).
- The empty-tab candidates are sorted by contribution id. Zero candidates →
  `None`; exactly one → the winner; two or more → `Err(sorted ids)`, so a
  conflict is a startup diagnostic rather than a silent winner.
- `wire_snapshot` puts the winner in `PackageUiSnapshot.empty_tab` with the
  provenance the caller's trust-domain resolver reports, and the `activation:
  "pane"` entries in `PackageUiSnapshot.surfaces` (the empty-tab election
  ignores pane surfaces).

`frontend/src/shell/PaneTree.tsx`:

```ts
const HOST_RENDERED_SURFACES: Record<string, string> = {
  "@clay/coding-agent": "coding-agent",
  "@clay/launcher": "launcher",
};
```

`hostRenderedSurface(surface)` returns a host panel name only when
`surface.provenance.trustDomain === "trusted"` **and** the package name matches
exactly. A same-named third-party package, or the same package loaded from a
non-bundled source, renders through `PackageSurfaceView` (SDUI) — the compiled
panel is never selected by name alone. Both host panels load through
`React.lazy`, so the launcher's code is not in the shell's critical path.

`packages/launcher` is a bundled first-party package (`packages/launcher/`) that
declares one `paneContents` contribution (`id: launcher.start`, `activation:
empty-tab`) and an `launcher.surface` extension point (version 1, `replace`) so
another package can replace the start surface without patching Clay. Its own
`component` tree is a minimal `panel` + caption + `Open folder…` button: that is
what a host **without** the compiled panel would paint, and it keeps the
contribution valid on its own.

## 3. Server data: recents and configured agents

`src/server/launcher.rs` owns both lists. It is display data only.

**Recent workspaces** — `launcher.json` in the Clay data root: the explicit
configuration root when one is set, else `~/.clay` (`data_root()`; `None` ⇒ no
launcher data, never an error). `STORE_VERSION = 1`:

```json
{ "version": 1, "workspaces": ["/abs/path", "..."] }
```

- Newest first, deduplicated, capped at `MAX_RECENTS = 8`.
- Written on every explicit open: `connection/tabs.rs` (opening a folder in a
  tab), `connection/workspace.rs` (the folder dialog's grant path), and
  `server/mod.rs` (bootstrap root). Best-effort: a failed write never fails the
  open.
- Read prunes entries whose directory no longer exists and reports how many it
  dropped (`pruned`), so the row list and the count are always truthful.
- A single stored path is capped at `MAX_PATH_CHARS = 4096`; malformed JSON
  degrades to a first-run store rather than an error.

**Configured agents** — a bounded scan of `<data root>/agents/`:
`MAX_AGENTS = 32` entries, each with a validated directory name, a display
label, its config root and a skill count from `skills/` (bounded by
`MAX_SKILL_DIRS = 64` subdirectory reads). An agent row exists **exactly when**
the directory exists — the launcher fabricates no agent types and no counts.

Paths are rendered with `home_dir()` substitution (`~/…`) for display; the wire
type carries both the display label and the absolute root.

## 4. Wire path

| Direction | Message | Notes |
|---|---|---|
| client → server | `ListLauncherEntries { client_id }` | Panel's initial request |
| client → server | `RemoveLauncherRecent { client_id, index }` | Index into the server's own list |
| server → client | `LauncherEntries { client_id, entries, pruned }` | Also the reply after a removal |

The frontend sends these as ordinary feature-request families
(`listLauncherEntries`, `removeLauncherRecent` in `editor/sync/messages.ts`) and
subscribes to the `launcherEntries` feature envelope
(`LauncherPanel.launcherEntriesFrom` validates the shape before use; unknown or
partial payloads are ignored). The panel asks once on mount and accepts every
later answer, so a removal needs no separate refresh path.

## 5. Panel behaviour

`LauncherPanel` renders two panes — `Workspaces` and `Agents` — as
`role="listbox"` lists of `role="option"` rows, each with a filter input, a
count badge, a body and a foot (`Open folder…` for workspaces; the
`~/.clay/agents/` first-run note for agents). Cursor, filter and pick are
per-pane client state.

| Key | Effect |
|---|---|
| `Tab` / `Shift+Tab` | Walk the panes' focusables in DOM order (native tab order; rows are `tabIndex` 0 only at the cursor) |
| `↑` / `↓` | Move the pane cursor; wraps; no-op on an empty pane |
| `Enter` | Toggle the pick for the cursor row |
| `⌘Enter` / `Ctrl+Enter` | Launch immediately (same as the primary button) |
| `Escape` | Clear both picks |
| `Backspace` | On the workspaces pane: remove the cursor row from recents (server-side, by index) |
| plain typing in a filter | Filters by name/path, case-insensitive substring |

The footer summarises the picks (`name + agent`, or `Nothing picked yet`) and
the primary button label tracks them (`Open <workspace>`, `Open <agent>`,
`Open <workspace> + <agent>`, or a disabled `Open`). At most one workspace and
one agent can be picked per launch; picking both and launching does both in that
order. Rows carry `data-cursor`/`data-selected` and design-system recipe
attributes (`panel`, `list`, `badge`, `textInput`) so the surface is themed by
the shipped design system like any other component.

## 6. Launch handoff

The panel owns no navigation. It calls the shell:

- workspace pick → `workspace.openTab(root)` (the ordinary tab-with-root path;
  the server records the recent);
- agent pick → `workspace.launchCodingAgent()`, which dispatches the
  client-routed `coding-agent.profile` command — the same path the Command
  Centre uses — opening the agent surface in the active pane.

Nothing in the panel writes a path into a request: opening a row rides the
existing root-grant path, and removing a recent names an index, which the server
resolves against its own list. Consequences: the webview cannot ask the server
to open an arbitrary path, a stale index cannot delete an unrelated entry (the
server re-reads and re-indexes on each write), and recents hold no credentials
and are never auto-opened.

## 7. Invariants and tradeoffs

- **No fabricated data.** Both lists come from the server; a row is a file the
  server can see or an agent directory that exists. Empty panes show first-run
  notes, not placeholder rows.
- **One landing, elected not hardcoded.** The shell never names a landing
  package in core; the launcher takes the slot because it is the bundled
  contribution, and any package can replace it through the extension point (or
  take the slot in a config that loads a different landing).
- **Trusted-only compiled panel.** Provenance is checked on every render, so
  third-party code cannot reach the host panel by reusing the package name.
- **Removal is destructive-by-index, not by path.** `Backspace` deletes one
  server-side entry; the UI reflects the reply, not a local edit.
- **Ceilings.** The agent list is a single bounded directory scan (no agent-type
  registry yet — the per-tab agent picker is the remaining Part D work), and
  recents are plain absolute paths (no per-root metadata, no cross-machine
  identity).

## 8. Testing

- `src/server/launcher.rs` unit tests: newest-first round trip and dedupe,
  cap + prune of missing directories, single-entry removal by index, malformed
  store degrading to first run.
- `frontend/src/launcher/LauncherPanel.test.tsx`: envelope parsing, filtering,
  keyboard model, pick/label/summary behaviour, removal request, empty-pane
  notes.
- `frontend/src/shell/WorkspacePanes.test.tsx`: the trusted launcher
  contribution renders the host panel (and the non-trusted case does not).
- `frontend/src/shell/workspace-controller.test.ts`: launch paths
  (`openTab(root)`, `launchCodingAgent()`).
- `tests/package_loading.rs::launcher_bundled_manifest_claims_the_empty_tab`:
  the manifest assembles, claims zero permissions, depends only on
  `ui.serverRegisterPaneContentContribution`, and declares the single empty-tab
  pane content.
- `tests/package_ui_conformance.rs`: source-independence guard (no
  package-name branching in host code beyond the declared maps) and the
  plan-118 absence rules.
- Live evidence: `scripts/capture-ui-review.sh --fixture ui-review-launcher
  --example-config` (the canonical `examples/config/` tree) — captures, the
  AT-SPI tree, and `runtime-tree.txt` recording `landing=PASS`; artifacts under
  `test-plan/artifacts/118-quiet-instrument-migration/launch-test/`.

## 9. Related

- [React SDUI and Package UI Projection](react-sdui-package-ui.md) — the
  contribution registry and snapshot flow the election sits in
- [Tabs and Independent Client Views](tabs-and-clients.md) — what a tab is, and
  the remaining Part D tab work
- [clay-agent Daemon](clay-agent.md) — the agent side the `Agents` pane lists
- [Repeatable UI Review Harness](ui-review-harness.md) — the `ui-review-launcher`
  fixture and the canonical-example leg
- [Design Artifact Gate](design-artifact-gate.md) — `start.html` and the
  approval record behind this surface
- [UI Design System Runtime](ui-design-system-runtime.md) — the recipes the panel
  consumes
