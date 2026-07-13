# Clay Documentation Index

This is the master Markdown index for Clay's public, programmatic documentation. Markdown files linked from the registry source section are the authoritative source for generated app/help/agent documentation registries.

## Documentation Contract

- [Clay JS API Markdown Schema](reference/clay-js-api/schema.md) — required frontmatter and body sections for public Clay JavaScript/TypeScript API documentation.
- [Clay Configuration System](reference/clay-js-api/configuration.md) — `~/.config/clay/init.js`, modular user configuration, key bindings, and configuration as documented Clay JS APIs.
- [Clay JS API Current Functionality Inventory](reference/clay-js-api/inventory.md) — Phase 7 public/internal API authority and runtime-path classifications.

## Developer Guides

- [Clay Primitives Reference](reference/primitives/index.md) — Phase 16 primitives navigation index, prioritized backlog, Phase 17 prerequisite checklist, and Phase 18.16 tiered syntax-engine contract.
- [Existing Primitive Audit](reference/primitives/audit.md) — Phase 16 static audit of behavior manifest, SDUI, configuration, file/workspace, document, and observability primitives.
- [Primitive Registry Schema](reference/primitives/registry.md) — Phase 16 primitive category taxonomy, registry schema, Clay JS API shape stubs, security boundaries, and advisory primitive budgets.
- [Rendering Customization Strategy](reference/primitives/rendering-strategy.md) — Phase 16 inert rendering declarations, decoration update shape, SDUI reuse, client rendering attachment points, budgets, and security boundaries.
- [Clay Shell and Package UI/Layout Strategy](reference/primitives/shell-layout-strategy.md) — Phase 18.1/18.2 shell vocabulary and runtime status, Phase 18.3 runtime-backed slot-aware package UI contribution contract, working area and pane/slot layout model, package UI/state/style contract, and Masonry implementation boundary.
- [Incremental Parse and Background Parse Update Strategy](reference/primitives/parse-update-strategy.md) — cancellable server-side parse tasks, tiered syntax engines, non-blocking open, viewport-prioritized results, diagnostics, budgets, and security boundaries.
- [Markdown Mode POC Requirements](reference/primitives/markdown-mode-requirements.md) — Phase 16 Markdown mode Phase 18 readiness checklist, primitive prerequisite map, performance targets, API stubs, and first-party package security scope.
- [Package Primitive Security and Provenance Requirements](reference/primitives/package-security.md) — Phase 16 package primitive prefix, permission, validation, conflict, prohibited-authority, and provenance model.
- [Phase 17 Package Loading Runtime Facades](reference/primitives/package-loading.md) — package load/runtime boundaries, conflict handling, runtime facade wiring, hot-path policy, and Phase 18 decoration/parse handoff.
- [Creating Clay Packages](reference/packages/creating-packages.md) — package authoring guide covering manifests, explicit loading, tiered syntax engines, UI/layout, components, input, actions, logic, data/state, configuration, styling/theme tokens, permissions, documentation, tests, and current/planned shell architecture.
- [@clay/markdown Package](reference/packages/markdown.md) — first-party Markdown full-mode package: Tier 1 native grammar, vocabulary styleMap, behavior manifest, base completion provider, and runtime boundary.
- [@clay/rust Package](reference/packages/rust.md) — first-party Rust full-mode package: Tier 1 native grammar, vocabulary styleMap, behavior manifest, base completion provider, and runtime boundary.
- [@clay/typescript Package](reference/packages/typescript.md) — first-party TypeScript full-mode package: Tier 1 native grammar, vocabulary styleMap, behavior manifest, base completion provider, and runtime boundary.
- [@clay/javascript Package](reference/packages/javascript.md) — first-party JavaScript full-mode package: Tier 1 native grammar, vocabulary styleMap, behavior manifest, base completion provider, and runtime boundary.
- [Primitive Implementation Gate](reference/primitives/implementation-gate.md) — Phase 16.5 package/mode validation gate, fixture format, deterministic failure coverage, and Phase 17/18 handoff.
- [Text Vocabulary and Two-Axis Decoration Contract](reference/primitives/syntax-vocabulary.md) — Phase 18.15 locked LSP-based `TokenType` + `Modifiers` vocabulary, Clay prose/text-attribute extensions, open-string scope escape, compatibility mapping from free-form `style_token`, and single-source-of-color `StyleRegistry` invariant.
- [Semantic Typography Roles](reference/primitives/typography.md) — Phase 18.16.5 package/mode authoring contract for semantic document, range, and component roles with user-owned concrete typography.
- [Range Diagnostics](reference/primitives/diagnostics.md) — Phase 18.17 byte-range diagnostic primitive for Tree-sitter recovery, package analyzers, theme-owned squiggles, and future LSP bridges.
- [Launch and GUI Smoke Validation](development/launch-and-gui-smoke.md) — command-first `cargo run`, tiered syntax-engine smoke, `smoke-gui`, foreground server/client, GUI status, and local IPC validation.
- [Performance Fixtures and Baseline Workflow](development/performance.md) — deterministic large-file fixture generation, Criterion baseline commands, Phase 14 budgets/guardrails, opt-in profiling hooks, and validation commands.
- [UI Observability and SDUI Structural Regression](development/ui-observability.md) — headless SDUI structural regression coverage, status observability, window-driver smoke relationship, and deferred GPU-backed pixel snapshot path.
- [Windows MSVC Development](development/windows.md) — Rust MSVC setup, Windows local named-pipe IPC notes, and validation commands.

