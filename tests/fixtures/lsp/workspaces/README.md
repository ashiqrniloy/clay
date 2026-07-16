# LSP workspace fixtures

Language-specific sample workspaces used by fake-server matrix coverage,
package policy tests, and environment-gated real-server smoke live next to this
directory:

- `../rust/` — minimal Cargo package for rust-analyzer
- `../typescript/` — `tsconfig.json` + TS/TSX sources for typescript-language-server
- `../javascript/` — `jsconfig.json` + JS/JSX sources for typescript-language-server
- `../markdown/` — `.marksman.toml` + linked Markdown notes for Marksman

The generic standards-shaped fake LSP server lives in `../fake-server/`.
