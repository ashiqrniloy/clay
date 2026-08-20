# `@clay/lsp-rust`

Opt-in rust-analyzer intelligence for documents already using the `rust` mode from `@clay/rust`.

## Setup

Install host prerequisites:

```bash
rustup component add rust-analyzer rust-src --toolchain stable
```

Authorize fixed contribution before first package load, then load base and bridge packages:

```js
import { authorizeLanguageServer } from "clay:language-server";
import { loadPackage } from "clay:packages";

await authorizeLanguageServer({
  package: "@clay/lsp-rust",
  contribution: "lsp-rust.server",
  workspaceRootIds: [1],
});
await loadPackage("@clay/rust");
await loadPackage("@clay/lsp-rust");
```

Package load never grants or installs rust-analyzer. Authorization is accepted only before first `loadPackage` call and binds exact package provenance, contribution, current directory roots, canonical `rustup` executable, literal `run stable rust-analyzer` arguments, and inherited `HOME`/`PATH` names.

Preferred multi-language setup authorizes every desired bridge before any `loadPackage`, then loads base and bridge packages with one-line calls. See `tests/fixtures/configuration/lsp-language-packages/init.js`. Empty `init.js` loads no language package and starts no language server.

## Behavior

Bridge starts lazily for first eligible `.rs` document. It negotiates rather than assumes server capabilities, uses rust-analyzer's selected UTF-8/UTF-16/UTF-32 position encoding, and synchronizes accepted open/change/reset/close versions. Current rust-analyzer support maps:

- full and delta semantic tokens to priority-100 semantic decorations;
- pull or pushed diagnostics to source-keyed `rust-analyzer` diagnostics;
- completion and snippets to priority-100, non-exclusive Clay completion;
- hover, open-document definition, inert code-action titles, and signature help to existing language-intelligence results.

Diagnostic composition is generic: each package/source/document/version replaces its own chunk. Current LSP error/warning spans suppress overlapping Tree-sitter recovery diagnostics only; unrelated Tree-sitter spans and LSP info remain additive. Empty source publication clears only this bridge's diagnostics.

Completion stays priority 100 and non-exclusive so `@clay/rust` keywords/snippets remain merged. Use `serverDisableCompletion({ provider: "lsp-rust.completion" })` or a base provider id/prefix when you want an explicit override.

Completion entries carrying extra edits or commands are omitted. Mutating `WorkspaceEdit` code actions and unapproved commands are omitted. Definition targets are published only when their target document is already synchronized in this root-bound worker; external schemes, out-of-root paths, and unopened targets remain inert. Unsupported negotiated features return empty results rather than fabricated data.

Semantic/diagnostic requests run in bounded document-analysis workers after canonical edit acceptance; local text application and paint never wait for package JavaScript or rust-analyzer. Documents above 256 KiB, queue/session/worker exhaustion, malformed/oversize frames, stale versions, timeout, child exit, revocation, reload, or shutdown fail closed and retain `@clay/rust` Tier 1 syntax, behavior, commands, keyword/snippet completion, and status UI.

## Security and containment

`language-server` is deny-by-default and composes with `parse-document`; `render-decorations` and `completion-provider` remain separately validated. Package code receives only server-stamped document/root identity, bounded canonical text, and accepted byte deltas. It cannot choose executable, argv, cwd, environment values, or roots, and exposes no process/stdio handle, raw op, filesystem API, network API, shell, client JavaScript, direct edit application, formatting, rename, import-management, or build/test authority.

rust-analyzer is a trusted same-user subprocess, not a sandbox. Workspace-root approval constrains Clay routing and audit identity; it does not provide operating-system filesystem, network, or process confinement.

## Troubleshooting

- `executable_not_found`: run install command above and ensure `rustup` is on Clay's launch `PATH`.
- Missing standard-library analysis: install `rust-src` for stable toolchain.
- `authorization_sealed`: move `authorizeLanguageServer` before every `loadPackage` call.
- No LSP output: verify directory root ID is current and document uses `rust` mode. Base package remains active while bridge is unavailable.
- Toolchain project override does not change fixed launch contract: Clay starts `rustup run stable rust-analyzer` by design.

## Implementation and tests

- Policy/load: `packages/lsp-rust/dist/{index,load,server}.js`
- Shared protocol modules: inventory specifiers `lsp-shared/client.js` and `lsp-shared/mapping.js`
- Deterministic package suite: `node --test packages/lsp-rust/rust-package.test.mjs`
- Cargo ownership/freshness suite: `cargo test --test lsp_bridge`
- Real workspace fixture: `tests/fixtures/lsp/rust/`

Protocol framing, JSON-RPC, capability policy, synchronization, URI/position conversion, and rust-analyzer method names stay package-side. Clay Rust core remains analyzer-neutral.
