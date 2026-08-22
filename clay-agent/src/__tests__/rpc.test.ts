import assert from "node:assert/strict";
import { test } from "node:test";
import { FrameTooLargeError, MAX_FRAME_BYTES, parseFrame, readNdjson } from "../rpc.js";

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
