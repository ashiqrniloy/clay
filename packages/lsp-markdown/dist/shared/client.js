import { encodeFrame, FrameDecoder, MAX_FRAME_BYTES } from "./framing.js";
import { parseCapabilities } from "./mapping.js";
import { pathToFileUri, rootPathToFileUri, VersionedDocument } from "./positions.js";

const MAX_PENDING_REQUESTS = 8;
const MAX_CANCELLED_TOMBSTONES = 64;
const READ_TIMEOUT_MS = 5000;

function validateMessage(message) {
  if (message?.jsonrpc !== "2.0") throw new Error("lsp.invalid_message: jsonrpc must equal 2.0");
  if ("method" in message && typeof message.method !== "string") throw new Error("lsp.invalid_message: method must be a string");
  if ("id" in message && message.id !== null && typeof message.id !== "string" && !Number.isInteger(message.id)) {
    throw new Error("lsp.invalid_message: id must be integer or string");
  }
  const hasMethod = "method" in message;
  const hasResult = "result" in message;
  const hasError = "error" in message;
  if ((hasMethod && (hasResult || hasError))
      || (!hasMethod && (!Object.prototype.hasOwnProperty.call(message, "id") || hasResult === hasError))
      || (hasError && (message.error === null || typeof message.error !== "object"
        || !Number.isInteger(message.error.code) || typeof message.error.message !== "string"))) {
    throw new Error("lsp.invalid_message: malformed request, notification, or response");
  }
}

export class LspResponseError extends Error {
  constructor(error) {
    super(`lsp.response_error: ${error?.code ?? "unknown"} ${error?.message ?? "request failed"}`);
    this.code = error?.code;
    this.data = error?.data;
    this.cancelled = this.code === -32800;
    this.contentModified = this.code === -32801;
  }
}

export class LspClient {
  #decoder;
  #nextId = 1;
  #pending = new Map();
  #completed = new Map();
  #cancelled = new Set();
  #documents = new Map();

  constructor(session, {
    onNotification = () => undefined,
    serverRequestHandlers = new Map(),
    maxFrameBytes = MAX_FRAME_BYTES,
    readTimeoutMs = READ_TIMEOUT_MS,
  } = {}) {
    if (!session || typeof session.sendBytes !== "function" || typeof session.readBytes !== "function") {
      throw new Error("lsp.invalid_session: exact byte session required");
    }
    this.session = session;
    this.onNotification = onNotification;
    this.serverRequestHandlers = serverRequestHandlers;
    this.maxFrameBytes = maxFrameBytes;
    this.readTimeoutMs = readTimeoutMs;
    this.#decoder = new FrameDecoder({ maxFrameBytes });
    this.state = "created";
    this.capabilities = null;
  }

