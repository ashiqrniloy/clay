// ============================================================================
// Clay canonical example configuration — examples/init.js
// ============================================================================
//
// Copy this file to ~/.config/clay/init.js and adjust it. It demonstrates
// EVERY user-facing configuration surface Clay supports today, with all
// documented options annotated. Configuration is plain JavaScript evaluated
// by the Clay server at startup and on runtime reload; failed evaluation
// preserves the previous working configuration and reports a clay.runtime.*
// diagnostic.
//
// Ground rules enforced by Clay (not this file):
//   - Configuration only runs at startup/reload — never on keystroke or paint.
//   - Every option is a documented Clay JS API. Hidden JSON/TOML keys,
//     ad hoc flags, and raw Deno ops are rejected by policy.
//   - init.js grants no filesystem/network/shell/package-install authority.
//   - `authorizeLanguageServer` grants must happen BEFORE the first
//     `loadPackage` call, which seals authority for the generation.
//
// Import map (modules available to init.js):
//   clay:configuration  clay:keybindings  clay:theme       clay:syntax
//   clay:packages       clay:editor       clay:modes       clay:documents
//   clay:workspace      clay:language-server                clay:commands

// ----------------------------------------------------------------------------
// 1. Modular configuration — clay:configuration
// ----------------------------------------------------------------------------
// init.js may split settings across local files next to init.js. Each module
// is evaluated as part of the configuration root (same authority, same
// diagnostics). Paths are relative to the config root (~/.config/clay).
//
// import { loadConfigurationModule, getConfigurationState } from "clay:configuration";
// await loadConfigurationModule({ path: "./keys.js" });      // e.g. all bindKey calls
// await loadConfigurationModule({ path: "./typography.js" }); // e.g. setTypography call
//
// getConfigurationState() is a read-only inspection API for app/help surfaces:
// const state = getConfigurationState();

// ----------------------------------------------------------------------------
// 2. Language-server grants — clay:language-server
// ----------------------------------------------------------------------------
// Configuration-only grant API. Binds exact package provenance, contribution
// fingerprint, resolved executable, inherited env names, and workspace root
// ids. Grants start NO process; sessions start later via
// startLanguageServerSession with a matching grant.
//
// MUST run before the first loadPackage() — that call seals authority.
// workspaceRootIds refer to the workspace roots open in the session
// (1-based, as listed by the workspace state).
//
import { authorizeLanguageServer } from "clay:language-server";

// Grants fail closed on environmental problems — missing executable
// (executable_not_found) or no matching workspace root
// (unknown_workspace_root). Keep that from sinking the rest of the
// configuration: each grant degrades independently and only its language
// server stays inactive until the tooling exists.
async function grantLanguageServer(options) {
  try {
    await authorizeLanguageServer(options);
  } catch {
    // Tooling not installed (or root absent) — skip this server only.
  }
}

await grantLanguageServer({
  package: "@clay/lsp-rust",
  contribution: "lsp-rust.server",
  workspaceRootIds: [1],
});

await grantLanguageServer({
  package: "@clay/lsp-typescript",
  contribution: "lsp-typescript.server",
  workspaceRootIds: [1],
});

await grantLanguageServer({
  package: "@clay/lsp-javascript",
  contribution: "lsp-javascript.server",
  workspaceRootIds: [1],
});

await grantLanguageServer({
  package: "@clay/lsp-markdown",
  contribution: "lsp-markdown.server",
  workspaceRootIds: [1],
});

// ----------------------------------------------------------------------------
// 3. First-party packages — clay:packages
// ----------------------------------------------------------------------------
// Explicit opt-in loading of bundled @clay/* packages. One line per package.
// loadPackage imports the package's load entry and runs it with host-stamped
// provenance. There is NO auto-loading: what you don't load here is inactive.
//
// Available first-party specifiers:
//   Grammar/mode packages:  @clay/markdown  @clay/rust  @clay/typescript
//                           @clay/javascript
//   LSP bridge packages:    @clay/lsp-rust  @clay/lsp-typescript
//                           @clay/lsp-javascript  @clay/lsp-markdown
//                           (authorize first, section 2; load grammar
//                            packages before their LSP bridges)
//   Settings UI:            @clay/settings
//   Themes:                 @clay/theme-gruvbox-material-dark
//                           @clay/theme-gruvbox-material-light
//                           @clay/theme-modus-operandi
//                           @clay/theme-modus-vivendi
//   Git read-only panel:    @clay/git
//
// serverListFirstPartyPackageSpecifiers() returns this list at runtime.
import { loadPackage } from "clay:packages";

await loadPackage("@clay/markdown");   // prose mode + parser + prose movement
await loadPackage("@clay/rust");       // code mode + tree-sitter grammar
await loadPackage("@clay/typescript");
await loadPackage("@clay/javascript");
await loadPackage("@clay/settings");   // settings panel (theme/appearance UI)
await loadPackage("@clay/lsp-rust");        // after section-2 grant
await loadPackage("@clay/lsp-typescript");
await loadPackage("@clay/lsp-javascript");
await loadPackage("@clay/lsp-markdown");
// await loadPackage("@clay/git");

