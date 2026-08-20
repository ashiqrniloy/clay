# Phase 18.19 Completion Extensions Primitive Review

## Source

- Plan: `plans/051-Phase18.19-Completion-Extensions-Snippets-Exclusive-Claim-Disable-Native.md` (task 2).
- Roadmap: `roadmap.md` Phase 18.19.
- Patterns: `.agents/skills/project-patterns/references/mode-primitive-first.md`, `authority-boundaries.md`, `protocol-and-performance.md`, `clay-js-api-naming.md`, `clay-js-api-schema.md`, `configuration-system.md`.
- Predecessor reviews: `docs/wiki/modules/phase18.11-completion-provider-primitive-review.md`, `docs/wiki/modules/phase18.18-language-package-primitive-review.md`.
- `src/protocol/completion.rs` (`CompletionItem`, `CompletionRequest`, `CompletionResultSet`, `CompletionRejection`, `CompletionReplacementRange`, `CompletionProvenance`, `CompletionProviderGeneration`, authority-boundary module doc).
- `src/server/completion.rs` (`CompletionProviderMeta`, `CompletionCoordinator`, `BufferWordCompletionProvider`, `list_ordered`, `providers_for_trigger_character`, `schedule_completion`, `cancel_package`, `cancel_generation`, `bump_generation`, `remove_older_generations`).
- `src/server/ops/mod.rs` (`ClayOpState.completion_providers`, `completion_providers_for_trigger`, `register_completion_provider_metadata`).
- `src/server/ops/completion.rs` (`op_clay_completion_register_completion_provider`, `op_clay_completion_providers_for_trigger`, `completion_provider_metas`).
- `src/packages/record/mod.rs` (completion provider contribution descriptor).
- `src/shell/transient_menu.rs` (`CompletionMenuAcceptAction`, `TransientMenuAction`, `TransientMenuSession`).
- `src/editor/surface/mod.rs` (`accept_completion_with_event`, `finish_edit_with_operation`, `EditOperation::Replace`).
- `runtime/js/completion.js` (`serverRegisterCompletionProvider`, `serverListCompletionProvidersForTrigger`, `completionTriggerCharactersFromEditorRules`, prohibited authority fields).
- `packages/{rust,typescript,javascript,markdown}/package.json` (`clay.contributions.completionProviders`).
- `tests/completion_provider.rs`, `tests/primitives_docs.rs`, `tests/editor_performance_invariants.rs`, `tests/performance_protocol.rs`.

## Overview

Phase 18.19 extends the Phase 18.11 `CompletionTriggerAndResult` framework with three generic deltas: a snippet item kind carrying LSP snippet syntax as inert text the client expands locally, an opt-in `exclusive` claim so a provider can replace rather than merge with lower-priority providers, and a `serverDisableCompletion` Clay JS API so users turn off the native/base provider (or any provider/package) from `~/.config/clay/init.js`. A small first-party snippet set per language (Rust `fn`/`match`/`impl`, TypeScript `interface`/`type`) validates the new kind end-to-end through the existing package/provider path.

This review records the primitive inventory available after Phase 18.18 landed and identifies the generic gaps that must be filled before implementation. The target outcome is a completion framework where snippets ride the existing `CompletionItem`/`CompletionProviderMeta`/accept path (not a separate subsystem), exclusive suppression is one shared in-memory filter over the already-priority-ordered provider list, and disable-native is a user-side toggle over already-registered metadata that reuses the existing generation stale-drop mechanism. No per-language Rust branch, no snippet-specific provider registry, no snippet-specific Masonry widget, no client-side JavaScript, and no new authority are introduced.

## Existing Primitive Inventory

### Completion protocol shapes

