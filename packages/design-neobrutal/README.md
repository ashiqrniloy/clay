# @clay/design-neobrutal

The official restrained utilitarian Neobrutal default UI design system package for Clay.

## Structure
- `package.json`: Declarative manifest contributing `clay.contributions.uiDesignSystem`.
- `docs/index.md`: Full documentation, accessibility semantics, and recipe reference.

## Development & Verification
This package is data-only (contains zero executable JavaScript). It is verified via:
- `cargo test --test presentation package_ui_conformance::`
- `cargo test --test security package_loading::`
