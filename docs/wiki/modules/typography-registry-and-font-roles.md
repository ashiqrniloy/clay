# Typography Registry and Font Roles

## Source

- `src/editor/typography.rs` — `TypographyRegistry`, `ResolvedFontProfile`, `UiTextVariant`, `UiTextMetrics`.
- `src/protocol/mod.rs` — `FontRole`, `DocumentFontRole`, `FontProfile`, `ActiveTypography`, validation constants and `validate()`.
- `src/protocol/decorations.rs` — `DecorationSpan.font_role`, `SyntaxStyleMapEntry`.
- `src/server/ops/typography.rs` — `op_clay_theme_set_typography`.
- `src/server/mod.rs` — `ActiveTypographyState`, `RuntimeGenerationStore`, `install_active_typography`.
- `src/server/connection.rs` — bootstrap fifth message and live broadcast loop.
- `src/client/mod.rs` — `ClientInitialState.active_typography`, `ClientConnectionEvent::ActiveTypography`, handshake reader.
- `src/masonry_editor.rs` — `apply_connection_event` typography branch, layout-invalidation flag, SDUI propagation.
- `src/editor/layout.rs` — `VisibleTextStyleRun`, `LayoutCacheKey::with_presentation`, role-aware `rebuild`.
- `src/editor/surface.rs` — `normalize_visible_text_style_runs`, `set_typography`, `document_font_role`, `layout_style_revision`.
- `src/masonry_sdui.rs` — `SduiNativeState.typography`, `text_metrics`/`component_metrics`, accessibility bounds.
- `src/shell/package_ui.rs`, `src/server/ui.rs`, `src/packages/record.rs` — component `style.fontRole` validation.
- `src/packages/modes.rs`, `src/server/ops/modes.rs`, `src/server/syntax.rs` — mode `defaultFontRole` and style-map roles.
- Tests: `tests/typography_protocol.rs`, `tests/editor_performance_invariants.rs`, `tests/decoration_transport.rs`, `tests/markdown_mode.rs`, `tests/primitives_docs.rs`, `tests/package_loading_docs.rs`, `tests/manual_smoke_docs.rs`.
- Authoritative public API: [`clay.theme.setTypography`](../../reference/clay-js-api/theme/set-typography.md).
- Authoritative package/mode contract: [Semantic Typography Roles](../../reference/primitives/typography.md).
- Decision: `decision-logs/2026-07-11-1418-semantic-font-roles-and-user-owned-typography.md`.
- Pattern: `.agents/skills/project-patterns/references/typography-role-ownership.md`.

## Overview

Phase 18.16.5 adds user-owned font configuration and semantic font roles across the editor, native shell, SDUI, and package components. Users atomically configure three profiles — `monospace`, `proportional`, `ui` — each an ordered family fallback stack plus a logical-pixel base size. Modes, syntax/semantic decoration spans, and text-bearing package components select one of a closed set of semantic roles; they never supply concrete family names or sizes. The client resolves roles through a cached `TypographyRegistry` and feeds Parley layout and native UI geometry from those cached profiles.

Typography is architecturally separate from `ActiveTheme`/`StyleRegistry`: the theme registry owns colors and text attributes; the typography registry owns family stacks and sizes. A font change reshapes text and changes geometry, so it has its own snapshot (`ActiveTypography`), its own revision, its own bootstrap message, and its own live broadcast.

## Responsibilities

- Own the complete, atomic `ActiveTypography` snapshot end-to-end: JS facade → server op → server state → bootstrap and live protocol delivery → client registry → Parley/UI geometry.
- Resolve semantic `FontRole` selections to cached Parley `FontStack`/`FontSize` values, with role-appropriate generic family fallback.
- Drive editor Parley layout (default profile plus ranged overrides), document geometry, and native UI/SDUI/component geometry from one set of resolved metrics.
- Invalidate role-aware layout cache and dependent UI geometry on revision, style-revision, or document-role change; do nothing on unchanged revisions.
- Keep package declarations inert and semantic: deny concrete font fields, executable renderer authority, installed-font discovery, and any package JavaScript/IPC in native hot paths.

## How It Works

