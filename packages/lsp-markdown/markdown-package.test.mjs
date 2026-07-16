import assert from "node:assert/strict";
import fs from "node:fs";
import test from "node:test";

import { lspMarkdownPackageManifest } from "./dist/index.js";
import { createMarksmanBridge, languageIdForRelativePath } from "./dist/server.js";
import { encodeFrame, FrameDecoder } from "./dist/shared/framing.js";

const identity = {
  package: "@clay/lsp-markdown",
  packageVersion: "0.1.0",
  packagePrefix: "lsp-markdown",
  analyzerId: "lsp-markdown.bridge",
  contribution: "lsp-markdown.server",
};

class FakeMarksmanSession {
  constructor({ advertiseSemantic = true, advertiseCodeAction = true } = {}) {
    this.decoder = new FrameDecoder();
    this.reads = [];
    this.messages = [];
    this.stopped = false;
    this.uri = "file:///workspace/README.md";
    this.advertiseSemantic = advertiseSemantic;
    this.advertiseCodeAction = advertiseCodeAction;
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
            textDocumentSync: { openClose: true, change: 1 },
            completionProvider: { triggerCharacters: ["[", "#", "("] },
            hoverProvider: true,
            definitionProvider: true,
            referencesProvider: true,
            renameProvider: true,
            ...(this.advertiseCodeAction
              ? { codeActionProvider: { resolveProvider: false } }
              : {}),
            ...(this.advertiseSemantic
              ? {
                semanticTokensProvider: {
                  full: { delta: false },
                  legend: { tokenTypes: ["class", "class", "enumMember"], tokenModifiers: [] },
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
              range: { start: { line: 2, character: 6 }, end: { line: 2, character: 11 } },
              severity: 1,
              code: "2",
              source: "Marksman",
              message: "Link to non-existent document",
            }],
          },
        });
        respond({ data: [2, 6, 5, 0, 0] });
      } else if (message.method === "textDocument/completion") {
        respond({
          isIncomplete: false,
          items: [
            {
              label: "Heading",
              detail: "other.md",
              textEdit: {
                range: range(11, 13),
                newText: "[[heading]]",
              },
            },
            {
              label: "mutating",
              additionalTextEdits: [{ range: range(0, 0), newText: "import x" }],
            },
          ],
        });
      } else if (message.method === "textDocument/hover") {
        respond({
          contents: { kind: "markdown", value: "# Heading\n\nTarget.<script>bad()</script>" },
          range: { start: { line: 2, character: 6 }, end: { line: 2, character: 11 } },
        });
      } else if (message.method === "textDocument/definition") {
        respond([
          { uri: this.uri, range: range(0, 7) },
          { uri: "file:///outside/other.md", range: range(0, 1) },
        ]);
      } else if (message.method === "textDocument/codeAction") {
        respond([
          {
            title: "Create a Table of Contents",
            kind: "source",
            edit: { changes: { [this.uri]: [{ range: range(0, 0), newText: "<!--toc-->\n" }] } },
          },
          { title: "Explain only" },
        ]);
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

function openEvent(relativePath = "README.md", text = "# Title\n\nSee [[other]]\n\nIncomplete [[\n") {
  return {
    kind: "open",
    identity,
    documentId: 7,
    documentVersion: 1,
    runtimeGeneration: 1,
    activeMode: "markdown",
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
      cursorByteOffset: 12,
      feature,
    },
    window: {},
  };
}

test("Markdown package manifest matches package.json and fixes marksman server launch authority", () => {
  assert.deepEqual(lspMarkdownPackageManifest(), JSON.parse(fs.readFileSync(new URL("./package.json", import.meta.url))));
  const manifest = lspMarkdownPackageManifest();
  assert.deepEqual(manifest.clay.capabilities, ["language-server"]);
  assert.deepEqual(manifest.clay.contributions.languageServers, [{
    id: "lsp-markdown.server",
    executable: "marksman",
    args: ["server"],
    inheritEnvironment: [],
  }]);
  assert.deepEqual(manifest.clay.contributions.languageIntelligenceProviders[0].modes, ["markdown"]);
  assert.deepEqual(manifest.clay.contributions.languageIntelligenceProviders[0].features, [
    "hover",
    "definition",
    "codeAction",
  ]);
  assert.equal(manifest.clay.contributions.completionProviders[0].priority, 100);
  assert.equal(manifest.clay.contributions.completionProviders[0].exclusive, undefined);
});

