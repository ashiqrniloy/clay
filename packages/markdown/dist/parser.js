const DEFAULT_PACKAGE_NAME = "@clay/markdown";
const DEFAULT_PACKAGE_VERSION = "0.1.0";
const DEFAULT_API_PREFIX = "markdown";

const MARKDOWN_IT_OPTIONS = Object.freeze({
  html: false,
  linkify: false,
  typographer: false
});

const STYLE_TOKENS = Object.freeze({
  strong: "markup.strong",
  emphasis: "markup.emphasis",
  inlineCode: "markup.inline-code",
  codeBlock: "markup.code-block",
  listMarker: "markup.list-marker"
});

const HEADING_STYLE_TOKENS = Object.freeze({
  1: "markup.heading.1",
  2: "markup.heading.2",
  3: "markup.heading.3",
  4: "markup.heading.4",
  5: "markup.heading.5",
  6: "markup.heading.6"
});

function utf8ByteLengthForCodePoint(codePoint) {
  if (codePoint <= 0x7f) return 1;
  if (codePoint <= 0x7ff) return 2;
  if (codePoint <= 0xffff) return 3;
  return 4;
}

function buildSourceIndex(text) {
  const codeUnitToByte = new Array(text.length + 1);
  codeUnitToByte[0] = 0;
  const lineCodeUnitStarts = [0];
  let byteOffset = 0;

  for (let index = 0; index < text.length;) {
    const codePoint = text.codePointAt(index);
    const codeUnitWidth = codePoint > 0xffff ? 2 : 1;
    byteOffset += utf8ByteLengthForCodePoint(codePoint);
    const next = index + codeUnitWidth;
    for (let fill = index + 1; fill <= next; fill += 1) {
      codeUnitToByte[fill] = byteOffset;
    }
    if (codePoint === 0x0a) {
      lineCodeUnitStarts.push(next);
    }
    index = next;
  }

  const lineByteStarts = lineCodeUnitStarts.map((start) => codeUnitToByte[start] ?? byteOffset);
  return { text, codeUnitToByte, lineCodeUnitStarts, lineByteStarts, totalBytes: byteOffset };
}

function codeUnitToByte(source, codeUnitOffset) {
  const safeOffset = Math.max(0, Math.min(source.text.length, Number(codeUnitOffset ?? 0)));
  return source.codeUnitToByte[safeOffset] ?? source.totalBytes;
}

function lineStartCodeUnit(source, lineNumber) {
  const safeLine = Math.max(0, Math.min(source.lineCodeUnitStarts.length - 1, Number(lineNumber ?? 0)));
  return source.lineCodeUnitStarts[safeLine] ?? 0;
}

function lineEndCodeUnit(source, lineNumber) {
  const start = lineStartCodeUnit(source, lineNumber);
  const newline = source.text.indexOf("\n", start);
  return newline === -1 ? source.text.length : newline;
}

function lineText(source, lineNumber) {
  const start = lineStartCodeUnit(source, lineNumber);
  const end = lineEndCodeUnit(source, lineNumber);
  return { start, end, text: source.text.slice(start, end) };
}

function tokenLineRange(token) {
  if (!Array.isArray(token?.map) || token.map.length < 2) return null;
  const startLine = Number(token.map[0]);
  const endLine = Number(token.map[1]);
  if (!Number.isInteger(startLine) || !Number.isInteger(endLine) || endLine <= startLine) return null;
  return { startLine, endLine };
}

function tokenSourceRange(source, token) {
  const range = tokenLineRange(token);
  if (!range) return null;
  const start = lineStartCodeUnit(source, range.startLine);
  const end = range.endLine < source.lineCodeUnitStarts.length
    ? lineStartCodeUnit(source, range.endLine)
    : source.text.length;
  return { start, end };
}

function normalizeViewport(viewport, source) {
  const byteStart = Math.max(0, Math.min(source.totalBytes, Number(viewport?.byteStart ?? 0)));
  const byteEnd = Math.max(0, Math.min(source.totalBytes, Number(viewport?.byteEnd ?? source.totalBytes)));
  return { byteStart, byteEnd };
}

function spanInsideViewport(span, viewport) {
  return span.byteStart >= viewport.byteStart && span.byteEnd <= viewport.byteEnd;
}

function pushSpan(spans, span, viewport) {
  if (span.byteStart < span.byteEnd && spanInsideViewport(span, viewport)) {
    spans.push(span);
  }
}

function syntaxSpan(source, codeUnitStart, codeUnitEnd, styleToken, priority) {
  return {
    byteStart: codeUnitToByte(source, codeUnitStart),
    byteEnd: codeUnitToByte(source, codeUnitEnd),
    kind: "syntax",
    styleToken,
    priority
  };
}

