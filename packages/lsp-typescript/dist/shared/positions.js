import { decodeUtf8, encodeUtf8, utf8ByteLength } from "./utf8.js";
export const POSITION_ENCODINGS = new Set(["utf-8", "utf-16", "utf-32"]);

function requireEncoding(encoding) {
  if (!POSITION_ENCODINGS.has(encoding)) throw new Error(`lsp.invalid_encoding: ${encoding}`);
}

function requireByteBoundary(bytes, offset) {
  if (!Number.isInteger(offset) || offset < 0 || offset > bytes.length) {
    throw new Error("lsp.invalid_position: byte offset is outside document");
  }
  if (offset < bytes.length && (bytes[offset] & 0xc0) === 0x80) {
    throw new Error("lsp.invalid_position: byte offset splits a UTF-8 code point");
  }
}

function units(text, encoding) {
  if (encoding === "utf-8") return utf8ByteLength(text);
  if (encoding === "utf-16") return text.length;
  return [...text].length;
}

function stringIndexForUnits(text, count, encoding) {
  if (!Number.isInteger(count) || count < 0) throw new Error("lsp.invalid_position: character must be non-negative integer");
  if (encoding === "utf-16") {
    if (count > text.length) throw new Error("lsp.invalid_position: character exceeds line");
    if (count > 0 && count < text.length) {
      const previous = text.charCodeAt(count - 1);
      const next = text.charCodeAt(count);
      if (previous >= 0xd800 && previous <= 0xdbff && next >= 0xdc00 && next <= 0xdfff) {
        throw new Error("lsp.invalid_position: character splits a surrogate pair");
      }
    }
    return count;
  }
  let consumed = 0;
  let index = 0;
  for (const scalar of text) {
    if (consumed === count) return index;
    const width = encoding === "utf-8" ? utf8ByteLength(scalar) : 1;
    if (consumed + width > count) throw new Error("lsp.invalid_position: character splits a code point");
    consumed += width;
    index += scalar.length;
  }
  if (consumed !== count) throw new Error("lsp.invalid_position: character exceeds line");
  return index;
}

function normalizeRelativePath(relativePath) {
  if (typeof relativePath !== "string" || relativePath.length === 0 || relativePath.includes("\\")) {
    throw new Error("lsp.invalid_path: relative path must be non-empty and use forward slashes");
  }
  const segments = relativePath.split("/");
  if (segments.some((part) => part === "" || part === "." || part === "..")) {
    throw new Error("lsp.invalid_path: relative path contains traversal or empty segment");
  }
  return segments.join("/");
}

function normalizeRoot(rootPath) {
  if (typeof rootPath !== "string" || !rootPath.startsWith("/") || rootPath.includes("\0") || rootPath.includes("\\")) {
    throw new Error("lsp.invalid_root: canonical root must be an absolute POSIX path");
  }
  const root = rootPath === "/" ? "/" : rootPath.replace(/\/+$/, "");
  if (root.includes("//") || root.split("/").some((part) => part === "." || part === "..")) {
    throw new Error("lsp.invalid_root: root path must already be canonical");
  }
  return root;
}

function encodePath(path) {
  return path.split("/").map((segment, index) => index === 0 ? "" : encodeURIComponent(segment)).join("/");
}

export function rootPathToFileUri(rootPath) {
  return `file://${encodePath(normalizeRoot(rootPath))}`;
}

export function pathToFileUri(rootPath, relativePath) {
  const root = normalizeRoot(rootPath);
  const relative = normalizeRelativePath(relativePath);
  return `file://${encodePath(root === "/" ? `/${relative}` : `${root}/${relative}`)}`;
}