### Configuration and server state

`runtime/js/theme.js::setTypography` validates its object input in JS, then calls `op_clay_theme_set_typography`. The op (`src/server/ops/typography.rs`) enforces a raw `TYPOGRAPHY_PAYLOAD_BUDGET_BYTES` cap before parsing, requires exactly `monospace`/`proportional`/`ui` keys each with only `families` and `size`, then hands a parsed `ActiveTypography` to `ClayOpState::set_active_typography`.

Server state lives in `ActiveTypographyState` (`src/server/mod.rs`): an `Arc<Mutex<ActiveTypography>>` plus a `broadcast::Sender` (capacity 16). `replace()` validates the whole candidate, and if all three profiles are byte-identical to the current snapshot it returns `None` and emits nothing — duplicate calls and reloads that reproduce the prior configuration do not churn clients. On a real change it bumps `revision` (`saturating_add(1)`), swaps the snapshot, and broadcasts. `RuntimeGenerationStore` exposes `active_typography()`, `subscribe_typography()`, and `replace_typography()` delegates.

`apply_runtime_evaluation` and `reload_runtime_generation` call `install_active_typography`, passing `evaluation.active_typography.clone().unwrap_or_default()`. Typography state therefore persists across JS evaluations within a generation and defaults to `ActiveTypography::default()` when no evaluation sets it, unlike per-evaluation-reset decorations/SDUI/records. A failed reload reports a `RuntimeDiagnostic` and keeps the previous snapshot active.

### Protocol and delivery

`ServerMessage::ActiveTypography(ActiveTypography)` is the fifth bootstrap message, sent after `ActiveTheme` in `send_welcome_snapshot_and_manifest` (`src/server/connection.rs`). Variant ordering in `ServerMessage` is fixed for rkyv wire stability.

Live updates are multiplexed in the per-connection event loop via `tokio::select!` over `subscribe_typography()`. A successful `replace` emits exactly one `ServerMessage::ActiveTypography` to each connected client; a broadcast lag (closed/lagged receiver) re-sends the current snapshot so a client never misses the authoritative state. `live_typography_update_reaches_connection_once` locks the one-update-per-replacement invariant.

The client handshake (`src/client/mod.rs::handshake_initial_state`) reads `ActiveTypography` as the fifth message, validates it via `typography.validate().is_ok()`, and rejects invalid snapshots with `UnexpectedMessage`. `ClientInitialState` carries `active_typography`; `run_connection` forwards `ServerMessage::ActiveTypography` to `ClientConnectionEvent::ActiveTypography`, silently dropping invalid live snapshots.

### Client registry and resolution

`TypographyRegistry` (`src/editor/typography.rs`) converts a validated `ActiveTypography` once into three `ResolvedFontProfile`s. `ResolvedFontProfile::from_wire` parses each family through `GenericFamily::parse`: recognized generic names become `FontFamily::Generic`, others become `FontFamily::Named`. If no family in the stack is generic, a role-appropriate fallback is appended (`Monospace` for monospace, `SansSerif` for proportional, `SystemUi` for ui) so unavailable named families never produce an empty stack. Because Clay disables system font enumeration (`use_system_fonts: false`), named families only resolve if registered via `RenderRoot::register_fonts`; generic families always resolve.

`install()` is the live-update entry: it rejects same-or-lower revisions (`Ok(false)`) so duplicate broadcasts are no-ops, and on a newer revision rebuilds all three resolved profiles. `profile(role)` returns the `ResolvedFontProfile` for `Monospace`/`Proportional`/`Ui`; `revision()` exposes the current revision; `font_stack()`/`size()` feed Parley.

`document_line_height()` computes `max(monospace.size, proportional.size) * DOCUMENT_LINE_HEIGHT_MULTIPLIER` (1.4), intentionally excluding the UI profile. This is the conservative shared baseline for viewport extraction, pixel-scroll progression, and logical scrollbar progress; visible Parley `Layout::height()` and caret geometry remain the exact rendered authority. `document_line_height_uses_largest_document_profile_not_ui` locks that the UI profile cannot influence document geometry.