`src/protocol/completion.rs::CompletionItem` is inert text-replacement data only: `label`, `insert_text`, `detail`, `commit_characters`, `text_format: CompletionItemTextFormat`, and `provenance: CompletionProvenance`. Task 4 implements `CompletionItemTextFormat { PlainText, Snippet }` with rkyv archive/serialize/deserialize derives, `PlainText` as the backward-compatible default, and `CompletionItem::with_snippet()` as the explicit builder. The item remains a validated, versioned server→client payload. The module authority doc prohibits snippets **with executable transforms** — inert snippet text carrying LSP syntax that the client expands locally is consistent with this boundary and adds no executable transform.

`CompletionRequest` carries `request_id`, `client_id`, `document_id`, `document_version`, `behavior_version`, `cursor_byte_offset`, `replacement_range: CompletionReplacementRange`, `trigger: CompletionTrigger`, and `provider_generation`. `CompletionResultSet` enforces `COMPLETION_RESULT_MAX_ITEMS`, per-field character caps (`COMPLETION_RESULT_MAX_ITEM_LABEL_CHARS`, `COMPLETION_RESULT_MAX_ITEM_INSERT_TEXT_CHARS`, `COMPLETION_RESULT_MAX_ITEM_DETAIL_CHARS`, `COMPLETION_RESULT_MAX_ITEM_COMMIT_CHARS`), and `COMPLETION_RESULT_PAYLOAD_BUDGET_BYTES` via `validate()` and `check_result_payload_budget()`; the existing `insert_text` cap applies equally to both text formats. `CompletionProviderGeneration` (a `u64`) is documented as "Bumped when providers are registered, disabled, revoked, or reloaded so in-flight work can be stale-dropped" — the disable concept is already anticipated by the protocol's own stale-drop contract.

`src/editor/snippet.rs` implements the bounded, language-neutral `parse_snippet` scanner. It expands `$1`..`$n`, final `$0`, `${1:default}`, `${1|a,b,c|}`, and `${1}` into `SnippetExpansion { text, placeholders }`; each `SnippetPlaceholder` stores a half-open byte range, numeric index, and final-tabstop flag. `SNIPPET_MAX_TABSTOPS` caps a snippet at 32 entries and expanded text is capped by `COMPLETION_RESULT_MAX_ITEM_INSERT_TEXT_CHARS`. Malformed/unterminated braced constructs, unsupported `${name}` variables, over-cap tabstops, and oversize text return typed `SnippetParseError` values. Backslash escapes, variable resolution, and nested placeholders are intentionally deferred because no first-party Phase 18.19 snippet needs them. The module is tested now and wired into completion accept by task 5.

### Provider registry and coordinator

`src/server/completion.rs::CompletionProviderMeta` is the server-side registration record (not rkyv — server-only): `id` (package-prefixed or `core.<name>`, must not claim `clay.*`), `provenance`, `priority: i32`, `exclusive: bool`, `trigger_metadata: CompletionTriggerMetadata`, `word_boundary: WordBoundaryRule`, `items: Vec<CompletionItem>` (bounded inert static items, "no callbacks, snippet transforms, commands, or external authority"), `timeout_ms`, `max_items`, and `generation`. Task 6 adds the inert `exclusive` flag; built-ins and omitted package fields default to `false`.

`CompletionCoordinator` owns the cancellable UI-reactive priority lane: `schedule_completion` aborts/stale-drops older in-flight requests; `list_ordered` produces a deterministic priority-descending then ID-ascending iterator; `providers_for_trigger_character` selects matching providers; `bump_generation`/`remove_older_generations` drop older-generation providers; `cancel_package` and `cancel_generation` cancel active work for a package prefix or generation. `BufferWordCompletionProvider::meta` registers the built-in `core.bufferWords` native base provider.

### ClayOpState provider listing

