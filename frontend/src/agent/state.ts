// Presentation binding between the AG-UI agent instance and React
// (Plan 097 Phase 10).
//
// This is NOT a Clay event reducer: message accumulation, chunk expansion,
// and RFC 6902 state deltas are applied by `AbstractAgent`'s upstream
// pipeline. Out-of-run snapshots (transcript restore, session inventory) are
// handed to the agent through its own public `setMessages`/`setState` APIs,
// exactly as the AG-UI docs prescribe for connected clients.

import type { Message } from "@ag-ui/core";

import { TauriClayAgent } from "./TauriClayAgent";
import { agentStream, pipeRelay, type AgentStreamEvent } from "./events";

export interface ChatStatus {
  streaming: boolean;
  /** Last diagnostic/error line (native `chat.status` parity). */
  status: string | null;
}

interface ChatAgentModule {
  readonly agent: TauriClayAgent;
  /** Subscribe to versioned notifications for useSyncExternalStore. */
  subscribe(listener: () => void): () => void;
  getVersion(): number;
  getSnapshot(): ChatSnapshot;
  /** Starts relay processing; call once per surface mount. */
  start(): () => void;
  /** DEV-only fixture seam (no-op outside dev builds). */
  seedForDev(input: {
    messages?: Message[];
    state?: Record<string, unknown>;
    streaming?: boolean;
    statusText?: string | null;
  }): void;
}

export interface ChatSnapshot {
  messages: Message[];
  status: ChatStatus;
  /** Agent/conversation state from STATE_SNAPSHOT events. */
  state: Record<string, unknown>;
}

function createChatAgent(): ChatAgentModule {
  const agent = new TauriClayAgent({});
  let version = 0;
  const listeners = new Set<() => void>();
  let status: ChatStatus = { streaming: false, status: null };
  let snapshot: ChatSnapshot = {
    messages: [],
    state: {},
    status,
  };

  /** Rebuilds the immutable snapshot synchronously after any mutation. */
  const rebuild = () => {
    snapshot = {
      messages: [...agent.messages],
      state: { ...agent.state },
      status,
    };
  };

  /** Listener notifications are coalesced per animation frame. */
  const notifyListeners = scheduled(() => {
    version += 1;
    listeners.forEach((listener) => listener());
  });

  const notify = () => {
    rebuild();
    notifyListeners();
  };

  function errorStatusFromMessages(): string | null {
    // Native parity: last Error entry wins; otherwise no sticky status.
    for (let index = agent.messages.length - 1; index >= 0; index -= 1) {
      const message = agent.messages[index];
      if (
        message &&
        (message as { metadata?: { clayKind?: string } }).metadata?.clayKind ===
          "error"
      ) {
        return String(message.content ?? "Error");
      }
    }
    return null;
  }

  /**
   * Applies out-of-run events to the agent via its public API. Run-scoped
   * lifecycle/text/reasoning events are ignored here — they only matter when
   * a run pipeline is active (`runAgent`), which applies them itself.
   */
  function applyOutOfRun(event: AgentStreamEvent) {
    switch (event.type) {
      case "MESSAGES_SNAPSHOT": {
        agent.setMessages(
          (event as unknown as { messages: Message[] }).messages.map(
            cloneMessage,
          ),
        );
        notify();
        break;
      }
      case "STATE_SNAPSHOT": {
        agent.setState(
          (event as unknown as { snapshot: Record<string, unknown> }).snapshot,
        );
        notify();
        break;
      }
      case "CUSTOM": {
        const name = (event as { name?: string }).name;
        if (name === "clay.diagnostic") {
          const value = (
            event as { value?: { code?: string; message?: string } }
          ).value;
          const message = value?.message ?? "";
          if (message === "empty prompt") break;
          if (value?.code === "agent.cancelled" || message === "cancelled") {
            status = { streaming: false, status: null };
          } else {
            status = { streaming: false, status: message };
          }
          notify();
        }
        break;
      }
      case "RUN_STARTED": {
        status = { ...status, streaming: true };
        notify();
        break;
      }
      case "RUN_FINISHED":
      case "RUN_ERROR": {
        const failure =
          event.type === "RUN_ERROR"
            ? String((event as { message?: string }).message ?? "Error")
            : null;
        status = {
          streaming: false,
          status: failure ?? errorStatusFromMessages(),
        };
        notify();
        break;
      }
      default:
        break;
    }
  }

  return {
    agent,
    subscribe(listener: () => void) {
      listeners.add(listener);
      return () => listeners.delete(listener);
    },
    getVersion: () => version,
    getSnapshot: () => snapshot,
    /** DEV-only visual-fixture seam: seed transcript/status without a server. */
    seedForDev(input: {
      messages?: Message[];
      state?: Record<string, unknown>;
      streaming?: boolean;
      statusText?: string | null;
    }) {
      if (!import.meta.env.DEV) return;
      if (input.messages !== undefined) {
        agent.setMessages(structuredClone(input.messages));
      }
      if (input.state !== undefined) {
        agent.setState(structuredClone(input.state));
      }
      if (input.streaming !== undefined || input.statusText !== undefined) {
        status = {
          streaming: input.streaming ?? status.streaming,
          status:
            input.statusText === undefined ? status.status : input.statusText,
        };
      }
      notify();
    },
    start: () => {
      const release = agentStream.retain();
      const subscription = pipeRelay({
        next: (event) => applyOutOfRun(event),
        error: () => {
          // Relay errors surface through the connection store flow.
        },
      });
      return () => {
        subscription.unsubscribe();
        release();
      };
    },
  };
}

/** Clone a message defensively before handing ownership to the agent. */
function cloneMessage(message: Message): Message {
  return structuredClone(message);
}

/**
 * Coalesces synchronous notification bursts into one animation frame so
 * per-token deltas never trigger more than one rerender per frame.
 */
function scheduled(notify: () => void): () => void {
  let queued = false;
  return () => {
    if (queued) return;
    queued = true;
    requestAnimationFrame(() => {
      queued = false;
      notify();
    });
  };
}

const globalScope = globalThis as typeof globalThis & {
  __clayChatAgent?: ChatAgentModule;
};

/** Process-wide chat agent singleton (native parity: one stream per client). */
export const chatAgent: ChatAgentModule = (globalScope.__clayChatAgent ??=
  createChatAgent());

/** Test seam: reset the singleton. */
export function resetChatAgentForTests(): void {
  delete globalScope.__clayChatAgent;
}
