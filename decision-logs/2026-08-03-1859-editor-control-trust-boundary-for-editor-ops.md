---
date: 2026-08-03 18:59
status: approved
decision_about: "Third-party access trust boundary for Clay editor ops (`editor-control`)"
proposed_by: "agent"
explicitly_approved_by_user: true
---

# Decision: Mode-scoped `editor-control` trust boundary for editor ops

## Decision

First- and third-party packages may access Clay's seven editor ops and trigger programmatic execution, but only under a mode-scoped deny-by-default boundary: a new `editor-control` package permission plus an explicit `clay.editorControl.modes` declaration; every op call is enforced server-side against the currently active major mode; conflicting packages coexist and the user resolves conflicts by deactivating packages. Programmatic triggering uses a new bounded advisory server→client push message (`ServerMessage::EditorCommandRequest`, protocol v8) that the client dispatches through the same path as keybinding-routed command IDs.

## Context

Plan 071 shipped movement/selection/caret/multi-cursor/text-object editing with all `op_clay_editor_*` ops registered trusted-only. The plan's follow-up list required a trust-domain decision: whether (and how) adopted third-party packages may call movement/selection ops. The user required: (1) packages must declare the specific modes they operate in; (2) Clay allows editor ops only while the editor is in that mode; (3) when multiple packages conflict in one mode, the user deactivates packages rather than Clay arbitrating; and (4) a recommended trust boundary for safe package use.

## Approval

- Proposed by: agent (boundary recommendation presented after source-grounded research)
- Approved by user: Yes — "The boundary is approved. Do the full implementation in this round already."
- Scope approved: both halves — gating + the `EditorCommandRequest` push channel (option b).

## Alternatives Considered

1. **Keep ops trusted-only forever** — rejected: blocks the stated product goal (packages controlling the editor in their modes); keymap override alone covers only key-driven behavior, not programmatic triggering (AI/snippet/macro flows).
2. **Ambient gate only (no execution channel)** — rejected: without a push path, ops only return descriptors and packages still cannot trigger behavior gesture-free; the user approved the full round.
3. **Synchronous cross-domain query for active mode** (third-party op round-trips to the trusted worker) — rejected: blocking op-to-worker round trips add latency/deadlock surface; the push-based replicated snapshot (`RuntimeCommand::UpdateActiveEditorMode`) is simpler and matches existing bridge direction.
4. **Automatic priority arbitration between conflicting packages** — rejected: adds machinery the user did not want; explicit user deactivation is the chosen resolution and matches Clay's adoption revoke/disable lifecycle.
5. **Wildcard mode declarations** — rejected for v1: deny-by-default favors exact mode IDs; wildcards are a future decision.

## Rationale and Evidence

- Provenance mechanism reused, not invented: `require_current_package_capability` (host-owned executing-package context; declarations cannot override identity) is the same gate `register_pattern` uses (`src/server/ops/mod.rs`).
- Active-mode truth source: `ModeRegistry.active_major_modes` + active manifest `BehaviorScope::Document` — the exact state JS-facing mode activation publishes (`publish_mode_behavior_manifest`), used both in production open-time activation (`classify_open_document` runs `serverActivateClassifiedMode` inside the trusted runtime) and in package-driven activation.
- The third-party worker holds its own `ClayOpState` (no shared mode registry), so the gate uses a host-replicated snapshot pushed over the existing `third_party_commands` bridge channel — same topology as Plan 061 task 12.
- Safety posture: editor ops touch client-local cursor/selection state only — no buffer mutation, filesystem, network, or shell. Worst case for a hostile granted package is caret/selection disturbance in exactly the modes the user approved. Grants are mode-scoped, generation-scoped (hot reload drops stale state), revocable, and never wildcard.
- Client hardening: pushed command IDs are re-parsed deny-by-default (`EditorClientCommand::from_command_id` / `SelectionQuery::from_command_id`); non-editor IDs (e.g. `clay.application.quit`) are rejected server-side and dropped client-side, so the channel can never execute arbitrary commands.
- Override semantics come mostly for free: packages owning a mode already contribute `keymaps` + `editorRules`, and manifest routing beats hardcoded widget defaults.

## References

- `plans/071-Editor-Movement-Selection-Caret-Ligatures.md` — tasks 18–21 (implementation notes, gate results).
- `src/server/ops/editor.rs::require_editor_control` — the gate.
- `src/packages/manifest.rs::parse_editor_control` — declaration validation.
- `src/protocol/editor_control.rs` — push wire type; `PROTOCOL_VERSION` 7→8.
- `src/server/js_runtime.rs` — `RuntimeCommand::UpdateActiveEditorMode`, `editor_commands` broadcast, gate tests.
- `docs/reference/packages/creating-packages.md` — "Editor Control" authoring section.
- `docs/wiki/modules/editor-movement-selection-caret.md` — trust-boundary wiki section.

## Consequences

- Positive: packages (first- and third-party) gain safe, revocable, mode-scoped programmatic editor control; the execution channel is advisory and cannot block editing; conflict resolution stays with the user.
- Known ceiling: documents editing under the bare default manifest (no registry-activated mode, e.g. built-in `core.*` fallbacks that never pass through mode activation) report no active mode — package callers deny there by design. Revisit if/when built-in modes activate through the registry.
- The `clay:editor` facade module remains admin/trusted-only; third parties call the shared ops directly. If a public editor facade is wanted for third parties, that is a separate classification change.
- Revisit conditions: multi-stroke chords landing (Helix-style bindings), wildcard mode scopes, or a product need for automatic conflict arbitration.
