import assert from "node:assert/strict";
import fs from "node:fs";
import test from "node:test";

import { lspRustPackageManifest } from "./dist/index.js";
import { createRustAnalyzerBridge } from "./dist/server.js";
import { encodeFrame, FrameDecoder } from "./dist/shared/framing.js";

const identity = {
  package: "@clay/lsp-rust",
  packageVersion: "0.1.0",
  packagePrefix: "lsp-rust",
  analyzerId: "lsp-rust.bridge",
  contribution: "lsp-rust.server",
};

class FakeRustAnalyzerSession {
  constructor() {
    this.decoder = new FrameDecoder();
    this.reads = [];
    this.messages = [];
    this.stopped = false;
    this.uri = "file:///workspace/src/main.rs";
    this.semanticRequests = 0;
  }

  queue(message) {
    this.reads.push(encodeFrame(message));
  }

  async sendBytes(bytes) {
    for (const message of this.decoder.push(bytes)) {
      this.messages.push(message);
      if (!("id" in message)) continue;
      const respond = (result) => this.queue({ jsonrpc: "2.0", id: message.id, result });
      if (message.method === "initialize") respond({ capabilities: {
        positionEncoding: "utf-8",
        textDocumentSync: { openClose: true, change: 2 },
        completionProvider: { triggerCharacters: [":", ".", "'", "("] },
        hoverProvider: true,
        definitionProvider: true,
        codeActionProvider: true,
        signatureHelpProvider: { triggerCharacters: ["(", ",", "<"] },
        semanticTokensProvider: {
          full: { delta: true },
          legend: { tokenTypes: ["function"], tokenModifiers: ["declaration"] },
        },
        diagnosticProvider: { identifier: "rust-analyzer" },
      } });
      else if (message.method === "textDocument/semanticTokens/full") {
        this.semanticRequests += 1;
        this.queue({
          jsonrpc: "2.0",
          method: "textDocument/publishDiagnostics",
          params: { uri: this.uri, version: 0, diagnostics: [{ range: range(0, 2), severity: 1, message: "stale" }] },
        });
        respond({ resultId: "semantic-1", data: [0, 0, 2, 0, 1] });
      } else if (message.method === "textDocument/semanticTokens/full/delta") {
        this.semanticRequests += 1;
        respond({ resultId: "semantic-2", edits: [{ start: 0, deleteCount: 5, data: [0, 0, 2, 0, 1] }] });
      } else if (message.method === "textDocument/diagnostic") {
        respond(message.params.previousResultId
          ? { kind: "unchanged", resultId: "diagnostic-1" }
          : { kind: "full", resultId: "diagnostic-1", items: [{ range: range(0, 2), severity: 2, code: "fake", source: "rust-analyzer", message: "warning" }] });
      } else if (message.method === "textDocument/completion") {
        respond({ items: [
          { label: "println!", insertText: "println!(\"${1}\")", insertTextFormat: 2, detail: "macro" },
          { label: "mutating", additionalTextEdits: [{ range: range(0, 0), newText: "use x;" }] },
        ] });
      } else if (message.method === "textDocument/hover") {
        respond({ contents: { kind: "markdown", value: "```rust\nfn main()\n```<script>bad()</script>" }, range: range(0, 2) });
      } else if (message.method === "textDocument/definition") {
        respond([{ uri: this.uri, range: range(0, 2) }, { uri: "file:///outside/lib.rs", range: range(0, 1) }]);
      } else if (message.method === "textDocument/codeAction") {
        respond([{ title: "Apply edit", edit: { changes: {} } }, { title: "Explain only" }]);
      } else if (message.method === "textDocument/signatureHelp") {
        respond({ signatures: [{ label: "drop(value: T)", parameters: [{ label: [5, 13], documentation: "value" }] }], activeSignature: 0, activeParameter: 0 });
      } else if (message.method === "shutdown") respond(null);
      else throw new Error(`unexpected request ${message.method}`);
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

function openEvent() {
  return {
    kind: "open",
    identity,
    documentId: 7,
    documentVersion: 1,
    runtimeGeneration: 1,
    activeMode: "rust",
    workspaceRootId: 1,
    canonicalRootPath: "/workspace",
    relativePath: "src/main.rs",
    text: "fn main() {}\n",
  };
}

function requestEvent(feature, requestId) {
  return {
    kind: "languageIntelligence",
    identity,
    request: {
      requestId,
      documentId: 7,
      documentVersion: 2,
      cursorByteOffset: 1,
      feature,
    },
    window: {},
  };
}

test("package manifest source matches package.json and fixes rustup launch authority", () => {
  assert.deepEqual(lspRustPackageManifest(), JSON.parse(fs.readFileSync(new URL("./package.json", import.meta.url))));
  const manifest = lspRustPackageManifest();
  assert.deepEqual(manifest.clay.capabilities, ["language-server"]);
  assert.deepEqual(manifest.clay.contributions.languageServers, [{
    id: "lsp-rust.server",
    executable: "rustup",
    args: ["run", "stable", "rust-analyzer"],
    inheritEnvironment: ["HOME", "PATH"],
  }]);
  assert.equal(manifest.clay.contributions.completionProviders[0].priority, 100);
  assert.equal(manifest.clay.contributions.completionProviders[0].exclusive, undefined);
});

test("Rust bridge maps negotiated features, drops stale/mutating/external output, and shuts down", async () => {
  const session = new FakeRustAnalyzerSession();
  const decorations = [];
  const diagnostics = [];
  const bridge = createRustAnalyzerBridge({
    startSession: async () => session,
    publishDecorations: (value) => decorations.push(value),
    publishDiagnostics: (value) => diagnostics.push(value),
    packageManifest: lspRustPackageManifest(),
  });

  await bridge.handle(openEvent());
  assert.equal(decorations.length, 1);
  assert.equal(decorations[0].spans[0].tokenType, "Function");
  assert.deepEqual(decorations[0].spans[0].modifiers, ["Declaration"]);
  assert.equal(diagnostics.length, 1, "stale pushed diagnostics must be ignored");
  assert.equal(diagnostics[0].spans[0].message, "warning");

  await bridge.handle({
    kind: "change",
    identity,
    documentId: 7,
    baseVersion: 1,
    documentVersion: 2,
    byteStart: 12,
    byteEnd: 12,
    insertedText: " ",
  });
  assert.equal(session.semanticRequests, 2);
  assert.equal(decorations.at(-1).documentVersion, 2);
  assert.equal(diagnostics.length, 1, "unchanged pull report must not republish");

  const completion = await bridge.handle({
    kind: "completion",
    identity,
    request: { documentId: 7, documentVersion: 2, cursorByteOffset: 2, trigger: { kind: "manual" } },
    window: {},
  });
  assert.equal(completion.items.length, 1);
  assert.equal(completion.items[0].textFormat, "snippet");

  const hover = await bridge.handle(requestEvent("hover", 1));
  assert.match(hover.hover.markdown, /fn main/);
  assert.doesNotMatch(hover.hover.markdown, /script|bad/);
  const definition = await bridge.handle(requestEvent("definition", 2));
  assert.deepEqual(definition.definition.locations, [{ documentId: 7, range: { byteStart: 0, byteEnd: 2 } }]);
  const actions = await bridge.handle(requestEvent("codeAction", 3));
  assert.deepEqual(actions.codeAction.actions, [{ title: "Explain only" }]);
  const signatures = await bridge.handle(requestEvent("signatureHelp", 4));
  assert.equal(signatures.signatureHelp.signatures[0].parameters[0].label, "value: T");

  await bridge.handle({ kind: "close", identity, documentId: 7, documentVersion: 2 });
  await bridge.handle({ kind: "shutdown", identity });
  assert.equal(session.stopped, true);
  assert.deepEqual(session.messages.slice(-3).map((message) => message.method), ["textDocument/didClose", "shutdown", "exit"]);
});

test("bridge rejects forged analyzer identity before session start", async () => {
  let started = false;
  const bridge = createRustAnalyzerBridge({
    startSession: async () => { started = true; },
    publishDecorations() {},
    publishDiagnostics() {},
    packageManifest: lspRustPackageManifest(),
  });
  await assert.rejects(() => bridge.handle({ ...openEvent(), identity: { ...identity, package: "@evil/pkg" } }), /invalid_identity/);
  assert.equal(started, false);
});
