# 10 — Keybindings and Commands

`bindKey`/`unbindKey` overrides, deny-by-default validation, command routing,
and the `editor-control` execution push channel (`clientExecuteEditorCommand`,
protocol v8). Deep reference:
`docs/development/manual-editor-capabilities-test-plan.md` (sections G/H).

## Setup

init.js:

```js
import { bindKey, unbindKey } from "clay:keybindings";
bindKey("Ctrl+B", "clay.editor.clientMoveCursor.prevWordStart", { scope: "editor" });
```

## Override and validation

| # | Action | Expected |
|---|--------|----------|
| K1 | `Ctrl+B` in any file | Moves one word back (init.js binding beats nothing; direction-specific IDs are bindable) |
| K2 | Add a second `bindKey("Ctrl+B", "clay.editor.clientMoveCursor.nextWordStart", …)`, reload | Last binding wins — now moves forward |
| K3 | `unbindKey("Ctrl+B", { scope: "editor" })`, reload | Default/no binding restored |
| K4 | `bindKey("Ctrl+G", "clay.application.quit", …)` | Rejected — non-editor/undeclared command IDs deny-by-default; diagnostic names it |
| K5 | `bindKey("Ctrl+Q Ctrl+W", …)` (multi-stroke) | Rejected — single strokes only (known ceiling) |
| K6 | Bind a textobject ID (`clay.editor.clientSelectTextobject.class.around.current`) | Accepted — auto-declared on first bind; works in grammar files |

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
clientExecuteEditorCommand({ commandId: "clay.editor.clientSetSelection.selectLine" });
```

| # | Action | Expected |
|---|--------|----------|
| K11 | Cold start with the call above | NOT delivered — no client subscribed yet (expected; advisory) |
| K12 | Open a file, trigger runtime reload via settings appearance switch while connected | init.js reruns; the line under the caret becomes selected — proves op → gate → broadcast → connection → widget dispatch |
| K13 | Change `commandId` to `"clay.application.quit"`, reload | Op rejects ("not a known editor command"); nothing published |
| K14 | Third-party package without `editor-control` permission calls the op | Denied (covered by automated tests; not reachable from init.js by design) |

## Negative checks

- Key routing never runs package JavaScript in the keypress path.
- Unknown command IDs at runtime map to a no-op result, never a crash.

## Known ceilings

- Multi-stroke chords unsupported by `bindKey`.
- Textobject/smart-select IDs ship with NO defaults; binding is the package
  or user's job.
