# Sequence Keybindings (Phase 24.5)

Phase 24.5 extends the keybinding system from single-stroke chords to
multi-stroke sequences (Emacs-style chords such as `Ctrl+X Ctrl+P`). The
extension is a **chord-string format change** to the existing `bindKey` /
`unbindKey` Clay JS APIs: no new op, facade function, command ID, or public
Rust capability was added (pinned by
`phase24_5_keybinding_internals_stay_crate_private` in
`tests/rust_visibility_api_mapping.rs`). User-facing reference:
`docs/reference/clay-js-api/keybindings/bind-key.md` (Multi-stroke chords).

## Source

- `src/server/ops/keybindings.rs` — `parse_key_sequence` (space-separated
  sequence parser, configuration path only), `key_sequence_string` (list
  serialization)
- `src/client/behavior.rs` — `ClientBehaviorState::route_key_sequence` (pure
  matcher), `ChordRouteOutcome`, `route_key` (single-stroke wrapper)
- `src/editor/surface/command.rs` — `PendingChord` (mutable pending-chord state),
  the state machine inside `route_key_with_event`, `EditorKeyOutcome::consumed`
- `src/masonry_pane_document.rs` — `local_key`: consumed-key handling and
  modal-menu containment order
- `src/behavior/manifest.rs` — `is_strict_prefix` + prefix-collision pass in
  `validate_manifest` (`AmbiguousKeyBinding`)
- `src/protocol/mod.rs` — `KeyBindingRule::global_server_first_sequence`
  constructor; the two sequence defaults
- `src/perf/budgets.rs` — `KEY_CHORD_PENDING_TIMEOUT_MS` and the advisory
  Command Centre budgets

## Flow: chord string → dispatch

1. **Parse (configuration time only).** `bindKey("Ctrl+X Ctrl+P", …)` /
   `unbindKey` (single and batch table forms) split the chord string on
   ASCII whitespace via `parse_key_sequence` and parse each stroke with the
   pre-existing `parse_key_chord` `+`-modifier grammar. Empty sequences and
   any malformed stroke reject the whole bind (`keybindings.invalid_key`).
   `"Space"` is the literal space key in the grammar, so whitespace splitting
   is unambiguous. The result becomes `KeyBindingRule.sequence:
   Vec<KeyStroke>` — already the archived protocol shape, so
   `PROTOCOL_VERSION` stays 17 and the rkyv archive round-trips unchanged
   (`multi_stroke_key_binding_rules_round_trip_the_archive_identically` in
   `tests/window_management_protocol.rs`). Parsing never runs on the
   keypress hot path; `key_sequence_string` serializes sequences for
   `listKeyBindings` and `key_binding_json`.
2. **Validate.** `bind_key` publishes through
   `ActiveBehaviorManifest::publish_replacement` → `validate_manifest`, which
   now rejects same-context **strict-prefix collisions** (see below).
3. **Match (client, per keystroke).** `EditorSurface::route_key_with_event`
   builds a fresh `ClientBehaviorState` from the installed manifest each
   keystroke (the router is immutable), then calls the pure matcher
   `route_key_sequence(pending, key)`.
4. **Dispatch.** `Matched` → `dispatch_routed(behavior)`; the surface's
   pending buffer is cleared and the routed behavior runs exactly as a
   single-stroke match would (client edit, completion request, client UI
   command, or server-first intent).

## The pure matcher

`route_key_sequence(&[KeyStroke], &KeyStroke) -> ChordRouteOutcome`
(`src/client/behavior.rs:79`) is allocation-free: rules are compared
slice-wise against `pending + key`. Outcomes:

- `Matched(RoutedBehavior)` — the extended candidate exactly equals a rule.
- `Pending` — the extended candidate is a strict prefix of at least one
  rule (longer rule whose leading strokes match).
- `Mismatch` — nothing matches.

Contexts are considered in the Phase 22.1 order (`EditorTextFocus` before
`Global`); within a context an exact match beats a longer rule's prefix, and
the first context with any non-mismatch result wins — so an exact match
beats a prefix in the same context, and an editor-context prefix beats a
`Global` exact match. The single-stroke `route_key` is now a thin wrapper
with an empty pending buffer: `Matched` → behavior, `Pending` →
`Unhandled` (a pending prefix must never insert text), `Mismatch` →
`route_unbound_key`. Direct-call tests of the old single-stroke API stay
green.

## Pending-chord state machine

