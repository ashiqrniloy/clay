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
  /** Optimistic clear after the panel dispatches a resume decision. */
  clearPendingApproval(): void;
  /** Test seam: clear the shared instance in place. */
  resetForTests(): void;
  /** Starts relay processing; call once per surface mount. */
  start(): () => void;
  /** Runs one prompt turn: awaits the run pipeline, then flushes the
   *  deferred server snapshot so the server list wins at the boundary. */
  runTurn(): Promise<void>;
  /** DEV-only fixture seam (no-op outside dev builds). */
  seedForDev(input: {
    messages?: Message[];
    state?: Record<string, unknown>;
    streaming?: boolean;
    statusText?: string | null;
  }): void;
}

/** AgentRpc `result` is an object after the AG-UI adapter parse, or a JSON
 *  string if an older server left it opaque. */
function parseAgentRpcResult(value: unknown): Record<string, unknown> | null {
  if (value && typeof value === "object" && !Array.isArray(value)) {
    return value as Record<string, unknown>;
  }
  if (typeof value === "string") {
    try {
      const parsed: unknown = JSON.parse(value);
      if (parsed && typeof parsed === "object" && !Array.isArray(parsed)) {
        return parsed as Record<string, unknown>;
      }
    } catch {
      return null;
    }
  }
  return null;
}

export interface ChatSnapshot {
  messages: Message[];
  status: ChatStatus;
  /** Agent/conversation state from STATE_SNAPSHOT events. */
  state: Record<string, unknown>;
  /** Pending tool approval from a suspended durable run; cleared when the
   *  resumed run starts or the run settles. */
  pendingApproval: {
    sessionId: string;
    runId: string;
    requestId: string;
    toolName: string;
  } | null;
}

