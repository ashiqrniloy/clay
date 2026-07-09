// @clay/theme-gruvbox-material-dark — Gruvbox Material (dark, medium) theme for Clay.
//
// This theme is pure inert style data: its overrides live in package.json under
// `clay.contributions.textStyles` and are parsed by Clay at load. There is no
// executable surface here. The `entry`/`loadEntry` modules are no-ops provided to
// satisfy the package manifest contract; Clay resolves the theme's `textStyles`
// into the single source of color (`StyleRegistry`) only when the user selects
// this theme via `setTheme("@clay/theme-gruvbox-material-dark")` in init.js.
export {};