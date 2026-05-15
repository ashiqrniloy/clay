// Clay document authority facade skeleton.
//
// Document APIs are planned server-authoritative APIs. The facade keeps stable
// user imports separate from Rust internals and future op wrapper names.

export type DocumentId = string;

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

export async function serverListDocuments(): Promise<DocumentId[]> {
  plannedApi("clay.documents.serverListDocuments");
}
