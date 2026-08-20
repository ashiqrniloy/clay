# 09 — Packages and Modes

Explicit package loading, mode classification/activation, package behavior
manifests (movement/caret/editor rules), settings UI, theme switching.

## Setup

init.js (subset of `examples/init.js`; the example keeps its package loads
in `examples/packages/first-party.js`):

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

## Plan 088 package/UI contract steps

| # | Action | Expected |
|---|--------|----------|
| P16 | Show a package panel with long content (the settings composition is `panel` + `scroll`) | Content scrolls inside the bounded panel; child rendering and accessibility are clipped to the host; no panel row paints over the editor |
| P17 | Inspect package controls in rest/hover/active/focus/disabled and status/error states | State changes use semantic token roles and focus/disabled semantics; diagnostics are not color-only; disabled actions do not dispatch |
| P18 | Apply canonical dark and light themes with package panels/overlays present | Theme change updates cached package chrome without raw colors, layout churn, or loss of package state; contrast remains readable |
| P19 | Repeat P16–P18 with large UI typography | Panel scroll, labels, controls, and hit/accessibility bounds remain usable; long labels clip/wrap intentionally rather than escaping their host |
| P20 | Inspect package action provenance and available overlay origins | Package actions remain inert and provenance-labelled; packages cannot request `completion`/`centered`, own tabs/panes, or call native widgets/raw ops |
| P21 | Apply a representative theme package with valid typed `designTokens` overrides, then return to a legacy `textStyles`-only theme | Typed overrides win only for their existing semantic roles; cached fallback/projection and AA validation remain intact; no package supplies structure, concrete fonts, or raw colors |

## Plan 088 task 12 Linux execution record (2026-08-15)

| Checks | Result | Evidence |
|---|---|---|
| P16 | NOT RUN visually — settings command is acknowledge-only | `@clay/settings` panel+scroll composition and flex-sizing tests pass; `settings.open` does not persist or make the panel visible, so no false screenshot pass was claimed |
| P17 | PASS automated / NOT RUN interactively | `package_ui_conformance`, `ui_primitive_conformance`, and AccessKit disabled/status tests pass; targeted panel interaction is blocked by host focus/input limits |
| P18 | PASS theme validation / NOT RUN package-panel visually | Dark/default and light-default Clay-window captures under `code-reviews/screenshots/2026-08-14-plan088-modernization/`; bundled theme AA tests pass, but settings/package panel input was host-blocked |
| P19 | PASS strongest available evidence | `large-typography/` capture has no welcome a11y regression; responsive/label-clipping structural tests pass, but a live settings panel could not be opened |
| P20 | PASS automated | Package catalog, provenance, anchor allowlist, raw-style denial, and public-surface tests pass; no new package authority was introduced |
| P21 | PASS automated / NOT RUN visually | `theme_packages`, contrast, and typed-design-token validation tests pass; no bundled first-party theme currently ships a non-empty `designTokens` fixture for a live comparison |

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

## Plan 089 task 9 Linux execution record (2026-08-17)

| Checks | Result | Evidence |
|---|---|---|
| P16–P21 | PASS structural / NOT RUN package-panel visually | Plan 089 did not add new package features; the visual review captured default/loading/error/recovery/large-typography states but settings/package panels remain unrendered because `settings.open` does not persist or make the panel visible |

## Phase 26 theme textStyles axes steps

Deep references: `docs/reference/packages/creating-packages.md` (textStyles
`background`/`scale` entry fields), `docs/reference/primitives/
syntax-vocabulary.md` (Phase 26 theme axes table). Setup: bundled first-party
themes (`@clay/theme-gruvbox-material-dark`, `-light`, `@clay/theme-modus-
operandi`, `-vivendi`) with the markdown + rust fixtures.

| # | Action | Expected |
|---|--------|----------|
| P22 | `setTheme` each bundled theme in turn (init.js, reload) | Every theme resolves the full 35-token vocabulary with distinct colors; `background` entries (Quote/CodeBlock/Diagnostic/SearchMatch) and `scale` entries (headings, CodeSpan) apply per theme; switching themes swaps the axes atomically with no partial paint |
| P23 | Theme with an invalid `scale` (non-finite or outside `(0, 4.0]`) or unknown `token` | Theme load rejected with a deterministic diagnostic; the previous theme stays active; no partial style reaches layout |
| P24 | Compare light vs dark themes on the same document | Background tints and heading scales stay visible in both; syntax colors remain distinct (dormant-token distinctness is enforced per theme package) |

## Phase 27 single-manifest / inspect steps

