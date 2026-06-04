# Markdown Mode Activation, Commands, Key Bindings, and Behavior-Manifest Transforms

## What It Does

The Markdown mode activation layer proves that a first-party language-mode package can
express all of its editing behavior, commands, and key bindings through the existing
generic protocol primitives — **with zero mode-specific logic in Rust**.

## Key Files

| File | Role |
|---|---|
| `src/protocol/mod.rs` | `EnterRule::ContinueLineMarkers`, `EnterRule::PreserveFenceBodyIndent` — generic variants usable by any mode; `PairRule` already supports multi-char delimiters |
| `src/server/ops/modes.rs` | `op_clay_modes_activate_major_mode` — decodes package-supplied `editorRules`, `commands`, and `keymaps` JSON into generic protocol types; publishes updated `BehaviorManifest` |
| `src/server/ops/mod.rs` | `ClayOpState::publish_mode_behavior_manifest` — mode-agnostic manifest publisher (combines default manifest + package editor rules + extra commands/keymaps + validation) |
| `packages/markdown/dist/load.js` | **All Markdown-specific knowledge**: declares `editorRules` with `continueLineMarkers` and specific markers/pairs, declares commands, declares key bindings |
| `packages/markdown/dist/index.js` | Package contract, mode metadata, command/transform declarations |
| `packages/markdown/dist/parser.js` | Package-owned markdown-it token-stream parser/decorator adapter that maps block tokens, inline child tokens, and package-owned source/line indexes to Clay decoration spans |
| `packages/markdown/package.json` | Package metadata with contributions (commands, keyRouting, textTransforms, sdui, decorations) |
| `tests/markdown_mode.rs` | Integration tests for mode activation plus parser-adapter package boundary checks |

## Architecture

### The boundary: package JS owns mode logic; Rust owns generic primitives

```
┌─────────────────────────────────────────────┐
│ @clay/markdown JS package (dist/load.js)    │
│                                             │
│  const MARKDOWN_EDITOR_RULES = {            │
│    enter: {                                 │
│      kind: "continueLineMarkers",           │  ← Markdown-specific knowledge
│      markers: ["-", "*", "+", "ordered-dot"]│     (which markers, exit behavior)
│    },                                       │
│    pairs: [{open:"**", close:"**"}, ...]    │  ← Markdown-specific delimiters
│  }                                          │
│                                             │
│  clay.modes.serverActivateMajorMode({       │
│    editorRules: MARKDOWN_EDITOR_RULES,      │  ← passed as inert JSON
│    commands: [...], keymaps: [...]          │
│  })                                         │
└──────────────────┬──────────────────────────┘
                   │ JSON via deno_core op
┌──────────────────▼──────────────────────────┐
│ src/server/ops/modes.rs                     │
│                                             │
│  parse_editor_rules(json) →                 │
│    EditorBehaviorRules {                    │  ← generic struct, no mode name
│      enter: EnterRule::ContinueLineMarkers  │
│        { markers, exit_on_empty_item }      │  ← generic variant
│      pairs: [PairRule {open:"**",..}, ..]   │  ← generic struct
│    }                                        │
│                                             │
│  No "markdown" string appears in validation.│
│  Any mode can pass its own markers/pairs.   │
└──────────────────┬──────────────────────────┘
                   │
┌──────────────────▼──────────────────────────┐
│ src/protocol/mod.rs                         │
│                                             │
│  EnterRule::ContinueLineMarkers {           │  ← generic — works for Org-mode,
│    markers: Vec<String>,                    │     RST, AsciiDoc, any mode with
│    exit_on_empty_item: bool                 │     list continuation
│  }                                          │
│                                             │
│  EnterRule::PreserveFenceBodyIndent {       │  ← generic — works for any mode
│    fence_markers: Vec<String>               │     with fenced constructs
│  }                                          │
└─────────────────────────────────────────────┘
```

### How any other mode would use the same primitives

An AsciiDoc package would pass:

```json
{
  "editorRules": {
    "enter": {
      "kind": "continueLineMarkers",
      "markers": ["-", "*", "**", "::", "."],
      "exitOnEmptyItem": true
    },
    "pairs": [
      { "open": "**", "close": "**" },
      { "open": "__", "close": "__" },
      { "open": "`",  "close": "`"  }
    ]
  }
}
```

An RST package would pass its own comment characters:

```json
{
  "editorRules": {
    "comments": [
      { "linePrefix": "..", "continuePrefix": "  " }
    ],
    "enter": { "kind": "preserveLeadingWhitespace" }
  }
}
```

**No new Rust code needed for any of these.** The JS package is the sole source of
mode-specific knowledge.

## Generic protocol types extended

