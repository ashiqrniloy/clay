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

/** One relayed AG-UI event with its owning-session tags. */
export type AgentStreamEvent = BaseEvent & {
  /** Connection that delivered the event (the receiving tab, not the owner:
   *  the relay is a process-wide fan-out). */
  clientId: number;
  tabId?: number;
  /** Session the event belongs to (plan 119 SC-6). Absent for process-wide
   *  messages (diagnostics, agent-RPC replies) and for a tab's pre-session
   *  STATE snapshot. This is what a per-tab store filters on. */
  sessionId?: string;
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
          // Failing to register is not a stream failure: erroring the subject
          // would kill the relay for every tab (and, with no subscribers yet,
          // surface as an unhandled rejection). Stay subscribed but inert —
          // the connection store owns the surfaced phase.
          void invoke("agent_subscribe", { onEvent: channel }).catch(() => {
            // No Tauri IPC: fixtures, tests, window teardown.
          });
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

/** Process-wide relay stream (native parity: one agent stream per client). */
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
