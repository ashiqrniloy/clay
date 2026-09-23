import assert from "node:assert/strict";
import fs from "node:fs";
import test from "node:test";

import { lspTypescriptPackageManifest } from "./dist/index.js";
import { createTypescriptBridge } from "./dist/server.js";
import { encodeFrame, FrameDecoder } from "../lsp-shared/framing.js";
import { languageIdForRelativePath } from "../lsp-shared/typescript-language-server.js";

const identity = {
  package: "@clay/lsp-typescript",
  packageVersion: "0.1.0",
  packagePrefix: "lsp-typescript",
  analyzerId: "lsp-typescript.bridge",
  contribution: "lsp-typescript.server",
};

class FakeTypescriptSession {
  constructor({ advertiseSemantic = true, advertiseSignature = true } = {}) {
    this.decoder = new FrameDecoder();
    this.reads = [];
    this.messages = [];
    this.stopped = false;
    this.uri = "file:///workspace/src/main.ts";
    this.advertiseSemantic = advertiseSemantic;
    this.advertiseSignature = advertiseSignature;
    this.openedLanguageId = null;
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
            completionProvider: { triggerCharacters: [".", "\"", "'", "/", "@", "<"], resolveProvider: true },
            hoverProvider: true,
            definitionProvider: true,
            codeActionProvider: true,
            ...(this.advertiseSignature
              ? { signatureHelpProvider: { triggerCharacters: ["(", ",", "<"], retriggerCharacters: [")"] } }
              : {}),
            ...(this.advertiseSemantic
              ? {
                semanticTokensProvider: {
                  full: true,
                  legend: { tokenTypes: ["function", "member"], tokenModifiers: ["declaration", "local"] },
                },
              }
              : {}),
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
              range: range(0, 8),
              severity: 1,
              code: 2322,
              source: "ts",
              message: "Type 'string' is not assignable to type 'number'.",
            }],
          },
        });
        respond({ data: [0, 0, 8, 0, 1] });
      } else if (message.method === "textDocument/completion") {
        respond({
          items: [
            { label: "answer", insertText: "answer()", insertTextFormat: 2, detail: "function" },
            { label: "mutating", additionalTextEdits: [{ range: range(0, 0), newText: "import x;" }] },
          ],
        });
      } else if (message.method === "textDocument/hover") {
        respond({
          contents: { kind: "markdown", value: "```ts\nfunction answer(): number\n```<script>bad()</script>" },
          range: range(0, 6),
        });
      } else if (message.method === "textDocument/definition") {
        respond([
          { uri: this.uri, range: range(0, 8) },
          { uri: "file:///outside/lib.ts", range: range(0, 1) },
        ]);
      } else if (message.method === "textDocument/codeAction") {
        respond([
          { title: "Apply edit", edit: { changes: {} } },
          { title: "Ignore only" },
        ]);
      } else if (message.method === "textDocument/signatureHelp") {
        respond({
          signatures: [{
            label: "greet(name: string): string",
            parameters: [{ label: [6, 18], documentation: "name" }],
          }],
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

function openEvent(relativePath = "src/main.ts", text = "function answer() { return 1 }\n") {
  return {
    kind: "open",
    identity,
    documentId: 7,
    documentVersion: 1,
    runtimeGeneration: 1,
    activeMode: "typescript",
    workspaceRootId: 1,
    canonicalRootPath: "/workspace",
    relativePath,
    text,
  };
}

function requestEvent(feature, requestId) {
  return {
    kind: "languageIntelligence",
    identity,
    request: {
      requestId,
      documentId: 7,
      documentVersion: 1,
      cursorByteOffset: 1,
      feature,
    },
    window: {},
  };
}

test("TypeScript package manifest matches package.json and fixes --stdio launch authority", () => {
  const manifest = JSON.parse(fs.readFileSync(new URL("./package.json", import.meta.url)));
  assert.equal(manifest.clay.preset, "lsp-bridge");
  assert.deepEqual(manifest.clay.capabilities, ["language-server"]);
  assert.deepEqual(manifest.clay.contributions.languageServers, [{
    id: "lsp-typescript.server",
    executable: "typescript-language-server",
    args: ["--stdio"],
    inheritEnvironment: ["HOME", "PATH"],
  }]);
  assert.deepEqual(manifest.clay.contributions.languageIntelligenceProviders[0].modes, ["typescript"]);
  assert.equal(manifest.clay.contributions.completionProviders[0].priority, 100);
  assert.equal(manifest.clay.contributions.completionProviders[0].exclusive, undefined);
});

test("TypeScript bridge maps negotiated features, extension language IDs, and drops stale/mutating output", async () => {
  const session = new FakeTypescriptSession();
  const decorations = [];
  const diagnostics = [];
  const bridge = createTypescriptBridge({
    startSession: async () => session,
    publishDecorations: (value) => decorations.push(value),
    publishDiagnostics: (value) => diagnostics.push(value),
    packageManifest: lspTypescriptPackageManifest(),
  });

  await bridge.handle(openEvent("src/badge.tsx", "export const Badge = () => null\n"));
  const opened = session.messages.find((message) => message.method === "textDocument/didOpen");
  assert.equal(opened.params.textDocument.languageId, "typescriptreact");
  assert.equal(languageIdForRelativePath("src/main.mts", ["typescript", "typescriptreact"]), "typescript");
  assert.equal(decorations.length, 1);
  assert.equal(decorations[0].spans[0].tokenType, "Function");
  assert.equal(diagnostics.length, 1);
  assert.equal(diagnostics[0].source, "lsp-typescript");
  assert.match(diagnostics[0].spans[0].message, /not assignable/);

  const completion = await bridge.handle({
    kind: "completion",
    identity,
    request: { documentId: 7, documentVersion: 1, cursorByteOffset: 2, trigger: { kind: "manual" } },
    window: {},
  });
  assert.equal(completion.items.length, 1);
  assert.equal(completion.items[0].textFormat, "snippet");

  const hover = await bridge.handle(requestEvent("hover", 1));
  assert.match(hover.hover.markdown, /answer/);
  assert.doesNotMatch(hover.hover.markdown, /script|bad/);
  const definition = await bridge.handle(requestEvent("definition", 2));
  assert.deepEqual(definition.definition.locations, [{ documentId: 7, range: { byteStart: 0, byteEnd: 8 } }]);
  const actions = await bridge.handle(requestEvent("codeAction", 3));
  assert.deepEqual(actions.codeAction.actions, [{ title: "Ignore only" }]);
  const signatures = await bridge.handle(requestEvent("signatureHelp", 4));
  assert.equal(signatures.signatureHelp.signatures[0].parameters[0].label, "name: string");

  await bridge.handle({ kind: "close", identity, documentId: 7, documentVersion: 1 });
  await bridge.handle({ kind: "shutdown", identity });
  assert.equal(session.stopped, true);
  assert.deepEqual(session.messages.slice(-3).map((message) => message.method), [
    "textDocument/didClose",
    "shutdown",
    "exit",
  ]);
});

test("TypeScript bridge leaves absent capabilities empty and rejects forged identity", async () => {
  const session = new FakeTypescriptSession({ advertiseSemantic: false, advertiseSignature: false });
  const decorations = [];
  const diagnostics = [];
  const bridge = createTypescriptBridge({
    startSession: async () => session,
    publishDecorations: (value) => decorations.push(value),
    publishDiagnostics: (value) => diagnostics.push(value),
    packageManifest: lspTypescriptPackageManifest(),
  });
  await bridge.handle(openEvent());
  assert.equal(decorations.length, 0);
  assert.equal(diagnostics.length, 0);
  const signatures = await bridge.handle(requestEvent("signatureHelp", 1));
  assert.equal(signatures.status, "empty");

  let started = false;
  const guarded = createTypescriptBridge({
    startSession: async () => { started = true; },
    publishDecorations() {},
    publishDiagnostics() {},
    packageManifest: lspTypescriptPackageManifest(),
  });
  await assert.rejects(
    () => guarded.handle({ ...openEvent(), identity: { ...identity, package: "@evil/pkg" } }),
    /invalid_identity/,
  );
  assert.equal(started, false);
  assert.throws(() => languageIdForRelativePath("src/main.js", ["typescript", "typescriptreact"]), /unsupported_language/);
});
