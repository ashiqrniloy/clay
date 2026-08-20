import test from "node:test";
import assert from "node:assert/strict";
import { FrameDecoder, encodeFrame } from "./framing.js";
import { decodeUtf8, encodeUtf8 } from "./utf8.js";
import { LspClient, LspResponseError } from "./client.js";
import {
  applySemanticTokenDelta,
  codeActionsToClay,
  completionToClay,
  definitionToClay,
  diagnosticsToClay,
  hoverToClay,
  inlayHintsToClay,
  parseCapabilities,
  semanticTokensToClay,
  signatureHelpToClay,
} from "./mapping.js";
import { fileUriToRelative, pathToFileUri, VersionedDocument } from "./positions.js";
import {
  DEFAULT_TOKEN_MODIFIERS,
  DEFAULT_TOKEN_TYPES,
  createLspBridge,
  lspClientCapabilities,
} from "./bridge.js";

const bytes = (text) => new TextEncoder().encode(text);

class FakeSession {
  constructor(responseFor) {
    this.decoder = new FrameDecoder();
    this.sent = [];
    this.reads = [];
    this.responseFor = responseFor;
    this.stopped = false;
  }

  async sendBytes(chunk) {
    for (const message of this.decoder.push(chunk)) {
      this.sent.push(message);
      const response = this.responseFor?.(message);
      if (response) {
        const frame = encodeFrame(response);
        for (let index = 0; index < frame.length; index += 3) this.reads.push(frame.slice(index, index + 3));
      }
    }
  }

  async readBytes() {
    return this.reads.shift() ?? new Uint8Array();
  }

  async stop() {
    this.stopped = true;
  }
}

function initializeResponse(id, extra = {}) {
  return {
    jsonrpc: "2.0",
    id,
    result: {
      capabilities: {
        textDocumentSync: 2,
        completionProvider: { triggerCharacters: ["."] },
        hoverProvider: true,
        definitionProvider: true,
        codeActionProvider: true,
        signatureHelpProvider: { triggerCharacters: ["("] },
        semanticTokensProvider: {
          legend: { tokenTypes: ["function"], tokenModifiers: ["declaration"] },
          full: { delta: true },
        },
        ...extra,
      },
    },
  };
}

test("dependency-free UTF-8 codec round-trips scalars and rejects malformed input", () => {
  assert.equal(decodeUtf8(encodeUtf8("ASCII 🦀 中文")), "ASCII 🦀 中文");
  assert.throws(() => encodeUtf8("\ud800"), /unpaired surrogate/);
  for (const input of [
    [0x80], [0xc0, 0x80], [0xe0, 0x80, 0x80],
    [0xed, 0xa0, 0x80], [0xf4, 0x90, 0x80, 0x80], [0xf0, 0x9f],
  ]) assert.throws(() => decodeUtf8(Uint8Array.from(input)), /invalid_utf8/);
});

test("framing preserves fragmented UTF-8 and coalesced messages", () => {
  const first = encodeFrame({ jsonrpc: "2.0", method: "note", params: "🦀" });
  const second = encodeFrame({ jsonrpc: "2.0", id: 1, result: [] });
  const stream = new Uint8Array(first.length + second.length);
  stream.set(first);
  stream.set(second, first.length);
  const coalesced = new FrameDecoder();
  assert.equal(coalesced.push(stream).length, 2);
  coalesced.finish();
  const fragmented = new FrameDecoder();
  const messages = [];
  for (let index = 0; index < stream.length; index += 1) messages.push(...fragmented.push(stream.slice(index, index + 1)));
  fragmented.finish();
  assert.equal(messages[0].params, "🦀");
  assert.deepEqual(messages[1].result, []);
});

