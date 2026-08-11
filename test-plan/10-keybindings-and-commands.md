# 10 — Keybindings and Commands

`bindKey`/`unbindKey` overrides, deny-by-default validation, command routing,
the `editor-control` execution push channel (`clientExecuteEditorCommand`,
protocol v8), and the Global-scope tab command bindings (Phase 22.4). Deep
reference: `docs/development/manual-editor-capabilities-test-plan.md`
(sections G/H) + `docs/reference/clay-js-api/shell/client-tab-*.md`.

## Setup

init.js:

```js
import { bindKey, unbindKey } from "clay:keybindings";
bindKey("Ctrl+B", "editor.clientMoveCursor.prevWordStart", { scope: "editor" });
```

## Override and validation

| # | Action | Expected |
|---|--------|----------|
| K1 | `Ctrl+B` in any file | Moves one word back (init.js binding beats nothing; direction-specific IDs are bindable) |
| K2 | Add a second `bindKey("Ctrl+B", "editor.clientMoveCursor.nextWordStart", …)`, reload | Last binding wins — now moves forward |
| K3 | `unbindKey("Ctrl+B", { scope: "editor" })`, reload | Default/no binding restored |
| K4 | `bindKey("Ctrl+G", "application.quit", …)` | Rejected — non-editor/undeclared command IDs deny-by-default; diagnostic names it |
| K5 | `bindKey("Ctrl+Q Ctrl+W", …)` (multi-stroke) | Rejected — single strokes only (known ceiling) |
| K6 | Bind a textobject ID (`editor.clientSelectTextobject.class.around.current`) | Accepted — auto-declared on first bind; works in grammar files |

## Default bindings sanity (must exist without any init.js)

| # | Key | Expected |
|---|-----|----------|
| K7 | Arrows, `Home`/`End`, `Ctrl+Home`/`Ctrl+End` | Basic movement (module 05) |
| K8 | `Ctrl+Left`/`Ctrl+Right`, `Ctrl+Up`/`Ctrl+Down` | Word / paragraph movement |
| K9 | `Ctrl+D`, `Ctrl+Shift+L`, `Ctrl+Alt+Up`/`Down`, `Shift+Alt+arrows`, `Ctrl+U` | Multi-cursor family (module 06) |
| K10 | `Ctrl+Z`/`Ctrl+Shift+Z`, `Ctrl+L` | History, select-line |

## Execution push channel (`clientExecuteEditorCommand`)

init.js:

```js
import { clientExecuteEditorCommand } from "clay:editor";
clientExecuteEditorCommand({ commandId: "editor.clientSetSelection.selectLine" });
```

| # | Action | Expected |
|---|--------|----------|
| K11 | Cold start with the call above | NOT delivered — no client subscribed yet (expected; advisory) |
| K12 | Open a file, trigger runtime reload via settings appearance switch while connected | init.js reruns; the line under the caret becomes selected — proves op → gate → broadcast → connection → widget dispatch |
| K13 | Change `commandId` to `"application.quit"`, reload | Op rejects ("not a known editor command"); nothing published |
| K14 | Third-party package without `editor-control` permission calls the op | Denied (covered by automated tests; not reachable from init.js by design) |

## Tab command bindings (Phase 22.4)

Tab chords ship as `Global`-scope defaults (module 14, T25–T40); this
section covers the configuration side. Policies: numbering follows the card
order; next/prev wrap; moves never wrap; numbered families are 1-based and
capped at 9 (IDs beyond 9 do not exist). Deep reference:
`docs/reference/clay-js-api/shell/client-tab-*.md` + `examples/init.js`
section 7 (tab annotation block).

init.js:

```js
import { bindKey, unbindKey } from "clay:keybindings";
bindKey("Ctrl+Alt+T", "shell.clientTabNew", { scope: "global" });
```

| # | Action | Expected |
|---|--------|----------|
| K15 | With the init.js above, reload; press `Ctrl+Alt+T` with 2 tabs open | New-tab flow starts (same as `Ctrl+T` / `+`); the shipped default `Ctrl+T` still works — user bindings ADD to defaults |
| K16 | Override a default chord: `bindKey("Ctrl+Tab", "shell.clientTabPrev", { scope: "global" })`, reload, press `Ctrl+Tab` | The override wins — `Ctrl+Tab` now goes to the PREVIOUS tab (user binding beats the shipped default on the same chord); then `unbindKey("Ctrl+Tab", { scope: "global" })`, reload → the default next-tab behavior returns |
| K17 | `bindKey("Ctrl+Alt+9", "shell.clientTabActivate.10", { scope: "global" })` | REJECTED deny-by-default — numbered variants exist only for 1..=9; the diagnostic names the ID |
| K18 | `bindKey("Alt+1", "shell.clientTabActivate.1", { scope: "global" })`; reload; press `Alt+1` with 2 tabs open | Accepted — numbered family IDs bind like any other command ID and activate the first tab; `Alt+2` (unbound) does nothing |

Tab command policy table (module 14 steps in parentheses):

| Command family | Default chord(s) | Policy |
|---|---|---|
| `clientTabNext` / `clientTabPrev` | `Ctrl+Tab` / `Ctrl+Shift+Tab` | wrap around (T25–T26); fewer than 2 tabs = no-op (T28) |
| `clientTabNew` | `Ctrl+T` | same flow as `+`; ignored while the picker is open (T29) |
| `clientTabClose` | `Ctrl+Shift+W` | last tab protected (T31); dirty tabs get the save-all/discard/cancel confirm menu (T32–T35) |
| `clientTabActivate.<N>` | `Ctrl+<N>` | 1-based card order; N in 1..=9; beyond count = no-op (T27, T39) |
| `clientTabMoveLeft` / `clientTabMoveRight` | `Ctrl+Shift+[` / `]` | boundary = no-op; never wraps (T36–T37) |
| `clientTabMoveTo.<N>` | `Ctrl+Shift+<N>` | 1-based; N in 1..=9; beyond count = no-op (T38) |

## Negative checks

- Key routing never runs package JavaScript in the keypress path.
- Unknown command IDs at runtime map to a no-op result, never a crash.

## Known ceilings

- Multi-stroke chords unsupported by `bindKey`.
- Textobject/smart-select IDs ship with NO defaults; binding is the package
  or user's job.
