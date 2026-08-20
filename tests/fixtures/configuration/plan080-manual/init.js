// ============================================================================
// Clay canonical example configuration — examples/init.js
// ============================================================================
//
// Copy the whole tree to your configuration root and adjust it:
//
//   cp -r examples/. ~/.config/clay/
//
// The example is split into three modules that mirror the three components of
// a real configuration:
//
//   init.js                      base Clay config (this file) — every
//                                user-facing configuration surface, fully
//                                functional standalone
//   packages/first-party.js      language-server grants + first-party
//                                @clay/* package loads (loaded at the end of
//                                this file via loadConfigurationModule)
//   packages/third-party.js      commented template for third-party packages
//                                (no third-party packages ship)
//
// Package configuration lives in separate modules so a broken or missing
// module degrades independently (recorded as a configuration.module_failed
// diagnostic) and never blocks the base configuration above it or app
// launch — "use Clay to fix Clay". The base init.js stays fully functional
// standalone; the module loads below are fault-isolated and optional.
//
// It demonstrates EVERY user-facing configuration surface Clay supports
// today, with all documented options annotated. Configuration is plain
// JavaScript evaluated by the Clay server at startup and on runtime reload;
// failed evaluation preserves the previous working configuration and reports
// a runtime.* diagnostic.
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
//   clay:workspace      clay:language-server  clay:shell   clay:commands

// ----------------------------------------------------------------------------
// 1. Modular configuration — clay:configuration
// ----------------------------------------------------------------------------
// init.js may split settings across local files next to init.js. Each module
// is evaluated as part of the configuration root (same authority, same
// diagnostics). Paths are relative to the config root (~/.config/clay).
//
// This example loads its package configuration from two modules at the end
// of this file (section 11), both with optional: true so a broken or missing
// package module records configuration.module_failed and never blocks the
// base configuration or app launch.
//
// Other splits work the same way, e.g.:
//   await loadConfigurationModule({ path: "./keys.js" });      // all bindKey calls
//   await loadConfigurationModule({ path: "./typography.js" }); // setTypography call
//
// getConfigurationState() is a read-only inspection API for app/help surfaces:
// const state = getConfigurationState();

import { loadConfigurationModule, getConfigurationState } from "clay:configuration";

// ----------------------------------------------------------------------------
// 2. Theme + appearance — clay:theme
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

// setAppearance selects the canonical appearance variant. To use it here,
// comment out the explicit setTheme above; an explicit theme always wins.
// Usually leave this to the settings panel (it persists the choice and reloads).
// setAppearance("light");  // canonical Modus Operandi
// setAppearance("dark");   // canonical Modus Vivendi
// setAppearance("system"); // OS signal, dark fallback

// ----------------------------------------------------------------------------
// 3. Typography + ligatures — clay:theme setTypography
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
  // Optional UI hierarchy. When present, all seven bounded ratios are required.
  // Each ratio is finite, > 0, and <= 4; these values preserve Clay defaults.
  hierarchy: {
    display: 1.5,
    title: 14 / 12,
    section: 13 / 12,
    body: 1,
    status: 1,
    detail: 10 / 12,
    caption: 0.75,
  },
});

// ----------------------------------------------------------------------------
// 4. Caret styling — clay:editor clientSetCursorStyle
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
// 5. Editor layout — clay:editor clientSetEditorLayout
// ----------------------------------------------------------------------------
// User-owned wrap-policy override. Resolution order:
// runtime override (this call) > per-mode editorRules.layout.wrap (package
// manifests) > WrapPolicy::from_font_role default (monospace → none,
// proportional → column 72).
//
// Options:
//   wrapPolicy  "none" | "viewport" | "column"   (required)
//     "none"      no wrapping — horizontal scrolling for code
//     "viewport"  wrap at the pane content width
//     "column"    wrap at columnCap average character widths (prose)
//   columnCap   number   column cap for "column" (default 72, clamped to
//                        16–240; ignored for "none"/"viewport")
//
// The override is package-unforgeable: the op lives in the trusted runtime
// extension only, so third-party package code cannot resolve it. It survives
// configuration reload. Comment the call out to keep per-mode defaults.
import { clientSetEditorLayout } from "clay:editor";

