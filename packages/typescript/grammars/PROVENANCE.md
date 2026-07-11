# TypeScript Tree-sitter WASM provenance

Tier 1 native source: Cargo runtime dependency `tree-sitter-typescript = 0.23.2` compiled into Clay for TypeScript and TSX.

Tier 2 WASM artifact: `typescript.wasm` is not committed yet. Reproduce from the same upstream grammar release before enabling a forced WASM override:

```bash
# from upstream tree-sitter-typescript v0.23.2 checkout
npx tree-sitter build --wasm --scope typescript --output typescript.wasm
sha256sum typescript.wasm
```

Record final `typescript.wasm` size and SHA-256 here when committed. No network fetch, package-manager install, shell build, or native-library load occurs at Clay runtime; build is release engineering only.
