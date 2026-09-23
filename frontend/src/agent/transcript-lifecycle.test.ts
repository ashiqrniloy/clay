// @vitest-environment jsdom
// Plan 109 I5 transcript lifecycle tests.
//
// Recorded root cause of the "only initial user message + last agent
// message" defect: the run pipeline's RUN_STARTED handler re-inserted the
// stale pre-prompt `input.messages` seed, clobbering the prompt-time server
// snapshot (the new user row vanished), and the server never republished a
// snapshot after a run settled — so the shorter pipeline list persisted.
// The fix: the store keeps the live server-authoritative list at run start,
// reconciles every snapshot boundary (preserving in-flight run messages),
// and the server republishes the book transcript after every settled run.

import { EventType } from "@ag-ui/core";
import type { Subscription } from "rxjs";
import { describe, expect, it, vi, beforeEach } from "vitest";

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
  Channel: class {},
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

import { createAgentSession, type AgentSessionModule } from "./state";
import { sessionFiles } from "./session-files";

const emit = harness.emit as (event: AgentStreamEvent) => void;
const flush = () => new Promise<void>((resolve) => setTimeout(resolve, 0));
const frame = () => new Promise<void>((resolve) => setTimeout(resolve, 40));

let tabStore: AgentSessionModule;

beforeEach(() => {
  harness.sendRequestCalls.length = 0;
  tabStore = createAgentSession({});
});

function messagesSnapshot(
  rows: Array<{
    id: string;
    role: string;
    content: string;
    clayKind?: string;
    sessionFile?: { path: string; op: string };
  }>,
): AgentStreamEvent {
  return {
    type: EventType.MESSAGES_SNAPSHOT,
    messages: rows.map((row) => ({
      id: row.id,
      role: row.role,
      content: row.content,
      ...(row.clayKind || row.sessionFile
        ? {
            metadata: {
              ...(row.clayKind ? { clayKind: row.clayKind } : {}),
              ...(row.sessionFile ? { sessionFile: row.sessionFile } : {}),
            },
          }
        : {}),
    })),
    clientId: 1,
  } as AgentStreamEvent;
}

function toolPhase(value: {
  phase: string;
  name: string;
  toolCallId: string;
  argsDigest?: string;
  outputDigest?: string;
  skillName?: string;
  sessionFile?: { path: string; op: string };
}): AgentStreamEvent {
  return {
    type: EventType.CUSTOM,
    name: "clay.toolPhase",
    value,
    clientId: 1,
  } as AgentStreamEvent;
}

function runStarted(runId: string): AgentStreamEvent {
  return {
    type: EventType.RUN_STARTED,
    threadId: "sess-1",
    runId,
    clientId: 1,
  } as AgentStreamEvent;
}

function runFinished(runId: string): AgentStreamEvent {
  return {
    type: EventType.RUN_FINISHED,
    threadId: "sess-1",
    runId,
    result: { usage: "12 tokens" },
    clientId: 1,
  } as AgentStreamEvent;
}

function textChunk(runId: string, delta: string): AgentStreamEvent {
  return {
    type: EventType.TEXT_MESSAGE_CHUNK,
    messageId: `clay-text-${runId}`,
    delta,
    clientId: 1,
  } as AgentStreamEvent;
}

