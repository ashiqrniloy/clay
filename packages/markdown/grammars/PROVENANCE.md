# Markdown Tree-sitter WASM provenance

Tier 1 native source: Cargo runtime dependency `tree-sitter-md-025 = 0.5.6` compiled into Clay.

Tier 2 WASM artifact: `markdown.wasm` is not committed yet. Reproduce from the same upstream grammar release before enabling a forced WASM override:

```bash
# from upstream tree-sitter-md-025 0.5.6 / tree-sitter-markdown checkout
npx tree-sitter build --wasm --output markdown.wasm
sha256sum markdown.wasm
```

Record final `markdown.wasm` size and SHA-256 here when committed. No network fetch, package-manager install, shell build, or native-library load occurs at Clay runtime; build is release engineering only.
