import { LspClient, LspResponseError } from "./client.js";
import {
  applySemanticTokenDelta,
  codeActionsToClay,
  completionToClay,
  definitionToClay,
  diagnosticsToClay,
  hoverToClay,
  inlayHintsToClay,
  semanticTokensToClay,
  signatureHelpToClay,
} from "./mapping.js";

export const DEFAULT_TOKEN_TYPES = Object.freeze([
  "namespace", "type", "class", "enum", "interface", "struct", "typeParameter", "parameter",
  "variable", "property", "enumMember", "event", "function", "method", "macro", "keyword",
  "modifier", "comment", "string", "number", "regexp", "operator", "decorator",
]);
export const DEFAULT_TOKEN_MODIFIERS = Object.freeze([
  "declaration", "definition", "readonly", "static", "deprecated", "abstract", "async",
  "modification", "documentation", "defaultLibrary",
]);

const INTELLIGENCE_FEATURES = Object.freeze([
  "hover", "definition", "codeAction", "signatureHelp",
]);

export function lspClientCapabilities({ diagnostics = "push", features } = {}) {
  const enabled = (name) => !features || features.includes(name);
  const textDocument = {
    synchronization: { dynamicRegistration: false, willSave: false, didSave: false },
    semanticTokens: {
      dynamicRegistration: false,
      requests: { range: true, full: { delta: true } },
      tokenTypes: [...DEFAULT_TOKEN_TYPES],
      tokenModifiers: [...DEFAULT_TOKEN_MODIFIERS],
      formats: ["relative"],
      overlappingTokenSupport: false,
      multilineTokenSupport: false,
    },
    publishDiagnostics: { relatedInformation: false },
  };
  if (enabled("completion")) {
    textDocument.completion = {
      dynamicRegistration: false,
      completionItem: {
        snippetSupport: true,
        documentationFormat: ["markdown", "plaintext"],
        insertReplaceSupport: false,
      },
    };
  }
  if (enabled("hover")) {
    textDocument.hover = { dynamicRegistration: false, contentFormat: ["markdown", "plaintext"] };
  }
  if (enabled("definition")) {
    textDocument.definition = { dynamicRegistration: false, linkSupport: true };
  }
  if (enabled("codeAction")) {
    textDocument.codeAction = { dynamicRegistration: false };
  }
  if (enabled("signatureHelp")) {
    textDocument.signatureHelp = {
      dynamicRegistration: false,
      signatureInformation: { documentationFormat: ["markdown", "plaintext"] },
    };
  }
  if (features?.includes("inlayHint")) {
    textDocument.inlayHint = { dynamicRegistration: false };
  }
  if (diagnostics === "pull") {
    textDocument.diagnostic = { dynamicRegistration: false, relatedDocumentSupport: false };
  }
  return Object.freeze({
    workspace: { configuration: true, workspaceFolders: true },
    textDocument,
  });
}

export function languageIdForRelativePath(
  relativePath,
  allowedLanguageIds,
  extensionMap,
  errorPrefix = "lsp",
) {
  const base = String(relativePath ?? "").split("/").pop() ?? "";
  const dot = base.lastIndexOf(".");
  if (dot <= 0 || dot === base.length - 1) {
    throw new Error(`${errorPrefix}.invalid_path: document extension is required`);
  }
  const languageId = extensionMap[base.slice(dot + 1).toLowerCase()];
  if (!languageId || !allowedLanguageIds.includes(languageId)) {
    throw new Error(`${errorPrefix}.unsupported_language: ${base}`);
  }
  return languageId;
}

function emptyResult(feature) {
  if (feature === "completion") return { status: "empty", items: [] };
  if (feature === "hover") return { status: "empty", hover: { markdown: "" } };
  if (feature === "definition") return { status: "empty", definition: { locations: [] } };
  if (feature === "codeAction") return { status: "empty", codeAction: { actions: [] } };
  return { status: "empty", signatureHelp: { signatures: [] } };
}

