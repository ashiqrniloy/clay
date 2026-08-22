// ============================================================================
// Clay canonical example configuration — examples/packages/first-party.js
// ============================================================================
//
// First-party package configuration: language-server grants + explicit
// @clay/* package loads. Loaded from examples/init.js (section 11) via:
//
//   await loadConfigurationModule({ path: "./packages/first-party.js", optional: true });
//
// Because the load is optional, this module may be broken or deleted without
// affecting the base configuration or app launch — a configuration.module_failed
// diagnostic records the failure. Fix this file and reload (Ctrl+Shift+R or
// auto-reload) to recover.
//
// Ground rules:
//   - authorizeLanguageServer grants MUST happen BEFORE the first
//     loadPackage call in this module — that call seals authority for the
//     generation. Keep this file's ordering: grants first, loads second.
//   - Grants fail closed on environmental problems (missing executable,
//     no matching workspace root): each grant degrades independently, and
//     only that language server stays inactive until tooling exists.

// ----------------------------------------------------------------------------
// Language-server grants — clay:language-server
// ----------------------------------------------------------------------------
// Configuration-only grant API. Binds exact package provenance, contribution
// fingerprint, resolved executable, inherited env names, and workspace root
// ids. Grants start NO process; sessions start later via
// startLanguageServerSession with a matching grant.
// workspaceRootIds refer to the workspace roots open in the session
// (1-based, as listed by the workspace state).
//
import { authorizeLanguageServer } from "clay:language-server";

async function grantLanguageServer(options) {
  try {
    await authorizeLanguageServer(options);
  } catch {
    // Tooling not installed (or root absent) — skip this server only.
  }
}

await grantLanguageServer({
  package: "@clay/lsp-rust",
  contribution: "lsp-rust.server",
  workspaceRootIds: [1],
});

await grantLanguageServer({
  package: "@clay/lsp-typescript",
  contribution: "lsp-typescript.server",
  workspaceRootIds: [1],
});

await grantLanguageServer({
  package: "@clay/lsp-javascript",
  contribution: "lsp-javascript.server",
  workspaceRootIds: [1],
});

await grantLanguageServer({
  package: "@clay/lsp-markdown",
  contribution: "lsp-markdown.server",
  workspaceRootIds: [1],
});

// ----------------------------------------------------------------------------
// First-party packages — clay:packages
// ----------------------------------------------------------------------------
// Explicit opt-in loading of bundled @clay/* packages. One line per package.
// loadPackage imports the package's load entry and runs it with host-stamped
// provenance. There is NO auto-loading: what you don't load here is inactive.
//
// Available first-party specifiers:
//   Grammar/mode packages:  @clay/markdown  @clay/rust  @clay/typescript
//                           @clay/javascript
//   LSP bridge packages:    @clay/lsp-rust  @clay/lsp-typescript
//                           @clay/lsp-javascript  @clay/lsp-markdown
//                           (authorize first, above; load grammar
//                            packages before their LSP bridges)
//   Chat landing:           @clay/chat
//   Settings UI:            @clay/settings
//   Themes:                 @clay/theme-gruvbox-material-dark
//                           @clay/theme-gruvbox-material-light
//                           @clay/theme-modus-operandi
//                           @clay/theme-modus-vivendi
//   Git read-only panel:    @clay/git
//
// serverListFirstPartyPackageSpecifiers() returns this list at runtime.
import { loadPackage } from "clay:packages";

await loadPackage("@clay/markdown");   // prose mode + parser + prose movement
await loadPackage("@clay/rust");       // code mode + tree-sitter grammar
await loadPackage("@clay/typescript");
await loadPackage("@clay/javascript");
await loadPackage("@clay/chat");       // empty-tab Chat landing
await loadPackage("@clay/settings");   // settings panel (theme/appearance UI)
await loadPackage("@clay/lsp-rust");        // after the grant above
await loadPackage("@clay/lsp-typescript");
await loadPackage("@clay/lsp-javascript");
await loadPackage("@clay/lsp-markdown");
// await loadPackage("@clay/git");