// ----------------------------------------------------------------------------
// 4. Theme + appearance — clay:theme
// ----------------------------------------------------------------------------
// setTheme selects a loaded first-party theme package by specifier. The theme
// owns colors, text styles (including diagnostic styles), and component
// tokens. Themes never execute user code or accept raw CSS.
import { setTheme, setTypography, setAppearance } from "clay:theme";

setTheme("@clay/theme-gruvbox-material-dark");
// setTheme("@clay/theme-gruvbox-material-light");
// setTheme("@clay/theme-modus-operandi");
// setTheme("@clay/theme-modus-vivendi");
// setTheme({ specifier: "@clay/theme-modus-vivendi" }); // object form

// setAppearance selects the appearance variant within the active theme.
// Usually left to the settings panel (it persists the choice and reloads);
// setting it here applies it at startup.
// setAppearance("dark");

// ----------------------------------------------------------------------------
// 5. Typography + ligatures — clay:theme setTypography
// ----------------------------------------------------------------------------
// All THREE profiles (monospace, proportional, ui) are required and replaced
// atomically; a failed call keeps the previous typography.
//
// Per profile:
//   families   string[]  font-family fallback stack
//   size       number    logical-pixel size
//   ligatures  object?   optional OpenType ligature/feature policy:
//     enableStandard        boolean   liga + clig          (default true)
//     enableContextual      boolean   calt                 (default true)
//     discretionaryFeatures string[]  up to 32 feature tags to enable
//                                     (e.g. "ss01", "zero", "onum")
//     rawFeatures           string?   CSS font-feature-settings format,
//                                     at most 256 bytes (escape hatch)
//     disableFeatures       string[]  up to 32 tags forced off (last wins)
//
// Ownership: the policy belongs to the FONT ROLE, not the mode — every
// document using that role shares it. Merge order is last-wins:
// standard → contextual → discretionary → rawFeatures → disableFeatures.
setTypography({
  monospace: {
    families: ["MartianMono Nerd Font", "monospace"],
    size: 16,
    ligatures: { enableStandard: true, enableContextual: true },
    // Ligature examples:
    //   ligatures: { enableContextual: false }        // disable calt ligatures
    //   ligatures: { discretionaryFeatures: ["zero"] }// slashed zero
    //   ligatures: { disableFeatures: ["calt"] }      // force calt off
    //   ligatures: { rawFeatures: '"calt" 0, "zero" 1' }
  },
  proportional: { families: ["Noto Sans", "sans-serif"], size: 17 },
  ui: { families: ["system-ui"], size: 13 },
});

// ----------------------------------------------------------------------------
// 6. Caret styling — clay:editor clientSetCursorStyle
// ----------------------------------------------------------------------------
// Runtime override for caret shape/blink. Resolution order:
// runtime override (this call) > per-mode editorRules.caretStyle (package
// manifests) > theme default. Color always stays theme-owned (base.caret).
//
// Options (all optional; omitted fields keep the layer below):
//   shape             "bar" | "line" | "block" | "underline"
//   blink             "solid" | "blink" | "phase" | "smooth"
//                     ("solid" never hides — reduced-motion friendly)
//   widthPx           number   stroke thickness for bar/line/underline
//   heightPct         number   caret height as fraction of line height
//   hollow            boolean  render "block" as an outline
//   stopBlinkOnTyping boolean  restart blink to visible on typing
//                              (default true)
import { clientSetCursorStyle } from "clay:editor";

clientSetCursorStyle({ shape: "bar", blink: "blink" });
// clientSetCursorStyle({ shape: "block", blink: "blink", hollow: false });
// clientSetCursorStyle({ shape: "underline", blink: "phase", stopBlinkOnTyping: false });
// clientSetCursorStyle({ shape: "bar", blink: "solid", widthPx: 2.5 });

// ----------------------------------------------------------------------------
// 7. Key bindings — clay:keybindings
// ----------------------------------------------------------------------------
// bindKey(key, commandId, options?)  — bind a chord to a command ID.
// unbindKey(key, options?)           — remove a binding.
// listKeyBindings(scope?)            — inspect bindings ("all" default).
//
// Key format: single stroke "Modifier+Key" (Ctrl/Shift/Alt + a key name,
// e.g. "Ctrl+O", "Shift+Alt+Down"). Multi-stroke chords ("g g") are NOT
// supported yet. Bindings validate deny-by-default; unknown or non-editor
// command IDs are rejected. Last binding for a chord wins.
//
// Bindable editor command IDs (Plan 071 surface):
//   Movement:      clay.editor.clientMoveCursor.nextWordStart
//                  clay.editor.clientMoveCursor.prevWordStart
//                  clay.editor.clientMoveCursor.nextParagraph
//                  clay.editor.clientMoveCursor.prevParagraph
//   Selection:     clay.editor.clientSetSelection.selectWord
//                  clay.editor.clientSetSelection.selectLine
//   Multi-cursor:  clay.editor.clientAddCursor.below / .above
//                  clay.editor.clientColumnSelect.down / .up / .left / .right
//                  clay.editor.clientSelectNextMatch / .clientSelectPrevMatch
//                  clay.editor.clientSelectAllMatches
//                  clay.editor.clientKeepSelection / .clientRemoveSelection
//                  clay.editor.clientCancelMultipleSelections
//                  clay.editor.clientUndoCursorMove
//   Text objects (grammar-backed; auto-declared on first bind):
//                  clay.editor.clientSelectTextobject.<kind>.<scope>.<direction>
//                    kind:      function | class | argument | comment | loop |
//                               conditional | call | statement
//                    scope:     inner | around
//                    direction: current | next | previous
//   Smart select:  clay.editor.clientSmartSelect.expand / .shrink
//   Clipboard/edit:clay.editor.clientCopySelection / clientCutSelection /
//                  clientPasteClipboard / clientUndo / clientRedo
//   App:           clay.documents.clientOpenFileDialog,
//                  clay.editor.clientShowOpenDocuments,
//                  clay.editor.clientRequestResync, ...

