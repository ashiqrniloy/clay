# Markdown Mode POC Requirements

This Phase 16 requirements document defines the **Phase 18 Markdown mode proof-of-concept scope in primitive terms only**. It does not implement runtime code, protocol messages, package loading, rendering hooks, or JavaScript package files.

The POC target is a first-party package with package identity `@clay/markdown` and API prefix `markdown`. The package proves that Markdown editing and rendering can be delivered through documented primitives rather than hard-coded Rust mode logic. Its package security baseline is governed by `docs/reference/primitives/package-security.md`.

## Phase 18 Readiness Checklist

Phase 18 may begin when the Phase-17 package/mode foundation can provide these minimum capabilities:

- [ ] Package loader accepts first-party package identity `@clay/markdown` and prefix `markdown`.
- [ ] Mode detection can classify `.md`, `.markdown`, `.mdown`, and MIME hint `text/markdown` as Markdown without scanning the filesystem.
- [ ] Major mode activation can atomically install a Markdown behavior manifest and primitive declarations for an open document.
- [ ] Behavior manifests can express Enter/list continuation and fenced-code indentation as inert `ClientFirstPredictable` text transform rules.
- [ ] Background parse handlers can publish validated, viewport-bounded Markdown decoration spans.
- [ ] Command registration can expose package-prefixed Markdown commands and bind keys to those commands.
- [ ] SDUI package contributions can provide a Markdown preview/toggle panel without client-side JavaScript.
- [ ] Documentation/registry tests can trace every Markdown capability to `docs/reference/primitives/registry.md` and to planned Clay JS API stubs.

## Mode Detection and Activation Rules

| Requirement | Primitive Category | Registry Entry | Status | Risk | Phase 18 Acceptance Target |
| --- | --- | --- | --- | --- | --- |
| Extension detection for `.md`, `.markdown`, `.mdown` | Document classification | `DocumentClassification` | New | Medium | `@clay/markdown` declares static extension patterns; server selects Markdown for matching open documents. |
| MIME hint detection for `text/markdown` | Document classification | `DocumentClassification` | New | Medium | Open document metadata may carry a MIME hint; server does not use package JavaScript or filesystem scans for classification. |
| Major mode declaration for Markdown | Major mode activation | `MajorModeActivation` | New | Medium | Package metadata declares a `markdown` major mode with display name `Markdown`. |
| Atomic activation of Markdown behavior/rendering primitives | Major mode activation | `MajorModeActivation` | New | Medium | Activation publishes one behavior version and validated primitive set for the document. |

Proposed first-party activation API shape, for planning only:

```ts
// clay:modes, planned Phase 17/18 surface
await modes.serverRegisterModePattern({
  packageName: "@clay/markdown",
  apiPrefix: "markdown",
  mode: "markdown",
  extensions: ["md", "markdown", "mdown"],
  mimeTypes: ["text/markdown"],
});

await modes.serverActivateMajorMode({
  documentId,
  mode: "markdown",
  packageName: "@clay/markdown",
});
```

## Editing Behavior Scope

Markdown editing behavior must be declarative manifest data. The Rust client may execute known transform engines locally, but the package must not run JavaScript in keypress or text-event handlers.

| Requirement | Primitive Category | Registry Entry | Status | Risk | Phase 18 Acceptance Target |
| --- | --- | --- | --- | --- | --- |
| Continue unordered list markers on Enter (`-`, `*`, `+`) | Text transform | `TextTransform` | Exists/Extend | Low | Manifest rule emits same marker and indentation; empty list item exits list. |
| Continue ordered list markers on Enter (`1.`, `2.`) | Text transform | `TextTransform` | Exists/Extend | Low | Manifest rule increments next number when deterministic; non-deterministic renumbering is deferred. |
| Preserve fenced code block indentation on Enter | Text transform | `TextTransform` | Exists/Extend | Low | Inside triple-backtick or triple-tilde fences, Enter copies fence body indentation. |
| Heading insertion command (`#`, `##`, `###`) | Command declaration + key routing | `CommandDeclaration`, `KeyRoutingOverride` | New + Exists/Extend | Medium | Command inserts or adjusts heading marker through documented editor edit APIs. |
| List toggle command | Command declaration + key routing | `CommandDeclaration`, `KeyRoutingOverride` | New + Exists/Extend | Medium | Command toggles unordered list marker for current line/selection. |

Proposed manifest rule shapes, for planning only:

```json
{
  "rule_kind": "markdown_list_continuation",
  "routing_policy": "ClientFirstPredictable",
  "markers": ["-", "*", "+", "ordered-dot"],
  "exit_on_empty_item": true
}
```

