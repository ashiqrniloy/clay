# Clay JS API

Sources: `decision-logs/2026-05-08-1958-clay-js-api-naming-and-package-distribution.md`, `decision-logs/2026-05-08-1509-clay-js-api-facade-for-rust-functions.md`, `decision-logs/2026-05-08-1840-clay-js-api-discovery-keybindings-custom-properties.md`, `decision-logs/2026-05-08-1419-markdown-authoritative-documentation-registry.md`.

## Naming — Four Layers

- **JS module specifier** groups imports, e.g. `clay:editor`. The `clay:` scheme is the import-map brand and the ONLY place the `clay` prefix is kept.
- **JS callable/export** is the concise JavaScript name users call, e.g. `serverInsertText`.
- **Stable registry ID** is a bare core domain plus name, e.g. `editor.serverInsertText`. Core command/API/diagnostic IDs NEVER carry a `clay.` prefix: `shell.clientSplitPaneVertical`, `editor.clientCopySelection`, `runtime.reloadConfiguration`, `documents.serverSaveDocument`. The retired `clay.<domain>.*` spelling is rejected by the doc registry and by package contribution validation.
- **`user_facing_name`** is the English help/search label, e.g. `Insert Text`.
- Dotted-ID ownership: a dotted identifier's first segment names its owner. Core IDs start with a reserved core domain (`shell`, `editor`, `documents`, `workspace`, `runtime`, `language`, `controlCenter`, `ui`, `sdui`, `modes`, `commands`, `keybindings`, `theme`, `configuration`, `packages`, `syntax`, `parse`, `decorations`, `diagnostics`, `behavior`, `completion`, …; canonical list: `RESERVED_CORE_API_DOMAINS` in `src/packages/manifest.rs`). Package-owned IDs ALWAYS start with the package's own `apiPrefix` (`<package>.<name>`, e.g. `markdown.togglePreview`, `settings.open`); third-party packages cannot claim a reserved core domain (bundled first-party packages such as `@clay/git` are exempt via the compiled inventory).
- Package `package.json` manifest key paths keep their `clay.` prefix because they address the manifest's `clay` metadata object, not an API: `clay.apiPrefix`, `clay.contributions.*`, `clay.editorControl.modes`, `clay.performance.*`, `clay.extensionPoints`.
- Callable exports: flat, concise, lower camel case, behavior-oriented (what the API does, not Rust module/op/protocol names). Do not repeat `clay`, the broad module/domain, or implementation category when the import context provides it. Reject `clayEditorInsertText`, `editorInsertText` from `clay:editor`, `opClayEditorInsertText`, `rustInsertText`, `performInsertTextOperation` unless a documented exception applies.
- Server/client authority markers for editor-core APIs touching document, UI, or behavior state: `server*` requests/mutates server-authoritative state (`serverInsertText`, `serverOpenDocument`); `client*` means client-local/client-executed behavior or transient UI/behavior-manifest effects (`clientSetCursorStyle`, `clientScrollTo`). `client*` does NOT mean arbitrary JavaScript runs in the Rust client.
- Module names are domain-based (`clay:editor`, `clay:documents`, `clay:keybindings`, `clay:configuration`, `clay:behavior`); do not split by authority until API density requires it.
- `user_facing_name` carries natural language so callable names stay concise.
- Keep raw `Deno.core.ops.op_*`, Rust paths, generated registry IDs, and protocol names out of user-facing exports.
- Pure-JS package APIs begin with the package name or registered prefix so provenance is obvious (`vimEnableMode`, `gitBlameShowInline`); declared in package metadata, used consistently.
- Exceptions need a documented reason in the API Markdown (external protocol compatibility, industry-standard naming, migration compatibility, unavoidable ambiguity).

```ts
import { serverInsertText, clientSetCursorStyle } from "clay:editor";
await serverInsertText({ documentId, offset, text: "hello" });
clientSetCursorStyle({ color: "#ffcc00", blinking: true, type: "bar" });
// JS module: clay:editor | JS export: serverInsertText | Stable ID: editor.serverInsertText | User-facing: Insert Text
vimEnableMode({ mode: "normal" }); // package prefix `vim`
```

## Boundary

- The public programmatic surface is the Clay JS/TS API, not raw Rust public functions or raw `Deno.core.ops.op_*` calls.
- Server-side Rust public functions must be exposed through explicit `deno_core` op wrappers and stable Clay JS/TS facade modules. Functions that should stay internal are private or `pub(crate)`.
- Client-side Rust functions are not directly exposed to JavaScript; JS effects on clients flow through server-authoritative APIs, protocol updates, or behavior manifests.
- Plans that add or change server-side Rust public functions must include a Clay JS API verification task identifying Rust function, op wrapper, JS facade, and docs path.
- Configuration facades that repeat the same options per call (e.g. `bindKey(chord, command, { scope })`) should offer an overloaded batch table form — one call, shared options hoisted once, `{ scope, bindings: { chord: command } }`. Batch forms validate every entry before applying any (all-or-nothing, entry-indexed diagnostics), keep single-item forms working, and add no new facade exports. Source: `decision-logs/2026-08-09-0002-bindkey-table-form-batch-bindings.md`.

## Schema Requirements

Every Clay JS API must have:

- Stable ID, JS module/export, JS facade path, backing Rust function path, `deno_core` op path/name, summary, owner, phase, visibility, permissions/security notes, agent guidance, lookup tags, app/help visibility.
- A searchable `user_facing_name` for help, command search, configuration UIs, and AI-agent discovery.
- `key_bindings` (empty list when no default binding; users may map bindings through configuration).
- `custom_properties`: every behavior-changing configurable property with type, default, allowed values when relevant, description; empty list only when none.
- Markdown under `docs/reference/clay-js-api/` plus `docs/index.md` as the authority; generated registries and lookup APIs derive from it.

## Documentation Registry Tests

Plans/implementations that add or change Clay JS APIs must include tests/acceptance criteria for a non-mutating generation/check that `cargo test` runs, plus a Cargo update command that rewrites checked-in artifacts (`cargo run --bin update-doc-registry`). `cargo test` must fail on: server-side Rust public function lacking a Clay JS API; Clay JS API lacking Markdown docs; docs missing from the master index; missing required metadata (JS usage, examples, options/configuration, user-facing name, key binding metadata, custom property metadata, authority notes, backing Rust/op/facade paths); stale checked-in registry; entries unavailable through app/help/agent lookup. Tests detect stale artifacts and print the update command; they must never silently mutate files.