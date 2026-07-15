// Configuration-only language-server authority facade.

const ops = globalThis.Deno?.core?.ops;

export interface AuthorizeLanguageServerOptions {
  package: string;
  contribution: string;
  workspaceRootIds: Array<number | string>;
}

export interface LanguageServerGrantSummary {
  package: string;
  version: string;
  sourceKind: string;
  contribution: string;
  executable: string;
  workspaceRootIds: string[];
  approvedBy: string;
}

/** Approve one fixed package contribution for current directory workspace roots.
 * Call from init.js before the first loadPackage call. Package loading seals
 * authority mutation for this runtime generation. */
export async function authorizeLanguageServer(
  options: AuthorizeLanguageServerOptions,
): Promise<LanguageServerGrantSummary> {
  if (!ops) {
    throw new Error("clay.language_server.runtime_unavailable: API requires the server runtime");
  }
  return JSON.parse(
    await ops.op_clay_language_server_authorize(JSON.stringify(options ?? null)),
  ) as LanguageServerGrantSummary;
}

export interface StartSessionOptions {
  package: string;
  contribution: string;
  workspaceRootId: number | string;
}

/** Opaque wrapper around a host-owned language-server child session.
 *
 * The session exposes only bounded opaque UTF-8 message read/write/stop. No
 * process, stdio, or native handle crosses the boundary; spawn parameters come
 * from the fixed validated contribution and the approved workspace root. LSP
 * Content-Length framing and server initialization are deferred to Phase 18.21
 * package adapters layered on top of this opaque transport. */
export interface LanguageServerSession {
  /** Send one bounded opaque UTF-8 message to the child stdin. */
  send(message: string): Promise<void>;
  /** Read up to maxBytes within timeoutMs from the child stdout. */
  read(maxBytes: number, timeoutMs: number): Promise<string>;
  /** Stop the child, reap it, and remove the session. */
  stop(): Promise<void>;
  /** Opaque session identifier assigned by the host. */
  readonly sessionId: number;
}

/** Start an authorized language-server session for one contribution + root. */
export async function startLanguageServerSession(
  options: StartSessionOptions,
): Promise<LanguageServerSession> {
  if (!ops) {
    throw new Error("clay.language_server.runtime_unavailable: API requires the server runtime");
  }
  const sessionId = JSON.parse(
    await ops.op_clay_language_server_start_session(JSON.stringify(options ?? null)),
  ).sessionId as number;
  return {
    sessionId,
    send: (message) =>
      ops.op_clay_language_server_send_message(
        JSON.stringify({
          sessionId,
          package: options.package,
          contribution: options.contribution,
          message,
        }),
      ).then(JSON.parse),
    read: (maxBytes, timeoutMs) =>
      ops.op_clay_language_server_read_message(
        JSON.stringify({
          sessionId,
          package: options.package,
          contribution: options.contribution,
          maxBytes,
          timeoutMs,
        }),
      )
        .then(JSON.parse)
        .then((result: { message: string }) => result.message),
    stop: () =>
      ops.op_clay_language_server_stop_session(JSON.stringify({ sessionId })).then(JSON.parse),
  };
}
