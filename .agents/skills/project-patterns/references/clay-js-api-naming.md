# Clay JS API Naming

Decision source: `decision-logs/2026-05-08-1958-clay-js-api-naming-and-package-distribution.md`.

- Distinguish four naming layers for every Clay JS API:
  - JS module specifier groups imports, e.g. `clay:editor`. The `clay:` scheme is the import-map brand and is the ONLY place the `clay` prefix is kept.
  - JS callable/export is the concise JavaScript name users call, e.g. `serverInsertText`.
  - Stable registry ID is a bare core domain plus name, e.g. `editor.serverInsertText`. Core command/API/diagnostic IDs NEVER carry a `clay.` prefix: `shell.clientSplitPaneVertical`, `editor.clientCopySelection`, `runtime.reloadConfiguration`, `documents.serverSaveDocument`. The retired `clay.<domain>.*` spelling is rejected by the doc registry and by package contribution validation.
  - `user_facing_name` is the English help/search label, e.g. `Insert Text`.
- Dotted-ID ownership rule: a dotted identifier's first segment names its owner. Core Clay IDs start with a reserved core domain (`shell`, `editor`, `documents`, `workspace`, `runtime`, `language`, `controlCenter`, `ui`, `sdui`, `modes`, `commands`, `keybindings`, `theme`, `configuration`, `packages`, `syntax`, `parse`, `decorations`, `diagnostics`, `behavior`, `completion`, …; canonical list: `RESERVED_CORE_API_DOMAINS` in `src/packages/manifest.rs`). Package-owned IDs ALWAYS start with the package's own `apiPrefix` (`<package>.<name>`, e.g. `markdown.togglePreview`, `settings.open`); third-party packages cannot claim a reserved core domain as their prefix (bundled first-party packages such as `@clay/git` are exempt via the compiled inventory).
- Package `package.json` manifest key paths keep their `clay.` prefix because they address the manifest's `clay` metadata object, not an API: `clay.apiPrefix`, `clay.contributions.*`, `clay.editorControl.modes`, `clay.performance.*`, `clay.extensionPoints`.
- Keep Clay-owned callable exports flat, concise, lower camel case, and behavior-oriented. Prefer what the API does over Rust module names, op names, protocol plumbing, or implementation details.
- Do not repeat `clay`, the broad module/domain, or implementation category in callable names when the import/module context already provides it. Reject names like `clayEditorInsertText`, `editorInsertText` from `clay:editor`, `opClayEditorInsertText`, `rustInsertText`, and `performInsertTextOperation` unless an exception is documented.
- For Clay-owned editor-core APIs, make server/client authority visible in callable names when the API touches document state, UI state, or behavior state:
  - `server*` means the call requests or mutates server-authoritative state, e.g. `serverInsertText`, `serverOpenDocument`.
  - `client*` means client-local or client-executed behavior/state, usually transient UI or behavior-manifest effects, e.g. `clientSetCursorStyle`, `clientScrollTo`.
  - `client*` does not mean arbitrary JavaScript runs in the Rust client.
- Keep module names domain-based by default, such as `clay:editor`, `clay:documents`, `clay:keybindings`, `clay:configuration`, and `clay:behavior`. Do not split modules by authority until API density or ergonomics require a later decision.
- Use `user_facing_name` for natural-language command/search/help labels so callable names can stay concise and do not need to read like sentences.
- Keep raw `Deno.core.ops.op_*`, Rust paths, generated registry IDs, and protocol implementation names out of user-facing callable exports.
- Pure-JS package APIs must begin with the package name or registered package prefix so provenance is obvious, e.g. `vimEnableMode` or `gitBlameShowInline`. The prefix should be declared in Clay package metadata and used consistently.
- Exceptions require a documented reason in the API Markdown, such as external protocol compatibility, industry-standard naming, migration compatibility, or unavoidable ambiguity.

## Examples

```ts
import { serverInsertText, clientSetCursorStyle } from "clay:editor";

await serverInsertText({ documentId, offset, text: "hello" });
clientSetCursorStyle({ color: "#ffcc00", blinking: true, type: "bar" });
```

```text
JS module: clay:editor
JS export: serverInsertText
Stable ID: editor.serverInsertText
User-facing name: Insert Text
```

```ts
// Package API; `vim` is the registered package prefix.
vimEnableMode({ mode: "normal" });
```