`src/server/ops/mod.rs::ClayOpState` stores providers as a flat `Mutex<Vec<CompletionProviderMeta>>` (`completion_providers`), disabled targets in `disabled_completion_providers: Mutex<BTreeSet<String>>`, and the current `completion_provider_generation`. `completion_providers_for_trigger` filters exact provider IDs and package names/API prefixes through shared `completion_provider_is_disabled`, sorts by priority descending then ID ascending, and calls `apply_exclusive_suppression` before cloning selected metadata. The untriggered `completion_providers` snapshot applies the same disabled filter, so runtime publication cannot reintroduce disabled static providers. `CompletionProviderRegistry::providers_for_trigger_character` — the registry selection path owned by `CompletionCoordinator` — applies the same matcher before exclusive suppression, and scheduling refuses a disabled provider. `register_completion_provider_metadata` rejects duplicate IDs and stamps newly registered metadata with the active generation.

### Accept path and transient menu

`src/shell/transient_menu.rs::CompletionMenuAcceptAction` carries `request_id`, `document_id`, `document_version`, `behavior_version`, `replacement_range`, `insert_text`, `text_format`, and `commit_characters` as inert data; `completion_item_to_menu_item` copies the format from the validated `CompletionItem`. `src/editor/surface/mod.rs::accept_completion_with_event` preserves the exact plain-text path. For `Snippet`, it calls bounded `parse_snippet`, converts relative placeholder byte ranges to document offsets, appends any inert commit character after expansion, and applies the expanded text through the same `EditOperation::Replace`/`finish_edit_with_operation` optimistic-edit path. No provider code, direct IPC/op, JavaScript, filesystem, or network work runs on accept.

`EditorSurface` owns one client-local `SnippetSession { placeholders, active_index }`. Acceptance sorts tabstops numerically with `$0` last and selects the first non-final placeholder through existing `CursorState`/`SelectionState`; no widget or decoration layer was added. Tab and Shift-Tab navigate the bounded list, reaching `$0` (or advancing past the last placeholder when `$0` is absent) ends the session, and Escape exits without a document edit. Edits inside the active placeholder shift its end and later absolute ranges; edits outside it and explicit caret/selection movement cancel the session rather than retaining stale offsets. `load_snapshot` also clears transient session state.

### Package completionProviders descriptor

`src/packages/record/mod.rs::CompletionItemContributionDescriptor` normalizes each package `completionProviders[].items` entry from either a backward-compatible non-empty string or `{ label, insertText, detail?, textFormat?: "plainText"|"snippet" }`. It enforces label/insert/detail character caps, unique labels, exact object fields, and one text format per provider; plain and snippet items must use separate providers. `src/server/ops/completion.rs::completion_provider_metas` maps this data generically to `CompletionItem`, preserving `detail`, `insert_text`, `text_format`, and provenance. The provider descriptor also carries `id`, `priority`, `exclusive`, trigger/boundary metadata, timeout, and item caps. No language ID enters parser or mapping code.

### Clay JS facade and permissions

`runtime/js/completion.js` exposes `serverRegisterCompletionProvider`, `serverDisableCompletion`, `serverListCompletionProvidersForTrigger`, and `completionTriggerCharactersFromEditorRules`. The register facade checks prohibited authority fields (`handler`, `callback`, `complete`, `function`, `clientJavaScript`, `nativeHandle`, `rawOps`, `module`) before delegating to `op_clay_completion_register_completion_provider`. The disable facade accepts exactly one non-empty `provider` or `packagePrefix`; facade and Rust op reject every extra field, and the op bounds targets at 128 characters. Completion providers declare only the `completion-provider` permission; disabling needs no package permission and no completion path grants filesystem, network, shell, AI, LSP, workspace, raw-op, or client-JavaScript authority.

### Docs registry and wiki coverage

`docs/reference/primitives/registry.md` records `CompletionTriggerAndResult` as a server→client primitive with `UiReactivePriority` cancellable result fetch and `TransientMenuSession` display/accept. `docs/wiki/modules/phase18.11-completion-provider-primitive-review.md` and `phase18.18-language-package-primitive-review.md` inventory the framework and the historical Phase 18.18 keyword-only base providers; tasks 4-8 add snippets end-to-end. Package implementation pages document the shipped Rust/TypeScript sets. Authoritative Clay JS API descriptor documentation and generated registry updates remain assigned to the later Phase 18.19 docs/API tasks.

