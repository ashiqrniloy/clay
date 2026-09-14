import {
  useEffect,
  useMemo,
  useRef,
  useState,
  useSyncExternalStore,
} from "react";

import {
  ClayBadge,
  ClayButton,
  ClayIcon,
  ClayIconButton,
  ClayText,
} from "../components";
import { accessIsEditable } from "../state/document-store";
import { tabLabel } from "../shell/tab-store";
import { createEditor, setReadOnly } from "./create-editor";
import { EditorProjection } from "./extensions/controller";
import { editorPerformance, PERFORMANCE_STAGE } from "./performance";
import type { BehaviorManifestDto } from "./extensions/types";
import type { DocumentSession } from "./sync/session";

import styles from "./editor.module.css";

export interface ClayEditorProps {
  session: DocumentSession;
  /** Host intercepts open so the workspace can focus a duplicate pane. */
  onOpenPath?: (path: string) => void;
}

/**
 * One EditorView per mount. React owns chrome only; text stays in CodeMirror.
 */
export function ClayEditor({ session, onOpenPath }: ClayEditorProps) {
  const meta = useSyncExternalStore(
    (listener) => session.store.subscribe(listener),
    () => session.store.get(),
  );
  const parentRef = useRef<HTMLDivElement | null>(null);
  const viewRef = useRef<ReturnType<typeof createEditor> | null>(null);
  const openHandler = useRef(onOpenPath);
  openHandler.current = onOpenPath;
  const projection = useMemo(
    () =>
      new EditorProjection({
        send: (payload) => session.request(payload),
        meta: () => session.store.get(),
        clientId: () => session.clientId(),
        openPath: (path) =>
          openHandler.current ? openHandler.current(path) : session.open(path),
        report: (message) => session.store.update({ diagnostic: message }),
      }),
    [session],
  );
  const [openPath, setOpenPath] = useState("");
  const [stripOpen, setStripOpen] = useState(false);
  const stripInput = useRef<HTMLInputElement | null>(null);
  const editable = !!meta && accessIsEditable(meta.access) && !meta.loading;
  const lastReadOnly = useRef<boolean | null>(null);

  // The relative-path strip is on demand (DESIGN.md §12): focusing it on open
  // keeps the keyboard path short — reveal, type, Enter, closed.
  useEffect(() => {
    if (stripOpen) stripInput.current?.focus();
  }, [stripOpen]);

  useEffect(() => {
    editorPerformance.count(PERFORMANCE_STAGE.reactCommit, 0, {
      documentId: meta?.documentId,
      version: meta?.version,
      feature: "clayEditor",
    });
  });

  useEffect(() => {
    const parent = parentRef.current;
    if (!parent || !meta) return;
    const view = createEditor({
      doc: session.snapshotDoc(),
      readOnly: !accessIsEditable(meta.access) || !!meta.loading,
      parent,
      placeholder: "Start typing",
      documentId: meta.documentId,
      version: meta.version,
      onUserChanges: (oldDoc, changes, traceId, index) => {
        session.emitUserChanges(oldDoc, changes, traceId, index);
      },
      onSave: () => session.save(),
      extra: projection.extensions,
    });
    viewRef.current = view;
    lastReadOnly.current = !accessIsEditable(meta.access) || !!meta.loading;
    session.attachView(view);
    projection.installInitial(
      session.behaviorManifest() as BehaviorManifestDto,
    );
    projection.attach(view);
    session.setClientCommandHandler((commandId) =>
      projection.runClientCommand(commandId),
    );
    for (const envelope of session.featureSnapshot())
      projection.handleEnvelope(envelope);
    const unsubscribe = session.subscribeFeatures((envelope) =>
      projection.handleEnvelope(envelope),
    );
    return () => {
      unsubscribe();
      projection.detach(view);
      session.setClientCommandHandler(null);
      session.detachView(view);
      view.destroy();
      viewRef.current = null;
    };
    // Recreate only when the server document identity changes.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [session, projection, meta?.documentId]);

  // Reconfigure the read-only compartment only when the derived state
  // actually flips; per-keystroke metadata updates are inert here.
  useEffect(() => {
    const view = viewRef.current;
    if (!view) return;
    const readOnly = !editable;
    if (lastReadOnly.current === readOnly) return;
    lastReadOnly.current = readOnly;
    // Progressive chunk loads gate editing until the document is complete.
    setReadOnly(view, readOnly);
  }, [editable]);

  if (!meta) {
    return (
      <div className={styles.empty} data-testid="editor-empty">
        <ClayText variant="title">No document</ClayText>
        <ClayText variant="body" muted>
          Open a file from the workspace to start editing.
        </ClayText>
      </div>
    );
  }

  const path = meta.path.trim();
  const label = path
    ? /^(?:[\\/]|[A-Za-z]:[\\/])/.test(path)
      ? tabLabel(path)
      : path
    : tabLabel(meta.workspaceRoot);
  const documentName = label.split(/[\\/]/).pop() || label;
  const documentPath = path && documentName !== path ? path : "";
  const documentState = !editable
    ? "read-only"
    : meta.dirty
      ? "dirty"
      : "clean";
  const canOpenPath = meta.workspaceRootId != null && openPath.length > 0;
  const submitOpenPath = () => {
    if (!canOpenPath) return;
    setStripOpen(false);
    if (onOpenPath) onOpenPath(openPath);
    else session.open(openPath);
  };

  return (
    <div className={styles.host} data-testid="clay-editor">
      <div className={styles.chrome} data-clay-ds="editor.chrome">
        <div className={styles.crumb}>
          <span className={styles.docName} data-testid="editor-doc-name">
            {documentName}
          </span>
          {documentPath ? (
            <span className={styles.docPath}>{documentPath}</span>
          ) : null}
        </div>
        <div className={styles.badges} aria-label="Document state">
          <ClayBadge>{`v${meta.version}`}</ClayBadge>
          <ClayBadge tone={meta.dirty ? "warning" : "success"}>
            {documentState}
          </ClayBadge>
          {meta.pending > 0 ? (
            <ClayBadge tone="muted">{`pending ${meta.pending}`}</ClayBadge>
          ) : null}
        </div>
        <div className={styles.actions}>
          <ClayButton
            variant="muted"
            aria-expanded={stripOpen}
            onPress={() => setStripOpen((open) => !open)}
          >
            Open
          </ClayButton>
          <ClayButton
            variant="muted"
            onPress={() => session.runClientCommand("editor.clientUndo")}
          >
            Undo
          </ClayButton>
          <ClayButton
            variant="muted"
            onPress={() => session.runClientCommand("editor.clientRedo")}
          >
            Redo
          </ClayButton>
          <ClayIconButton
            icon="document.reload"
            label="Reload"
            variant="muted"
            onPress={() => session.reload(false)}
          />
          <ClayButton
            variant="primary"
            isDisabled={!editable}
            onPress={() => session.save()}
          >
            <ClayIcon name="document.save" /> Save
          </ClayButton>
          <ClayIconButton
            icon="action.close"
            label="Close"
            variant="danger"
            onPress={() => session.close(meta.dirty)}
          />
        </div>
      </div>
      {stripOpen ? (
        <div className={styles.openStrip} data-testid="editor-open-strip">
          <span className={styles.openLabel}>Open path</span>
          <input
            ref={stripInput}
            className={styles.path}
            aria-label="Open relative path"
            value={openPath}
            onChange={(event) => setOpenPath(event.target.value)}
            onKeyDown={(event) => {
              if (event.key === "Enter") submitOpenPath();
              if (event.key === "Escape") setStripOpen(false);
            }}
            placeholder="relative/path"
          />
          <ClayButton isDisabled={!canOpenPath} onPress={submitOpenPath}>
            Open
          </ClayButton>
          <span className={styles.openNote} role="status">
            {`relative to ${meta.workspaceRoot || "the workspace"}`}
          </span>
          <span className={styles.spacer} />
          <ClayIconButton
            icon="action.close"
            label="Hide open path"
            variant="muted"
            onPress={() => setStripOpen(false)}
          />
        </div>
      ) : null}
      {meta.loading && (
        <div className={styles.alert} role="status">
          <ClayText variant="status">Loading full document…</ClayText>
        </div>
      )}
      {meta.diagnostic && (
        <div className={styles.alert} role="alert">
          <ClayText variant="status">{meta.diagnostic}</ClayText>
        </div>
      )}
      <div className={styles.column}>
        <div
          ref={parentRef}
          className={styles.canvas}
          role="region"
          aria-label={`Editor ${label}`}
        />
      </div>
    </div>
  );
}