`PendingChord { strokes: Vec<KeyStroke>, started_at: std::time::Instant }`
(`src/editor/surface/mod.rs:219`) lives in `EditorSurface` because it is
mutable routing state that must survive across keystrokes, while
`ClientBehaviorState::new` is reconstructed per keystroke from the manifest.
It holds only already-validated strokes from the incoming event stream.

Inside `route_key_with_event` (`src/editor/surface/mod.rs`), after the Tab /
Escape special cases:

- **Stale check:** if the pending chord's `started_at` is older than
  `KEY_CHORD_PENDING_TIMEOUT_MS` (1500 ms, `src/perf/budgets.rs:257`,
  advisory), it is cancelled on the next keystroke and the key is
  re-evaluated as a fresh stroke.
- **Matched:** clear the buffer, dispatch.
- **Pending:** extend the buffer (first stroke keeps `Instant::now()`),
  return `EditorKeyOutcome::consumed()`.
- **Mismatch:** clear the buffer, then re-evaluate the key **fresh** —
  exact match, new prefix, or unbound fallback. Abandoning a prefix never
  eats typing (Emacs behavior; `editor_abandoned_chord_does_not_eat_the_next_key`).

The buffer grows one stroke per Pending outcome and is bounded by the
longest bound sequence (`pending_chord_buffer_grows_one_stroke_per_pending_outcome`
in `tests/editor_performance_invariants.rs`,
`editor_pending_chord_buffer_never_exceeds_longest_bound_sequence` in
`src/editor/surface/mod.rs`). Plan 089 adds a compact deterministic state-machine
sweep: 128 fixed cases cover complete two-/three-stroke sequences, mismatch
re-evaluation, and stale-timeout re-evaluation, asserting that every case
clears pending state and never swallows more than its intended fallback text.

**Why the consumed flag:** `finish_local_outcome` marks a key handled only
when the outcome `changed`, so an unhandled pending stroke would bubble to
the shell's global keybindings. `EditorKeyOutcome::consumed()` sets a flag
that `local_key` (`src/masonry_pane_document.rs:2069`) checks right after
routing: `ctx.set_handled()` + early return, so a pending stroke neither
inserts text nor bubbles. Modal containment needs no change: `route_menu_key`
runs before `route_key_with_event` in `local_key`
(`src/masonry_pane_document.rs:2132`), so server-owned modal menus own the
complete key stream and chords route only when no modal menu is active.

## Prefix-collision validation

`validate_manifest` (`src/behavior/manifest.rs`) runs a pairwise same-context
strict-prefix pass (`is_strict_prefix` slice helper) after its Debug-string
dedup loop, rejecting a rule whose sequence is a strict prefix of another
rule's in the same context with `AmbiguousKeyBinding` naming the shorter
(prefix) rule's command — the prefix fires on the earlier stroke and the
longer chord would be unreachable. The runtime `bindKey` path is covered
automatically because every bind publishes through `validate_manifest`.
Cross-context prefixes and divergent rules sharing a common prefix stay
valid: the pending-chord matcher resolves them by the next stroke. No
default manifest contains prefix collisions — the two new defaults
(`Ctrl+X Ctrl+P`, `Ctrl+X Ctrl+F`) both start with `Ctrl+X`, which no
single-stroke default uses, and diverge at the second stroke
(`default_keymaps_are_prefix_collision_free`).

## Defaults, budgets, and guards

- **Sequence defaults** (`src/protocol/mod.rs`, `default_keymaps`):
  `controlCenter.open` = `Ctrl+X Ctrl+P`, `controlCenter.openPath` =
  `Ctrl+X Ctrl+F` (both `Global`, `ServerFirst`, command IDs and routing
  policies unchanged; pre-24.5 the temporary defaults were `Ctrl+Shift+P` /
  `Ctrl+Alt+P`). Built via the `KeyBindingRule::global_server_first_sequence`
  constructor.
- **Advisory budgets** (`src/perf/budgets.rs`, Phase 21 promotion rule —
  no wall-clock CI gate): `COMMAND_CENTRE_OPEN_P95_BUDGET_MS = 50`,
  `COMMAND_CENTRE_FILTER_UPDATE_P95_BUDGET_MS = 4`,
  `COMMAND_CENTRE_LISTING_MAX_ENTRIES = TRANSIENT_MENU_MAX_ITEMS` (aliased so
  a menu-cap change cannot silently unbind listings), and
  `COMMAND_CENTRE_LISTING_PAYLOAD_BUDGET_BYTES = 64 KiB` (far below the 1 MiB
  codec frame ceiling). The Command Centre round trip itself is unchanged —
  filter updates stay local bounded fuzzy scores, relists stay connection-
  executed, and the listing reads only `document_id()` metadata, never
  document text.