import { bindKey, unbindKey } from "clay:keybindings";

bindKey("Ctrl+O", "clay.documents.clientOpenFileDialog", { scope: "editor" });

// Text objects + smart select ship with NO default bindings by design —
// bind them to your taste (single strokes only):
bindKey("Alt+I", "clay.editor.clientSelectTextobject.function.inner.current", { scope: "editor" });
bindKey("Alt+O", "clay.editor.clientSelectTextobject.function.around.current", { scope: "editor" });
bindKey("Alt+A", "clay.editor.clientSelectTextobject.argument.inner.current", { scope: "editor" });
bindKey("Alt+C", "clay.editor.clientSelectTextobject.comment.around.current", { scope: "editor" });
bindKey("Alt+E", "clay.editor.clientSmartSelect.expand", { scope: "editor" });
bindKey("Alt+R", "clay.editor.clientSmartSelect.shrink", { scope: "editor" });

// Rebinding a built-in: Ctrl+B becomes "previous word start".
bindKey("Ctrl+B", "clay.editor.clientMoveCursor.prevWordStart", { scope: "editor" });
// unbindKey("Ctrl+B", { scope: "editor" });

// ----------------------------------------------------------------------------
// 8. Syntax engine preference — clay:syntax
// ----------------------------------------------------------------------------
// Force the parser tier for a language or first-party package.
//   target: language/package name (e.g. "rust")
//   tier:   "native" | "wasm" | "javascript" (alias "js")
// Packages cannot promote themselves over the native tier without this
// explicit user preference.

import { setSyntaxEnginePreference } from "clay:syntax";

// setSyntaxEnginePreference("rust", "wasm");

// ----------------------------------------------------------------------------
// 9. Programmatic editor control — clay:editor
// ----------------------------------------------------------------------------
// These run through the `editor-control` trust boundary. init.js is trusted
// user configuration (no package context), so it passes the gate without a
// mode declaration. Packages additionally need the approved `editor-control`
// permission and must declare clay.editorControl.modes in package.json.
//
// clientExecuteEditorCommand({ commandId }) pushes ONE known editor command
// ID through the gated server→client channel (advisory; dropped silently if
// unknown or undeliverable). Only the command IDs listed in section 7 are
// accepted.

import { clientExecuteEditorCommand } from "clay:editor";

// Example — select the current line on delivery:
// clientExecuteEditorCommand({ commandId: "clay.editor.clientSetSelection.selectLine" });

// Typed validation facades (validate arguments deny-by-default and return the
// direction-specific command ID; useful for programmatic flows):
//   clientMoveCursor({ direction, granularity?, extend?, count? })
//     direction: nextWordStart | prevWordStart | nextWordEnd | prevWordEnd |
//                nextParagraph | prevParagraph | firstNonWhitespace |
//                lastNonWhitespace | matchingPair | left | right | up |
//                down | start | end
//   clientSetSelection({ action, extend?, direction? })
//     action: selectWord | selectLine | selectParagraph
//   clientAddCursor({ direction: "below" | "above" })
//   clientColumnSelect({ direction: "down" | "up" | "left" | "right" })
//   clientSelectTextobject({ object, around?, direction? })
//     object: function | class | argument | comment | loop | conditional |
//             call | statement
//   clientSmartSelect({ action: "expand" | "shrink" })
// Argless helpers return stable command IDs for binding/execution:
//   clientSelectNextMatch() clientSelectPrevMatch() clientSelectAllMatches()
//   clientCancelMultipleSelections() clientKeepSelection()
//   clientRemoveSelection() clientUndoCursorMove()

// ----------------------------------------------------------------------------
// 10. Planned — NOT callable yet (documented placeholders)
// ----------------------------------------------------------------------------
// These clay:configuration exports exist as facade stubs and inventory
// entries but have no server-side validators yet. Calling them throws.
// They will become user-facing configuration in later phases:
//   - setPackageOption      behavior-changing package options
//   - setModePreference     per-mode user preferences
//   - setDecorationTheme    decoration palette overrides
//   - setParsePolicy        concrete parse-policy validators
// Do not write hidden-key workarounds for them; hidden keys are rejected.
