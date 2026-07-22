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
export type ClientOpenFileDialogCommandId = "clay.documents.clientOpenFileDialog";
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
export declare function serverGetDocumentSnapshot(documentId: DocumentId): Promise<DocumentSnapshot>;
export declare function serverGetDocumentLease(documentId: DocumentId): Promise<DocumentLease>;
export declare function clientOpenFileDialog(): ClientOpenFileDialogCommandId;
export declare function serverOpenDocument(options: OpenDocumentOptions): Promise<OpenDocumentResult>;
export declare function serverSaveDocument(options: SaveDocumentOptions): Promise<SaveDocumentResult>;
export declare function serverReloadDocument(options: ReloadDocumentOptions): Promise<ReloadDocumentResult>;
export declare function serverGetDocumentStatus(documentId: DocumentId): Promise<DocumentMetadata>;
export declare function serverListDocuments(): Promise<DocumentMetadata[]>;