  async #send(message) {
    await this.session.sendBytes(encodeFrame(message, this.maxFrameBytes));
  }

  async sendRequest(method, params) {
    if (typeof method !== "string" || method.length === 0) throw new Error("lsp.invalid_method: non-empty request method required");
    if (this.#pending.size >= MAX_PENDING_REQUESTS) throw new Error("lsp.too_many_requests: pending request budget exhausted");
    const id = this.#nextId;
    this.#nextId += 1;
    this.#pending.set(id, { method });
    try {
      await this.#send({ jsonrpc: "2.0", id, method, params });
      return id;
    } catch (error) {
      this.#pending.delete(id);
      throw error;
    }
  }

  async notify(method, params) {
    if (typeof method !== "string" || method.length === 0) throw new Error("lsp.invalid_method: non-empty notification method required");
    await this.#send({ jsonrpc: "2.0", method, params });
  }

  async cancelRequest(id) {
    if (!this.#pending.has(id)) return false;
    this.#pending.delete(id);
    this.#cancelled.add(id);
    while (this.#cancelled.size > MAX_CANCELLED_TOMBSTONES) {
      this.#cancelled.delete(this.#cancelled.values().next().value);
    }
    await this.notify("$/cancelRequest", { id });
    return true;
  }

  async receiveBytes(bytes) {
    for (const message of this.#decoder.push(bytes)) await this.#dispatch(message);
  }

  async #dispatch(message) {
    validateMessage(message);
    if ("method" in message) {
      if (!("id" in message)) {
        await this.onNotification(message.method, message.params);
        return;
      }
      const handler = this.serverRequestHandlers.get(message.method);
      if (!handler) {
        await this.#send({ jsonrpc: "2.0", id: message.id, error: { code: -32601, message: "Method not found" } });
        return;
      }
      try {
        await this.#send({ jsonrpc: "2.0", id: message.id, result: await handler(message.params) ?? null });
      } catch {
        await this.#send({ jsonrpc: "2.0", id: message.id, error: { code: -32603, message: "Internal error" } });
      }
      return;
    }
    if (this.#cancelled.delete(message.id)) return;
    if (!this.#pending.delete(message.id)) throw new Error("lsp.unknown_response: duplicate or unknown response id");
    this.#completed.set(message.id, "error" in message ? { error: new LspResponseError(message.error) } : { result: message.result });
  }

  async pump(timeoutMs = this.readTimeoutMs) {
    const bytes = await this.session.readBytes(this.maxFrameBytes, timeoutMs);
    if (!(bytes instanceof Uint8Array) || bytes.length === 0) {
      this.#decoder.finish();
      throw new Error("lsp.server_closed: language server stream ended");
    }
    await this.receiveBytes(bytes);
  }

  async response(id) {
    while (!this.#completed.has(id)) {
      if (!this.#pending.has(id)) throw new Error("lsp.request_cancelled: request is no longer pending");
      await this.pump();
    }
    const completed = this.#completed.get(id);
    this.#completed.delete(id);
    if (completed.error) throw completed.error;
    return completed.result;
  }

  async request(method, params) {
    const id = await this.sendRequest(method, params);
    return this.response(id);
  }

  async initialize({ processId = null, clientInfo = { name: "Clay" }, rootPath, initializationOptions = null, capabilities = {} }) {
    if (this.state !== "created") throw new Error("lsp.invalid_state: initialize is only valid once");
    const rootUri = rootPathToFileUri(rootPath);
    const result = await this.request("initialize", {
      processId,
      clientInfo,
      rootUri,
      workspaceFolders: [{ uri: rootUri, name: rootPath.split("/").filter(Boolean).at(-1) ?? "/" }],
      initializationOptions,
      capabilities: {
        general: { positionEncodings: ["utf-8", "utf-16", "utf-32"] },
        ...capabilities,
      },
    });
    this.capabilities = parseCapabilities(result);
    await this.notify("initialized", {});
    this.state = "running";
    return this.capabilities;
  }

  async openDocument({ documentId, rootPath, relativePath, languageId, version, text }) {
    if (this.state !== "running") throw new Error("lsp.invalid_state: client is not running");
    if (this.#documents.has(documentId)) throw new Error("lsp.invalid_document: document already open");
    const uri = pathToFileUri(rootPath, relativePath);
    const document = new VersionedDocument(text, version, this.capabilities.positionEncoding);
    this.#documents.set(documentId, { uri, document });
    if (this.capabilities.textDocumentOpenClose) {
      await this.notify("textDocument/didOpen", { textDocument: { uri, languageId, version, text } });
    }
    return uri;
  }

  async changeDocument({ documentId, baseVersion, version, byteStart, byteEnd, insertedText }) {
    const entry = this.#documents.get(documentId);
    if (!entry) throw new Error("lsp.invalid_document: document is not open");
    const range = entry.document.applyByteChange({ baseVersion, version, byteStart, byteEnd, text: insertedText });
    if (this.capabilities.textDocumentSync === 0) return;
    const contentChanges = this.capabilities.textDocumentSync === 1
      ? [{ text: entry.document.text }]
      : [{ range, text: insertedText }];
    await this.notify("textDocument/didChange", { textDocument: { uri: entry.uri, version }, contentChanges });
  }

  async resetDocument({ documentId, version, text }) {
    const entry = this.#documents.get(documentId);
    if (!entry || !Number.isInteger(version) || version <= entry.document.version) {
      throw new Error("lsp.stale_document: reset requires newer open document version");
    }
    entry.document.reset(text, version);
    if (this.capabilities.textDocumentSync !== 0) {
      await this.notify("textDocument/didChange", {
        textDocument: { uri: entry.uri, version },
        contentChanges: [{ text }],
      });
    }
  }

  async closeDocument(documentId) {
    const entry = this.#documents.get(documentId);
    if (!entry) return false;
    this.#documents.delete(documentId);
    if (this.capabilities.textDocumentOpenClose) {
      await this.notify("textDocument/didClose", { textDocument: { uri: entry.uri } });
    }
    return true;
  }

  document(documentId, version) {
    const entry = this.#documents.get(documentId);
    if (!entry || entry.document.version !== version) throw new Error("lsp.stale_document: exact open document version required");
    return entry;
  }

  async shutdown() {
    if (this.state === "stopped") return;
    try {
      if (this.state === "running") {
        await this.request("shutdown", null);
        await this.notify("exit");
      }
    } finally {
      this.state = "stopped";
      this.#pending.clear();
      this.#completed.clear();
      this.#documents.clear();
      await this.session.stop();
    }
  }
}
