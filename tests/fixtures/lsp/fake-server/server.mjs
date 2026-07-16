#!/usr/bin/env node
/**
 * Spawnable Content-Length LSP fake server for Clay process-service tests.
 *
 * Usage:
 *   node server.mjs --profile=rust
 *   CLAY_FAKE_LSP_PROFILE=typescript node server.mjs
 *
 * Speaks LSP on stdio. Profiles live in ./profiles.mjs.
 */
import { readSync, writeSync } from "node:fs";
import { getProfile } from "./profiles.mjs";

const MAX_FRAME_BYTES = 1024 * 1024;
const MAX_HEADER_BYTES = 8 * 1024;

function parseArgs(argv) {
  let profile = process.env.CLAY_FAKE_LSP_PROFILE ?? "rust";
  for (const arg of argv.slice(2)) {
    if (arg.startsWith("--profile=")) profile = arg.slice("--profile=".length);
  }
  return { profile };
}

function encodeFrame(message) {
  const body = Buffer.from(JSON.stringify(message), "utf8");
  if (body.length > MAX_FRAME_BYTES) {
    throw new Error(`fake lsp frame exceeds ${MAX_FRAME_BYTES}`);
  }
  return Buffer.concat([
    Buffer.from(`Content-Length: ${body.length}\r\n\r\n`, "utf8"),
    body,
  ]);
}

class FrameDecoder {
  constructor() {
    this.buffer = Buffer.alloc(0);
  }

  push(chunk) {
    this.buffer = Buffer.concat([this.buffer, chunk]);
    const messages = [];
    while (true) {
      const headerEnd = this.buffer.indexOf("\r\n\r\n");
      if (headerEnd < 0) {
        if (this.buffer.length > MAX_HEADER_BYTES) {
          throw new Error("fake lsp header too large");
        }
        break;
      }
      const header = this.buffer.subarray(0, headerEnd).toString("ascii");
      const match = /Content-Length:\s*(\d+)/i.exec(header);
      if (!match) throw new Error("missing Content-Length");
      const length = Number(match[1]);
      if (!Number.isInteger(length) || length < 0 || length > MAX_FRAME_BYTES) {
        throw new Error("invalid Content-Length");
      }
      const total = headerEnd + 4 + length;
      if (this.buffer.length < total) break;
      const body = this.buffer.subarray(headerEnd + 4, total);
      this.buffer = this.buffer.subarray(total);
      messages.push(JSON.parse(body.toString("utf8")));
    }
    return messages;
  }
}

const { profile: profileName } = parseArgs(process.argv);
const profile = getProfile(profileName);
const decoder = new FrameDecoder();
const state = { uri: profile.uri, semanticRequests: 0 };
let exiting = false;

function writeAll(payload) {
  if (exiting) return;
  const bytes = typeof payload === "string"
    ? Buffer.from(payload)
    : encodeFrame(payload);
  writeSync(1, bytes);
}

function handleMessages(messages) {
  for (const message of messages) {
    if (message.method === "exit") {
      exiting = true;
      process.exit(0);
      return;
    }
    if (!("id" in message) || !("method" in message)) continue;
    if (profileName === "hung" && message.method !== "initialize" && message.method !== "shutdown") {
      continue;
    }
    const responses = profile.respond(message, state);
    if (state.emitRaw) {
      writeAll(state.emitRaw);
      state.emitRaw = undefined;
    }
    if (!responses) {
      writeSync(2, `fake-lsp unexpected method: ${message.method}\n`);
      continue;
    }
    for (const response of responses) writeAll(response);
    if (state.exitAfterWrite) {
      exiting = true;
      process.exit(0);
      return;
    }
  }
}

writeSync(2, `clay-fake-lsp profile=${profileName}\n`);

const chunk = Buffer.alloc(4096);
while (!exiting) {
  let bytesRead = 0;
  try {
    bytesRead = readSync(0, chunk, 0, chunk.length);
  } catch (error) {
    writeSync(2, `fake-lsp stdin error: ${error && error.message ? error.message : error}\n`);
    process.exit(2);
  }
  if (bytesRead === 0) break;
  try {
    handleMessages(decoder.push(chunk.subarray(0, bytesRead)));
  } catch (error) {
    writeSync(2, `fake-lsp decode error: ${error.message}\n`);
    process.exit(2);
  }
}
