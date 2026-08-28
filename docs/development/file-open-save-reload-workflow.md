# File Open, Save, and Reload Workflow

Clay's file-open, save, and reload operations are server-first and platform-native. The client never reads or writes files directly; every path is canonicalized and authorized by the server. This document covers the complete daily-editing workflow: opening files (selected-file dialog, workspace fuzzy, workspace file browser), saving, reloading from disk, dirty-state tracking, and conflict recovery.

## Quick Reference

| Operation | Command ID | Default Chord | Scope | Authority |
|---|---|---|---|---|
| Open file dialog | `documents.clientOpenFileDialog` | none (bind `Ctrl+O`) | editor | client UI → server selected-file grant |
| Open workspace file | `documents.serverOpenDocument` | none | editor | server workspace-root read |
| Fuzzy file open | `workspace.openFuzzyFile` | none (bind `Ctrl+P`) | editor | server workspace listing |
| Toggle file browser | `workspace.toggleFileBrowser` | canonical example binds `Ctrl+B` | editor | per-tab server workspace-pane visibility |
| Save document | `documents.serverSaveDocument` | none (bind `Ctrl+S`) | editor | server workspace/selected-file write |
| Reload document | `documents.serverReloadDocument` | none | editor | server workspace/selected-file read |
| Show open documents | `editor.clientShowOpenDocuments` | none | editor | client-local session list |
| Copy selection | `editor.clientCopySelection` | none (bind `Ctrl+Shift+C`) | editor | client clipboard write |

No default Rust-level shortcuts are hardcoded. Every key binding above is configured through `init.js` or a smoke fixture.

## Opening Files

### Selected-file dialog (`clientOpenFileDialog`)

Opens the native OS file picker. After the user selects a file, the client sends the path to the server with a single-use capability token. The server canonicalizes and validates the path, creates a single-file grant (not a workspace root), streams the UTF-8 file into its canonical rope under the resident-memory budget, and sends back a bounded `DocumentOpened` head; remaining bytes use versioned chunk requests.

```js
// ~/.config/clay/init.js
import { clientOpenFileDialog } from "clay:documents";
import { bindKey } from "clay:keybindings";
import { loadPackage } from "clay:packages";

await loadPackage("@clay/markdown");
bindKey("Ctrl+O", clientOpenFileDialog(), { scope: "editor" });
bindKey("Ctrl+S", "documents.serverSaveDocument", { scope: "editor" });
```

Selected-file grants are single-file: the server authorizes only the canonical path the user picked. Sibling files, parent directories, and project roots are not implicitly authorized. Selected-file documents support save and reload through the same server workspace path as workspace-root documents.

**Platform support (client dialogs):**

| Platform | Backend | Filters | Cancellation |
|---|---|---|---|
| Windows | COM `IFileOpenDialog` | `.md`, `.markdown`, `.mdown`, `*.*` | non-error no-op |
| Linux | xdg-desktop-portal `FileChooser.OpenFile` | glob: `*.md`, `*.markdown`, `*.mdown`, `*` | non-error no-op |
| macOS | objc2-app-kit `NSOpenPanel` | Markdown extensions, `allowsOtherFileTypes` | non-error no-op |
| Other | N/A | N/A | returns `Unsupported` diagnostic |

On unsupported platforms, `clientOpenFileDialog` returns a status diagnostic: `client.file_dialog.not_supported_on_this_platform`. No panic, no crash, no blank dialog.

### Workspace file browser and fuzzy open

Workspace-root files are opened through the server workspace model (`docs/wiki/modules/server-file-workspace.md`). The server maintains a registry of workspace roots (configured at startup, discovered from opened-file ancestry, or added via `clientOpenFolderDialog`). Each tab starts with its workspace pane hidden; `workspace.toggleFileBrowser` publishes the bounded tree for that tab or an editor-only snapshot that releases the left slot. File browser and fuzzy-file commands list directory entries and open selected paths through `WorkspaceState::open_existing_file`; `Ctrl+O` remains usable while the pane is hidden.