## Clay JS API Registry Source Files

The generated documentation registry must read this section as the explicit inclusion list for public Clay JS API documentation. Add every public API Markdown file here before updating the generated registry.

- [quit](reference/clay-js-api/application/quit.md) — `clay.application.quit`
- [buildCodeEditingManifest](reference/clay-js-api/behavior/build-code-editing-manifest.md) — `clay.behavior.buildCodeEditingManifest`
- [getActiveBehaviorManifest](reference/clay-js-api/behavior/get-active-behavior-manifest.md) — `clay.behavior.getActiveBehaviorManifest`
- [listBehaviorRoutes](reference/clay-js-api/behavior/list-behavior-routes.md) — `clay.behavior.listBehaviorRoutes`
- [serverExecuteCommand](reference/clay-js-api/commands/server-execute-command.md) — `clay.commands.serverExecuteCommand`
- [serverListCommands](reference/clay-js-api/commands/server-list-commands.md) — `clay.commands.serverListCommands`
- [serverOpenFile](reference/clay-js-api/commands/server-open-file.md) — `clay.commands.serverOpenFile`
- [serverOpenDirectory](reference/clay-js-api/commands/server-open-directory.md) — `clay.commands.serverOpenDirectory`
- [serverRegisterCommand](reference/clay-js-api/commands/server-register-command.md) — `clay.commands.serverRegisterCommand`
- [serverRevealInTree](reference/clay-js-api/commands/server-reveal-in-tree.md) — `clay.commands.serverRevealInTree`
- [serverPublishDecorations](reference/clay-js-api/decorations/server-publish-decorations.md) — `clay.decorations.serverPublishDecorations`
- [serverPublishDiagnostics](reference/clay-js-api/diagnostics/server-publish-diagnostics.md) — `clay.diagnostics.serverPublishDiagnostics`
- [getConfigurationState](reference/clay-js-api/configuration/get-configuration-state.md) — `clay.configuration.getConfigurationState`
- [loadConfigurationModule](reference/clay-js-api/configuration/load-configuration-module.md) — `clay.configuration.loadConfigurationModule`
- [clientOpenFileDialog](reference/clay-js-api/documents/client-open-file-dialog.md) — `clay.documents.clientOpenFileDialog`
- [serverGetDocumentLease](reference/clay-js-api/documents/server-get-document-lease.md) — `clay.documents.serverGetDocumentLease`
- [serverGetDocumentSnapshot](reference/clay-js-api/documents/server-get-document-snapshot.md) — `clay.documents.serverGetDocumentSnapshot`
- [serverGetDocumentStatus](reference/clay-js-api/documents/server-get-document-status.md) — `clay.documents.serverGetDocumentStatus`
- [serverListDocuments](reference/clay-js-api/documents/server-list-documents.md) — `clay.documents.serverListDocuments`
- [serverOpenDocument](reference/clay-js-api/documents/server-open-document.md) — `clay.documents.serverOpenDocument`
- [serverReloadDocument](reference/clay-js-api/documents/server-reload-document.md) — `clay.documents.serverReloadDocument`
- [serverSaveDocument](reference/clay-js-api/documents/server-save-document.md) — `clay.documents.serverSaveDocument`
- [clientCopySelection](reference/clay-js-api/editor/client-copy-selection.md) — `clay.editor.clientCopySelection`
- [clientMoveCursor](reference/clay-js-api/editor/client-move-cursor.md) — `clay.editor.clientMoveCursor`
- [clientScrollTo](reference/clay-js-api/editor/client-scroll-to.md) — `clay.editor.clientScrollTo`
- [clientSetCursorStyle](reference/clay-js-api/editor/client-set-cursor-style.md) — `clay.editor.clientSetCursorStyle`
- [clientSetSelection](reference/clay-js-api/editor/client-set-selection.md) — `clay.editor.clientSetSelection`
- [clientSetViewport](reference/clay-js-api/editor/client-set-viewport.md) — `clay.editor.clientSetViewport`
- [serverDeleteRange](reference/clay-js-api/editor/server-delete-range.md) — `clay.editor.serverDeleteRange`
- [serverInsertNewline](reference/clay-js-api/editor/server-insert-newline.md) — `clay.editor.serverInsertNewline`
- [serverInsertText](reference/clay-js-api/editor/server-insert-text.md) — `clay.editor.serverInsertText`
- [serverListGitStatuses](reference/clay-js-api/git/server-list-git-statuses.md) — `clay.git.serverListGitStatuses`
- [serverRefreshGitStatus](reference/clay-js-api/git/server-refresh-git-status.md) — `clay.git.serverRefreshGitStatus`
- [bindKey](reference/clay-js-api/keybindings/bind-key.md) — `clay.keybindings.bindKey`
- [listKeyBindings](reference/clay-js-api/keybindings/list-key-bindings.md) — `clay.keybindings.listKeyBindings`
- [unbindKey](reference/clay-js-api/keybindings/unbind-key.md) — `clay.keybindings.unbindKey`
- [serverActivateMajorMode](reference/clay-js-api/modes/server-activate-major-mode.md) — `clay.modes.serverActivateMajorMode`
- [serverClassifyDocument](reference/clay-js-api/modes/server-classify-document.md) — `clay.modes.serverClassifyDocument`
- [serverRegisterModePattern](reference/clay-js-api/modes/server-register-mode-pattern.md) — `clay.modes.serverRegisterModePattern`
- [serverLoadPackage](reference/clay-js-api/packages/server-load-package.md) — `clay.packages.serverLoadPackage`
- [loadPackage](reference/clay-js-api/packages/load-package.md) — `clay.packages.loadPackage`
- [serverRegisterParseHandler](reference/clay-js-api/parse/server-register-parse-handler.md) — `clay.parse.serverRegisterParseHandler`
- [serverRegisterSyntaxGrammar](reference/clay-js-api/syntax/server-register-syntax-grammar.md) — `clay.syntax.serverRegisterSyntaxGrammar`
- [setSyntaxEnginePreference](reference/clay-js-api/syntax/set-syntax-engine-preference.md) — `clay.syntax.setSyntaxEnginePreference`
- [setTheme](reference/clay-js-api/theme/set-theme.md) — `clay.theme.setTheme`
- [setTypography](reference/clay-js-api/theme/set-typography.md) — `clay.theme.setTypography`
- [completionTriggerCharactersFromEditorRules](reference/clay-js-api/completion/completion-trigger-characters-from-editor-rules.md) — `clay.completion.completionTriggerCharactersFromEditorRules`
- [serverListCompletionProvidersForTrigger](reference/clay-js-api/completion/server-list-completion-providers-for-trigger.md) — `clay.completion.serverListCompletionProvidersForTrigger`
- [serverRegisterCompletionProvider](reference/clay-js-api/completion/server-register-completion-provider.md) — `clay.completion.serverRegisterCompletionProvider`
- [serverValidatePackageManifest](reference/clay-js-api/packages/server-validate-package-manifest.md) — `clay.packages.serverValidatePackageManifest`
- [serverValidatePackagePermissions](reference/clay-js-api/packages/server-validate-package-permissions.md) — `clay.packages.serverValidatePackagePermissions`
- [setPackageOption](reference/clay-js-api/configuration/set-package-option.md) — `clay.configuration.setPackageOption`
- [serverRegisterComponentContribution](reference/clay-js-api/ui/server-register-component-contribution.md) — `clay.ui.serverRegisterComponentContribution`
- [serverRegisterInputContribution](reference/clay-js-api/ui/server-register-input-contribution.md) — `clay.ui.serverRegisterInputContribution`
- [serverRegisterPanelContribution](reference/clay-js-api/ui/server-register-panel-contribution.md) — `clay.ui.serverRegisterPanelContribution`
- [serverRegisterUiStateScope](reference/clay-js-api/ui/server-register-ui-state-scope.md) — `clay.ui.serverRegisterUiStateScope`
- [serverRegisterThemeToken](reference/clay-js-api/ui/server-register-theme-token.md) — `clay.ui.serverRegisterThemeToken`
- [serverRegisterTransientOverlayContribution](reference/clay-js-api/ui/server-register-transient-overlay-contribution.md) — `clay.ui.serverRegisterTransientOverlayContribution`
- [serverSetLayoutOverride](reference/clay-js-api/ui/server-set-layout-override.md) — `clay.ui.serverSetLayoutOverride`
- [defineButton](reference/clay-js-api/sdui/define-button.md) — `clay.sdui.defineButton`
- [defineEditorView](reference/clay-js-api/sdui/define-editor-view.md) — `clay.sdui.defineEditorView`
- [defineFlex](reference/clay-js-api/sdui/define-flex.md) — `clay.sdui.defineFlex`
- [defineLabel](reference/clay-js-api/sdui/define-label.md) — `clay.sdui.defineLabel`
- [defineList](reference/clay-js-api/sdui/define-list.md) — `clay.sdui.defineList`
- [definePanel](reference/clay-js-api/sdui/define-panel.md) — `clay.sdui.definePanel`
- [defineStack](reference/clay-js-api/sdui/define-stack.md) — `clay.sdui.defineStack`
- [publishTree](reference/clay-js-api/sdui/publish-tree.md) — `clay.sdui.publishTree`
- [clientOpenFolderDialog](reference/clay-js-api/workspace/client-open-folder-dialog.md) — `clay.workspace.clientOpenFolderDialog`
- [serverAddWorkspaceRoot](reference/clay-js-api/workspace/server-add-workspace-root.md) — `clay.workspace.serverAddWorkspaceRoot`
- [serverCancelListing](reference/clay-js-api/workspace/server-cancel-listing.md) — `clay.workspace.serverCancelListing`
- [serverCreateListingCancelToken](reference/clay-js-api/workspace/server-create-listing-cancel-token.md) — `clay.workspace.serverCreateListingCancelToken`
- [serverDiscoverWorkspaceRootForPath](reference/clay-js-api/workspace/server-discover-workspace-root-for-path.md) — `clay.workspace.serverDiscoverWorkspaceRootForPath`
- [serverListDirectory](reference/clay-js-api/workspace/server-list-directory.md) — `clay.workspace.serverListDirectory`
- [serverListWorkspaceRoots](reference/clay-js-api/workspace/server-list-workspace-roots.md) — `clay.workspace.serverListWorkspaceRoots`

## Registry Rules

- Markdown plus this master index is the source of truth.
- Generated registry artifacts must be derived from the files linked under **Clay JS API Registry Source Files**.
- Do not hand-edit generated registry artifacts as the authoritative documentation source.
- Public API documentation belongs under `docs/reference/clay-js-api/`.
- Internal implementation education belongs in `docs/wiki/` and should link to reference docs instead of duplicating public API usage.