test("framing rejects malformed, duplicate, oversize, invalid JSON, and truncated frames", () => {
  for (const frame of [
    "X: 2\r\n\r\n{}",
    "Content-Length: 2\r\nContent-Length: 2\r\n\r\n{}",
    "Content-Length: 11x\r\n\r\n{}",
    "Content-Length: 2\r\nContent-Type: application/vscode-jsonrpc\r\nContent-Type: application/vscode-jsonrpc\r\n\r\n{}",
    "Content-Length: 1048577\r\n\r\n",
    "Content-Length: 1\r\n\r\n{",
  ]) assert.throws(() => new FrameDecoder().push(bytes(frame)), /lsp\./);
  const malformedJson = new FrameDecoder();
  assert.throws(() => malformedJson.push(bytes("Content-Length: 2\r\n\r\n{x")), /invalid_json/);
  const truncated = new FrameDecoder();
  truncated.push(bytes("Content-Length: 4\r\n\r\n{}"));
  assert.throws(() => truncated.finish(), /truncated_frame/);
  assert.throws(() => encodeFrame({ text: "x".repeat(1024 * 1024) }), /frame_too_large/);
  assert.throws(() => encodeFrame(null), /invalid_message/);
});

test("positions convert UTF-8, UTF-16, UTF-32, CRLF, and reject split scalars", () => {
  const text = "a🦀b\r\nç";
  const utf16 = new VersionedDocument(text, 1, "utf-16");
  assert.deepEqual(utf16.byteToPosition(5), { line: 0, character: 3 });
  assert.equal(utf16.positionToByte({ line: 0, character: 3 }), 5);
  assert.equal(utf16.positionToByte({ line: 1, character: 1 }), bytes(text).length);
  assert.throws(() => utf16.positionToByte({ line: 0, character: 2 }), /surrogate/);
  assert.throws(() => utf16.byteToPosition(2), /UTF-8/);
  assert.throws(() => utf16.byteToPosition(7), /line ending/);
  const utf8 = new VersionedDocument(text, 1, "utf-8");
  assert.equal(utf8.positionToByte({ line: 0, character: 5 }), 5);
  const utf32 = new VersionedDocument(text, 1, "utf-32");
  assert.equal(utf32.positionToByte({ line: 0, character: 2 }), 5);
  const range = utf16.applyByteChange({ baseVersion: 1, version: 2, byteStart: 1, byteEnd: 5, text: "x" });
  assert.deepEqual(range, { start: { line: 0, character: 1 }, end: { line: 0, character: 3 } });
  assert.equal(utf16.text, "axb\r\nç");
  assert.throws(() => utf16.applyByteChange({ baseVersion: 1, version: 3, byteStart: 0, byteEnd: 0, text: "" }), /stale/);
});

test("file URIs stay within canonical root", () => {
  const uri = pathToFileUri("/tmp/a b", "src/🦀.rs");
  assert.equal(uri, "file:///tmp/a%20b/src/%F0%9F%A6%80.rs");
  assert.equal(fileUriToRelative(uri, "/tmp/a b"), "src/🦀.rs");
  for (const external of [
    "https://example.com/a.rs",
    "file:///tmp/other/a.rs",
    "file://host/tmp/a%20b/a.rs",
    "file:///tmp/a%20b/a%2Fb.rs",
    "file:///tmp/a%20b/a.rs?query",
    "file:///tmp/a%20b/%2e%2e/secret",
  ]) assert.throws(() => fileUriToRelative(external, "/tmp/a b"), /invalid_uri|out_of_root/);
  assert.throws(() => pathToFileUri("/tmp/root", "../secret"), /invalid_path/);
});

test("capabilities default to UTF-16 and preserve advertised feature absence", () => {
  const capabilities = parseCapabilities(initializeResponse(1).result);
  assert.equal(capabilities.positionEncoding, "utf-16");
  assert.equal(capabilities.textDocumentSync, 2);
  assert.equal(capabilities.semanticTokens, true);
  assert.deepEqual(capabilities.completionTriggerCharacters, ["."]);
  const empty = parseCapabilities({ capabilities: {} });
  assert.equal(empty.completion, false);
  assert.equal(empty.textDocumentSync, 0);
  assert.throws(() => parseCapabilities({ capabilities: { positionEncoding: "utf-7" } }), /encoding/);
});

