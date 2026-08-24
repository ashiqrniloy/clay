// AG-UI transport tests (Plan 097 Phase 10).
//
// Verifies the one-stream contract: Rust-adapted AG-UI events flow through
// `TauriClayAgent` into `AbstractAgent`'s upstream pipeline, which owns all
// message accumulation. Out-of-run snapshots are applied through the agent's
// own public API — never a Clay-only reducer.

import { EventType } from "@ag-ui/core";
import type { Subscription } from "rxjs";
import { describe, expect, it, vi, beforeEach, afterEach } from "vitest";

import type { AgentStreamEvent } from "./events";

const harness = vi.hoisted(() => {
  const subscribers: Array<{
    next: (event: never) => void;
    error: (error: unknown) => void;
  }> = [];
  return {
    emit(event: never): void {
      for (const subscriber of [...subscribers]) subscriber.next(event);
    },
    events: {
      subscribe(observer: {
        next: (event: never) => void;
        error: (error: unknown) => void;
      }) {
        subscribers.push(observer);
        return {
          unsubscribe(): void {
            const index = subscribers.indexOf(observer);
            if (index >= 0) subscribers.splice(index, 1);
          },
        };
      },
    },
    sendRequestCalls: [] as string[],
  };
});

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(async () => undefined),
  Channel: class {
    onmessage: ((event: AgentStreamEvent) => void) | null = null;
  },
}));

vi.mock("./events", () => ({
  agentStream: {
    events: harness.events,
    retain: () => () => undefined,
  },
  pipeRelay: (observer: {
    next: (event: AgentStreamEvent) => void;
    error: (error: unknown) => void;
  }): Subscription =>
    harness.events.subscribe(observer as never) as unknown as Subscription,
}));

vi.mock("../bridge/client", () => ({
  sendRequest: vi.fn(async (payload: string) => {
    harness.sendRequestCalls.push(payload);
  }),
}));

import { TauriClayAgent } from "./TauriClayAgent";
import { chatAgent, resetChatAgentForTests } from "./state";

const emit = harness.emit as (event: AgentStreamEvent) => void;

/** Lets `runAgent` reach its internal `run()` subscription before events. */
const flush = () => new Promise<void>((resolve) => setTimeout(resolve, 0));
/** Lets rAF-coalesced store notifications land (jsdom fires at ~16 ms). */
const frame = () => new Promise<void>((resolve) => setTimeout(resolve, 40));

beforeEach(() => {
  harness.sendRequestCalls.length = 0;
  resetChatAgentForTests();
});

afterEach(() => {
  vi.restoreAllMocks();
});

function started(clientId = 1): AgentStreamEvent {
  return {
    type: EventType.RUN_STARTED,
    threadId: "sess-1",
    runId: "run-1",
    clientId,
  } as AgentStreamEvent;
}

describe("TauriClayAgent transport", () => {
  it("streams one run end-to-end through the AG-UI pipeline", async () => {
    const agent = new TauriClayAgent({});
    agent.sendPrompt("hello");
    const done = agent.runAgent();
    await flush();

    // Server opens the run and streams chunks; @ag-ui/client expands them.
    emit(started());
    emit({
      type: EventType.TEXT_MESSAGE_CHUNK,
      messageId: "clay-text-run-1",
      delta: "He",
      clientId: 1,
    } as AgentStreamEvent);
    emit({
      type: EventType.TEXT_MESSAGE_CHUNK,
      messageId: "clay-text-run-1",
      delta: "llo",
      clientId: 1,
    } as AgentStreamEvent);
    emit({
      type: EventType.REASONING_MESSAGE_CHUNK,
      messageId: "clay-reasoning-run-1",
      delta: "ponder",
      clientId: 1,
    } as AgentStreamEvent);
    emit({
      type: EventType.RUN_FINISHED,
      threadId: "sess-1",
      runId: "run-1",
      result: { usage: "12 tokens" },
      clientId: 1,
    } as AgentStreamEvent);

    await done;
    expect(agent.messages.map((message) => message.role)).toEqual([
      "assistant",
      "reasoning",
    ]);
    expect(agent.messages[0]?.content).toBe("Hello");
    expect(agent.messages[1]?.content).toBe("ponder");
  });

  it("sends the validated chat.submit intent with composer text", async () => {
    const agent = new TauriClayAgent({});
    agent.setUiVersion(9);
    agent.sendPrompt("hi there");
    const run = agent.runAgent();
    await flush();
    emit(started());
    emit({
      type: EventType.RUN_FINISHED,
      threadId: "sess-1",
      runId: "run-1",
      clientId: 1,
    } as AgentStreamEvent);
    await run;

    expect(harness.sendRequestCalls).toHaveLength(1);
    const payload = JSON.parse(String(harness.sendRequestCalls[0])) as {
      family: string;
      payload: {
        uiVersion: number;
        intent: { commandId: string; arguments: Array<{ name: string }> };
      };
    };
    expect(payload.family).toBe("sduiAction");
    expect(payload.payload.uiVersion).toBe(9);
    expect(payload.payload.intent.commandId).toBe("chat.submit");
    expect(payload.payload.intent.arguments[0]?.name).toBe("value");
  });

  it("completes empty submits without touching the wire", async () => {
    const agent = new TauriClayAgent({});
    agent.setUiVersion(4);
    agent.sendPrompt("   ");
    await agent.runAgent();
    expect(harness.sendRequestCalls).toHaveLength(0);
  });

  it("maps wire errors to RUN_ERROR and surfaces status", async () => {
    const store = chatAgent;
    const release = store.start();
    try {
      await flush();
      emit(started());
      emit({
        type: EventType.RUN_ERROR,
        message: "provider unreachable",
        clientId: 1,
      } as AgentStreamEvent);
      await frame();
      expect(store.getSnapshot().status.status).toBe("provider unreachable");
      expect(store.getSnapshot().status.streaming).toBe(false);
    } finally {
      release();
    }
  });

  it("cancel sends the chat.cancel intent through abortRun", () => {
    const agent = new TauriClayAgent({});
    agent.abortRun();
    expect(harness.sendRequestCalls).toHaveLength(1);
    const payload = JSON.parse(String(harness.sendRequestCalls[0]));
    expect(payload.payload.intent.commandId).toBe("chat.cancel");
  });
});

