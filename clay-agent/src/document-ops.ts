/**
 * Clay document operations served through daemon→server reverse RPC.
 *
 * The read/write/edit tool operations never touch the local filesystem for
 * documents inside the Clay registry: every call round-trips to the Rust
 * server, which owns versions, leases, and dirty buffers. `document.read`
 * returns the dirty buffer for open documents; writes go through the
 * server's lease/CAS path.
 *
 * ponytail: `document.read` returns the full (server-capped) text and paging
 * happens here. A server-side paged read is the upgrade if full-file reads
 * become a hot path.
 */
import type {
  EditOperations,
  ReadOperations,
  ReadTextOptions,
  ReadTextResult,
  WriteOperations,
} from "@arnilo/prism-coding-tools/agent";

export type ReverseRequest = (
  method: string,
  params: Record<string, unknown>,
) => Promise<unknown>;

export interface ClayDocumentOps {
  readonly read: ReadOperations;
  readonly write: WriteOperations;
  readonly edit: EditOperations;
}

interface DocumentReadResult {
  readonly text: string;
  readonly version: number;
  readonly dirty: boolean;
  readonly open: boolean;
  /** The server cut a resident buffer at the read cap (plan 119 P2-1). */
  readonly truncated?: boolean;
  /** Full document size in bytes, even when `truncated`. */
  readonly totalBytes?: number;
}

interface DocumentWriteResult {
  readonly version: number;
  readonly saved: boolean;
  readonly open: boolean;
}

async function readDocument(
  request: ReverseRequest,
  sessionId: string,
  absolutePath: string,
  maxBytes: number,
  signal?: AbortSignal,
): Promise<DocumentReadResult> {
  if (signal?.aborted) throw new Error("Operation aborted");
  const result = (await request("document.read", {
    path: absolutePath,
    maxBytes,
    sessionId,
  })) as DocumentReadResult;
  if (signal?.aborted) throw new Error("Operation aborted");
  if (!result || typeof result.text !== "string") {
    throw new Error("document.read must return { text, version, dirty, open }");
  }
  // The server cap is the bound (a resident document is 256MB); the callers'
  // byte ceilings are defense in depth behind it. Fail loudly rather than page
  // or scan a prefix as if it were the whole file — that would read as EOF to
  // the model.
  if (result.truncated === true) {
    throw new Error(
      `Document is ${result.totalBytes ?? maxBytes} bytes, exceeds the ${maxBytes} byte read cap`,
    );
  }
  return result;
}

/** Mirrors the local/acp text paging contract so the read tool sees identical pages. */
function readPage(text: string, options: ReadTextOptions): ReadTextResult {
  const requestedLines = options.limit ?? options.maxLines;
  const lines = text.split("\n");
  if (lines.length > 1 && lines[lines.length - 1] === "") lines.pop();
  const totalBytes = Buffer.byteLength(text, "utf8");
  if (totalBytes > options.maxScanBytes) {
    throw new Error(`Text read exceeded ${options.maxScanBytes} byte scan limit`);
  }
  const start = Math.max(0, options.offset - 1);
  if (start >= lines.length) {
    throw new Error(`Offset ${options.offset} is beyond end of file (${lines.length} lines total)`);
  }
  const output: string[] = [];
  let outputBytes = 0;
  let truncatedBy: "lines" | "bytes" | null = null;
  let firstLineExceedsLimit = false;
  for (let i = start; i < lines.length; i++) {
    if (output.length >= requestedLines) {
      truncatedBy = "lines";
      break;
    }
    const line = lines[i];
    const lineBytes = Buffer.byteLength(line, "utf8");
    if (output.length === 0 && lineBytes > options.maxBytes) {
      firstLineExceedsLimit = true;
      truncatedBy = "bytes";
      break;
    }
    const withSeparator = output.length === 0 ? lineBytes : lineBytes + 1;
    if (outputBytes + withSeparator > options.maxBytes) {
      truncatedBy = "bytes";
      break;
    }
    output.push(line);
    outputBytes += withSeparator;
  }
  const consumed = start + output.length;
  const hasMore = firstLineExceedsLimit || consumed < lines.length;
  const nextOffset = !hasMore
    ? undefined
    : firstLineExceedsLimit
      ? options.offset
      : options.offset + output.length;
  return {
    content: firstLineExceedsLimit ? "" : output.join("\n"),
    startLine: options.offset,
    outputLines: firstLineExceedsLimit ? 0 : output.length,
    hasMore,
    nextOffset,
    truncatedBy,
    firstLineExceedsLimit,
    scannedBytes: totalBytes,
    totalLines: lines.length,
    totalBytes,
  };
}

/**
 * Build the document operations for one session. Every call names that
 * session: the server resolves the file's workspace root from it (plan 119
 * SC-6), so a tool call can only touch the folder its session owns.
 */
export function createClayDocumentOps(options: {
  request: ReverseRequest;
  sessionId: string;
}): ClayDocumentOps {
  const request = options.request;
  const sessionId = options.sessionId;

  const readFile = async (
    absolutePath: string,
    options: { maxBytes: number; signal?: AbortSignal },
  ): Promise<Buffer> => {
    const doc = await readDocument(
      request,
      sessionId,
      absolutePath,
      options.maxBytes,
      options.signal,
    );
    const buffer = Buffer.from(doc.text, "utf8");
    if (buffer.byteLength > options.maxBytes) {
      throw new Error(`File is ${buffer.byteLength} bytes, exceeds ${options.maxBytes} byte limit`);
    }
    return buffer;
  };

  const statFile = async (
    absolutePath: string,
    options?: { signal?: AbortSignal },
  ): Promise<{ size: number }> => {
    if (options?.signal?.aborted) throw new Error("Operation aborted");
    const result = (await request("document.stat", {
      path: absolutePath,
      sessionId,
    })) as { size: number };
    if (!result || typeof result.size !== "number") {
      throw new Error("document.stat must return { size }");
    }
    return { size: result.size };
  };

  const access = async (absolutePath: string, options?: { signal?: AbortSignal }): Promise<void> => {
    await statFile(absolutePath, options);
  };

  const writeFile = async (
    absolutePath: string,
    content: string,
    options?: { maxBytes?: number; signal?: AbortSignal },
  ): Promise<void> => {
    if (options?.signal?.aborted) throw new Error("Operation aborted");
    const bytes = Buffer.byteLength(content, "utf8");
    if (options?.maxBytes !== undefined && bytes > options.maxBytes) {
      throw new Error(`Write input is ${bytes} bytes, exceeds ${options.maxBytes} byte limit`);
    }
    const result = (await request("document.write", {
      path: absolutePath,
      content,
      sessionId,
    })) as DocumentWriteResult;
    if (!result || typeof result.version !== "number") {
      throw new Error("document.write must return { version }");
    }
  };

  const mkdir = async (dir: string, options?: { signal?: AbortSignal }): Promise<void> => {
    if (options?.signal?.aborted) throw new Error("Operation aborted");
    await request("document.mkdir", { path: dir, sessionId });
  };

  return {
    read: {
      readFile,
      readText: async (absolutePath, options) => {
        const doc = await readDocument(
          request,
          sessionId,
          absolutePath,
          Math.max(options.maxBytes, options.maxScanBytes),
          options.signal,
        );
        return readPage(doc.text, options);
      },
      access,
      statFile,
    },
    write: { writeFile, mkdir },
    edit: {
      readFile,
      writeFile,
      access,
      statFile,
    },
  };
}