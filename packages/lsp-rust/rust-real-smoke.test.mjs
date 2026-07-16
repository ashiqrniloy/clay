import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import test from "node:test";

import { createRustAnalyzerBridge } from "./dist/server.js";
import { lspRustPackageManifest } from "./dist/index.js";

const enabled = process.env.CLAY_LSP_REAL_SMOKE === "1";
const root = new URL("../../tests/fixtures/lsp/rust/", import.meta.url).pathname.replace(/\/$/, "");
const identity = {
  package: "@clay/lsp-rust",
  packageVersion: "0.1.0",
  packagePrefix: "lsp-rust",
  analyzerId: "lsp-rust.bridge",
  contribution: "lsp-rust.server",
};

class ChildSession {
  constructor() {
    this.child = spawn("rustup", ["run", "stable", "rust-analyzer"], {
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
      const timer = setTimeout(() => reject(new Error(`real rust-analyzer read timed out\n${this.stderr}`)), timeoutMs);
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

function languageRequest(feature, requestId, byteOffset, version = 1) {
  return {
    kind: "languageIntelligence",
    identity,
    request: { requestId, documentId: 1, documentVersion: version, cursorByteOffset: byteOffset, feature },
    window: {},
  };
}

test("real rust-analyzer maps semantic, diagnostics, completion, hover, definition, and shutdown", { skip: !enabled, timeout: 30000 }, async () => {
  let source = "fn answer() -> u32 { 42 }\nfn main() {\n    let value: u32 = \"bad\";\n    let broken = ;\n    let result = answer();\n    prin\n}\n";
  let version = 1;
  const decorations = [];
  const diagnostics = [];
  const bridge = createRustAnalyzerBridge({
    startSession: async () => new ChildSession(),
    publishDecorations: (value) => decorations.push(value),
    publishDiagnostics: (value) => diagnostics.push(value),
    packageManifest: lspRustPackageManifest(),
  });

  try {
    await bridge.handle({
      kind: "open",
      identity,
      documentId: 1,
      documentVersion: version,
      runtimeGeneration: 1,
      activeMode: "rust",
      workspaceRootId: 1,
      canonicalRootPath: root,
      relativePath: "src/main.rs",
      text: source,
    });
    assert.ok(decorations.at(-1).spans.length > 0, "semantic token refinement expected");
    for (let attempt = 0; diagnostics.at(-1)?.spans.length === 0 && attempt < 10; attempt += 1) {
      await new Promise((resolve) => setTimeout(resolve, 250));
      version += 1;
      source += " ";
      await bridge.handle({ kind: "reset", identity, documentId: 1, documentVersion: version, text: source });
    }
    assert.ok(diagnostics.at(-1).spans.length > 0, "syntax/type diagnostic expected");

    const completion = await bridge.handle({
      kind: "completion",
      identity,
      request: { documentId: 1, documentVersion: version, cursorByteOffset: source.indexOf("prin") + 4, trigger: { kind: "manual" } },
      window: {},
    });
    assert.ok(completion.items.length > 0, "completion expected");
    assert.ok(completion.items.some((item) => item.textFormat === "snippet"), "snippet completion expected");

    const hover = await bridge.handle(languageRequest("hover", 2, source.indexOf("value"), version));
    assert.ok(hover.hover.markdown.length > 0, "hover type expected");
    const definition = await bridge.handle(languageRequest("definition", 3, source.lastIndexOf("answer") + 1, version));
    assert.ok(definition.definition.locations.some((location) => location.documentId === 1), "same-document definition expected");
  } finally {
    await bridge.handle({ kind: "close", identity, documentId: 1, documentVersion: version });
    await bridge.handle({ kind: "shutdown", identity });
  }
});
