# Plan 133 task 6 (U5) — `string_enum_impl!` macro

**Result:** 15 hand-written `parse` + `as_str` pairs (89 variant/string pairs
across 7 files) are now generated from one declaration each by
`src/str_enum.rs::string_enum_impl!`.

```
before, per enum:  impl X { fn as_str(self) -> &'static str { match self { … } }
                            fn parse(value: &str) -> Option<Self> { match value { … } } }
after:             string_enum_impl! { pub X { Variant => "text", … } }
```

## The macro (`src/str_enum.rs`, 126 lines incl. docs)

- `string_enum_impl!` generates `const fn as_str(self)`, `fn parse(value: &str)
  -> Option<Self>`, an optional `const fn all_as_str()` (trailing `all_as_str`
  token), and a `#[cfg(test)] const ALL: &'static [Self]` used by the golden
  test. Attributes/doc comments and the visibility in front of the type name are
  copied onto the generated `impl`.
- `parse` is **not** `const fn` on purpose: rustc 1.98 rejects matching `&str`
  in constant functions (`error[E0658]: cannot match on 'str' in constant
  functions`, verified with a scratch crate before writing the macro).
- The macro takes the **impl, not the type**. Deviation from the plan's
  suggested `string_enum! { … enum Name { … } }` signature, deliberately: the
  converted enums carry `rkyv`/`serde` derives, `#[serde(rename_all = …)]`,
  `#[rkyv(…)]` and variant attributes (`#[default]`), and keeping the enum
  declarations untouched means the archivable/wire attributes cannot be
  disturbed by the refactor. The macro's shape is the smaller one that still
  removes the duplication.
- Three enums keep a module-private `parse` (`pub RelationOperation,
  parse_private { … }`): no visibility widening, so no public-API churn.

## Converted (15)

`packages::extension_points::{RelationOperation, ExtensionContributionKind}`,
`packages::self_update::ChannelKind` (all three `parse_private`),
`perf::fixtures::FixtureKind`, `protocol::theme::Appearance`,
`shell::components::ComponentKind`, `shell::theme::{ThemeTokenType (+
all_as_str), ElevationLevel, ZLevel, DensityLevel}`,
`shell::design_system::{BorderStyle, OutlineStyle, TransitionTiming,
TransformPreset, RecipeState}`. Pairs and visibilities:
`task6-string-enum-map.txt`.

## Skipped (with reasons)

- **`protocol::textobjects::TextobjectKind`** — its `parse` already derives from
  its own `pub const ALL` vocabulary list (also used by the tests), so there is
  no duplicated table to collapse, and the macro's test-only `ALL` would clash
  with it. It is called out in the test file's header for the same reason.
- **`Result`-returning parsers** (`protocol/behavior.rs:686`,
  `shell/design_system.rs:169` `ThemeColorRef`) and **parse-only enums**
  (`shell::components::DeferredComponentKind`, `protocol::textobjects::
  {TextobjectDirection, SmartSelectAction}`) — outside the macro's
  `Option<Self>` + `as_str` contract; the plan's U5 scope is the triple.
- The plan says "the 10 enums"; the review that number came from is absent from
  the repo (task-1 finding, `baseline.md`), so the measured inventory is the
  source of truth: 16 enums have the triple, 15 were converted and 1 is skipped
  above.

## Pre-migration verification (machine-checked, not eyeballed)

The codemod (`task6-string-enum.py`) refuses to touch a file unless, for every
target enum:

- the `as_str` table and the `parse` table are the same bijection (variant →
  string both ways) — the literal `parse` table is therefore byte-identical;
- the impl's arm order equals the enum declaration's variant order;
- the `parse` body is *only* a table (no trim/lowercase/normalisation
  statements);
- every enum declaration variant appears in the table.

Its dry run produced `task6-string-enum-extraction.txt` (the authoritative
pre-change (variant, vis, attrs, docs) inventory, including which methods had
documentation that had to be carried onto the invocation).

## Golden test (`src/str_enum/tests.rs`, 332 lines)

`string_roundtrip_per_enum` checks all 15 enums with pairs extracted from the
pre-change tables (not re-derived from the macro):

- `Enum::ALL.len() == golden.len()` — the macro's variant list is exhaustive, so
  a new variant that was not added to the golden list fails;
- `as_str(variant) == golden string` for every variant (this is the
  byte-identical `as_str` surface);
- strings are unique;
- `parse(as_str(v)) == Some(v)`; `parse` is skipped only for the three
  `parse_private` enums, where the macro generates it from the same list;
- `parse` rejects `""`, `"nope"`, `"NONE"`, `"None"`, `" not-a-real-value"`.

Plus `example_round_trips` for a local enum, which doubles as the macro's
executable documentation.

## Guard updates forced by U5

- `tests/package_ui_conformance.rs::catalog_is_drift_free_across_doc_enum_and_react_registry`
  extracted the kind strings from the hand-written `parse` arms
  (`"editorView" => Some(Self::`); it now reads the `ComponentKind`
  `string_enum_impl!` invocation (`Variant => "editorView",`) and collects every
  quoted token on an arm line. `double_quoted()` was factored out and shared with
  the token-catalog guard.
- `tests/primitives_docs.rs::no_component_kind_or_token_renamed` used
  `"kind" => Some(Self::` for the implemented kinds; it now uses
  `=> "kind",`. (`DeferredComponentKind`'s `"table" => Some(Self::Table)` arm is
  unchanged and still checked with the old marker, since that enum is not
  converted.)

## Method notes (in-flight defects fixed before gating)

1. The first application produced residuals missing the impl's closing brace
   (3 files: `protocol/theme.rs`, `shell/components.rs`, `shell/theme.rs`) — the
   cutter excluded the impl's `}`; fixed in the codemod and patched in place.
2. Carried doc comments/attributes were first emitted *outside* the macro
   invocation, which rustc rejects as "unused doc comment"/"unused attribute"
   (11 lines in 5 invocations); moved inside the braces, where the macro's
   `$(#[$meta:meta])*` picks them up. `#[allow(dead_code)]` (needed for
   `ComponentKind::as_str` and the three theme level enums, used only by
   conformance/tests) now lands on the generated impl.
