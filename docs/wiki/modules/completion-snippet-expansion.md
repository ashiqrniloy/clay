# Completion Snippet Expansion

## Source

- `src/editor/snippet.rs` — bounded LSP snippet parser
- `src/editor/surface.rs` — `EditorSurface` snippet session and accept branching
- `src/masonry_editor.rs` — snippet-aware keyboard routing
- `src/protocol/completion.rs` — `CompletionItemTextFormat` and `text_format` field
- `src/server/completion.rs` — `CompletionProviderMeta::exclusive` and `apply_exclusive_suppression`
- `src/server/ops/completion.rs` — `op_clay_completion_disable`
- `src/server/ops/mod.rs` — `ClayOpState::disabled_completion_providers` and generation counter
- `runtime/js/completion.ts` — `serverDisableCompletion` JS facade
- `tests/completion_provider.rs` — snippet provider integration tests
- `tests/primitives_docs.rs` — deterministic primitive docs test

## Overview

Phase 18.19 extends the Phase 18.11/18.18 completion framework with three capabilities: inert snippet items with client-local expansion, opt-in exclusive provider claim with priority suppression, and a `serverDisableCompletion` configuration API with disabled-provider filtering. All three ride existing primitives — `CompletionItem`, `CompletionProviderMeta`, `TransientMenuSession`, `EditorSurface`, `ClayOpState`, and the `server*` facade convention — with no new subsystem, permission, or hot-path JS.

## What It Does

### Snippet text-format kind

`CompletionItemTextFormat` (`src/protocol/completion.rs`) is a two-variant `rkyv`-serializable enum:

```rust
pub enum CompletionItemTextFormat { PlainText, Snippet }
```

`CompletionItem` gained a `text_format: CompletionItemTextFormat` field (defaults to `PlainText`) and a `with_snippet()` builder. All existing literal `CompletionItem` construction sites were updated.

`CompletionMenuAcceptAction` (`src/shell/transient_menu.rs`) now carries `text_format: CompletionItemTextFormat` threaded from `CompletionItem` through `completion_result_to_menu_session`. When `text_format` is `Snippet`, the accept path in `EditorSurface::accept_completion_with_event` branches to parse the `insert_text` as LSP snippet syntax via `parse_snippet`, installs a `SnippetSession` on the surface, and produces a single `Replace` edit with the expanded text. `PlainText` items follow the original inert text-replacement path unchanged.

### Bounded snippet parser (`src/editor/snippet.rs`)

`parse_snippet(input: &str) -> Result<SnippetExpansion, SnippetParseError>` is a client-local, allocation-bounded parser that handles:

- **Bare tabstops** `$1` through `$N` — emitted as zero-width placeholders when no default is present
- **Braced tabstops** `${1}` — same as bare tabstops with brace delimiters
- **Placeholders** `${1:default text}` — emitted with `default` as the replacement text and the placeholder range
- **Choices** `${1|a,b,c|}` — first option inserted as the default
- **Final tabstop** `$0` or `${0}` — always sorted last in the placeholder list

The parser is bounded at every allocation point:
- Input capacity is capped at `COMPLETION_RESULT_MAX_ITEM_INSERT_TEXT_CHARS * 4` (bounded initial allocation)
- Each character insertion checks against `COMPLETION_RESULT_MAX_ITEM_INSERT_TEXT_CHARS` (256) via `push_text_char`, rejecting `ExpandedTextTooLong` before allocation grows
- `SNIPPET_MAX_TABSTOPS` (32) limits the total placeholder count, rejecting `TooManyTabstops`

Deferred syntax (not yet needed by first-party snippets): backslash escapes, variables (`$name` / `${name}`), nested placeholders inside defaults, mirror transforms (same placeholder appearing multiple times). Each deferred item is documented with a `ponytail:` comment naming the ceiling and upgrade path.

`SnippetExpansion` carries:
- `expanded_text: String` — the text to insert (defaults in place, tabstop markers removed)
- `placeholders: Vec<SnippetPlaceholder>` — ordered list of `{ byte_start: usize, byte_end: usize, final_tabstop: bool }` for Tab/Shift-Tab navigation

### Snippet session (`EditorSurface`)

`SnippetSession { placeholders: Vec<SnippetPlaceholder>, active_index: usize }` is a private `Option<SnippetSession>` on `EditorSurface`. Installation during `accept_completion_with_event`:

