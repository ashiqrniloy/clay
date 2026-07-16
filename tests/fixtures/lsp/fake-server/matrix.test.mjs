import assert from "node:assert/strict";
import test from "node:test";

import { FakeLspSession, encodeFrame } from "./session.mjs";
import { createRustAnalyzerBridge } from "../../../../packages/lsp-rust/dist/server.js";
import { lspRustPackageManifest } from "../../../../packages/lsp-rust/dist/index.js";
import { createTypescriptBridge } from "../../../../packages/lsp-typescript/dist/server.js";
import { lspTypescriptPackageManifest } from "../../../../packages/lsp-typescript/dist/index.js";
import { createJavascriptBridge } from "../../../../packages/lsp-javascript/dist/server.js";
import { lspJavascriptPackageManifest } from "../../../../packages/lsp-javascript/dist/index.js";
import { createMarksmanBridge } from "../../../../packages/lsp-markdown/dist/server.js";
import { lspMarkdownPackageManifest } from "../../../../packages/lsp-markdown/dist/index.js";

function identityFor(packageName, prefix, contribution, analyzerId) {
  return {
    package: packageName,
    packageVersion: "0.1.0",
    packagePrefix: prefix,
    analyzerId,
    contribution,
  };
}

function openEvent(identity, profile, documentId = 7) {
  return {
    kind: "open",
    identity,
    documentId,
    documentVersion: 1,
    runtimeGeneration: 1,
    activeMode: profile.languageId === "plaintext" ? "markdown" : profile.languageId,
    workspaceRootId: 1,
    canonicalRootPath: "/workspace",
    relativePath: profile.relativePath,
    text: profile.text,
  };
}

function requestEvent(identity, feature, requestId, documentVersion = 1) {
  return {
    kind: "languageIntelligence",
    identity,
    request: {
      requestId,
      documentId: 7,
      documentVersion,
      cursorByteOffset: 1,
      feature,
    },
  };
}

async function runBridgeSuite({
  label,
  profileName,
  createBridge,
  packageManifest,
  identity,
  expectSignatureHelp,
  openExtraDocuments = [],
}) {
  const session = new FakeLspSession(profileName, { fragmentReads: false });
  const decorations = [];
  const diagnostics = [];
  const started = { count: 0 };
  const bridge = createBridge({
    startSession: async () => {
      started.count += 1;
      return session;
    },
    publishDecorations: (value) => decorations.push(value),
    publishDiagnostics: (value) => diagnostics.push(value),
    packageManifest,
  });

  for (const extra of openExtraDocuments) {
    await bridge.handle(extra);
  }
  await bridge.handle(openEvent(identity, session.profile));
  assert.equal(started.count, 1, `${label} starts exactly one session`);
  assert.ok(decorations.length >= 1, `${label} publishes semantic decorations`);
  assert.ok(diagnostics.length >= 1, `${label} publishes diagnostics`);

  const completion = await bridge.handle({
    kind: "completion",
    identity,
    request: {
      documentId: 7,
      documentVersion: 1,
      cursorByteOffset: 2,
      trigger: { kind: "manual" },
    },
    window: {},
  });
  assert.ok(Array.isArray(completion.items), `${label} completion items`);

  const hover = await bridge.handle(requestEvent(identity, "hover", 1));
  assert.ok(hover.hover?.markdown, `${label} hover`);
  assert.doesNotMatch(hover.hover.markdown, /<script|bad\(\)/);

  const definition = await bridge.handle(requestEvent(identity, "definition", 2));
  assert.ok(definition.definition, `${label} definition payload`);
  if (Array.isArray(definition.definition.locations)) {
    for (const location of definition.definition.locations) {
      assert.notEqual(location.documentId, undefined);
    }
  }

  const actions = await bridge.handle(requestEvent(identity, "codeAction", 3));
  assert.ok(actions.codeAction, `${label} codeAction payload`);
  for (const action of actions.codeAction.actions ?? []) {
    assert.equal(action.edit, undefined, `${label} rejects mutating edits`);
  }

  const signatures = await bridge.handle(requestEvent(identity, "signatureHelp", 4));
  if (expectSignatureHelp) {
    assert.ok(signatures.signatureHelp?.signatures?.length >= 1, `${label} signature help`);
  } else {
    assert.deepEqual(
      signatures.signatureHelp?.signatures ?? [],
      [],
      `${label} empty signature help`,
    );
  }

  await bridge.handle({ kind: "close", identity, documentId: 7, documentVersion: 1 });
  await bridge.handle({ kind: "shutdown", identity });
  assert.equal(session.stopped, true, `${label} stops session`);
}

