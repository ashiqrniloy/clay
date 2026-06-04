// Clay document authority facade.
//
// Runtime-backed document APIs call server-owned op wrappers while keeping
// stable user imports separate from Rust internals and raw op names.

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

interface DocumentOps {
  op_clay_documents_open_document?: (optionsJson: string) => Promise<string>;
  op_clay_documents_save_document?: (optionsJson: string) => Promise<string>;
  op_clay_documents_reload_document?: (optionsJson: string) => Promise<string>;
  op_clay_documents_get_document_status?: (documentIdJson: string) => Promise<string>;
  op_clay_documents_list_documents?: () => Promise<string>;
}

declare const globalThis: { Deno?: { core?: { ops?: DocumentOps } } };

function documentOps(): Required<DocumentOps> {
  const ops = globalThis.Deno?.core?.ops;
  if (
    typeof ops?.op_clay_documents_open_document !== "function" ||
    typeof ops?.op_clay_documents_save_document !== "function" ||
    typeof ops?.op_clay_documents_reload_document !== "function" ||
    typeof ops?.op_clay_documents_get_document_status !== "function" ||
    typeof ops?.op_clay_documents_list_documents !== "function"
  ) {
    throw new Error("clay:documents runtime ops are unavailable in this environment");
  }
  return ops as Required<DocumentOps>;
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
  return JSON.parse(await documentOps().op_clay_documents_open_document(JSON.stringify(options))) as OpenDocumentResult;
}

export async function serverSaveDocument(options: SaveDocumentOptions): Promise<SaveDocumentResult> {
  return JSON.parse(await documentOps().op_clay_documents_save_document(JSON.stringify(options))) as SaveDocumentResult;
}

export async function serverReloadDocument(options: ReloadDocumentOptions): Promise<ReloadDocumentResult> {
  return JSON.parse(await documentOps().op_clay_documents_reload_document(JSON.stringify(options))) as ReloadDocumentResult;
}

export async function serverGetDocumentStatus(documentId: DocumentId): Promise<DocumentMetadata> {
  return JSON.parse(await documentOps().op_clay_documents_get_document_status(JSON.stringify(documentId))) as DocumentMetadata;
}

export async function serverListDocuments(): Promise<DocumentMetadata[]> {
  return JSON.parse(await documentOps().op_clay_documents_list_documents()) as DocumentMetadata[];
}