| Type | New variants | Why generic |
|---|---|---|
| `EnterRule` | `ContinueLineMarkers { markers, exit_on_empty_item }` | Any mode with list continuation (Markdown, Org, RST, AsciiDoc...) |
| `EnterRule` | `PreserveFenceBodyIndent { fence_markers }` | Any mode with fenced code blocks (Markdown, RST...) |
| `PairRule` | (unchanged) — `open`/`close` always accepted any string | Multi-char delimiters (`**`, `__`, `` ` ``) work out of the box |

## Invariants

- **No mode-specific types in Rust.** `EnterRule` variants use language-agnostic names.
- **No mode-specific branching.** `ModeRegistry::select_behavior_manifest_for_document` stays completely generic — no `if mode_id == "markdown"` branch.
- **No Markdown-named fields on `EditorBehaviorRules`.** The old `markdown_transforms: Option<MarkdownTransformRules>` field has been removed.
- **JS package owns all Markdown-specific data.** Marker strings, pair delimiters, command IDs, key bindings, parser adapter path, and markdown-it SDUI parse status are declared in `dist/load.js`, `dist/parser.js`, and `dist/sdui.js`.
- **Editor ops are JSON-in, generic-types-out.** `parse_editor_rules`, `parse_enter_rule`, `parse_pair_rule` handle JSON → `EnterRule`/`PairRule` with no mode awareness.

## Security

- Package JS runs only at `loadMarkdownPackage()` call time — never from typing/paint handlers.
- The `editorRules` JSON parser rejects executable field names (`callback`, `code`, `javascript`, `hook`).
- `EnterRule` variants contain only inert data (`Vec<String>`, `bool`) — no closures, no raw ops.
- Commands carry `ServerIntent` authority and `ServerFirst` routing — no `BuiltInClientEdit`.

## Tests

| Test | Validates |
|---|---|
| `markdown_classifies_supported_extensions_and_mime` | All four patterns match; unknown extension rejected |
| `markdown_mode_installs_behavior_manifest_atomically` | Per-document manifest with correct scope/version/provenance |
| `markdown_manifest_fits_behavior_manifest_budget` | Minimal manifest within `BEHAVIOR_MANIFEST_PAYLOAD_BUDGET_BYTES` |
| `markdown_editor_rules_parse_continue_line_markers` | `EnterRule::ContinueLineMarkers` carries correct markers |
| `markdown_empty_list_item_exits_list` | `exit_on_empty_item = true` |
| `markdown_editor_rules_parse_preserve_fence_body_indent` | `EnterRule::PreserveFenceBodyIndent` with ```` ``` ```` and `~~~` |
| `markdown_editor_rules_parse_pair_rules_with_multi_char_delimiters` | `**`, `__`, `` ` `` as valid pair rules |
| `markdown_editor_rules_reject_executable_fields` | `PairRule` type system enforces inertness |
| `markdown_editor_rules_parse_all_fields` | All `editorRules` sub-fields populated correctly |
| `markdown_activation_publishes_manifest_with_commands` | Three Markdown commands in composed manifest |
| `markdown_activation_publishes_manifest_with_keymaps` | Package record has 3 key routing descriptors |
| `markdown_behavior_version_increments_on_reactivation` | Reactivation bumps `behavior_version` |
| `non_markdown_manifest_uses_preserve_leading_whitespace` | Default manifest has `EnterRule::PreserveLeadingWhitespace` |
| `markdown_mode_manifest_does_not_wait_for_parse_handler` | Manifest selection succeeds without parse handler |
| `markdown_package_has_no_mdast_dependency` | Parser dependency metadata uses markdown-it and removes the superseded mdast dependency |
| `markdown_runtime_code_has_no_from_markdown_import` | Active package runtime/source files do not import or inject the superseded mdast parser |
| `markdown_parser_adapter_uses_markdown_it_package_boundary` | Parser adapter/export metadata stays package-owned, uses markdown-it token parsing, and avoids raw Deno ops |
| `markdown_parser_adapter_publishes_protocol_spans_without_parser_data` | Parser internals remain behind an injectable adapter and published spans use Clay protocol fields |
| `markdown_it_adapter_has_token_stream_range_fixtures` | Token/range fixtures cover headings, nested inline emphasis, inline code, fences, list markers, and UTF-8 |
| `markdown_fixture_activates_with_markdown_it_adapter` | Fixture classification and package load runtime wiring use markdown-it parser/SDUI adapters through generic Clay facade signatures |
| `markdown_sdui_status_reports_markdown_it_parse_state` | Structural SDUI reports markdown-it parse status and validates command-action provenance |
| `markdown_disabled_falls_back_to_plain_text_after_rewrite` | Disabled package removes Markdown command/keybinding authority and permits plain-text fallback |
| `markdown_typing_does_not_wait_for_markdown_it_parse` | Slow parser simulation does not block local typing acknowledgement/application |
| `server::js_runtime::tests::markdown_package_runtime_loads_markdown_it_workflow` | Actual package load module validates the manifest, activates Markdown, registers commands/parse, publishes injected markdown-it-token decorations, and publishes SDUI through Clay facades |
| `server::js_runtime::tests::markdown_parser_adapter_publishes_viewport_bounded_decorations` | Runtime-backed adapter test verifies required span kinds, exact UTF-8 byte ranges, inline child traversal, fence/list marker ranges, viewport filtering, facade publication, and parse-not-render behavior |
| `server::js_runtime::tests::markdown_it_adapter_large_fixture_span_counts_are_stable` | Runtime-backed large repeated token-stream fixture verifies deterministic nonzero span counts for headings, strong/emphasis, inline code, fenced code blocks, and list markers |
