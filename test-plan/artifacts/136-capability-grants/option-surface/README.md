# Plan 136 task 8 — Clay JS option-surface drift guard

Evidence for the hardened drift guard and the option-surface audit it forced.

## Guard contract

`tests/clay_js_api_inventory.rs::declared_option_keys_are_documented_for_every_public_api`
walks every public inventory entry whose facade declares an `options` (or
`declaration`) parameter with a locally declared object type (a positional or
`unknown` parameter declares no option surface) and requires each non-`never`
member to be documented in **both**

- the machine-readable `custom_properties` (page frontmatter + `api-inventory.toml`;
  dotted paths document nested members, so `viewport.byteStart` covers
  `viewport.byteStart` and `analyzer.id` covers `analyzer`), and
- a bullet or code-span-led line of the page's `## Options` section.

Coverage on the fixed tree: **236 declared keys across 59 APIs** (asserted, so the
guard fails loudly if it stops walking the surface). `never`-typed members are the
denial list; `Record<string, unknown>` members are named options with an open value
type and are checked like any other key. The guard reuses the existing
`inventory_entries()` / `custom_property_names()` helpers and
`ClayJsApiRegistry::from_docs` instead of a second parser, and it never loosens the
frontmatter↔inventory↔registry equality or the denied-authority checks.

One allowlist entry, with a reason:
`syntax.serverRegisterSyntaxGrammar` — the declared type mirrors the
`clay.contributions.syntaxGrammars` manifest descriptor, which the op reads from the
host-enabled package record.

## Preconditions (declared surfaces brought in line with the ops)

Deleted from the typings because nothing reads them (silent-ignore traps):

| File | Removed | Evidence |
| --- | --- | --- |
| `runtime/js/completion.d.ts` | `completionProvider`, `contribution`, `providerId`, `triggerCharacters`, `triggers`, `wordBoundaryChars`, `items`, `priority`, `exclusive`, `timeoutMs`, `maxItems` | `op_clay_completion_register_completion_provider` reads only `exportName`, `moduleSpecifier`, `runtimeBridge`; provider metadata comes from the host-enabled `clay.contributions.completionProviders` record |
| `runtime/js/language.d.ts` | nested `module?: string` on `LanguageIntelligenceProviderDeclaration` | the op reads `moduleSpecifier` (flat or nested) and the facade binds only the top-level `module` object |
| `runtime/js/decorations.d.ts` | `behaviorVersion?: number` | `op_clay_decorations_publish_decorations` never reads it (also dropped from the JS test fixture) |

Documented option surfaces that were implemented but invisible in the
machine-readable metadata (58 custom properties + 20 `## Options` bullets + 39
`## Custom properties` mirror lines), including the audit findings:

| API | Finding |
| --- | --- |
| `parse.serverRegisterParseHandler` | the op reads **`mode`**; the page/registry documented `modeId`, which the op never reads. Added `parseWindowBytes` (real alias of `maxWindowBytes`) |
| `decorations.serverPublishDecorations` | the op requires **`viewport`**; the page/registry documented `viewportByteRange`, which the op never reads. Added `currentDocumentVersion` |
| `modes.serverRegisterModePattern` | `displayName`, `defaultFontRole`, `shebangPatterns`, `contentProbes` are read by the registration op; `commands`/`keymaps` are cached by the `clay:modes` facade and published at activation |
| `editor.clientAddCursor` / `clientColumnSelect` / `clientMoveCursor` / `clientSetSelection` / `clientScrollTo` / `clientSetViewport` | the optional `documentId` surface target was undocumented; it is now declared and documented uniformly with the rest of the `client*` family |
| `documents.serverOpenDocument` / `serverSaveDocument` / `serverReloadDocument` | op-read `path`, `workspaceRootId`, `documentId`, `knownVersion`, `force` were not inventoried |
| `agent.*` (10 APIs) | daemon-read options (`sessionId`, `strategy`, `enabled`, `decisions`, `entryId`, `method`, `handler`, `args`, `skills`, `tools`, `toolNames`, `graft*`, `wiki`, `workspaceRoot`, …) are now inventoried with correct types; `agent.commandDispatch.args` was missing entirely |
| `theme.setTheme` / `setAppearance` / `setTypography` / `setDesignSystem` / `setIconPack` | `specifier`, `appearance`, `monospace`, `proportional`, `ui`, `hierarchy` had no `## Options` line |
| `ui.serverRegisterComponentContribution` | `style` / `action` are parsed by `src/shell/package_ui.rs` and are now inventoried |
| `completion.serverDisableCompletion` | the prose "either `provider` or `packagePrefix`" is now a real option listing |
| `language.serverRegisterLanguageIntelligenceProvider` | `provider` (nested-declaration shape) and `budgets.timeoutMs` are read by the op and are now documented |
| `behavior.buildCodeEditingManifest`, `modes.serverRegisterModePattern`, `shell.setPaneFocusPolicy`, `ui.serverRegisterInputContribution`, `workspace.serverListDirectory`, `folding.serverPublishFoldingRanges`, `diagnostics.serverPublishDiagnostics` | missing option listings/properties completed |

