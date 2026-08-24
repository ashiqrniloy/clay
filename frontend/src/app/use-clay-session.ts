// Session hook: owns the bridge lifecycle for the React tree — bootstrap,
// event subscription, store wiring, reconnect. One instance per app.

import {
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
  useSyncExternalStore,
} from "react";

import {
  bootstrapSession,
  reconnectSession,
  subscribeToEvents,
  unsubscribeFromEvents,
} from "../bridge/client";
import type { BridgeEnvelope } from "../bridge/types";
import {
  createConnectionStore,
  type ConnectionState,
} from "../state/connection-store";
import { documentSession } from "../editor/session-singleton";
import { workspace } from "../shell/workspace-singleton";
import { themeStore } from "../state/stores";

const connectionStore = createConnectionStore();

export interface SessionHandle {
  connection: ConnectionState;
  /** Increments per session generation; keys router replacement. */
  generation: number;
  reconnect: () => void;
}

export function useClaySession(): SessionHandle {
  const [connection, setConnection] = useState<ConnectionState>(() =>
    connectionStore.get(),
  );
  const [generation, setGeneration] = useState(0);
  const generationRef = useRef(0);

  const setFromStore = useCallback(() => {
    setConnection(connectionStore.get());
  }, []);

  useEffect(() => connectionStore.subscribe(setFromStore), [setFromStore]);

  useEffect(() => {
    let cancelled = false;
    void (async () => {
      connectionStore.set({ phase: "bootstrapping" });
      try {
        // Subscribe before bootstrap: the bridge drops events emitted while
        // no sink is registered, and installInitial's status request during
        // bootstrap can otherwise race its own reply into a dropped window.
        await subscribeToEvents((envelope: BridgeEnvelope) => {
          if (envelope.kind === "themeSnapshot") {
            themeStore.setTheme(envelope.data);
            return;
          }
          if (envelope.kind === "runtimeSnapshot") {
            themeStore.setTheme(envelope.data.snapshot.activeTheme);
            themeStore.setTypography(envelope.data.snapshot.activeTypography);
            workspace.handleEnvelope(envelope);
            return;
          }
          if (envelope.kind === "disconnected") {
            workspace.handleEnvelope(envelope);
            const remaining = workspace
              .getSnapshot()
              .tabs.filter((tab) => !tab.disconnected);
            if (remaining.length === 0) {
              connectionStore.set({
                phase: "disconnected",
                reason: envelope.data.reason,
              });
            }
            return;
          }
          workspace.handleEnvelope(envelope);
          documentSession.handleEnvelope(
            envelope.kind === "routed"
              ? { kind: "event", data: envelope.data.event }
              : envelope,
          );
        });
        const bootstrap = await bootstrapSession();
        if (!cancelled) {
          themeStore.setTheme(bootstrap.activeTheme);
          themeStore.setTypography(bootstrap.activeTypography);
          workspace.installBootstrap(bootstrap);
          documentSession.installInitial(bootstrap);
          connectionStore.set({ phase: "ready", bootstrap });
          void workspace.restore();
        }
      } catch (error) {
        if (!cancelled) {
          connectionStore.set({
            phase: "disconnected",
            reason:
              typeof error === "object" && error !== null && "message" in error
                ? String((error as { message: unknown }).message)
                : "bootstrap failed",
          });
        }
      }
    })();
    return () => {
      cancelled = true;
      void unsubscribeFromEvents();
    };
  }, []);

  const reconnect = useCallback(() => {
    connectionStore.set({ phase: "bootstrapping" });
    void (async () => {
      try {
        const bootstrap = await reconnectSession();
        themeStore.setTheme(bootstrap.activeTheme);
        themeStore.setTypography(bootstrap.activeTypography);
        workspace.reset();
        workspace.installBootstrap(bootstrap);
        documentSession.installInitial(bootstrap);
        connectionStore.set({ phase: "ready", bootstrap });
        void workspace.restore();
        generationRef.current += 1;
        setGeneration(generationRef.current);
      } catch (error) {
        connectionStore.set({
          phase: "disconnected",
          reason:
            typeof error === "object" && error !== null && "message" in error
              ? String((error as { message: unknown }).message)
              : "reconnect failed",
        });
      }
    })();
  }, []);

  return useMemo(
    () => ({ connection, generation, reconnect }),
    [connection, generation, reconnect],
  );
}

/** Reads the shared connection store (used by tests and the shell). */
export function useSessionConnection(): ConnectionState {
  return useSyncExternalStore(
    (listener) => connectionStore.subscribe(listener),
    () => connectionStore.get(),
  );
}
