# Package UI Layout and Clay Shell

- Before reviewing or changing package UI, SDUI, shell layout, styling, or input routing, run `npx ui-skills start`, inspect the relevant category, and load the smallest useful skill set (prefer 1, max 3); record selected slugs in the plan evidence.

Decision source: `decision-logs/2026-06-09-1431-clay-owned-shell-layout-and-package-ui-contribution-model.md`.

- Clay owns the package-facing application shell, working area, pane/split tree, fixed pane slots, component catalog, style/theme token model, and action routing contract.
- Packages declare inert, validated UI/layout/input/action/data/style contributions; they must not directly create Masonry widgets, mutate native layout, provide raw CSS, run client-side JavaScript, or call raw `Deno.core.ops`.
- The Rust client renders package UI through Clay-owned Masonry widgets/native paths. Masonry is the implementation substrate, not the package author API.
- Long-term UI structure is: working area -> pane/split tree -> pane/window layout -> mandatory `main` container plus optional `left`, `right`, `top`, and `bottom` slots -> Clay components.
- Panels can be fixed (participate in layout) or transient (overlay/dismissible); packages may request defaults, but Clay validates composition and users may override through documented configuration APIs.
- Styling must use centralized Clay theme tokens, typed component style variables, and semantic package tokens that Clay maps to native properties/render styles. Raw CSS and renderer callbacks are not package-facing APIs.
- Plans that modify packages, modes, SDUI, layout, styling, input routing, or package configuration must keep `docs/reference/packages/creating-packages.md` current with implemented APIs, examples, limitations, tests, and migration notes.
- Phase 20 multi-document sessions, dirty/save status chrome, and recovery menus are Clay-owned shell surfaces. Packages may bind documented command IDs and contribute inert UI around them; they must not own tabs, native save dialogs, clipboard-contents APIs, or reconnect/resync loops. See the Phase 20 authoring contract section in `creating-packages.md`.
- Markdown and future modes must consume these generic shell/layout primitives rather than adding mode-specific Rust layout branches or fixture-only side panels.

## Transient Menu Surfaces (Command Centre)

Decision source: `decision-logs/2026-08-11-1711-command-centre-surface-path-mode-and-sequence-keybindings.md`.

- One shared `TransientMenuSession`-based surface hosts command-palette, path-browsing, and picker sessions; add new session kinds, not new overlay systems.
- Menu sessions are server-owned and interactive via protocol round-trip (query/selection/activate/cancel intents, server-pushed snapshots); the client only renders and forwards keystrokes while a modal session is active.
- New overlay anchors extend the `TransientMenuOrigin` enum (e.g. centred Spotlight-style); backdrop effects use theme tokens (scrim), never custom render-pipeline work beyond Masonry/Vello upstream.
- Filtering is a shared Clay-owned fuzzy subsequence matcher used by all transient menus, not per-menu ad-hoc filters.
