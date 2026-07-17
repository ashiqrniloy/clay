# Phase 20: Daily Editing Product Hardening

## Objectives

- Make Clay usable for real daily editing sessions after the package/mode path has proven customizable editing and rendering.
- Ship the missing daily-use editor capabilities: clipboard cut/paste, undo/redo, IME/composition, accessibility polish, cross-platform native file dialogs, multi-document behavior, selected-file save/conflict UX, and richer pending-edit/resync recovery.
- Integrate those capabilities with package modes and server authority instead of bypassing them with client-only filesystem, JavaScript, or package-specific Rust branches.
- Revisit Phase 15's deferred pixel-buffer/GPU snapshot path now that Masonry 0.4 exposes `TestHarness` / `assert_render_snapshot`, while keeping structural observability as the fast headless default if pixel snapshots remain CI-brittle.
- Treat the Phase 18.15 theme registry as already delivered for the Phase 20 "theme system" roadmap item; only verify completeness and accessibility/theme polish rather than rebuilding themes.

## Expected Outcome

- Clay supports copy/cut/paste, undo/redo, IME composition, dirty/save/conflict flows, multi-document switching with per-document mode/status/lease/dirty/manifest metadata, and user-visible recovery prompts without granting packages or the client broad filesystem authority.
- Linux and macOS native file-open dialogs reuse Phase 18.8 command execution and Phase 18.12 selected-file grant/file-browser primitives the same way Windows already does.
- Daily-editing features remain off the ordinary typing/paint hot path except for local predictive edits, local preedit rendering, and inert behavior-manifest routing.
- Users, package authors, and agents can discover save/open/reload/clipboard/undo/document-session APIs through Clay JS docs, configuration surfaces, and the generated registry.
- Pixel-accurate snapshot coverage is either added for shipped editor/SDUI/mode compositions under Masonry's harness, or explicitly re-deferred with evidence that CI-friendly deterministic offscreen rendering is still insufficient.

## Tasks

- [x] Verify the Phase 20 entry gate and review existing daily-editing primitives before implementation
  - Acceptance Criteria:
    - Functional: Confirm Plan 054 / roadmap Phase 19 hot-reload semantics are complete; inventory current clipboard (copy-only), IME (`Commit` only), save/reload/dirty server primitives, file-dialog backends, single-editor replace-on-open behavior, status/recovery chrome, accessibility labels, theme registry (Phase 18.15), and structural UI observability against every Phase 20 focus area; record that theme ownership was pulled forward into Phase 18.15.
    - Performance: Identify which Phase 20 operations must remain client-local (copy/cut/paste read, preedit paint, undo inverse application, active-document switch chrome) versus server-first (save/reload/conflict, document list/switch authority, file dialog grant consumption) and confirm none block ordinary paint/layout/scroll beyond existing local-edit work.
    - Code Quality: Produce a primitive/gap matrix that prefers extending existing clipboard, workspace, command-execution, transient-menu, status-observation, and document-registry primitives over new parallel systems; prohibit mode/language-specific Rust branches.
    - Security: Record that clipboard read/write stays user-mediated and client-owned, save/open remain selected-file/workspace-root authorized, and no package, configuration, or AI path gains clipboard, filesystem, shell, network, or raw-op authority.
  - Approach:
    - Documentation Reviewed:
      - `roadmap.md` — Phase 20 focus areas/expected outcome; Phase 18.15 theme supersession note; Phase 15 pixel-snapshot deferral; Phase 21 multi-client scaling boundary.
      - `docs/wiki/modules/{masonry-editor,client-file-dialog,server-file-workspace,server-document-state,client-snapshot-bootstrap,editor-theme-registry,transient-menu-session,workspace-file-browser,persistent-runtime-hot-reload}.md`
      - `docs/development/{ui-observability,launch-and-gui-smoke,performance}.md`
      - `docs/reference/clay-js-api/{editor/client-copy-selection,documents/server-save-document,documents/client-open-file-dialog,workspace/client-open-folder-dialog}.md`
      - `docs/reference/primitives/{index,registry,syntax-vocabulary,typography}.md`
      - `.agents/skills/project-patterns/references/{authority-boundaries,behavior-manifests,protocol-and-performance,package-ui-layout,mode-primitive-first,planning-checklist,clay-js-api-naming,documentation-as-code,maintenance-validation}.md`
      - Local `masonry 0.4.0` / `masonry_core 0.4.0` IME + testing docs; local `arboard 3.6.1` clipboard API.
    - Options Considered:
      - Treat Phase 20 as greenfield product features: rejected; most persistence/authority primitives already exist and only lack daily UX.
      - Rebuild theme system in Phase 20: rejected; roadmap supersession already pulled themes into Phase 18.15.
      - Primitive-first inventory with explicit reuse/gap matrix, then decision log for contested ownership (undo, multi-doc, IME). Chosen.
    - Chosen Approach:
      - Wrote `docs/wiki/modules/phase20-daily-editing-product-hardening-primitive-review.md` mapping each focus area to existing primitives and concrete gaps. Entry gate passes on Plan 054 completion (no unchecked tasks); roadmap Phase 19 heading still lacks an explicit `Complete` suffix (documentation lag only). Pinned source evidence: copy-only `ClipboardSink`, `Ime::Commit` without preedit, no undo/redo stack, `opening_second_file_browser_file_replaces_editor_snapshot`, Linux/macOS file-open still `Unsupported`, server save/dirty/conflict present but status chrome has no dirty/conflict UX, Masonry `TestHarness`/`assert_render_snapshot` available but Clay does not depend on `masonry_testing` yet, Phase 18.15 theme registry already delivered.
    - API Notes and Examples:
      ```text
      Existing: clay.editor.clientCopySelection -> SystemClipboard.set_text(selection)
      Existing: WorkspaceState::{save_document,reload_document} + dirty/stale metadata
      Existing: open_markdown_file_dialog() Windows-only; open_folder_dialog() Linux portal
      Gap: cut/paste, undo/redo, Ime::{Enabled,Preedit,Disabled}, multi-doc session, Linux/macOS file open, save conflict UX
      ```
    - Files to Create/Edit:
      - `docs/wiki/modules/phase20-daily-editing-product-hardening-primitive-review.md`: inventory, gaps, ownership, budgets, rejected alternatives.
      - `docs/wiki/index.md`: link the primitive review.
      - `tests/primitives_docs.rs`: deterministic coverage for the Phase 20 entry gate and generic gap inventory.
    - References:
      - `.agents/skills/create-plan/references/clay.md`
      - `.agents/skills/project-patterns/references/mode-primitive-first.md`
      - `decision-logs/2026-06-04-1923-replace-markdown-parser-with-markdown-it-and-primitive-first-mode-planning.md`
      - `docs/wiki/modules/phase20-daily-editing-product-hardening-primitive-review.md`
  - Test Cases to Write:
    - `cargo test --test primitives_docs phase20_daily_editing_product_hardening_primitive_review`: requires entry-gate evidence, Phase 18.15 theme supersession note, all focus-area gap rows, no-hot-path rule, and security boundaries.
    - `cargo test --test primitives_docs`
    - `cargo fmt --check`
  - Completion Evidence:
    - Entry gate: Plan 054 and Plan 046 have zero unchecked tasks; Phase 20 may proceed.
    - Review published and indexed; `cargo test --test primitives_docs phase20_daily_editing_product_hardening_primitive_review` and full `primitives_docs` suite pass.
    - Preferred defaults recorded for the next semantics-decision task: client inverse-edit undo, clipboard `get_text` + cut/paste, local IME preedit until commit, multi-doc client session map keyed by server `DocumentId`.

