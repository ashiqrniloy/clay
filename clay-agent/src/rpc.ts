// Local trusted lane (daemon stdio): cap bounds memory, not trust. Prism's
// per-tool output ceilings default to 64 MiB (shell stdout, search scan, git
// patch), so 64 MiB here means the wire never binds before Prism's caps do.
export const MAX_FRAME_BYTES = 64 * 1024 * 1024;

export class FrameTooLargeError extends Error {
  readonly code = "frame_too_large";
  constructor() {
    super("JSON-RPC frame exceeds 64 MiB");
    this.name = "FrameTooLargeError";
  }
}

export interface JsonRpcRequest {
  readonly jsonrpc?: string;
  readonly id?: string | number | null;
  readonly method?: unknown;
  readonly params?: unknown;
}

export function parseFrame(line: string): JsonRpcRequest {
  if (Buffer.byteLength(line, "utf8") > MAX_FRAME_BYTES) throw new FrameTooLargeError();
  const parsed: unknown = JSON.parse(line);
  if (typeof parsed !== "object" || parsed === null || Array.isArray(parsed)) {
    throw new Error("JSON-RPC request must be an object");
  }
  return parsed as JsonRpcRequest;
}

export function encodeFrame(value: unknown): string {
  const line = JSON.stringify(value);
  if (Buffer.byteLength(line, "utf8") > MAX_FRAME_BYTES) throw new FrameTooLargeError();
  return `${line}\n`;
}

export async function readNdjson(
  input: AsyncIterable<Buffer | string>,
  onLine: (line: string) => Promise<void>,
): Promise<void> {
  let buf = "";
  for await (const chunk of input) {
    buf += typeof chunk === "string" ? chunk : chunk.toString("utf8");
    if (Buffer.byteLength(buf, "utf8") > MAX_FRAME_BYTES && !buf.includes("\n")) {
      throw new FrameTooLargeError();
    }
    let idx = buf.indexOf("\n");
    while (idx >= 0) {
      const line = buf.slice(0, idx).replace(/\r$/, "");
      buf = buf.slice(idx + 1);
      if (Buffer.byteLength(line, "utf8") > MAX_FRAME_BYTES) throw new FrameTooLargeError();
      if (line.length > 0) await onLine(line);
      idx = buf.indexOf("\n");
    }
  }
  if (buf.length > 0) {
    if (Buffer.byteLength(buf, "utf8") > MAX_FRAME_BYTES) throw new FrameTooLargeError();
    await onLine(buf.replace(/\r$/, ""));
  }
}

/**
 * Same framing as `readNdjson`, but later lines are delivered while an earlier
 * handler is still awaiting. Required for clay-agent stdio: `session.prompt`
 * holds the turn until tools finish, and `document.*` reverse-RPC replies
 * arrive on this same stdin. Awaiting the prompt handler before reading the
 * next line deadlocks every reverse request until `REVERSE_TIMEOUT_MS`.
 */
export async function readNdjsonConcurrent(
  input: AsyncIterable<Buffer | string>,
  onLine: (line: string) => Promise<void>,
): Promise<void> {
  const pending: Promise<void>[] = [];
  await readNdjson(input, async (line) => {
    pending.push(onLine(line));
  });
  await Promise.all(pending);
}
