// Clay document authority facade skeleton.
//
// Document APIs are planned server-authoritative APIs. The facade keeps stable
// user imports separate from Rust internals and future op wrapper names.

export type DocumentId = string;
export type WorkspaceRootId = string;

export interface DocumentSnapshot {
  documentId: DocumentId;
  version: number;
  text: string;
  readOnly: boolean;
}

export interface DocumentLease {
  documentId: DocumentId;
  leaseId: string;
  readOnly: boolean;
}

export interface DocumentMetadata {
  documentId: DocumentId;
  version: number;
  readOnly: boolean;
  leaseId?: string;
  dirty: boolean;
  workspaceRootId: WorkspaceRootId;
  path: string;
}

export interface OpenDocumentOptions {
  workspaceRootId: WorkspaceRootId;
  path: string;
}

export interface OpenDocumentResult {
  metadata: DocumentMetadata;
  text: string;
}

export interface SaveDocumentOptions {
  documentId: DocumentId;
  knownVersion?: number;
}

export interface SaveDocumentResult {
  documentId: DocumentId;
  version: number;
  dirty: boolean;
}

export interface ReloadDocumentOptions {
  documentId: DocumentId;
  knownVersion?: number;
  force?: boolean;
}

export interface ReloadDocumentResult {
  metadata: DocumentMetadata;
  text: string;
}

function plannedApi(name: string): never {
  throw new Error(`${name} is planned; Clay JS runtime op wiring is not implemented yet`);
}

export async function serverGetDocumentSnapshot(documentId: DocumentId): Promise<DocumentSnapshot> {
  void documentId;
  plannedApi("clay.documents.serverGetDocumentSnapshot");
}

export async function serverGetDocumentLease(documentId: DocumentId): Promise<DocumentLease> {
  void documentId;
  plannedApi("clay.documents.serverGetDocumentLease");
}

export async function serverOpenDocument(options: OpenDocumentOptions): Promise<OpenDocumentResult> {
  void options;
  plannedApi("clay.documents.serverOpenDocument");
}

export async function serverSaveDocument(options: SaveDocumentOptions): Promise<SaveDocumentResult> {
  void options;
  plannedApi("clay.documents.serverSaveDocument");
}

export async function serverReloadDocument(options: ReloadDocumentOptions): Promise<ReloadDocumentResult> {
  void options;
  plannedApi("clay.documents.serverReloadDocument");
}

export async function serverGetDocumentStatus(documentId: DocumentId): Promise<DocumentMetadata> {
  void documentId;
  plannedApi("clay.documents.serverGetDocumentStatus");
}

export async function serverListDocuments(): Promise<DocumentMetadata[]> {
  plannedApi("clay.documents.serverListDocuments");
}
