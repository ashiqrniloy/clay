# Package UI Layout and Clay Shell

Decision source: `decision-logs/2026-06-09-1431-clay-owned-shell-layout-and-package-ui-contribution-model.md`.

- Clay owns the package-facing application shell, working area, pane/split tree, fixed pane slots, component catalog, style/theme token model, and action routing contract.
- Packages declare inert, validated UI/layout/input/action/data/style contributions; they must not directly create Masonry widgets, mutate native layout, provide raw CSS, run client-side JavaScript, or call raw `Deno.core.ops`.
- The Rust client renders package UI through Clay-owned Masonry widgets/native paths. Masonry is the implementation substrate, not the package author API.
- Long-term UI structure is: working area -> pane/split tree -> pane/window layout -> mandatory `main` container plus optional `left`, `right`, `top`, and `bottom` slots -> Clay components.
- Panels can be fixed (participate in layout) or transient (overlay/dismissible); packages may request defaults, but Clay validates composition and users may override through documented configuration APIs.
- Styling must use centralized Clay theme tokens, typed component style variables, and semantic package tokens that Clay maps to native properties/render styles. Raw CSS and renderer callbacks are not package-facing APIs.
- Plans that modify packages, modes, SDUI, layout, styling, input routing, or package configuration must keep `docs/reference/packages/creating-packages.md` current with implemented APIs, examples, limitations, tests, and migration notes.
- Markdown and future modes must consume these generic shell/layout primitives rather than adding mode-specific Rust layout branches or fixture-only side panels.
