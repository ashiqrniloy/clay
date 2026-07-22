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
export declare function authorizeLanguageServer(options: AuthorizeLanguageServerOptions): Promise<LanguageServerGrantSummary>;
export interface StartSessionOptions {
    contribution: string;
    workspaceRootId: number | string;
}
/** Opaque wrapper around a host-owned language-server child session.
 *
 * Exact byte methods preserve arbitrary chunk boundaries for package-owned
 * framing. No process, stdio, or native handle crosses the boundary; spawn
 * parameters come from the fixed contribution and approved workspace root. */
export interface LanguageServerSession {
    /** Send exact bounded bytes to child stdin. */
    sendBytes(bytes: Uint8Array): Promise<void>;
    /** Read up to maxBytes as exact bytes within timeoutMs. */
    readBytes(maxBytes: number, timeoutMs: number): Promise<Uint8Array>;
    /** Compatibility text send; byte-framed adapters must use sendBytes. */
    send(message: string): Promise<void>;
    /** Compatibility UTF-8 read; byte-framed adapters must use readBytes. */
    read(maxBytes: number, timeoutMs: number): Promise<string>;
    /** Stop the child, reap it, and remove the session. */
    stop(): Promise<void>;
    /** Opaque session identifier assigned by the host. */
    readonly sessionId: number;
}
/** Start an authorized language-server session for one contribution + root. */
export declare function startLanguageServerSession(options: StartSessionOptions): Promise<LanguageServerSession>;
