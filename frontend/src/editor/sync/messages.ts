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

export function getStatusPayload(documentId: number): string {
  return JSON.stringify({
    family: "getDocumentStatus",
    payload: { clientId: 0, documentId },
  });
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
