# Rust Tree-sitter WASM provenance

Tier 1 native source: Cargo runtime dependency `tree-sitter-rust = 0.24.2` compiled into Clay.

Tier 2 WASM artifact: `rust.wasm` is not committed yet. Reproduce from the same upstream grammar release before enabling a forced WASM override:

```bash
# from upstream tree-sitter-rust v0.24.2 checkout
npx tree-sitter build --wasm --output rust.wasm
sha256sum rust.wasm
```

Record final `rust.wasm` size and SHA-256 here when committed. No network fetch, package-manager install, shell build, or native-library load occurs at Clay runtime; build is release engineering only.
