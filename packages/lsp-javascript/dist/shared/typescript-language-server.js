import { LspClient, LspResponseError } from "./client.js";
import {
  codeActionsToClay,
  completionToClay,
  definitionToClay,
  diagnosticsToClay,
  hoverToClay,
  semanticTokensToClay,
  signatureHelpToClay,
} from "./mapping.js";

const TOKEN_TYPES = [
  "namespace", "type", "class", "enum", "interface", "struct", "typeParameter", "parameter",
  "variable", "property", "enumMember", "event", "function", "method", "macro", "keyword",
  "modifier", "comment", "string", "number", "regexp", "operator", "decorator",
];
const TOKEN_MODIFIERS = [
  "declaration", "definition", "readonly", "static", "deprecated", "abstract", "async",
  "modification", "documentation", "defaultLibrary",
];

const EXTENSION_LANGUAGE_IDS = Object.freeze({
  ts: "typescript",
  mts: "typescript",
  cts: "typescript",
  tsx: "typescriptreact",
  js: "javascript",
  mjs: "javascript",
  cjs: "javascript",
  jsx: "javascriptreact",
});

export const typescriptLanguageServerClientCapabilities = Object.freeze({
  workspace: { configuration: true, workspaceFolders: true },
  textDocument: {
    synchronization: { dynamicRegistration: false, willSave: false, didSave: false },
    completion: {
      dynamicRegistration: false,
      completionItem: {
        snippetSupport: true,
        documentationFormat: ["markdown", "plaintext"],
        insertReplaceSupport: false,
      },
    },
    hover: { dynamicRegistration: false, contentFormat: ["markdown", "plaintext"] },
    definition: { dynamicRegistration: false, linkSupport: true },
    codeAction: { dynamicRegistration: false },
    signatureHelp: {
      dynamicRegistration: false,
      signatureInformation: { documentationFormat: ["markdown", "plaintext"] },
    },
    semanticTokens: {
      dynamicRegistration: false,
      requests: { range: true, full: { delta: true } },
      tokenTypes: TOKEN_TYPES,
      tokenModifiers: TOKEN_MODIFIERS,
      formats: ["relative"],
      overlappingTokenSupport: false,
      multilineTokenSupport: false,
    },
    publishDiagnostics: { relatedInformation: false },
  },
});

export function defaultTypescriptLanguageServerInitializationOptions({ tsserverPath } = {}) {
  const options = {
    hostInfo: "Clay",
    disableAutomaticTypingAcquisition: true,
  };
  if (typeof tsserverPath === "string" && tsserverPath.length > 0) {
    options.tsserver = { path: tsserverPath };
  }
  return options;
}

export function languageIdForRelativePath(relativePath, allowedLanguageIds) {
  const base = String(relativePath ?? "").split("/").pop() ?? "";
  const dot = base.lastIndexOf(".");
  if (dot <= 0 || dot === base.length - 1) {
    throw new Error("lsp-typescript.invalid_path: document extension is required");
  }
  const languageId = EXTENSION_LANGUAGE_IDS[base.slice(dot + 1).toLowerCase()];
  if (!languageId || !allowedLanguageIds.includes(languageId)) {
    throw new Error(`lsp-typescript.unsupported_language: ${base}`);
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

export function createTypescriptLanguageServerBridge({
  packageName,
  contribution,
  diagnosticSource,
  languageIds,
  startSession,
  publishDecorations,
  publishDiagnostics,
  packageManifest,
  tsserverPath,
  initializationOptions,
}) {
  if (typeof packageName !== "string" || packageName.length === 0) {
    throw new Error("lsp-typescript.invalid_identity: packageName required");
  }
  if (typeof contribution !== "string" || contribution.length === 0) {
    throw new Error("lsp-typescript.invalid_identity: contribution required");
  }
  if (typeof diagnosticSource !== "string" || diagnosticSource.length === 0) {
    throw new Error("lsp-typescript.invalid_identity: diagnosticSource required");
  }
  if (!Array.isArray(languageIds) || languageIds.length === 0) {
    throw new Error("lsp-typescript.invalid_identity: languageIds required");
  }

  let client;
  const documents = new Map();
  const documentsByUri = new Map();

  function assertIdentity(event) {
    if (event?.identity?.package !== packageName || event.identity.contribution !== contribution) {
      throw new Error("lsp-typescript.invalid_identity: host-stamped package contribution mismatch");
    }
  }

  function exactDocument(documentId, version) {
    if (!client) throw new Error("lsp-typescript.not_started: typescript-language-server session is unavailable");
    return client.document(documentId, version);
  }

  function publishDiagnosticItems(documentId, version, items) {
    const entry = exactDocument(documentId, version);
    publishDiagnostics({
      packageManifest,
      documentId,
      documentVersion: version,
      currentDocumentVersion: version,
      viewport: { byteStart: 0, byteEnd: entry.document.byteLength },
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
      package: packageName,
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
      capabilities: typescriptLanguageServerClientCapabilities,
      initializationOptions: {
        ...defaultTypescriptLanguageServerInitializationOptions({ tsserverPath }),
        ...(initializationOptions ?? {}),
      },
    });
    return client;
  }

  async function refreshSemantic(documentId, version) {
    const entry = exactDocument(documentId, version);
    if (!client.capabilities.semanticTokens || !client.capabilities.semanticTokensFull) return;
    const response = await client.request("textDocument/semanticTokens/full", {
      textDocument: { uri: entry.uri },
    });
    if (!response || !Array.isArray(response.data)) return;
    publishDecorations({
      packageManifest,
      documentId,
      documentVersion: version,
      currentDocumentVersion: version,
      viewport: { byteStart: 0, byteEnd: entry.document.byteLength },
      spans: semanticTokensToClay(response.data, client.capabilities.semanticLegend, entry.document),
    });
  }

  async function refresh(documentId, version) {
    try {
      await refreshSemantic(documentId, version);
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
    if (!client?.capabilities.completion) return emptyResult("completion");
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
    throw new Error(`lsp-typescript.unsupported_feature: ${feature}`);
  }

  async function handle(event) {
    assertIdentity(event);
    if (event.kind === "open") {
      await ensureClient(event);
      const languageId = languageIdForRelativePath(event.relativePath, languageIds);
      const uri = await client.openDocument({
        documentId: event.documentId,
        rootPath: event.canonicalRootPath,
        relativePath: event.relativePath,
        languageId,
        version: event.documentVersion,
        text: event.text,
      });
      documents.set(event.documentId, { uri, version: event.documentVersion, languageId });
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
      return null;
    }
    if (event.kind === "completion") return completion(event);
    if (event.kind === "languageIntelligence") return intelligence(event);
    if (event.kind === "shutdown") {
      await client?.shutdown();
      client = undefined;
      documents.clear();
      documentsByUri.clear();
      return null;
    }
    throw new Error(`lsp-typescript.invalid_event: ${event.kind}`);
  }

  return Object.freeze({ handle });
}