clientSetEditorLayout({ wrapPolicy: "column", columnCap: 72 });
// clientSetEditorLayout({ wrapPolicy: "none" });      // code: horizontal scroll
// clientSetEditorLayout({ wrapPolicy: "viewport" });  // wrap at pane width
// clientSetEditorLayout({ wrapPolicy: "column" });   // default 72-column prose

// ----------------------------------------------------------------------------
// 6. Key bindings — clay:keybindings
// ----------------------------------------------------------------------------
// bindKey(key, commandId, options?)              — bind one chord (single form).
// bindKey({ scope, bindings: { chord: id, ... } }) — bind a whole table in one
//                                                    call (table form; scope typed
//                                                    once; all-or-nothing).
// unbindKey(key, options?)                       — remove one binding.
// unbindKey({ scope, keys: [chord, ...] })       — remove many at once.
// listKeyBindings(scope?)                        — inspect bindings ("all" default).
//
// Key format: single stroke "Modifier+Key" (Ctrl/Shift/Alt + a key name,
// e.g. "Ctrl+O", "Shift+Alt+Down") OR a space-separated multi-stroke chord
// (e.g. "Ctrl+X Ctrl+P", "g g"). Single-stroke is the fast path; a
// multi-stroke chord holds a pending chord until the sequence completes,
// times out, or mismatches — on mismatch the key re-evaluates fresh, so a
// half-typed chord never eats typing. Example (commented; the shipped
// defaults are re-declared in the batch tables below):
//   // bindKey("Ctrl+X Ctrl+P", "controlCenter.open", { scope: "global" });
// Bindings validate deny-by-default; unknown or non-editor command IDs are
// rejected, and a chord that is a strict prefix of another binding in the
// same scope is rejected as ambiguous at bind time. Last binding for a chord
// wins (within a table, duplicate chords collapse to the last value).
// Table-form calls validate every entry before applying any: one bad entry
// rejects the whole table and names its 1-based index, so nothing binds
// halfway.
//
// Bindable editor command IDs (Plan 071 surface):
//   Movement:      editor.clientMoveCursor.nextWordStart
//                  editor.clientMoveCursor.prevWordStart
//                  editor.clientMoveCursor.nextParagraph
//                  editor.clientMoveCursor.prevParagraph
//   Selection:     editor.clientSetSelection.selectWord
//                  editor.clientSetSelection.selectLine
//   Multi-cursor:  editor.clientAddCursor.below / .above
//                  editor.clientColumnSelect.down / .up / .left / .right
//                  editor.clientSelectNextMatch / .clientSelectPrevMatch
//                  editor.clientSelectAllMatches
//                  editor.clientKeepSelection / .clientRemoveSelection
//                  editor.clientCancelMultipleSelections
//                  editor.clientUndoCursorMove
//   Text objects (grammar-backed; auto-declared on first bind):
//                  editor.clientSelectTextobject.<kind>.<scope>.<direction>
//                    kind:      function | class | argument | comment | loop |
//                               conditional | call | statement
//                    scope:     inner | around
//                    direction: current | next | previous
//   Smart select:  editor.clientSmartSelect.expand / .shrink
//   Clipboard/edit:editor.clientCopySelection / clientCutSelection /
//                  clientPasteClipboard / clientUndo / clientRedo
//   App:           documents.clientOpenFileDialog,
//                  editor.clientShowOpenDocuments,
//                  editor.clientRequestResync, ...
//                  (clientShowOpenDocuments opens on the focused pane and
//                  lists every pane's open documents since Phase 22.2)
//   Workspace:     workspace.toggleFileBrowser  Ctrl+B
//                  (hidden by default; visibility is per tab)
//
// Bindable shell command IDs (Phase 22.1 window splits + Phase 22.4 tabs; all
// ship with default chords, so no bindKey is needed unless you want different
// ones. Overrides use { scope: "global" } to match the shipped default context):
//   Splits:        shell.clientSplitPaneVertical    Ctrl+\        side by side
//                  shell.clientSplitPaneHorizontal  Ctrl+-        stacked
//                  (Phase 22.7 direction aliases, no default chords — bind
//                   your own if you prefer direction vocabulary:
//                   shell.clientSplitPaneRight  = Vertical (beside)
//                   shell.clientSplitPaneDown   = Horizontal (below))
//                  shell.clientAddEqualPane         Ctrl+Shift+\  redivide equal
//                  shell.clientClosePane            Ctrl+Alt+W
//                  (close is document-aware since Phase 22.2: a pane with a
//                  dirty document is protected until its save-conflict menu
//                  resolves; closing a clean pane releases its document lease)
//   Pane focus:    shell.clientFocusPaneNext        Ctrl+Alt+Right
//                  shell.clientFocusPanePrev        Ctrl+Alt+Left
//   Resize:        shell.clientResizePaneLeft       Ctrl+Alt+Shift+Left
//                  shell.clientResizePaneRight      Ctrl+Alt+Shift+Right
//                  shell.clientResizePaneUp         Ctrl+Alt+Shift+Up
//                  shell.clientResizePaneDown       Ctrl+Alt+Shift+Down
//   Move pane:     shell.clientMovePaneNext         Ctrl+Alt+]
//                  shell.clientMovePanePrev         Ctrl+Alt+[
//
// Tab management (Phase 22.4; same bindKey story as the splits above):
//   Next/prev:    shell.clientTabNext            Ctrl+Tab        wraps around
//                 shell.clientTabPrev            Ctrl+Shift+Tab  wraps around
//   New:          shell.clientTabNew             Ctrl+T          same flow as "+"
//   Close:        shell.clientTabClose           Ctrl+Shift+W    last tab protected;
//                                                                    dirty tabs confirm
//   Activate:     shell.clientTabActivate.<N>    Ctrl+<N>        1-based, N in 1..=9;
//                                                                    beyond count = no-op
//   Move:         shell.clientTabMoveLeft        Ctrl+Shift+[    boundary = no-op
//                 shell.clientTabMoveRight       Ctrl+Shift+]    boundary = no-op
//                 shell.clientTabMoveTo.<N>      Ctrl+Shift+<N>  1-based, N in 1..=9;
//                                                                    beyond count = no-op
// Policies: numbering follows the card order (registry order, entry-less
// mounted tabs appended). Next/prev wrap at both ends; moves never wrap; the
// active-tab status survives moves. Close protects the last tab, and a tab
// with unsaved documents gets the save-all/discard/cancel confirm menu.
// Window state persists across restarts (Phase 22.5): tab order, the active
// tab, each tab's workspace + split tree, and each pane's open document
// (unsaved edits do not; a missing workspace root or file is skipped on
// restore).
// Numbered families are capped at 9 by design — reach tab 10+ with
// Ctrl+Tab / Ctrl+Shift+Tab or a card click. Tab-bar keyboard focus
// traversal (per-card focus) stays deferred after Phase 22.6 — tab cards
// remain click/command driven (plan 077 Further Actions).
//
// Rebinding tab commands (Phase 22.4; example chords, commented out — one
// per family kind: scalar, numbered activate, numbered move-to; the chord
// parser accepts single characters, Tab, arrows, Space, Enter, Backspace,
// Delete, and Escape — no PageUp/PageDown/F-keys):
// bindKey({ scope: "global", bindings: {
//   "Alt+Right": "shell.clientTabNext",
//   "Alt+1": "shell.clientTabActivate.1",
//   "Ctrl+Alt+Shift+9": "shell.clientTabMoveTo.9",
// }});

