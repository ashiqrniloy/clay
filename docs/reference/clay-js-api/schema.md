# Clay JS API Markdown Schema

Clay JS API documentation is authored as Markdown with YAML frontmatter. These files are the source of truth for generated documentation registries and future app/help/agent lookup APIs.

## Scope

Use this schema for every public Clay JavaScript/TypeScript API facade, including future editor commands, protocol helpers, behavior manifest helpers, permission/capability APIs, extension APIs, AI tools, SDUI helpers, and file/workspace operations.

Do not use this schema to expose raw Rust functions or raw `Deno.core.ops.op_*` calls as public APIs. Raw Rust functions and ops are implementation details behind stable Clay JS facades.

## Naming Convention

Every Clay JS API distinguishes four naming layers:

- `js_module`: the import context, such as `clay:editor`.
- `js_export` / `name`: the concise callable JavaScript export, such as `serverInsertText`.
- `id`: the globally namespaced stable registry identifier, such as `clay.editor.serverInsertText`.
- `user_facing_name`: the English help/search label, such as `Insert Text`.

Clay-owned callable exports should be flat, lower camel case, behavior-oriented names. Do not repeat `clay`, the module/domain name, raw Rust paths, or raw `op_*` names in callable exports when the surrounding module and registry metadata already provide that context. For Clay-owned editor-core APIs, include `server*` or `client*` authority prefixes when the API touches document state, UI state, or behavior state. Pure-JS package APIs must begin with the package name or registered package prefix. See `.agents/skills/project-patterns/references/clay-js-api-naming.md` for the reusable project pattern.

## Master Index Requirement

Every Clay JS API Markdown file that participates in registry generation must be linked from `docs/index.md` under **Clay JS API Registry Source Files**. The generated registry must use that section as the explicit inclusion list.

## Required Frontmatter Fields

| Field | Type | Required | Description |
| --- | --- | --- | --- |
| `id` | string | yes | Stable lookup ID, using dotted namespace form such as `clay.editor.serverInsertText`. |
| `kind` | string enum | yes | Documentation surface kind. Use `clay-js-api` for Clay JS API pages. |
| `js_module` | string | yes | Public Clay JS module specifier, such as `clay:editor`. |
| `js_export` | string | yes | Named export exposed by the JS/TS facade, such as `serverInsertText`. |
| `js_facade` | path | yes | Project-relative JS/TS facade source path that exports the API. |
| `backing_rust` | path symbol | yes | Project-relative Rust function path backing the API, including symbol name. |
| `deno_op` | string | yes | `deno_core` op wrapper name, such as `op_clay_editor_insert_text`. |
| `deno_op_path` | path symbol | yes | Project-relative Rust op wrapper path, including symbol name. |
| `name` | string | yes | Stable API name. Usually the JS export/callable name. |
| `user_facing_name` | string | yes | Searchable user-facing command/function name shown in help, command search, configuration UIs, and agent discovery. |
| `summary` | string | yes | One-sentence description used in indexes and lookup results. |
| `owner` | string enum | yes | Owning component, such as `server`, `client`, `editor`, `extension-runtime`, or `ai-runtime`. |
| `phase` | string | yes | Roadmap phase that introduced or owns the API contract. |
| `visibility` | string enum | yes | `public`, `experimental`, `internal`, or `deprecated`. Registry generation should include only intended public/experimental surfaces unless configured otherwise. |
| `permissions` | string list | yes | Required permissions/capabilities, or an empty list for APIs with no additional authority. |
| `key_bindings` | string list | yes | Default key bindings that invoke this API, or an empty list when no default key binding exists. Users may map bindings to this API through configuration. |
| `custom_properties` | object list | yes | User-configurable properties that can change API behavior. Use an empty list when the API has no behavior-changing properties. |
| `security` | string | yes | Security and authority notes. This records requirements only; it does not grant authority. |
| `agent_guidance` | string | yes | Guidance for AI agents on when to use or avoid the API. |
| `lookup_tags` | string list | yes | Tags for app/help/agent discovery. Must not be empty for public APIs. |
| `app_visible` | boolean | yes | Whether app surfaces may show this API in user-facing help/search. |
| `help_visible` | boolean | yes | Whether this API appears in generated help/reference listings. |
| `stability` | string | yes | Stability state such as `planned`, `experimental`, `stable`, or `deprecated`. |
| `async` | boolean | yes | Whether the JavaScript API returns a promise or otherwise completes asynchronously. |