```json
{
  "rule_kind": "markdown_fenced_code_indent",
  "routing_policy": "ClientFirstPredictable",
  "fence_markers": ["```", "~~~"],
  "copy_body_indent": true
}
```

## Decoration and Rendering Scope

The POC targets decorated editing, not full Markdown preview fidelity. Decoration results are inert spans produced by server-side background parsing and rendered by the Rust client.

Minimum decoration span kinds:

- `markdown.heading`: ATX headings (`#` through `######`) with emphasis level metadata.
- `markdown.strong`: bold spans delimited by `**` or `__`.
- `markdown.emphasis`: italic spans delimited by `*` or `_` where unambiguous.
- `markdown.code_span`: inline backtick code spans.
- `markdown.code_block`: fenced code blocks delimited by triple backticks or tildes.
- `markdown.list_marker`: unordered and ordered list markers.

| Requirement | Primitive Category | Registry Entry | Status | Risk | Phase 18 Acceptance Target |
| --- | --- | --- | --- | --- | --- |
| Heading/bold/italic/code-span/code-block syntax spans | Decoration ranges | `DecorationRange` | New | Medium | Package publishes byte ranges with kind, style token, priority, document version, and provenance. |
| List marker and heading emphasis styling | Decoration ranges | `DecorationRange` | New | Medium | Client locally renders validated spans; no package draw callbacks or arbitrary styles. |
| Background Markdown parse task | Incremental parse update | `IncrementalParseUpdate` | New | High | Server-side parser returns viewport-prioritized `DecorationUpdate` payloads and stale versions are discarded. |
| Fenced code block region invalidation | Incremental parse update | `IncrementalParseUpdate` | New | High | Region-level invalidation covers edits inside or near fences; full-file parse is reserved for activation/resync. |
| Optional folding for headings/code fences | Folding ranges | `FoldingRange` | Deferred | Low | Not required for Phase 18 POC; may be added after Markdown editing/rendering proves the primitive flow. |

Proposed decoration payload shape, for planning only:

```ts
type MarkdownDecorationSpan = {
  byteStart: number;
  byteEnd: number;
  kind:
    | "markdown.heading"
    | "markdown.strong"
    | "markdown.emphasis"
    | "markdown.code_span"
    | "markdown.code_block"
    | "markdown.list_marker";
  styleToken: string;
  priority: number;
};
```

## Commands, Key Bindings, and SDUI Panel

Commands are package-prefixed and registered through Clay JS APIs. Key bindings are inert manifest entries that route to registered commands. The preview/toggle UI uses SDUI, not direct widget mutation.

| Requirement | Primitive Category | Registry Entry | Status | Risk | Phase 18 Acceptance Target |
| --- | --- | --- | --- | --- | --- |
| Toggle Markdown preview command | Command declaration | `CommandDeclaration` | New | Medium | Register `markdown.togglePreview` with user-facing name `Toggle Markdown Preview`. |
| Insert heading command | Command declaration | `CommandDeclaration` | New | Medium | Register `markdown.insertHeading` with heading level as documented custom property/argument. |
| Toggle list command | Command declaration | `CommandDeclaration` | New | Medium | Register `markdown.toggleList` for current line/selection. |
| Heading insertion key binding | Key routing override | `KeyRoutingOverride` | Exists/Extend | Low | Example binding may be `Ctrl+Alt+1` / `Ctrl+Alt+2` / `Ctrl+Alt+3`, final binding chosen in Phase 18. |
| List toggle key binding | Key routing override | `KeyRoutingOverride` | Exists/Extend | Low | Example binding may be `Ctrl+Shift+8`, final binding chosen in Phase 18. |
| Preview toggle key binding | Key routing override | `KeyRoutingOverride` | Exists/Extend | Low | Example binding may be `Ctrl+Shift+M`, final binding chosen in Phase 18. |
| Preview/decoration toggle panel | SDUI panel/status contribution | `SduiPanelStatusContribution` | Exists/Extend | Low | SDUI panel shows preview state and decoration toggle actions through inert nodes. |

Proposed command registration sketch, for planning only:

```ts
await commands.serverRegisterCommand({
  id: "markdown.togglePreview",
  packageName: "@clay/markdown",
  apiPrefix: "markdown",
  userFacingName: "Toggle Markdown Preview",
  routingPolicy: "ServerFirst",
  permissions: []
});
```

## Minimum Clay JS API Stubs Needed Before Phase 18

