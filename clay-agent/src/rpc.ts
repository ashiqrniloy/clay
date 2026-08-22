export const MAX_FRAME_BYTES = 1024 * 1024;

export class FrameTooLargeError extends Error {
  readonly code = "frame_too_large";
  constructor() {
    super("JSON-RPC frame exceeds 1 MiB");
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
