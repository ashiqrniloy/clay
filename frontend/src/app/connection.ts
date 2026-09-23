// Pure view-model derivation from the typed server status. Kept free of
// React/IPC so the connection-state machine is unit-testable without a DOM.

import type { ServerStatus } from "../lib/server";

export type ConnectionView =
  | { kind: "loading"; message: string }
  | { kind: "ready"; message: string }
  | { kind: "error"; message: string; retryable: boolean };

export function connectionView(status: ServerStatus): ConnectionView {
  switch (status.state) {
    case "connecting":
      return {
        kind: "loading",
        message: `Connecting to Clay server at ${status.endpoint}…`,
      };
    case "connected":
      return {
        kind: "ready",
        message:
          status.pid === null
            ? `Clay server connected at ${status.endpoint}`
            : `Clay server connected (pid ${status.pid})`,
      };
    case "disconnected":
      return {
        kind: "error",
        message: `Clay server unavailable: ${status.reason}`,
        retryable: true,
      };
  }
}
