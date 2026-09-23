import assert from "node:assert/strict";
import { test } from "bun:test";
import {
  FrameTooLargeError,
  MAX_FRAME_BYTES,
  parseFrame,
  readNdjson,
  readNdjsonConcurrent,
} from "../rpc.js";

test("parseFrame rejects oversize lines", () => {
  const line = "x".repeat(MAX_FRAME_BYTES + 1);
  assert.throws(() => parseFrame(line), FrameTooLargeError);
});

test("parseFrame accepts a JSON-RPC object", () => {
  const parsed = parseFrame('{"jsonrpc":"2.0","id":1,"method":"session.list","params":{}}');
  assert.equal(parsed.method, "session.list");
  assert.equal(parsed.id, 1);
});

test("readNdjson rejects a buffer with no newline past the cap", async () => {
  async function* chunks() {
    yield "y".repeat(MAX_FRAME_BYTES + 8);
  }
  await assert.rejects(() => readNdjson(chunks(), async () => {}), FrameTooLargeError);
});

test("readNdjsonConcurrent delivers a later line while an earlier handler awaits", async () => {
  let release: (() => void) | undefined;
  const gate = new Promise<void>((resolve) => {
    release = resolve;
  });
  const seen: number[] = [];
  async function* chunks() {
    yield '{"id":1}\n{"id":2}\n';
  }
  await Promise.race([
    readNdjsonConcurrent(chunks(), async (line) => {
      const id = (JSON.parse(line) as { id: number }).id;
      seen.push(id);
      if (id === 1) await gate;
      else release?.();
    }),
    new Promise<never>((_, reject) => {
      setTimeout(() => reject(new Error("stdio reader deadlocked on earlier handler")), 1000);
    }),
  ]);
  assert.deepEqual(seen, [1, 2]);
});
