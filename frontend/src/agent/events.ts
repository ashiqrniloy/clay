// AG-UI stream plumbing for Clay (Plan 097 Phase 10).
//
// The Rust bridge adapts Clay's internal agent union to AG-UI events
// (`clay::server::agent_agui`) and fans them out over a Tauri channel. This
// module owns that channel and exposes it as one shared RxJS stream. Every
// event is a standard AG-UI `BaseEvent` tagged with the delivering session;
// there is no Clay-only event vocabulary on this stream.

import { Channel, invoke } from "@tauri-apps/api/core";
import { Observable, Subject, type Subscription } from "rxjs";

import type { BaseEvent } from "@ag-ui/core";

import { normalizeBridgeError } from "../bridge/errors";

/** One relayed AG-UI event with its owning session tags. */
export type AgentStreamEvent = BaseEvent & {
  clientId: number;
  tabId?: number;
};

interface AgentStreamModule {
  /** Hot stream of relayed events. */
  readonly events: Observable<AgentStreamEvent>;
  /** Reference-counted subscription lifecycle for the Tauri channel. */
  retain(): () => void;
}

function createAgentStream(): AgentStreamModule {
  const subject = new Subject<AgentStreamEvent>();
  let refCount = 0;

  return {
    events: subject.asObservable(),
    retain: () => {
      refCount += 1;
      if (refCount === 1) {
        // Degrade gracefully outside the Tauri webview (plain-browser
        // fixtures): the landing still renders; prompts fail closed.
        try {
          const channel = new Channel<AgentStreamEvent>();
          channel.onmessage = (event) => subject.next(event);
          void invoke("agent_subscribe", { onEvent: channel }).catch(
            (error) => {
              subject.error(normalizeBridgeError(error));
            },
          );
        } catch {
          // No Tauri IPC: stay subscribed but inert.
        }
      }
      let released = false;
      return () => {
        if (released) return;
        released = true;
        refCount -= 1;
        if (refCount === 0) {
          // Re-subscribing later re-registers a fresh channel.
          void invoke("agent_unsubscribe").catch(() => {
            // Bridge gone (window teardown); nothing to clean up.
          });
        }
      };
    },
  };
}

const globalScope = globalThis as typeof globalThis & {
  __clayAgentStream?: AgentStreamModule;
};

/** Process-wide relay stream (native parity: one chat stream per client). */
export const agentStream: AgentStreamModule = (globalScope.__clayAgentStream ??=
  createAgentStream());

/** Convenience: forward the relay into an observer. */
export function pipeRelay(observer: {
  next: (event: AgentStreamEvent) => void;
  error: (error: unknown) => void;
}): Subscription {
  return agentStream.events.subscribe({
    next: (event) => observer.next(event),
    error: (error) => observer.error(error),
  });
}