`UiTextVariant` (`Body`/`Status`/`Title`/`Detail`) is a semantic scale, never a package-provided point size. `from_typography_token` maps shell theme tokens (`typography.title`, `typography.status`, otherwise `typography.body`). Each variant has a fixed scale: Body/Status = 1.0, Title = 14/12, Detail = 10/12. `ui_text_metrics(role, variant)` returns `UiTextMetrics` with `font_size = profile.size * variant.scale()`, `line_height = font_size * 1.2`, and `row_height = line_height + vertical_padding`. `button_height()`, `list_height(detail)`, and `status_height()` derive row/element heights from those metrics. `ui_variants_scale_from_configured_role_size` locks scale ownership.

### Editor layout and role normalization

`EditorSurface` (`src/editor/surface.rs`) owns the `TypographyRegistry` and a `layout_style_revision: u64`. `set_typography(active)` calls `typography.install()` and, on change, resets `LayoutState` to default, zeroes `visual_scroll_y`/`last_visual_max_scroll_y`, clears `pin_caret_visible`, and bumps `layout_style_revision`. `bump_layout_style_revision()` is also called on decoration application, `StyleRegistry` (theme) change, `BehaviorManifest` document-font-role change, and `load_snapshot` (document reload). Any future event that affects layout-visible presentation must bump this revision.

`document_font_role()` reads `BehaviorManifest.document_font_role` (defaulting to `FontRole::Proportional` when absent). Mode `defaultFontRole` propagates `ModeDeclaration` → `MajorModeActivation` → `BehaviorManifest.document_font_role` at activation time (`src/packages/modes.rs`, `src/server/ops/modes.rs`); `core.code` defaults to `Monospace`, `core.text` and Markdown to `Proportional`.

`normalize_visible_text_style_runs()` is the viewport-bounded normalization step. It sweeps `visible_spans`, drops invalid/UTF-8-boundary-violating/out-of-document-end ranges, computes `TextAttributes` from `StyleRegistry::style_for`, and resolves a font role per boundary segment by `font_role_precedes`. Only `Syntax` and `Semantic` layers may carry a role (`span_font_role` gates on `DecorationKind`); `Diagnostic`/`SearchMatch` and out-of-bounds/stale spans cannot alter the font role. Adjacent equal runs merge. Result is a `Vec<VisibleTextStyleRun>` cached alongside the Parley layout.

`font_role_precedes` ordering is deterministic and ignores arrival order: priority (higher wins) → `decoration_layer_rank` (Semantic=2, Syntax=1, Diagnostic/Search=0) → provenance (package_prefix/name/version lexicographic) → `font_role_rank` (Monospace=2, Proportional=1, Ui/None=0). Text attributes compose independently by OR over active spans.

`LayoutState::rebuild()` (`src/editor/layout.rs`) takes `&TypographyRegistry`, `document_font_role`, and the owned `Vec<VisibleTextStyleRun>`. It pushes the default profile's `FontStack` and `FontSize` (and `LineHeight::FontSizeRelative(DOCUMENT_LINE_HEIGHT_MULTIPLIER)`) via `RangedBuilder::push_default`, then for each run pushes ranged `FontStack`, `FontSize`, `FontWeight::BOLD`/`FontStyle::Italic`/`Underline`/`Strikethrough` overrides over the run's byte range. Placeholder text follows the document default role.

`LayoutCacheKey` carries `text_revision`, `viewport_revision`, `max_width`, plus `typography_revision`, `layout_style_revision`, and `document_font_role` set via `with_presentation`. `should_rebuild()` checks key equality and `ctx.fonts_changed()` separately; a typography revision bump, style-revision bump, or document-role change invalidates the cache and triggers a rebuild. `mixed_role_normalization_stays_bounded_by_visible_span_boundaries` locks that normalization never escapes the visible viewport.

### Geometry

