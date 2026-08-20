# `@clay/lsp-markdown`

Opt-in Marksman intelligence for documents already using the `markdown` mode from `@clay/markdown`.

## Setup

Install host prerequisites:

```bash
# self-contained Marksman binary on PATH; example:
# curl -L https://github.com/artempyanykh/marksman/releases/download/.../marksman-linux-x64 -o ~/.cargo/bin/marksman
marksman --version
```

Authorize the fixed contribution before first package load, then load base and bridge packages:

```js
import { authorizeLanguageServer } from "clay:language-server";
import { loadPackage } from "clay:packages";

await authorizeLanguageServer({
  package: "@clay/lsp-markdown",
  contribution: "lsp-markdown.server",
  workspaceRootIds: [1],
});
await loadPackage("@clay/markdown");
await loadPackage("@clay/lsp-markdown");
```

Package load never grants or installs Marksman. Authorization binds exact package provenance, contribution, current directory roots, canonical `marksman` executable, literal `server` argument, and an empty inherited-environment list.

Marksman discovers Markdown workspaces more reliably when the approved root contains a project marker such as `.marksman.toml`. Without a recognizable folder, link/hover/completion quality degrades even though the child still starts.

Preferred multi-language setup authorizes every desired bridge before any `loadPackage`, then loads base and bridge packages with one-line calls. See `tests/fixtures/configuration/lsp-language-packages/init.js`. Empty `init.js` loads no language package and starts no language server.

## Behavior

Bridge starts lazily for first eligible `.md`/`.markdown`/`.mdown` document. It negotiates rather than assumes server capabilities and synchronizes accepted open/change/reset/close versions. Marksman currently advertises full-document sync (`change: 1`) rather than incremental edits; the shared adapter therefore sends full-text changes when required.

Current Marksman support maps:

- full semantic tokens (no delta) to priority-100 semantic decorations;
- pushed diagnostics (for example broken wiki links) to source-keyed `lsp-markdown` diagnostics;
- completion for wiki/heading triggers (`[`, `#`, `(`) to priority-100, non-exclusive Clay completion;
- hover, open-document definition, and inert title-only code actions to existing language-intelligence results.

Diagnostic composition is generic: each package/source/document/version replaces its own chunk. Current LSP error/warning spans suppress overlapping Tree-sitter recovery diagnostics only; unrelated Tree-sitter spans and LSP info remain additive. Empty source publication clears only this bridge's diagnostics.

Completion stays priority 100 and non-exclusive so `@clay/markdown` base completion remains merged. Use `serverDisableCompletion({ provider: "lsp-markdown.completion" })` or a base provider id/prefix when you want an explicit override.

Signature help and pull diagnostics are not advertised; capability-negotiated absence remains a normal empty result. Completion entries carrying extra edits or commands are omitted. Mutating `WorkspaceEdit` code actions (including Marksman's table-of-contents edit) and unapproved commands are omitted. Definition targets publish only when already synchronized in this root-bound worker; external schemes, out-of-root paths, and unopened targets remain inert.

Semantic requests and notification-driven diagnostics run in bounded document-analysis workers after canonical edit acceptance; local text application, Markdown preview, Tier 1 Tree-sitter decorations, and paint never wait for package JavaScript or Marksman. Bridge outputs do not erase preview or base Markdown syntax. Documents above 256 KiB, queue/session/worker exhaustion, malformed/oversize frames, stale versions, timeout, child exit, revocation, reload, or shutdown fail closed and retain `@clay/markdown` editing, preview, syntax, behavior, commands, and base completion.

## Security and containment

`language-server` is deny-by-default and composes with `parse-document`; `render-decorations` and `completion-provider` remain separately validated. Package code receives only server-stamped document/root identity, bounded canonical text, and accepted byte deltas. It cannot choose executable, argv, cwd, environment values, or roots, and exposes no process/stdio handle, raw op, filesystem API, network API, shell, client JavaScript, direct edit application, formatting, rename, import-management, or build/test authority.

Marksman is a trusted same-user subprocess, not a sandbox. Workspace-root approval constrains Clay routing and audit identity; it does not provide operating-system filesystem, network, or process confinement.

## Troubleshooting

- `executable_not_found`: install Marksman onto Clay's launch `PATH`.
- `authorization_sealed`: move `authorizeLanguageServer` before every `loadPackage` call.
- Empty hover/definition/completion: add `.marksman.toml` (or another Marksman project marker) under the authorized root and reopen Markdown documents.
- No LSP output: verify directory root ID is current and document uses `markdown` mode. Preview and Tier 1 syntax remain active while the bridge is unavailable.

## Implementation and tests

- Policy/load: `packages/lsp-markdown/dist/{index,load,server}.js`
- Shared protocol modules: inventory specifiers `lsp-shared/client.js` and `lsp-shared/mapping.js`
- Deterministic package suite: `node --test packages/lsp-markdown/markdown-package.test.mjs`
- Cargo ownership/freshness suite: `cargo test --test lsp_bridge`
- Real workspace fixture: `tests/fixtures/lsp/markdown/`

Protocol framing, JSON-RPC, capability policy, synchronization, URI/position conversion, and Marksman method names stay package-side. Clay Rust core remains analyzer-neutral.