function boundedSafeCompletions(result) {
  const items = Array.isArray(result) ? result : result?.items;
  if (!Array.isArray(items)) return completionToClay(result);
  let safe = items.filter((item) => !item?.additionalTextEdits && !item?.command);
  while (safe.length > 0) {
    try {
      return completionToClay(Array.isArray(result) ? safe : { ...result, items: safe });
    } catch (error) {
      if (!String(error).includes("lsp.completions_too_large")) throw error;
      safe = safe.slice(0, Math.floor(safe.length / 2));
    }
  }
  return emptyResult("completion");
}

function definitionValues(result) {
  return result === null ? [] : Array.isArray(result) ? result : [result];
}

export function createLspBridge({
  packageName,
  contribution,
  diagnosticSource,
  languageId,
  languageIds,
  languageIdsByExtension,
  diagnostics = "push",
  features,
  errorPrefix = "lsp",
  startSession,
  publishDecorations,
  publishDiagnostics,
  packageManifest,
  initializationOptions,
}) {
  if (typeof packageName !== "string" || packageName.length === 0) {
    throw new Error(`${errorPrefix}.invalid_identity: packageName required`);
  }
  if (typeof contribution !== "string" || contribution.length === 0) {
    throw new Error(`${errorPrefix}.invalid_identity: contribution required`);
  }
  if (typeof diagnosticSource !== "string" || diagnosticSource.length === 0) {
    throw new Error(`${errorPrefix}.invalid_identity: diagnosticSource required`);
  }
  if (diagnostics !== "push" && diagnostics !== "pull") {
    throw new Error(`${errorPrefix}.invalid_identity: diagnostics must be push or pull`);
  }
  const enabled = (name) => !features || features.includes(name);
  if (features && !Array.isArray(features)) {
    throw new Error(`${errorPrefix}.invalid_identity: features must be an array`);
  }
  const resolveLanguageId = typeof languageId === "string"
    ? () => languageId
    : (relativePath) => languageIdForRelativePath(
      relativePath,
      languageIds,
      languageIdsByExtension,
      errorPrefix,
    );
  if (typeof languageId !== "string") {
    if (!Array.isArray(languageIds) || languageIds.length === 0) {
      throw new Error(`${errorPrefix}.invalid_identity: languageIds required`);
    }
    if (!languageIdsByExtension || typeof languageIdsByExtension !== "object") {
      throw new Error(`${errorPrefix}.invalid_identity: languageIdsByExtension required`);
    }
  }

  let client;
  const documents = new Map();
  const documentsByUri = new Map();
  const semanticState = new Map();
  const diagnosticState = new Map();
  const capabilities = lspClientCapabilities({ diagnostics, features });

  function assertIdentity(event) {
    if (event?.identity?.package !== packageName || event.identity.contribution !== contribution) {
      throw new Error(`${errorPrefix}.invalid_identity: host-stamped package contribution mismatch`);
    }
  }

  function exactDocument(documentId, version) {
    if (!client) throw new Error(`${errorPrefix}.not_started: language-server session is unavailable`);
    return client.document(documentId, version);
  }

  function publishDiagnosticItems(documentId, version, items) {
    const entry = exactDocument(documentId, version);
    publishDiagnostics({
      packageManifest,
      documentId,
      documentVersion: version,
      currentDocumentVersion: version,
      viewport: { byteStart: 0, byteEnd: entry.document.bytes.length },
      source: diagnosticSource,
      spans: diagnosticsToClay(items, entry.document),
    });
  }

  async function onNotification(method, params) {
    if (method !== "textDocument/publishDiagnostics") return;
    const documentId = documentsByUri.get(params?.uri);
    if (documentId === undefined) return;
    const tracked = documents.get(documentId);
    if (!tracked) return;
    const version = params?.version ?? tracked.version;
    if (version !== tracked.version) return;
    publishDiagnosticItems(documentId, version, params?.diagnostics ?? []);
  }

  async function ensureClient(event) {
    if (client) return client;
    const session = await startSession({
      contribution,
      workspaceRootId: event.workspaceRootId,
    });
    client = new LspClient(session, {
      onNotification,
      serverRequestHandlers: new Map([
        ["workspace/configuration", (params) => (params?.items ?? []).map(() => null)],
      ]),
    });
    await client.initialize({
      rootPath: event.canonicalRootPath,
      capabilities,
      ...(initializationOptions === undefined ? {} : { initializationOptions }),
    });
    return client;
  }

  async function refreshSemantic(documentId, version) {
    const entry = exactDocument(documentId, version);
    if (!client.capabilities.semanticTokens || !client.capabilities.semanticTokensFull) return;
    const previous = semanticState.get(documentId);
    const useDelta = Boolean(previous?.resultId && client.capabilities.semanticTokensFull?.delta);
    const response = useDelta
      ? await client.request("textDocument/semanticTokens/full/delta", {
        textDocument: { uri: entry.uri },
        previousResultId: previous.resultId,
      })
      : await client.request("textDocument/semanticTokens/full", { textDocument: { uri: entry.uri } });
    if (!response) return;
    let data;
    if (Array.isArray(response.data)) {
      data = response.data;
    } else if (useDelta) {
      data = applySemanticTokenDelta(previous?.data ?? [], response.edits ?? []);
    } else {
      return;
    }
    if (response.resultId) semanticState.set(documentId, { data, resultId: response.resultId });
    publishDecorations({
      packageManifest,
      documentId,
      documentVersion: version,
      currentDocumentVersion: version,
      viewport: { byteStart: 0, byteEnd: entry.document.bytes.length },
      spans: semanticTokensToClay(data, client.capabilities.semanticLegend, entry.document),
    });
  }

  async function refreshInlays(documentId, version) {
    if (!features?.includes("inlayHint") || !client.capabilities.inlayHint) return;
    const entry = exactDocument(documentId, version);
    const end = entry.document.bytes.length;
    const response = await client.request("textDocument/inlayHint", {
      textDocument: { uri: entry.uri },
      range: {
        start: { line: 0, character: 0 },
        end: end === 0 ? { line: 0, character: 0 } : entry.document.byteToPosition(end),
      },
    });
    publishDecorations({
      packageManifest,
      documentId,
      documentVersion: version,
      currentDocumentVersion: version,
      viewport: { byteStart: 0, byteEnd: end },
      kind: "inlayHint",
      spans: inlayHintsToClay(response, entry.document),
    });
  }

  async function refreshDiagnostics(documentId, version) {
    if (diagnostics !== "pull" || !client.capabilities.pullDiagnostics) return;
    const entry = exactDocument(documentId, version);
    const previousResultId = diagnosticState.get(documentId);
    const response = await client.request("textDocument/diagnostic", {
      textDocument: { uri: entry.uri },
      ...(previousResultId ? { previousResultId } : {}),
    });
    if (response?.kind === "unchanged") return;
    if (response?.kind !== "full" || !Array.isArray(response.items)) {
      throw new Error(`${errorPrefix}.invalid_diagnostics: expected full or unchanged document report`);
    }
    diagnosticState.set(documentId, response.resultId);
    publishDiagnosticItems(documentId, version, response.items);
  }

  async function refresh(documentId, version) {
    try {
      await refreshSemantic(documentId, version);
      await refreshInlays(documentId, version);
      await refreshDiagnostics(documentId, version);
    } catch (error) {
      if (!(error instanceof LspResponseError && (error.cancelled || error.contentModified))) throw error;
    }
  }

  function positionParams(documentId, version, byteOffset) {
    const entry = exactDocument(documentId, version);
    return { entry, position: entry.document.byteToPosition(byteOffset) };
  }

  async function completion(event) {
    const request = event.request;
    if (!enabled("completion") || !client?.capabilities.completion) return emptyResult("completion");
    const { entry, position } = positionParams(request.documentId, request.documentVersion, request.cursorByteOffset);
    const trigger = request.trigger?.kind === "character"
      ? { triggerKind: 2, triggerCharacter: request.trigger.character }
      : { triggerKind: 1 };
    return boundedSafeCompletions(await client.request("textDocument/completion", {
      textDocument: { uri: entry.uri },
      position,
      context: trigger,
    }));
  }

  async function intelligence(event) {
    const request = event.request;
    const feature = request.feature;
    if (!INTELLIGENCE_FEATURES.includes(feature)) {
      throw new Error(`${errorPrefix}.unsupported_feature: ${feature}`);
    }
    if (!enabled(feature)) return emptyResult(feature);
    const { entry, position } = positionParams(request.documentId, request.documentVersion, request.cursorByteOffset);
    const textDocumentPosition = { textDocument: { uri: entry.uri }, position };
    if (feature === "hover") {
      if (!client.capabilities.hover) return emptyResult(feature);
      return hoverToClay(await client.request("textDocument/hover", textDocumentPosition), entry.document);
    }
    if (feature === "definition") {
      if (!client.capabilities.definition) return emptyResult(feature);
      const response = await client.request("textDocument/definition", textDocumentPosition);
      const open = definitionValues(response).filter((location) => documentsByUri.has(location?.targetUri ?? location?.uri));
      return definitionToClay(open, ({ uri, range }) => {
        const targetId = documentsByUri.get(uri);
        const target = documents.get(targetId);
        const targetEntry = exactDocument(targetId, target.version);
        return { documentId: targetId, range: targetEntry.document.rangeToBytes(range) };
      });
    }
    if (feature === "codeAction") {
      if (!client.capabilities.codeAction) return emptyResult(feature);
      return codeActionsToClay(await client.request("textDocument/codeAction", {
        textDocument: { uri: entry.uri },
        range: { start: position, end: position },
        context: { diagnostics: [] },
      }));
    }
    if (feature === "signatureHelp") {
      if (!client.capabilities.signatureHelp) return emptyResult(feature);
      return signatureHelpToClay(await client.request("textDocument/signatureHelp", textDocumentPosition));
    }
    throw new Error(`${errorPrefix}.unsupported_feature: ${feature}`);
  }

  async function handle(event) {
    assertIdentity(event);
    if (event.kind === "open") {
      await ensureClient(event);
      const resolvedLanguageId = resolveLanguageId(event.relativePath);
      const uri = await client.openDocument({
        documentId: event.documentId,
        rootPath: event.canonicalRootPath,
        relativePath: event.relativePath,
        languageId: resolvedLanguageId,
        version: event.documentVersion,
        text: event.text,
      });
      documents.set(event.documentId, { uri, version: event.documentVersion, languageId: resolvedLanguageId });
      documentsByUri.set(uri, event.documentId);
      await refresh(event.documentId, event.documentVersion);
      return null;
    }
    if (event.kind === "change") {
      await client.changeDocument({
        documentId: event.documentId,
        baseVersion: event.baseVersion,
        version: event.documentVersion,
        byteStart: event.byteStart,
        byteEnd: event.byteEnd,
        insertedText: event.insertedText,
      });
      documents.get(event.documentId).version = event.documentVersion;
      await refresh(event.documentId, event.documentVersion);
      return null;
    }
    if (event.kind === "reset") {
      await client.resetDocument({ documentId: event.documentId, version: event.documentVersion, text: event.text });
      documents.get(event.documentId).version = event.documentVersion;
      await refresh(event.documentId, event.documentVersion);
      return null;
    }
    if (event.kind === "close") {
      const tracked = documents.get(event.documentId);
      await client?.closeDocument(event.documentId);
      if (tracked) documentsByUri.delete(tracked.uri);
      documents.delete(event.documentId);
      semanticState.delete(event.documentId);
      diagnosticState.delete(event.documentId);
      return null;
    }
    if (event.kind === "completion") return completion(event);
    if (event.kind === "languageIntelligence") return intelligence(event);
    if (event.kind === "shutdown") {
      await client?.shutdown();
      client = undefined;
      documents.clear();
      documentsByUri.clear();
      semanticState.clear();
      diagnosticState.clear();
      return null;
    }
    throw new Error(`${errorPrefix}.invalid_event: ${event.kind}`);
  }

  return Object.freeze({ handle });
}