Stale doc statements corrected: the completion page no longer claims the op
range-checks `timeoutMs`; the syntax page states that grammar metadata is read from
the manifest, not from call options.

## Delegated plan 131 task 6 item (planned rows)

`inventory_rust_paths_name_existing_source_files` now also checks each row's
`deno_op_path` and skips only fields carrying the explicit `planned:` marker, so a
planned row can name an unwritten op but cannot read as an existing path.
`application.quit`'s `deno_op_path` (never written; the escape-key path is the
client-local `src/client_commands.rs::EditorClientCommand`) now carries the marker
in both the inventory and the page frontmatter.

## Mutation checks (run by hand, not committed)

| Check | Result |
| --- | --- |
| `mutation-1-drop-documented-key.txt` — delete `moduleSpecifier` from the completion page frontmatter | FAILED as intended: "`moduleSpecifier` is declared in runtime/js/completion.d.ts and listed under ## Options but is missing from custom_properties" |
| `mutation-2-redeclare-ignored-key.txt` — re-add `priority?: number` to `ServerRegisterCompletionProviderOptions` | FAILED as intended: "`priority` is declared in runtime/js/completion.d.ts but is documented in neither custom_properties (frontmatter + api-inventory.toml) nor a ## Options listing" |
| `mutation-3-planned-row-without-marker.txt` — remove the `planned:` marker from `application.quit`'s `deno_op_path` | FAILED as intended: "application.quit: deno_op_path names missing source file src/server/ops/application.rs" (the plan 131 task 6 delegation: planned rows may name unwritten files, but only with the explicit marker) |
| `guard-pass.txt` — unmodified tree after both reverts | `option-surface guard: 236 declared keys across 59 APIs` … `test result: ok` |

Gate note: the first full-gate attempt failed one unrelated test,
`agent_protocol::mock_daemon_prompt_persists_no_secret_on_ack`, with
`failed to spawn clay-agent: Text file busy (os error 26)` — an environment flake
(the binary was busy while spawning); it passes in isolation and the clean rerun
in `gate-check-full.log` is the recorded result.

## Follow-ups found by the audit (documented-but-unimplemented direction, not fixed here)

The guard covers declared → documented. The opposite direction still needs a
reading of the ops as the source of truth:

- `syntax.serverRegisterSyntaxGrammar` declares the manifest descriptor as options
  and the page claimed `packageManifest` registers it; the op only registers the
  host-enabled package record (page now says so; typings kept via the allowlist).
- `parse.serverRegisterParseHandler.resultBudgetBytes` is documented but not read by
  the op.
- The `client*` family's optional `documentId` is inert (the client applies the
  command to the focused editor); it is declared and documented uniformly, and
  would need either multi-document targeting or deletion from the family.
- The completion page's ignored-metadata field list is documentation of rejection;
  the keys are no longer declared by the typings.