## Generic Phase 18.19 Gaps

### Snippet `text_format`, bounded parser, and client-local session (implemented in tasks 4-5)

Task 4 implements `text_format: CompletionItemTextFormat { PlainText, Snippet }` (default `PlainText`, rkyv-archived) and bounded `src/editor/snippet.rs::parse_snippet`; task 5 wires both through `CompletionMenuAcceptAction` into `accept_completion_with_event`. `PlainText` keeps the previous verbatim `Replace` behavior. `Snippet` expands tabstops/placeholders/choices locally, applies expanded text through the existing Replace/optimistic-event path, and installs bounded `SnippetSession` state using existing caret/selection primitives. Tab/Shift-Tab navigate numerically, `$0` is final, Esc exits, active-placeholder edits shift later ranges, and unrelated movement/editing cancels stale state. The parser still enforces the existing insert-text character cap and `SNIPPET_MAX_TABSTOPS = 32`; variable resolution remains deferred.

Rejected implementation: a separate `snippet_text: Option<String>` field (duplicates `insert_text` and forces two-field consistency); a separate snippet provider subsystem/registry (the roadmap fixes snippets as a `CompletionProvider` variant, not a separate subsystem); server-side snippet expansion (adds a round trip and moves authority); a snippet-specific Masonry widget (placeholder highlighting reuses existing decoration/selection primitives).

### Opt-in `exclusive` claim and shared suppression helper (implemented in task 6)

`CompletionProviderMeta` carries `exclusive: bool` (default `false`) and the package `completionProviders` descriptor accepts `exclusive?: bool`, rejecting non-booleans at package validation. Shared `apply_exclusive_suppression` receives the already priority-descending/ID-ascending matching references. If any provider in the highest-priority tier is exclusive, it retains that whole tier and drops strictly lower priorities; otherwise all matches remain (priority-merge default unchanged). Both `ClayOpState::completion_providers_for_trigger` and the coordinator-owned `CompletionProviderRegistry::providers_for_trigger_character` call this helper. Lower-priority exclusive providers cannot claim a request, equal-priority peers remain, no provider executes during filtering, and no wire-protocol field was added.

Rejected implementation: a per-language or mode-specific Rust branch; adding an active-major-mode field to `CompletionRequest` (deferred — the server already knows the document's active mode; add explicit `(package_prefix, mode)` scoping only if Phase 18.21 LSP requires finer control); granting `exclusive` any authority (it only suppresses other providers' results for a request).

### `serverDisableCompletion` Clay JS API and disabled-provider set (implemented in task 7)

`ClayOpState` now owns `disabled_completion_providers: Mutex<BTreeSet<String>>` plus a monotonically increasing completion-provider generation. `completion.serverDisableCompletion` (op `op_clay_completion_disable`) accepts exactly one provider ID or package prefix, records it idempotently, and bumps the generation only on first insertion. Existing and subsequently registered metadata is stamped with the current generation. Shared `completion_provider_is_disabled` filtering matches exact IDs, `provenance.package_prefix` (for example `rust`), or `provenance.package_name` (for example `@clay/rust`) before listing/runtime snapshot publication. Disabling `core.bufferWords` or `@clay/rust` therefore removes the targeted metadata without deleting registration records.

The coordinator-owned registry keeps the same disabled-target set semantics. `CompletionCoordinator::disable_completion(target, generation)` prevents trigger selection and direct scheduling of disabled providers, advances known document generations, and aborts older in-flight tasks through the existing cancellation path. Reload creates a fresh runtime/registry generation, which is the intentionally minimal re-enable path. The API follows the `server*` naming convention of sibling completion APIs because it mutates server-authoritative provider state.

