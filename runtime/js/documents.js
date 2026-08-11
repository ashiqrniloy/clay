// Clay document authority facade.
//
// Runtime-backed document APIs call server-owned op wrappers while keeping
// stable user imports separate from Rust internals and raw op names.
function documentOps() {
    const ops = globalThis.Deno?.core?.ops;
    if (typeof ops?.op_clay_documents_open_document !== "function" ||
        typeof ops?.op_clay_documents_save_document !== "function" ||
        typeof ops?.op_clay_documents_reload_document !== "function" ||
        typeof ops?.op_clay_documents_get_document_status !== "function" ||
        typeof ops?.op_clay_documents_list_documents !== "function") {
        throw new Error("clay:documents runtime ops are unavailable in this environment");
    }
    return ops;
}
function plannedApi(name) {
    throw new Error(`${name} is planned; Clay JS runtime op wiring is not implemented yet`);
}
export async function serverGetDocumentSnapshot(documentId) {
    void documentId;
    plannedApi("documents.serverGetDocumentSnapshot");
}
export async function serverGetDocumentLease(documentId) {
    void documentId;
    plannedApi("documents.serverGetDocumentLease");
}
export function clientOpenFileDialog() {
    return "documents.clientOpenFileDialog";
}
export async function serverOpenDocument(options) {
    return JSON.parse(await documentOps().op_clay_documents_open_document(JSON.stringify(options)));
}
export async function serverSaveDocument(options) {
    return JSON.parse(await documentOps().op_clay_documents_save_document(JSON.stringify(options)));
}
export async function serverReloadDocument(options) {
    return JSON.parse(await documentOps().op_clay_documents_reload_document(JSON.stringify(options)));
}
export async function serverGetDocumentStatus(documentId) {
    return JSON.parse(await documentOps().op_clay_documents_get_document_status(JSON.stringify(documentId)));
}
export async function serverListDocuments() {
    return JSON.parse(await documentOps().op_clay_documents_list_documents());
}