| API Stub | Registry Entry | Status | Risk | Why Markdown Needs It |
| --- | --- | --- | --- | --- |
| `modes.serverRegisterModePattern` | `DocumentClassification` | New | Medium | Bind file extensions and MIME hints to Markdown mode. |
| `modes.serverActivateMajorMode` | `MajorModeActivation` | New | Medium | Activate Markdown for an open document and publish behavior version. |
| `commands.serverRegisterCommand` | `CommandDeclaration` | New | Medium | Register package-prefixed Markdown commands. |
| `keybindings.bindKey` | `KeyRoutingOverride` | Exists/Extend | Low | Bind Markdown command shortcuts. |
| `behavior.getActiveBehaviorManifest` | `TextTransform` | Exists/Extend | Low | Verify installed Enter/list/code-block transform manifest. |
| `decorations.serverPublishDecorations` | `DecorationRange` | New | Medium | Publish syntax/emphasis/code spans for local rendering. |
| `parse.serverRegisterParseHandler` | `IncrementalParseUpdate` | New | High | Register server-side Markdown parse/update handler. |
| `sdui.definePanel` and `sdui.publishTree` | `SduiPanelStatusContribution` | Exists/Extend | Low | Publish preview/decoration toggle panel. |
| `folding.serverPublishFoldingRanges` | `FoldingRange` | Deferred | Low | Optional heading/code block folding; not a Phase 18 readiness gate. |

## Performance Targets

Phase 18 must measure Markdown mode against existing typed constants in `src/perf/budgets.rs`.

| Target | Budget Constant | Required Phase 18 Behavior |
| --- | --- | --- |
| Startup parse / mode activation cost | `MODE_ACTIVATION_P95_BUDGET_MS` | Opening a Markdown document and installing the first behavior/decorator state should stay within the advisory activation budget or record a benchmarked deferral. |
| Incremental edit decoration cost | `INCREMENTAL_PARSE_UPDATE_BUDGET_BYTES` | Parse edit notifications and published parse results are bounded and cancellable; no full-document update for ordinary edits. |
| Decoration payload per update | `DECORATION_PAYLOAD_BUDGET_BYTES` | Each `DecorationUpdate` contains only viewport-relevant spans and package provenance. |
| Scroll/render latency | `SCROLL_LAYOUT_RENDER_ADJACENT_P95_BUDGET_MS` | Scrolling adjacent Markdown content with existing decoration spans must not regress the adjacent layout/render budget. |
| Local keypress-to-paint latency | `KEYPRESS_TO_LOCAL_PAINT_P95_BUDGET_MS` | List continuation and code-block indentation are `ClientFirstPredictable` manifest transforms and do not wait for server JavaScript. |
| Behavior manifest payload | `BEHAVIOR_MANIFEST_PAYLOAD_BUDGET_BYTES` | Markdown key bindings and transform rules fit inside the behavior manifest payload budget. |

## Security and Permissions

- Package identity: `@clay/markdown`.
- API prefix: `markdown`.
- Default permissions: `[]` for inert mode declarations, key bindings, text transforms, SDUI panel declarations, and local decoration metadata.
- Required permission-bearing primitives before execution: `mode-registration`, `mode-activation`, `command-registration`, `render-decorations`, and `parse-document` as defined by `docs/reference/primitives/package-security.md`.
- The package cannot access filesystem paths beyond document content already open in Clay.
- The package cannot use network, shell, AI mutation, remote listeners, WASM execution, raw `Deno.core.ops`, direct renderer/widget mutation, or client-side JavaScript authority.
- All behavior must use documented Clay JS APIs; raw operation names are not user-facing package APIs.
- Parse and command handlers run server-side and asynchronously; the client receives only validated behavior manifests, SDUI trees, and decoration/folding data.

## Deferred or Out of Scope for Phase 18 POC

| Capability | Registry Entry | Status | Rationale |
| --- | --- | --- | --- |
| Full CommonMark HTML preview fidelity | `SduiPanelStatusContribution` | Deferred | POC needs preview/decorated editor behavior, not complete HTML compatibility. |
| Semantic diagnostics for broken links/frontmatter | `DecorationRange` | Deferred | Requires workspace/file authority decisions not needed for first Markdown editing POC. |
| Heading/code-block folding | `FoldingRange` | Deferred | Useful but not needed to prove package-controlled editing/rendering. |
| Completion providers for links/headings | `CompletionTriggerAndResult` | Deferred | Completion primitive is recorded but not required for initial Markdown POC. |
| Minor mode overlays | `MinorModeActivation` | Deferred | Markdown POC uses one major mode only. |
| Network-backed preview assets | `PackagePermissionDeclaration` | Deferred | Network authority is prohibited by default and would require a later decision log. |

## References

- `roadmap.md` Phase 16 and Phase 18.
- `docs/reference/primitives/registry.md`.
- `docs/reference/primitives/rendering-strategy.md`.
- `docs/reference/primitives/parse-update-strategy.md`.
- `.agents/skills/project-patterns/references/package-distribution.md`.
- `.agents/skills/project-patterns/references/behavior-manifests.md`.
- `.agents/skills/project-patterns/references/protocol-and-performance.md`.
- `src/perf/budgets.rs`.
