# Clay Documentation Index

This is the master Markdown index for Clay's public, programmatic documentation. Markdown files linked from the registry source section are the authoritative source for generated app/help/agent documentation registries.

## Documentation Contract

- [Clay JS API Markdown Schema](reference/clay-js-api/schema.md) — required frontmatter and body sections for public Clay JavaScript/TypeScript API documentation.
- [Clay Configuration System](reference/clay-js-api/configuration.md) — `~/.config/clay/init.js`, modular user configuration, key bindings, and configuration as documented Clay JS APIs.
- [Clay JS API Current Functionality Inventory](reference/clay-js-api/inventory.md) — Phase 7 public/internal API authority and runtime-path classifications.

## Developer Guides

- [Windows MSVC Development](development/windows.md) — Rust MSVC setup, Windows local named-pipe IPC notes, and validation commands.

## Clay JS API Registry Source Files

The generated documentation registry must read this section as the explicit inclusion list for public Clay JS API documentation. Add every public API Markdown file here before updating the generated registry.

- [quit](reference/clay-js-api/application/quit.md) — `clay.application.quit`
- [getActiveBehaviorManifest](reference/clay-js-api/behavior/get-active-behavior-manifest.md) — `clay.behavior.getActiveBehaviorManifest`
- [listBehaviorRoutes](reference/clay-js-api/behavior/list-behavior-routes.md) — `clay.behavior.listBehaviorRoutes`
- [getConfigurationState](reference/clay-js-api/configuration/get-configuration-state.md) — `clay.configuration.getConfigurationState`
- [loadConfigurationModule](reference/clay-js-api/configuration/load-configuration-module.md) — `clay.configuration.loadConfigurationModule`
- [serverGetDocumentLease](reference/clay-js-api/documents/server-get-document-lease.md) — `clay.documents.serverGetDocumentLease`
- [serverGetDocumentSnapshot](reference/clay-js-api/documents/server-get-document-snapshot.md) — `clay.documents.serverGetDocumentSnapshot`
- [clientMoveCursor](reference/clay-js-api/editor/client-move-cursor.md) — `clay.editor.clientMoveCursor`
- [clientScrollTo](reference/clay-js-api/editor/client-scroll-to.md) — `clay.editor.clientScrollTo`
- [clientSetCursorStyle](reference/clay-js-api/editor/client-set-cursor-style.md) — `clay.editor.clientSetCursorStyle`
- [clientSetSelection](reference/clay-js-api/editor/client-set-selection.md) — `clay.editor.clientSetSelection`
- [clientSetViewport](reference/clay-js-api/editor/client-set-viewport.md) — `clay.editor.clientSetViewport`
- [serverDeleteRange](reference/clay-js-api/editor/server-delete-range.md) — `clay.editor.serverDeleteRange`
- [serverInsertNewline](reference/clay-js-api/editor/server-insert-newline.md) — `clay.editor.serverInsertNewline`
- [serverInsertText](reference/clay-js-api/editor/server-insert-text.md) — `clay.editor.serverInsertText`
- [bindKey](reference/clay-js-api/keybindings/bind-key.md) — `clay.keybindings.bindKey`
- [listKeyBindings](reference/clay-js-api/keybindings/list-key-bindings.md) — `clay.keybindings.listKeyBindings`
- [unbindKey](reference/clay-js-api/keybindings/unbind-key.md) — `clay.keybindings.unbindKey`

## Registry Rules

- Markdown plus this master index is the source of truth.
- Generated registry artifacts must be derived from the files linked under **Clay JS API Registry Source Files**.
- Do not hand-edit generated registry artifacts as the authoritative documentation source.
- Public API documentation belongs under `docs/reference/clay-js-api/`.
- Internal implementation education belongs in `docs/wiki/` and should link to reference docs instead of duplicating public API usage.
