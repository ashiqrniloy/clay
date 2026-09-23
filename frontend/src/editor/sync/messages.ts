import type { EditOperation } from "./operations";

/** Protocol `ClientMessage` JSON (family/payload, camelCase). */
export function editPayload(
  documentId: number,
  transactionId: number,
  behaviorVersion: number,
  operation: EditOperation,
): string {
  return JSON.stringify({
    family: "edit",
    payload: {
      documentId,
      clientId: 0,
      leaseId: null,
      baseVersion: 0,
      behaviorVersion,
      transactionId,
      operation,
    },
  });
}

export function requestResyncPayload(
  documentId: number,
  knownVersion: number,
): string {
  return JSON.stringify({
    family: "requestResync",
    payload: { clientId: 0, documentId, knownVersion },
  });
}

export function savePayload(documentId: number, knownVersion: number): string {
  return JSON.stringify({
    family: "saveDocument",
    payload: { clientId: 0, documentId, knownVersion },
  });
}

export function reloadPayload(
  documentId: number,
  knownVersion: number,
  force: boolean,
): string {
  return JSON.stringify({
    family: "reloadDocument",
    payload: { clientId: 0, documentId, knownVersion, force },
  });
}

export function closePayload(documentId: number, force: boolean): string {
  return JSON.stringify({
    family: "closeDocument",
    payload: { clientId: 0, documentId, force },
  });
}

export function openPayload(workspaceRootId: number, path: string): string {
  return JSON.stringify({
    family: "openDocument",
    payload: { clientId: 0, workspaceRootId, path },
  });
}

/** Agent settings page (plan 117): request the server-built listing of
 *  delivered config files. The reply (agentSettingsFiles) reaches feature
 *  listeners; paths never cross the webview boundary. */
export function listAgentSettingsPayload(): string {
  return JSON.stringify({
    family: "listAgentSettingsFiles",
    payload: { clientId: 0 },
  });
}

/** Agent settings page (plan 117): open one listed file into the normal
 *  document pipeline by server-validated name. */
export function openAgentSettingsPayload(name: string): string {
  return JSON.stringify({
    family: "openAgentSettingsFile",
    payload: { clientId: 0, name },
  });
}

/** Launcher (plan 118 Part D): request the server-resolved start-surface
 *  rows (recent workspaces + configured agent types). The reply
 *  (`launcherEntries`) reaches feature listeners; the webview sends no path. */
export function listLauncherEntriesPayload(): string {
  return JSON.stringify({
    family: "listLauncherEntries",
    payload: { clientId: 0 },
  });
}

/** Launcher: drop one recent workspace by its index in the server's own list
 *  (never a path). The reply is a fresh `launcherEntries` listing. */
export function removeLauncherRecentPayload(index: number): string {
  return JSON.stringify({
    family: "removeLauncherRecent",
    payload: { clientId: 0, index },
  });
}

export function getStatusPayload(documentId: number): string {
  return JSON.stringify({
    family: "getDocumentStatus",
    payload: { clientId: 0, documentId },
  });
}

/** Client-side mirror of `MAX_CHUNK_BYTES`; the server clamps anyway. */
export const DOCUMENT_CHUNK_BYTES = 256 * 1024;

export function documentChunkRequestPayload(
  documentId: number,
  documentVersion: number,
  offset: number,
  maxBytes: number,
): string {
  return JSON.stringify({
    family: "documentChunkRequest",
    payload: { clientId: 0, documentId, documentVersion, offset, maxBytes },
  });
}

export function viewportRenderRequestPayload(
  documentId: number,
  documentVersion: number,
  requestId: number,
  byteStart: number,
  byteEnd: number,
  traceId: number,
  clientId = 0,
): string {
  const payload: Record<string, number> = {
    clientId,
    documentId,
    documentVersion,
    requestId,
    byteStart,
    byteEnd,
  };
  if (traceId > 0) payload.traceId = traceId;
  return JSON.stringify({ family: "viewportRenderRequest", payload });
}

export type EditRejection =
  | "leaseRequired"
  | "readOnlyDocument"
  | { staleVersion: { clientBaseVersion: number; serverVersion: number } }
  | { futureVersion: { clientBaseVersion: number; serverVersion: number } }
  | { leaseExpired: { leaseId: number } }
  | { regionLocked: unknown }
  | { invalidDocument: { documentId: number } }
  | { invalidRange: { message: string } }
  | {
      invalidBehaviorVersion: {
        behaviorVersion: number;
        serverBehaviorVersion: number;
      };
    }
  | string;

export function rejectionKey(reason: EditRejection): string {
  if (typeof reason === "string") return reason;
  const keys = Object.keys(reason);
  return keys[0] ?? "unknown";
}

const RESYNC_REASONS = new Set([
  "staleVersion",
  "futureVersion",
  "leaseRequired",
  "leaseExpired",
  "readOnlyDocument",
  "regionLocked",
  "invalidBehaviorVersion",
]);

export function shouldRequestResync(reason: EditRejection): boolean {
  return RESYNC_REASONS.has(rejectionKey(reason));
}