- [x] Approve and record undo/redo, clipboard cut/paste, IME composition, and multi-document session semantics
  - Acceptance Criteria:
    - Functional: Compared realistic ownership alternatives for undo history, clipboard paste authority, IME preedit commitment, and multi-document active-session model; obtained explicit user approval; recorded exact semantics in an approved decision log before implementation.
    - Performance: Approved design keeps preedit/undo/clipboard local application off IPC waits; save/conflict/switch authority remain server-first/background relative to paint; undo stacks and open-document session metadata are bounded.
    - Code Quality: One coherent model for inverse-edit undo, user-mediated clipboard read on paste, non-server-committed preedit, and per-document client session state keyed by server `DocumentId`.
    - Security: Selected-file grants, leases, and ordinary edit validation remain in force; Phase 20 does not invent package/config/AI clipboard-contents or bypass paths; broader package/configuration/AI clipboard/filesystem/shell/network/raw-op authority is deferred to a later decision.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/create-decision-log/SKILL.md`
      - Primitive review from the previous task
      - `docs/wiki/flows/{versioned-text-synchronization,client-server-edit-ack,client-behavior-routing}.md`
      - `src/{client/clipboard,masonry_editor,editor/surface,server/workspace,server/document}.rs`
      - Local `arboard 3.6.1` (`Clipboard::get_text` / `set_text`); local `masonry_core 0.4.0` `Ime` enum docs.
    - Options Considered:
      - Server-owned undo log with dedicated undo protocol: richer collaboration story, but larger Phase 20 surface and Phase 21 overlap.
      - Client-only undo that rewrites local rope without server transactions: breaks server authority and multi-client consistency.
      - Client undo/redo stack that applies inverse ranges as ordinary optimistic edits (server validates as normal `Edit`s). Preferred default for Phase 20.
      - Commit IME preedit bytes continuously as edits: rejected; creates noisy versions and broken composition.
      - Local preedit overlay until `Ime::Commit`, then one ordinary insert/replace edit. Preferred.
      - Continue replace-on-open single buffer: rejected by Phase 20 multi-document requirement.
      - Client multi-document session retaining per-document shadow/caret/viewport/pending/mode/status while server remains registry/lease authority. Preferred.
      - Finalize permanent package/config/AI denial of clipboard/filesystem/shell/network/raw-op authority in this decision: rejected by user; deferred.
    - Chosen Approach:
      - Presented the preferred defaults, obtained user approval with one amendment (defer package/config/AI authority), and recorded `decision-logs/2026-07-17-1841-phase20-daily-editing-semantics.md`:
        - Undo/redo: per-document bounded client stack (256) of inverse operations emitted as normal edits under the editable lease; clear on full resync/hard open-replace; cancel unfinished IME before undo/redo.
        - Clipboard: extend `ClipboardSink` with `get_text`; cut = copy + delete selection; paste inserts clipboard text as an ordinary local edit; no server clipboard proxy; Phase 20 does not invent package/config/AI clipboard-contents APIs.
        - IME: handle `Enabled`/`Preedit`/`Commit`/`Disabled`; preedit is paint-only; commit becomes one edit; set IME area for candidate UI; cancel on focus loss/document switch.
        - Multi-document: server list/open/switch authority + client session map keyed by `DocumentId` (bound 64); opening another file no longer destroys prior session state.
        - Deferred: package/configuration/AI clipboard, filesystem, shell, network, and raw-op authority.
    - API Notes and Examples:
      ```text
      Undo stack entry: { inverse insert|delete|replace, caret/selection restore }; depth 256
      Paste: arboard::Clipboard::get_text() -> local insert/replace -> ClientMessage::Edit
      IME: Preedit(text, cursor) paints overlay; Commit(text) clears overlay and edits
      Multi-doc: WorkspaceState open registry + ClientDocumentSession { document_id, shadow, caret, viewport, pending, history, dirty_view }; max 64
      ```
    - Files to Create/Edit:
      - `decision-logs/2026-07-17-1841-phase20-daily-editing-semantics.md`: approved exact decision.
      - `.agents/skills/project-patterns/references/{authority-boundaries,protocol-and-performance}.md`: durable Phase 20 ownership/budget guidance.
      - `docs/wiki/modules/phase20-daily-editing-product-hardening-primitive-review.md`: link approved semantics; defer authority note.
      - `plans/055-Phase20-Daily-Editing-Product-Hardening.md`: record decision reference after approval.
    - References:
      - `.agents/skills/project-patterns/references/{authority-boundaries,protocol-and-performance,behavior-manifests}.md`
      - `decision-logs/2026-07-17-1841-phase20-daily-editing-semantics.md`
  - Test Cases to Write:
    - Manual decision-log review: approved status, explicit user approval, alternatives, limits, consequences, revisit conditions.
    - Manual project-pattern review: durable rules extracted without copying the full log.
    - `cargo test --test primitives_docs phase20_daily_editing_product_hardening_primitive_review`
    - `cargo fmt --check` and `git diff --check`
  - Completion Evidence:
    - User approved the recommended undo/clipboard/IME/multi-document semantics and amended that package/config/AI clipboard/filesystem/shell/network/raw-op authority must be established later.
    - Decision log, project-pattern updates, and primitive-review Approved Semantics section recorded; primitive-review docs test still passes.

- [x] Complete clipboard cut and paste on the existing client clipboard primitive
  - Acceptance Criteria:
    - Functional: Explicit user cut/copy/paste shortcuts and bindable client UI command IDs work on Linux/Windows/macOS modifier conventions; cut copies then deletes the selection as one user gesture producing ordinary edits; paste inserts clipboard UTF-8 text at caret or replaces selection; collapsed cut/copy remain no-ops; clipboard failures become sanitized runtime diagnostics.
    - Performance: Clipboard OS read/write happens only on explicit cut/copy/paste commands, never during paint/layout/scroll or ordinary key insertion; paste does not block the GUI on IPC acknowledgement.
    - Code Quality: Extend `ClipboardSink` / `SystemClipboard` rather than scattering `arboard` calls; reuse selection extraction and edit enqueue paths; keep package/mode code free of clipboard branches.
    - Security: Clipboard read is limited to explicit paste in Phase 20; no server clipboard proxy; diagnostics never include full clipboard contents; package/config/AI clipboard authority remains deferred per `decision-logs/2026-07-17-1841-phase20-daily-editing-semantics.md`.
  - Approach:
    - Documentation Reviewed:
      - `docs/wiki/modules/masonry-editor.md`, `docs/reference/clay-js-api/editor/client-copy-selection.md`
      - Local `arboard 3.6.1` API: `Clipboard::new`, `get_text`, `set_text`
      - Approved Phase 20 decision log
    - Options Considered:
      - Add a server clipboard proxy: rejected; clipboard is a client OS resource and must stay user-mediated.
      - Extend the existing client sink + bindable command IDs. Chosen.
    - Chosen Approach:
      - Add `get_text` to `ClipboardSink`, implement cut/paste helpers on `EditorWidget`, route `Ctrl/Cmd+X` and `Ctrl/Cmd+V` (plus bindable command IDs), and document new Clay JS command-ID facades beside copy.
    - API Notes and Examples:
      ```rust
      pub trait ClipboardSink {
          fn set_text(&mut self, text: String) -> Result<(), ClipboardError>;
          fn get_text(&mut self) -> Result<String, ClipboardError>;
      }

      let mut clipboard = SystemClipboard;
      let text = clipboard.get_text()?;
      // insert/replace through existing local edit + enqueue path
      ```
      ```ts
      import { clientPasteClipboard, clientCutSelection, bindKey } from "clay:...";
      bindKey("Ctrl+V", clientPasteClipboard(), { scope: "editor" });
      ```
    - Files to Create/Edit:
      - `src/client/clipboard.rs`: add `get_text`; keep fake/memory sinks testable.
      - `src/masonry_editor.rs` / `src/main.rs`: cut/paste routing and diagnostics.
      - `src/editor/surface.rs`: selection delete/replace helpers if missing.
      - `runtime/js/editor.ts`, `docs/reference/clay-js-api/editor/client-{cut-selection,paste-clipboard}.md`, `docs/index.md`
      - `docs/wiki/modules/masonry-editor.md`, `docs/development/launch-and-gui-smoke.md`
    - References:
      - `.agents/skills/project-patterns/references/{authority-boundaries,clay-js-api-naming,documentation-as-code}.md`
  - Test Cases to Write:
    - Unit: cut copies and deletes selection; copy unchanged; paste inserts/replaces; empty selection no-op; get/set failure diagnostics.
    - Fake clipboard sink tests without requiring a desktop clipboard.
    - Manifest/command-ID routing tests for cut/paste bindable IDs.
    - `cargo test -p clay --lib client::clipboard`
    - `cargo test -p clay --lib masonry_editor`

  - Completion evidence (2026-07-17):
    - Extended `ClipboardSink` / `SystemClipboard` with `get_text`; added `read_text_from_system_clipboard`; fake/memory sink tests cover set/get/failure without a desktop clipboard.
    - `EditorWidget` cut = copy then ordinary `DeleteForward` local edit; paste reads clipboard, normalizes line endings via `normalize_clipboard_paste_text` / `EditorSurface::paste_text_with_event`, and inserts/replaces as an ordinary local edit. Collapsed cut/copy and empty paste are no-ops; write/read failures emit sanitized `clay.client.clipboard.write_failed` / `read_failed` diagnostics without clipboard contents.
    - Native chords: `Ctrl/Cmd+X` cut, `Ctrl/Cmd+C` copy, `Ctrl/Cmd+V` paste. Bindable command IDs: `clay.editor.clientCutSelection`, `clay.editor.clientCopySelection`, `clay.editor.clientPasteClipboard` routed through `main.rs` ClientUiCommand handling and keybinding allowlist.
    - Clay JS facades + docs: `runtime/js/editor.ts`, `docs/reference/clay-js-api/editor/client-{cut-selection,paste-clipboard}.md`, `docs/index.md`, inventory + regenerated `docs/generated/clay-js-api-registry.json`.
    - Tests: `cargo test -p clay --lib clipboard|cut_selection|copy_selection|paste_text`; `cargo test -p clay --bin clay client_`; facade/doc-registry/manual-smoke checks for the new command IDs.

- [x] Implement per-document undo and redo as ordinary inverse edits
  - Acceptance Criteria:
    - Functional: Undo/redo restore text and caret/selection for the active editable document according to the approved decision; redo stack clears on new divergent edits; read-only observers cannot undo; full resync/open behavior follows the approved clamp/clear rules; default chords `Ctrl/Cmd+Z` and `Ctrl/Cmd+Shift+Z` (or platform redo equivalent) are bindable.
    - Performance: Undo/redo apply locally first and enqueue inverse edits through the existing bounded queue; stack depth and entry payload are bounded; no full-document IPC for ordinary undo.
    - Code Quality: Generic per-document history primitive reused by all modes; no Markdown/language-specific undo logic; history stored beside client session state, not in package JS.
    - Security: Undo cannot bypass leases, region locks, or server validation; rejected undo edits recover through existing resync paths rather than silently diverging.
  - Approach:
    - Documentation Reviewed:
      - Approved decision log; `docs/wiki/flows/versioned-text-synchronization.md`
      - `src/editor/surface.rs`, `src/client/mod.rs`, `src/server/document.rs`
    - Options Considered:
      - Server undo log: deferred unless decision review requires it.
      - Client inverse-edit stack emitting normal `Edit` transactions. Chosen (approved in decision log).
    - Chosen Approach:
      - Record coherent local edit operations (insert/delete/replace) into a bounded per-document deque; undo pushes inverse onto redo; unfinished IME preedit cancel hooks land with the IME task (no preedit state yet).
    - API Notes and Examples:
      ```rust
      session.history.push(EditInverse { range, prior_text, caret_before, caret_after });
      let inverse = session.history.undo()?;
      surface.apply_local_edit(inverse);
      queue.try_send_edit(inverse.into_client_message());
      ```
    - Files to Create/Edit:
      - `src/editor/history.rs` (or equivalent module): bounded undo/redo stack.
      - `src/editor/surface.rs`, `src/masonry_editor.rs`, client session types: record and apply history.
      - `runtime/js/editor.ts`, docs for `clientUndo` / `clientRedo` command IDs.
      - Wiki + smoke docs.
    - References:
      - `.agents/skills/project-patterns/references/{authority-boundaries,protocol-and-performance,behavior-manifests}.md`
  - Test Cases to Write:
    - Undo insert/delete/replace restores text and caret.
    - Redo restores undone edit; new edit clears redo.
    - Stack depth ceiling drops oldest entries deterministically.
    - Read-only observer undo is rejected/no-op.
    - Resync behavior matches approved decision.
    - `cargo test -p clay --lib editor::history` (or module name chosen)

  - Completion evidence (2026-07-17):
    - Added generic `src/editor/history.rs` (`EditHistory` / `HistoryEntry`) with depth ceiling `EDIT_HISTORY_MAX_DEPTH = 256` and entry payload ceiling `EDIT_HISTORY_MAX_ENTRY_BYTES = 64 KiB`; oversized entries clear history instead of retaining unbounded text.
    - `EditorSurface` records every successful local insert/delete/replace into history, applies undo/redo as ordinary inverse/forward ops with caret/selection restore, and clears history on snapshot/resync/open-replace. Read-only observers are no-ops.
    - `EditorWidget::undo` / `redo` enqueue through the existing local-edit queue; native chords `Ctrl/Cmd+Z`, `Ctrl/Cmd+Shift+Z`, and `Ctrl+Y` (non-macOS). Bindable command IDs `clay.editor.clientUndo` / `clay.editor.clientRedo` with Clay JS docs, facade, inventory, and registry coverage.
    - Tests: `cargo test -p clay --lib history::`, `undo_`, `redo_`; masonry enqueue coverage; keybinding/client UI routing; `clay_js_facade_layout` / `clay_js_doc_registry` file-browser workflow API set.

- [x] Implement IME/composition preedit overlay and commit semantics
  - Acceptance Criteria:
    - Functional: `Ime::Enabled`/`Preedit`/`Commit`/`Disabled` are handled; preedit text and cursor span render at the caret without becoming canonical document text; empty preedit clears the overlay; commit inserts/replaces as one ordinary edit; candidate-window IME area is updated; unfinished composition is cancelled safely on focus loss/document switch according to the decision.
    - Performance: Preedit updates are local paint/layout invalidations only; no IPC/server/JS work per preedit event; commit uses the existing local-edit path.
    - Code Quality: Follow Masonry/winit IME contracts; keep composition state in the editor surface/widget, not packages; accessibility exposes composing state without leaking unfinished text to server logs.
    - Security: Preedit stays client-local; diagnostics do not record raw composition strings beyond sanitized failure codes; Phase 20 does not invent package composition-buffer APIs.
  - Approach:
    - Documentation Reviewed:
      - Local `masonry_core 0.4.0` `Ime` docs and `TextArea` preedit handling (`Ime::Preedit`, `set_ime_area`)
      - Local `winit 0.30` IME event semantics mirrored by Masonry
      - Current `EditorWidget` `TextEvent::Ime(Ime::Commit(_))` branch
    - Options Considered:
      - Embed Masonry `TextArea` as the editor: too large a rewrite for Phase 20.
      - Port the composition state machine patterns from Masonry `TextArea` into Clay's rope editor surface. Chosen.
    - Chosen Approach:
      - Add `CompositionState` on the editor surface; paint preedit with theme/typography-aware styling; on commit, clear composition and apply one edit; emit `ImeMoved`/IME area updates through existing Masonry signals where available.
    - API Notes and Examples:
      ```rust
      match ime {
          Ime::Enabled => { /* ready for preedit; update ime area */ }
          Ime::Preedit(text, cursor) => surface.set_preedit(text, cursor),
          Ime::Commit(text) => {
              surface.clear_preedit();
              surface.insert_or_replace(text); // ordinary edit
          }
          Ime::Disabled => surface.clear_preedit(),
      }
      ```
    - Files to Create/Edit:
      - `src/editor/surface.rs`, `src/masonry_editor.rs`: composition state, paint, event handling.
      - Possibly `src/editor/theme.rs` / typography metrics for preedit underline styling.
      - Wiki: `docs/wiki/modules/masonry-editor.md` and a short composition note in daily-editing review/impl wiki.
      - Smoke docs for manual IME validation on Linux/ibus or OS composition.
    - References:
      - Masonry `TextArea` IME handling; `.agents/skills/project-patterns/references/protocol-and-performance.md`
  - Test Cases to Write:
    - Preedit does not change canonical/shadow rope text or enqueue edits.
    - Commit after preedit inserts final text once and clears overlay.
    - Empty preedit clears overlay.
    - Disabled/focus-loss cancels composition per decision.
    - IME area updates requested when caret moves during composition.
    - `cargo test -p clay --lib` focused editor/IME tests

  - Completion evidence (2026-07-17):
    - Added `src/editor/composition.rs` (`CompositionState`) and wired it into `EditorSurface` with `set_preedit` / `cancel_composition` / `is_composing`.
    - `EditorWidget::on_text_event` handles `Ime::{Enabled,Preedit,Commit,Disabled}` plus window-focus-loss cancel; preedit is paint-only with underline overlay and optional cursor-span tint; commit clears overlay and inserts once through the ordinary local-edit path.
    - Masonry `set_ime_area` / `clear_ime_area` update candidate-window geometry from caret/preedit bounds during layout and IME events.
    - Unfinished composition cancels on Disabled, focus loss, pointer caret move, undo/redo, cut/paste, local edits, and hard open/resync (`load_snapshot`).
    - Accessibility label exposes `Composing.` without raw preedit text.
    - Docs: masonry-editor wiki IME section, Phase 20 primitive-review IME row marked done, smoke checklist IME note.
    - Tests: `editor::composition::*`, surface preedit/cancel/load/undo tests, masonry composing a11y + commit/undo composition tests.


- [x] Verify the Phase 18.15 theme system against Phase 20 requirements and apply only accessibility/theme polish gaps
  - Acceptance Criteria:
    - Functional: Confirm `StyleRegistry`, `setTheme`, Gruvbox Material themes, and inert `textStyles` contributions already satisfy the Phase 20 theme-system roadmap item; close any remaining accessibility contrast/status/theme-label gaps discovered during daily-editing UX work without inventing a second theme architecture.
    - Performance: No paint-path theme resolution regressions; theme switches remain configuration/reload-time work.
    - Code Quality: Do not reintroduce per-language color literals or package raw CSS; keep Phase 18.15 single source of color.
    - Security: Theme packages remain inert data only.
  - Approach:
    - Documentation Reviewed:
      - `roadmap.md` supersession note; `docs/wiki/modules/editor-theme-registry.md`; `docs/reference/primitives/syntax-vocabulary.md`; `docs/reference/clay-js-api/theme/*`
    - Options Considered:
      - Rebuild themes in Phase 20: rejected by supersession.
      - Verify + polish only. Chosen.
    - Chosen Approach:
      - Run theme/registry tests, review status/accessibility contrast needs for dirty/recovery chrome, and document Phase 20 theme item as satisfied by Phase 18.15 plus any tiny polish landed here.
    - API Notes and Examples:
      ```ts
      import { setTheme } from "clay:theme";
      await setTheme("@clay/theme-gruvbox-material-dark");
      ```
    - Files to Create/Edit:
      - Possibly small status/chrome token uses in `src/masonry_editor.rs` / shell theme tokens.
      - Wiki/roadmap cross-links clarifying supersession completion evidence in the Phase 20 review page.
    - References:
      - `decision-logs/2026-07-09-0352-tiered-tree-sitter-themable-syntax-vocabulary-theme-registry-and-opt-in-lsp.md`
  - Test Cases to Write:
    - Existing theme registry tests still pass.
    - Any new status/chrome token usages resolve through `StyleRegistry`.
    - Primitive docs test asserts Phase 20 theme item is marked satisfied-by-18.15.

  - Completion evidence (2026-07-17):
    - Verified Plan 046 / Phase 18.15 remains complete: `StyleRegistry`, `ActiveTheme`, `clay.theme.setTheme`, inert Gruvbox Material `textStyles`, and status/shell chrome tokens already satisfy the Phase 20 theme-system roadmap item (no second theme architecture).
    - Polish only: track active theme specifier on `EditorSurface`; expose `SduiStatusObservation.theme_label` + accessibility `Theme …` marker; add WCAG AA `status_chrome_meets_contrast` helpers/tests for Clay default and both Gruvbox Material themes.
    - Docs: Phase 20 primitive review Theme Supersession Note now records **satisfied by Phase 18.15**; editor-theme-registry / masonry-editor / ui-observability updated; `primitives_docs` asserts the satisfaction markers.
    - Tests: `editor::theme::clay_default_status_chrome_meets_aa_contrast`, `theme_display_label_strips_package_prefix`, `status_observation_exposes_active_theme_label`, `gruvbox_themes_status_chrome_meets_aa_contrast`, `phase20_daily_editing_product_hardening_primitive_review`.

- [x] Improve accessibility labels and roles for daily-editing surfaces
  - Acceptance Criteria:
    - Functional: Editor, status chrome, dirty/conflict state, multi-document active title, file-dialog failures, pending-edit/resync prompts, and SDUI panels expose stable AccessKit roles/labels that assistive tools and tests can inspect; accessibility updates remain consistent with visible status text.
    - Performance: Accessibility updates ride existing request paths after state changes; no per-keystroke full-tree rebuild beyond current Masonry behavior.
    - Code Quality: Centralize label composition helpers; keep structural observability tests asserting roles/labels.
    - Security: Labels use sanitized display names/diagnostics only — no absolute host paths, secrets, or clipboard contents.
  - Approach:
    - Documentation Reviewed:
      - `docs/development/ui-observability.md`; AccessKit usage in `src/masonry_editor.rs` / `src/masonry_sdui.rs`
      - Phase 15 accessibility notes in `roadmap.md`
    - Options Considered:
      - Defer accessibility to a later polish phase: rejected; Phase 20 explicitly includes it and daily UX depends on inspectable status.
      - Extend existing accessibility label/role hooks while adding dirty/multi-doc/recovery states. Chosen.
    - Chosen Approach:
      - Expand `accessibility_label` / SDUI accessibility children to include dirty marker, active document display name, composition-active flag, and recovery prompt summary; add observable assertions.
    - API Notes and Examples:
      ```rust
      node.set_role(Role::Document); // or existing editor role
      node.set_label(format!("{title}{dirty_marker}. {status}"));
      ```
    - Files to Create/Edit:
      - `src/masonry_editor.rs`, `src/masonry_sdui.rs`, possibly shell/file-browser widgets
      - `docs/development/ui-observability.md`, wiki masonry-editor page
    - References:
      - `.agents/skills/project-patterns/references/package-ui-layout.md`
  - Test Cases to Write:
    - Accessibility label includes dirty/active-document/recovery summary when set.
    - Status observation and accessibility stay consistent.
    - SDUI role/label structural assertions still pass.
    - `cargo test -p clay --lib masonry_editor`
    - `cargo test -p clay --lib masonry_sdui`

  - Completion evidence (2026-07-17):
    - Added `src/editor/accessibility.rs` centralized helpers for basename-only display names, dirty/composing/theme/recovery markers, and AccessKit label composition.
    - `EditorStatus` / `SduiStatusObservation` now carry `dirty`, `document_display_name`, `composing`, `pending_edit_count`, and `recovery_summary`; status text and accessibility stay consistent.
    - Document open installs sanitized display names; local edits mark dirty; active transient menus and file/conflict diagnostics feed recovery summaries.
    - SDUI/shell roots use `Role::Group`; active menus publish `Role::Menu`/`MenuItem` with item accessibility labels; editor remains `MultilineTextInput` with enriched status child.
    - Docs: ui-observability, masonry-editor, server-driven-ui, Phase 20 primitive review accessibility row marked done for daily-editing polish.
    - Tests: `editor::accessibility::*`, dirty/display-name/recovery/local-edit a11y tests, `active_menu_exposes_menu_role_and_item_accessibility_labels`, existing SDUI/editor accessibility suites green.

- [x] Add Linux and macOS native file-open dialogs that reuse selected-file grant primitives
  - Acceptance Criteria:
    - Functional: `clay.documents.clientOpenFileDialog` opens a native file picker on Linux (xdg-desktop-portal FileChooser) and macOS (native panel), returning selected path/cancel/unsupported/failure through the existing result API; selected paths still consume server-issued single-use capabilities and create single-file grants only; Windows behavior remains intact; folder dialog remains available where already implemented.
    - Performance: Dialogs run only on explicit user command; no dialog/IPC work in paint/typing paths.
    - Code Quality: Keep platform backends behind `src/client/file_dialog.rs` cfg seams; generalize filters beyond Markdown-only where the shared API already allows all-files fallback; no broad client filesystem scanning.
    - Security: Dialog selection does not grant sibling-directory authority; capability token rules from Plan 030/043 remain enforced; unsupported platforms report sanitized diagnostics.
  - Approach:
    - Documentation Reviewed:
      - `docs/wiki/modules/client-file-dialog.md`
      - Current `src/client/file_dialog.rs` Windows COM + Linux folder portal backends
      - xdg-desktop-portal Desktop FileChooser `OpenFile` docs; macOS `NSOpenPanel` patterns via existing project Windows counterpart structure
      - Phase 18.8 command execution + Phase 18.12 selected-file grant docs
    - Options Considered:
      - Depend on a third-party rfd-style crate: convenient, but adds abstraction and less control over filters/capability messaging.
      - Extend the existing platform-neutral backend with Linux portal open-file and macOS panel implementations. Chosen for consistency with current Windows code.
    - Chosen Approach:
      - Implement Linux `OpenFile` (non-directory) via the same portal stack as folder open; implement macOS native open panel behind `#[cfg(target_os = "macos")]`; keep returning `Unsupported` only for truly unavailable platforms; update docs/smoke for Linux primary and macOS secondary validation.
    - API Notes and Examples:
      ```rust
      // Linux portal OpenFile (directory=false) -> FileDialogResult::Selected(path)
      // macOS NSOpenPanel -> FileDialogResult::Selected(path)
      // existing: enqueue ClientMessage::OpenSelectedFile { path, capability }
      ```
    - Files to Create/Edit:
      - `src/client/file_dialog.rs`, `src/main.rs`
      - `docs/wiki/modules/client-file-dialog.md`, `docs/development/launch-and-gui-smoke.md`, Windows/macOS notes as needed
      - Possibly `docs/reference/clay-js-api/documents/client-open-file-dialog.md` platform matrix update
    - References:
      - `.agents/skills/project-patterns/references/authority-boundaries.md`
      - `docs/wiki/modules/server-file-workspace.md` capability-token rules
  - Test Cases to Write:
    - Linux/macOS cfg unit tests for filter model and unsupported fallbacks on other targets.
    - Existing Windows filter/cancellation tests remain green.
    - Capability-token rejection tests unchanged.
    - Manual smoke: Linux portal open file + selected-file grant; document macOS manual matrix.
    - `cargo test file_dialog`

  - Completion Evidence (2026-07-17):
    - Linux `open_markdown_file_dialog()` now uses xdg-desktop-portal `OpenFile` (non-directory) with Markdown/all-files portal filters (`a(sa(us))`, `*.*` → `*`); folder dialog still uses `directory=true`.
    - macOS `NSOpenPanel` backend added for file and folder open behind `#[cfg(target_os = "macos")]` with Markdown extension tokens + `allowsOtherFileTypes`.
    - Shared filter helpers: `portal_glob_for_pattern`, `macos_allowed_extensions`; unsupported platforms still return sanitized `Unsupported`.
    - Selected path still flows through existing `ClientMessage::OpenSelectedFile` / capability grant path (no sibling-directory authority).
    - Docs/smoke/wiki/API/platform matrix updated; registry regenerated; `cargo test file_dialog`, focused doc/smoke/inventory suites green.

- [ ] Revisit Masonry pixel-buffer snapshot coverage and add or explicitly re-defer it
  - Acceptance Criteria:
    - Functional: Investigate Masonry 0.4 `TestHarness` / `assert_render_snapshot` against Clay's `EditorWidget`/shell/SDUI compositions; either land a minimal deterministic snapshot suite for shipped editor/SDUI/mode compositions with fixed theme/typography/DPI inputs, or record evidence that Clay's custom Vello/Parley editor path still cannot use the harness reliably in CI and keep structural observability as the hard gate.
    - Performance: Snapshot tests remain opt-in or clearly separated from the fastest lib test path if expensive; structural tests stay the default fast layer.
    - Code Quality: No screenshot golden churn without pinned fonts/theme/window size; document update workflow.
    - Security: Harness does not open remote listeners, read user documents, or grant filesystem/shell authority.
  - Approach:
    - Documentation Reviewed:
      - `docs/development/ui-observability.md` deferred GPU snapshot section
      - Masonry 0.4 widget tests using `assert_render_snapshot!`
      - `roadmap.md` Phase 15/20 snapshot guidance
    - Options Considered:
      - Ignore Masonry harness and wait for a future release: rejected; Phase 20 requires an explicit revisit.
      - Adopt harness for pure SDUI/shell chrome first, keep custom editor text structural if editor snapshots are non-deterministic. Likely pragmatic split.
      - Full pixel suite for editor + SDUI + mode compositions if harness proves stable. Ideal if evidence supports it.
    - Chosen Approach:
      - Spike `TestHarness` on a reduced Clay shell/SDUI fixture; measure CI determinism; then either add bounded snapshot tests or write a short decision/evidence note re-deferring with updated prerequisites.
    - API Notes and Examples:
      ```rust
      let mut harness = TestHarness::create_with_size(properties, widget, Size::new(800., 600.));
      assert_render_snapshot!(harness, "clay_shell_empty_editor");
      ```
    - Files to Create/Edit:
      - Possibly `tests/ui_snapshots.rs` + `tests/snapshots/*.snap`
      - `docs/development/ui-observability.md`: either enablement guide or updated deferral evidence
      - `Cargo.toml` features/dev-deps if Masonry testing utilities require them
    - References:
      - Masonry testing widget docs (`testing_widget.md`); `.agents/skills/project-patterns/references/maintenance-validation.md`
  - Test Cases to Write:
    - Spike harness compile/run on Linux CI-like env.
    - If enabled: at least one shell/SDUI snapshot and one mode/package composition snapshot with fixed theme.
    - Structural observability tests remain passing regardless.
    - `cargo test --test ui_snapshots` (if created) and `cargo test -p clay --lib masonry_sdui`

- [ ] Implement multi-document session behavior with per-document mode, status, dirty state, leases, and manifest versions
  - Acceptance Criteria:
    - Functional: Opening a second file preserves the previous document session; users can switch active documents; each document retains mode selection, status, dirty state, lease/access, behavior/manifest generation metadata, caret/viewport, and pending-edit state according to the approved decision; server open-document registry remains authoritative for identity/leases; duplicate opens still follow read-only observer rules.
    - Performance: Document switch is local session activation plus any required server focus/status fetch; no full-text re-download when client already has the shadow; ordinary typing stays per-active-document ordered.
    - Code Quality: Generic session map keyed by `DocumentId`; no package-specific multi-doc UI branches; reuse shell slots/SDUI/transient menus for tab/list chrome rather than one-off widgets when practical.
    - Security: Switching documents does not expand workspace authority; closed documents release client access through existing server paths; display names stay sanitized.
  - Approach:
    - Documentation Reviewed:
      - `docs/wiki/modules/{server-file-workspace,server-document-state,masonry-shell,slot-aware-package-ui}.md`
      - `docs/reference/clay-js-api/documents/server-list-documents.md`
      - Approved multi-document decision semantics
    - Options Considered:
      - Full VS Code-like tabstrip with drag/split in Phase 20: likely overscope; splits already exist at shell level but tab UX can start minimal.
      - Minimal active-document switcher (list/commands/status) plus retained sessions. Preferred MVP.
      - Continue replace-on-open: rejected by roadmap.
    - Chosen Approach:
      - Introduce `ClientDocumentSession` map in the client/editor widget; change `DocumentOpened` handling to upsert rather than destroy sibling sessions; add server/client commands to activate/list documents; show dirty/active markers in status/accessibility; keep deep multi-client scaling for Phase 21.
    - API Notes and Examples:
      ```ts
      import { serverListDocuments, serverOpenDocument } from "clay:documents";
      const docs = await serverListDocuments();
      // activateDocument command focuses an already-open DocumentId
      ```
    - Files to Create/Edit:
      - `src/client/*`, `src/masonry_editor.rs`, `src/editor/surface.rs`, protocol messages if activation requires them
      - `src/server/workspace.rs` / connection dispatch for list/activate if missing pieces
      - Shell/SDUI contributions for document list/switcher
      - Clay JS docs for any new activate/list helpers
      - Wiki modules for multi-document session
    - References:
      - `.agents/skills/project-patterns/references/{authority-boundaries,package-ui-layout,protocol-and-performance}.md`
  - Test Cases to Write:
    - Opening a second file retains the first session's text/caret/dirty view.
    - Switching active document restores caret/viewport/mode status.
    - Lease/read-only observer semantics preserved per document.
    - Replace regression test `opening_second_file_browser_file_replaces_editor_snapshot` with retain/switch expectations.
    - `cargo test -p clay --lib` multi-document session tests
    - Protocol/workspace list tests

- [ ] Ship selected-file save/conflict persistence UX before save-after-open is user-facing
  - Acceptance Criteria:
    - Functional: Dirty state is visible in status/accessibility after accepted edits on selected-file and workspace documents; default save chord routes to `clay.documents.serverSaveDocument` (or a Clay-owned built-in wrapper) for the active document; stale-metadata conflicts preserve dirty text and present an explicit user choice (reload/overwrite-cancel/compare later) rather than silent overwrite; reload-of-dirty is blocked unless forced through the same recovery UX; save failures keep dirty true and show sanitized diagnostics.
    - Performance: Save/reload remain asynchronous server file IO; GUI stays responsive; no save work on the paint path.
    - Code Quality: Reuse `WorkspaceState::save_document` / `reload_document` and `TransientMenuSession` or status+command prompts; do not add client filesystem writes.
    - Security: Conflict UX cannot be used to open arbitrary paths; force-save still reauthorizes canonical paths; diagnostics sanitize host paths.
  - Approach:
    - Documentation Reviewed:
      - `docs/wiki/modules/server-file-workspace.md` dirty/stale-save behavior
      - `docs/reference/clay-js-api/documents/server-save-document.md`
      - `docs/wiki/modules/transient-menu-session.md`
    - Options Considered:
      - Autosave + file watchers in the same task: roadmap allows deferring to dedicated workflow docs if they outgrow Phase 9 wiki; keep out of MVP unless trivial.
      - Explicit dirty indicator + Ctrl/Cmd+S + conflict prompt. Chosen MVP.
    - Chosen Approach:
      - Thread dirty metadata from document status events into editor status; bind save; on `StaleFileMetadata` / dirty-reload conflicts, open a transient recovery menu with explicit actions that call existing server APIs.
    - API Notes and Examples:
      ```ts
      bindKey("Ctrl+S", "clay.documents.serverSaveDocument", { scope: "editor" });
      // conflict menu actions -> serverReloadDocument / serverSaveDocument(force?) / dismiss
      ```
    - Files to Create/Edit:
      - `src/masonry_editor.rs`, client connection events, status model
      - Server/protocol status events if dirty is not already pushed to the client on ack
      - Transient menu recovery flow
      - Docs + smoke checklist for save/conflict
    - References:
      - `.agents/skills/project-patterns/references/{authority-boundaries,behavior-manifests,package-ui-layout}.md`
  - Test Cases to Write:
    - Accepted edit marks dirty in client status; successful save clears it.
    - Stale save keeps dirty and surfaces conflict prompt/diagnostic.
    - Dirty reload without force is rejected and offered in recovery UX.
    - Selected-file grant documents save through the same path as workspace-root documents.
    - `cargo test` workspace save/conflict + editor status integration tests

- [ ] Add dedicated file-open/save/reload workflow documentation
  - Acceptance Criteria:
    - Functional: Publish a dedicated workflow doc covering selected-file open, workspace open, save, save-as (if implemented), reload, dirty state, conflict resolution, cancellation, unsupported dialog platforms, and capability-token behavior; link it from `docs/index.md` and development smoke docs; update Phase 9 module wiki rather than overloading it if the flow has outgrown that page.
    - Performance: Documentation only; no runtime cost.
    - Code Quality: Examples use Clay JS APIs and command IDs, not raw ops/protocol.
    - Security: Explicitly state Phase 20 non-goals (no client filesystem authority shortcuts, no Phase 20 package clipboard/path-scanning surfaces); broader package/config/AI authority deferred.
  - Approach:
    - Documentation Reviewed:
      - `docs/wiki/modules/server-file-workspace.md`, `docs/development/launch-and-gui-smoke.md`, Phase 9 notes in roadmap
    - Options Considered:
      - Keep expanding the Phase 9 wiki only: roadmap anticipates a dedicated doc when flows outgrow it.
      - Add `docs/development/file-open-save-reload-workflow.md` (name flexible) plus wiki links. Chosen.
    - Chosen Approach:
      - Write the workflow doc after save/conflict/multi-doc/dialog tasks land so examples match shipped behavior; include Linux/macOS/Windows matrices and manual smoke steps.
    - API Notes and Examples:
      ```ts
      import { loadPackage } from "clay:packages";
      import { clientOpenFileDialog } from "clay:documents";
      import { bindKey } from "clay:keybindings";
      await loadPackage("@clay/markdown");
      bindKey("Ctrl+O", clientOpenFileDialog(), { scope: "editor" });
      bindKey("Ctrl+S", "clay.documents.serverSaveDocument", { scope: "editor" });
      ```
    - Files to Create/Edit:
      - `docs/development/file-open-save-reload-workflow.md` (or final chosen name)
      - `docs/index.md`, `docs/development/launch-and-gui-smoke.md`
      - Possibly trim/point from `docs/wiki/modules/server-file-workspace.md`
    - References:
      - `.agents/skills/project-patterns/references/documentation-as-code.md`
  - Test Cases to Write:
    - Doc-link/registry or development-doc link check if such a gate exists; otherwise manual index-link verification.
    - Smoke doc steps reviewed against implemented commands.

- [ ] Add user-visible pending-edit, error, reconnect, and resync recovery UX
  - Acceptance Criteria:
    - Functional: Pending outbound edits, edit rejections, disconnects, and resync requirements are visible in status/accessibility and, when user action is required, through explicit recovery prompts/commands (request resync, reopen document, dismiss); reconnect/resync no longer relies only on stderr; recovery actions reuse existing `RequestResync` / open / reload primitives.
    - Performance: Status updates are event-driven on the GUI thread; recovery commands are explicit and non-blocking for paint.
    - Code Quality: Extend `EditorStatus` / `SduiStatusObservation` rather than inventing a second diagnostics channel; sanitize all messages.
    - Security: Recovery UX cannot escalate authority; messages omit secrets and unauthorized paths.
  - Approach:
    - Documentation Reviewed:
      - `docs/wiki/flows/client-server-edit-ack.md`
      - `docs/wiki/modules/masonry-editor.md`
      - Existing `ClientConnectionEvent` variants
    - Options Considered:
      - Modal blocking dialogs on every rejection: too noisy.
      - Status chrome + on-demand recovery prompt for actionable failures. Chosen.
    - Chosen Approach:
      - Track pending-transaction count / last rejection code in status; on disconnect show reconnect guidance; on resync-needed offer explicit resync command; keep automatic resync for the already-automated rejection classes where present.
    - API Notes and Examples:
      ```text
      status: Connected Editable · doc.md* · pending:2 · version:14
      recovery: "Edit rejected (stale). [Resync] [Dismiss]"
      ```
    - Files to Create/Edit:
      - `src/masonry_editor.rs`, `src/client/mod.rs`, possibly command registration for resync/recovery
      - Docs/smoke + accessibility assertions
    - References:
      - `.agents/skills/project-patterns/references/{protocol-and-performance,authority-boundaries}.md`
  - Test Cases to Write:
    - Pending count increments/decrements with enqueue/ack.
    - Disconnect updates status and accessibility.
    - Actionable rejection surfaces recovery affordance without panicking.
    - `cargo test -p clay --lib masonry_editor`

- [ ] Update the package UI/layout authoring contract for multi-document and recovery surfaces
  - Acceptance Criteria:
    - Functional: `docs/reference/packages/creating-packages.md` documents how packages interact (and do not interact) with multi-document sessions, dirty/save status, and recovery chrome; Clay remains owner of shell slots, document switcher, and native widgets; packages continue to contribute inert UI only.
    - Performance: No package paint-path requirements added.
    - Code Quality: Examples match shipped APIs; limitations and temporary fallbacks are explicit.
    - Security: Contract states Phase 20 does not give packages clipboard-contents APIs, arbitrary file writes, or direct native dialogs; broader package/config/AI authority remains deferred.
  - Approach:
    - Documentation Reviewed:
      - `docs/reference/packages/creating-packages.md`
      - `.agents/skills/project-patterns/references/package-ui-layout.md`
      - Decision log `2026-06-09-1431-clay-owned-shell-layout-and-package-ui-contribution-model.md`
    - Options Considered:
      - Skip authoring updates because Phase 20 is mostly core UX: rejected by Clay plan requirements whenever UI/layout/status surfaces change.
      - Update the package guide with multi-doc/status non-goals and contribution points. Chosen.
    - Chosen Approach:
      - Add a Phase 20 section describing document-session ownership, status chrome, and what packages may observe through existing document/status APIs.
    - API Notes and Examples:
      ```ts
      // Packages still declare inert panels/commands only.
      // Document dirty/save chrome is Clay-owned; packages must not create native save dialogs.
      ```
    - Files to Create/Edit:
      - `docs/reference/packages/creating-packages.md`
      - Possibly package UI wiki cross-links
    - References:
      - `.agents/skills/create-plan/references/clay.md` Package UI/Layout task requirements
  - Test Cases to Write:
    - Package docs/link tests if present (`tests/package_loading_docs.rs` or equivalent) still pass.
    - Manual review that the new section is linked/discoverable.

- [ ] Create or verify Clay JS APIs for public programmatic surfaces
  - Acceptance Criteria:
    - Functional: Every new/changed public programmatic capability from this plan (cut/paste command IDs, undo/redo command IDs, document activate/list helpers if public, save/conflict-related configuration/commands, file-dialog platform notes) has a Clay JS facade, Markdown doc, master-index link, generated registry entry, and lookup tags; server-side Rust functions that should not be public are `pub(crate)`/private.
    - Performance: API wrappers add no hot-path work; command-ID helpers remain synchronous and side-effect free where that is the existing pattern.
    - Code Quality: Naming follows `clay-js-api-naming.md` (`client*` vs `server*`); raw `Deno.core.ops` are not user-facing.
    - Security: Docs state permissions/authority boundaries for clipboard, documents, and dialogs.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/create-plan/references/clay.md` Clay JS API task
      - `.agents/skills/project-patterns/references/{clay-js-api-boundary,clay-js-api-naming,clay-js-api-schema,documentation-as-code,doc-registry-tests}.md`
    - Options Considered:
      - Expose clipboard text read as a JS API in Phase 20: deferred with broader package/config/AI clipboard authority; not required for daily editing commands.
      - Expose bindable command IDs and server document lifecycle APIs only. Chosen.
    - Chosen Approach:
      - Inventory public surfaces after implementation; add missing facades/docs; run `cargo run --bin update-doc-registry` (or project equivalent); ensure coverage tests fail on omissions.
    - API Notes and Examples:
      ```ts
      import {
        clientCopySelection,
        clientCutSelection,
        clientPasteClipboard,
        clientUndo,
        clientRedo,
      } from "clay:editor";
      ```
    - Files to Create/Edit:
      - `runtime/js/editor.ts`, `runtime/js/documents.ts` as needed
      - `docs/reference/clay-js-api/**`, `docs/index.md`, `docs/generated/**`
      - Visibility mapping tests
    - References:
      - `decision-logs/2026-05-08-1509-clay-js-api-facade-for-rust-functions.md`
      - `decision-logs/2026-05-08-1840-clay-js-api-discovery-keybindings-custom-properties.md`
  - Test Cases to Write:
    - `cargo test --test rust_visibility_api_mapping`
    - Doc registry freshness / Clay JS API coverage tests used by the repo
    - `cargo run --bin update-doc-registry` when docs change

- [ ] Create or verify Clay configuration APIs
  - Acceptance Criteria:
    - Functional: Any behavior-changing settings introduced by Phase 20 (for example undo stack depth ceilings, default save bindings, recovery prompt toggles if configurable) are documented Clay JS APIs usable from `~/.config/clay/init.js`, not free-floating keys; key bindings for cut/paste/undo/redo/save/open are discoverable.
    - Performance: Configuration evaluation remains startup/reload-time work.
    - Code Quality: Follow configuration-through-init-js pattern; empty custom_properties when no settings exist.
    - Security: Configuration in Phase 20 does not invent clipboard-exfiltration, filesystem, network, shell, or package-manager authority APIs; broader package/config/AI authority remains deferred.
  - Approach:
    - Documentation Reviewed:
      - `docs/reference/clay-js-api/configuration.md`
      - `decision-logs/2026-05-08-1841-configuration-through-init-js-and-clay-js-apis.md`
      - `.agents/skills/project-patterns/references/configuration-system.md`
    - Options Considered:
      - Hidden undocumented undo depth constants only: rejected if user-visible behavior depends on them.
      - Prefer fixed bounded defaults with documented constants; add configuration APIs only for settings that genuinely need user control. Chosen.
    - Chosen Approach:
      - Review shipped behavior; document bindKey examples for daily-editing commands; promote a setting to a Clay JS API only when needed; update registry/tests.
    - API Notes and Examples:
      ```js
      // ~/.config/clay/init.js
      import { bindKey } from "clay:keybindings";
      import { clientOpenFileDialog, /* ... */ } from "clay:documents";
      bindKey("Ctrl+O", clientOpenFileDialog(), { scope: "editor" });
      bindKey("Ctrl+S", "clay.documents.serverSaveDocument", { scope: "editor" });
      ```
    - Files to Create/Edit:
      - Configuration docs / API pages as required
      - `docs/index.md`, generated registry
      - Smoke/init.js examples in development docs
    - References:
      - `.agents/skills/create-plan/references/clay.md` Configuration task
  - Test Cases to Write:
    - Configuration/API documentation coverage tests.
    - Guard tests rejecting undocumented behavior-changing keys if new settings are added.

- [ ] Verify end-to-end daily-editing behavior on Linux and record platform matrices
  - Acceptance Criteria:
    - Functional: Automated tests cover clipboard cut/paste, undo/redo, IME unit paths, multi-document retain/switch, dirty/save/conflict, and recovery status; manual smoke covers Linux file-open dialog, IME composition with a real input method where available, save conflict, and multi-document switching; Windows/macOS matrices are documented for dialogs/shortcuts even if agent-run validation is Linux-primary.
    - Performance: Hot-path tests still assert no clipboard/save/JS work in paint; existing Phase 14/15 budgets remain non-regressed at advisory level.
    - Code Quality: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, and `cargo test --all-targets` pass on Linux.
    - Security: Capability-token and workspace authorization tests remain green; no new authority surfaces unmarked in docs.
  - Approach:
    - Documentation Reviewed:
      - `docs/development/launch-and-gui-smoke.md`
      - New file workflow doc from earlier task
      - `.agents/skills/project-patterns/references/maintenance-validation.md`
    - Options Considered:
      - Rely only on unit tests: rejected; roadmap requires interactive smoke for daily editing.
      - Automated + documented manual Linux smoke with Windows/macOS checklists. Chosen.
    - Chosen Approach:
      - Run full Linux gates; extend smoke docs with Phase 20 checklist; note platform gaps honestly.
    - API Notes and Examples:
      ```bash
      cargo fmt --check
      cargo clippy --all-targets -- -D warnings
      cargo test --all-targets
      cargo run -- smoke-gui
      ```
    - Files to Create/Edit:
      - `docs/development/launch-and-gui-smoke.md`
      - Plan checkboxes / evidence notes as tasks complete
    - References:
      - Roadmap manual GUI validation note
  - Test Cases to Write:
    - Full Linux automated gate commands above.
    - Manual checklist execution notes captured in smoke docs or plan evidence.

- [ ] Update or verify the code wiki after implementation
  - Acceptance Criteria:
    - Functional: The project code wiki is updated after all implementation tasks are complete, or explicitly verified as unchanged for non-code work.
    - Performance: Wiki updates add no runtime work and document performance-relevant implementation details changed by the plan.
    - Code Quality: Wiki pages explain what changed code does, how it works, invariants/tradeoffs, source/test paths, examples where useful, and links from the master wiki index.
    - Security: Wiki pages document touched security boundaries, permissions, validation, secrets handling, or external authority without exposing secrets.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/project-wiki/SKILL.md`: Use the project wiki workflow and quality bar.
      - `.agents/skills/create-plan/references/wiki-task.md`
    - Options Considered:
      - Update after each task: more granular, but noisy and likely to churn.
      - Update once after tests pass: keeps docs aligned with final code.
    - Chosen Approach:
      - After implementation and verification pass, update the Markdown code wiki once using `project-wiki`, including the master index and relevant pages for clipboard/IME/undo, multi-document sessions, file dialogs, save/conflict UX, recovery chrome, and pixel-snapshot decision outcome.
    - API Notes and Examples:
      ```text
      docs/wiki/index.md
      docs/wiki/modules/<module>.md
      ```
    - Files to Create/Edit:
      - `docs/wiki/index.md`: Add or update navigation links for changed implementation areas.
      - `docs/wiki/**`: Add or update implementation wiki pages for changed code (expected: masonry-editor, client-clipboard or masonry-editor clipboard section, client-file-dialog, server-file-workspace, multi-document session page, ui-observability, daily-editing primitive review completion notes).
    - References:
      - `.agents/skills/project-wiki/SKILL.md`
  - Test Cases to Write:
    - Manual wiki review: Confirm the master index links relevant pages and updated pages explain what changed implementation does and how it works.
    - Wiki/index link tests if the repository has them.

## Compromises Made

- Theme system delivery is intentionally out of scope for new architecture work because roadmap Phase 18.15 already owns it; Phase 20 only verifies and applies accessibility/theme polish.
- Multi-client scaling, lease transfer/steal UX, remote/container transport, and hard CI latency threshold promotion remain Phase 21+ work even if multi-document local sessions land here.
- Autosave, generic file watchers, and full save-as flows are included only if they fit the selected-file persistence UX without outgrowing this phase; otherwise they are documented as follow-ups in the dedicated workflow doc and Further Actions.
- Pixel snapshots may remain deferred after the required Masonry harness revisit if Clay's custom editor path cannot produce deterministic CI goldens; structural observability stays mandatory in that case.
- Package/configuration/AI authority over clipboard, filesystem, shell, network, and raw ops is intentionally not finalized in Phase 20; a later decision must establish it. Phase 20 ships Clay-owned user commands without inventing those surfaces.

## Further Actions

- Establish package/configuration/AI authority for clipboard, filesystem, shell, network, and raw ops in a dedicated later decision before exposing those surfaces.
- After remaining Phase 20 tasks complete, record additional improvements, rationale, and priority here.
