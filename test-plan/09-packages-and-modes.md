# 09 — Packages and Modes

Explicit package loading, mode classification/activation, package behavior
manifests (movement/caret/editor rules), settings UI, theme switching.

## Setup

init.js (subset of `examples/init.js`):

```js
import { loadPackage } from "clay:packages";
import { setTheme } from "clay:theme";
await loadPackage("@clay/markdown");
await loadPackage("@clay/rust");
await loadPackage("@clay/settings");
setTheme("@clay/theme-gruvbox-material-dark");
```

Workspace `/tmp/clay-manual` with `test.rs`, `test.md`, `plain.txt`.

## Loading

| # | Action | Expected |
|---|--------|----------|
| P1 | Launch with the setup above | All three packages load silently; no diagnostics |
| P2 | Remove the `loadPackage` lines, relaunch | No markdown/rust behavior: `test.md` opens as core.text, no decorations — explicit opt-in, no auto-loading |
| P3 | `loadPackage("@clay/does-not-exist")` | Diagnostic naming the specifier; app continues |
| P4 | `loadPackage("some-third-party")` | Rejected — first-party specifiers only |
| P5 | Syntax error inside a loaded package's load entry | Diagnostic; previous generation retained; editor keeps working |

## Mode classification

| # | Action | Expected |
|---|--------|----------|
| P6 | Open `test.md` | Markdown mode activates (package-registered pattern matches path/content); prose editor rules active (module 05, M10–M13) |
| P7 | Open `test.rs` | Rust mode; code movement + grammar (module 08) |
| P8 | Open `plain.txt` | core.text fallback (built-in), still editable with sane defaults |
| P9 | Rename `test.md` → `test.xyz`, reopen | Falls back to core.code/core.text per classification rules; no crash |

## Package behavior manifests

| # | Action | Expected |
|---|--------|----------|
| P10 | In `test.md`, check word movement | Prose policy from markdown package manifest (underscore segments stop) |
| P11 | In `test.rs`, `}` electric outdent + `//` continuation on Enter | Code editor rules from rust package manifest |
| P12 | Per-mode `caretStyle` override if a package declares one | Mode override wins over theme default; runtime `clientSetCursorStyle` wins over both |

## Settings UI and theme switching

| # | Action | Expected |
|---|--------|----------|
| P13 | Open settings panel (package command/keybinding), switch theme | Theme applies; choice persists across reload |
| P14 | Switch appearance in settings | Runtime generation reloads live (module 02, C9 mechanism) |
| P15 | Interact with a settings-panel control | Editor text/caret/version/status unchanged by panel interaction |

## Negative checks

- Packages never create native widgets or run client-side JavaScript;
  contributions are inert declarations routed by Clay.
- Fixed panels (settings/browser) resize the editor main rect; they must not
  cover text/caret hit targets (transient overlays may cover).
- Package load never grants filesystem/network/shell authority beyond its
  documented contribution scope.

## Known ceilings

- core.code/core.text activation is server-side classification, not package
  registry activation.
- Third-party packages require the adoption/approval flow (not covered here;
  see trust-boundary automated tests).
