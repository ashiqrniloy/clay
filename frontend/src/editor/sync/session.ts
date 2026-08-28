import { Text } from "@codemirror/state";
import type { EditorView } from "@codemirror/view";

import type { BootstrapDto, BridgeEnvelope } from "../../bridge/types";
import {
  accessIsEditable,
  createDocumentStore,
  metaFromInitial,
  type DocumentMeta,
  type DocumentStore,
} from "../../state/document-store";
import type { BytePositionIndex } from "../position-index";
import { programmaticAnnotations } from "../transactions";
import {
  editorPerformance,
  PERFORMANCE_STAGE,
  type PerformanceSpan,
} from "../performance";
import {
  closePayload,
  DOCUMENT_CHUNK_BYTES,
  documentChunkRequestPayload,
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
  "viewportRenderPatch",
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
  /** Last authoritative snapshot (initial / open / resync / chunks). */
  snapshotDoc(): Text;
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
  emitUserChanges(
    oldDoc: Text,
    changes: readonly TextChange[],
    traceId?: number,
    index?: BytePositionIndex,
  ): void;
  save(): void;
  reload(force?: boolean): void;
  close(force?: boolean): void;
  open(path: string): void;
  /** Path of an open request awaiting its server reply, if any. */
  inFlightOpenPath(): string | null;
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
  const typingSpans = new Map<number, PerformanceSpan>();
  /** Detached text snapshot — the single current `Text` only while no view
   * is attached. When a view exists, `view.state.doc` owns the document. */
  let detachedDoc: Text = Text.empty;
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
  /** Path whose OpenDocument is in flight; lets open replies find their pane. */
  let inFlightOpenPath: string | null = null;
  let pendingOpenTrace: { traceId: number; span: PerformanceSpan } | null =
    null;
  const sendOpen = (rootId: number, path: string) => {
    inFlightOpenPath = path;
    void options.send(openPayload(rootId, path)).catch((error: unknown) => {
      pendingOpenTrace?.span.end();
      pendingOpenTrace = null;
      inFlightOpenPath = null;
      store.update({
        diagnostic: error instanceof Error ? error.message : "request failed",
      });
    });
  };
  const openDocument = (path: string) => {
    const traceId = editorPerformance.trace();
    pendingOpenTrace = {
      traceId,
      span: editorPerformance.span(PERFORMANCE_STAGE.editorOpen, traceId, {
        feature: "documentOpen",
      }),
    };
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

  /** UTF-8 byte length of a JS string (wire offsets are byte-based). */
  const utf8Length = (text: string): number => {
    let bytes = 0;
    for (let i = 0; i < text.length; i += 1) {
      const code = text.charCodeAt(i);
      if (code < 0x80) bytes += 1;
      else if (code < 0x800) bytes += 2;
      else if (code >= 0xd800 && code <= 0xdbff && i + 1 < text.length) {
        bytes += 4;
        i += 1;
      } else bytes += 3;
    }
    return bytes;
  };

  /** Rope-backed Text from an arbitrary string without a flat copy.
   * `Text.of` takes lines, so split and let it rejoin them. */
  const textOf = (value: string): Text =>
    value ? Text.of(value.split("\n")) : Text.empty;

  interface DocumentLoad {
    totalBytes: number;
    traceId: number;
    readySpan: PerformanceSpan;
    /** Wire bytes appended into the snapshot/editor so far. */
    nextAppend: number;
    /** One outstanding request at a time: server responses are clamped to
     * UTF-8 char boundaries, so region starts are only known after the
     * previous reply lands. A fixed stride would strand on short chunks. */
    inflight: Set<number>;
    done: boolean;
  }

  let load: DocumentLoad | null = null;

  /** The one current document: the live view's rope, or the detached snapshot. */
  const currentDoc = (): Text => (view ? view.state.doc : detachedDoc);

  /** Installs an authoritative snapshot as the whole current text.
   * Content equality (not length): a same-length reload still installs. */
  const installAuthoritative = (next: Text) => {
    if (view) {
      const current = view.state.doc;
      if (current.eq(next)) return;
      view.dispatch({
        changes: { from: 0, to: current.length, insert: next },
        annotations: programmaticAnnotations(),
      });
    } else {
      if (detachedDoc.eq(next)) return;
      detachedDoc = next;
    }
  };

  const finishLoad = () => {
    if (!load || load.done) return;
    load.done = true;
    load.inflight.clear();
    load.readySpan.end({ byteCount: load.totalBytes });
    editorPerformance.mark(PERFORMANCE_STAGE.editorReady, load.traceId, {
      documentId: store.get()?.documentId,
      version: store.get()?.version,
      byteCount: load.totalBytes,
    });
    store.update({ loading: false });
  };

  const abortLoad = (message: string) => {
    if (!load || load.done) return;
    // Gate stays closed: editing a half-assembled document would desync
    // version/ack semantics. Reload/resync restarts the load cleanly.
    load.done = true;
    load.inflight.clear();
    store.update({ diagnostic: message });
  };

  const pumpLoad = () => {
    const meta = store.get();
    if (!load || load.done || !meta || meta.documentId === 0) return;
    if (load.inflight.size > 0 || load.nextAppend >= load.totalBytes) return;
    const offset = load.nextAppend;
    load.inflight.add(offset);
    send(
      documentChunkRequestPayload(
        meta.documentId,
        meta.version,
        offset,
        DOCUMENT_CHUNK_BYTES,
      ),
    );
  };

  const appendLoaded = (text: string) => {
    if (!load) return;
    load.nextAppend += utf8Length(text);
    if (!text) return;
    if (view) {
      view.dispatch({
        changes: { from: view.state.doc.length, insert: text },
        annotations: programmaticAnnotations(),
      });
      editorPerformance.mark(PERFORMANCE_STAGE.patchApply, load.traceId, {
        documentId: store.get()?.documentId,
        version: store.get()?.version,
        byteCount: utf8Length(text),
        feature: "documentChunk",
      });
    } else {
      detachedDoc = detachedDoc.append(textOf(text));
    }
  };

  /** Starts (or restarts) progressive assembly from an authoritative head:
   * paint the first chunk immediately, then fetch the remainder in order. */
  const startLoad = (
    head: unknown,
    openTrace: {
      traceId: number;
      span: PerformanceSpan;
    } | null = pendingOpenTrace,
  ) => {
    pendingOpenTrace = null;
    const data = (head ?? {}) as Record<string, unknown>;
    const headText = String(data.firstChunk ?? "");
    installAuthoritative(textOf(headText));
    const totalBytes = Math.max(
      0,
      Number(data.totalBytes ?? utf8Length(headText)),
    );
    const headBytes = utf8Length(headText);
    const traceId = openTrace?.traceId ?? editorPerformance.trace();
    const meta = store.get();
    if (!openTrace)
      editorPerformance.mark(PERFORMANCE_STAGE.editorOpen, traceId, {
        documentId: meta?.documentId,
        version: meta?.version,
        byteCount: totalBytes,
      });
    load = {
      totalBytes,
      traceId,
      readySpan:
        openTrace?.span ??
        editorPerformance.span(PERFORMANCE_STAGE.editorOpen, traceId, {
          documentId: meta?.documentId,
          version: meta?.version,
        }),
      nextAppend: headBytes,
      inflight: new Set(),
      done: false,
    };
    if (totalBytes <= headBytes) {
      finishLoad();
      return;
    }
    store.update({ loading: true });
    pumpLoad();
  };

  const session: DocumentSession = {
    store,
    snapshotDoc: () => currentDoc(),
    installInitial(bootstrap) {
      inflight.clear();
      typingSpans.clear();
      clientId = bootstrap.clientId;
      featureEvents.length = 0;
      behaviorManifest = bootstrap.behaviorManifest;
      store.set(
        metaFromInitial({
          documentId: bootstrap.initialDocument.documentId,
          version: bootstrap.initialDocument.version,
          access: bootstrap.initialDocument.access,
          workspaceRoot: bootstrap.initialDocument.workspaceRoot,
          behaviorVersion: bootstrap.behaviorManifest.behaviorVersion,
        }),
      );
      startLoad(bootstrap.initialDocument.head);
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
      installAuthoritative(detachedDoc);
    },
    detachView(current) {
      if (view === current) {
        // Latest user text (acked or not) becomes the detached snapshot.
        detachedDoc = current.state.doc;
        view = null;
      }
    },
    setClientCommandHandler(handler) {
      clientCommandHandler = handler;
    },
    runClientCommand(commandId) {
      return clientCommandHandler?.(commandId) ?? false;
    },
    emitUserChanges(oldDoc, changes, traceId, index) {
      const meta = store.get();
      if (!meta || !accessIsEditable(meta.access)) return;
      // Progressive load in flight: the document is gated read-only; never
      // queue edits against a partially assembled snapshot.
      if (load && !load.done) return;
      const operations = changesToOperations(oldDoc, changes, index);
      if (operations.length === 0) return;
      let pending = meta.pending;
      for (const [index, operation] of operations.entries()) {
        const transactionId =
          index === 0 && traceId && traceId > 0 ? traceId : nextTransactionId;
        nextTransactionId = Math.max(nextTransactionId + 1, transactionId + 1);
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
        if (index === 0 && traceId && traceId > 0 && typingSpans.size < 256)
          typingSpans.set(
            transactionId,
            editorPerformance.span(PERFORMANCE_STAGE.editorTyping, traceId, {
              documentId: meta.documentId,
              version: meta.version,
            }),
          );
      }
      if (traceId && traceId > 0)
        editorPerformance.mark(PERFORMANCE_STAGE.editorTyping, traceId, {
          documentId: meta.documentId,
          version: meta.version,
        });
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
      const transactionId = Number(data.transactionId);
      if (
        (kind === "editAck" || kind === "editRejected") &&
        transactionId > 0
      ) {
        editorPerformance.mark(
          PERFORMANCE_STAGE.bridgeServerDelivery,
          transactionId,
          {
            documentId: Number(data.documentId),
            version: Number(data.version),
            transactionId,
          },
        );
      }
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
        case "documentChunk":
          onDocumentChunk(data);
          return;
        case "documentChunkRejected":
          onDocumentChunkRejected(data);
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
    inFlightOpenPath() {
      return inFlightOpenPath;
    },
    requestResync() {
      const meta = store.get();
      if (!meta) return;
      // A resync replaces the assembled prefix; stop consuming old chunks.
      if (load && !load.done) load.done = true;
      send(requestResyncPayload(meta.documentId, meta.version));
    },
  };

  function onAck(data: Record<string, unknown>) {
    const transactionId = Number(data.transactionId);
    typingSpans.get(transactionId)?.end();
    typingSpans.delete(transactionId);
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
    typingSpans.get(transactionId)?.end();
    typingSpans.delete(transactionId);
    inflight.delete(transactionId);
    const reason = data.reason as EditRejection;
    const pending = Math.max(0, (store.get()?.pending ?? 1) - 1);
    store.update({
      pending,
      diagnostic: `edit rejected: ${JSON.stringify(reason)}`,
    });
    if (shouldRequestResync(reason)) session.requestResync();
  }

  function onDocumentChunk(data: Record<string, unknown>) {
    if (!load || load.done) return;
    const meta = store.get();
    if (!meta || Number(data.documentId) !== meta.documentId) return;
    if (Number(data.documentVersion) !== meta.version) return;
    // Only chunks this load actually requested are consumed; duplicates and
    // unsolicited offsets drop here.
    const offset = Number(data.offset);
    if (!load.inflight.delete(offset)) return;
    const text = String(data.text ?? "");
    editorPerformance.mark(
      PERFORMANCE_STAGE.bridgeServerDelivery,
      load.traceId,
      {
        documentId: meta.documentId,
        version: meta.version,
        byteCount: utf8Length(text),
        feature: "documentChunk",
      },
    );
    appendLoaded(text);
    if (load.nextAppend >= load.totalBytes) {
      finishLoad();
      return;
    }
    pumpLoad();
  }

  function onDocumentChunkRejected(data: Record<string, unknown>) {
    if (!load || load.done) return;
    const meta = store.get();
    if (!meta || Number(data.documentId) !== meta.documentId) return;
    const reason = data.reason as EditRejection;
    const key =
      typeof reason === "string"
        ? reason
        : (Object.keys(reason)[0] ?? "unknown");
    if (key === "staleVersion") {
      // The assembled prefix belongs to an older generation. Resync restarts
      // the whole load against the current version.
      session.requestResync();
      return;
    }
    abortLoad(`document chunk rejected: ${key}`);
  }

  function onResync(data: Record<string, unknown>) {
    inflight.clear();
    const access = (data.access ?? {}) as DocumentMeta["access"];
    store.update({
      documentId: Number(data.documentId ?? store.get()?.documentId ?? 0),
      version: Number(data.version ?? 0),
      access,
      pending: 0,
      dirty: false,
      diagnostic: null,
    });
    startLoad(data.head);
  }

  function applyMetadata(
    metadata: Record<string, unknown>,
    options: { resetPending: boolean },
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
        loading: false,
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
    const rootId = store.get()?.workspaceRootId;
    if (pendingOpenPath && rootId != null) {
      const path = pendingOpenPath;
      pendingOpenPath = null;
      sendOpen(rootId, path);
    }
  }

  function onOpened(data: Record<string, unknown>) {
    inFlightOpenPath = null;
    const metadata = (data.metadata ?? {}) as Record<string, unknown>;
    applyMetadata(metadata, { resetPending: true });
    if (data.head != null) {
      startLoad(data.head, pendingOpenTrace);
    } else {
      store.update({ loading: false });
    }
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
      installAuthoritative(Text.empty);
      load = null;
      typingSpans.clear();
    }
  }

  function onStatus(data: Record<string, unknown>) {
    const metadata = (data.metadata ?? data) as Record<string, unknown>;
    applyMetadata(metadata, { resetPending: false });
  }

  return session;
}