All hardcoded font-size constants were removed. `scroll_vertical_pixels`, `update_visible_line_count_for_height`, `scrollbar_thumb_rect`, and `paint_caret` derive line height from `TypographyRegistry::document_line_height()`. `paint_caret` no longer has an empty-document fallback branch; it always uses Parley `caret_geometry_for_visible_byte_offset` and returns early on `None`. `empty_document_caret_uses_default_document_profile` and `custom_typography_keeps_scrollbar_and_viewport_geometry_bounded` lock bounded behavior. `typography_geometry_uses_shared_profile_baseline_not_fixed_font_size` guards against `TEXT_FONT_SIZE` returning.

### Native UI, SDUI, and accessibility

`masonry_editor.rs::paint_status_line` derives status bar height from `UiTextMetrics::status_height()`, resolves `FontStack` from the UI profile, sets `FontSize` from `metrics.font_size` and `LineHeight` from `UiTextMetrics::LINE_HEIGHT_MULTIPLIER`, and vertically centers text. `STATUS_TEXT_SIZE`/`STATUS_BAR_HEIGHT` were removed.

`SduiNativeState` owns a cloned `TypographyRegistry` set via `set_typography()` both at bootstrap (`EditorWidget` constructor) and on every `ActiveTypography` connection event. `set_typography` resets SDUI scroll/content_height/viewport_height/action state on change. `text_metrics(role, variant)` delegates to `typography.ui_text_metrics()`; `body_metrics()` is the UI/Body shorthand; `component_metrics(component, fallback)` resolves a `PackageUiComponentTree`'s `font_role` and `text_variant`. `paint_text` resolves `FontStack` from `typography.profile(role).font_stack()` and `FontSize` from `UiTextMetrics`; row heights, button heights, and list heights derive from the same metrics.

`SduiThemeStyle` (`src/shell/theme.rs`) maps typed `typography.title`/`typography.body`/`typography.status` tokens to `UiTextVariant` variants (via `ResolvedThemeValue::Typography`), not scalar font sizes; actual pixels are deferred to `TypographyRegistry::ui_text_metrics()` at render time.

Accessibility geometry uses `SduiAccessibilityEntry { role, label, bounds: Rect }`. `append_accessibility_children()` builds AccessKit nodes with `Node::set_bounds()` from computed cursor_y/depth geometry; `collect_accessibility_entries()`/`collect_package_accessibility_entries()` walk the SDUI and `PackageUiComponentTree` trees computing bounds. `EditorWidget::accessibility()` composes the SDUI subtree plus a bounded Status node. Since SDUI paint nodes are not laid-out widgets, bounds are computed from paint geometry rather than `LayoutCtx::size()`. `ui_size_change_scales_row_hit_and_accessibility_bounds_together` locks that paint rect, hit-test rect, and accessibility bounds scale identically with UI size.

### Package component roles

`PackageUiComponentTree` (`src/shell/package_ui.rs`) carries `font_role: FontRole` (default `Ui`) and `text_variant: Option<UiTextVariant>`. Only `panel`, `label`, `button`, `list`, and `statusItem` may declare `style.fontRole`; `editorView` and structural components cannot. The two-gate validation: `ComponentKind::supports_text_font_role()` in `src/server/ui.rs` and `src/packages/record.rs`, plus deny-by-default field-name rejection (`fontFamily`/`fontFamilies`/`fontSize`/`fontStack`) in `reject_syntax_grammar_prohibited_authority`. `package_component_font_role_is_semantic_and_text_only` rejects unknown roles, concrete fields, and roles on non-text kinds. `package_component_font_role_uses_selected_profile_without_concrete_sizes` locks that a monospace-fontRole component resolves to the monospace profile, not the UI profile.

## Code Examples

User configuration in `~/.config/clay/init.js`:

```ts
import { setTypography } from "clay:theme";

setTypography({
  monospace: { families: ["JetBrains Mono", "monospace"], size: 20 },
  proportional: { families: ["Inter", "sans-serif"], size: 20 },
  ui: { families: ["system-ui"], size: 12 },
});
```

Package mode default and style-map role:

```js
serverRegisterModePattern({
  modeId: "example-code",
  defaultFontRole: "monospace",
});
// styleMap entry: { "code": { "styleToken": "markup.code-block", "fontRole": "monospace" } }
```

Resolving a profile inside the editor (internal):

