---
date: 2026-08-11 17:11
status: approved
decision_about: "Command Centre architecture: one surface, two modes, sequence keybindings, fuzzy matching, scrim backdrop"
proposed_by: "both"
explicitly_approved_by_user: true
---

# Decision: Command Centre as one floating surface with command and path modes, built by wiring existing server-owned state

## Decision

Clay gains a Command Centre: a single keyboard-first floating overlay
(Spotlight-style, centred, over a translucent scrim) with two modes sharing
one `TransientMenuSession`-based surface:

1. **Command execution mode** — lists every executable command (built-in
   command table, `shell.client*` family, package-registered commands) with
   package provenance and key binding shown per item; activation routes
   through the shared `CommandExecutor` (or the client shell-command path for
   `shell.client*`).
2. **Path mode** — dired-style filesystem browsing: editable path bar seeded
   with the active document's directory, bounded depth-1 listings,
   filter-as-you-type, descend/ascend navigation, open file into the active
   pane of the current tab, or open a directory as the current tab's
   workspace via `TabRegistry::open_workspace`.

Supporting decisions confirmed by the user:

- The client keybinding router (`route_key` in `src/client/behavior.rs`) is
  extended from single-stroke to multi-stroke sequences with pending-chord
  state, timeout, and cancel-on-mismatch, so Emacs-style chords are bindable.
- Filtering uses real fuzzy matching (Clay-owned subsequence scorer with
  ranking), shared by all transient menus — not the existing substring filter.
- The backdrop is a translucent scrim via new theme tokens only; no custom
  blur is implemented beyond what Masonry 0.4/Vello provide upstream.
- A file selected in path mode opens in the active pane of the current tab,
  preserving per-tab workspace isolation.
- Filesystem browsing outside granted workspace roots is authorized only by
  user-driven navigation inside this built-in surface (browse grant);
  conversions to explicit `SingleFile`/`Directory` grants happen on open;
  package code receives no equivalent authority.

Implementation is phased as Phase 24.1–24.5 in `roadmap.md` and reuses the
existing `ControlCenter`, `TransientMenuSession`, `FileBrowserState`,
built-in command table, and per-tab workspace binding — the bulk of the work
is protocol and client wiring of already-tested server-owned state.

## Context

The user requested an Emacs-like command centre with two use cases
(filesystem browsing with workspace loading, and searchable command
execution) and asked for a proposal that takes the current implementation
into account. A codebase sweep showed the state layer already exists but is
unwired: `ControlCenter` (`src/server/control_center.rs`) is complete and
tested but marked `#![allow(dead_code)]`; `TransientMenuSession` and its
Masonry overlay projection are exercised only in tests; no protocol messages
carry menu query/selection updates; `FileBrowserState` is live but scoped to
workspace roots with no dired-style path bar.

## Approval

- Proposed by: both (agent proposal, user answered four open decisions)
- Approved by user: Yes
- Approval evidence: The user replied "Based on these, and your proposal
  which I accept, create the phases for the implementation" after answering:
  (1) extend route keys to sequences for Emacs-style keybindings,
  (2) fuzzy matching is required, (3) scrim instead of true blur, nothing
  beyond Masonry/Vello upstream, (4) files open in the active pane of the
  current tab.

## Alternatives Considered

1. **Emacs-style minibuffer anchored at the bottom** — Rejected by the user.
   The existing `TransientMenuOrigin::CommandPalette` bottom anchor stays for
   completion pickers; the Command Centre uses a new centred floating origin.
2. **Two separate UIs for commands and file browsing** — Rejected. One
   surface with two session kinds reuses one interaction round-trip, one
   matcher, and one overlay anchor, matching the Emacs minibuffer precedent
   where `M-x` and `C-x C-f` share one input mechanism.
3. **Keep substring filtering** — Rejected by the user; fuzzy matching with
   ranked subsequence scoring is required and shared across transient menus.
4. **True backdrop blur** — Rejected for now. Masonry 0.4/Vello has no cheap
   backdrop-filter pass; a translucent scrim approximates the Spotlight look
   without custom render-pipeline work. Revisit only if upstream gains filter
   passes.
5. **Native OS file/folder dialogs as the primary open flow** — Rejected as
   primary (kept as fallback in `src/client/file_dialog.rs`); the user wants
   an in-app dired-style keyboard flow.
6. **Package-accessible arbitrary filesystem browsing** — Rejected. Browse
   authority is bound to the built-in user-driven surface; packages keep the
   existing workspace-root grant model.

## Rationale and Evidence

- `src/server/control_center.rs` already implements command listing,
  filtering, provenance/keybinding detail strings, and execution through
  `CommandExecutor`; only the UI round-trip is missing.
- `src/shell/transient_menu.rs` and `src/masonry_sdui.rs`
  (`set_active_menu`, overlay observations, z-order tokens) already project
  menu sessions to the client; `TransientMenuOrigin` is designed to be
  extended with new anchors.
- `src/shell/file_browser.rs` and `src/server/workspace.rs` provide bounded,
  grant-scoped listing machinery; `OpenFilePieces`/root-grant flows cover
  converting a picked path into an explicit grant.
- `src/server/tab_registry.rs::open_workspace` and `connection.rs` per-tab
  snapshot push already implement loading a workspace into a tab.
- `src/client/behavior.rs::route_key` currently matches only
  `sequence.len() == 1`, which is why multi-stroke chords need a router
  extension rather than configuration alone.

## References

- `src/server/control_center.rs`, `src/shell/transient_menu.rs`,
  `src/shell/file_browser.rs`, `src/server/command_execution.rs`,
  `src/server/tab_registry.rs`, `src/server/workspace.rs`,
  `src/client/behavior.rs`, `src/masonry_sdui.rs`, `src/masonry_shell.rs`.
- `roadmap.md` — Phase 24: Command Centre (24.1–24.5).
- `decision-logs/2026-06-27-2014-unified-user-authorized-package-authority.md`
  — authority model the browse grant extends.
- `decision-logs/2026-06-09-1431-clay-owned-shell-layout-and-package-ui-contribution-model.md`
  — Clay-owned shell/overlay rendering model.
