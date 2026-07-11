# Markdown grammar artifact

`markdown.wasm` is the optional Tier 2 Tree-sitter WebAssembly grammar for Markdown (`tree-sitter-md-025`). Clay's default Tier 1 path uses the compiled-in Rust crate; forced Tier 2 WASM must use a package-root-confined artifact and the query in `../queries/highlights.scm`.

Clay never fetches, builds, or shells out for grammar artifacts at runtime.
