// Bounded connection/bootstrap projection consumed by the React shell.
// Deliberately dependency-free: a plain observable store so the connection
// state machine stays unit-testable without React or a Tauri runtime.

import type {
  BootstrapDto,
  BridgeEnvelope,
  TabRegistryEvent,
  ThemeSnapshot,
} from "../bridge/types";

export type ConnectionState =
  | { phase: "idle" }
  | { phase: "bootstrapping" }
  | { phase: "ready"; bootstrap: BootstrapDto }
  | { phase: "disconnected"; reason: string };

/** Pure reducer for connection transitions; exercised directly by tests. */
export function applyEnvelope(
  state: ConnectionState,
  envelope: BridgeEnvelope,
): ConnectionState {
  if (envelope.kind === "disconnected") {
    if (state.phase === "disconnected") return state;
    return { phase: "disconnected", reason: envelope.data.reason };
  }
  return state;
}

export interface ConnectionStore {
  get(): ConnectionState;
  set(next: ConnectionState): void;
  subscribe(listener: () => void): () => void;
}

export function createConnectionStore(): ConnectionStore {
  let state: ConnectionState = { phase: "idle" };
  const listeners = new Set<() => void>();
  return {
    get: () => state,
    set(next) {
      state = next;
      for (const listener of [...listeners]) listener();
    },
    subscribe(listener) {
      listeners.add(listener);
      return () => {
        listeners.delete(listener);
      };
    },
  };
}

/** Extracts the tab registry snapshot carried by an event envelope, if any. */
export function tabsFromEnvelope(
  envelope: BridgeEnvelope,
): TabRegistryEvent["data"] | null {
  if (envelope.kind === "event" && envelope.data.kind === "tabRegistry") {
    return (envelope.data as unknown as TabRegistryEvent).data;
  }
  return null;
}

export interface SessionStoresShape {
  connection: ConnectionState;
  theme: ThemeSnapshot | null;
}

/**
 * Reduces one envelope against the session's UI-relevant slices. The theme
 * snapshot replaces wholesale (Rust already resolved it); unknown event
 * kinds are ignored for forward compatibility.
 */
export function reduceSession(
  state: SessionStoresShape,
  envelope: BridgeEnvelope,
): SessionStoresShape {
  if (envelope.kind === "themeSnapshot") {
    return { ...state, theme: envelope.data };
  }
  const connection = applyEnvelope(state.connection, envelope);
  if (connection !== state.connection) return { ...state, connection };
  return state;
}