test("all seven feature mappings are bounded, inert, and byte based", () => {
  const document = new VersionedDocument("fn 🦀(x)\n", 4, "utf-16");
  const legend = { tokenTypes: ["function"], tokenModifiers: ["declaration"] };
  assert.deepEqual(semanticTokensToClay([0, 0, 2, 0, 1], legend, document)[0], {
    byteStart: 0,
    byteEnd: 2,
    kind: "semantic",
    tokenType: "Function",
    modifiers: ["Declaration"],
    priority: 100,
  });
  assert.deepEqual(applySemanticTokenDelta([0, 0, 2, 0, 1], [{ start: 2, deleteCount: 1, data: [3] }]), [0, 0, 3, 0, 1]);
  assert.equal(diagnosticsToClay([{
    range: { start: { line: 0, character: 3 }, end: { line: 0, character: 5 } },
    severity: 1,
    code: "E1",
    message: "bad",
  }], document)[0].byteEnd, 7);
  assert.deepEqual(completionToClay([{ label: "print", insertText: "print(${1:x})", insertTextFormat: 2 }]).items[0].textFormat, "snippet");
  assert.throws(() => completionToClay([{ label: "x", additionalTextEdits: [] }]), /mutating/);
  const hover = hoverToClay({ contents: { kind: "markdown", value: "ok<script>alert(1)</script><b>bold</b>" } }, document);
  assert.equal(hover.hover.markdown, "okbold");
  const definition = definitionToClay({ uri: "file:///tmp/root/a.rs", range: { start: { line: 0, character: 0 }, end: { line: 0, character: 2 } } }, ({ uri, range }) => ({
    workspaceRootId: 9,
    relativePath: fileUriToRelative(uri, "/tmp/root"),
    range: document.rangeToBytes(range),
  }));
  assert.equal(definition.definition.locations[0].relativePath, "a.rs");
  const actions = codeActionsToClay([
    { title: "unsafe", edit: { changes: {} } },
    { title: "unknown", command: { command: "server.unknown" } },
    { title: "safe", command: { command: "server.safe" } },
  ], new Map([["server.safe", "lsp.safe"]]));
  assert.deepEqual(actions.codeAction.actions, [{ title: "safe", commandId: "lsp.safe" }]);
  const signatures = signatureHelpToClay({ signatures: [{ label: "f(x)", parameters: [{ label: "x", documentation: "arg" }] }], activeSignature: 0, activeParameter: 0 });
  assert.equal(signatures.signatureHelp.signatures[0].parameters[0].label, "x");
});

test("mapping budgets reject oversized collections and cap inert markdown", () => {
  const document = new VersionedDocument("x", 1);
  assert.throws(() => semanticTokensToClay(new Array(129 * 5).fill(0), { tokenTypes: ["variable"], tokenModifiers: [] }, document), /semantic_tokens/);
  assert.throws(() => diagnosticsToClay(new Array(129).fill({}), document), /diagnostics_too_large/);
  assert.throws(() => completionToClay(new Array(257).fill({ label: "x" })), /completions_too_large/);
  const range = { start: { line: 0, character: 0 }, end: { line: 0, character: 1 } };
  assert.throws(
    () => diagnosticsToClay(new Array(3).fill({ range, message: "x".repeat(4096) }), document),
    /mapped payload/,
  );
  assert.throws(
    () => completionToClay(new Array(5).fill({ label: "x", insertText: "x".repeat(4096) })),
    /mapped payload/,
  );
  assert.equal(hoverToClay({ contents: "x".repeat(5000) }, document).hover.markdown.length, 4096);
  assert.throws(() => signatureHelpToClay({ signatures: [{ label: "f()", parameters: [] }], activeSignature: 2 }), /active signature/);
  assert.throws(() => applySemanticTokenDelta([0], [{ start: 2, deleteCount: 0 }]), /exceeds/);
});

