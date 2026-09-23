# @clay/launcher

First-party start surface: the content of a fresh window and of every new
empty tab (plan 118 Part D, `DESIGN.md` §12).

## What it contributes

One `empty-tab` pane content, `launcher.start`, declared in
`clay.contributions.ui.paneContents` and registered at load time through
`ui.serverRegisterPaneContentContribution`. The host renders it for its trusted
provenance as the compiled launcher panel; the declared component tree is inert
fallback data for the generic SDUI renderer and for the contribution inventory.
With no `empty-tab` contribution installed, the core fallback stays the Open
File / Open Folder empty state — no product-named landing lives in core.

## Where its data comes from

The host, not the package:

- **Recent workspaces** are recorded server-side (`launcher.json` under the
  Clay data root, newest first, capped at eight) whenever a folder is opened —
  the folder dialog, a tab opened on a folder, or an in-tab folder open.
- **Agent types** are the directories under `<data root>/agents/`: one entry per
  agent that actually resolves, with its skill count.
- Both arrive through the validated session path (`ListLauncherEntries` →
  `LauncherEntries`) and are refreshed after a removal. The panel never
  fabricates an entry: a workspace whose folder disappeared is pruned on read
  (reported as one bounded `launcher.recents_pruned` diagnostic), and an agent
  without a config folder is not listed.

## Authority

None beyond the folder dialog the surface can open. Entries are display data
(names and paths); removing one names an index in the server's own list, never a
path; opening a folder rides the ordinary dialog capability and tab path. The
recents file holds plain paths and no credentials.