// Control Center (Phase 24.2): built-in server-first command
// controlCenter.open ships with the default Ctrl+X Ctrl+P chord (Global
// scope) in the default behavior manifest, re-declared in the batch table
// below as an idempotent no-op override. The Control Center is a transient
// menu session: listing grants no authority, and it cannot be styled,
// positioned, filtered, or dismissed from init.js. Multi-stroke chord
// sequences are supported (Phase 24.5): space-separated strokes, e.g.
// "Ctrl+X Ctrl+P", "g g".

// Path Browser (Phase 24.3): built-in server-first command
// controlCenter.openPath ships with the Phase 24.5 sequence default
// Ctrl+X Ctrl+F chord (Global scope), re-declared in the batch table below
// as an idempotent no-op override. It opens a dired-style browse session
// seeded from the active document's directory; Enter descends/opens,
// Alt+Enter opens a directory as this tab's workspace, Backspace on an
// empty filter ascends. Browse navigation alone grants nothing — opening a
// file or workspace is the explicit grant — and packages get no
// arbitrary-path access. The command id is stable across the Phase 24.5
// sequence-default handoff (previously the temporary Ctrl+Alt+P chord).

// Default keybindings — implemented below so this file, taken as-is, installs
// every shipped default chord (they are already active without init.js;
// re-declaring them is an idempotent no-op override and doubles as the
// complete reference). Source of truth: default_keymaps() in
// src/protocol/mod.rs. Both call forms are shown: batch tables for the
// defaults, single-form calls for one-off binds.

