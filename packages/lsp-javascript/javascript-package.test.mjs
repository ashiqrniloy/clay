import assert from "node:assert/strict";
import fs from "node:fs";
import test from "node:test";

import { lspJavascriptPackageManifest } from "./dist/index.js";
import { createJavascriptBridge } from "./dist/server.js";
import { encodeFrame, FrameDecoder } from "./dist/shared/framing.js";
import { languageIdForRelativePath } from "./dist/shared/typescript-language-server.js";

const identity = {
  package: "@clay/lsp-javascript",
  packageVersion: "0.1.0",
  packagePrefix: "lsp-javascript",
  analyzerId: "lsp-javascript.bridge",
  contribution: "lsp-javascript.server",
};

class FakeJavascriptSession {
  constructor() {
    this.decoder = new FrameDecoder();
    this.reads = [];
    this.messages = [];
    this.stopped = false;
    this.uri = "file:///workspace/src/main.js";
  }

  queue(message) {
    this.reads.push(encodeFrame(message));
  }

  async sendBytes(bytes) {
    for (const message of this.decoder.push(bytes)) {
      this.messages.push(message);
      if (!("id" in message)) continue;
      const respond = (result) => this.queue({ jsonrpc: "2.0", id: message.id, result });
      if (message.method === "initialize") {
        respond({
          capabilities: {
            textDocumentSync: { openClose: true, change: 2 },
            completionProvider: { triggerCharacters: [".", "\"", "'", "/", "@", "<"] },
            hoverProvider: true,
            definitionProvider: true,
            codeActionProvider: true,
            signatureHelpProvider: { triggerCharacters: ["(", ",", "<"] },
            semanticTokensProvider: {
              full: true,
              legend: { tokenTypes: ["function"], tokenModifiers: ["declaration"] },
            },
          },
        });
      } else if (message.method === "textDocument/semanticTokens/full") {
        const uri = message.params?.textDocument?.uri ?? this.uri;
        this.uri = uri;
        this.queue({
          jsonrpc: "2.0",
          method: "textDocument/publishDiagnostics",
          params: {
            uri,
            version: 1,
            diagnostics: [{
              range: range(0, 6),
              severity: 2,
              message: "JSDoc type warning",
              source: "js",
            }],
          },
        });
        respond({ data: [0, 0, 6, 0, 1] });
      } else if (message.method === "textDocument/completion") {
        respond([{ label: "answer", insertText: "answer()", insertTextFormat: 2 }]);
      } else if (message.method === "textDocument/hover") {
        respond({ contents: { kind: "markdown", value: "```js\nfunction answer()\n```" } });
      } else if (message.method === "textDocument/definition") {
        respond({ uri: this.uri, range: range(0, 6) });
      } else if (message.method === "textDocument/codeAction") {
        respond([{ title: "Explain only" }]);
      } else if (message.method === "textDocument/signatureHelp") {
        respond({
          signatures: [{ label: "greet(name)", parameters: [{ label: "name" }] }],
          activeSignature: 0,
          activeParameter: 0,
        });
      } else if (message.method === "shutdown") {
        respond(null);
      } else {
        throw new Error(`unexpected request ${message.method}`);
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

function range(start, end) {
  return { start: { line: 0, character: start }, end: { line: 0, character: end } };
}

test("JavaScript package manifest matches package.json and reuses typescript-language-server launch contract", () => {
  assert.deepEqual(lspJavascriptPackageManifest(), JSON.parse(fs.readFileSync(new URL("./package.json", import.meta.url))));
  const manifest = lspJavascriptPackageManifest();
  assert.deepEqual(manifest.clay.contributions.languageServers, [{
    id: "lsp-javascript.server",
    executable: "typescript-language-server",
    args: ["--stdio"],
    inheritEnvironment: ["HOME", "PATH"],
  }]);
  assert.deepEqual(manifest.clay.contributions.languageIntelligenceProviders[0].modes, ["javascript"]);
  assert.notEqual(manifest.clay.apiPrefix, "lsp-typescript");
});

test("JavaScript bridge stays mode-separated and maps JSX module extensions", async () => {
  const session = new FakeJavascriptSession();
  const decorations = [];
  const diagnostics = [];
  const bridge = createJavascriptBridge({
    startSession: async () => session,
    publishDecorations: (value) => decorations.push(value),
    publishDiagnostics: (value) => diagnostics.push(value),
    packageManifest: lspJavascriptPackageManifest(),
  });

  await bridge.handle({
    kind: "open",
    identity,
    documentId: 3,
    documentVersion: 1,
    runtimeGeneration: 1,
    activeMode: "javascript",
    workspaceRootId: 1,
    canonicalRootPath: "/workspace",
    relativePath: "src/badge.jsx",
    text: "export const Badge = () => null\n",
  });
  const opened = session.messages.find((message) => message.method === "textDocument/didOpen");
  assert.equal(opened.params.textDocument.languageId, "javascriptreact");
  assert.equal(languageIdForRelativePath("src/main.mjs", ["javascript", "javascriptreact"]), "javascript");
  assert.equal(decorations[0].spans[0].tokenType, "Function");
  assert.equal(diagnostics[0].source, "lsp-javascript");

  const completion = await bridge.handle({
    kind: "completion",
    identity,
    request: { documentId: 3, documentVersion: 1, cursorByteOffset: 1, trigger: { kind: "manual" } },
    window: {},
  });
  assert.equal(completion.items[0].label, "answer");

  await bridge.handle({ kind: "close", identity, documentId: 3, documentVersion: 1 });
  await bridge.handle({ kind: "shutdown", identity });
  assert.equal(session.stopped, true);
  assert.throws(
    () => languageIdForRelativePath("src/main.ts", ["javascript", "javascriptreact"]),
    /unsupported_language/,
  );
});

test("JavaScript bridge rejects forged analyzer identity before session start", async () => {
  let started = false;
  const bridge = createJavascriptBridge({
    startSession: async () => { started = true; },
    publishDecorations() {},
    publishDiagnostics() {},
    packageManifest: lspJavascriptPackageManifest(),
  });
  await assert.rejects(() => bridge.handle({
    kind: "open",
    identity: { ...identity, contribution: "lsp-typescript.server" },
    documentId: 1,
    documentVersion: 1,
    workspaceRootId: 1,
    canonicalRootPath: "/workspace",
    relativePath: "src/main.js",
    text: " console.log(1)\n",
  }), /invalid_identity/);
  assert.equal(started, false);
});
