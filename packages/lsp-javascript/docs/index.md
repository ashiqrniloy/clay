# `@clay/lsp-javascript`

Opt-in `typescript-language-server` intelligence for documents already using the `javascript` mode from `@clay/javascript`.

## Setup

Install host prerequisites:

```bash
npm install -g typescript-language-server typescript@5.9.3
```

TypeScript must remain on a `tsserver.js`-compatible release. TypeScript 7.x currently removes that backend and fails server start. Prefer a workspace `typescript` dependency, or keep the compatible global install next to `typescript-language-server` so the server's bundled resolver can find it. Packages never invent Clay APIs from SDK/plugin resolution.

Authorize the fixed contribution before first package load, then load base and bridge packages:

```js
import { authorizeLanguageServer } from "clay:language-server";
import { loadPackage } from "clay:packages";

await authorizeLanguageServer({
  package: "@clay/lsp-javascript",
  contribution: "lsp-javascript.server",
  workspaceRootIds: [1],
});
await loadPackage("@clay/javascript");
await loadPackage("@clay/lsp-javascript");
```

Package load never grants or installs the language server. Authorization binds exact package provenance, contribution, current directory roots, canonical `typescript-language-server` executable, literal `--stdio`, and inherited `HOME`/`PATH` names.

Preferred multi-language setup authorizes every desired bridge before any `loadPackage`, then loads base and bridge packages with one-line calls. See `tests/fixtures/configuration/lsp-language-packages/init.js`. Empty `init.js` loads no language package and starts no language server.

## Behavior

Bridge starts lazily for first eligible `.js`/`.jsx`/`.mjs`/`.cjs` document. It negotiates rather than assumes server capabilities, uses the selected UTF-8/UTF-16/UTF-32 position encoding (UTF-16 default when omitted), and synchronizes accepted open/change/reset/close versions. Current `typescript-language-server` support maps:

- full semantic tokens to priority-100 semantic decorations;
- pushed diagnostics to source-keyed `lsp-javascript` diagnostics;
- completion and snippets to priority-100, non-exclusive Clay completion;
- hover, open-document definition, inert code-action titles, and signature help to existing language-intelligence results.

Diagnostic composition is generic: each package/source/document/version replaces its own chunk. Current LSP error/warning spans suppress overlapping Tree-sitter recovery diagnostics only; unrelated Tree-sitter spans and LSP info remain additive. Empty source publication clears only this bridge's diagnostics.

Completion stays priority 100 and non-exclusive so `@clay/javascript` keywords remain merged. Use `serverDisableCompletion({ provider: "lsp-javascript.completion" })` or a base provider id/prefix when you want an explicit override.

Pull diagnostics and semantic-token deltas are omitted when the server does not advertise them. Completion entries carrying extra edits or commands are omitted. Mutating `WorkspaceEdit` code actions and unapproved commands are omitted. Definition targets publish only when already synchronized in this root-bound worker. Unsupported negotiated features return empty results rather than fabricated data.

Initialization keeps `disableAutomaticTypingAcquisition: true`. Optional `tsserver.path` may be supplied by tests or an operator-controlled factory; package code cannot choose executable, argv, cwd, environment values, or roots.

Semantic requests and notification-driven diagnostics run in bounded document-analysis workers after canonical edit acceptance; local text application and paint never wait for package JavaScript or the child process. Documents above 256 KiB, queue/session/worker exhaustion, malformed/oversize frames, stale versions, timeout, child exit, revocation, reload, or shutdown fail closed and retain `@clay/javascript` Tier 1 syntax, behavior, commands, keyword completion, and status UI.

## Duplicate process cost

`@clay/lsp-typescript` and `@clay/lsp-javascript` intentionally keep separate package identities and grants. Under the current one-session-per-package/contribution/root rule, authorizing and loading both packages for the same root can start two `typescript-language-server` processes. That duplicate cost is explicit, measured in smoke/performance work, and not hidden behind a silent shared child.

## Security and containment

`language-server` is deny-by-default and composes with `parse-document`; `render-decorations` and `completion-provider` remain separately validated. Package code receives only server-stamped document/root identity, bounded canonical text, and accepted byte deltas. It exposes no process/stdio handle, raw op, filesystem API, network API, shell, client JavaScript, direct edit application, formatting, rename, import-management, or build/test authority.

`typescript-language-server` is a trusted same-user subprocess, not a sandbox. Workspace-root approval constrains Clay routing and audit identity; it does not provide operating-system filesystem, network, or process confinement.

## Troubleshooting

- `executable_not_found`: install `typescript-language-server` onto Clay's launch `PATH`.
- `Could not find a valid TypeScript installation`: install workspace `typescript`, or keep compatible global `typescript@5.9.3` resolvable beside the language server. TypeScript 7.x is currently unsupported by this server.
- `authorization_sealed`: move `authorizeLanguageServer` before every `loadPackage` call.
- No LSP output: verify directory root ID is current and document uses `javascript` mode. Base package remains active while bridge is unavailable.

## Implementation and tests

- Policy/load: `packages/lsp-javascript/dist/{index,load,server}.js`
- Shared TypeScript-language-server policy: inventory specifier `lsp-shared/typescript-language-server.js`
- Deterministic package suite: `node --test packages/lsp-javascript/javascript-package.test.mjs`
- Cargo ownership/freshness suite: `cargo test --test lsp_bridge`
- Real workspace fixture: `tests/fixtures/lsp/javascript/`

Protocol framing, JSON-RPC, capability policy, synchronization, URI/position conversion, and server method names stay package-side. Clay Rust core remains analyzer-neutral.
