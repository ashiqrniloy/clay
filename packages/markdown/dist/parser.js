const DEFAULT_PACKAGE_NAME = "@clay/markdown";
const DEFAULT_PACKAGE_VERSION = "0.1.0";
const DEFAULT_API_PREFIX = "markdown";

export const MARKDOWN_IT_OPTIONS = Object.freeze({
  html: false,
  linkify: false,
  typographer: false
});

export const DEFAULT_WINDOWED_MARKDOWN_POLICY = Object.freeze({
  largeFileThresholdBytes: 5 * 1024 * 1024,
  parseWindowBytes: 64 * 1024,
  guardBytes: 4 * 1024,
  memoryBudgetBytes: 30 * 1024 * 1024,
  timeoutMs: 5000
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

function toFiniteNumber(value, fallback) {
  const number = Number(value ?? fallback);
  return Number.isFinite(number) ? number : fallback;
}

function buildSourceIndex(text, metadata = {}) {
  const absoluteByteStart = Math.max(0, Math.trunc(toFiniteNumber(metadata.absoluteByteStart, 0)));
  const baseLine = Math.max(0, Math.trunc(toFiniteNumber(metadata.baseLine, 0)));
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
  return {
    text,
    codeUnitToByte,
    lineCodeUnitStarts,
    lineByteStarts,
    totalBytes: byteOffset,
    absoluteByteStart,
    absoluteByteEnd: absoluteByteStart + byteOffset,
    baseLine
  };
}

function codeUnitToWindowByte(source, codeUnitOffset) {
  const safeOffset = Math.max(0, Math.min(source.text.length, Number(codeUnitOffset ?? 0)));
  return source.codeUnitToByte[safeOffset] ?? source.totalBytes;
}

function codeUnitToAbsoluteByte(source, codeUnitOffset) {
  return source.absoluteByteStart + codeUnitToWindowByte(source, codeUnitOffset);
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

function windowMetadataFromOptions(options, text, totalBytes) {
  const parseWindow = options.parseWindow ?? options.window ?? null;
  const absoluteByteStart = Math.max(0, Math.trunc(toFiniteNumber(
    options.absoluteByteStart ?? options.byteStart ?? parseWindow?.byteStart ?? parseWindow?.start,
    0
  )));
  const declaredByteEnd = parseWindow?.byteEnd ?? parseWindow?.end;
  const absoluteByteEnd = declaredByteEnd === undefined
    ? absoluteByteStart + totalBytes
    : Math.max(0, Math.trunc(toFiniteNumber(declaredByteEnd, absoluteByteStart + totalBytes)));
  if (absoluteByteEnd < absoluteByteStart) {
    throw new Error("markdown.parser.invalid_parse_window: byteStart must be <= byteEnd");
  }
  if (absoluteByteEnd - absoluteByteStart !== totalBytes) {
    throw new Error("markdown.parser.invalid_parse_window: window byte range must match UTF-8 text length");
  }
  return {
    absoluteByteStart,
    baseLine: Math.max(0, Math.trunc(toFiniteNumber(options.baseLine ?? parseWindow?.baseLine, 0)))
  };
}

function viewportNumbers(viewport, fallbackStart, fallbackEnd) {
  const byteStart = Math.trunc(toFiniteNumber(viewport?.byteStart ?? viewport?.start, fallbackStart));
  const byteEnd = Math.trunc(toFiniteNumber(viewport?.byteEnd ?? viewport?.end, fallbackEnd));
  if (byteStart > byteEnd) {
    throw new Error("markdown.parser.invalid_viewport: byteStart must be <= byteEnd");
  }
  return { byteStart, byteEnd };
}

function normalizeViewport(viewport, source) {
  const requested = viewportNumbers(viewport, source.absoluteByteStart, source.absoluteByteEnd);
  return {
    byteStart: Math.max(source.absoluteByteStart, Math.min(source.absoluteByteEnd, requested.byteStart)),
    byteEnd: Math.max(source.absoluteByteStart, Math.min(source.absoluteByteEnd, requested.byteEnd))
  };
}

function spanIntersectsViewport(span, viewport) {
  return span.byteStart < viewport.byteEnd && span.byteEnd > viewport.byteStart;
}

function pushSpan(spans, span, viewport) {
  if (span.byteStart >= span.byteEnd || !spanIntersectsViewport(span, viewport)) return;
  spans.push({
    ...span,
    byteStart: Math.max(span.byteStart, viewport.byteStart),
    byteEnd: Math.min(span.byteEnd, viewport.byteEnd)
  });
}

function syntaxSpan(source, codeUnitStart, codeUnitEnd, styleToken, priority) {
  return {
    byteStart: codeUnitToAbsoluteByte(source, codeUnitStart),
    byteEnd: codeUnitToAbsoluteByte(source, codeUnitEnd),
    kind: "syntax",
    styleToken,
    ...(styleToken === STYLE_TOKENS.inlineCode || styleToken === STYLE_TOKENS.codeBlock
      ? { fontRole: "monospace" }
      : {}),
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

function sortSpans(spans) {
  spans.sort((left, right) =>
    right.priority - left.priority ||
    left.byteStart - right.byteStart ||
    left.byteEnd - right.byteEnd ||
    left.styleToken.localeCompare(right.styleToken)
  );
  return spans;
}

function dedupeSortedSpans(spans) {
  sortSpans(spans);
  const deduped = [];
  let previousKey = null;
  for (const span of spans) {
    const key = `${span.byteStart}:${span.byteEnd}:${span.kind}:${span.styleToken}:${span.priority}`;
    if (key !== previousKey) deduped.push(span);
    previousKey = key;
  }
  return deduped;
}

async function defaultMarkdownIt() {
  try {
    const module = await import("markdown-it");
    const MarkdownIt = module.default ?? module;
    return new MarkdownIt(MARKDOWN_IT_OPTIONS);
  } catch {
    return basicMarkdownIt();
  }
}

function basicMarkdownIt() {
  return {
    parse(text) {
      const tokens = [];
      const lines = String(text ?? "").split("\n");
      let inFence = false;
      let fenceStart = 0;
      let fenceMarker = "```";
      for (let line = 0; line < lines.length; line += 1) {
        const value = lines[line];
        const fence = /^( {0,3})(`{3,}|~{3,})/.exec(value);
        if (fence) {
          if (!inFence) {
            inFence = true;
            fenceStart = line;
            fenceMarker = fence[2];
          } else if (value.trimStart().startsWith(fenceMarker)) {
            tokens.push({ type: "fence", map: [fenceStart, line + 1], markup: fenceMarker });
            inFence = false;
          }
          continue;
        }
        if (inFence) continue;
        const heading = /^( {0,3})(#{1,6})(?:[ \t]+|$)/.exec(value);
        if (heading) tokens.push(basicToken("heading_open", { tag: `h${heading[2].length}`, map: [line, line + 1] }));
        if (/^[ \t]*(?:[-+*]|\d+[.)])(?=[ \t])/.test(value)) tokens.push(basicToken("list_item_open", { map: [line, line + 1] }));
        tokens.push(basicToken("inline", { map: [line, line + 1], content: value, children: basicInlineChildren(value) }));
      }
      return tokens;
    }
  };
}

function basicToken(type, fields = {}) {
  return { ...fields, type };
}

function basicInlineChildren(value) {
  const children = [];
  const pattern = /(\*\*[^*]+\*\*)|(\*[^*]+\*)|(`[^`]+`)/g;
  let match;
  while ((match = pattern.exec(value)) !== null) {
    if (match[1]) children.push({ type: "strong_open", markup: "**" }, { type: "text", content: match[1].slice(2, -2) }, { type: "strong_close", markup: "**" });
    if (match[2]) children.push({ type: "em_open", markup: "*" }, { type: "text", content: match[2].slice(1, -1) }, { type: "em_close", markup: "*" });
    if (match[3]) children.push({ type: "code_inline", markup: "`", content: match[3].slice(1, -1) });
  }
  return children;
}

async function parseMarkdownItTokens(text, options) {
  if (Array.isArray(options.tokens)) return options.tokens;
  const markdownIt = options.markdownIt ?? await defaultMarkdownIt();
  if (typeof markdownIt.parse !== "function") {
    throw new Error("markdown.parser.invalid_markdown_it: parser must expose parse(text, env)");
  }
  return markdownIt.parse(text, {});
}

function parseWindowInputs(options) {
  const windows = options.parseWindows ?? options.parse_windows;
  return Array.isArray(windows) ? windows : null;
}

function syntaxBudgetBytes(options = {}) {
  return Math.max(0, Math.trunc(toFiniteNumber(
    options.memoryBudgetBytes ?? options.policy?.memoryBudgetBytes,
    DEFAULT_WINDOWED_MARKDOWN_POLICY.memoryBudgetBytes
  )));
}

function parseInputByteLength(options = {}) {
  const windows = parseWindowInputs(options);
  if (windows) {
    let total = 0;
    for (const window of windows) {
      total += buildSourceIndex(String(window?.text ?? "")).totalBytes;
    }
    return total;
  }
  return buildSourceIndex(String(options.text ?? "")).totalBytes;
}

function plainTextFallbackReason(options = {}) {
  if (options.fallbackMode === "plain-text-fallback" || options.highlightingState === "plain-text-fallback") {
    return "requested";
  }
  if (options.budgetExceeded === true || options.syntaxBudgetExceeded === true || options.memoryBudgetExceeded === true) {
    return "budget-exceeded";
  }
  const budget = syntaxBudgetBytes(options);
  if (budget === 0) return "budget-exceeded";
  if (parseInputByteLength(options) > budget) return "budget-exceeded";
  return null;
}

function fallbackStatus(reason) {
  return reason ? {
    highlightingState: "plain-text-fallback",
    reason,
    parse: "plain text fallback; Markdown parser paused",
    decorations: "plain text fallback; syntax decorations cleared"
  } : null;
}

function normalizeWindowOptions(options = {}) {
  const text = String(options.text ?? "");
  const provisional = buildSourceIndex(text);
  const metadata = windowMetadataFromOptions(options, text, provisional.totalBytes);
  const source = buildSourceIndex(text, metadata);
  return { text, source, viewport: normalizeViewport(options.viewport, source) };
}

export async function parseMarkdownWindowDecorations(options = {}) {
  const { text, source, viewport } = normalizeWindowOptions(options);
  const tokens = await parseMarkdownItTokens(text, options);
  const spans = [];

  for (const token of tokens) {
    if (token?.hidden) continue;
    addHeadingSpan(spans, token, source, viewport);
    addFenceSpan(spans, token, source, viewport);
    addListMarkerSpan(spans, token, source, viewport);
    walkMarkdownItInlineChildren(token, source, viewport, spans);
  }

  return dedupeSortedSpans(spans);
}

async function parseMarkdownWindowSetDecorations(options = {}) {
  const windows = parseWindowInputs(options) ?? [];
  const spans = [];
  for (const window of windows) {
    if (!window || typeof window !== "object") continue;
    const byteStart = window.byteStart ?? window.byte_start;
    const byteEnd = window.byteEnd ?? window.byte_end;
    const baseLine = window.baseLine ?? window.base_line;
    spans.push(...await parseMarkdownWindowDecorations({
      ...options,
      text: String(window.text ?? ""),
      tokens: window.tokens,
      absoluteByteStart: byteStart,
      baseLine,
      parseWindow: { byteStart, byteEnd, baseLine },
      viewport: options.viewport ?? window.viewport,
      blockContext: window.blockContext ?? window.block_context ?? options.blockContext
    }));
  }
  return dedupeSortedSpans(spans);
}

export async function parseMarkdownDecorations(options = {}) {
  if (plainTextFallbackReason(options)) return [];
  if (parseWindowInputs(options)) {
    return parseMarkdownWindowSetDecorations(options);
  }
  return parseMarkdownWindowDecorations(options);
}

function updateViewportFromOptions(options = {}) {
  const windows = parseWindowInputs(options);
  if (options.viewport) {
    return viewportNumbers(options.viewport, 0, 0);
  }
  if (windows && windows.length > 0) {
    let byteStart = Number.POSITIVE_INFINITY;
    let byteEnd = 0;
    for (const window of windows) {
      const start = Math.max(0, Math.trunc(toFiniteNumber(window?.byteStart ?? window?.byte_start, 0)));
      const text = String(window?.text ?? "");
      const provisional = buildSourceIndex(text);
      const end = Math.trunc(toFiniteNumber(window?.byteEnd ?? window?.byte_end, start + provisional.totalBytes));
      byteStart = Math.min(byteStart, start);
      byteEnd = Math.max(byteEnd, end);
    }
    if (byteStart !== Number.POSITIVE_INFINITY) return { byteStart, byteEnd };
  }
  const { viewport } = normalizeWindowOptions(options);
  return {
    byteStart: viewport.byteStart,
    byteEnd: viewport.byteEnd
  };
}

export async function parseMarkdownDecorationUpdate(options = {}) {
  const fallback = fallbackStatus(plainTextFallbackReason(options));
  const spans = fallback ? [] : await parseMarkdownDecorations(options);
  const viewport = updateViewportFromOptions(options);
  return {
    documentId: Number(options.documentId),
    documentVersion: Number(options.documentVersion),
    behaviorVersion: Number(options.behaviorVersion ?? 0),
    packagePrefix: options.packagePrefix ?? options.apiPrefix ?? DEFAULT_API_PREFIX,
    mode: options.mode ?? "markdown",
    viewport,
    spans,
    status: fallback ?? {
      highlightingState: parseWindowInputs(options) ? "windowed" : "full",
      parse: parseWindowInputs(options) ? "windowed visible syntax current" : "full document syntax current",
      decorations: parseWindowInputs(options) ? "visible and near-viewport chunks current" : "full document decorations current"
    }
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
