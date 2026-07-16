import { decodeUtf8, encodeUtf8 } from "./utf8.js";

export const MAX_FRAME_BYTES = 1024 * 1024;
export const MAX_HEADER_BYTES = 8 * 1024;
const HEADER_END = new Uint8Array([13, 10, 13, 10]);

function concat(left, right) {
  const joined = new Uint8Array(left.length + right.length);
  joined.set(left);
  joined.set(right, left.length);
  return joined;
}

function indexOf(haystack, needle) {
  outer: for (let i = 0; i <= haystack.length - needle.length; i += 1) {
    for (let j = 0; j < needle.length; j += 1) {
      if (haystack[i + j] !== needle[j]) continue outer;
    }
    return i;
  }
  return -1;
}

function parseHeader(bytes, maxFrameBytes) {
  if (bytes.some((byte) => byte > 0x7f)) {
    throw new Error("lsp.invalid_header: header must be ASCII");
  }
  const header = decodeUtf8(bytes);
  let length;
  let contentTypeSeen = false;
  for (const line of header.split("\r\n")) {
    const separator = line.indexOf(":");
    if (separator < 1) throw new Error("lsp.invalid_header: malformed header line");
    const name = line.slice(0, separator).trim().toLowerCase();
    const value = line.slice(separator + 1).trim();
    if (name === "content-length") {
      if (length !== undefined || !/^(0|[1-9][0-9]*)$/.test(value)) {
        throw new Error("lsp.invalid_header: Content-Length must appear once as a decimal integer");
      }
      length = Number(value);
    } else if (name === "content-type") {
      if (contentTypeSeen || !/^application\/vscode-jsonrpc(?:\s*;\s*charset=utf-8)?$/i.test(value)) {
        throw new Error("lsp.invalid_header: unsupported Content-Type");
      }
      contentTypeSeen = true;
    } else {
      throw new Error(`lsp.invalid_header: unsupported header ${name}`);
    }
  }
  if (length === undefined) throw new Error("lsp.invalid_header: missing Content-Length");
  if (!Number.isSafeInteger(length) || length > maxFrameBytes) {
    throw new Error("lsp.frame_too_large: Content-Length exceeds frame budget");
  }
  return length;
}

export function encodeFrame(message, maxFrameBytes = MAX_FRAME_BYTES) {
  if (message === null || typeof message !== "object" || Array.isArray(message)) {
    throw new Error("lsp.invalid_message: JSON-RPC message must be an object");
  }
  const body = encodeUtf8(JSON.stringify(message));
  if (body.length > maxFrameBytes) throw new Error("lsp.frame_too_large: JSON body exceeds frame budget");
  return concat(encodeUtf8(`Content-Length: ${body.length}\r\n\r\n`), body);
}

export class FrameDecoder {
  #buffer = new Uint8Array();
  #bodyLength;

  constructor({ maxFrameBytes = MAX_FRAME_BYTES, maxHeaderBytes = MAX_HEADER_BYTES } = {}) {
    this.maxFrameBytes = maxFrameBytes;
    this.maxHeaderBytes = maxHeaderBytes;
  }

  push(chunk) {
    if (!(chunk instanceof Uint8Array)) throw new Error("lsp.invalid_bytes: frame input must be Uint8Array");
    if (this.#buffer.length + chunk.length > this.maxFrameBytes + this.maxHeaderBytes + HEADER_END.length) {
      throw new Error("lsp.frame_too_large: buffered frame exceeds budget");
    }
    this.#buffer = concat(this.#buffer, chunk);
    const messages = [];
    while (true) {
      if (this.#bodyLength === undefined) {
        const end = indexOf(this.#buffer, HEADER_END);
        if (end < 0) {
          if (this.#buffer.length > this.maxHeaderBytes) {
            throw new Error("lsp.header_too_large: header terminator not found within budget");
          }
          break;
        }
        if (end > this.maxHeaderBytes) throw new Error("lsp.header_too_large: header exceeds budget");
        this.#bodyLength = parseHeader(this.#buffer.subarray(0, end), this.maxFrameBytes);
        this.#buffer = this.#buffer.slice(end + HEADER_END.length);
      }
      if (this.#buffer.length < this.#bodyLength) break;
      const body = this.#buffer.subarray(0, this.#bodyLength);
      this.#buffer = this.#buffer.slice(this.#bodyLength);
      this.#bodyLength = undefined;
      let message;
      try {
        message = JSON.parse(decodeUtf8(body));
      } catch {
        throw new Error("lsp.invalid_json: body must be valid UTF-8 JSON");
      }
      if (message === null || typeof message !== "object" || Array.isArray(message)) {
        throw new Error("lsp.invalid_message: JSON-RPC message must be an object");
      }
      messages.push(message);
    }
    return messages;
  }

  finish() {
    if (this.#bodyLength !== undefined || this.#buffer.length !== 0) {
      throw new Error("lsp.truncated_frame: stream ended before frame completed");
    }
  }
}