```js
import { clientOpenFolderDialog } from "clay:workspace";

bindKey("Ctrl+B", "workspace.toggleFileBrowser", { scope: "editor" });
bindKey("Ctrl+P", "workspace.openFuzzyFile", { scope: "editor" });
bindKey("Ctrl+Shift+O", clientOpenFolderDialog(), { scope: "editor" });
```

Workspace opens differ from selected-file opens: the file must be inside a registered workspace root; the server does not create a single-file grant; and `DocumentOpened` snapshots carry workspace-root metadata instead of single-file grant metadata. Save and reload work identically for both kinds of documents.

### Built-in path browser (`controlCenter.openPath`)

Phase 24.3 adds a built-in dired-style path browser over the transient-menu round trip: `Ctrl+X Ctrl+F` (Phase 24.5 sequence default, fully rebindable via `bindKey`/`unbindKey`) opens a session whose editable path bar is seeded from the active document's canonical parent, then the bound tab's workspace root, then the server cwd. Primary activation descends into directories and opens files; Alt+Enter on a directory opens it as the tab's workspace; Backspace on an empty filter ascends; typing a path (trailing separator for directories) jumps anywhere. Listings are bounded depth-1 snapshots, and the browse grant is ephemeral: navigation creates no grant, a file open converts browse authority into exactly one `SingleFile` grant through the same selected-file open path, and an Alt+Enter workspace open converts it into one `Directory` root grant for the bound tab only. The native dialogs remain the fallback capability issuers and are unchanged.

### Document state after open

Both selected-file and workspace opens produce a `DocumentOpened` client event.
The workspace controller owns one `DocumentSession` per pane and routes the
reply by its in-flight path before falling back to document identity. There is
no app-wide document-session singleton.

The owning session:

1. Installs the bounded `DocumentTextHead` and paints its first chunk immediately.
2. Keeps one current CodeMirror `Text`: `view.state.doc` while attached, or a
   detached snapshot only while no view exists.
3. Installs server behavior/theme/typography metadata and updates edit
   authority without duplicating document text.
4. Requests remaining bytes one offset at a time through `DocumentChunkRequest`;
   each response is deduplicated, appended programmatically with history
   disabled, and bounded by `MAX_CHUNK_BYTES`.
5. Marks the session ready only after all bytes arrive. A rejected/stale chunk
   stops the load or restarts it through resync; it never exposes a partial
   editable document.

Opening another file in a pane replaces that pane's current document. Other
panes and tabs retain their own sessions; opening the same document elsewhere
is routed by the server's document/lease authority rather than a client mirror.

## Saving Documents

### Server-first save (`serverSaveDocument`)

Save is a server file IO operation, never client-local. The client sends `ClientMessage::SaveDocument` with the document ID and known version; the server clones the canonical Crop rope (Arc-root), releases the document mutex, and streams rope chunks to the authorized file path atomically (exclusive unpredictable temp file + `fsync` + permission restore + target-identity revalidation + rename). The write never materializes a whole-document `String`.

```js
bindKey("Ctrl+S", "documents.serverSaveDocument", { scope: "editor" });
```

When `Ctrl+S` fires:

1. The keybinding matches `documents.serverSaveDocument` in the behavior manifest.
2. The editor host intercepts the command locally (before the generic server `CommandExecutor` route) and calls the save flow.
3. The edit queue sends `ClientMessage::SaveDocument` for the active document's ID and current confirmed version.
4. The server `save_document_unlocked` reauthorizes the canonical path, compares disk metadata for staleness, writes atomically, and returns `ServerMessage::DocumentSaved { document_id, version, dirty }`.
5. The client receives `ClientConnectionEvent::DocumentSaved`: dirty chrome clears, stale conflict diagnostics clear, and the version updates.

Save is asynchronous and non-blocking. The GUI remains responsive; typing continues optimistically. The status line updates when the `DocumentSaved` event arrives.

**Platform support (server-side):**

| Platform | Atomic save | Permissions | Notes |
|---|---|---|---|
| Linux | POSIX atomic rename | rejects targets without write bits | temp in target directory starts `0o600`, original mode restored (fail closed on restore error) |
| macOS | POSIX atomic rename | rejects targets without write bits | same as Linux |
| Windows | `MoveFileExW(MOVEFILE_REPLACE_EXISTING)` | standard file permissions | Rust `std::fs::rename` path |

