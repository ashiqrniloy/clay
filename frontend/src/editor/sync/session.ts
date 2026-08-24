import type { EditorView } from "@codemirror/view";

import type { BootstrapDto, BridgeEnvelope } from "../../bridge/types";
import {
  accessIsEditable,
  createDocumentStore,
  metaFromInitial,
  type DocumentMeta,
  type DocumentStore,
} from "../../state/document-store";
import { clayOrigin } from "../transactions";
import {
  closePayload,
  editPayload,
  getStatusPayload,
  openPayload,
  reloadPayload,
  requestResyncPayload,
  savePayload,
  shouldRequestResync,
  type EditRejection,
} from "./messages";
import { changesToOperations, type TextChange } from "./operations";

export type SendFn = (payload: string) => Promise<void>;

const FEATURE_EVENT_KINDS = new Set([
  "behaviorManifestInstalled",
  "decorationSet",
  "decorationBatch",
  "diagnosticSet",
  "foldingRangeSet",
  "completionResult",
  "completionRejected",
  "languageIntelligenceResult",
  "languageIntelligenceRejected",
  "selectionQueryResult",
  "editorCommandRequest",
  "caretStyleOverride",
  "editorLayoutOverride",
]);

export interface DocumentSession {
  store: DocumentStore;
  /** Last authoritative snapshot (initial / open / resync). Not live typing. */
  snapshotText(): string;
  installInitial(bootstrap: BootstrapDto): void;
  clientId(): number;
  behaviorManifest(): BootstrapDto["behaviorManifest"];
  request(payload: string): Promise<void>;
  featureSnapshot(): readonly BridgeEnvelope[];
  subscribeFeatures(listener: (envelope: BridgeEnvelope) => void): () => void;
  attachView(view: EditorView): void;
  detachView(view: EditorView): void;
  setClientCommandHandler(
    handler: ((commandId: string) => boolean) | null,
  ): void;
  runClientCommand(commandId: string): boolean;
  handleEnvelope(envelope: BridgeEnvelope): void;
  emitUserChanges(oldText: string, changes: readonly TextChange[]): void;
  save(): void;
  reload(force?: boolean): void;
  close(force?: boolean): void;
  open(path: string): void;
  requestResync(): void;
}

interface Options {
  send: SendFn;
  store?: DocumentStore;
}

