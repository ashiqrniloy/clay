import { LspClient, LspResponseError } from "./shared/client.js";
import {
  codeActionsToClay,
  completionToClay,
  definitionToClay,
  diagnosticsToClay,
  hoverToClay,
  semanticTokensToClay,
} from "./shared/mapping.js";
import {
  contributionId,
  languageIds,
  lspMarkdownPackageManifest,
  packageName,
} from "./index.js";

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
  md: "markdown",
  markdown: "markdown",
  mdown: "markdown",
});

export const marksmanClientCapabilities = Object.freeze({
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
    references: { dynamicRegistration: false },
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

export function languageIdForRelativePath(relativePath, allowedLanguageIds = languageIds) {
  const base = String(relativePath ?? "").split("/").pop() ?? "";
  const dot = base.lastIndexOf(".");
  if (dot <= 0 || dot === base.length - 1) {
    throw new Error("lsp-markdown.invalid_path: document extension is required");
  }
  const languageId = EXTENSION_LANGUAGE_IDS[base.slice(dot + 1).toLowerCase()];
  if (!languageId || !allowedLanguageIds.includes(languageId)) {
    throw new Error(`lsp-markdown.unsupported_language: ${base}`);
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

export function createMarksmanBridge({
  startSession,
  publishDecorations,
  publishDiagnostics,
  packageManifest,
}) {
  let client;
  const documents = new Map();
  const documentsByUri = new Map();

  function assertIdentity(event) {
    if (event?.identity?.package !== packageName || event.identity.contribution !== contributionId) {
      throw new Error("lsp-markdown.invalid_identity: host-stamped package contribution mismatch");
    }
  }

  function exactDocument(documentId, version) {
    if (!client) throw new Error("lsp-markdown.not_started: marksman session is unavailable");
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
      source: "lsp-markdown",
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
      contribution: contributionId,
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
      capabilities: marksmanClientCapabilities,
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
      return emptyResult(feature);
    }
    throw new Error(`lsp-markdown.unsupported_feature: ${feature}`);
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
    throw new Error(`lsp-markdown.invalid_event: ${event.kind}`);
  }

  return Object.freeze({ handle });
}

let defaultBridge;

export async function handleDocumentAnalysis(event) {
  if (!defaultBridge) {
    const [
      { startLanguageServerSession },
      { serverPublishDecorations },
      { serverPublishDiagnostics },
    ] = await Promise.all([
      import("clay:language-server"),
      import("clay:decorations"),
      import("clay:diagnostics"),
    ]);
    defaultBridge = createMarksmanBridge({
      startSession: startLanguageServerSession,
      publishDecorations: serverPublishDecorations,
      publishDiagnostics: serverPublishDiagnostics,
      packageManifest: lspMarkdownPackageManifest(),
    });
  }
  return defaultBridge.handle(event);
}