Rejected implementation: overloading the generic `setPackageOption` configuration API (hides completion-specific semantics and cannot target arbitrary provider IDs); a matching `serverEnableCompletion` (YAGNI — re-enable happens via package reload/generation; deferred); granting `disableCompletion` any authority beyond suppressing already-registered metadata.

### Structured item descriptor and first-party snippet sets (implemented in task 8)

The package descriptor accepts strings as plain text and structured `{ label, insertText, detail?, textFormat? }` objects as normalized bounded items. Invalid formats, unknown object fields, oversized fields, duplicate labels, and mixed plain/snippet providers fail package assembly. `serverListCompletionProvidersForTrigger` returns structured items with `label`, `insertText`, `detail`, and `textFormat`, making snippet metadata observable without exposing execution authority.

`@clay/rust` ships priority-0 `rust.snippets` (`fn`, `match`, `impl`) beside `rust.keywords`; `@clay/typescript` ships priority-0 `typescript.snippets` (`interface`, `type`) beside `typescript.keywords`. Existing load entries register both contributions from the same one-line `loadPackage` path and unchanged `completion-provider` permission. `static_package_completion_result` now merges every matching provider after shared priority/exclusive selection, prefix-filters each provider's items, and enforces total result count/payload budgets. Item provenance remains per provider; no package JavaScript runs during requests. The Rust `fn` body is exercised through local expansion and Tab navigation to prove package syntax fits the generic client session.

## What Existing Primitives Already Achieve

Without new scheduling, package execution authority, rendering authority, or client JavaScript, Clay can already:

- register package-prefixed completion providers with priority, triggers, word boundaries, bounded static items, timeout, and item caps through `serverRegisterCompletionProvider`;
- resolve completion requests on a cancellable UI-reactive priority lane that aborts/stale-drops superseded work and validates results against the current document/behavior version;
- deterministically order matching providers by priority then ID and merge their results;
- disable an exact provider or every provider from a package name/API prefix, bump provider generation, and invalidate older in-flight coordinator work;
- accept a completion item as a local `EditOperation::Replace` over the replacement range with no provider code and no IPC;
- enforce per-field and payload budgets on every result item;
- list providers for a trigger through `serverListCompletionProvidersForTrigger`;
- ship priority-0 keyword base providers per language (Phase 18.18) and dedicated Rust/TypeScript snippet providers (task 8).

Tasks 4-8 therefore implement snippet text-format/parser/session/data, one shared suppression helper, and a disabled-provider set + disable op without adding a completion subsystem, menu widget, language branch, or authority.

## Data Flow and Reuse Rule

```text
package completionProviders descriptor (items: string | { label, insertText, textFormat, detail? }, exclusive?)
  -> completion_provider_metas -> CompletionProviderMeta { items: Vec<CompletionItem { text_format }>, exclusive }
  -> ClayOpState.completion_providers (flat Mutex<Vec<..>>)
  -> completion_providers_for_trigger:
       filter disabled (ID + package-prefix) -> apply_exclusive_suppression -> priority-then-ID order
  -> static_package_completion_result merges selected provider items within total count/payload budgets
  -> CompletionResultSet::validate (per-field + payload budgets, incl. snippet insert_text)
  -> rkyv transport -> TransientMenuSession display
  -> accept_completion_with_event:
       PlainText -> EditOperation::Replace (unchanged)
       Snippet    -> parse_snippet (bounded) -> Replace expanded text + install SnippetSession (Tab/Shift-Tab/Esc)

serverDisableCompletion -> disabled set + generation bump -> filtered snapshots/listings
CompletionCoordinator::disable_completion -> same target filter + abort older-generation tasks
No language name enters Rust completion/accept branches. No provider code runs on accept.
```

Future first-party snippet additions and Phase 18.21 LSP enrichment reuse this pipeline; they do not add a per-language Rust completion branch, a parallel snippet registry, a client widget, or a language-specific accept path.

