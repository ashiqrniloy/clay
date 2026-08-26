// Metadata/session projection only. Document *text* lives in CodeMirror.

export type DocumentAccess =
  | { readOnly: null }
  | { editable: { leaseId: number } }
  | Record<string, unknown>;

export function accessIsEditable(access: DocumentAccess | undefined): boolean {
  if (!access || typeof access !== "object") return false;
  return "editable" in access && access.editable != null;
}

export interface DocumentMeta {
  documentId: number;
  version: number;
  dirty: boolean;
  access: DocumentAccess;
  path: string;
  workspaceRootId: number | null;
  workspaceRoot: string;
  pending: number;
  /** Progressive chunk load in flight; editing is gated until false. */
  loading: boolean;
  behaviorVersion: number;
  diagnostic: string | null;
}

export interface DocumentStore {
  get(): DocumentMeta | null;
  set(next: DocumentMeta | null): void;
  update(patch: Partial<DocumentMeta>): DocumentMeta | null;
  subscribe(listener: () => void): () => void;
}

export function createDocumentStore(
  initial: DocumentMeta | null = null,
): DocumentStore {
  let state = initial;
  const listeners = new Set<() => void>();
  const notify = () => {
    for (const listener of [...listeners]) listener();
  };
  return {
    get: () => state,
    set(next) {
      state = next;
      notify();
    },
    update(patch) {
      if (!state) return null;
      state = { ...state, ...patch };
      notify();
      return state;
    },
    subscribe(listener) {
      listeners.add(listener);
      return () => {
        listeners.delete(listener);
      };
    },
  };
}

export function metaFromInitial(input: {
  documentId: number;
  version: number;
  access: DocumentAccess;
  workspaceRoot: string;
  behaviorVersion: number;
}): DocumentMeta {
  return {
    documentId: input.documentId,
    version: input.version,
    dirty: false,
    access: input.access,
    path: "",
    workspaceRootId: null,
    workspaceRoot: input.workspaceRoot,
    pending: 0,
    loading: false,
    behaviorVersion: input.behaviorVersion,
    diagnostic: null,
  };
}
