---
date: 2026-08-31 17:45
status: approved
decision_about: "Delete the dead Phase 19 native dialog backends; the Tauri portal command is the only dialog implementation"
proposed_by: "agent"
explicitly_approved_by_user: true
---

# Decision: Delete the dead native dialog backends; Tauri ashpd portal command is the only dialog path

## Decision

Delete `src/client/file_dialog.rs` (Windows COM `IFileOpenDialog`, Linux `zbus` portal, macOS `NSOpenPanel` backends plus the neutral `FileDialogFilter`/`FileDialogResult` types) and its re-exports from `src/client/mod.rs`. The sole file/folder dialog implementation is the Tauri-side `ashpd` XDG file-chooser portal command in `src-tauri/src/commands.rs` (`dialog_open_file`, `dialog_open_folder`, `tab_open_dialog`) feeding the unchanged server selected-path grant flow. Platforms whose portals are unavailable receive the sanitized `file dialog failed` diagnostic; native Windows/macOS pickers are long-term platform targets to be added at that same command seam.

## Context

The 2026-08-31 repository review found the ~630-line `src/client/file_dialog.rs` module with zero callers: the only reference was the `pub use` re-export in `src/client/mod.rs`, and no `src/`, `src-tauri/`, or test code called `open_markdown_file_dialog`/`open_folder_dialog`. The React client reaches dialogs exclusively through `invoke("dialog_open_file" | "dialog_open_folder" | "tab_open_dialog")` (`frontend/src/bridge/client.ts::openFileDialog`/`openFolderDialog`), whose implementation lives in `src-tauri/src/commands.rs::pick_path` (ashpd XDG file-chooser portal, per-dialog busy locks, grant feed via `BridgeState::accept_selected_path`). The deleted Rust backends contained all 11 `unsafe` uses in the core crate, were never compiled by Linux CI (Windows/macOS gates were untested), and contradicted `docs/wiki/index.md`, which already stated the backends were "removed in Plan 097 Phase 12". Keeping never-compiled `unsafe` platform code contradicts the wiki and the security posture.

## Approval

- Proposed by: agent (comprehensive repository review 2026-08-31, plan `plans/105-Repository-Review-Remediation.md`, P1-1 task; user requested plan execution task by task).
- Approved by user: Yes
- Approval evidence: user instruction to complete the next task "Delete the unreferenced native dialog backends and align the wiki (review P1-1)" from the approved plan; the plan's Options Considered (delete vs. keep-and-fix-wiki) was reviewed as part of plan creation.

## Alternatives Considered

1. **Keep the backends and "fix" the wiki to claim they are live** — rejected: zero callers anywhere in `src/` or `src-tauri/`; Linux CI never compiles the Windows/macOS gates, so the code would rot silently and the doc would drift in the other direction instead.
2. **Rewrite the backends as the Tauri implementation** — rejected: `src-tauri/src/commands.rs` already has a narrower, tested, busy-lock-protected portal implementation (`pick_path`) wired to the server grant paths; two dialog implementations would be duplication with no second consumer.
3. **Delete the module and align docs** — selected: matches the already-documented Plan 097 Phase 12 intent, removes all 11 `unsafe` uses in the core crate, and makes the Linux ashpd path the single documented implementation with Windows/macOS as explicit, honestly-documented targets.

## Consequences

- The documented dialog surface (Clay JS APIs `documents.clientOpenFileDialog`, `workspace.clientOpenFolderDialog`, the `bindKey` command IDs, and the server grant/capability flow) is unchanged; only internal implementation owners moved to `src-tauri/src/commands.rs` + `frontend/src/bridge/client.ts`.
- Windows/macOS native file pickers do **not** exist in the current desktop bridge; dialogs work over the XDG portal (Linux). This documents a gap for the long-term Windows target, to be closed at the `src-tauri/src/commands.rs` command seam when that phase lands. This is a change from earlier doc claims ("working today" on all three platforms) that were already inaccurate after the Plan 097 cutover.
- Deleting the COM backends removes all 11 `unsafe` sites in the file; the remaining `unsafe` in `src/` is the narrow, SAFETY-commented `libc`/`env::set_var` usage.
- The maintenance rule "wiki pages and code must not contradict each other; platform-gated code that CI never compiles must be either wired to a real caller or deleted" is folded into `.agents/skills/project-patterns/references/maintenance-validation.md`.

## References

- `plans/105-Repository-Review-Remediation.md` (P1-1 task)
- `src-tauri/src/commands.rs::pick_path` / `dialog_open_file` / `dialog_open_folder` / `tab_open_dialog` — the retained implementation
- `docs/wiki/modules/client-file-dialog.md` (rewritten), `docs/development/launch-and-gui-smoke.md`, `docs/development/windows.md`, `docs/development/file-open-save-reload-workflow.md`, `docs/reference/clay-js-api/configuration.md`, `docs/reference/clay-js-api/{documents,workspace}/client-open-*-dialog.md`, `docs/reference/clay-js-api/api-inventory.toml`
- Plan 097 Phase 12 (original replacement decision), Phase 9 Tauri adapter
- Verification: `cargo check --all-targets`, `cargo clippy --all-targets -- -D warnings`, lib + protocol (200) + security (134) + presentation (40) suites green after deletion; `rg 'file_dialog|FileDialogFilter|FileDialogResult|open_folder_dialog|open_markdown_file_dialog' src tests src-tauri/src` returns no live references; `update-doc-registry` regenerated `docs/generated/clay-js-api-registry.json`.