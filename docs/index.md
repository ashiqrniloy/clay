# Clay Documentation Index

This is the master Markdown index for Clay's public, programmatic documentation. Markdown files linked from the registry source section are the authoritative source for generated app/help/agent documentation registries.

## Documentation Contract

- [Clay JS API Markdown Schema](reference/clay-js-api/schema.md) — required frontmatter and body sections for public Clay JavaScript/TypeScript API documentation.
- [Clay Configuration System](reference/clay-js-api/configuration.md) — `~/.config/clay/init.js`, modular user configuration, key bindings, and configuration as documented Clay JS APIs.

## Clay JS API Registry Source Files

The generated documentation registry must read this section as the explicit inclusion list for public Clay JS API documentation. Add every public API Markdown file here before updating the generated registry.

No Clay JS API reference files have been authored yet. Future entries should use this form:

```markdown
- [serverInsertText](reference/clay-js-api/editor/server-insert-text.md) — `clay.editor.serverInsertText`
```

## Registry Rules

- Markdown plus this master index is the source of truth.
- Generated registry artifacts must be derived from the files linked under **Clay JS API Registry Source Files**.
- Do not hand-edit generated registry artifacts as the authoritative documentation source.
- Public API documentation belongs under `docs/reference/clay-js-api/`.
- Internal implementation education belongs in `docs/wiki/` and should link to reference docs instead of duplicating public API usage.
