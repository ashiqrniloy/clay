import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

import { FakeLspSession, getProfile, profileNames, encodeFrame, FrameDecoder } from "./session.mjs";
import { LspClient } from "../../../../packages/lsp-shared/client.js";

const root = path.dirname(fileURLToPath(import.meta.url));
const serverPath = path.join(root, "server.mjs");

test("fake-server exposes one generic profile set for all bridge languages", () => {
  for (const name of ["rust", "typescript", "javascript", "markdown", "minimal", "hung", "exit-early", "malformed", "oversize"]) {
    assert.ok(profileNames().includes(name), `missing profile ${name}`);
    assert.equal(typeof getProfile(name).respond, "function");
  }
});

test("in-process fake session covers initialize, feature mapping hooks, and shutdown", async () => {
  const session = new FakeLspSession("rust", { fragmentReads: false });
  const client = new LspClient(session);
  const caps = await client.initialize({
    rootPath: "/workspace",
    capabilities: {
      textDocument: {
        synchronization: { dynamicRegistration: false },
      },
    },
  });
  assert.equal(caps.semanticTokensFull?.delta, true);
  assert.equal(caps.pullDiagnostics, true);

  await client.notify("textDocument/didOpen", {
    textDocument: {
      uri: session.uri,
      languageId: "rust",
      version: 1,
      text: "fn main() {}\n",
    },
  });
  const tokens = await client.request("textDocument/semanticTokens/full", {
    textDocument: { uri: session.uri },
  });
  assert.deepEqual(tokens.data, [0, 0, 2, 0, 1]);
  await client.shutdown();
  assert.equal(session.stopped, true);
});

test("minimal profile leaves optional providers empty", async () => {
  const session = new FakeLspSession("minimal", { fragmentReads: false });
  const client = new LspClient(session);
  const caps = await client.initialize({ rootPath: "/workspace", capabilities: {} });
  assert.equal(caps.hover, false);
  assert.equal(caps.completion, false);
  assert.equal(caps.semanticTokens, false);
  await client.shutdown();
});

test("malformed and oversize profiles produce framing failures", async () => {
  for (const profile of ["malformed", "oversize"]) {
    const session = new FakeLspSession(profile, { fragmentReads: false });
    const client = new LspClient(session);
    await client.initialize({ rootPath: "/workspace", capabilities: {} });
    await assert.rejects(
      () => client.request("textDocument/hover", {
        textDocument: { uri: session.uri },
        position: { line: 0, character: 0 },
      }),
      /lsp\./,
    );
  }
});

test("spawnable fake server speaks Content-Length initialize/shutdown on stdio", async () => {
  const child = spawn(process.execPath, [serverPath, "--profile=minimal"], {
    stdio: ["pipe", "pipe", "pipe"],
    env: { PATH: process.env.PATH },
  });
  const decoder = new FrameDecoder();
  const chunks = [];
  const waiters = [];
  let stderr = "";
  child.stdout.on("data", (chunk) => {
    const bytes = new Uint8Array(chunk);
    const waiter = waiters.shift();
    if (waiter) waiter(bytes);
    else chunks.push(bytes);
  });
  child.stderr.on("data", (chunk) => {
    stderr += chunk.toString();
  });

  async function readMessage(timeoutMs = 2000) {
    const deadline = Date.now() + timeoutMs;
    while (Date.now() < deadline) {
      while (chunks.length) {
        for (const message of decoder.push(chunks.shift())) return message;
      }
      await new Promise((resolve, reject) => {
        const timer = setTimeout(() => reject(new Error(`timeout\n${stderr}`)), deadline - Date.now());
        waiters.push((bytes) => {
          clearTimeout(timer);
          chunks.push(bytes);
          resolve();
        });
      });
    }
    throw new Error(`no message\n${stderr}`);
  }

  child.stdin.write(encodeFrame({
    jsonrpc: "2.0",
    id: 1,
    method: "initialize",
    params: {
      processId: null,
      rootUri: "file:///workspace",
      capabilities: {},
    },
  }));
  const initialize = await readMessage();
  assert.equal(initialize.id, 1);
  assert.equal(initialize.result.serverInfo.name, "clay-fake-lsp");

  child.stdin.write(encodeFrame({
    jsonrpc: "2.0",
    id: 2,
    method: "shutdown",
    params: null,
  }));
  const shutdown = await readMessage();
  assert.equal(shutdown.id, 2);
  child.stdin.write(encodeFrame({ jsonrpc: "2.0", method: "exit" }));
  await new Promise((resolve) => child.on("exit", resolve));
  assert.match(stderr, /profile=minimal/);
});

test("exit-early profile terminates after initialize response", async () => {
  const child = spawn(process.execPath, [serverPath, "--profile=exit-early"], {
    stdio: ["pipe", "pipe", "pipe"],
    env: { PATH: process.env.PATH },
  });
  const decoder = new FrameDecoder();
  let buffer = new Uint8Array();
  child.stdout.on("data", (chunk) => {
    const next = new Uint8Array(buffer.length + chunk.length);
    next.set(buffer);
    next.set(chunk, buffer.length);
    buffer = next;
  });
  child.stdin.write(encodeFrame({
    jsonrpc: "2.0",
    id: 1,
    method: "initialize",
    params: { processId: null, rootUri: "file:///workspace", capabilities: {} },
  }));
  const code = await new Promise((resolve) => child.on("exit", resolve));
  assert.equal(code, 0);
  const messages = decoder.push(buffer);
  decoder.finish();
  assert.equal(messages[0].id, 1);
});