```rust
let default_profile = typography.profile(document_font_role);
builder.push_default(StyleProperty::FontStack(default_profile.font_stack()));
builder.push_default(StyleProperty::FontSize(default_profile.size()));
for run in style_runs {
    builder.push(StyleProperty::FontStack(typography.profile(run.font_role).font_stack()), run.range.clone());
}
```

## Primitive Coverage

- `SemanticTypographyRole` — field-level extension of existing mode/decoration/syntax/UI primitives, not a new package setter or permission. Owning modules: `src/protocol/mod.rs`, `src/packages/modes.rs`, `src/server/ops/modes.rs`, `src/server/ops/decorations.rs`, `src/server/syntax.rs`, `src/server/ui.rs`, `src/packages/record.rs`.
- JS facade/op: `clay.theme.setTypography` (`runtime/js/theme.js`) → `op_clay_theme_set_typography` (`src/server/ops/typography.rs`). No separate package typography op exists; the only public surface is the user-facing setter documented in [`set-typography.md`](../../reference/clay-js-api/theme/set-typography.md).
- Validation/budgets: `MAX_FONT_FAMILIES_PER_PROFILE=8`, `MAX_FONT_FAMILY_BYTES=128`, `MIN_FONT_SIZE=6.0`, `MAX_FONT_SIZE=96.0`, `TYPOGRAPHY_PAYLOAD_BUDGET_BYTES=1024`; `FontProfile::validate()` requires a non-empty stack, a trailing generic fallback, finite bounded size, and no control characters; `ActiveTypography::validate()` validates all three profiles.
- Hot-path policy: configuration/protocol/normalization run outside paint/input/layout; native hot paths read cached `TypographyRegistry`/profile/style/layout state only — no package JavaScript, IPC, filesystem/network access, font download, or server-side installed-font discovery. `typography_updates_do_not_enter_editor_hot_paths` guards this.
- Future-mode reuse: declare `defaultFontRole` and optional style-map/decoration `fontRole` only; no language-name branches in client/editor/server rendering code. `first_party_modes_declare_roles_without_rendering_language_branches` statically asserts absence of mode-id string literals in layout/surface/editor/sdui sources.

## Invariants and Constraints

- Typography is separate from theme: `ActiveTypography` has its own snapshot, revision, bootstrap message (5th), and live broadcast; it is never merged into `ActiveTheme`/`TextThemeOverride`.
- `install()` is monotonic: only strictly newer revisions replace state; equal/lower revisions are no-ops.
- Profiles are atomic: the server replaces all three together; there is no per-profile setter and no hidden JSON/TOML typography key.
- A profile stack always ends in a generic family; named-family absence produces a generic fallback without server font inspection or package notification.
- Document geometry excludes the UI profile; mixed-role lines use the largest active document profile as a conservative baseline, with Parley supplying exact rendered metrics.
- Only `Syntax` and `Semantic` decoration layers may carry `DocumentFontRole`; `Diagnostic`/`SearchMatch` and out-of-bounds/stale/invalid-UTF-8 spans fail closed and cannot alter layout.
- Role precedence is deterministic and independent of source arrival order: priority → layer rank → provenance lexicographic → role rank.
- `f32` sizes in `FontProfile` mean `ActiveTypography` (and transitive types like `EditorAction`, `ReloadedDocumentRefresh`) derive `PartialEq`, not `Eq`.
- Packages declare semantic names only; `fontFamily`/`fontFamilies`/`fontSize`/`fontStack`, font paths/bytes/URLs/downloads, raw CSS/Parley properties, renderer callbacks, and raw ops are rejected.
- Semantic role declaration grants no filesystem, network, shell, package-manager, AI, WASM, workspace, native-widget, or client-runtime authority.

## Tests

