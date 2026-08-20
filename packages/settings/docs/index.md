# @clay/settings

First-party settings UI surface (Phase 20.6).

A catalog-composed SDUI panel that lets users switch theme, appearance, and
typography from the UI. The surface is declared in `clay.contributions.ui.panels`
in `package.json` and registered by `dist/load.js` via
`serverRegisterPanelContribution`. It uses only implemented
`ComponentKind` kinds — no native chrome, no client JavaScript, no raw CSS.

## Catalog composition

| Control | Catalog kind |
|---------|-------------|
| Container | `panel` (fixed right slot, `defaultVisibility: hidden`) + `scroll` (bounded settings content) |
| Sections | `collapse` |
| Theme picker | `dropdown` (items carry `settings.setTheme`) |
| Appearance picker | `dropdown` (light / dark / system) |
| Font families / sizes / hierarchy | `textInput` (`validationState` style for out-of-bound feedback) |
| Row / section labels | `label` (`typography.title` / body) |
| Apply / Reset / Close | `button` (`primary` / `muted` / `default`) |
| Action row | `flex` (`gap: spacing.sm`) |

## Command intents

All controls emit inert `settings.*` command intents, validated by the
server-side settings command executor (`src/server/command_execution.rs`):

- `settings.open` / `settings.close` — surface visibility.
- `settings.setTheme` — `arguments.item_id` is a first-party
  `@clay/theme-*` specifier.
- `settings.setAppearance` — `arguments.item_id` is `light` | `dark` |
  `system`.
- `settings.setTypography` — font profiles / hierarchy; bounds enforced
  at apply time by the `setTypography` op.
- `settings.reset` — restore defaults.

## Precedence and live apply

Live application (persist → reload → runtime-state fanout) is wired by the
configuration-precedence task (plan 067, task 7). The dropdown/list choice
value reaches the handler as `arguments.item_id` via the SDUI action source
forwarding added in Phase 20.6.

## Activation

```js
// ~/.config/clay/init.js
import { loadPackage } from "clay:packages";
await loadPackage("@clay/settings");
```