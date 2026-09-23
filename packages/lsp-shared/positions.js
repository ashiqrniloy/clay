import { encodeUtf8, utf8ByteLength } from "./utf8.js";
export const POSITION_ENCODINGS = new Set(["utf-8", "utf-16", "utf-32"]);

function requireEncoding(encoding) {
  if (!POSITION_ENCODINGS.has(encoding)) throw new Error(`lsp.invalid_encoding: ${encoding}`);
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

const CHUNK_LINES = 64;

// Deterministic xorshift32 priorities keep the treap shape stable for a given
// edit sequence (same PRNG as frontend/src/editor/position-index.ts).
let priorityState = 0x9e3779b9;

function nextPriority() {
  let value = priorityState;
  value ^= (value << 13) >>> 0;
  value ^= value >>> 17;
  value ^= (value << 5) >>> 0;
  priorityState = value;
  return value || 1;
}

/** UTF-8 bytes of one line including its phantom trailing newline. Also
 * rejects unpaired surrogates exactly like `encodeUtf8` does. */
function lineByteWidth(line) {
  let width = 1;
  for (let at = 0; at < line.length; at += 1) {
    const code = line.codePointAt(at);
    if (code >= 0xd800 && code <= 0xdfff) throw new Error("lsp.invalid_utf8: unpaired surrogate");
    if (code > 0xffff) at += 1;
    width += code <= 0x7f ? 1 : code <= 0x7ff ? 2 : code <= 0xffff ? 3 : 4;
  }
  return width;
}

function makeLeaf(texts) {
  const l16 = new Uint32Array(texts.length);
  const l8 = new Uint32Array(texts.length);
  let w16 = 0;
  let w8 = 0;
  for (let index = 0; index < texts.length; index += 1) {
    l16[index] = texts[index].length + 1;
    l8[index] = lineByteWidth(texts[index]);
    w16 += l16[index];
    w8 += l8[index];
  }
  return { kind: "leaf", lines: texts.length, w16, w8, prio: nextPriority(), texts, l16, l8 };
}

function makeBranch(left, right) {
  return {
    kind: "branch",
    lines: left.lines + right.lines,
    w16: left.w16 + right.w16,
    w8: left.w8 + right.w8,
    prio: left.prio >= right.prio ? left.prio : right.prio,
    left,
    right,
  };
}

/** Treap merge; every line in `a` precedes every line in `b`. */
function join(a, b) {
  if (a === null) return b;
  if (b === null) return a;
  if (a.prio >= b.prio) {
    return a.kind === "leaf" ? makeBranch(a, b) : makeBranch(a.left, join(a.right, b));
  }
  return b.kind === "leaf" ? makeBranch(a, b) : makeBranch(join(a, b.left), b.right);
}

/** Treap split: first `count` lines to the left result. */
function split(node, count) {
  if (node === null || count <= 0) return [null, node];
  if (count >= node.lines) return [node, null];
  if (node.kind === "leaf") {
    return [makeLeaf(node.texts.slice(0, count)), makeLeaf(node.texts.slice(count))];
  }
  if (count < node.left.lines) {
    const [head, tail] = split(node.left, count);
    return [head, join(tail, node.right)];
  }
  if (count > node.left.lines) {
    const [head, tail] = split(node.right, count - node.left.lines);
    return [join(node.left, head), tail];
  }
  return [node.left, node.right];
}

function buildTree(texts) {
  let root = null;
  for (let start = 0; start < texts.length; start += CHUNK_LINES) {
    root = join(root, makeLeaf(texts.slice(start, start + CHUNK_LINES)));
  }
  return root;
}

/** Replaces tree lines `[start, start + count)` with `texts`. */
function replaceRange(root, start, count, texts) {
  const [head, rest] = split(root, start);
  const [, tail] = split(rest, count);
  return join(join(head, buildTree(texts)), tail);
}

function locateLine(root, line) {
  let node = root;
  let remaining = line;
  let start8 = 0;
  let start16 = 0;
  while (node.kind === "branch") {
    const left = node.left;
    if (remaining < left.lines) {
      node = left;
      continue;
    }
    remaining -= left.lines;
    start8 += left.w8;
    start16 += left.w16;
    node = node.right;
  }
  if (remaining < 0 || remaining >= node.lines) throw new Error("lsp.invalid_position: line is outside document");
  for (let index = 0; index < remaining; index += 1) {
    start8 += node.l8[index];
    start16 += node.l16[index];
  }
  return { line, start8, start16, w8: node.l8[remaining], w16: node.l16[remaining], text: node.texts[remaining] };
}

function locateByte(root, offset) {
  let node = root;
  let line = 0;
  let start8 = 0;
  let start16 = 0;
  let remaining = offset;
  while (node.kind === "branch") {
    const left = node.left;
    if (remaining < left.w8) {
      node = left;
      continue;
    }
    remaining -= left.w8;
    line += left.lines;
    start8 += left.w8;
    start16 += left.w16;
    node = node.right;
  }
  for (let index = 0; index < node.lines; index += 1) {
    if (remaining < node.l8[index]) {
      return {
        line: line + index,
        start8,
        start16,
        w8: node.l8[index],
        w16: node.l16[index],
        text: node.texts[index],
        intra8: remaining,
      };
    }
    remaining -= node.l8[index];
    start8 += node.l8[index];
    start16 += node.l16[index];
  }
  throw new Error("lsp.invalid_position: byte offset is outside document");
}

function collectLines(node, lines) {
  if (node.kind === "branch") {
    collectLines(node.left, lines);
    collectLines(node.right, lines);
    return lines;
  }
  for (const text of node.texts) lines.push(text);
  return lines;
}

/** Line text without a trailing CR (its convertible content). */
function strippedLine(text) {
  return text.length > 0 && text.charCodeAt(text.length - 1) === 13 ? text.slice(0, -1) : text;
}

/** Walks `content` up to line-relative byte `intra8`.
 *
 * Returns `{ char16, scalars }`, or `null` when `intra8` points into the
 * stripped line ending (CR/LF). Throws when the byte splits a code point.
 *
 * ponytail: a line longer than one scan walks its whole length per
 * conversion, as the baseline did; port the frontend's scan blocks if a
 * 1 MiB single line ever becomes hot.
 */
function scanLine(content, intra8) {
  let seen8 = 0;
  let char16 = 0;
  let scalars = 0;
  while (char16 < content.length) {
    if (seen8 === intra8) return { char16, scalars };
    const code = content.codePointAt(char16);
    const width16 = code > 0xffff ? 2 : 1;
    const width8 = code <= 0x7f ? 1 : code <= 0x7ff ? 2 : code <= 0xffff ? 3 : 4;
    if (seen8 + width8 > intra8) throw new Error("lsp.invalid_position: byte offset splits a UTF-8 code point");
    seen8 += width8;
    char16 += width16;
    scalars += 1;
  }
  return seen8 === intra8 ? { char16, scalars } : null;
}

export class VersionedDocument {
  #root = null;
  #text = null;
  #bytes = null;

  constructor(text, version, encoding = "utf-16") {
    requireEncoding(encoding);
    this.encoding = encoding;
    this.reset(text, version);
  }

  reset(text, version) {
    if (typeof text !== "string" || !Number.isInteger(version) || version < 0) {
      throw new Error("lsp.invalid_document: text and non-negative integer version are required");
    }
    this.#text = text;
    this.#bytes = null;
    this.version = version;
    this.#root = buildTree(text.split("\n"));
  }

  get text() {
    if (this.#text === null) this.#text = collectLines(this.#root, []).join("\n");
    return this.#text;
  }

  get bytes() {
    if (this.#bytes === null) this.#bytes = encodeUtf8(this.text);
    return this.#bytes;
  }

  // UTF-8 byte length without materializing the whole byte array (the
  // viewport/range bounds adapters need on every refresh).
  get byteLength() {
    return this.#root.w8 - 1;
  }

  #locate(offset) {
    if (!Number.isInteger(offset) || offset < 0 || offset > this.byteLength) {
      throw new Error("lsp.invalid_position: byte offset is outside document");
    }
    const located = locateByte(this.#root, offset);
    const content = strippedLine(located.text);
    return { ...located, scanned: scanLine(content, located.intra8) };
  }

  #character(located) {
    if (this.encoding === "utf-8") return located.intra8;
    if (this.encoding === "utf-16") return located.scanned.char16;
    return located.scanned.scalars;
  }

  #lineBounds(line) {
    if (!Number.isInteger(line) || line < 0) {
      throw new Error("lsp.invalid_position: line is outside document");
    }
    const located = locateLine(this.#root, line);
    const content = strippedLine(located.text);
    return {
      start: located.start8,
      end: located.start8 + located.w8 - 1 - (located.text.length - content.length),
      text: content,
    };
  }

  byteToPosition(offset) {
    const located = this.#locate(offset);
    if (located.scanned === null) throw new Error("lsp.invalid_position: byte offset points into line ending");
    return { line: located.line, character: this.#character(located) };
  }

  positionToByte(position) {
    if (position === null || typeof position !== "object") throw new Error("lsp.invalid_position: position object required");
    const { start, text } = this.#lineBounds(position.line);
    const stringIndex = stringIndexForUnits(text, position.character, this.encoding);
    return start + utf8ByteLength(text.slice(0, stringIndex));
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
    const start = this.#locate(byteStart);
    const end = this.#locate(byteEnd);
    if (byteStart > byteEnd || typeof text !== "string") throw new Error("lsp.invalid_change: invalid byte range or text");
    if (start.scanned === null || end.scanned === null) {
      throw new Error("lsp.invalid_position: byte offset points into line ending");
    }
    const range = {
      start: { line: start.line, character: this.#character(start) },
      end: { line: end.line, character: this.#character(end) },
    };
    const prefix = start.text.slice(0, start.scanned.char16);
    const suffix = end.text.slice(end.scanned.char16);
    this.#root = replaceRange(this.#root, start.line, end.line - start.line + 1, (prefix + text + suffix).split("\n"));
    this.#text = null;
    this.#bytes = null;
    this.version = version;
    return range;
  }
}