## Hot-Path Classification

| Work | Allowed location |
| --- | --- |
| Snippet `parse_snippet` (tabstop/placeholder/choice expansion) | client accept path only; bounded synchronous Rust text op, no IPC, no provider code |
| Snippet session Tab/Shift-Tab/Esc navigation | client-local transient state; no server work for plain caret moves within placeholders |
| `apply_exclusive_suppression` and disabled-provider filtering | in-memory metadata filter within the existing selection pass; no extra scheduling, no provider execution |
| `serverDisableCompletion` op + generation bump | load/config-time metadata work; reuses `bump_generation` stale-drop |
| Provider registration, descriptor validation, structured-item normalization | package load time only |
| Protocol decode, version/source replacement, cache pruning | client event application before paint |
| Masonry paint, layout, keypress, pointer, scroll, text-event handlers | installed inert completion items, cached snippet session state, and the existing behavior manifest only |

No package JavaScript, snippet parsing, IPC, server validation, completion computation, or configuration evaluation belongs in Masonry paint, layout, keypress, pointer, scroll, or text-event handlers. Snippet expansion on accept is a bounded synchronous Rust text op, not a server round trip. `COMPLETION_RESULT_PAYLOAD_BUDGET_BYTES`, `COMPLETION_RESULT_MAX_ITEMS`, the per-field character caps, `BEHAVIOR_MANIFEST_PAYLOAD_BUDGET_BYTES`, and `KEYPRESS_TO_LOCAL_PAINT_P95_BUDGET_MS` remain applicable; snippet expanded text is bounded by the existing insert-text cap.

## Security and Authority Boundary

Snippet syntax, the `exclusive` flag, and `disableCompletion` are inert metadata and a user-side toggle. This phase adds no filesystem, network, shell, AI, workspace mutation, language-server subprocess, native-ui, package-control, package-manager, raw-ops, client-runtime, raw CSS, Vello/Parley callback, native handle, or arbitrary WASM authority. Specifically:

- Snippets are inert text expanded by a bounded Rust parser in the client; they carry no executable transforms, no command side effects on accept, no raw op names, no native handles, no CSS, no file paths, no shell/network/AI directives, and no client-side JavaScript. This is consistent with the existing `CompletionItem` authority doc, which prohibits snippets **with executable transforms** — inert snippet text has none.
- `exclusive` grants no authority; it only suppresses other providers' results for a request. A package cannot suppress providers it does not own priority over; reserved `clay.*`/`core.*` namespaces remain protected.
- `serverDisableCompletion` grants no authority; it only suppresses already-registered provider metadata. It cannot register providers, escalate priority, grant `exclusive`, or bypass permissions.

No new `PackagePermission` and no new decision log are required for this phase (the authority boundary is unchanged). If implementation discovers a need for new authority, stop and open a decision log before proceeding.

## Rejected Implementation Shapes

- Do not add a `snippet_text: Option<String>` field on `CompletionItem`; reuse `insert_text` with a `text_format` discriminant.
- Do not add a separate snippet provider subsystem, snippet registry, or snippet-specific Masonry widget; snippets are a `CompletionProvider` variant riding the existing `CompletionItem`/accept path.
- Do not expand snippets server-side or run provider code on accept; expansion is a bounded client-local Rust text op.
- Do not add a per-language or mode-specific Rust branch for exclusive suppression or disable filtering; one shared helper over the priority-ordered list serves all providers.
- Do not overload `setPackageOption` for completion disabling; use a dedicated `completion.serverDisableCompletion` API.
- Do not grant `exclusive` or `disableCompletion` any authority beyond suppressing already-registered metadata.
- Do not run package JavaScript, snippet parsing, IPC, server validation, completion computation, or configuration evaluation in Masonry paint/layout/input paths.
- Do not implement LSP process spawning, hover, go-to-definition, code actions, or rename in Phase 18.19; those are Phase 18.20/18.21.