import { bindKey, unbindKey } from "clay:keybindings";

// Editor-scope defaults (batch table form — one call, scope typed once):
bindKey({
  scope: "editor",
  bindings: {
    "Enter": "text.insert_newline",
    "Tab": "text.insert_tab",
  },
});

// Global-scope defaults — Phase 22.1 splits and pane focus:
bindKey({
  scope: "global",
  bindings: {
    "Ctrl+\\": "shell.clientSplitPaneVertical",
    "Ctrl+-": "shell.clientSplitPaneHorizontal",
    "Ctrl+Shift+\\": "shell.clientAddEqualPane",
    "Ctrl+Alt+W": "shell.clientClosePane",
    "Ctrl+Alt+Left": "shell.clientFocusPanePrev",
    "Ctrl+Alt+Right": "shell.clientFocusPaneNext",
    "Ctrl+Alt+Shift+Left": "shell.clientResizePaneLeft",
    "Ctrl+Alt+Shift+Right": "shell.clientResizePaneRight",
    "Ctrl+Alt+Shift+Up": "shell.clientResizePaneUp",
    "Ctrl+Alt+Shift+Down": "shell.clientResizePaneDown",
    "Ctrl+Alt+[": "shell.clientMovePanePrev",
    "Ctrl+Alt+]": "shell.clientMovePaneNext",
    // Built-in server-first reload; this re-declaration is idempotent.
    "Ctrl+Shift+R": "runtime.reloadConfiguration",
    // Built-in server-first Control Center (Phase 24.2); this re-declaration
    // is idempotent. Global scope, ServerFirst routing; override/remove via
    // bindKey/unbindKey (see the commented example below).
    "Ctrl+X Ctrl+P": "controlCenter.open",
    // Built-in server-first Path Browser (Phase 24.3); this re-declaration
    // is idempotent. Phase 24.5 sequence default; override/remove via
    // bindKey/unbindKey (see the commented example below).
    "Ctrl+X Ctrl+F": "controlCenter.openPath",
  },
});

// Global-scope defaults — Phase 22.4 tab management (numbered families are
// capped at 9 by design; reach tab 10+ with Ctrl+Tab / Ctrl+Shift+Tab):
bindKey({
  scope: "global",
  bindings: {
    "Ctrl+Tab": "shell.clientTabNext",
    "Ctrl+Shift+Tab": "shell.clientTabPrev",
    "Ctrl+T": "shell.clientTabNew",
    "Ctrl+Shift+W": "shell.clientTabClose",
    "Ctrl+Shift+[": "shell.clientTabMoveLeft",
    "Ctrl+Shift+]": "shell.clientTabMoveRight",
    "Ctrl+1": "shell.clientTabActivate.1",
    "Ctrl+2": "shell.clientTabActivate.2",
    "Ctrl+3": "shell.clientTabActivate.3",
    "Ctrl+4": "shell.clientTabActivate.4",
    "Ctrl+5": "shell.clientTabActivate.5",
    "Ctrl+6": "shell.clientTabActivate.6",
    "Ctrl+7": "shell.clientTabActivate.7",
    "Ctrl+8": "shell.clientTabActivate.8",
    "Ctrl+9": "shell.clientTabActivate.9",
    "Ctrl+Shift+1": "shell.clientTabMoveTo.1",
    "Ctrl+Shift+2": "shell.clientTabMoveTo.2",
    "Ctrl+Shift+3": "shell.clientTabMoveTo.3",
    "Ctrl+Shift+4": "shell.clientTabMoveTo.4",
    "Ctrl+Shift+5": "shell.clientTabMoveTo.5",
    "Ctrl+Shift+6": "shell.clientTabMoveTo.6",
    "Ctrl+Shift+7": "shell.clientTabMoveTo.7",
    "Ctrl+Shift+8": "shell.clientTabMoveTo.8",
    "Ctrl+Shift+9": "shell.clientTabMoveTo.9",
  },
});