function createChatAgent(): ChatAgentModule {
  const agent = new TauriClayAgent({});
  let version = 0;
  const listeners = new Set<() => void>();
  let status: ChatStatus = { streaming: false, status: null };
  /** Last server transcript seen mid-run; flushed when the run settles. */
  let pendingServerMessages: Message[] | null = null;
  let pendingApproval: ChatSnapshot["pendingApproval"] = null;
  let snapshot: ChatSnapshot = {
    messages: [],
    state: {},
    status,
    pendingApproval,
  };

  /** Rebuilds the immutable snapshot synchronously after any mutation. */
  const rebuild = () => {
    snapshot = {
      messages: [...agent.messages],
      state: { ...agent.state },
      status,
      pendingApproval,
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

  // Run-pipeline mutations (chunks, finished snapshot) live on AbstractAgent.
  // Without this, the panel only paints applyOutOfRun snapshots and looks idle
  // while the run is actually streaming.
  //
  // Plan 109 I5 lifecycle fix (recorded root cause): the run pipeline's
  // RUN_STARTED handler re-inserts `input.messages` — a stale pre-prompt
  // copy captured before the server snapshot carrying the new user row
  // arrived — which clobbered the prompt-time transcript, leaving only the
  // initial user message plus the live run's deltas. The hooks keep the
  // live (server-authoritative) list at run start and reconcile every
  // snapshot boundary against the server list while preserving this run's
  // in-flight delta messages.
  const runScopedId = (id: unknown): boolean =>
    typeof id === "string" &&
    (id.startsWith("clay-text-") ||
      id.startsWith("clay-reasoning-") ||
      id.startsWith("clay-tool-"));
  agent.subscribe({
    onMessagesChanged: () => notify(),
    onStateChanged: () => notify(),
    onRunStartedEvent: ({ agent: live }) => ({
      messages: [...live.messages],
      stopPropagation: true,
    }),
    onMessagesSnapshotEvent: ({ agent: live, event }) => {
      const incoming = event.messages;
      const serverIds = new Set(incoming.map((message) => message.id));
      // The server transcript coalesces the run's deltas live, so a settled
      // run's rows can already be represented in the snapshot — keep only
      // in-flight run messages the server list does not contain yet.
      const represented = new Set(
        incoming.map((message) => `${message.role}:${String(message.content ?? "")}`),
      );
      const inFlight = live.messages.filter(
        (message) =>
          !serverIds.has(message.id) &&
          runScopedId(message.id) &&
          !represented.has(`${message.role}:${String(message.content ?? "")}`),
      );
      return {
        messages: [...incoming, ...inFlight],
        stopPropagation: true,
      };
    },
    onCustomEvent: ({ event, messages }) => {
      if (event.name !== "clay.toolPhase") return undefined;
      const value = (
        event as { value?: Record<string, unknown> }
      ).value;
      if (!value) return undefined;
      const name = typeof value.name === "string" ? value.name : "";
      const toolCallId =
        typeof value.toolCallId === "string" ? value.toolCallId : "";
      const phase = typeof value.phase === "string" ? value.phase : "";
      // Plan 109 I5: one evolving tool row per call, mirroring the
      // server's transcript evolution (progress rows are transient).
      if (!name || !toolCallId || !phase || phase === "progress") {
        return undefined;
      }
      const id = `clay-tool-${toolCallId}`;
      const existing = messages.find((message) => message.id === id);
      const args =
        typeof value.argsDigest === "string" ? value.argsDigest : "";
      const output =
        typeof value.outputDigest === "string" ? value.outputDigest : "";
      const prior =
        typeof existing?.content === "string" ? existing.content : "";
      const text =
        phase === "started"
          ? args
            ? `${name} ${args}`
            : `${name} - running`
          : `${prior.replace(/ - running$/, "") || name}${
              output ? ` -> ${output}` : ""
            }`;
      const row = {
        id,
        role: "tool",
        toolCallId,
        content: text,
        metadata: {
          clayKind: name === "load_skill" ? "skill" : "tool",
          toolName: name,
          toolCallId,
          ...(name === "load_skill" &&
          typeof value.skillName === "string" &&
          value.skillName
            ? { skillName: value.skillName }
            : {}),
        },
      } as Message;
      return {
        messages: existing
          ? messages.map((message) => (message.id === id ? row : message))
          : [...messages, row],
      };
    },
  });

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
        // Plan 109 I5: while a run pipeline is active, its async delta
        // publishes would race an immediate setMessages (a late chunk
        // publish could resurrect stale rows after the snapshot). Defer —
        // the in-pipeline reconcile hook covers mid-run rendering, and the
        // last queued snapshot flushes right after runAgent() resolves
        // (pipeline quiescent), so the server list always wins at the
        // boundary.
        if (agent.isRunning) {
          pendingServerMessages = (
            event as unknown as { messages: Message[] }
          ).messages.map(cloneMessage);
          break;
        }
        agent.setMessages(
          (event as unknown as { messages: Message[] }).messages.map(
            cloneMessage,
          ),
        );
        notify();
        break;
      }
      case "STATE_SNAPSHOT": {
        const incoming = (event as unknown as { snapshot: Record<string, unknown> })
          .snapshot;
        const current = (agent.state ?? {}) as Record<string, unknown>;
        // Inventory snapshots omit provider/model. Replace would wipe a
        // picker selection and leave the composer stuck on "no provider".
        agent.setState({ ...current, ...incoming });
        notify();
        break;
      }
      case "CUSTOM": {
        const name = (event as { name?: string }).name;
        // clay.toolPhase rows (plan 109 I5) evolve transcript boxes through
        // the run pipeline's onCustomEvent hook; out-of-run tool activity
        // lands in the server transcript and reconciles at the next
        // snapshot boundary.
        if (name === "clay.permissionRequest") {
          // Durable-run tool approval (plan 108): the run suspended on a
          // side-effect gate. Surface it for the Allow/Deny strip; cleared
          // when the resumed run starts or the run settles.
          const value = (
            event as { value?: { sessionId?: string; runId?: string; requestId?: string; toolName?: string } }
          ).value;
          if (value?.sessionId && value.runId && value.requestId && value.toolName) {
            pendingApproval = {
              sessionId: value.sessionId,
              runId: value.runId,
              requestId: value.requestId,
              toolName: value.toolName,
            };
            notify();
          }
          break;
        }
        if (name === "clay.contextTokens") {
          // Plan 117 token meter: the meter numerator rides a custom event
          // per provider turn (bounded counter, never content); the ceiling
          // resolves client-side from the models inventory.
          const tokens = (event as { value?: { tokens?: unknown } }).value?.tokens;
          if (typeof tokens === "number" && Number.isFinite(tokens) && tokens >= 0) {
            const current = (agent.state ?? {}) as Record<string, unknown>;
            agent.setState({ ...current, contextTokens: tokens });
            notify();
          }
          break;
        }
        if (name === "clay.agentRpc") {
          // Plan 109 I7: the daemon's context-inspector response rides the
          // generic agent-RPC custom event. The fetch is session-scoped and
          // idempotent (no correlation needed): the payload IS the latest
          // server-authoritative view, cached by version in agent state.
          const rpc = (event as { value?: { code?: string; result?: unknown } }).value;
          const result = parseAgentRpcResult(rpc?.result);
          if (rpc?.code === "session.context" && result) {
            const current = (agent.state ?? {}) as Record<string, unknown>;
            if (typeof result.itemId === "string") {
              // Drawer detail (`session.context { itemId }`).
              agent.setState({ ...current, contextItemDetail: result });
            } else if (Array.isArray(result.categories)) {
              // Category list (`session.context`).
              agent.setState({ ...current, contextView: result });
            }
            notify();
          }
          // Plan 109 I8: the Observational Memory tab's read model —
          // worker selection + bounded observer activity (drops included),
          // session-scoped and idempotent like the context view.
          if (rpc?.code === "session.om.activity" && result) {
            if (typeof result.sessionId === "string") {
              const current = (agent.state ?? {}) as Record<string, unknown>;
              agent.setState({ ...current, omView: result });
              notify();
            }
          }
          // The `session.om.set` response carries the effective selection;
          // mirror it into the view so the dropdowns reflect it immediately.
          if (rpc?.code === "session.om.set" && result) {
            if (typeof result.sessionId === "string") {
              const current = (agent.state ?? {}) as Record<string, unknown>;
              const view = (current.omView ?? {}) as Record<string, unknown>;
              agent.setState({
                ...current,
                omView: {
                  ...view,
                  sessionId: result.sessionId,
                  observation: result.observation ?? null,
                  reflection: result.reflection ?? null,
                },
              });
              notify();
            }
          }
          // Plan 117 @-mentions: bounded workspace listing for the dropdown.
          if (rpc?.code === "workspace.files" && result && Array.isArray(result.files)) {
            const current = (agent.state ?? {}) as Record<string, unknown>;
            agent.setState({ ...current, workspaceFiles: result.files });
            notify();
          }
          // Plan 117 /resume: the panel's recent-sessions list — labeled,
          // workspace-scoped, bounded server-side.
          if (rpc?.code === "session.resumable" && result && Array.isArray(result.sessions)) {
            const current = (agent.state ?? {}) as Record<string, unknown>;
            agent.setState({ ...current, resumableSessions: result.sessions });
            notify();
          }
          break;
        }
        if (name === "clay.diagnostic") {
          const value = (
            event as { value?: { code?: string; message?: string } }
          ).value;
          const message = value?.message ?? "";
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
        pendingApproval = null;
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
        pendingApproval = null;
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
    /** Optimistic clear after the panel dispatches a resume decision. */
    clearPendingApproval() {
      if (!pendingApproval) return;
      pendingApproval = null;
      notify();
    },
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
    async runTurn(): Promise<void> {
      await agent.runAgent();
      if (pendingServerMessages) {
        agent.setMessages(pendingServerMessages);
        pendingServerMessages = null;
        notify();
      }
    },
    /** Test seam: clear the shared instance in place. The `chatAgent` export
   *  binding is a const, so deleting the global below never gave importers
   *  a fresh object — every test in a file shared one agent. */
  resetForTests(): void {
    agent.setMessages([]);
    agent.setState({});
    pendingServerMessages = null;
    pendingApproval = null;
    status = { streaming: false, status: null };
    rebuild();
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

/** Test seam: reset the singleton in place (the const export binding keeps
 *  one instance per module — deleting the global never refreshed importers). */
export function resetChatAgentForTests(): void {
  chatAgent.resetForTests();
}