- `src/editor/typography.rs`: `typography_registry_resolves_each_role_and_revision`, `missing_named_family_retains_generic_fallback`, `unchanged_typography_revision_does_not_invalidate_layout`, `document_line_height_uses_largest_document_profile_not_ui`, `ui_variants_scale_from_configured_role_size`.
- `src/editor/layout.rs`: `mixed_role_line_height_keeps_largest_inline_profile_in_bounds`, `unicode_and_emoji_shape_with_unavailable_named_font_fallback`, `layout_cache_invalidates_on_typography_style_or_document_role_change`.
- `src/editor/surface.rs`: `markdown_code_range_uses_monospace_inside_proportional_layout`, `overlapping_style_runs_resolve_deterministically_and_merge_adjacent_runs`, `diagnostic_and_invalid_utf8_spans_cannot_change_font_role`, `mixed_role_normalization_stays_bounded_by_visible_span_boundaries`, `empty_document_caret_uses_default_document_profile`, `custom_typography_keeps_scrollbar_and_viewport_geometry_bounded`.
- `src/masonry_editor.rs`: `live_typography_update_requests_layout_render_and_accessibility`.
- `src/masonry_sdui.rs`: `ui_size_change_scales_row_hit_and_accessibility_bounds_together`, `package_component_font_role_uses_selected_profile_without_concrete_sizes`.
- `src/server/ui.rs`: `package_component_font_role_is_semantic_and_text_only`.
- `src/server/connection.rs`: `live_typography_update_reaches_connection_once` plus bootstrap fifth-message consumption across all connection tests.
- `src/server/mod.rs`: `typography_defaults_exist_without_init_configuration` (with failed-reload path).
- `src/server/js_runtime.rs`: `set_typography_replaces_all_profiles_atomically`, `set_typography_failure_preserves_previous_revision`, `typography_configuration_grants_no_additional_authority`, `typography_configuration_rejects_oversized_snapshot`, `invalid_mode_font_role_fails_before_registration_and_keeps_core_fallback`, markdown/parser adapter fontRole assertions.
- `tests/typography_protocol.rs`: wire/validation, first-party `defaultFontRole` declarations, no language-name rendering branches.
- `tests/editor_performance_invariants.rs`: `typography_geometry_uses_shared_profile_baseline_not_fixed_font_size`, `typography_updates_do_not_enter_editor_hot_paths`.
- `tests/markdown_mode.rs`: `core_and_markdown_modes_publish_semantic_document_font_defaults`.
- Documentation structure and discoverability use generic `tests/primitives_docs.rs` inventory/wiki validators; executable tests remain authoritative for behavior instead of phase-specific prose needles.
- Package reference documentation uses generic manifest/API/security validators in `tests/package_loading_docs.rs`; executable package/runtime tests remain authoritative for behavior.
- `tests/manual_smoke_docs.rs`: `phase18_16_5_typography_smoke_covers_fallback_geometry_and_authority`.

Run focused:

```bash
cargo test --lib editor
cargo test --lib masonry_sdui
cargo test --test editor typography_protocol::
cargo test --test editor editor_performance_invariants::
cargo test --test protocol primitives_docs::
cargo test --test protocol package_loading_docs::
cargo test --test protocol manual_smoke_docs::
```

## Related

- [Editor Theme Registry](editor-theme-registry.md) — color/text-attribute ownership boundary.
- [Decoration Transport](decoration-transport.md) — `DecorationSpan.font_role` transport and role overrides.
- [Mode Registry](mode-registry.md) — `defaultFontRole` propagation.
- [Masonry Editor Widget Status Observability](masonry-editor.md) — status-line typography and layout invalidation.
- [Slot-Aware Package UI](slot-aware-package-ui.md) — component `style.fontRole` catalog.
- [Server-Driven UI Protocol Schema](server-driven-ui.md) — SDUI typography metrics and accessibility bounds.
- [Client Snapshot Bootstrap](client-snapshot-bootstrap.md) — fifth bootstrap message and registry revalidation.
- [Protocol Codec](protocol-codec.md) — `ServerMessage::ActiveTypography` and variant ordering.
- [Configuration Runtime](configuration-runtime.md) — `setTypography` atomicity and reload behavior.
- [Phase 18.16.5 Semantic Typography Primitive Review](phase18.16.5-typography-primitive-review.md) — pre-implementation inventory and rejected shapes.
- [Semantic Typography Roles](../../reference/primitives/typography.md) — authoritative package/mode contract.
- [`clay.theme.setTypography`](../../reference/clay-js-api/theme/set-typography.md) — authoritative public API.