// Single form — one binding per call (batch tables work for these too):
bindKey("Ctrl+O", "documents.clientOpenFileDialog", { scope: "editor" });
bindKey("Ctrl+B", "workspace.toggleFileBrowser", { scope: "editor" });

// Text objects + smart select ship with NO default bindings by design —
// bound here as single-form examples (single strokes and multi-stroke
// chords both work):
bindKey("Alt+I", "editor.clientSelectTextobject.function.inner.current", { scope: "editor" });
bindKey("Alt+O", "editor.clientSelectTextobject.function.around.current", { scope: "editor" });
bindKey("Alt+A", "editor.clientSelectTextobject.argument.inner.current", { scope: "editor" });
bindKey("Alt+C", "editor.clientSelectTextobject.comment.around.current", { scope: "editor" });
bindKey("Alt+E", "editor.clientSmartSelect.expand", { scope: "editor" });
bindKey("Alt+R", "editor.clientSmartSelect.shrink", { scope: "editor" });

// Rebinding example (single form): move workspace toggle to another chord
// if desired. Last binding for a chord wins:
// bindKey("Alt+W", "workspace.toggleFileBrowser", { scope: "editor" });
// unbindKey("Ctrl+B", { scope: "editor" });  // or remove the default example binding

// Rebinding a shipped default (example: "add equal pane" on a different
// chord; scope "global" matches the shipped default context):
// bindKey("Ctrl+Shift+=", "shell.clientAddEqualPane", { scope: "global" });

// Rebinding the Control Center default (Phase 24.2): unbind the shipped
// Ctrl+X Ctrl+P chord, then bind another global chord (single-stroke or
// multi-stroke). Without the unbind the default remains bound; last binding
// for a chord wins:
// unbindKey("Ctrl+X Ctrl+P", { scope: "global" });
// bindKey("Alt+X", "controlCenter.open", { scope: "global" });

// Rebinding the Path Browser default (Phase 24.3): unbind the shipped
// Ctrl+X Ctrl+F chord, then bind another global chord (command id stays
// stable across the Phase 24.5 sequence-default handoff):
// unbindKey("Ctrl+X Ctrl+F", { scope: "global" });
// bindKey("Alt+P", "controlCenter.openPath", { scope: "global" });

// ----------------------------------------------------------------------------
// 7. Window split pane focus — clay:shell
// ----------------------------------------------------------------------------
// Pane focus policy for split panes (Phase 22.1). One option:
// Since Phase 22.3 the policy applies per active tab (each tab carries its
// own pane-focus policy; switching tabs preserves each tab's policy).
//   paneFocusPolicy  "click" | "cursor"
//     "click"   (default) pointer-down inside a pane activates it
//     "cursor"  focus follows the pointer across panes; focus changes are
//               skipped while dragging a divider or panel resize handle
// Tab/Shift+Tab pane cycling works under both policies. The split commands
// themselves and their default chords are listed in the key bindings
// section above; unknown paneFocusPolicy values fail evaluation with a
// shell.invalid_pane_focus_policy diagnostic.
import { setPaneFocusPolicy } from "clay:shell";

// setPaneFocusPolicy({ paneFocusPolicy: "click" });   // default
// setPaneFocusPolicy({ paneFocusPolicy: "cursor" });

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
// unknown or undeliverable). Only the command IDs listed in section 6 are
// accepted.

import { clientExecuteEditorCommand } from "clay:editor";

// Example — select the current line on delivery:
// clientExecuteEditorCommand({ commandId: "editor.clientSetSelection.selectLine" });

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

// ----------------------------------------------------------------------------
// 11. Package configuration modules — fault-isolated optional loads
// ----------------------------------------------------------------------------
// Package configuration is segregated from the base config and loaded as
// optional modules: a broken or missing module records a
// configuration.module_failed warning (bounded, root-relative) and never
// blocks the base configuration above it or app launch — "use Clay to fix
// Clay". The grant-before-loadPackage ordering constraint lives INSIDE
// packages/first-party.js (see its header).
//
// Optional modules may not exist at evaluation time; paths are still
// validated to stay inside the config root.
await loadConfigurationModule({
  path: "./packages/first-party.js",
  optional: true,
});

await loadConfigurationModule({
  path: "./packages/third-party.js",
  optional: true,
});