test("client enforces initialize, incremental sync, close, shutdown order", async () => {
  const session = new FakeSession((message) => {
    if (message.method === "initialize") return initializeResponse(message.id);
    if (message.method === "shutdown") return { jsonrpc: "2.0", id: message.id, result: null };
    return null;
  });
  const client = new LspClient(session);
  await assert.rejects(() => client.openDocument({}), /not running/);
  const capabilities = await client.initialize({ rootPath: "/tmp/root" });
  assert.equal(capabilities.positionEncoding, "utf-16");
  await client.openDocument({ documentId: 7, rootPath: "/tmp/root", relativePath: "a.rs", languageId: "rust", version: 1, text: "a🦀" });
  await client.changeDocument({ documentId: 7, baseVersion: 1, version: 2, byteStart: 1, byteEnd: 5, insertedText: "x" });
  assert.deepEqual(session.sent.find((message) => message.method === "textDocument/didChange").params.contentChanges[0].range, {
    start: { line: 0, character: 1 },
    end: { line: 0, character: 3 },
  });
  assert.equal(client.document(7, 2).document.text, "ax");
  assert.throws(() => client.document(7, 1), /stale/);
  await client.resetDocument({ documentId: 7, version: 3, text: "reset" });
  assert.deepEqual(
    session.sent.filter((message) => message.method === "textDocument/didChange")[1].params.contentChanges,
    [{ text: "reset" }],
  );
  await client.closeDocument(7);
  await client.shutdown();
  assert.deepEqual(session.sent.map((message) => message.method), [
    "initialize",
    "initialized",
    "textDocument/didOpen",
    "textDocument/didChange",
    "textDocument/didChange",
    "textDocument/didClose",
    "shutdown",
    "exit",
  ]);
  assert.equal(session.stopped, true);
});

test("shutdown stops session even when server never replies", async () => {
  const session = new FakeSession((message) => message.method === "initialize" ? initializeResponse(message.id) : null);
  const client = new LspClient(session);
  await client.initialize({ rootPath: "/tmp/root" });
  await assert.rejects(() => client.shutdown(), /server_closed/);
  assert.equal(client.state, "stopped");
  assert.equal(session.stopped, true);
});

test("client cancellation ignores late response and ContentModified stays typed", async () => {
  const session = new FakeSession((message) => message.method === "stale"
    ? { jsonrpc: "2.0", id: message.id, error: { code: -32801, message: "changed" } }
    : null);
  const client = new LspClient(session);
  const cancelled = await client.sendRequest("slow", {});
  assert.equal(await client.cancelRequest(cancelled), true);
  await client.receiveBytes(encodeFrame({ jsonrpc: "2.0", id: cancelled, result: "late" }));
  const stale = await client.sendRequest("stale", {});
  await assert.rejects(() => client.response(stale), (error) => error instanceof LspResponseError && error.contentModified);
  assert.equal(session.sent.some((message) => message.method === "$/cancelRequest"), true);
  await assert.rejects(
    () => client.receiveBytes(encodeFrame({ jsonrpc: "2.0", id: 999, result: null })),
    /unknown_response/,
  );
});

test("pending request count and malformed JSON-RPC fail closed", async () => {
  const client = new LspClient(new FakeSession());
  for (let index = 0; index < 8; index += 1) await client.sendRequest(`pending/${index}`, {});
  await assert.rejects(() => client.sendRequest("pending/overflow", {}), /too_many_requests/);
  await assert.rejects(
    () => client.receiveBytes(encodeFrame({ jsonrpc: "1.0", method: "bad" })),
    /jsonrpc/,
  );
  await assert.rejects(
    () => client.receiveBytes(encodeFrame({ jsonrpc: "2.0", id: 1 })),
    /malformed/,
  );
  await assert.rejects(
    () => client.receiveBytes(encodeFrame({ jsonrpc: "2.0", id: 1, error: "bad" })),
    /malformed/,
  );
});

test("empty child read reports deterministic server exit", async () => {
  await assert.rejects(() => new LspClient(new FakeSession()).pump(), /server_closed/);
});

test("factory token tables and diagnostic mode match rust/markdown defaults", () => {
  assert.deepEqual([...DEFAULT_TOKEN_TYPES], [
    "namespace", "type", "class", "enum", "interface", "struct", "typeParameter", "parameter",
    "variable", "property", "enumMember", "event", "function", "method", "macro", "keyword",
    "modifier", "comment", "string", "number", "regexp", "operator", "decorator",
  ]);
  assert.deepEqual([...DEFAULT_TOKEN_MODIFIERS], [
    "declaration", "definition", "readonly", "static", "deprecated", "abstract", "async",
    "modification", "documentation", "defaultLibrary",
  ]);
  const pull = lspClientCapabilities({ diagnostics: "pull" });
  assert.deepEqual(pull.textDocument.diagnostic, {
    dynamicRegistration: false,
    relatedDocumentSupport: false,
  });
  assert.deepEqual(pull.textDocument.semanticTokens.tokenTypes, [...DEFAULT_TOKEN_TYPES]);
  const push = lspClientCapabilities({
    diagnostics: "push",
    features: ["completion", "hover", "definition", "codeAction"],
  });
  assert.equal(push.textDocument.diagnostic, undefined);
  assert.equal(push.textDocument.signatureHelp, undefined);
  assert.equal(push.textDocument.inlayHint, undefined);
  const withInlay = lspClientCapabilities({ features: ["inlayHint"] });
  assert.deepEqual(withInlay.textDocument.inlayHint, { dynamicRegistration: false });
  const omitted = lspClientCapabilities();
  assert.equal(omitted.textDocument.inlayHint, undefined);
});