1. `parse_snippet` produces expanded text and raw placeholder offsets relative to zero
2. Placeholder byte offsets are adjusted relative to `replacement_range.start`
3. A single `EditOperation::Replace` inserts the expanded text
4. Placeholders are sorted by `(final_tabstop, index, byte_start)` — non-final tabstops first, then `$0` (final)
5. `active_index` is set to the first non-final placeholder; when none exist, session ends immediately

Navigation:
- **Tab** → `select_next_snippet_placeholder`: moves `active_index` forward; if at last placeholder or none remain, ends the session
- **Shift-Tab** → `select_previous_snippet_placeholder`: moves `active_index` backward; wraps to first
- **Escape** → clears the session without producing an edit event

Selection after navigation uses existing `CursorState`/`SelectionState`: caret at `byte_start` for zero-width placeholders, or a selection from `byte_start` to `byte_end` when a default is present.

Session lifetime:
- **Edit inside active placeholder** → `update_snippet_session_after_edit` shifts all later placeholder byte ranges by the delta between replacement and original lengths (checked arithmetic with `shift_snippet_offset`)
- **Edit outside active placeholder, caret move, selection change, or `load_snapshot`** → session is cancelled (cleared to `None`)

### Masonry editor routing

`route_key_with_event` (`src/masonry_editor.rs`) and `local_key` intercept Tab/Shift-Tab/Escape when a snippet session is active before falling through to normal menu/editor routing. `has_active_snippet_session()` gates the interception; when a session is active, Escape routes to `local_key` (which clears the session) instead of submitting `ExitRequested`.

### Exclusive claim and provider suppression

`exclusive: bool` (default `false`) on `CompletionProviderMeta` and the package `CompletionProviderContributionDescriptor`. `apply_exclusive_suppression` (`src/server/completion.rs`) is a shared free function that receives priority-descending, ID-ascending matched provider references:

- If any provider in the highest-priority tier has `exclusive = true`, retain that whole tier and drop all strictly lower priorities
- Equal-priority peers remain regardless of exclusive status
- A lower-priority exclusive provider cannot claim a request whose top tier is non-exclusive
- Lists with no exclusive provider pass through unchanged

The helper is wired into all three selection paths: `ClayOpState::completion_providers_for_trigger` (`src/server/ops/mod.rs`), `CompletionProviderRegistry::providers_for_trigger_character`, and the static package completion result merge in `src/server/connection.rs`. No provider execution occurs during exclusive filtering.

### serverDisableCompletion and disabled-provider filtering

`disabled_completion_providers: Mutex<BTreeSet<String>>` on `ClayOpState` (`src/server/ops/mod.rs`) records provider IDs and package prefixes to suppress. `completion_provider_generation: Mutex<CompletionProviderGeneration>` is a monotonic counter incremented by each disable call.

`op_clay_completion_disable` (`src/server/ops/completion.rs`) validates a JSON options object with exactly one non-empty `"provider"` or `"packagePrefix"` key (at most 128 chars), records the target in the disabled set, bumps the generation counter, stamps all existing provider metas with the new generation, and returns `{ target, disabled, providerGeneration }`.

The disabled set is consulted by `completion_provider_is_disabled` (`src/server/completion.rs`) and applied as a filter in every selection path. Disabled state persists across `begin_evaluation` runtime reloads; the reload path stamps new metadata with the current generation from the surviving counter. Re-enabling requires a package reload or runtime restart.

The JS facade `serverDisableCompletion` (`runtime/js/completion.ts`) validates exactly one non-empty target before delegating to the op; the embedded `CLAY_FACADE_COMPLETION` constant in `src/server/js_runtime.rs` mirrors this facade for configuration-module loading.

### First-party snippet providers

`@clay/rust` ships `rust.snippets` with three `fn`/`match`/`impl` templates at priority 0 alongside `rust.keywords`. `@clay/typescript` ships `typescript.snippets` with `interface`/`type` templates at priority 0. Both use `textFormat: "snippet"` in their structured `CompletionItemContributionDescriptor` items, share existing trigger characters and permission, and load through the same one `serverRegisterCompletionProvider({ packageManifest })` call that maps the full `completionProviders` array.

## How It Works

### Accept flow (snippet path)

```
typed completion request → provider selection → result → TransientMenuSession
    → activate_menu_selection → menu_activate_completion
    → CompletionMenuAcceptAction { text_format: Snippet, insert_text, ... }
    → EditorSurface::accept_completion_with_event
        → if text_format == Snippet:
            → parse_snippet(insert_text) → SnippetExpansion
            → adjust placeholder offsets += replacement_range.start
            → finish_edit_with_operation(Replace { expanded_text })
            → install_snippet_session(placeholders)
```