describe("transcript lifecycle (plan 109 I5)", () => {
  it("prompt → immediate snapshot ordering no longer drops history", async () => {
    const store = tabStore;
    const release = store.start();
    try {
      // Turn 1 settled: the server list already has history.
      emit(
        messagesSnapshot([
          { id: "clay-entry-0", role: "user", content: "first question" },
          {
            id: "clay-entry-1",
            role: "assistant",
            content: "first answer",
          },
        ]),
      );
      await frame();

      // Turn 2: prompt → the server records the user row and publishes the
      // snapshot BEFORE the daemon's RUN_STARTED (the reported race).
      store.agent.sendPrompt("second question");
      const run = store.runTurn();
      await flush();
      emit(
        messagesSnapshot([
          { id: "clay-entry-0", role: "user", content: "first question" },
          { id: "clay-entry-1", role: "assistant", content: "first answer" },
          { id: "clay-entry-2", role: "user", content: "second question" },
        ]),
      );
      emit(runStarted("run-2"));
      emit(textChunk("run-2", "second answer"));
      emit(runFinished("run-2"));
      // The server republishes the settled transcript AFTER the terminal
      // event (usage row included).
      emit(
        messagesSnapshot([
          { id: "clay-entry-0", role: "user", content: "first question" },
          { id: "clay-entry-1", role: "assistant", content: "first answer" },
          { id: "clay-entry-2", role: "user", content: "second question" },
          { id: "clay-entry-3", role: "assistant", content: "second answer" },
          {
            id: "clay-entry-4",
            role: "assistant",
            content: "12 tokens",
            clayKind: "usage",
          },
        ]),
      );
      await run;
      await frame();

      const rows = store.getSnapshot().messages;
      // The stale pre-prompt seed must not resurrect: all three user/agent
      // turns plus the usage row survive.
      expect(rows.map((row) => row.content)).toEqual([
        "first question",
        "first answer",
        "second question",
        "second answer",
        "12 tokens",
      ]);
      expect(
        rows.filter((row) => row.role === "user").map((row) => row.content),
      ).toEqual(["first question", "second question"]);
    } finally {
      release();
    }
  });

  it("multi-turn with tools and a skill load renders every row in order", async () => {
    const store = tabStore;
    const release = store.start();
    try {
      store.agent.sendPrompt("list files then load the skill");
      const run = store.runTurn();
      await flush();
      emit(
        messagesSnapshot([
          {
            id: "clay-entry-0",
            role: "user",
            content: "list files then load the skill",
          },
        ]),
      );
      emit(runStarted("run-1"));
      emit(textChunk("run-1", "checking"));
      emit(
        toolPhase({
          phase: "started",
          name: "edit",
          toolCallId: "c0",
          argsDigest: '{"path":"src/main.rs"}',
          sessionFile: { path: "src/main.rs", op: "edit" },
        }),
      );
      // Plan 118 task 36: the live row carries the file record, so the Files
      // tab counts the call without waiting for the settle snapshot.
      await flush();
      const liveRow = store
        .getSnapshot()
        .messages.find((row) => row.id === "clay-tool-c0") as
        | { metadata?: { sessionFile?: { path: string; op: string } } }
        | undefined;
      expect(liveRow?.metadata?.sessionFile).toEqual({
        path: "src/main.rs",
        op: "edit",
      });
      emit(
        toolPhase({
          phase: "started",
          name: "glob",
          toolCallId: "c1",
          argsDigest: '{"pattern":"**/*.rs"}',
        }),
      );
      emit(
        toolPhase({
          phase: "finished",
          name: "glob",
          toolCallId: "c1",
          outputDigest: "src/main.rs, src/lib.rs",
        }),
      );
      emit(
        toolPhase({
          phase: "started",
          name: "load_skill",
          toolCallId: "c2",
          argsDigest: '{"name":"rust-review"}',
          skillName: "rust-review",
        }),
      );
      emit(
        toolPhase({
          phase: "finished",
          name: "load_skill",
          toolCallId: "c2",
          outputDigest: "Loaded skill",
        }),
      );
      emit(runFinished("run-1"));
      emit(
        messagesSnapshot([
          {
            id: "clay-entry-0",
            role: "user",
            content: "list files then load the skill",
          },
          { id: "clay-entry-1", role: "assistant", content: "checking" },
          {
            id: "clay-tool-c0",
            role: "tool",
            content: 'edit {"path":"src/main.rs"}',
            clayKind: "tool",
            sessionFile: { path: "src/main.rs", op: "edit" },
          },
          {
            id: "clay-tool-c1",
            role: "tool",
            content: 'glob {"pattern":"**/*.rs"} -> src/main.rs, src/lib.rs',
            clayKind: "tool",
          },
          {
            id: "clay-tool-c2",
            role: "tool",
            content: 'load_skill {"name":"rust-review"} -> Loaded skill',
            clayKind: "skill",
          },
          {
            id: "clay-entry-2",
            role: "assistant",
            content: "12 tokens",
            clayKind: "usage",
          },
        ]),
      );
      await run;
      await frame();

      const rows = store.getSnapshot().messages;
      expect(rows.map((row) => row.role)).toEqual([
        "user",
        "assistant",
        "tool",
        "tool",
        "tool",
        "assistant",
      ]);
      expect(rows[3]?.content).toBe(
        'glob {"pattern":"**/*.rs"} -> src/main.rs, src/lib.rs',
      );
      expect(
        (rows[4] as { metadata?: { clayKind?: string } }).metadata?.clayKind,
      ).toBe("skill");
      // The Files tab's projection over the settled transcript.
      expect(sessionFiles(rows)).toEqual([
        { path: "src/main.rs", role: "modified" },
      ]);

      // Remount/restore (snapshot replay) renders the same complete
      // server-authoritative history.
      const restored = [...rows];
      emit(
        messagesSnapshot(
          restored.map((row) => ({
            id: row.id,
            role: row.role,
            content: String(row.content ?? ""),
            ...((row as { metadata?: { clayKind?: string } }).metadata?.clayKind
              ? {
                  clayKind: (row as { metadata?: { clayKind?: string } })
                    .metadata?.clayKind,
                }
              : {}),
          })),
        ),
      );
      await frame();
      expect(store.getSnapshot().messages.map((row) => row.id)).toEqual(
        restored.map((row) => row.id),
      );
    } finally {
      release();
    }
  });

  it("mid-run steer appears as a user-kind entry without losing in-flight text", async () => {
    const store = tabStore;
    const release = store.start();
    try {
      store.agent.sendPrompt("start");
      const run = store.runTurn();
      await flush();
      emit(runStarted("run-1"));
      emit(textChunk("run-1", "partial"));
      // The server records the steer and publishes mid-run; the pipeline
      // reconciles around it and keeps the in-flight run message.
      emit(
        messagesSnapshot([
          { id: "clay-entry-0", role: "user", content: "start" },
          { id: "clay-entry-1", role: "user", content: "steer: focus tests" },
        ]),
      );
      emit(textChunk("run-1", " answer"));
      emit(runFinished("run-1"));
      // Server republish after the settled run: the coalesced assistant
      // text is server-transcript content now.
      emit(
        messagesSnapshot([
          { id: "clay-entry-0", role: "user", content: "start" },
          { id: "clay-entry-1", role: "user", content: "steer: focus tests" },
          {
            id: "clay-entry-2",
            role: "assistant",
            content: "partial answer",
          },
        ]),
      );
      await run;
      await frame();

      const rows = store.getSnapshot().messages;
      const roles = rows.map((row) => row.role);
      expect(roles).toContain("user");
      expect(rows.some((row) => row.content === "steer: focus tests")).toBe(
        true,
      );
      // In-flight run text survived the mid-run snapshot boundary.
      expect(
        rows.some(
          (row) => row.role === "assistant" && row.content === "partial answer",
        ),
      ).toBe(true);
    } finally {
      release();
    }
  });
});
