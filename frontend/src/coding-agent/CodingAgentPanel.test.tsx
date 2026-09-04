// @vitest-environment jsdom
// CodingAgentPanel surface tests (plan 108 task 8): provenance-exact host
// rendering for the bundled @clay/coding-agent split surface. Mirrors the
// ChatPanel test harness: the AG-UI relay is mocked; the stream seam and the
// Tauri bridge are the only outs.

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
type StreamEvent = Record<string, unknown> & { type: string };

const harness = vi.hoisted(() => {
  const listeners = new Set<(event: StreamEvent) => void>();
  return {
    emit(event: StreamEvent): void {
      for (const listener of listeners) listener(event);
    },
    subscribe(next: (event: StreamEvent) => void): void {
      listeners.add(next);
    },
  };
});

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(async () => undefined),
  Channel: class {},
}));

vi.mock("../agent/events", () => ({
  agentStream: {
    events: {
      subscribe: (observer: { next: (event: StreamEvent) => void }) => {
        harness.subscribe(observer.next);
        return { unsubscribe: () => undefined };
      },
    },
    retain: () => () => undefined,
  },
  pipeRelay: (observer: {
    next: (event: StreamEvent) => void;
    error: (error: unknown) => void;
  }) => {
    harness.subscribe(observer.next);
    return { unsubscribe: () => undefined };
  },
}));

vi.mock("../bridge/client", () => ({
  sendRequest: vi.fn(async () => undefined),
}));

import { CodingAgentPanel } from "./CodingAgentPanel";
import { resetChatAgentForTests } from "../agent/state";

beforeEach(() => {
  resetChatAgentForTests();
});

afterEach(() => {
  cleanup();
});

const surface = {
  id: "coding-agent.surface",
  actionTargets: ["coding-agent.profile", "coding-agent.close"],
  provenance: {
    packageName: "@clay/coding-agent",
    packageVersion: "0.1.0",
    apiPrefix: "coding-agent",
    trustDomain: "trusted" as const,
  },
  component: {
    kind: "panel" as const,
    id: "coding-agent.root",
    title: "Coding Agent",
    children: [
      {
        kind: "label" as const,
        id: "coding-agent.transcriptTitle",
        text: "Coding Agent",
      },
      {
        kind: "label" as const,
        id: "coding-agent.emptyHint",
        text: "No conversation yet.",
      },
      {
        kind: "textInput" as const,
        id: "coding-agent.composer",
        title: "Message",
        multiline: true,
      },
    ],
  },
};

