import assert from "node:assert/strict";
import fs from "node:fs";
import { spawn } from "node:child_process";
import test from "node:test";

import { createMarksmanBridge } from "./dist/server.js";
import { lspMarkdownPackageManifest } from "./dist/index.js";

const enabled = process.env.CLAY_LSP_REAL_SMOKE === "1";
const root = new URL("../../tests/fixtures/lsp/markdown/", import.meta.url).pathname.replace(/\/$/, "");
const identity = {
  package: "@clay/lsp-markdown",
  packageVersion: "0.1.0",
  packagePrefix: "lsp-markdown",
  analyzerId: "lsp-markdown.bridge",
  contribution: "lsp-markdown.server",
};

class ChildSession {
  constructor() {
    const executable = process.env.MARKSMAN_PATH
      || (process.env.PATH ?? "").split(":").map((dir) => `${dir}/marksman`).find((candidate) => {
        try {
          fs.accessSync(candidate, fs.constants.X_OK);
          return true;
        } catch {
          return false;
        }
      });
    if (!executable) throw new Error("marksman executable not found on PATH");
    this.child = spawn(executable, ["server"], {
      cwd: root,
      env: {},
      stdio: ["pipe", "pipe", "pipe"],
    });
    this.chunks = [];
    this.waiters = [];
    this.stderr = "";
    this.child.stdout.on("data", (chunk) => {
      const bytes = new Uint8Array(chunk);
      const waiter = this.waiters.shift();
      if (waiter) waiter(bytes);
      else this.chunks.push(bytes);
    });
    this.child.stderr.on("data", (chunk) => {
      this.stderr = (this.stderr + chunk.toString()).slice(-65536);
    });
  }

  async sendBytes(bytes) {
    await new Promise((resolve, reject) => this.child.stdin.write(bytes, (error) => error ? reject(error) : resolve()));
  }

  async readBytes(_maxBytes, timeoutMs) {
    if (this.chunks.length) return this.chunks.shift();
    return new Promise((resolve, reject) => {
      const timer = setTimeout(() => reject(new Error(`real marksman read timed out\n${this.stderr}`)), timeoutMs);
      this.waiters.push((bytes) => {
        clearTimeout(timer);
        resolve(bytes);
      });
    });
  }

  async stop() {
    if (this.child.exitCode === null) this.child.kill("SIGKILL");
  }
}

function languageRequest(feature, requestId, documentId, byteOffset, version = 1) {
  return {
    kind: "languageIntelligence",
    identity,
    request: { requestId, documentId, documentVersion: version, cursorByteOffset: byteOffset, feature },
    window: {},
  };
}

test("real marksman maps Markdown semantic, diagnostics, completion, hover, definition, and shutdown", {
  skip: !enabled,
  timeout: 45000,
}, async () => {
  const readme = "# Title\n\nSee [[other]] and continue.\n\nIncomplete [[\n";
  const other = "# Heading\n\nBack to [[README]].\n";
  let brokenText = "See [[missing-doc]].\n";
  let brokenVersion = 1;
  const decorations = [];
  const diagnostics = [];
  const bridge = createMarksmanBridge({
    startSession: async () => new ChildSession(),
    publishDecorations: (value) => decorations.push(value),
    publishDiagnostics: (value) => diagnostics.push(value),
    packageManifest: lspMarkdownPackageManifest(),
  });

  try {
    await bridge.handle({
      kind: "open",
      identity,
      documentId: 1,
      documentVersion: 1,
      runtimeGeneration: 1,
      activeMode: "markdown",
      workspaceRootId: 1,
      canonicalRootPath: root,
      relativePath: "README.md",
      text: readme,
    });
    await bridge.handle({
      kind: "open",
      identity,
      documentId: 2,
      documentVersion: 1,
      runtimeGeneration: 1,
      activeMode: "markdown",
      workspaceRootId: 1,
      canonicalRootPath: root,
      relativePath: "other.md",
      text: other,
    });
    await bridge.handle({
      kind: "open",
      identity,
      documentId: 3,
      documentVersion: brokenVersion,
      runtimeGeneration: 1,
      activeMode: "markdown",
      workspaceRootId: 1,
      canonicalRootPath: root,
      relativePath: "broken.md",
      text: brokenText,
    });

    assert.ok(decorations.some((entry) => (entry.spans?.length ?? 0) > 0), "semantic token refinement expected");
    for (let attempt = 0; !diagnostics.some((entry) => (entry.spans?.length ?? 0) > 0) && attempt < 16; attempt += 1) {
      await new Promise((resolve) => setTimeout(resolve, 250));
      brokenVersion += 1;
      brokenText += " ";
      await bridge.handle({
        kind: "reset",
        identity,
        documentId: 3,
        documentVersion: brokenVersion,
        text: brokenText,
      });
    }
    assert.ok(diagnostics.some((entry) => (entry.spans?.length ?? 0) > 0), "Marksman diagnostic expected");

    const wikiOffset = readme.indexOf("[[other]]") + 2;
    const hover = await bridge.handle(languageRequest("hover", 2, 1, wikiOffset));
    assert.ok(hover.hover.markdown.length > 0, "hover expected");
    const definition = await bridge.handle(languageRequest("definition", 3, 1, wikiOffset));
    assert.ok(definition.definition.locations.some((location) => location.documentId === 2), "cross-document definition expected");

    const completion = await bridge.handle({
      kind: "completion",
      identity,
      request: {
        documentId: 1,
        documentVersion: 1,
        cursorByteOffset: readme.indexOf("Incomplete [[") + "Incomplete [[".length,
        trigger: { kind: "character", character: "[" },
      },
      window: {},
    });
    assert.ok(completion.items.length > 0, "completion expected");

    const actions = await bridge.handle(languageRequest("codeAction", 4, 1, 1));
    assert.equal(actions.status === "empty" || actions.codeAction.actions.every((action) => !action.edit), true);
    const signatures = await bridge.handle(languageRequest("signatureHelp", 5, 1, 1));
    assert.equal(signatures.status, "empty");
  } finally {
    await bridge.handle({ kind: "close", identity, documentId: 1, documentVersion: 1 });
    await bridge.handle({ kind: "close", identity, documentId: 2, documentVersion: 1 });
    await bridge.handle({ kind: "close", identity, documentId: 3, documentVersion: brokenVersion });
    await bridge.handle({ kind: "shutdown", identity });
  }
});