The temp file is created with `OpenOptions::create_new` under an unpredictable name (process-random `RandomState` seed + counter) with up to 8 bounded collision retries, so a pre-created file or symlink at a guessed temp path is never truncated or followed. Immediately before the rename, the server revalidates the target's stable identity (Unix `(dev, ino)`, Windows volume serial + file index) plus length and modification time against what `prepare_save` observed: an external edit — including a same-length edit — or an atomic replacement during the temp write fails the save with `StaleFileMetadata` and preserves the external bytes instead of clobbering them.

### Dirty state

The document is marked dirty when an accepted local edit increments the server-confirmed version. Dirty is visible in:

- **Status chrome:** `Connected — Editable — note.md — doc 42 — v5 — Dirty`
- **Accessibility label:** `Editor document 42: note.md. Dirty. Text: …`
- **Status observation:** `SduiStatusObservation.dirty` field

Dirty clears on:
- Successful save (`DocumentSaved` with `dirty: false`)
- Clean reload from disk (`DocumentReloaded`)
- Full resync snapshot (`ResyncSnapshot`)
- Same-document replace open (`DocumentOpened` for already-open doc)

Dirty persists on:
- Failed save (stale metadata, missing file)
- Connection loss / disconnect
- Switching to another open document (background document retains dirty)

## Reloading Documents

### Server-first reload (`serverReloadDocument`)

Reload replaces the editor text with the current on-disk version. Clean documents reload without friction; dirty documents require explicit force.

Open and reload read through one opened handle while the workspace mutex is released. The server reserves the 256 MiB session resident-rope budget, sniffs NUL bytes in the first 8 KiB, and streams UTF-8 through a bounded `RopeBuilder` with a three-byte carry, so a file that grows between validation and EOF is rejected with `DocumentBudgetExceeded` without a document-sized transient `String`. The workspace mutex is never held across the disk read; successful open/reload responses carry a bounded head and the client fetches remaining chunks.

```js
// Not typically bound to a direct key; reachable via Control Center or recovery menus.
import { serverReloadDocument } from "clay:documents";

await serverReloadDocument({ documentId, force: false });
```

The `force` flag determines dirty-document behavior:

| Flag | Dirty? | Behavior |
|---|---|---|
| `force: false` | clean | replaces text, stays clean |
| `force: false` | dirty | returns `FileOperationFailed` → `DirtyDocument` error code |
| `force: true` | clean or dirty | replaces text, clears dirty |

When `DocumentReloaded` arrives:
1. Editor text is replaced with the reloaded snapshot via `load_resync_snapshot`.
2. Dirty state clears (from server metadata).
3. Edit queue authority updates to the new version.
4. Open pending edits for that document are discarded.

## Conflict Recovery

### Stale save conflict

When `save_document` detects that the on-disk file metadata differs from the last-known metadata (file modified externally since the last open/reload/save), the server returns `FileOperationFailed` with error code `StaleFileMetadata`. The document stays dirty in memory; the client opens a `TransientMenuSession` recovery menu:

**Recovery menu (stale save):**
- **Reload latest from disk** → `serverReloadDocument(documentId, force: true)` — discards local edits, replaces with disk version.
- **Keep my unsaved edits** → dismisses the menu, keeps dirty text, no file IO. User can save again after the external change is resolved.
- **Compare later** → dismisses the menu, keeps dirty text. User can inspect the external change manually.

Force-save (overwriting the external change without inspection) is intentionally not offered.

### Dirty reload conflict

When `reload_document` is called without `force` on a dirty document, the server returns `FileOperationFailed` with error code `DirtyDocument`. The client opens a recovery menu:

**Recovery menu (dirty reload):**
- **Save first, then reload** → `serverSaveDocument` then `serverReloadDocument(force: true)`. If the save succeeds, reload proceeds; if the save itself hits a stale conflict, that conflict must be resolved first.
- **Discard and reload** → `serverReloadDocument(documentId, force: true)` — discards local edits, replaces with disk version.
- **Keep my unsaved edits** → dismisses the menu, keeps dirty text.