export function createDocumentSession(options: Options): DocumentSession {
  const store = options.store ?? createDocumentStore();
  let view: EditorView | null = null;
  let clientCommandHandler: ((commandId: string) => boolean) | null = null;
  let nextTransactionId = 1;
  const inflight = new Set<number>();
  let authoritativeText = "";
  let clientId = 0;
  let behaviorManifest: BootstrapDto["behaviorManifest"] = {
    manifestId: "default.text",
    behaviorVersion: 0,
    commands: [],
    keymaps: [],
  };
  const featureEvents: BridgeEnvelope[] = [];
  const featureListeners = new Set<(envelope: BridgeEnvelope) => void>();
  /** Open requested before the handshake metadata delivered a root id. */
  let pendingOpenPath: string | null = null;
  const sendOpen = (rootId: number, path: string) => {
    void options.send(openPayload(rootId, path)).catch((error: unknown) => {
      store.update({
        diagnostic: error instanceof Error ? error.message : "request failed",
      });
    });
  };
  const openDocument = (path: string) => {
    const rootId = store.get()?.workspaceRootId;
    if (rootId == null) {
      // Restore and early opens can race the handshake metadata event; queue
      // until applyMetadata learns the workspace root id.
      pendingOpenPath = path;
      store.update({ diagnostic: null });
      return;
    }
    pendingOpenPath = null;
    sendOpen(rootId, path);
  };
  const send = (payload: string) => {
    void options.send(payload).catch((error: unknown) => {
      const message =
        typeof error === "object" && error !== null && "message" in error
          ? String((error as { message: unknown }).message)
          : "request failed";
      store.update({ diagnostic: message });
    });
  };

  const replaceText = (
    text: string,
    origin: "resync" | "correction" | "remote" | "programmatic",
  ) => {
    if (!view) return;
    const current = view.state.doc.toString();
    if (current === text) return;
    view.dispatch({
      changes: { from: 0, to: view.state.doc.length, insert: text },
      annotations: clayOrigin.of(origin),
    });
  };

  const session: DocumentSession = {
    store,
    snapshotText: () => authoritativeText,
    installInitial(bootstrap) {
      inflight.clear();
      clientId = bootstrap.clientId;
      featureEvents.length = 0;
      behaviorManifest = bootstrap.behaviorManifest;
      authoritativeText = bootstrap.initialDocument.text;
      store.set(
        metaFromInitial({
          documentId: bootstrap.initialDocument.documentId,
          version: bootstrap.initialDocument.version,
          access: bootstrap.initialDocument.access,
          workspaceRoot: bootstrap.initialDocument.workspaceRoot,
          behaviorVersion: bootstrap.behaviorManifest.behaviorVersion,
        }),
      );
      replaceText(bootstrap.initialDocument.text, "programmatic");
      send(getStatusPayload(bootstrap.initialDocument.documentId));
    },
    clientId: () => clientId,
    behaviorManifest: () => behaviorManifest,
    request: options.send,
    featureSnapshot: () => featureEvents,
    subscribeFeatures(listener) {
      featureListeners.add(listener);
      return () => featureListeners.delete(listener);
    },
    attachView(next) {
      view = next;
      if (
        authoritativeText &&
        next.state.doc.toString() !== authoritativeText
      ) {
        replaceText(authoritativeText, "programmatic");
      }
    },
    detachView(current) {
      if (view === current) view = null;
    },
    setClientCommandHandler(handler) {
      clientCommandHandler = handler;
    },
    runClientCommand(commandId) {
      return clientCommandHandler?.(commandId) ?? false;
    },
    emitUserChanges(oldText, changes) {
      const meta = store.get();
      if (!meta || !accessIsEditable(meta.access)) return;
      const operations = changesToOperations(oldText, changes);
      if (operations.length === 0) return;
      let pending = meta.pending;
      for (const operation of operations) {
        const transactionId = nextTransactionId;
        nextTransactionId += 1;
        inflight.add(transactionId);
        pending += 1;
        send(
          editPayload(
            meta.documentId,
            transactionId,
            meta.behaviorVersion,
            operation,
          ),
        );
      }
      store.update({
        pending,
        dirty: true,
        diagnostic: null,
      });
    },
    handleEnvelope(envelope) {
      if (envelope.kind !== "event") return;
      const event = envelope.data as {
        kind?: string;
        data?: Record<string, unknown>;
      };
      const kind = event.kind;
      const data = event.data ?? {};
      if (kind && FEATURE_EVENT_KINDS.has(kind)) {
        // ponytail: bounded inactive-pane replay; replace with chunk-keyed LRU
        // only if 256 validated events per pane proves insufficient.
        featureEvents.push(envelope);
        if (featureEvents.length > 256) featureEvents.shift();
        for (const listener of featureListeners) listener(envelope);
      }
      switch (kind) {
        case "editAck":
          onAck(data);
          return;
        case "editRejected":
          onRejected(data);
          return;
        case "resyncSnapshot":
          onResync(data);
          return;
        case "documentOpened":
          onOpened(data);
          return;
        case "documentReloaded":
          onOpened(data);
          return;
        case "documentSaved":
          onSaved(data);
          return;
        case "documentClosed":
          onClosed(data);
          return;
        case "documentStatus":
          onStatus(data);
          return;
        case "fileOperationFailed":
          store.update({
            diagnostic: String(data.message ?? "file operation failed"),
          });
          return;
        case "behaviorManifestInstalled":
          if (typeof data.behaviorVersion === "number") {
            store.update({ behaviorVersion: data.behaviorVersion });
          }
          if (data.manifest && typeof data.manifest === "object") {
            behaviorManifest =
              data.manifest as BootstrapDto["behaviorManifest"];
          }
          return;
        default:
          return;
      }
    },
    save() {
      const meta = store.get();
      if (!meta) return;
      send(savePayload(meta.documentId, meta.version));
    },
    reload(force = false) {
      const meta = store.get();
      if (!meta) return;
      send(reloadPayload(meta.documentId, meta.version, force));
    },
    close(force = false) {
      const meta = store.get();
      if (!meta) return;
      send(closePayload(meta.documentId, force));
    },
    open: openDocument,
    requestResync() {
      const meta = store.get();
      if (!meta) return;
      send(requestResyncPayload(meta.documentId, meta.version));
    },
  };

  function onAck(data: Record<string, unknown>) {
    const transactionId = Number(data.transactionId);
    inflight.delete(transactionId);
    const pending = Math.max(0, (store.get()?.pending ?? 1) - 1);
    store.update({
      version: Number(data.version ?? store.get()?.version ?? 0),
      pending,
      diagnostic: null,
    });
  }

  function onRejected(data: Record<string, unknown>) {
    const transactionId = Number(data.transactionId);
    inflight.delete(transactionId);
    const reason = data.reason as EditRejection;
    const pending = Math.max(0, (store.get()?.pending ?? 1) - 1);
    store.update({
      pending,
      diagnostic: `edit rejected: ${JSON.stringify(reason)}`,
    });
    if (shouldRequestResync(reason)) session.requestResync();
  }

  function onResync(data: Record<string, unknown>) {
    inflight.clear();
    const text = String(data.text ?? "");
    const access = (data.access ?? {}) as DocumentMeta["access"];
    store.update({
      documentId: Number(data.documentId ?? store.get()?.documentId ?? 0),
      version: Number(data.version ?? 0),
      access,
      pending: 0,
      dirty: false,
      diagnostic: null,
    });
    authoritativeText = text;
    replaceText(text, "resync");
  }

  function applyMetadata(
    metadata: Record<string, unknown>,
    options: { text?: string; resetPending: boolean },
  ) {
    if (options.resetPending) inflight.clear();
    if (!store.get()) {
      // Metadata for a pane whose document is not bound yet (e.g. restore
      // delivering the workspace root id before the open completes).
      store.set({
        documentId: 0,
        version: 0,
        dirty: false,
        access: {},
        path: "",
        workspaceRootId: null,
        workspaceRoot: "",
        pending: 0,
        behaviorVersion: behaviorManifest.behaviorVersion,
        diagnostic: null,
      });
    }
    const access = (metadata.access ??
      store.get()?.access ??
      {}) as DocumentMeta["access"];
    store.update({
      documentId: Number(metadata.documentId ?? store.get()?.documentId ?? 0),
      version: Number(metadata.version ?? store.get()?.version ?? 0),
      dirty:
        typeof metadata.dirty === "boolean"
          ? metadata.dirty
          : (store.get()?.dirty ?? false),
      access,
      path:
        typeof metadata.path === "string"
          ? metadata.path
          : (store.get()?.path ?? ""),
      workspaceRootId:
        typeof metadata.workspaceRootId === "number"
          ? metadata.workspaceRootId
          : (store.get()?.workspaceRootId ?? null),
      pending: options.resetPending ? 0 : (store.get()?.pending ?? 0),
      diagnostic: options.resetPending
        ? null
        : (store.get()?.diagnostic ?? null),
    });
    if (typeof options.text === "string") {
      authoritativeText = options.text;
      replaceText(options.text, "resync");
    }
    const rootId = store.get()?.workspaceRootId;
    if (pendingOpenPath && rootId != null) {
      const path = pendingOpenPath;
      pendingOpenPath = null;
      sendOpen(rootId, path);
    }
  }

  function onOpened(data: Record<string, unknown>) {
    const metadata = (data.metadata ?? {}) as Record<string, unknown>;
    applyMetadata(metadata, {
      text: String(data.text ?? ""),
      resetPending: true,
    });
  }

  function onSaved(data: Record<string, unknown>) {
    store.update({
      version: Number(data.version ?? store.get()?.version ?? 0),
      dirty: Boolean(data.dirty),
      diagnostic: null,
    });
  }

  function onClosed(data: Record<string, unknown>) {
    const current = store.get();
    if (!current) return;
    if (Number(data.documentId) !== current.documentId) return;
    if (data.closed) {
      store.set(null);
      replaceText("", "programmatic");
    }
  }

  function onStatus(data: Record<string, unknown>) {
    const metadata = (data.metadata ?? data) as Record<string, unknown>;
    applyMetadata(metadata, { resetPending: false });
  }

  return session;
}