Required string fields must be non-empty after trimming. Required list fields must be present; `lookup_tags` must contain at least one tag for public APIs. `key_bindings` and `custom_properties` may be empty but must be present so users and agents can reliably discover whether defaults or configurable behavior exist. Boolean fields must be literal booleans, not strings.

## Required Markdown Body Sections

Each API page must contain these headings in this order unless a later parser deliberately relaxes ordering:

1. `# <API name>`
2. `## Summary`
3. `## Description`
4. `## When to use`
5. `## JavaScript usage`
6. `## Example`
7. `## Options`
8. `## Key bindings`
9. `## Custom properties`
10. `## Return and async behavior`
11. `## Errors`
12. `## Permissions and security`
13. `## Agent guidance`
14. `## Backing implementation`
15. `## Lookup metadata`

The body should explain what the API does, why and when to use it, configuration/options/defaults/allowed values, key bindings, user-configurable custom properties, return values, async behavior, errors/failure modes, permissions, authority boundaries, and implementation links.

## Example API Document

````markdown
---
id: clay.editor.serverInsertText
kind: clay-js-api
js_module: clay:editor
js_export: serverInsertText
js_facade: runtime/js/editor.ts::serverInsertText
backing_rust: src/server/editor.rs::insert_text
deno_op: op_clay_editor_insert_text
deno_op_path: src/server/ops/editor.rs::op_clay_editor_insert_text
name: serverInsertText
user_facing_name: Insert Text
summary: Insert inert text through the server-authoritative edit path.
owner: server
phase: Phase 3
visibility: public
permissions: [document-edit]
key_bindings: []
custom_properties:
  - name: normalize_line_endings
    type: boolean
    default: true
    description: Whether inserted text should be normalized to Clay's document line-ending convention.
security: Requires document edit authority; does not grant filesystem, network, shell, extension, or AI authority.
agent_guidance: Use when a script needs to request authoritative text insertion for a document it may edit.
lookup_tags: [editor, js-api, text, mutation]
app_visible: true
help_visible: true
stability: planned
async: true
---

# serverInsertText

## Summary

Insert inert text through the server-authoritative edit path.

## Description

`serverInsertText` requests a text insertion in a document using Clay's documented JavaScript facade rather than raw ops or Rust internals.

## When to use

Use this API when an extension, command, or AI tool needs to request text insertion for a document where it already has edit authority.

## JavaScript usage

```ts
import { serverInsertText } from "clay:editor";

await serverInsertText({ documentId, offset, text: "hello" });
```

## Example

```ts
await serverInsertText({
  documentId: "current",
  offset: 0,
  text: "hello\n",
});
```

## Options

- `documentId`: Target document identifier.
- `offset`: Protocol-defined insertion offset.
- `text`: Inert text to insert.

## Key bindings

No default key binding is assigned. Users may bind a key to `clay.editor.serverInsertText` in `~/.config/clay/init.js`.

## Custom properties

- `normalize_line_endings` (`boolean`, default `true`): normalizes inserted text to Clay's document line-ending convention before submitting the edit.

## Return and async behavior

Returns a promise that resolves after the server accepts or rejects the edit request.

## Errors

Fails when the document is missing, the caller lacks edit authority, the offset is invalid, or the edit conflicts with document version rules.

## Permissions and security

Requires document edit authority. This API does not grant filesystem, network, shell, extension loading, or AI mutation authority.

## Agent guidance

Prefer this API over lower-level protocol or op access. Do not use it for file IO, shell commands, or network effects.

## Backing implementation

- JS facade: `runtime/js/editor.ts::serverInsertText`
- Deno op: `src/server/ops/editor.rs::op_clay_editor_insert_text`
- Rust function: `src/server/editor.rs::insert_text`

## Lookup metadata

- Stable ID: `clay.editor.serverInsertText`
- User-facing name: Insert Text
- Kind: `clay-js-api`
- Default key bindings: none
- Custom properties: `normalize_line_endings`
- Tags: `editor`, `js-api`, `text`, `mutation`
````

## Parser and Registry Expectations

- Parsing and registry generation are offline developer/test operations.
- Registry generation must not run in the editor paint/input path.
- Generated entries should be ordered deterministically by stable `id`.
- Validation must reject missing required fields, empty required strings, duplicate IDs, missing master-index links, missing lookup tags for public APIs, malformed key binding/custom property metadata, and malformed generated entries.
- The checked-in generated registry is derived output. Tests should report staleness and instruct developers to run `cargo run --bin update-doc-registry`; tests must not silently mutate generated files.

## Security Boundary

Schema metadata records authority requirements and security notes. It does not grant permissions, execute scripts, load extensions, inspect user files, access the network, or expose runtime user content.