### Accessibility during conflict

Recovery menus are server-owned menu snapshots rendered by the React client as accessible listbox/menu surfaces; item labels include the action description and selected state.

## Multi-Document and Multi-Pane Sessions

The server caps open documents per client at 64, but frontend ownership is
pane-scoped rather than a global document mirror. Each pane has one session and
one current `Text`; detached text exists only while that pane has no attached
view. A four-pane layout therefore has four independent session/document
routes, and tabs have separate runtimes/connections.

**Switch or restore a document:**

- The shell persists pane paths/layout, then `workspace-controller.ts` creates
  or reuses the owning pane session during restore.
- `DocumentOpened` replies match the session's in-flight path before document
  ID/active-pane fallback, so simultaneous pane opens cannot cross-wire.
- A newly attached view installs the pane's detached snapshot; it does not
  replay a second document copy or a cached feature stream into another pane.
- Switching panes/tabs is client-local presentation work; save, reload,
  resync, leases, versions, and file authority remain server-first.

Dirty state, pending edits, confirmed version, progressive loading, and feature
layers are per pane/document session. Programmatic head/chunk/reload/resync
installs use no-history transactions, so undo cannot restore partial transfer
chunks.

```js
bindKey("Ctrl+Tab", "editor.clientShowOpenDocuments", { scope: "editor" });
```

## Platform Capabilities Matrix

| Capability | Windows | Linux | macOS | Other |
|---|---|---|---|---|
| Native file-open dialog | COM `IFileOpenDialog` | xdg-desktop-portal | `NSOpenPanel` | Unsupported diagnostic |
| Native folder dialog | COM `IFileOpenDialog` | xdg-desktop-portal | `NSOpenPanel` | Unsupported diagnostic |
| Atomic save | `MoveFileExW` rename | POSIX atomic rename | POSIX atomic rename | N/A |
| Markdown filters | extension filter list | portal glob filters | `setAllowedFileTypes` (deprecated) | N/A |
| All-files fallback | `*.*` | `*` (normalized) | `allowsOtherFileTypes: true` | N/A |
| Clipboard copy/cut/paste | `Ctrl+C`/`X`/`V` | `Ctrl+C`/`X`/`V` | `Cmd+C`/`X`/`V` | persistent text-only client `arboard` sink |
| Undo / redo | `Ctrl+Z` / `Ctrl+Shift+Z` or `Ctrl+Y` | same | `Cmd+Z` / `Cmd+Shift+Z` | 256-entry inverse stack |
| IME preedit overlay | WebKitGTK IME → CodeMirror composition | ibus/fcitx when available | same path per platform | composition is local until commit |
| Snapshot retain (64 docs) | yes | yes | yes | yes |
| Undo/redo (256 entries) | yes | yes | yes | yes |
| Save/conflict recovery menus | yes | yes | yes | yes |
| Pending-edit / disconnect recovery | yes | yes | yes | yes |