| # | Action | Expected |
|---|--------|----------|
| P25 | Isolated HOME: `target/debug/clay package inspect @clay/markdown` and `@clay/rust` | Prints `Preset: prose-mode` / `code-mode`, expanded permissions (includes `parse-document`), and `Syntax: … owned by native descriptor`. Status `bundled` is fine. No pnpm required. |
| P26 | Same for `@clay/lsp-rust` | Prints `Preset: lsp-bridge` and `language-server` in permissions. Does **not** start rust-analyzer. Inspect is not a grant. |
| P27 | `init.js` is only `await loadPackage("@clay/markdown")` then `await loadPackage("@clay/rust")` | Open `test.md` / `test.rs` still get mode, commands, completion, syntax. No user `serverRegister*` calls. |
| P28 | Load `@clay/lsp-rust` without `authorizeLanguageServer` | Package metadata may load; no language-server child / no implicit process authority. |

Negative: inspect/loadPackage never grant filesystem/network/shell/language-server. Children are not a sandbox.

## Phase 27.8 Linux execution record (2026-08-19)

| Checks | Result | Evidence |
|---|---|---|
| P25 | PASS CLI | Isolated-HOME `target/debug/clay package inspect @clay/markdown` / `@clay/rust` printed preset, expanded perms, native syntax ownership, status `bundled` |
| P26 | PASS CLI | Same for `@clay/lsp-rust`: `lsp-bridge` + `language-server` in permissions; no child started |
| P27 | PASS automated | Existing one-line `loadPackage` + apply-record tests (`default_init_js_load_package_lines_activate_markdown_and_rust`, language expansion tests) |
| P28 | PASS automated | `lsp_*_package_loads_after_exact_grant_without_starting_child` |

Negative: `textStyles` is inert manifest data — it grants no permission and
executes no code; `background`/`scale` never change the protocol wire shape
(no `DecorationSpan` field added; payload budgets unchanged).

## Phase 26 Linux execution record (2026-08-19)

| Checks | Result | Evidence |
|---|---|---|
| P22 | PASS live | `code-reviews/screenshots/2026-08-18-phase26-review/` — 17 captures across all four themes × rust/ts/js/markdown show per-theme background tints, heading scales, and distinct syntax colors (review-log V1/V2) |
| P23 | PASS automated | `size_scale_ladder_descends_headings_and_clamps_theme_overrides` (clamp to `HIERARCHY_SCALE_MAX`), theme parser validation (`scale` finite in `(0, 4.0]`), `tests/theme_packages.rs` dormant-token distinctness; live theme reload is host-blocked |
| P24 | PASS live | `*-gruvbox-light/` + `*-modus-operandi/` vs `*-gruvbox-dark/` + `*-default/` captures; V4 (light gutter digit contrast) is the one open light-theme defect, tracked in the review log |

## Phase 28 command and intelligence contributions

Deep references: `docs/reference/packages/creating-packages.md`,
`docs/reference/primitives/registry.md`, and the package-specific docs under
`docs/reference/clay-js-api/`.

| # | Action | Expected |
|---|---|---|
| P29 | Load `@clay/markdown` and `@clay/rust`, then open Markdown and Rust files | Behavior manifests install the declared comment, list, heading, enter, completion, and package-command contributions; Markdown uses prose chrome and Rust uses code chrome. No package JS runs in editor paint/text-event paths |
| P30 | Load authorized `@clay/lsp-rust` and inspect a Rust document, then compare with Markdown | The bridge opts into `inlayHint` only for the authorized provider; code mode may show bounded inlay overlays while prose defaults them off. LSP provider failure remains a bounded diagnostic, not a package/runtime crash |
| P31 | Negative: remove `render-folding` from a folding package fixture and reload, or publish an oversized/invalid package range | Package activation/publication is denied; no partial ranges or client chrome appear. If the public package fixture cannot be loaded manually, mark N/A rather than treating the automated permission/budget tests as a live pass |

## Phase 28 Linux execution record (2026-08-20)

| Checks | Result | Evidence |
|---|---|---|
| P29 | PASS structural / NOT RUN full live command path | Fresh shell captures show the package-backed connected editor states; package manifest, alias, and mode activation tests pass. Editable keyboard delivery prevented a complete live command round trip. |
| P30 | UNRESOLVED live; PASS structural | The LSP GUI worker failed to resolve the existing `lsp-shared` helper. Bridge adapter and prose/code chrome tests pass; no inlay visibility claim is made. |
| P31 | N/A live; PASS automated | No public manual package fixture exposed folding publication without permission. `folding_publish_round_trip_and_budget_deny`, decoration permission, and payload-bound tests pass. |

## Phase 28.7 P1 GUI analyzer follow-up (2026-08-21)

| Checks | Result | Evidence |
|---|---|---|
| P30 | UNRESOLVED live; PASS worker/bridge structural | The authorized Rust GUI path now resolves the shared helper, receives host-stamped session options, and carries the real tab workspace into the analyzer runtime. No `analysis.worker_failed` appeared; the first real inlay response was empty while rust-analyzer warmed up. Keyboard input was unavailable for the no-op edit and `Ctrl+Alt+I` toggle, so both retained states stay unresolved. Evidence: `code-reviews/screenshots/2026-08-20-phase28.7-followups/inlay-visible/` and `inlay-toggled-off/`. |