test("createLspBridge advertises inlayHint only when enabled", () => {
  const off = lspClientCapabilities({ features: ["hover"] });
  assert.equal(off.textDocument.inlayHint, undefined);
  const on = lspClientCapabilities({ features: ["hover", "inlayHint"] });
  assert.equal(on.textDocument.inlayHint.dynamicRegistration, false);
});

test("inlay maps to decoration kind not syntax", () => {
  const document = new VersionedDocument("fn main(x: i32) {}", 1, "utf-8");
  const spans = inlayHintsToClay([
    { position: { line: 0, character: 8 }, label: "x:", kind: 2 },
    { position: { line: 0, character: 11 }, label: [{ value: ": " }, { value: "i32" }], kind: 1 },
    { position: { line: 0, character: 0 }, label: "\u0007", kind: 1 },
  ], document);
  assert.equal(spans.length, 2);
  assert.equal(spans[0].kind, "inlayHint");
  assert.equal(spans[0].inlay.placement, "before");
  assert.equal(spans[0].tokenType, "Parameter");
  assert.equal(spans[1].kind, "inlayHint");
  assert.equal(spans[1].inlay.label, ": i32");
  assert.equal(spans[1].inlay.placement, "after");
});

test("factory rejects forged identity before session start", async () => {
  let started = false;
  const bridge = createLspBridge({
    packageName: "@clay/lsp-rust",
    contribution: "lsp-rust.server",
    diagnosticSource: "rust-analyzer",
    languageId: "rust",
    diagnostics: "pull",
    startSession: async () => { started = true; },
    publishDecorations() {},
    publishDiagnostics() {},
    packageManifest: {},
  });
  await assert.rejects(
    () => bridge.handle({
      kind: "open",
      identity: { package: "@evil/pkg", contribution: "lsp-rust.server" },
      documentId: 1,
      documentVersion: 1,
      relativePath: "src/main.rs",
      text: "",
    }),
    /invalid_identity/,
  );
  assert.equal(started, false);
});

test("language-server session start uses host-stamped options only", async () => {
  let received;
  const bridge = createLspBridge({
    packageName: "@clay/lsp-rust",
    contribution: "lsp-rust.server",
    diagnosticSource: "rust-analyzer",
    languageId: "rust",
    diagnostics: "pull",
    startSession: async (options) => {
      received = options;
      throw new Error("stop after capturing session options");
    },
    publishDecorations() {},
    publishDiagnostics() {},
    packageManifest: {},
  });
  await assert.rejects(
    () => bridge.handle({
      kind: "open",
      identity: { package: "@clay/lsp-rust", contribution: "lsp-rust.server" },
      documentId: 1,
      documentVersion: 1,
      workspaceRootId: 7,
      canonicalRootPath: "/workspace",
      relativePath: "src/main.rs",
      text: "fn main() {}\n",
    }),
    /stop after capturing session options/,
  );
  assert.deepEqual(received, { contribution: "lsp-rust.server", workspaceRootId: 7 });
});

test("server requests require explicit allowlist", async () => {
  const session = new FakeSession();
  const client = new LspClient(session, {
    serverRequestHandlers: new Map([["workspace/configuration", () => []]]),
  });
  await client.receiveBytes(encodeFrame({ jsonrpc: "2.0", id: 1, method: "workspace/configuration", params: {} }));
  await client.receiveBytes(encodeFrame({ jsonrpc: "2.0", id: 2, method: "workspace/applyEdit", params: {} }));
  assert.deepEqual(session.sent[0], { jsonrpc: "2.0", id: 1, result: [] });
  assert.equal(session.sent[1].error.code, -32601);
});
