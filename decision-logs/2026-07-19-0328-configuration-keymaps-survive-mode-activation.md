---
date: 2026-07-19 03:28
status: approved
decision_about: "Configuration keymap precedence across mode activation"
proposed_by: "agent"
explicitly_approved_by_user: true
---

# Decision: Preserve configuration keymaps across mode activation

## Decision

Keymaps registered by `init.js` during configuration evaluation are durable overlays. Package major-mode activation applies mode commands/keymaps first, then reapplies configuration keymaps so user chords win conflicts and survive document classification.

## Context

Opening a selected file activated its language package by replacing the behavior manifest with `BehaviorManifest::minimal_text_editing` plus package rules. This erased configured bindings, including `Ctrl+S`, so save appeared inert after a file opened even though direct save protocol handling worked.

## Approval

- Proposed by: agent as root-cause fix
- Approved by user: Yes
- Approval evidence: User reported Ctrl+S did not persist files and explicitly requested fixing the first issue.

## Alternatives Considered

1. **Hardcode Ctrl+S in Rust** — rejected because Clay keybindings are configured through documented `bindKey` APIs.
2. **Merge the entire previous manifest** — rejected because old mode-specific commands/keymaps could leak into a newly activated mode.
3. **Route save natively regardless of manifest** — rejected because it would mask the broader loss of every configured chord.

## Rationale and Evidence

`ClayOpState` already knows whether an operation runs during configuration evaluation. Recording only those `bindKey`/`unbindKey` rules cleanly separates user configuration overlays from package mode keymaps. Reapplying overlays in `publish_mode_behavior_manifest` fixes all configured chords at the shared replacement point and gives explicit user configuration precedence.

`src/client/mod.rs::tests::selected_file_edit_then_save_persists_and_reports_clean` starts a real Unix IPC server with a Ctrl+S configuration, opens and activates a selected Rust file, verifies the binding survives, sends edit then save, and checks clean `DocumentSaved` metadata plus disk bytes. Linux formatting, clippy, and all-target tests pass.

## References

- `src/server/ops/mod.rs` — configured keymap storage and mode-manifest overlay.
- `src/client/mod.rs` — end-to-end regression test.
- `docs/wiki/modules/embedded-js-runtime.md`
- `docs/wiki/modules/masonry-editor.md`

## Consequences

- User `init.js` chords persist across selected-file opens, mode switches, and package activation.
- Configuration bindings override package bindings for the same context/chord.
- Package-only mode bindings do not leak into later modes.
- This changes no filesystem, package, shell, network, or AI authority.