test("Marksman bridge maps advertised features, full sync, and drops stale/mutating/external output", async () => {
  const session = new FakeMarksmanSession();
  const decorations = [];
  const diagnostics = [];
  const bridge = createMarksmanBridge({
    startSession: async () => session,
    publishDecorations: (value) => decorations.push(value),
    publishDiagnostics: (value) => diagnostics.push(value),
    packageManifest: lspMarkdownPackageManifest(),
  });

  await bridge.handle(openEvent("notes/guide.mdown", "# Title\n\nSee [[other]]\n"));
  const opened = session.messages.find((message) => message.method === "textDocument/didOpen");
  assert.equal(opened.params.textDocument.languageId, "markdown");
  assert.equal(languageIdForRelativePath("notes/guide.markdown"), "markdown");
  assert.equal(decorations.length, 1);
  assert.equal(decorations[0].spans[0].tokenType, "Class");
  assert.equal(diagnostics.length, 1);
  assert.equal(diagnostics[0].source, "lsp-markdown");
  assert.match(diagnostics[0].spans[0].message, /non-existent/);

  await bridge.handle({
    kind: "change",
    identity,
    documentId: 7,
    baseVersion: 1,
    documentVersion: 2,
    byteStart: 0,
    byteEnd: 0,
    insertedText: "x",
  });
  const didChange = session.messages.filter((message) => message.method === "textDocument/didChange").at(-1);
  assert.equal(didChange.params.contentChanges.length, 1);
  assert.equal("range" in didChange.params.contentChanges[0], false);
  assert.match(didChange.params.contentChanges[0].text, /^x# Title/);

  const completion = await bridge.handle({
    kind: "completion",
    identity,
    request: {
      documentId: 7,
      documentVersion: 2,
      cursorByteOffset: 2,
      trigger: { kind: "character", character: "[" },
    },
    window: {},
  });
  assert.equal(completion.items.length, 1);
  assert.equal(completion.items[0].insertText, "[[heading]]");

  const hover = await bridge.handle({
    ...requestEvent("hover", 1),
    request: { ...requestEvent("hover", 1).request, documentVersion: 2 },
  });
  assert.match(hover.hover.markdown, /Heading/);
  assert.doesNotMatch(hover.hover.markdown, /script|bad/);
  const definition = await bridge.handle({
    ...requestEvent("definition", 2),
    request: { ...requestEvent("definition", 2).request, documentVersion: 2 },
  });
  assert.deepEqual(definition.definition.locations, [{ documentId: 7, range: { byteStart: 0, byteEnd: 7 } }]);
  const actions = await bridge.handle({
    ...requestEvent("codeAction", 3),
    request: { ...requestEvent("codeAction", 3).request, documentVersion: 2 },
  });
  assert.deepEqual(actions.codeAction.actions, [{ title: "Explain only" }]);
  const signatures = await bridge.handle({
    ...requestEvent("signatureHelp", 4),
    request: { ...requestEvent("signatureHelp", 4).request, documentVersion: 2 },
  });
  assert.equal(signatures.status, "empty");

  await bridge.handle({ kind: "close", identity, documentId: 7, documentVersion: 2 });
  await bridge.handle({ kind: "shutdown", identity });
  assert.equal(session.stopped, true);
  assert.deepEqual(session.messages.slice(-3).map((message) => message.method), [
    "textDocument/didClose",
    "shutdown",
    "exit",
  ]);
});

test("Marksman bridge leaves absent capabilities empty and rejects forged identity", async () => {
  const session = new FakeMarksmanSession({ advertiseSemantic: false, advertiseCodeAction: false });
  const decorations = [];
  const diagnostics = [];
  const bridge = createMarksmanBridge({
    startSession: async () => session,
    publishDecorations: (value) => decorations.push(value),
    publishDiagnostics: (value) => diagnostics.push(value),
    packageManifest: lspMarkdownPackageManifest(),
  });
  await bridge.handle(openEvent());
  assert.equal(decorations.length, 0);
  assert.equal(diagnostics.length, 0);
  const actions = await bridge.handle(requestEvent("codeAction", 1));
  assert.equal(actions.status, "empty");

  let started = false;
  const guarded = createMarksmanBridge({
    startSession: async () => { started = true; },
    publishDecorations() {},
    publishDiagnostics() {},
    packageManifest: lspMarkdownPackageManifest(),
  });
  await assert.rejects(
    () => guarded.handle({ ...openEvent(), identity: { ...identity, package: "@evil/pkg" } }),
    /invalid_identity/,
  );
  assert.equal(started, false);
  assert.throws(() => languageIdForRelativePath("src/main.rs"), /unsupported_language/);
});
