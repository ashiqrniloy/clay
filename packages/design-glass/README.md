# @clay/design-glass

The official luminous frosted glass reference UI design system package for Clay with solid active-theme fallbacks.

## Structure
- `package.json`: Declarative manifest contributing `clay.contributions.uiDesignSystem`.
- `docs/index.md`: Full documentation, accessibility semantics, and recipe reference.

## Development & Verification
This package is data-only (contains zero executable JavaScript). It is verified via:
- `cargo test --test presentation theme_packages::`
- `cargo test --test presentation package_ui_conformance::`
- `cargo test --test security package_loading::`
