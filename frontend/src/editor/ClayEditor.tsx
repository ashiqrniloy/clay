import {
  useEffect,
  useMemo,
  useRef,
  useState,
  useSyncExternalStore,
} from "react";

import { ClayButton, ClayText } from "../components";
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
  const editable = !!meta && accessIsEditable(meta.access) && !meta.loading;
  const lastReadOnly = useRef<boolean | null>(null);

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

  return (
    <div className={styles.host} data-testid="clay-editor">
      <div className={styles.chrome}>
        <div className={styles.meta}>
          <ClayText variant="status">{label}</ClayText>
          <ClayText variant="detail" muted>
            v{meta.version}
            {meta.dirty ? " · dirty" : " · clean"}
            {editable ? " · editable" : " · read-only"}
            {meta.pending > 0 ? ` · pending ${meta.pending}` : ""}
          </ClayText>
        </div>
        <div className={styles.actions}>
          <ClayButton
            variant="primary"
            isDisabled={!editable}
            onPress={() => session.save()}
          >
            Save
          </ClayButton>
          <ClayButton onPress={() => session.reload(false)}>Reload</ClayButton>
          <ClayButton variant="muted" onPress={() => session.close(meta.dirty)}>
            Close
          </ClayButton>
          <input
            className={styles.path}
            aria-label="Open path"
            value={openPath}
            onChange={(event) => setOpenPath(event.target.value)}
            placeholder="relative/path"
          />
          <ClayButton
            isDisabled={meta.workspaceRootId == null || openPath.length === 0}
            onPress={() =>
              onOpenPath ? onOpenPath(openPath) : session.open(openPath)
            }
          >
            Open
          </ClayButton>
        </div>
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
      </div>
      <div
        ref={parentRef}
        className={styles.canvas}
        role="region"
        aria-label={`Editor ${label}`}
      />
    </div>
  );
}
