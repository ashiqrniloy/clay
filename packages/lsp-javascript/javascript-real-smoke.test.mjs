import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import test from "node:test";

import { createJavascriptBridge } from "./dist/server.js";
import { lspJavascriptPackageManifest } from "./dist/index.js";

const enabled = process.env.CLAY_LSP_REAL_SMOKE === "1";
const root = new URL("../../tests/fixtures/lsp/javascript/", import.meta.url).pathname.replace(/\/$/, "");
const identity = {
  package: "@clay/lsp-javascript",
  packageVersion: "0.1.0",
  packagePrefix: "lsp-javascript",
  analyzerId: "lsp-javascript.bridge",
  contribution: "lsp-javascript.server",
};

class ChildSession {
  constructor() {
    this.child = spawn("typescript-language-server", ["--stdio"], {
      cwd: root,
      env: { HOME: process.env.HOME, PATH: process.env.PATH },
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
      const timer = setTimeout(() => reject(new Error(`real typescript-language-server read timed out\n${this.stderr}`)), timeoutMs);
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

test("real typescript-language-server maps JavaScript semantic, diagnostics, completion, hover, and shutdown", {
  skip: !enabled,
  timeout: 45000,
}, async () => {
  let source = "/** @returns {number} */\nexport function answer() { return 42 }\nexport function main() {\n  /** @type {number} */\n  const value = \"bad\";\n  const broken = ;\n  const result = answer();\n  prin\n}\n";
  let version = 1;
  const decorations = [];
  const diagnostics = [];
  const bridge = createJavascriptBridge({
    startSession: async () => new ChildSession(),
    publishDecorations: (value) => decorations.push(value),
    publishDiagnostics: (value) => diagnostics.push(value),
    packageManifest: lspJavascriptPackageManifest(),
  });

  try {
    await bridge.handle({
      kind: "open",
      identity,
      documentId: 1,
      documentVersion: version,
      runtimeGeneration: 1,
      activeMode: "javascript",
      workspaceRootId: 1,
      canonicalRootPath: root,
      relativePath: "src/main.js",
      text: source,
    });
    assert.ok(decorations.at(-1)?.spans.length > 0, "semantic token refinement expected");
    for (let attempt = 0; (diagnostics.at(-1)?.spans.length ?? 0) === 0 && attempt < 12; attempt += 1) {
      await new Promise((resolve) => setTimeout(resolve, 250));
      version += 1;
      source += " ";
      await bridge.handle({ kind: "reset", identity, documentId: 1, documentVersion: version, text: source });
    }
    assert.ok((diagnostics.at(-1)?.spans.length ?? 0) > 0, "JavaScript diagnostic expected");

    const completion = await bridge.handle({
      kind: "completion",
      identity,
      request: {
        documentId: 1,
        documentVersion: version,
        cursorByteOffset: source.indexOf("prin") + 4,
        trigger: { kind: "manual" },
      },
      window: {},
    });
    assert.ok(completion.items.length > 0, "completion expected");

    const hover = await bridge.handle({
      kind: "languageIntelligence",
      identity,
      request: {
        requestId: 2,
        documentId: 1,
        documentVersion: version,
        cursorByteOffset: source.indexOf("value"),
        feature: "hover",
      },
      window: {},
    });
    assert.ok(hover.hover.markdown.length > 0, "hover expected");
  } finally {
    await bridge.handle({ kind: "close", identity, documentId: 1, documentVersion: version });
    await bridge.handle({ kind: "shutdown", identity });
  }
});
