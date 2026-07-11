# JavaScript Tree-sitter WASM provenance

Tier 1 native source: Cargo runtime dependency `tree-sitter-javascript = 0.25.0` compiled into Clay.

Tier 2 WASM artifact: `javascript.wasm` is not committed yet. Reproduce from the same upstream grammar release before enabling a forced WASM override:

```bash
# from upstream tree-sitter-javascript v0.25.0 checkout
npx tree-sitter build --wasm --output javascript.wasm
sha256sum javascript.wasm
```

Record final `javascript.wasm` size and SHA-256 here when committed. No network fetch, package-manager install, shell build, or native-library load occurs at Clay runtime; build is release engineering only.