### Exclusive suppression flow

```
completion request trigger →
    providers_for_trigger_character (registry or ClayOpState)
    → filter by matching trigger character
    → sort by priority descending, ID ascending
    → apply_exclusive_suppression(matched)
        → find highest priority tier
        → if any in tier has exclusive: retain tier, drop lower priorities
    → disabled filter (completion_provider_is_disabled)
    → schedule/merge remaining providers
```

### Disable flow

```
serverDisableCompletion({ provider: "core.bufferWords" })
    → op_clay_completion_disable → ClayOpState::disable_completion
        → insert "core.bufferWords" into disabled_completion_providers
        → increment completion_provider_generation
        → stamp all provider metas with new generation
        → CompletionCoordinator::disable_completion
            → abort in-flight tasks with old generation
            → stale-drop published results
```

## Invariants and Constraints

- Snippet accept is client-local Rust only; no provider code runs on accept, no JS, no IPC.
- `parse_snippet` is allocation-bounded at every character insertion; no input length can force unbounded allocation.
- `SnippetSession` is transient editor surface state cleared by `load_snapshot`, caret moves, selection changes, or out-of-bounds edits.
- `exclusive` is inert metadata consulted at selection time; equal-priority peers are never dropped by a peer's exclusive flag.
- `serverDisableCompletion` suppresses providers only; it does not remove registration metadata, mutate package state, or grant authority.
- Disabled state persists across runtime reloads; the reload path stamps fresh metadata with the current generation from the surviving counter.
- Structured `CompletionItemContributionDescriptor` items are validated at load time: per-field char caps, `textFormat` must be `"plainText"` or `"snippet"`, mixing plain and snippet items in one provider is rejected.
- No per-language Rust branch exists in completion registration or selection.

## Security and Authority Boundary

- Snippet bodies are inert LSP placeholder syntax expanded client-local; they carry no executable transforms, callbacks, commands, or provider code.
- `exclusive` and `serverDisableCompletion` grant no filesystem, network, shell, extension loading, AI mutation, workspace, package, WASM, client-side JavaScript, raw-op, native-widget, or package-manager authority.
- `serverDisableCompletion` requires no `clay.contributions.permissions` entry.
- The authority boundary on `CompletionItem` remains: items are inert text-replacement data — snippets with inert placeholder syntax are data, not executable transforms.

## Tests

- `src/editor/snippet.rs`: 10 unit tests covering bare tabstops, braced tabstops, placeholders with defaults, choices, final tabstop ordering, unterminated brace error, unsupported variable error, malformed input error, expanded-text-too-long rejection, too-many-tabstops rejection
- `src/protocol/completion.rs`: `CompletionItemTextFormat` rkyv round-trip, `CompletionItem::new` defaults to `PlainText`
- `src/editor/surface.rs`: snippet accept selects first placeholder, Tab/Shift-Tab navigation, Escape exits, active-placeholder editing shifts later ranges, manual completion routing
- `tests/completion_provider.rs`: first-party snippet providers end-to-end, exclusive claim selection, disable filtering and generation bump, stale-drop on disable
- `tests/editor_performance_invariants.rs`: snippet accept hot-path guard (no Deno.core, op_clay_, enqueue_, std::fs, TcpStream, reqwest, ureq)
- `tests/package_primitive_gate.rs`: structured item validation (valid, mixed-format rejection, invalid textFormat, oversized insertText)
- `tests/primitives_docs.rs`: Phase 18.19 primitive review linked and complete

Run with:

```text
cargo test --lib snippet --quiet
cargo test --lib surface --quiet
cargo test --test completion_provider --quiet
cargo test --test editor_performance_invariants --quiet
cargo test --test primitives_docs phase18_19 --quiet
```

## Related

- [Transient Menu Session](transient-menu-session.md) — completion accept path and `CompletionMenuAcceptAction`
- [First-Party Language Packages](first-party-language-packages.md) — `rust.snippets` / `typescript.snippets` providers
- [Phase 18.19 Completion Extensions Primitive Review](phase18.19-completion-extensions-primitive-review.md)
- [Phase 18.11 Completion Provider Framework Primitive Review](phase18.11-completion-provider-primitive-review.md)
- [Embedded JavaScript Runtime](embedded-js-runtime.md) — JS facade and op extension registration
- [Completion Provider API Reference](../../reference/clay-js-api/completion/server-register-completion-provider.md)
- [Disable Completion API Reference](../../reference/clay-js-api/completion/server-disable-completion.md)