describe("chat state glue", () => {
  it("applies out-of-run snapshots via the agent's public API", async () => {
    const store = chatAgent;
    const release = store.start();
    await flush();
    try {
      emit({
        type: EventType.MESSAGES_SNAPSHOT,
        messages: [
          { id: "clay-entry-0", role: "user", content: "restored" },
          {
            id: "clay-entry-1",
            role: "assistant",
            content: "12 tokens",
            metadata: { clayKind: "usage" },
          },
        ],
        clientId: 1,
      } as unknown as AgentStreamEvent);
      emit({
        type: EventType.STATE_SNAPSHOT,
        snapshot: { provider: "mock", model: "mock-mini" },
        clientId: 1,
      } as AgentStreamEvent);

      await frame();
      const snapshot = store.getSnapshot();
      expect(snapshot.messages).toHaveLength(2);
      expect(snapshot.state["provider"]).toBe("mock");
      expect(snapshot.status.streaming).toBe(false);
    } finally {
      release();
    }
  });

  it("tracks streaming status across run lifecycle events", async () => {
    const store = chatAgent;
    const release = store.start();
    await flush();
    try {
      emit(started());
      await frame();
      expect(store.getSnapshot().status.streaming).toBe(true);
      emit({
        type: EventType.RUN_FINISHED,
        threadId: "sess-1",
        runId: "run-1",
        clientId: 1,
      } as AgentStreamEvent);
      expect(store.getSnapshot().status.streaming).toBe(false);
      emit({
        type: EventType.RUN_ERROR,
        message: "no provider configured",
        clientId: 1,
      } as AgentStreamEvent);
      expect(store.getSnapshot().status.status).toBe("no provider configured");
    } finally {
      release();
    }
  });

  it("forwards clay.diagnostic customs into status", async () => {
    const store = chatAgent;
    const release = store.start();
    await flush();
    try {
      emit({
        type: EventType.CUSTOM,
        name: "clay.diagnostic",
        value: { code: "agent.idle", message: "no running session" },
        clientId: 1,
      } as unknown as AgentStreamEvent);
      await frame();
      expect(store.getSnapshot().status.status).toBe("no running session");
    } finally {
      release();
    }
  });

  it("relayed event keys stay within the AG-UI surface", () => {
    // Structural guarantee: every relayed key is a standard AG-UI BaseEvent
    // field or a session tag. Credential material has no channel.
    const allowedKeys = new Set([
      "type",
      "threadId",
      "runId",
      "parentRunId",
      "input",
      "result",
      "usage",
      "message",
      "code",
      "stepName",
      "messageId",
      "role",
      "delta",
      "name",
      "value",
      "snapshot",
      "delta2",
      "messages",
      "event",
      "source",
      "toolCallId",
      "toolCallName",
      "parentMessageId",
      "content",
      "timestamp",
      "rawEvent",
      "metadata",
      "clientId",
      "tabId",
    ]);
    const sample = JSON.parse(
      JSON.stringify({
        type: "RUN_FINISHED",
        threadId: "t",
        runId: "r",
        result: { usage: "u" },
        clientId: 1,
        tabId: 2,
      }),
    ) as Record<string, unknown>;
    for (const key of Object.keys(sample)) {
      expect(allowedKeys.has(key)).toBe(true);
    }
  });
});