test("generic fake-server matrix covers rust/typescript/javascript/markdown package bridges", async () => {
  await runBridgeSuite({
    label: "rust",
    profileName: "rust",
    createBridge: createRustAnalyzerBridge,
    packageManifest: lspRustPackageManifest(),
    identity: identityFor("@clay/lsp-rust", "lsp-rust", "lsp-rust.server", "lsp-rust.bridge"),
    expectSignatureHelp: true,
  });

  await runBridgeSuite({
    label: "typescript",
    profileName: "typescript",
    createBridge: createTypescriptBridge,
    packageManifest: lspTypescriptPackageManifest(),
    identity: identityFor(
      "@clay/lsp-typescript",
      "lsp-typescript",
      "lsp-typescript.server",
      "lsp-typescript.bridge",
    ),
    expectSignatureHelp: true,
  });

  await runBridgeSuite({
    label: "javascript",
    profileName: "javascript",
    createBridge: createJavascriptBridge,
    packageManifest: lspJavascriptPackageManifest(),
    identity: identityFor(
      "@clay/lsp-javascript",
      "lsp-javascript",
      "lsp-javascript.server",
      "lsp-javascript.bridge",
    ),
    expectSignatureHelp: true,
  });

  const markdownIdentity = identityFor(
    "@clay/lsp-markdown",
    "lsp-markdown",
    "lsp-markdown.server",
    "lsp-markdown.bridge",
  );
  await runBridgeSuite({
    label: "markdown",
    profileName: "markdown",
    createBridge: createMarksmanBridge,
    packageManifest: lspMarkdownPackageManifest(),
    identity: markdownIdentity,
    expectSignatureHelp: false,
    openExtraDocuments: [{
      kind: "open",
      identity: markdownIdentity,
      documentId: 8,
      documentVersion: 1,
      runtimeGeneration: 1,
      activeMode: "markdown",
      workspaceRootId: 1,
      canonicalRootPath: "/workspace",
      relativePath: "other.md",
      text: "# other\n\nBack to [[README]].\n",
    }],
  });
});

test("generic fake-server matrix rejects forged identity before session start", async () => {
  let started = false;
  const bridge = createRustAnalyzerBridge({
    startSession: async () => {
      started = true;
      return new FakeLspSession("rust");
    },
    publishDecorations() {},
    publishDiagnostics() {},
    packageManifest: lspRustPackageManifest(),
  });
  await assert.rejects(
    () => bridge.handle({
      kind: "open",
      identity: identityFor("@evil/pkg", "lsp-rust", "lsp-rust.server", "lsp-rust.bridge"),
      documentId: 7,
      documentVersion: 1,
      runtimeGeneration: 1,
      activeMode: "rust",
      workspaceRootId: 1,
      canonicalRootPath: "/workspace",
      relativePath: "src/main.rs",
      text: "fn main() {}\n",
    }),
    /invalid_identity/,
  );
  assert.equal(started, false);
});

test("hung fake profile answers initialize only and stays silent on feature requests", async () => {
  const session = new FakeLspSession("hung", { fragmentReads: false });
  await session.sendBytes(encodeFrame({
    jsonrpc: "2.0",
    id: 1,
    method: "initialize",
    params: { processId: null, rootUri: "file:///workspace", capabilities: {} },
  }));
  const first = await session.readBytes();
  assert.ok(first.length > 0);
  // Drain any fragmented initialize leftovers.
  while ((await session.readBytes()).length) {}

  await session.sendBytes(encodeFrame({
    jsonrpc: "2.0",
    id: 2,
    method: "textDocument/hover",
    params: {
      textDocument: { uri: "file:///workspace/hang.txt" },
      position: { line: 0, character: 0 },
    },
  }));
  assert.equal(session.reads.length, 0, "hung profile must not enqueue feature responses");
  assert.equal(
    session.messages.filter((message) => message.method === "textDocument/hover").length,
    1,
  );
});