describe("CodingAgentPanel", () => {
  it("renders the 50/50 split with transcript, tabs, status row, and strip", async () => {
    render(
      <CodingAgentPanel
        surface={surface}
        uiVersion={4}
        workspaceRoot="/tmp/ws"
      />,
    );
    expect(
      screen.getByRole("region", { name: "Coding Agent" }),
    ).toBeInTheDocument();
    expect(screen.getByRole("log", { name: "Transcript" })).toBeInTheDocument();
    for (const tab of ["Files", "Memory", "Context"]) {
      expect(screen.getByRole("tab", { name: tab })).toBeInTheDocument();
    }
    // Status row: workspace path + provider/model (git branch seams until the
    // pi-parity task lands the generic fields).
    expect(screen.getByText(/\/tmp\/ws/)).toBeInTheDocument();
    // Extension strip carries the MCP allow-list names from STATE_SNAPSHOT.
    expect(
      await screen.findByText(/MCP: none/),
    ).toBeInTheDocument();
  });

  it("renders transcript boxes from the AG-UI stream and colors them by type", async () => {
    render(
      <CodingAgentPanel
        surface={surface}
        uiVersion={4}
        workspaceRoot="/tmp/ws"
      />,
    );
    // Seed via the shared relay seam (same flow ChatPanel tests use): the
    // out-of-run snapshot path applies messages + state and notifies.
    const store = (await import("../agent/state")).chatAgent;
    const release = store.start();
    try {
      harness.emit({
        type: "MESSAGES_SNAPSHOT",
        messages: [
          { id: "m0", role: "user", content: "plan this phase" },
          { id: "m1", role: "assistant", content: "I read the plan." },
        ],
        clientId: 1,
      } as never);
      harness.emit({
        type: "STATE_SNAPSHOT",
        snapshot: {
          provider: "mock",
          model: "mini",
          mcpServers: ["files"],
        },
        clientId: 1,
      } as never);
      expect(await screen.findByText("plan this phase")).toBeInTheDocument();
      expect(screen.getByText("I read the plan.")).toBeInTheDocument();
      expect((await screen.findAllByText(/mock\/mini/)).length).toBeGreaterThan(0);
      expect(screen.getByText(/MCP: files/)).toBeInTheDocument();
    } finally {
      release();
    }
  });

  it("shows the full content of a selected truncated box in the right pane", async () => {
    render(
      <CodingAgentPanel
        surface={surface}
        uiVersion={4}
        workspaceRoot="/tmp/ws"
      />,
    );
    const store = (await import("../agent/state")).chatAgent;
    const release = store.start();
    try {
      harness.emit({
        type: "MESSAGES_SNAPSHOT",
        messages: [
          {
            id: "m0",
            role: "user",
            content: "line one\nline two\nline three\nline four",
          },
        ],
        clientId: 1,
      } as never);
      const box = await screen.findByRole("button", {
        name: /user line one/,
      });
      fireEvent.click(box);
      const detail = await screen.findByRole("region", {
        name: "Selected transcript entry",
      });
      expect(detail).toHaveTextContent("line four");
    } finally {
      release();
    }
  });

  it("lists context category counts without content", async () => {
    render(
      <CodingAgentPanel
        surface={surface}
        uiVersion={4}
        workspaceRoot="/tmp/ws"
      />,
    );
    const store = (await import("../agent/state")).chatAgent;
    const release = store.start();
    try {
      harness.emit({
        type: "MESSAGES_SNAPSHOT",
        messages: [
          { id: "m0", role: "user", content: "hello" },
          { id: "m1", role: "assistant", content: "hi" },
        ],
        clientId: 1,
      } as never);
      fireEvent.click(await screen.findByRole("tab", { name: "Context" }));
      expect(await screen.findByText("User prompts")).toBeInTheDocument();
      expect(screen.getByText("Agent messages")).toBeInTheDocument();
      expect(screen.getByText("Skills loaded")).toBeInTheDocument();
    } finally {
      release();
    }
  });

  it("filters the slash surface as the composer text grows", async () => {
    render(
      <CodingAgentPanel
        surface={surface}
        uiVersion={4}
        workspaceRoot="/tmp/ws"
      />,
    );
    const composer = screen.getByLabelText(/Message/);
    fireEvent.change(composer, { target: { value: "/open" } });
    expect(
      await screen.findByRole("option", { name: /open-session-as-fork/ }),
    ).toBeInTheDocument();
    expect(screen.getAllByRole("option").length).toBe(2);
    fireEvent.change(composer, { target: { value: "plain text" } });
    expect(screen.queryByRole("listbox")).not.toBeInTheDocument();
  });

  it("keeps Shift+Tab a no-op when the model exposes no effort levels", () => {
    render(
      <CodingAgentPanel
        surface={surface}
        uiVersion={4}
        workspaceRoot="/tmp/ws"
      />,
    );
    const composer = screen.getByLabelText(/Message/);
    const event = new KeyboardEvent("keydown", {
      key: "Tab",
      shiftKey: true,
      bubbles: true,
      cancelable: true,
    });
    fireEvent(composer, event);
    expect(event.defaultPrevented).toBe(false);
    expect(screen.queryByText(/effort /)).not.toBeInTheDocument();
  });
});