- **Deterministic guards** (`tests/editor_performance_invariants.rs`):
  `command_centre_open_filter_and_listing_stay_bounded_off_hot_paths` (no
  document-text reads on the open/filter/listing paths, `max_entries` wired
  to the budget constant) and the pending-chord growth guard above.

## Authority review (browse grant)

The built-in browse grant (path-mode traversal outside workspace roots,
`controlCenter.openPath`) remains reachable only from the user-driven
built-in path-mode surface: `open_command_centre_session` has exactly two
call sites, both in `src/server/connection/mod.rs` (command-intent dispatch and
server-menu activation — user-driven client messages). Package code cannot
reach it: `validate_package_command` rejects reserved/`clay.`-prefixed IDs
(`is_package_owned_id`), `CommandRegistry::register_command` rejects
duplicate IDs, and the op layer never calls the session opener
(`phase24_5_command_centre_sessions_are_not_a_package_programmatic_surface`,
`control_center_command_ids_are_not_registerable_by_packages`). Packages may
bind chords only for registered commands; the multi-stroke format grants no
new binding authority (`unknown_command_binding_is_rejected`). Full budget
and authority record: `docs/development/performance.md` (Phase 24.5).

## Extension guidance

- Bind any space-separated sequence: `bindKey("g g", "workspace.refresh")`
  or `bindKey("Ctrl+X Ctrl+P", "controlCenter.open")`; single-stroke chords
  keep the fast path (immediate dispatch, no pending hold).
- A same-scope strict prefix is rejected at bind time
  (`keybindings.bind_failed`, diagnostic names the colliding rule) — bind
  the longer chord first or use divergent second strokes.
- The pending timeout is server-owned (not user configuration); function
  keys remain unsupported by the chord grammar.
- `bindKey`/`unbindKey` (single and batch table forms) remain the complete
  public keybinding surface; there is no separate "bind sequence" API.

## Tests

- `src/server/ops/keybindings.rs`: `parse_key_sequence_accepts_single_and_multi_stroke_chords`,
  `parse_key_sequence_rejects_empty_and_malformed_sequences`,
  `key_sequence_string_round_trips_multi_stroke_rules`.
- `src/client/behavior.rs`: `route_key_sequence_*` — single-stroke
  regression, two-stroke tracking, mismatch clearing, context precedence for
  exact matches and prefixes.
- `src/editor/surface/mod.rs`: `editor_pending_chord_consumes_strokes_and_dispatches_on_completion`,
  `editor_abandoned_chord_does_not_eat_the_next_key`,
  `editor_stale_pending_chord_cancels_on_the_next_key`,
  `editor_generated_chord_sequences_preserve_prefix_mismatch_and_timeout_transitions`,
  `editor_pending_chord_buffer_never_exceeds_longest_bound_sequence`.
- `src/behavior/manifest.rs`: `manifest_rejects_prefix_collisions_within_a_context`,
  `manifest_accepts_divergent_rules_sharing_a_common_prefix`,
  `manifest_accepts_prefix_collisions_across_contexts`.
- `src/server/js_runtime/mod.rs` (configuration): `configuration_bind_key_sequence_publishes_multi_stroke_rule`,
  `configuration_unbind_key_sequence_removes_only_the_matching_rule`,
  `configuration_bind_key_prefix_collision_is_rejected`.
- `src/protocol/mod.rs`: `default_keymaps_are_prefix_collision_free`;
  `tests/window_management_protocol.rs`:
  `multi_stroke_key_binding_rules_round_trip_the_archive_identically`.
- `tests/rust_visibility_api_mapping.rs`:
  `phase24_5_keybinding_internals_stay_crate_private`.
- Commands: `cargo test --lib --quiet route_key_sequence`,
  `cargo test --test security --quiet`, `cargo test --test protocol --quiet`.

## Related

- [Behavior Runtime Registration](behavior-runtime-registration.md)
- [Behavior Manifests](behavior-manifests.md)
- [Client Behavior Routing](../flows/client-behavior-routing.md)
- [Control Center](control-center.md)
- [Transient Menu Round Trip](transient-menu-round-trip.md)
- [Path Browser](path-browser.md)
- `docs/reference/clay-js-api/keybindings/bind-key.md` — authoritative user
  API; this page documents the implementation behind it.
