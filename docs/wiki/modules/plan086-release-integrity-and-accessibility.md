# Plan 086 Release Integrity and Accessibility

## Source

- `plans/086-Audit-Remediation-P0-Release-Integrity-and-Accessibility.md`
- `src/editor/accessibility.rs`
- `src/masonry_shell/mod.rs`
- `src/masonry_editor.rs`
- `src/masonry_pane_document.rs`
- `src/masonry_package_region.rs`
- `src/protocol/codec.rs`
- `src/server/configuration.rs`
- `tests/live_atspi_smoke.rs`
- `tests/suites/security.rs`
- `tests/suites/protocol.rs`

## Overview

Plan 086 closes the release-integrity P0s around Clay's first accessibility tree, IPC archive validation, dependency advisories, hermetic configuration tests, and live Linux verification. It changes no public Clay JavaScript API and adds no configuration escape hatch for accessibility or protocol validation.

The implementation has two separate safety contracts:

1. Every AccessKit node emitted by Masonry is attached to a reachable parent, and Clay-owned synthetic nodes keep deterministic identities across incremental updates.
2. Every IPC archive passes the frame-size gate and rkyv byte validation at the one `Codec::decode_frame` boundary before it becomes an owned protocol message.

## Responsibilities

- Own deterministic IDs and slot namespaces for Clay-created virtual accessibility nodes.
- Keep the Masonry widget walk and each widget's `accessibility()` child list consistent, including inactive tabs and transient menus.
- Sanitize document names, menu/status text, and live announcements before exposing them to assistive technology.
- Keep archive validation, frame budgets, audit policy, and accessibility safety compiled host controls rather than user configuration.
- Provide consumer-level, live AT-SPI, malformed-archive, and hermetic configuration regression coverage.

## How It Works

### Deterministic virtual accessibility nodes

`src/editor/accessibility.rs::virtual_a11y_node_id(owner, slot)` derives a `NodeId` from the retained owner `WidgetId` and a bounded 9-bit slot:

```rust
let id = virtual_a11y_node_id(owner_widget_id, virtual_a11y_slots::STATUS);
```

The `0xD000_0000_0000_0000` prefix separates virtual IDs from Masonry's small sequential widget IDs. The owner occupies the masked high bits and the slot occupies the low nine bits. A plain `assert!` rejects a slot that would corrupt the namespace. `virtual_a11y_slots` owns the numbering:

- shell: TabList `1`, announcement `2`, Tab `3 + client_id`;
- editor and pane-document status: `1`;
- package-region menu status: `1`, menu item `2 + index`.

The owner is retained by the widget that attaches the node, so redraws reuse the same ID. Replacing that owner intentionally creates a new namespace rather than reusing stale descendants. Existing caps keep slots bounded: active connections are at most 64 and transient-menu items at most 256.

### Reachable-tree invariant

Masonry traverses `children_ids()` for layout, paint, update, and accessibility. The accessibility child list must therefore match the traversed children, minus intentionally stashed subtrees, plus Clay's virtual nodes.

- `EditorWidget::accessibility()` always attaches the region, panel host, and overlay host, even when the region has no visible SDUI content. Its owner-derived status node is appended to the same parent.
- `ClayShellWidget::children_ids()` keeps registered pane hosts and pending orphans available to Masonry. `layout()` stashes inactive-tab hosts and pending orphans, while active hosts are unstashed and laid out normally. Stashing suppresses their accessibility walk without breaking Masonry's registered-child invariant.
- `PackageRegionWidget` keeps its reconciled root pod attached when a menu is open and adds virtual MenuItem/Status nodes beside it. Menu-open state therefore cannot emit an unattached pod subtree.
- Pane, editor, shell, and menu status/announcement nodes are attached by their retained owner rather than allocated with `WidgetId::next()` on every accessibility pass.

The consumer regression helpers in `src/masonry_shell/mod.rs` feed real `TreeUpdate` values through `accesskit_consumer::Tree`, checking initial trees, unchanged redraws, tab add/reorder/remove, status updates, menu query/selection/close, inactive-pane reachability, and stale virtual-node removal.

### Label and announcement safety

`sanitize_document_display_name` keeps a basename only, removes separators/control characters, falls back to `untitled`, and caps names at 64 characters. Recovery/menu/status summaries use the 256-character transient-menu budget. Hosted transient-menu item labels additionally pass through `compose_menu_item_accessibility_label` before `MenuA11y`: controls and path separators are removed, invalid/empty labels fall back to safe display text or `Menu item`, and selected-state suffixes stay inside 256 characters while display/action data remains unchanged. `compose_announcement` emits bounded action text such as `Split pane vertically`, `Closed pane; 1 pane remains`, and `Switched to tab 2: syntax-grammars`; it never includes absolute workspace paths, clipboard contents, or raw preedit text.

### Checked IPC decoding

`Codec::decode_frame` is the only wire deserialization boundary. It:

1. requires the four-byte big-endian length prefix;
2. rejects a declared length above the configured maximum before payload allocation/copy;
3. requires declared and actual payload lengths to match;
4. copies the payload into an aligned `rkyv::util::AlignedVec`;
5. calls `rkyv::from_bytes` with generic `CheckBytes<HighValidator>` and `Deserialize` bounds.

