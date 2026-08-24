// @vitest-environment jsdom
// ChatPanel surface tests (Plan 097 Phase 10): provenance-exact host chat
// view renders declared copy, AG-UI-driven transcript, and session controls.

import { EventType } from "@ag-ui/core";
import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import type { AgentStreamEvent } from "../agent/events";

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
  };
});

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(async () => undefined),
  Channel: class {},
}));

vi.mock("../agent/events", () => ({
  agentStream: {
    events: harness.events,
    retain: () => () => undefined,
  },
  pipeRelay: (observer: {
    next: (event: AgentStreamEvent) => void;
    error: (error: unknown) => void;
  }) => harness.events.subscribe(observer as never),
}));

vi.mock("../bridge/client", () => ({
  sendRequest: vi.fn(async () => undefined),
}));

import { ChatPanel } from "./ChatPanel";
import { resetChatAgentForTests } from "../agent/state";

const emit = harness.emit as (event: AgentStreamEvent) => void;

const surface = {
  id: "chat.entry",
  actionTargets: ["chat.submit", "chat.cancel"],
  provenance: {
    packageName: "@clay/chat",
    packageVersion: "0.1.0",
    apiPrefix: "chat",
    trustDomain: "trusted" as const,
  },
  component: {
    kind: "panel" as const,
    id: "chat.root",
    title: "Chat",
    children: [
      {
        kind: "label" as const,
        id: "chat.greeting",
        text: "What do you want to do today?",
      },
      {
        kind: "label" as const,
        id: "chat.providerHint",
        text: "Configure a provider to start chatting.",
      },
      {
        kind: "button" as const,
        id: "chat.button.agent",
        label: "Agent",
        action: { commandId: "agent.clientOpenAgentPicker" },
      },
    ],
  },
};

beforeEach(() => {
  resetChatAgentForTests();
});

afterEach(() => {
  cleanup();
});

describe("ChatPanel", () => {
  it("renders the declared landing with provider hint when unconfigured", () => {
    render(<ChatPanel surface={surface} uiVersion={4} />);
    expect(
      screen.getByText("What do you want to do today?"),
    ).toBeInTheDocument();
    expect(
      screen.getByText("Configure a provider to start chatting."),
    ).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Agent" })).toBeInTheDocument();
    expect(screen.getByRole("log", { name: "Transcript" })).toHaveAttribute(
      "aria-live",
      "polite",
    );
  });

  it("shows the transcript from an out-of-run messages snapshot", async () => {
    const store = (await import("../agent/state")).chatAgent;
    const release = store.start();
    try {
      render(<ChatPanel surface={surface} uiVersion={4} />);
      emit({
        type: EventType.MESSAGES_SNAPSHOT,
        messages: [
          { id: "m0", role: "user", content: "hi" },
          {
            id: "m1",
            role: "assistant",
            content: "12 tokens",
            metadata: { clayKind: "usage" },
          },
          {
            id: "m2",
            role: "assistant",
            content: "no provider",
            metadata: { clayKind: "error" },
          },
        ],
        clientId: 1,
      } as never);
      emit({
        type: EventType.STATE_SNAPSHOT,
        snapshot: { provider: "mock", model: "mini" },
        clientId: 1,
      } as never);

      expect(await screen.findByText("hi")).toBeInTheDocument();
      expect(screen.getByText("12 tokens")).toBeInTheDocument();
      expect(screen.getByText(/mock\/mini/)).toBeInTheDocument();
      // Error status surfaces in the native-parity status line.
      expect(screen.getByText("no provider")).toBeInTheDocument();
    } finally {
      release();
    }
  });

  it("keeps composer disabled while streaming and offers cancel", async () => {
    const store = (await import("../agent/state")).chatAgent;
    const release = store.start();
    try {
      render(<ChatPanel surface={surface} uiVersion={4} />);
      // Configure first so the composer is enabled.
      emit({
        type: EventType.STATE_SNAPSHOT,
        snapshot: { provider: "mock" },
        clientId: 1,
      } as never);
      const input = await screen.findByLabelText("Message");
      expect(input).toBeEnabled();

      emit({
        type: EventType.RUN_STARTED,
        threadId: "s",
        runId: "r",
        clientId: 1,
      } as never);
      expect(store.getSnapshot().status.streaming).toBe(true);
      expect(await screen.findByText("Streaming")).toBeInTheDocument();
      expect(
        screen.getByRole("button", { name: "Cancel" }),
      ).toBeInTheDocument();
      expect(screen.getByLabelText("Message")).toBeDisabled();
    } finally {
      release();
    }
  });
});