## Tests

- `src/protocol/completion.rs` unit tests: plain-text and snippet `CompletionItem` rkyv round trips; `PlainText` default; snippet validation; oversized snippet `insert_text` rejection.
- `src/editor/snippet.rs` unit tests: verbatim text, bare-dollar literals, tabstops, placeholders, first-choice expansion, empty/braced tabstops, malformed/unterminated syntax, deferred variable rejection, 32-tabstop cap, and expanded-text cap.
- `src/editor/surface/mod.rs` unit tests: plain-text compatibility; snippet expansion and first-placeholder selection; Tab/Shift-Tab/final-tabstop lifecycle; Escape without edit; active-placeholder edit range shifting.
- `src/shell/transient_menu.rs` unit test: completion item `text_format` survives projection into `CompletionMenuAcceptAction`.
- `tests/editor_performance_invariants.rs::snippet_accept_is_bounded_client_local_text_work`: locks `parse_snippet` + existing `finish_edit_with_operation` reuse and rejects direct op/enqueue, JavaScript, filesystem, shell, or network work in accept/parser bodies.
- `src/server/completion.rs` unit tests: non-exclusive priority merge, highest-tier exclusive suppression with equal-priority retention, and lower-priority exclusive non-claim.
- `src/server/ops/completion.rs` unit tests: package descriptor boolean validation/default, descriptor-to-meta propagation, and coordinator/`ClayOpState` selection parity.
- `src/server/js_runtime/mod.rs` completion facade tests: registration/listing preserve `exclusive: true`; disable returns its idempotence/generation result, filters trigger/runtime snapshots, and rejects empty, ambiguous, or authority-bearing input.
- `src/server/ops/completion.rs` unit tests: exact native-provider disable bumps generation and retains a stamped peer; package-name disable removes every provider from runtime/trigger snapshots.
- `tests/completion_provider.rs::disabling_provider_invalidates_in_flight_generation_and_blocks_reschedule`: coordinator disable aborts old-generation work, publishes no stale result, and refuses rescheduling the disabled provider.
- `tests/completion_provider.rs::first_party_rust_and_typescript_packages_ship_dedicated_snippet_providers`: locks exact provider IDs/labels, snippet format, details, final tabstops, priorities, and item budgets from real package manifests.
- `src/server/connection/mod.rs::static_package_completion_merges_equal_priority_plain_and_snippet_providers`: locks live static-provider merge and final result validation.
- `src/server/js_runtime/mod.rs::language_package_completion_trigger_metadata_is_queryable`: loads packages through `loadPackage`, observes both snippet providers and structured `textFormat: "snippet"` items through the public listing facade.
- `tests/package_primitive_gate.rs`: locks string compatibility plus structured-item validation, mixed-format rejection, and insert-text budget enforcement.
- Documentation structure and discoverability use generic `tests/primitives_docs.rs` inventory/wiki validators; executable tests remain authoritative for behavior instead of phase-specific prose needles.
- Implementation coverage (later tasks): `tests/clay_js_api_inventory.rs`/`tests/clay_js_doc_registry.rs` (authoritative descriptor and `serverDisableCompletion` docs/registry coverage).

Run:

```bash
cargo test --test protocol primitives_docs::
```

## Related

- [Primitive Architecture](primitive-architecture.md)
- [Phase 18.11 Completion Provider Framework Primitive Review](phase18.11-completion-provider-primitive-review.md)
- [Phase 18.18 First-Party Language Package Full Implementation Primitive Review](phase18.18-language-package-primitive-review.md)
- [Transient Menu and Command Execution Primitive Review](phase18.8-transient-menu-command-execution-primitive-review.md)
- [First-Party Language Packages](first-party-language-packages.md)
- [Primitive Registry](../../reference/primitives/registry.md)
