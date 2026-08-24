// Typed mirror of `src-tauri/src/server.rs::ServerStatus` (serde contract:
// internally tagged with "state", camelCase fields; pinned by Rust tests).
export type ServerStatus =
  | { state: "connecting"; endpoint: string }
  // pid is null when the shell adopted an already-running server.
  | { state: "connected"; endpoint: string; pid: number | null }
  | { state: "disconnected"; reason: string };

type Invoke = (cmd: string, args?: Record<string, unknown>) => Promise<unknown>;

// Injected for tests; production wires the Tauri invoke binding once in main.tsx.
let invokeBinding: Invoke = () =>
  Promise.reject(new Error("Tauri invoke binding not installed"));

export function installInvoke(binding: Invoke): void {
  invokeBinding = binding;
}

function expectStatus(payload: unknown): ServerStatus {
  if (
    typeof payload === "object" &&
    payload !== null &&
    "state" in payload &&
    typeof (payload as { state: unknown }).state === "string"
  ) {
    return payload as ServerStatus;
  }
  throw new Error(`malformed server status payload: ${String(payload)}`);
}

export async function fetchServerStatus(): Promise<ServerStatus> {
  return expectStatus(await invokeBinding("server_status"));
}

export async function restartServer(): Promise<ServerStatus> {
  return expectStatus(await invokeBinding("server_restart"));
}