function firstNonWhitespaceOffset(text) {
  const match = /\S/.exec(text);
  return match ? match.index : 0;
}

function addHeadingSpan(spans, token, source, viewport) {
  if (token?.type !== "heading_open") return;
  const range = tokenLineRange(token);
  if (!range) return;
  const heading = lineText(source, range.startLine);
  const match = /^( {0,3})(#{1,6})(?:[ \t]+|$)/.exec(heading.text);
  if (!match) return;
  const depth = Math.max(1, Math.min(6, Number(token.tag?.slice(1)) || match[2].length));
  const closingSequenceMatch = /(?:[ \t]+#+)?[ \t]*$/.exec(heading.text);
  const contentEnd = heading.end - (closingSequenceMatch?.[0]?.length ?? 0);
  const spanEnd = Math.max(heading.start + match[1].length + match[2].length, contentEnd);
  pushSpan(
    spans,
    syntaxSpan(source, heading.start + match[1].length, spanEnd, HEADING_STYLE_TOKENS[depth], 90 - depth),
    viewport
  );
}

function addFenceSpan(spans, token, source, viewport) {
  if (token?.type !== "fence") return;
  const range = tokenSourceRange(source, token);
  if (!range) return;
  const openingLine = lineText(source, token.map[0]);
  const marker = typeof token.markup === "string" && token.markup.length > 0 ? token.markup : null;
  if (marker && !openingLine.text.slice(firstNonWhitespaceOffset(openingLine.text)).startsWith(marker)) return;
  pushSpan(spans, syntaxSpan(source, range.start, range.end, STYLE_TOKENS.codeBlock, 70), viewport);
}

function addListMarkerSpan(spans, token, source, viewport) {
  if (token?.type !== "list_item_open") return;
  const range = tokenLineRange(token);
  if (!range) return;
  const line = lineText(source, range.startLine);
  const markerMatch = /^[ \t]*(?:[-+*]|\d+[.)])(?=[ \t])/.exec(line.text);
  if (!markerMatch) return;
  const markerOffset = markerMatch[0].search(/[-+*\d]/);
  const markerStart = line.start + markerOffset;
  const markerEnd = line.start + markerMatch[0].trimEnd().length;
  pushSpan(spans, syntaxSpan(source, markerStart, markerEnd, STYLE_TOKENS.listMarker, 80), viewport);
}

function inlineSourceBase(token, source) {
  const range = tokenSourceRange(source, token);
  if (!range) return null;
  const inlineContent = String(token.content ?? "");
  if (inlineContent.length === 0) return range.start;
  const blockSource = source.text.slice(range.start, range.end);
  const contentOffset = blockSource.indexOf(inlineContent);
  if (contentOffset >= 0) return range.start + contentOffset;

  // Fallback for unusual containers whose token.content is normalized by
  // markdown-it: start at the first visible character in the mapped source
  // range and keep parser details inside this package adapter.
  const visibleOffset = firstNonWhitespaceOffset(blockSource);
  return range.start + visibleOffset;
}

function markerForInlineToken(token, fallback) {
  return typeof token?.markup === "string" && token.markup.length > 0 ? token.markup : fallback;
}

function findDelimitedRange(inlineContent, marker, content, cursor) {
  const openStart = inlineContent.indexOf(marker, cursor);
  if (openStart < 0) return null;
  const afterOpen = openStart + marker.length;

  let closeSearchStart = afterOpen;
  if (content.length > 0) {
    const contentStart = inlineContent.indexOf(content, afterOpen);
    if (contentStart >= 0) {
      closeSearchStart = contentStart + content.length;
    }
  }

  const closeStart = inlineContent.indexOf(marker, closeSearchStart);
  if (closeStart < 0) return null;
  return { start: openStart, end: closeStart + marker.length };
}

function advanceCursorToContent(inlineContent, content, cursor) {
  if (content.length === 0) return cursor;
  const start = inlineContent.indexOf(content, cursor);
  return start < 0 ? cursor : start + content.length;
}

function popInlineStack(stack, style) {
  for (let index = stack.length - 1; index >= 0; index -= 1) {
    if (stack[index].style === style) {
      return stack.splice(index, 1)[0];
    }
  }
  return null;
}

function walkMarkdownItInlineChildren(token, source, viewport, spans) {
  if (token?.type !== "inline" || !Array.isArray(token.children) || token.children.length === 0) return;
  const base = inlineSourceBase(token, source);
  if (base === null) return;

  const inlineContent = String(token.content ?? "");
  const stack = [];
  let cursor = 0;

  for (const child of token.children) {
    if (!child || child.hidden) continue;

    if (child.type === "strong_open" || child.type === "em_open") {
      const style = child.type === "strong_open" ? STYLE_TOKENS.strong : STYLE_TOKENS.emphasis;
      const marker = markerForInlineToken(child, child.type === "strong_open" ? "**" : "*");
      const openStart = inlineContent.indexOf(marker, cursor);
      if (openStart >= 0) {
        stack.push({ style, marker, start: openStart });
        cursor = openStart + marker.length;
      }
      continue;
    }

    if (child.type === "strong_close" || child.type === "em_close") {
      const style = child.type === "strong_close" ? STYLE_TOKENS.strong : STYLE_TOKENS.emphasis;
      const opener = popInlineStack(stack, style);
      const marker = markerForInlineToken(child, opener?.marker ?? (child.type === "strong_close" ? "**" : "*"));
      const closeStart = inlineContent.indexOf(marker, cursor);
      if (opener && closeStart >= 0) {
        const end = closeStart + marker.length;
        pushSpan(spans, syntaxSpan(source, base + opener.start, base + end, style, style === STYLE_TOKENS.strong ? 60 : 50), viewport);
        cursor = end;
      }
      continue;
    }

    if (child.type === "code_inline") {
      const marker = markerForInlineToken(child, "`");
      const content = String(child.content ?? "");
      const range = findDelimitedRange(inlineContent, marker, content, cursor);
      if (range) {
        pushSpan(spans, syntaxSpan(source, base + range.start, base + range.end, STYLE_TOKENS.inlineCode, 65), viewport);
        cursor = range.end;
      }
      continue;
    }

    if (typeof child.content === "string") {
      cursor = advanceCursorToContent(inlineContent, child.content, cursor);
    }
  }
}

async function defaultMarkdownIt() {
  const module = await import("markdown-it");
  const MarkdownIt = module.default ?? module;
  return new MarkdownIt(MARKDOWN_IT_OPTIONS);
}

async function parseMarkdownItTokens(text, options) {
  if (Array.isArray(options.tokens)) return options.tokens;
  const markdownIt = options.markdownIt ?? await defaultMarkdownIt();
  if (typeof markdownIt.parse !== "function") {
    throw new Error("markdown.parser.invalid_markdown_it: parser must expose parse(text, env)");
  }
  return markdownIt.parse(text, {});
}

export async function parseMarkdownDecorations(options = {}) {
  const text = String(options.text ?? "");
  const source = buildSourceIndex(text);
  const viewport = normalizeViewport(options.viewport, source);
  if (viewport.byteStart > viewport.byteEnd) {
    throw new Error("markdown.parser.invalid_viewport: byteStart must be <= byteEnd");
  }

  const tokens = await parseMarkdownItTokens(text, options);
  const spans = [];

  for (const token of tokens) {
    if (token?.hidden) continue;
    addHeadingSpan(spans, token, source, viewport);
    addFenceSpan(spans, token, source, viewport);
    addListMarkerSpan(spans, token, source, viewport);
    walkMarkdownItInlineChildren(token, source, viewport, spans);
  }

  spans.sort((left, right) =>
    right.priority - left.priority ||
    left.byteStart - right.byteStart ||
    left.byteEnd - right.byteEnd ||
    left.styleToken.localeCompare(right.styleToken)
  );

  return spans;
}

export async function parseMarkdownDecorationUpdate(options = {}) {
  const text = String(options.text ?? "");
  const source = buildSourceIndex(text);
  const viewport = normalizeViewport(options.viewport, source);
  const spans = await parseMarkdownDecorations({ ...options, text, viewport });
  return {
    documentId: Number(options.documentId),
    documentVersion: Number(options.documentVersion),
    behaviorVersion: Number(options.behaviorVersion ?? 0),
    packagePrefix: options.packagePrefix ?? options.apiPrefix ?? DEFAULT_API_PREFIX,
    mode: options.mode ?? "markdown",
    viewport,
    spans
  };
}

export async function publishMarkdownDecorations(clay, options = {}) {
  const update = await parseMarkdownDecorationUpdate(options);
  return clay.decorations.serverPublishDecorations({
    packageName: options.packageName ?? DEFAULT_PACKAGE_NAME,
    packageVersion: options.packageVersion ?? DEFAULT_PACKAGE_VERSION,
    packagePrefix: update.packagePrefix,
    permissions: ["render-decorations"],
    documentId: update.documentId,
    documentVersion: update.documentVersion,
    currentDocumentVersion: Number(options.currentDocumentVersion ?? update.documentVersion),
    behaviorVersion: update.behaviorVersion,
    viewport: update.viewport,
    spans: update.spans
  });
}