export function fileUriToRelative(uri, rootPath) {
  let parsed;
  try {
    parsed = new URL(uri);
  } catch {
    throw new Error("lsp.invalid_uri: malformed URI");
  }
  if (parsed.protocol !== "file:" || parsed.username || parsed.password || parsed.port || parsed.hostname
      || parsed.search || parsed.hash || /%(?:2f|5c)/i.test(parsed.pathname)) {
    throw new Error("lsp.invalid_uri: only unambiguous local file URIs are supported");
  }
  let path;
  try {
    path = parsed.pathname.split("/").map(decodeURIComponent).join("/");
  } catch {
    throw new Error("lsp.invalid_uri: malformed percent encoding");
  }
  if (path.includes("\0") || path.includes("\\")) throw new Error("lsp.invalid_uri: invalid file path");
  const root = normalizeRoot(rootPath);
  const prefix = root === "/" ? "/" : `${root}/`;
  if (!path.startsWith(prefix) || path === root) throw new Error("lsp.out_of_root: location is outside approved root");
  return normalizeRelativePath(path.slice(prefix.length));
}

export class VersionedDocument {
  constructor(text, version, encoding = "utf-16") {
    requireEncoding(encoding);
    this.encoding = encoding;
    this.reset(text, version);
  }

  reset(text, version) {
    if (typeof text !== "string" || !Number.isInteger(version) || version < 0) {
      throw new Error("lsp.invalid_document: text and non-negative integer version are required");
    }
    this.text = text;
    this.version = version;
    this.bytes = encodeUtf8(text);
    this.lineStarts = [0];
    for (let index = 0; index < this.bytes.length; index += 1) {
      if (this.bytes[index] === 10) this.lineStarts.push(index + 1);
    }
  }

  #lineBounds(line) {
    if (!Number.isInteger(line) || line < 0 || line >= this.lineStarts.length) {
      throw new Error("lsp.invalid_position: line is outside document");
    }
    const start = this.lineStarts[line];
    let end = line + 1 < this.lineStarts.length ? this.lineStarts[line + 1] - 1 : this.bytes.length;
    if (end > start && this.bytes[end - 1] === 13) end -= 1;
    return { start, end };
  }

  byteToPosition(offset) {
    requireByteBoundary(this.bytes, offset);
    let low = 0;
    let high = this.lineStarts.length;
    while (low + 1 < high) {
      const middle = (low + high) >> 1;
      if (this.lineStarts[middle] <= offset) low = middle;
      else high = middle;
    }
    const { start, end } = this.#lineBounds(low);
    if (offset > end) throw new Error("lsp.invalid_position: byte offset points into line ending");
    return { line: low, character: units(decodeUtf8(this.bytes.subarray(start, offset)), this.encoding) };
  }

  positionToByte(position) {
    if (position === null || typeof position !== "object") throw new Error("lsp.invalid_position: position object required");
    const { start, end } = this.#lineBounds(position.line);
    const line = decodeUtf8(this.bytes.subarray(start, end));
    const stringIndex = stringIndexForUnits(line, position.character, this.encoding);
    return start + utf8ByteLength(line.slice(0, stringIndex));
  }

  rangeToBytes(range) {
    if (range === null || typeof range !== "object") throw new Error("lsp.invalid_range: range object required");
    const byteStart = this.positionToByte(range.start);
    const byteEnd = this.positionToByte(range.end);
    if (byteStart > byteEnd) throw new Error("lsp.invalid_range: start exceeds end");
    return { byteStart, byteEnd };
  }

  applyByteChange({ baseVersion, version, byteStart, byteEnd, text }) {
    if (baseVersion !== this.version || !Number.isInteger(version) || version <= baseVersion) {
      throw new Error("lsp.stale_document: change versions are not ordered");
    }
    requireByteBoundary(this.bytes, byteStart);
    requireByteBoundary(this.bytes, byteEnd);
    if (byteStart > byteEnd || typeof text !== "string") throw new Error("lsp.invalid_change: invalid byte range or text");
    const range = { start: this.byteToPosition(byteStart), end: this.byteToPosition(byteEnd) };
    const inserted = encodeUtf8(text);
    const next = new Uint8Array(byteStart + inserted.length + this.bytes.length - byteEnd);
    next.set(this.bytes.subarray(0, byteStart));
    next.set(inserted, byteStart);
    next.set(this.bytes.subarray(byteEnd), byteStart + inserted.length);
    this.reset(decodeUtf8(next), version);
    return range;
  }
}
