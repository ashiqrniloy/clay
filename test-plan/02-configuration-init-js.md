# 02 — Configuration (init.js)

Verify the server-side configuration evaluation path. Canonical example:
`examples/init.js` (repository root). API reference:
`docs/reference/clay-js-api/configuration.md`.

## Setup

Back up your real config first:

```bash
cp ~/.config/clay/init.js ~/.config/clay/init.js.bak 2>/dev/null || true
cp examples/init.js ~/.config/clay/init.js   # or your own config
```

## Evaluation

| # | Action | Expected |
|---|--------|----------|
| C1 | Launch with `examples/init.js` | All sections evaluate; theme set (gruvbox-material-dark), packages loaded (markdown/rust/typescript/javascript/settings), caret bar+blink, custom bindings active |
| C2 | Watch launching terminal | No `clay.runtime.*` diagnostics; each bad line would print one |
| C3 | Introduce `bindKey("Ctrl+X", "clay.nonexistent.command", …)` | Diagnostic naming rejected command ID; editor otherwise unaffected (deny-by-default) |
| C4 | Introduce `setPackageOption(...)` (planned stub) | Diagnostic: planned API not callable; no crash |
| C5 | Introduce a raw `Deno.core.ops.op_clay_...` call | Rejected/diagnostic — raw ops are not a user surface |
| C6 | Delete init.js entirely, launch | Built-in fallback modes (`core.code`/`core.text`) still edit files (Phase 18.9 behavior) |

## Modular configuration

| # | Action | Expected |
|---|--------|----------|
| C7 | Split: move the `bindKey` calls into `~/.config/clay/keys.js`; in init.js: `await loadConfigurationModule({ path: "./keys.js" })` | Bindings from the module work identically |
| C8 | Point `path` at a file with a syntax error | Diagnostic from the module evaluation; rest of config still applied as documented |

## Live reload (no restart)

| # | Action | Expected |
|---|--------|----------|
| C9 | With settings package loaded, open settings and switch appearance | Runtime generation reloads WHILE the client stays connected; theme/appearance change confirms init.js reran |
| C10 | Before the switch, add a new `bindKey` to init.js; after reload press the chord | New binding active without restart |
| C11 | Change `clientSetCursorStyle` shape, reload via appearance | Caret shape changes live |

## Negative checks

- Configuration JavaScript runs ONLY at startup/reload — typing, scrolling,
  paint must never trigger config evaluation.
- No configuration path grants filesystem/network/shell/package-install
  authority beyond its documented scope.

## Cleanup

```bash
mv ~/.config/clay/init.js.bak ~/.config/clay/init.js 2>/dev/null || rm ~/.config/clay/init.js
```
