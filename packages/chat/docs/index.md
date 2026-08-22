# @clay/chat

First-party Chat landing (Phase 25).

A catalog-composed empty-tab surface: greeting, agent/provider/model buttons,
Open File, Open Folder, and a focused composer. Chat works with no workspace.
The package registers the Chat profile (no tools, no sandbox). Chat is
prompt/response only — it does not isolate filesystem, shell, or network.
Coding Agent is not a disabled stub here; it appears when
`@clay/coding-agent` loads in Phase 29.

The surface is declared in `clay.contributions.ui.paneContents` and registered
by `dist/load.js` via `serverRegisterPaneContentContribution`. It uses only
implemented `ComponentKind` kinds — no native chrome, no client JavaScript, no
raw CSS.

## Catalog composition

| Control | Catalog kind |
|---------|-------------|
| Container | `panel` (empty-tab `main`) |
| Greeting | `label` (`typography.display`) |
| Unconfigured provider | `label` (`typography.caption`) |
| Transcript | `scroll` + `list` (`chat.transcript`) |
| Status | `label` (`chat.status`) |
| Agent / Provider / Model | `button` → `agent.clientOpen*Picker` |
| Open File / Open Folder | `button` → existing client dialog commands |
| Cancel | `button` → `chat.cancel` (Escape while streaming) |
| Composer | `textInput` (`chat.submit`; empty send is a no-op; Enter sends) |

## Command intents

- `chat.profile` — Chat profile registration (no tools).
- Agent / Provider / Model buttons call `agent.clientOpen*Picker` — same
  Command Centre session kinds as the catalogue commands.
- `chat.submit` — send composer text. Empty/whitespace is a no-op. Enter sends.
- `chat.cancel` — stop the in-flight run. Escape while streaming also cancels.
- Open File / Open Folder use `documents.clientOpenFileDialog` and
  `workspace.clientOpenFolderDialog`.
- New tab starts a new session. Session picker resume reloads bounded history.

## Extension points

- `chat.entrySurface` — replace or append the empty-tab landing.
- `chat.chromeActions` — append or replace chrome commands.

Whole-package `replaces` remains available. A third-party replacement stays
in the third-party runtime.

## Activation

Default path is one line in `examples/packages/first-party.js` (loaded from
`init.js` via `loadConfigurationModule`). No silent compiled Chat.

```js
import { loadPackage } from "clay:packages";
await loadPackage("@clay/chat");
```

That line applies `package.json` contributions and runs `dist/load.js`.
No copied manifests, no raw ops, no manual primitive registration.
Without it, empty tabs stay the core Open File / Open Folder fallback and
no Chat profile is registered.

Load grants no filesystem, network, shell, daemon, or AI-mutation authority.
Unconfigured providers show the instructional caption until a provider is set.
