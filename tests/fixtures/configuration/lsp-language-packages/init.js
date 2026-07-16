// Preferred Phase 18.21 end-user shape: authorize every desired language-server
// contribution before the first package load, then load each base language
// package and its LSP bridge with one-line `loadPackage` calls.
//
// Empty init.js loads nothing. Loading an `@clay/lsp-*` package without a
// matching pre-load grant fails closed. No bridge auto-installs a language
// server, and base packages remain usable when a bridge is absent or fails.
import { authorizeLanguageServer } from "clay:language-server";
import { loadPackage } from "clay:packages";

await authorizeLanguageServer({
  package: "@clay/lsp-rust",
  contribution: "lsp-rust.server",
  workspaceRootIds: [1],
});
await authorizeLanguageServer({
  package: "@clay/lsp-typescript",
  contribution: "lsp-typescript.server",
  workspaceRootIds: [1],
});
await authorizeLanguageServer({
  package: "@clay/lsp-javascript",
  contribution: "lsp-javascript.server",
  workspaceRootIds: [1],
});
await authorizeLanguageServer({
  package: "@clay/lsp-markdown",
  contribution: "lsp-markdown.server",
  workspaceRootIds: [1],
});

await loadPackage("@clay/rust");
await loadPackage("@clay/lsp-rust");
await loadPackage("@clay/typescript");
await loadPackage("@clay/lsp-typescript");
await loadPackage("@clay/javascript");
await loadPackage("@clay/lsp-javascript");
await loadPackage("@clay/markdown");
await loadPackage("@clay/lsp-markdown");