Clay locks `rkyv` at `0.8.17`, removing the three fixed 2026 advisories without adding audit ignores. Codec tests use truncation, deterministic byte mutations, odd/misaligned declarations, and read-side oversized declarations. The invariant is fail-closed/no panic/no out-of-bounds access; a bytechecked archive may still decode as a different valid message when corruption changes a semantically valid field.

### Configuration and dependency closure

Accessibility and archive validation are unconditional. `setPackageOption` rejects `accessibility.enabled`, `accessibility.validation`, `protocol.archiveValidation`, `protocol.codecValidation`, and `protocol.rkyvValidation`; no hidden key can disable either safety boundary. The existing six-export `clay:configuration` contract is unchanged.

The three connection/configuration workflows use unique mode-700 temporary roots and five-second whole-workflow timeouts. The timeout bounds pending menu/session and runtime cleanup; it does not weaken production behavior. Cargo audit reports zero vulnerabilities, with only the documented unmaintained `bincode`, `paste`, and `ttf-parser` warnings remaining.

## Invariants and Constraints

- No per-pass global virtual-ID allocation; IDs are owner-plus-slot derived.
- Inactive tabs remain registered for Masonry lifecycle/reconnect behavior but are stashed and unreachable to assistive technology.
- Menu, region, shell, and editor semantic nodes must be emitted in the same update as their owner attachment.
- Accessibility labels remain sanitized/bounded; hosted menu item labels remove controls/path separators and keep selected suffixes within 256 characters; safety controls are not configuration options.
- The 1 MiB default frame budget and checked decode path remain in force for malformed input.
- Consumer validation is deterministic and blocking; real AT-SPI smoke is environment-gated and reports missing prerequisites instead of passing falsely.
- No new public JS facade, raw op, package permission, or configuration API was introduced by Plan 086.

## Tests and Verification

- `src/masonry_shell/mod.rs`: consumer-accepted initial and incremental accessibility trees, stable virtual IDs, inactive-tab stashing, tab/menu/status reachability, and stale-node removal.
- `src/masonry_package_region.rs`: menu query/selection/close updates and 256-item sanitized-label projection through `accesskit_consumer::Tree`.
- `src/editor/accessibility.rs`: shared bounded menu-item label helper and fallback tests.
- `src/protocol/codec.rs`: malformed/truncated/mutated/misaligned corpus and oversized declaration rejection.
- `src/server/configuration.rs`: internal accessibility/archive-validation settings fail closed.
- `tests/live_atspi_smoke.rs`: isolated server/client, live AT-SPI tree query, tab/status/region assertions, stability re-dump, and child liveness; synthetic top-level-frame focus ingress is covered deterministically by the shell unit test.
- `test-plan/01-launch-and-connection.md`, `03-files-and-workspace.md`, `10-keybindings-and-commands.md`, `13-window-splits.md`, `14-tabs.md`: real Linux execution records and blockers.
- Key commands:

```bash
cargo fmt --check
cargo check --all-targets
cargo clippy --all-targets -- -D warnings
cargo test --all-targets
cargo audit
CLAY_LIVE_A11Y_SMOKE=1 cargo test --test security live_atspi_smoke::live_atspi_accessibility_smoke -- --ignored --exact --test-threads=1
```

The live test is intentionally ignored unless `CLAY_LIVE_A11Y_SMOKE=1` is set and the host exposes a usable AT-SPI bus.

## Plan 089 focus/accessibility closure

The two Plan 086 focus blockers are closed at the shared Masonry seam. The
local exact-0.4.0 `vendor/masonry_core` patch invalidates focused/next/fallback
IDs while `MutateCtx::remove_child` detaches a subtree; Masonry's normal
focus rewrite then rebuilds the path and clears ancestor flags, and
`RenderRoot::handle_access_event` ignores actions aimed at Masonry's synthetic
Window node. `ClayShellWidget` transfers focus before same-tab pane removal or
clears it while a replacement tab is still stashed; registry reconciliation
focuses the survivor after layout. The editor Entry path remains the valid
control case.

Regression coverage lives in `src/masonry_shell/mod.rs` and runs the real
`accesskit_consumer::Tree` through dirty-close rejection/discard, clean focused
pane removal, active-tab removal, and synthetic frame focus. The
environment-gated `tests/live_atspi_smoke.rs` remains available for ordinary
live tree/liveness coverage; blind AT-SPI focus actions are not used on hosts
where the event bridge can block. Remove the local dependency patch when
upstream Masonry carries equivalent focus invalidation and event-ingress
guards.

The prior transient-menu item-label ceiling follow-up remains resolved by Plan
087 Task 5. Original crash evidence is retained for provenance:
`code-reviews/screenshots/2026-08-14-plan086-a11y/focus-frame-crash.log` and
`manual-dirty-pane-close-crash.log`.

## Related

- [Masonry Shell Runtime](masonry-shell.md)
- [Masonry Editor Widget Status Observability](masonry-editor.md)
- [SDUI / Package-UI Retained Masonry Reconciliation](masonry-sdui-region.md)
- [Tabs and Independent Client Views](tabs-and-clients.md)
- [Protocol Codec](protocol-codec.md)
- [Configuration Runtime](configuration-runtime.md)
- [Maintenance Validation](maintenance-validation.md)
- `docs/development/accessibility.md`
- `docs/development/security.md`
- `plans/086-Audit-Remediation-P0-Release-Integrity-and-Accessibility.md`
