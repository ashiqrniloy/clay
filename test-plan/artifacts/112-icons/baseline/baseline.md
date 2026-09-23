# Plan 112 Baseline Evidence (Task 1)

Date: 2026-09-07 · Host: Linux · Branch: `enhancement/UI` · HEAD: `1b65568 WIP`

## Worktree ownership
- `git status --porcelain`: exactly one entry — untracked `plans/112-Configurable-Icon-Packs-and-UI-Integration.md` (this plan). No modified tracked files. No release artifacts touched.

## Toolchain
- rustc/cargo 1.96.1 (31fca3adb) · node v24.19.0 · npm 11.17.0

## Blocking gates (all pass, no inherited failures)
| Gate | Command | Result |
|---|---|---|
| fmt | `cargo fmt --check` | exit 0 |
| check | `cargo check --all-targets` | exit 0 (10.05s; src-tauri is workspace member, covered) |
| clippy | `cargo clippy --all-targets -- -D warnings` | exit 0 |
| typecheck | `npm --prefix frontend run typecheck` | exit 0 |
| tests | `npm --prefix frontend test` | exit 0 — 35 files, 252 tests passed (8.49s) |
| build | `npm --prefix frontend run build` | exit 0 (3.29s) |
| budget | `npm --prefix frontend run check:budget` | exit 0 — shell gzip 155.3/180 kB, total gzip 372.3/400 kB |

Rust `cargo test` is not a Task 1 gate (deferred to Task 10 full verification); no inherited failure is known.

## Frontend bundle baseline (gzip, from build + budget)
- `index` shell: 471.54 kB raw / 146.1 kB gzip (budget 180 kB)
- `codemirror`: 361.65 kB raw / 117.8 kB gzip — CodeMirror is a separate chunk; typing/render hot path is CodeMirror-local and does not route through React rerenders or icon state
- `controls`: 59.82 kB raw / 20.3 kB gzip · `WorkspacePanes` 62.73/21.67 · `chat-agent-core` 146.79/38.26 · `CodingAgentPanel` 18.08/6.10 · `PackageWorkspace` 7.94/3.08
- No typing/scroll benchmark harness exists beyond `frontend/src/editor/performance.test.ts` and the bundle budget; Task 10 compares against the figures above.

## Shipped renderer
- Tauri v2 + React client (`src-tauri/` workspace member, `frontend/`), Rust server authoritative. Native Masonry renderer is retired; not a baseline target.

## Package inventory (19 first-party)
chat, coding-agent, design-glass, design-neobrutal, git, javascript, lsp-javascript, lsp-markdown, lsp-rust, lsp-shared, lsp-typescript, markdown, rust, settings, theme-gruvbox-material-dark, theme-gruvbox-material-light, theme-modus-operandi, theme-modus-vivendi, typescript. No icon pack exists yet.

## Migration inventory — ClayButton consumers (graft grep: 109 hits, 32 symbols, 15 files)
button.tsx, index.ts, tab-strip.tsx, ClayEditor.tsx, ChatPanel.tsx, CodingAgentPanel.tsx, SettingsPanel.tsx, PaneTree.tsx, WorkspacePanes.tsx, sdui/renderer.tsx, sdui/registry.tsx, routes/fixture.tsx, routes/workspace.tsx, test/components.test.tsx, test/design-system-conformance.test.tsx

## Migration inventory — both SDUI representations
- Native SDUI tree: `src/protocol/sdui.rs` (list items: text/detail/action, no icon field), painted via `src/masonry_sdui*` (retired path) — active path is React registry.
- Package component JSON → DTO → React: `src-tauri/src/bridge/dto.rs` → `frontend/src/sdui/types.ts` → `frontend/src/sdui/registry.tsx` / `renderer.tsx`.
- First-party package UI producers: `packages/git/dist/status.js` (defineLabel nodes: root/head/dirty/refresh), `packages/markdown/dist/sdui.js` (defineButton "Toggle Preview"), `packages/chat`, `packages/coding-agent`, `packages/settings` via component JSON.

## Placement surfaces confirmed present
- `frontend/src/editor/ClayEditor.tsx` action row (Save/Reload/Close/Open)
- `frontend/src/components/tab-strip.tsx` (+/× font glyphs), `frontend/src/components/modal.tsx` close, `frontend/src/components/controls.tsx` (dropdown chevrons, text buttons)
- `frontend/src/coding-agent/CodingAgentPanel.tsx` composer/header/empty-Files/recent-sessions/Back navigation
- `src/shell/file_browser.rs` rows (folder/file/link kind text, parent nav)
- `packages/git/dist/status.js`, `packages/markdown/dist/sdui.js`

## Intentional keep-text exclusions (baseline confirmation)
Agent nav tabs (Files/Memory/Context/Session Info), permissions Allow/Deny, model/provider/effort choices, error/status text, destructive confirmations — all currently text and stay text per Placement and Label Policy.

## Typing independence (pre-capability check)
`grep -rn "icon" frontend/src` → no component/icon implementation files; no `@phosphor-icons/*`, `lucide`, or SVG imports exist in `frontend/src`. Editor typing path (`frontend/src/editor/`: create-editor, transactions, compartments, sync) has zero icon coupling; icons are React chrome that does not exist yet. Normal typing cannot depend on icon state today.

## Icons in native chrome (context only)
Correction (Task 2): `src/shell/primitives.rs` / `paint_icon_slot` were removed with the native client; chrome primitives are now React components/CSS. No React icon component, tooltip, or icon CSS variables exist in `frontend/src` yet. Recipe matrix already documents `button.icon`, `dropdown.indicator`, `collapse.chevron`, and `iconSlot` slots (token mapping: `dimension.icon.size`, `text.icon`, `opacity.disabled`) — these are the contract Tasks 7/8 consume. Rust core tokens `dimension.icon.size`/`text.icon` remain in `src/shell/theme.rs`. Bespoke `ClayTabStrip` close glyph is React internal chrome. Feeds Task 2 primitive review.