Full Linux-primary verification evidence, shortcut matrix, and Windows/macOS checklist notes live in [Launch and GUI Smoke Validation](launch-and-gui-smoke.md#phase-20-daily-editing-platform-matrix-and-linux-verification).

## Capability Tokens

Selected-file and selected-folder dialogs use server-issued single-use capability tokens:

1. After `Hello` handshake, the server issues one `FileOpenCapabilityIssued { token }`.
2. The client stores the token and attaches it to each picker request (`OpenSelectedFile` or `AddSelectedWorkspaceRoot`).
3. The server validates the token against its `FileOpenCapabilityPool`; stale, empty, or consumed tokens are rejected with a sanitized diagnostic.
4. After a successful or failed attempt, the server re-issues one pending token.

Requests without a valid token create no file grant, workspace root, or document. This is a structural authority gate; a same-user client that completes `Hello` could also receive tokens, so full defense requires the long-term OS-verifiable picker exchange.

## Manual Smoke Steps

### Selected-file open → edit → save → conflict

1. Start with `cargo run -- smoke-gui --config-fixture file-browser-workflow`.
2. Press `Ctrl+O`, pick a `.md` file. Confirm the file replaces the editor buffer.
3. Type a small edit. Confirm the status line shows `— Dirty`.
4. Press `Ctrl+S`. Confirm the status line loses `— Dirty` after save completes.
5. Outside Clay, modify the same file on disk (e.g., `echo "external change" >> file.md`).
6. Type another edit in Clay. Press `Ctrl+S`. Confirm a recovery menu appears: "Reload latest from disk", "Keep my unsaved edits", "Compare later".
7. Choose "Keep my unsaved edits". Confirm the menu dismisses, dirty remains.
8. Choose "Reload latest from disk". Confirm the external change appears, dirty clears.

### Workspace open → save → reload

1. Start with a workspace-root fixture.
2. `Ctrl+P` fuzzy-open a file. Confirm it loads in the editor.
3. `Ctrl+S` save. Confirm clean.
4. Outside Clay, modify the file on disk.
5. Trigger reload (Control Center or bound key). Confirm the disk text replaces the editor.
6. Type an edit (dirty), then attempt reload without force. Confirm the dirty-reload conflict menu appears.

### Multi-document switch and per-document dirty

1. Open a `.md` file via `Ctrl+O`. Type an edit. Confirm dirty.
2. Open a second `.md` file via `Ctrl+O`. Confirm the first file's session is retained.
3. `clientShowOpenDocuments` → select the first file. Confirm dirty is still shown.
4. Switch back to the second file. Confirm its state is preserved.

### Platform-specific dialog validation

On Linux, invoke the same file-dialog command repeatedly while the portal picker is open: only one file picker may exist. Cancel, invoke again, and confirm it reopens. Repeat independently for the folder picker; a stale completion must never act on a later generation.

- **Windows:** Confirm the native open dialog shows a Markdown filter dropdown with all-files fallback.
- **Linux:** Confirm the portal-native dialog opens (requires a portal-compliant desktop environment with `xdg-desktop-portal` running).
- **macOS:** Confirm `NSOpenPanel` opens with Markdown extensions and allows other file types.
- **Other:** Confirm the status diagnostic `not supported on this platform` appears instead of a crash.

## Authority Boundaries

All file operations stay within server-validated boundaries:

| Operation | Client authority | Server authority |
|---|---|---|
| Open file dialog | Renders native dialog; returns user-picked path | Canonicalizes path, creates single-file grant |
| Open file (path browser) | Sends primary `MenuActivate` on the installed entry | Activation converts ephemeral browse authority into one single-file grant; path comes from server-held installed entries only |
| Open workspace file | Sends relative path | Validates root containment, loads text |
| Save | Sends document ID + version | Reauthorizes path, writes atomically |
| Reload | Sends document ID + force flag | Reauthorizes path, re-reads, validates |
| Folder dialog | Renders native dialog; returns user-picked path | Canonicalizes directory, adds workspace root |
| Open workspace (path browser) | Sends secondary `MenuActivate` on the installed entry | Activation converts ephemeral browse authority into one `Directory` root grant for the bound tab; other tabs untouched |

No operation grants:
- Client-side filesystem read/write beyond the dialog picker
- Package-level filesystem scanning
- Shell, network, or AI mutation authority
- Broad workspace expansion from a single-file grant

Save-as, file watchers, and autosave remain deferred. When implemented, they will follow the same server-first, token-gated pattern.

## Related

- [Server File Workspace Model](../wiki/modules/server-file-workspace.md) — server workspace roots, grants, open/save/reload internals
- [Client File Dialog Backend](../wiki/modules/client-file-dialog.md) — platform dialog implementations
- [Launch and GUI Smoke Validation](launch-and-gui-smoke.md) — command-first smoke paths and fixtures
- [Client/Server Edit Acknowledgement Flow](../wiki/flows/client-server-edit-ack.md) — edit queue, acks, resync
- [Clay JS API: serverSaveDocument](../reference/clay-js-api/documents/server-save-document.md)
- [Clay JS API: serverReloadDocument](../reference/clay-js-api/documents/server-reload-document.md)
- [Clay JS API: clientOpenFileDialog](../reference/clay-js-api/documents/client-open-file-dialog.md)
