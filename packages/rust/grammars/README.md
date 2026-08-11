# Rust grammar artifact

`rust.wasm` is the Tree-sitter WebAssembly grammar for Rust (`tree-sitter-rust`).
It is sourced from the upstream `tree-sitter-rust` release and is first-party
only: Clay loads it as a package-root-confined `tree-sitter-wasm` artifact and
never fetches, builds, or shells out to acquire it.

Phase 18.10 validates and registers the grammar/query/style metadata at package load time through `syntax.serverRegisterSyntaxGrammar`. The actual WASM artifact remains package-root-confined and is bound only by Clay-owned server syntax code.
