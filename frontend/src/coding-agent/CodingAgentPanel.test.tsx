// @vitest-environment jsdom
// CodingAgentPanel surface tests (plan 108 task 8): provenance-exact host
// rendering for the bundled @clay/coding-agent split surface. Mirrors the
// CodingAgentPanel test harness: the AG-UI relay is mocked; the stream seam and the
// Tauri bridge are the only outs.

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
  act,
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
  within,
} from "@testing-library/react";
type StreamEvent = Record<string, unknown> & { type: string };

const harness = vi.hoisted(() => {
  const listeners = new Set<(event: StreamEvent) => void>();
  return {
    emit(event: StreamEvent): void {
      for (const listener of listeners) listener(event);
    },
    subscribe(next: (event: StreamEvent) => void): () => void {
      listeners.add(next);
      return () => listeners.delete(next);
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
        const unsubscribe = harness.subscribe(observer.next);
        return { unsubscribe };
      },
    },
    retain: () => () => undefined,
  },
  pipeRelay: (observer: {
    next: (event: StreamEvent) => void;
    error: (error: unknown) => void;
  }) => {
    const unsubscribe = harness.subscribe(observer.next);
    return { unsubscribe };
  },
}));

vi.mock("../bridge/client", () => ({
  sendRequest: vi.fn(async () => undefined),
}));

import { CodingAgentPanel, groupModelsByProvider } from "./CodingAgentPanel";

/** Mirrors the panel's worker-dropdown clear value (plan 109 I8). */
const OM_WORKER_CLEAR_VALUE = "om-worker:none";
import { sendRequest } from "../bridge/client";
import { resetAgentSessionForTests } from "../agent/state";
import { createDocumentSession } from "../editor/sync/session";

beforeEach(() => {
  resetAgentSessionForTests();
  // Per-test isolation: intents are asserted per test, so a previous test's
  // send history must not leak into another's "every intent is declared" check.
  vi.clearAllMocks();
});

afterEach(() => {
  cleanup();
});

const surface = {
  id: "coding-agent.surface",
  // The shipped action targets (packages/coding-agent/package.json): a
  // `sendIntent` with an id outside this list is not a valid surface intent
  // and the server rejects it — a composer button was silently dead that way
  // (plan 117 follow-up).
  actionTargets: [
    "coding-agent.profile",
    "coding-agent.close",
    "agent.clientOpenModelPicker",
    "agent.clientOpenSessionPicker",
    "documents.clientOpenFileDialog",
  ],
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
  it("renders the approved composition: transcript, inspector tabs, state strip, and environment foot", async () => {
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
    // Environment foot: workspace path, branch, and the MCP summary (the
    // per-server detail lives in the Context tab).
    expect(screen.getByText("/tmp/ws")).toBeInTheDocument();
    expect(screen.getByText("git —")).toBeInTheDocument();
    expect(await screen.findByText("MCP none")).toBeInTheDocument();
    // The composer owns the single focus boundary; the strip reports state.
    expect(screen.getByLabelText(/Message/)).toBeInTheDocument();
    expect(screen.getAllByRole("status").length).toBeGreaterThan(0);
  });

  it("renders transcript boxes from the AG-UI stream and colors them by type", async () => {
    render(
      <CodingAgentPanel
        surface={surface}
        uiVersion={4}
        workspaceRoot="/tmp/ws"
      />,
    );
    // Seed via the shared relay seam (same flow the transport tests use): the
    // out-of-run snapshot path applies messages + state and notifies.
    const store = (await import("../agent/state")).agentSession;
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
          mcpServers: [
            { serverId: "files", connected: true, tools: 2, error: "" },
          ],
        },
        clientId: 1,
      } as never);
      expect(await screen.findByText("plan this phase")).toBeInTheDocument();
      expect(screen.getByText("I read the plan.")).toBeInTheDocument();
      expect((await screen.findAllByText(/mock\/mini/)).length).toBeGreaterThan(
        0,
      );
      expect(screen.getByText("MCP files · 2 tools")).toBeInTheDocument();
    } finally {
      release();
    }
  });

  it("shows the full content of a selected card in Session Info (plan 109 I10)", async () => {
    render(
      <CodingAgentPanel
        surface={surface}
        uiVersion={4}
        workspaceRoot="/tmp/ws"
      />,
    );
    const store = (await import("../agent/state")).agentSession;
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
          {
            id: "clay-tool-t1",
            role: "tool",
            content: "exited 0",
            metadata: { clayKind: "tool", toolName: "shell", toolCallId: "t1" },
          },
        ],
        clientId: 1,
      } as never);
      const box = await screen.findByRole("button", {
        name: /user line one/,
      });
      fireEvent.click(box);
      // The card select auto-switches to the Session Info tab.
      const sessionInfoTab = await screen.findByRole("tab", {
        name: "Session Info",
      });
      expect(sessionInfoTab).toHaveAttribute("aria-selected", "true");
      // "line four" renders in both the truncated card and the detail.
      expect((await screen.findAllByText(/line four/)).length).toBeGreaterThan(
        1,
      );
      // A tool card renders its kind and tool-name metadata.
      fireEvent.click(screen.getByRole("button", { name: /shell exited 0/ }));
      // Kind row + tool-name row show "shell"; the content pre shows the
      // bounded output.
      expect((await screen.findAllByText("shell")).length).toBeGreaterThan(1);
      expect((await screen.findAllByText("exited 0")).length).toBeGreaterThan(
        1,
      );
    } finally {
      release();
    }
  });

  it("Back from Session Info restores the tab the selection came from (plan 109 I10)", async () => {
    render(
      <CodingAgentPanel
        surface={surface}
        uiVersion={4}
        workspaceRoot="/tmp/ws"
      />,
    );
    const store = (await import("../agent/state")).agentSession;
    const release = store.start();
    try {
      harness.emit({
        type: "MESSAGES_SNAPSHOT",
        messages: [{ id: "m0", role: "user", content: "inspect me" }],
        clientId: 1,
      } as never);
      // Land on Context first: the selection must return here.
      fireEvent.click(screen.getByRole("tab", { name: "Context" }));
      const box = await screen.findByRole("button", {
        name: /user inspect me/,
      });
      fireEvent.click(box);
      expect(
        await screen.findByRole("tab", { name: "Session Info" }),
      ).toHaveAttribute("aria-selected", "true");
      fireEvent.click(screen.getByRole("button", { name: "Back" }));
      expect(screen.getByRole("tab", { name: "Context" })).toHaveAttribute(
        "aria-selected",
        "true",
      );
    } finally {
      release();
    }
  });

  it("Session Info with no selection shows guidance (plan 109 I10)", async () => {
    render(
      <CodingAgentPanel
        surface={surface}
        uiVersion={4}
        workspaceRoot="/tmp/ws"
      />,
    );
    const store = (await import("../agent/state")).agentSession;
    const release = store.start();
    try {
      fireEvent.click(screen.getByRole("tab", { name: "Session Info" }));
      expect(
        await screen.findByText(/Select a transcript card/i),
      ).toBeInTheDocument();
    } finally {
      release();
    }
  });

  it("completes daemon-registered commands from session state (plan 109 R1)", async () => {
    render(
      <CodingAgentPanel
        surface={surface}
        uiVersion={4}
        workspaceRoot="/tmp/ws"
      />,
    );
    const store = (await import("../agent/state")).agentSession;
    const release = store.start();
    try {
      // The daemon registry rides STATE (plan-108 registry is the single
      // source); no hand-synced frontend list exists anymore.
      harness.emit({
        type: "STATE_SNAPSHOT",
        snapshot: {
          provider: "mock",
          model: "mini",
          commands: [
            { name: "/compact", description: "Compact the current session." },
            { name: "/new", description: "Start a fresh session." },
            { name: "/open-session", description: "Load a session by id." },
            {
              name: "/open-session-as-fork",
              description: "Load and immediately fork.",
            },
          ],
          branch: "main",
          extensions: ["wiki"],
        },
        clientId: 1,
      } as never);
      expect((await screen.findAllByText(/mock\/mini/)).length).toBeGreaterThan(
        0,
      );
      const composer = screen.getByLabelText(/Message/);
      fireEvent.change(composer, { target: { value: "/open" } });
      expect(
        await screen.findByRole("option", { name: /open-session-as-fork/ }),
      ).toBeInTheDocument();
      expect(screen.getAllByRole("option").length).toBe(2);
      fireEvent.change(composer, { target: { value: "plain text" } });
      expect(screen.queryByRole("listbox")).not.toBeInTheDocument();
      // Unregistered names never complete.
      fireEvent.change(composer, { target: { value: "/deploy" } });
      expect(screen.queryByRole("listbox")).not.toBeInTheDocument();
      // Plan 109 R2: the real branch replaces the `git —` placeholder.
      expect(screen.getByText("git main")).toBeInTheDocument();
      // Plan 109 R3: the foot reflects daemon-reported extensions.
      expect(screen.getByText("extensions wiki")).toBeInTheDocument();
    } finally {
      release();
    }
  });

  it("requests the tab's state at mount so branch + MCP are known before the first prompt (plan 117 follow-up)", async () => {
    vi.mocked(sendRequest).mockClear();
    render(
      <CodingAgentPanel
        surface={surface}
        uiVersion={4}
        workspaceRoot="/tmp/ws"
      />,
    );
    // Nothing emits a snapshot before the first prompt, so the panel asks for
    // the tab's STATE itself (tab-resolved server-side).
    expect(sendRequest).toHaveBeenCalledWith(
      expect.stringContaining("listSessions"),
    );
    expect(sendRequest).toHaveBeenCalledWith(
      expect.stringContaining("tabState"),
    );

    const store = (await import("../agent/state")).agentSession;
    const release = store.start();
    try {
      harness.emit({
        type: "STATE_SNAPSHOT",
        snapshot: {
          sessionId: "",
          branch: "feature/pane-open",
          mcpServers: [{ serverId: "graft", connected: true, tools: 6 }],
          skills: [],
        },
        clientId: 1,
      } as never);
      // The foot reports the tab's branch (server-side `.git` read on the
      // mount snapshot) and its MCP summary; per-server detail is the
      // Context tab's.
      expect(
        await screen.findByText("git feature/pane-open"),
      ).toBeInTheDocument();
      expect(
        await screen.findByText("MCP graft · 6 tools"),
      ).toBeInTheDocument();
    } finally {
      release();
    }
  });

  it("renders `git —` and omits the extension segment without daemon data (plan 109 R2/R3)", async () => {
    render(
      <CodingAgentPanel
        surface={surface}
        uiVersion={4}
        workspaceRoot="/tmp/ws"
      />,
    );
    // The state store is a module singleton; pin the fields under test.
    const store = (await import("../agent/state")).agentSession;
    const release = store.start();
    try {
      harness.emit({
        type: "STATE_SNAPSHOT",
        snapshot: { branch: "", extensions: [], mcpServers: [] },
        clientId: 1,
      } as never);
      // Not a repo (or not yet read) → the `—` fallback; no extensions
      // reported → the segment is omitted, MCP stays.
      await screen.findByText("MCP none");
      expect(screen.getByText("/tmp/ws")).toBeInTheDocument();
      expect(screen.getByText("git —")).toBeInTheDocument();
      expect(screen.queryByText(/^extensions /)).not.toBeInTheDocument();
      expect(screen.getByText("MCP none")).toBeInTheDocument();
    } finally {
      release();
    }
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

  it("cycles declared effort levels via the manifest-bound chord and offers the dropdown", async () => {
    render(
      <CodingAgentPanel
        surface={surface}
        uiVersion={4}
        workspaceRoot="/tmp/ws"
        effortChord={{
          shift: true,
          ctrl: false,
          alt: false,
          meta: false,
          key: "Tab",
        }}
      />,
    );
    const store = (await import("../agent/state")).agentSession;
    const release = store.start();
    try {
      harness.emit({
        type: "STATE_SNAPSHOT",
        snapshot: {
          provider: "mock",
          model: "mini",
          models: [
            {
              provider: "mock",
              model: "mini",
              displayName: "Mock demo",
              thinkingLevels: ["low", "medium", "high"],
            },
          ],
          effort: "medium",
        },
        clientId: 1,
      } as never);
      // The cataloged dropdown appears for declared levels; the status row
      // reflects the active level.
      expect((await screen.findAllByText("medium")).length).toBeGreaterThan(0);
      const composer = screen.getByLabelText(/Message/);
      fireEvent.keyDown(composer, { key: "Tab", shiftKey: true });
      // medium -> high (next in the declared ladder) is now pending; the
      // chord is consumed (no focus change).
      expect((await screen.findAllByText("high")).length).toBeGreaterThan(0);
    } finally {
      release();
    }
  });

  it("rebound chord: the composer's cycle key follows the manifest binding", async () => {
    render(
      <CodingAgentPanel
        surface={surface}
        uiVersion={4}
        workspaceRoot="/tmp/ws"
        effortChord={{
          shift: false,
          ctrl: true,
          alt: false,
          meta: false,
          key: "e",
        }}
      />,
    );
    const store = (await import("../agent/state")).agentSession;
    const release = store.start();
    try {
      harness.emit({
        type: "STATE_SNAPSHOT",
        snapshot: {
          provider: "mock",
          model: "mini",
          models: [
            {
              provider: "mock",
              model: "mini",
              displayName: "Mock demo",
              thinkingLevels: ["low", "high"],
            },
          ],
        },
        clientId: 1,
      } as never);
      await screen.findByText("Effort");
      const composer = screen.getByLabelText(/Message/);
      // Default Shift+Tab no longer cycles: nothing pending changes.
      const shiftTab = new KeyboardEvent("keydown", {
        key: "Tab",
        shiftKey: true,
        bubbles: true,
        cancelable: true,
      });
      fireEvent(composer, shiftTab);
      expect(shiftTab.defaultPrevented).toBe(false);
      // The rebound chord cycles.
      fireEvent.keyDown(composer, { key: "e", ctrlKey: true });
      expect((await screen.findAllByText("low")).length).toBeGreaterThan(0);
    } finally {
      release();
    }
  });

  it("/resume opens the workspace-scoped session picker instead of prompting", () => {
    render(
      <CodingAgentPanel
        surface={surface}
        uiVersion={4}
        workspaceRoot="/tmp/ws"
      />,
    );
    const composer = screen.getByLabelText(/Message/);
    fireEvent.change(composer, { target: { value: "/resume" } });
    fireEvent.submit(composer.closest("form")!);
    expect(sendRequest).toHaveBeenCalledWith(
      expect.stringContaining("clientOpenSessionPicker"),
    );
  });

  it("/model opens the daemon model picker instead of prompting", () => {
    render(
      <CodingAgentPanel
        surface={surface}
        uiVersion={4}
        workspaceRoot="/tmp/ws"
      />,
    );
    const composer = screen.getByLabelText(/Message/);
    fireEvent.change(composer, { target: { value: "/model" } });
    fireEvent.submit(composer.closest("form")!);
    expect(sendRequest).toHaveBeenCalledWith(
      expect.stringContaining("clientOpenModelPicker"),
    );
  });

  it("renders the model dropdown with the active selection from inventory state", async () => {
    render(
      <CodingAgentPanel
        surface={surface}
        uiVersion={4}
        workspaceRoot="/tmp/ws"
      />,
    );
    const store = (await import("../agent/state")).agentSession;
    const release = store.start();
    try {
      harness.emit({
        type: "STATE_SNAPSHOT",
        snapshot: {
          provider: "mock",
          model: "mini",
          providers: [
            { id: "mock", configured: true },
            { id: "secret", configured: false },
          ],
          models: [
            { provider: "mock", model: "mini", displayName: "Mini" },
            { provider: "mock", model: "big" },
            { provider: "secret", model: "hidden" },
          ],
        },
        clientId: 1,
      } as never);
      // Dropdown trigger shows the active model's display label; the grouped
      // list itself is the ClayDropdown catalog behavior (verified in the
      // grouping unit test below). React Aria mirrors the selection into a
      // hidden native select, so the label may appear twice.
      expect((await screen.findAllByText("Mini")).length).toBeGreaterThan(0);
    } finally {
      release();
    }
  });

  it("groups configured providers' models and drops unconfigured ones", () => {
    const groups = groupModelsByProvider(
      [
        { provider: "b", model: "m2", displayName: "Two" },
        { provider: "a", model: "m1" },
        { provider: "b", model: "m3" },
        { provider: "z", model: "hidden" },
      ],
      [
        { id: "a", configured: true },
        { id: "b", configured: true },
        { id: "z", configured: false },
      ],
    );
    expect(groups.map((group) => group.label)).toEqual(["b", "a"]);
    expect(groups[0]?.options).toEqual([
      { id: "model:b/m2", label: "Two" },
      { id: "model:b/m3", label: "m3" },
    ]);
    expect(groups[1]?.options).toEqual([{ id: "model:a/m1", label: "m1" }]);
  });

  it("Context tab (plan 109 I7): server-authoritative categories, drawer, and item detail", async () => {
    render(
      <CodingAgentPanel
        surface={surface}
        uiVersion={4}
        workspaceRoot="/tmp/ws"
      />,
    );
    const store = (await import("../agent/state")).agentSession;
    const release = store.start();
    try {
      // A live session id activates the context fetch path.
      harness.emit({
        type: "STATE_SNAPSHOT",
        snapshot: { sessionId: "s1", provider: "mock", model: "mini" },
        clientId: 1,
      } as never);
      // Open the Context tab (controlled strip) — the fetch fires.
      fireEvent.click(screen.getByRole("tab", { name: "Context" }));
      const sendRequestMock = vi.mocked(sendRequest);
      await screen.findByText("Loading context…");
      expect(
        sendRequestMock.mock.calls.some(([payload]) =>
          String(payload).includes('"context"'),
        ),
      ).toBe(true);

      // The daemon response rides the clay.agentRpc custom event.
      harness.emit({
        type: "CUSTOM",
        name: "clay.agentRpc",
        value: {
          code: "session.context",
          result: {
            sessionId: "s1",
            version: 7,
            categories: [
              {
                kind: "userMessage",
                label: "User prompts",
                count: 2,
                items: [{ id: "e1#0", title: "User prompt", preview: "hello" }],
              },
              { kind: "thinking", label: "Thinking", count: 0, items: [] },
            ],
          },
        },
      } as never);
      expect(await screen.findByText("User prompts")).toBeInTheDocument();
      expect(screen.getByText("2")).toBeInTheDocument();

      // Drawer: click the category → bounded item list.
      fireEvent.click(screen.getByText("User prompts"));
      const item = await screen.findByRole("button", { name: /hello/ });
      expect(screen.getByRole("region")).toBeTruthy();

      // Item detail: click an item → full redacted content.
      fireEvent.click(item);
      harness.emit({
        type: "CUSTOM",
        name: "clay.agentRpc",
        value: {
          code: "session.context",
          result: {
            sessionId: "s1",
            itemId: "e1#0",
            kind: "userMessage",
            title: "User prompt",
            content: "hello full content",
          },
        },
      } as never);
      expect(await screen.findByText("hello full content")).toBeInTheDocument();

      // Back restores the category list.
      fireEvent.click(screen.getByRole("button", { name: "Back" }));
      expect(await screen.findByText("User prompts")).toBeInTheDocument();
    } finally {
      release();
    }
  });

  it("Context tab accepts AgentRpc result as a JSON string", async () => {
    render(
      <CodingAgentPanel
        surface={surface}
        uiVersion={4}
        workspaceRoot="/tmp/ws"
      />,
    );
    const store = (await import("../agent/state")).agentSession;
    const release = store.start();
    try {
      harness.emit({
        type: "STATE_SNAPSHOT",
        snapshot: { sessionId: "s-string", provider: "mock", model: "mini" },
        clientId: 1,
      } as never);
      fireEvent.click(screen.getByRole("tab", { name: "Context" }));
      await screen.findByText("Loading context…");
      harness.emit({
        type: "CUSTOM",
        name: "clay.agentRpc",
        value: {
          code: "session.context",
          result: JSON.stringify({
            sessionId: "s-string",
            version: 1,
            categories: [
              {
                kind: "userMessage",
                label: "User prompts",
                count: 1,
                items: [{ id: "e1#0", title: "User prompt", preview: "hello" }],
              },
            ],
          }),
        },
      } as never);
      expect(await screen.findByText("User prompts")).toBeInTheDocument();
    } finally {
      release();
    }
  });

  it("Memory tab (plan 109 I8): activity log with drops plus OM worker-model dropdowns", async () => {
    render(
      <CodingAgentPanel
        surface={surface}
        uiVersion={4}
        workspaceRoot="/tmp/ws"
      />,
    );
    const store = (await import("../agent/state")).agentSession;
    const release = store.start();
    try {
      harness.emit({
        type: "STATE_SNAPSHOT",
        snapshot: {
          sessionId: "s1",
          provider: "mock",
          model: "mini",
          models: [
            { provider: "mock", model: "mini", displayName: "Mock demo" },
          ],
          providers: [{ id: "mock", configured: true }],
        },
        clientId: 1,
      } as never);
      fireEvent.click(screen.getByRole("tab", { name: "Memory" }));
      await screen.findByText("Observational Memory is off for this session.");
      const sendRequestMock = vi.mocked(sendRequest);
      expect(
        sendRequestMock.mock.calls.some(([payload]) =>
          String(payload).includes('"omActivity"'),
        ),
      ).toBe(true);

      // The daemon view rides the clay.agentRpc custom event.
      harness.emit({
        type: "CUSTOM",
        name: "clay.agentRpc",
        value: {
          code: "session.om.activity",
          result: {
            sessionId: "s1",
            attached: true,
            observation: { provider: "mock", model: "mini" },
            reflection: null,
            activity: [
              {
                id: "e1#1",
                kind: "observation",
                summary: "User prefers dark mode",
              },
              {
                id: "e2#1",
                kind: "reflection",
                summary: "User values dark mode",
              },
              {
                id: "e3#dropped",
                kind: "drop",
                summary: "Dropped 1 observation",
              },
            ],
          },
        },
      } as never);
      expect(
        await screen.findByText("User prefers dark mode"),
      ).toBeInTheDocument();
      expect(screen.getByText("User values dark mode")).toBeInTheDocument();
      // Drops are visible, never silently vanished.
      expect(screen.getByText("Dropped 1 observation")).toBeInTheDocument();

      // Worker dropdowns reflect the server selection. React Aria mirrors
      // the selection into a hidden native select, so the label may appear
      // twice; the observation slot shows the selected model, the
      // reflection slot the cleared placeholder.
      const observationTrigger = (
        await screen.findAllByText("Mock demo")
      )[0]!.closest("button")!;
      expect(observationTrigger).toBeTruthy();
      expect(
        (await screen.findAllByText("Not set (workers off)")).length,
      ).toBeGreaterThan(0);

      // Selecting a worker model sends the SelectWorker command with the
      // session bound. React Aria mirrors Select into a hidden native
      // select; changing it drives onSelectionChange deterministically.
      // The reflection slot is uniquely identifiable here: it is the only
      // hidden select currently on the cleared placeholder.
      const hiddenSelect = [...document.querySelectorAll("select")].find(
        (select) => select.value === OM_WORKER_CLEAR_VALUE,
      );
      expect(hiddenSelect).toBeTruthy();
      await act(async () => {
        fireEvent.change(hiddenSelect!, {
          target: { value: "model:mock/mini" },
        });
      });
      await waitFor(() => {
        expect(
          sendRequestMock.mock.calls.some(([payload]) => {
            const body = String(payload);
            return (
              body.includes('"selectWorker"') &&
              body.includes('"worker":"reflection"') &&
              body.includes('"sessionId":"s1"')
            );
          }),
        ).toBe(true);
      });
    } finally {
      release();
    }
  });

  it("composer icon actions: submit-Send, disabled gates, and Close intent (plan 112 T8)", async () => {
    render(
      <CodingAgentPanel
        surface={surface}
        uiVersion={4}
        workspaceRoot="/tmp/ws"
      />,
    );
    const send = screen.getByRole("button", { name: "Send" });
    // Icon-only composition: glyph inside, native submit semantics kept.
    expect(send.querySelector("svg")).not.toBeNull();
    expect(send).toHaveAttribute("type", "submit");
    // Empty draft gates activation.
    expect(send).toHaveProperty("disabled", true);
    const close = screen.getByRole("button", { name: "Close" });
    expect(close.querySelector("svg")).not.toBeNull();
    fireEvent.click(close);
    await waitFor(() => {
      expect(sendRequest).toHaveBeenCalledWith(
        expect.stringContaining("coding-agent.close"),
      );
    });
  });

  it("streaming state offers Stop and keeps the composer as a steering lane (plan 118 port)", async () => {
    // Ported from the deleted landing panel's suite: the cancel handler and the
    // streaming state have exactly one owner — this panel drives the shared
    // transport's `agent.cancel`; nothing surface-specific owned them.
    render(
      <CodingAgentPanel
        surface={surface}
        uiVersion={4}
        workspaceRoot="/tmp/ws"
      />,
    );
    const store = (await import("../agent/state")).agentSession;
    const release = store.start();
    try {
      harness.emit({
        type: "STATE_SNAPSHOT",
        snapshot: { provider: "mock", model: "mini" },
        clientId: 1,
      });
      await waitFor(() =>
        expect(screen.getByLabelText(/Message/)).toBeEnabled(),
      );

      harness.emit({
        type: "RUN_STARTED",
        threadId: "s",
        runId: "r",
        clientId: 1,
      });
      expect(store.getSnapshot().status.streaming).toBe(true);
      expect(await screen.findByText("Working")).toBeInTheDocument();
      expect(
        screen.getByText("steer or stop from the composer"),
      ).toBeInTheDocument();
      // The composer stays enabled (steer), and the action swaps to Stop.
      expect(screen.getByLabelText(/Message/)).toHaveProperty(
        "placeholder",
        "Steer the agent, or wait",
      );
      fireEvent.click(screen.getByRole("button", { name: "Stop" }));
      await waitFor(() => {
        expect(sendRequest).toHaveBeenCalledWith(
          expect.stringContaining("agent.cancel"),
        );
      });
    } finally {
      release();
    }
  });

  it("composer intents are declared action targets (plan 117 follow-up)", async () => {
    render(
      <CodingAgentPanel
        surface={surface}
        uiVersion={4}
        workspaceRoot="/tmp/ws"
      />,
    );
    fireEvent.click(screen.getByRole("button", { name: "Close" }));
    await waitFor(() => {
      expect(sendRequest).toHaveBeenCalledWith(
        expect.stringContaining("sduiAction"),
      );
    });
    const sent = vi.mocked(sendRequest).mock.calls.map(([payload]) => {
      const parsed = JSON.parse(String(payload)) as {
        payload?: { intent?: { commandId?: string } };
      };
      return parsed.payload?.intent?.commandId;
    });
    const intents = sent.filter((id): id is string => typeof id === "string");
    expect(intents.length).toBeGreaterThan(0);
    for (const intent of intents) {
      expect(surface.actionTargets).toContain(intent);
    }
  });

  it("agent tab labels and approval actions keep text; no icon takeover (plan 112 T8)", async () => {
    render(
      <CodingAgentPanel
        surface={surface}
        uiVersion={4}
        workspaceRoot="/tmp/ws"
      />,
    );
    for (const tab of ["Files", "Memory", "Context"]) {
      const tabNode = screen.getByRole("tab", { name: tab });
      expect(tabNode.querySelector("svg")).toBeNull();
    }
    // Permission prompt (approval strip) keeps text buttons — no icon-only
    // Allow/Deny. Approval arrives as clay.permissionRequest (plan 108).
    const store = (await import("../agent/state")).agentSession;
    const release = store.start();
    try {
      harness.emit({
        type: "CUSTOM",
        name: "clay.permissionRequest",
        value: {
          sessionId: "s1",
          runId: "r1",
          requestId: "req1",
          toolName: "write",
        },
        clientId: 1,
      } as never);
      const allow = await screen.findByRole("button", { name: "Allow" });
      expect(allow.querySelector("svg")).toBeNull();
      expect(allow.textContent).toContain("Allow");
    } finally {
      release();
    }
  });

  it("Files tab (plan 118 task 36): shows the session-history empty state, never a file browser", () => {
    render(
      <CodingAgentPanel
        surface={surface}
        uiVersion={4}
        workspaceRoot="/tmp/ws"
      />,
    );
    expect(
      screen.getByText("No files in this session yet"),
    ).toBeInTheDocument();
    // The approved empty state says what the list *is*, and which keystrokes
    // move the session forward instead.
    expect(
      screen.getByText(/Files the agent reads, writes or searches appear here/),
    ).toBeInTheDocument();
    expect(screen.getByText(/mention a file/)).toBeInTheDocument();
    expect(screen.getAllByText(/workspace view/).length).toBeGreaterThan(0);
    // The editor-in-inspector path is gone: no document surface, no tree, and
    // no second way to open the workspace.
    expect(screen.queryByTestId("clay-editor")).not.toBeInTheDocument();
    expect(screen.queryByText(/Workspace browser/)).not.toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: /Open file|Resume session/ }),
    ).toBeNull();
  });

  it("Files tab (plan 118 task 36): lists the files this session touched, newest first", async () => {
    const onOpen = vi.fn();
    render(
      <CodingAgentPanel
        surface={surface}
        uiVersion={4}
        workspaceRoot="/tmp/ws"
        onOpenInWorkspace={onOpen}
      />,
    );
    const store = (await import("../agent/state")).agentSession;
    const release = store.start();
    try {
      fireEvent.click(screen.getByRole("tab", { name: "Files" }));
      // The transcript's tool rows carry the file records the server derived
      // from the call's own arguments (a resumed session rebuilds them from
      // the same rows, so the list survives a resume).
      harness.emit({
        type: "MESSAGES_SNAPSHOT",
        messages: [
          {
            id: "clay-tool-t1",
            role: "tool",
            content: 'read {"path":"src/main.rs"}',
            metadata: {
              clayKind: "tool",
              toolName: "read",
              sessionFile: { path: "src/main.rs", op: "read" },
            },
          },
          {
            id: "clay-tool-t2",
            role: "tool",
            content: 'write {"path":"plans/118.md"}',
            metadata: {
              clayKind: "tool",
              toolName: "write",
              sessionFile: { path: "plans/118.md", op: "write" },
            },
          },
        ],
        clientId: 1,
      } as never);

      // Basename first, directory muted, role word and its mark letter.
      const newest = await screen.findByRole("button", {
        name: "Open plans/118.md in the workspace view",
      });
      expect(within(newest).getByText("118.md")).toBeInTheDocument();
      expect(within(newest).getByText("plans")).toBeInTheDocument();
      expect(within(newest).getByText("created")).toBeInTheDocument();
      expect(within(newest).getByText("A")).toBeInTheDocument();
      const older = screen.getByRole("button", {
        name: "Open src/main.rs in the workspace view",
      });
      expect(within(older).getByText("read")).toBeInTheDocument();
      expect(within(older).getByText("R")).toBeInTheDocument();
      // Newest first: the write happened after the read.
      const list = screen.getByRole("list", {
        name: "Files this session has touched",
      });
      const rows = within(list).getAllByRole("button");
      expect(rows[0]).toBe(newest);
      expect(rows[1]).toBe(older);
      // Rows keep their text label (plan 112 T8: no icon takeover).
      expect(newest.querySelector("svg")).toBeNull();
      // The count and the filter are the other panels' affordances.
      expect(screen.getByLabelText("Filter session files")).toBeInTheDocument();
      expect(screen.getByText("2")).toBeInTheDocument();

      // ⏎ on a row switches the tab's view at that file (the row is a button:
      // Enter activates it, and the shell does the view switch + open).
      fireEvent.click(newest);
      expect(onOpen).toHaveBeenCalledWith("plans/118.md");
    } finally {
      release();
    }
  });

  it("Files tab (plan 118 task 36): filters by path and keeps the editor out", async () => {
    render(
      <CodingAgentPanel
        surface={surface}
        uiVersion={4}
        workspaceRoot="/tmp/ws"
      />,
    );
    const store = (await import("../agent/state")).agentSession;
    const release = store.start();
    try {
      fireEvent.click(screen.getByRole("tab", { name: "Files" }));
      harness.emit({
        type: "STATE_SNAPSHOT",
        snapshot: { provider: "mock", model: "mini" },
        clientId: 1,
      } as never);
      harness.emit({
        type: "MESSAGES_SNAPSHOT",
        messages: [
          {
            id: "clay-tool-t9",
            role: "tool",
            content: 'edit {"path":"src/shell/theme.rs"}',
            metadata: {
              clayKind: "tool",
              toolName: "edit",
              sessionFile: { path: "src/shell/theme.rs", op: "edit" },
            },
          },
        ],
        clientId: 1,
      } as never);
      expect(
        await screen.findByRole("button", {
          name: "Open src/shell/theme.rs in the workspace view",
        }),
      ).toBeInTheDocument();
      const filter = screen.getByLabelText("Filter session files");
      fireEvent.change(filter, { target: { value: "nope" } });
      expect(
        screen.queryByRole("button", {
          name: "Open src/shell/theme.rs in the workspace view",
        }),
      ).toBeNull();
      fireEvent.change(filter, { target: { value: "theme" } });
      expect(
        screen.getByRole("button", {
          name: "Open src/shell/theme.rs in the workspace view",
        }),
      ).toBeInTheDocument();
      // The Files tab never hosts a document surface.
      expect(screen.queryByTestId("clay-editor")).not.toBeInTheDocument();
    } finally {
      release();
    }
  });
});

describe("Settings tab (plan 117 follow-up)", () => {
  it("lists the agent's delivered files from the pane session and opens one", async () => {
    const sent: string[] = [];
    const session = createDocumentSession({
      send: async (payload) => {
        sent.push(String(payload));
      },
    });
    render(
      <CodingAgentPanel
        surface={surface}
        uiVersion={4}
        workspaceRoot="/tmp/ws"
        session={session}
      />,
    );
    // The listing is requested on this pane's own session — no shell state,
    // no separate side panel.
    fireEvent.click(await screen.findByRole("tab", { name: "Settings" }));
    await waitFor(() => {
      expect(sent.some((raw) => raw.includes("listAgentSettingsFiles"))).toBe(
        true,
      );
    });
    expect(screen.getByText("Loading…")).toBeInTheDocument();
    // The reply arrives as an agentSettingsFiles feature event on that same
    // session (the server's direct reply, plumbed through
    // ClientConnectionEvent::AgentSettingsFiles).
    act(() => {
      session.handleEnvelope({
        kind: "event",
        data: {
          kind: "agentSettingsFiles",
          data: {
            clientId: 1,
            files: [
              {
                name: "SYSTEM.md",
                displayPath: "/root/SYSTEM.md",
                sizeBytes: 0,
                modifiedMs: 1,
                edited: false,
              },
              {
                name: "skills/graft/SKILL.md",
                displayPath: "/root/skills/graft/SKILL.md",
                sizeBytes: 850,
                modifiedMs: 2,
                edited: false,
              },
            ],
          },
        },
      } as never);
    });
    const list = await screen.findByLabelText("Agent files");
    expect(list.textContent).toContain("SYSTEM.md");
    expect(list.textContent).toContain("skills/graft/SKILL.md");
    expect(list.textContent).toContain("built-in");
    // Selecting a file rides the document pipeline, then releases the agent
    // surface so the opened document is what the pane shows.
    fireEvent.click(screen.getByText("skills/graft/SKILL.md"));
    await waitFor(() => {
      expect(sent.some((raw) => raw.includes("openAgentSettingsFile"))).toBe(
        true,
      );
    });
    await waitFor(() => {
      expect(sendRequest).toHaveBeenCalledWith(
        expect.stringContaining("coding-agent.close"),
      );
    });
  });
});

describe("MCP UI surfaces (plan 117)", () => {
  it("Context tab lists connected servers with tool counts and marks hidden ones", async () => {
    render(
      <CodingAgentPanel
        surface={surface}
        uiVersion={4}
        workspaceRoot="/tmp/ws"
      />,
    );
    const store = (await import("../agent/state")).agentSession;
    const release = store.start();
    try {
      harness.emit({
        type: "STATE_SNAPSHOT",
        snapshot: {
          provider: "mock",
          model: "mini",
          mcpServers: [
            { serverId: "files", connected: true, tools: 2, error: "" },
            {
              serverId: "search",
              connected: false,
              tools: 0,
              error: "spawn failed",
            },
          ],
        },
        clientId: 1,
      } as never);
      // Capabilities are reference data, so they live in the Context tab
      // (DESIGN.md §12) rather than in the transcript.
      fireEvent.click(screen.getByRole("tab", { name: "Context" }));
      const mcpCard = await screen.findByLabelText("MCP servers connected");
      expect(within(mcpCard).getByText("files")).toBeInTheDocument();
      expect(within(mcpCard).getByText("2 tools")).toBeInTheDocument();
      expect(
        within(mcpCard).getByText("hidden: spawn failed"),
      ).toBeInTheDocument();
    } finally {
      release();
    }
  });

  it("server section is absent when no server is configured", async () => {
    render(
      <CodingAgentPanel
        surface={surface}
        uiVersion={4}
        workspaceRoot="/tmp/ws"
      />,
    );
    const store = (await import("../agent/state")).agentSession;
    const release = store.start();
    try {
      harness.emit({
        type: "STATE_SNAPSHOT",
        snapshot: { provider: "mock", model: "mini", mcpServers: [] },
        clientId: 1,
      } as never);
      // Flush the coalesced state render, then assert absence: nothing
      // will ever paint the section (no servers configured).
      await waitFor(() => {
        expect(screen.getByText("MCP none")).toBeInTheDocument();
      });
      fireEvent.click(screen.getByRole("tab", { name: "Context" }));
      expect(
        screen.queryByLabelText("MCP servers connected"),
      ).not.toBeInTheDocument();
    } finally {
      release();
    }
  });

  it("the foot shows the MCP summary and the Context tab the detail, both across transcript growth", async () => {
    render(
      <CodingAgentPanel
        surface={surface}
        uiVersion={4}
        workspaceRoot="/tmp/ws"
      />,
    );
    const store = (await import("../agent/state")).agentSession;
    const release = store.start();
    try {
      harness.emit({
        type: "STATE_SNAPSHOT",
        snapshot: {
          provider: "mock",
          model: "mini",
          mcpServers: [
            { serverId: "files", connected: true, tools: 3, error: "" },
            {
              serverId: "ghost",
              connected: false,
              tools: 0,
              error: "connection closed",
            },
          ],
        },
        clientId: 1,
      } as never);
      expect(
        await screen.findByText("MCP files · 3 tools · ghost · hidden"),
      ).toBeInTheDocument();
      fireEvent.click(screen.getByRole("tab", { name: "Context" }));
      const section = await screen.findByLabelText("MCP servers connected");
      expect(within(section).getByText("3 tools")).toBeInTheDocument();
      expect(
        within(section).getByText("hidden: connection closed"),
      ).toBeInTheDocument();

      // Messages arrive; both MCP surfaces persist (foot summary + section).
      harness.emit({
        type: "TEXT_MESSAGE_CONTENT",
        delta: "hello",
        messageId: "m1",
      } as never);
      expect(
        screen.getByLabelText("MCP servers connected"),
      ).toBeInTheDocument();
      expect(
        screen.getByText("MCP files · 3 tools · ghost · hidden"),
      ).toBeInTheDocument();
    } finally {
      release();
    }
  });

  it("@-mentions: dropdown merges catalog + files, filter narrows, selection embeds (plan 117)", async () => {
    render(
      <CodingAgentPanel
        surface={surface}
        uiVersion={4}
        workspaceRoot="/tmp/ws"
      />,
    );
    const store = (await import("../agent/state")).agentSession;
    const release = store.start();
    try {
      harness.emit({
        type: "STATE_SNAPSHOT",
        snapshot: {
          provider: "mock",
          model: "mini",
          sessionId: "s1",
          skills: [{ name: "brief", description: "Answer tersely." }],
        },
        clientId: 1,
      } as never);
      harness.emit({
        type: "CUSTOM",
        name: "clay.agentRpc",
        value: {
          code: "workspace.files",
          result: { files: ["src/main.rs", "README.md"] },
        },
      } as never);
      const composer = screen.getByLabelText("Message") as HTMLTextAreaElement;

      // Bare @ opens both sections.
      fireEvent.change(composer, { target: { value: "@" } });
      const dropdown = await screen.findByRole("listbox", { name: "Mentions" });
      expect(within(dropdown).getByText("@skill:brief")).toBeInTheDocument();
      expect(
        within(dropdown).getByText("@file:src/main.rs"),
      ).toBeInTheDocument();

      // Typing narrows across both kinds.
      fireEvent.change(composer, { target: { value: "@br" } });
      expect(within(dropdown).getByText("@skill:brief")).toBeInTheDocument();
      expect(
        within(dropdown).queryByText("@file:src/main.rs"),
      ).not.toBeInTheDocument();

      // @file: narrows to files only.
      fireEvent.change(composer, { target: { value: "@file:re" } });
      expect(within(dropdown).getByText("@file:README.md")).toBeInTheDocument();
      expect(
        within(dropdown).queryByText("@skill:brief"),
      ).not.toBeInTheDocument();

      // Selection embeds the mention token into the draft.
      fireEvent.click(within(dropdown).getByText("@file:README.md"));
      expect(composer.value).toBe("@file:README.md ");
      expect(
        screen.queryByRole("listbox", { name: "Mentions" }),
      ).not.toBeInTheDocument();
    } finally {
      release();
    }
  });

  it("@-mentions: Tab selects the highlighted match (plan 117)", async () => {
    render(
      <CodingAgentPanel
        surface={surface}
        uiVersion={4}
        workspaceRoot="/tmp/ws"
      />,
    );
    const store = (await import("../agent/state")).agentSession;
    const release = store.start();
    try {
      harness.emit({
        type: "STATE_SNAPSHOT",
        snapshot: {
          provider: "mock",
          model: "mini",
          sessionId: "s1",
          skills: [{ name: "brief", description: "Answer tersely." }],
        },
        clientId: 1,
      } as never);
      const composer = screen.getByLabelText("Message") as HTMLTextAreaElement;
      fireEvent.change(composer, { target: { value: "@" } });
      await screen.findByRole("listbox", { name: "Mentions" });
      fireEvent.keyDown(composer, { key: "Tab" });
      expect(composer.value).toBe("@skill:brief ");
    } finally {
      release();
    }
  });

  it("token meter: compact occupancy vs ceiling with diagnostic threshold tones (plan 117)", async () => {
    render(
      <CodingAgentPanel
        surface={surface}
        uiVersion={4}
        workspaceRoot="/tmp/ws"
      />,
    );
    const store = (await import("../agent/state")).agentSession;
    const release = store.start();
    const emitState = (
      tokens: number | null,
      model = "mini",
      window = 100_000,
    ) => {
      harness.emit({
        type: "STATE_SNAPSHOT",
        snapshot: {
          provider: "mock",
          model,
          models: [
            { provider: "mock", model: "mini", contextWindow: window },
            { provider: "mock", model: "big", contextWindow: 1_000_000 },
          ],
          ...(tokens === null ? {} : { contextTokens: tokens }),
        },
        clientId: 1,
      } as never);
    };
    try {
      const meter = () => screen.getByTitle(/Context usage/);

      // Initial state carries the window; unreported usage estimates from
      // the transcript (empty → 0) so the meter renders from the start.
      emitState(null);
      const initial = await screen.findByTitle(/Context usage/);
      expect(initial).toHaveTextContent("0/100k");
      expect(initial.className).not.toMatch(/meterWarning|meterError/);

      // Thresholds: 59% normal, 61% warning, 79% warning, 81% error.
      emitState(59_000);
      await waitFor(() => expect(meter()).toHaveTextContent("59k/100k"));
      expect(meter().className).not.toMatch(/meterWarning|meterError/);
      emitState(61_000);
      await waitFor(() => expect(meter().className).toMatch(/meterWarning/));
      emitState(79_000);
      await waitFor(() => expect(meter().className).toMatch(/meterWarning/));
      emitState(81_000);
      await waitFor(() => expect(meter().className).toMatch(/meterError/));

      // Compact format: 220000/270000 → `220k/270k` (81% — error tone).
      emitState(220_000, "mini", 270_000);
      await waitFor(() => expect(meter()).toHaveTextContent("220k/270k"));

      // Ceiling follows the active model.
      emitState(220_000, "big", 1_000_000);
      await waitFor(() => expect(meter()).toHaveTextContent("220k/1m"));
    } finally {
      release();
    }
  });

  it("token meter: heuristic estimates occupancy when usage is unreported (plan 117)", async () => {
    render(
      <CodingAgentPanel
        surface={surface}
        uiVersion={4}
        workspaceRoot="/tmp/ws"
      />,
    );
    const store = (await import("../agent/state")).agentSession;
    const release = store.start();
    try {
      // 7200 chars ÷ 3.6 chars/token (claude family) = 2000 tokens.
      harness.emit({
        type: "STATE_SNAPSHOT",
        snapshot: {
          provider: "mock",
          model: "claude-x",
          models: [
            { provider: "mock", model: "claude-x", contextWindow: 270_000 },
          ],
        },
        clientId: 1,
      } as never);
      harness.emit({
        type: "MESSAGES_SNAPSHOT",
        messages: [{ id: "m0", role: "user", content: "x".repeat(7200) }],
        clientId: 1,
      } as never);
      await waitFor(() =>
        expect(screen.getByTitle(/Context usage/)).toHaveTextContent("2k/270k"),
      );
    } finally {
      release();
    }
  });

  it("effort dropdown is active from session start (plan 117): levels resolve from the model inventory before any prompt", async () => {
    render(
      <CodingAgentPanel
        surface={surface}
        uiVersion={4}
        workspaceRoot="/tmp/ws"
      />,
    );
    const store = (await import("../agent/state")).agentSession;
    const release = store.start();
    try {
      // Mount-time inventory STATE: trio + models inventory, NO session and
      // no `effort` key — exactly what a fresh webview holds.
      harness.emit({
        type: "STATE_SNAPSHOT",
        snapshot: {
          provider: "mock",
          model: "mini",
          models: [
            {
              provider: "mock",
              model: "mini",
              displayName: "Mock demo",
              thinkingLevels: ["low", "medium", "high"],
            },
          ],
        },
        clientId: 1,
      } as never);
      // The cataloged dropdown renders pre-first-message (React Aria
      // Select trigger labeled "Effort"); selecting a level rides the
      // next prompt through the existing pendingEffort path.
      expect(await screen.findByText("Effort")).toBeInTheDocument();
    } finally {
      release();
    }
  });

  it("is the header title, lists only the configured agents, and marks the current one", async () => {
    const session = createDocumentSession({ send: async () => undefined });
    render(
      <CodingAgentPanel
        surface={surface}
        uiVersion={4}
        workspaceRoot="/tmp/ws"
        session={session}
        agentType="coding-agent"
        onPickAgent={() => undefined}
      />,
    );
    act(() => {
      session.handleEnvelope({
        kind: "event",
        data: {
          kind: "launcherEntries",
          // The server's ServerMessage shape: the listing rides under
          // `entries` (the same envelope the launcher parses).
          data: {
            clientId: 1,
            entries: {
              workspaces: [],
              pruned: 0,
              agents: [
                {
                  name: "coding-agent",
                  label: "Coding Agent",
                  configRoot: "~/.clay/agents/coding-agent",
                  skillCount: 3,
                },
                {
                  name: "reviewer",
                  label: "Reviewer",
                  configRoot: "~/.clay/agents/reviewer",
                  skillCount: 1,
                },
              ],
            },
          },
        },
      } as never);
    });

    // The title reads the current agent, and it consumes the agentPicker
    // trigger family (not the plain dropdown).
    const trigger = await waitFor(() => {
      const button = document.querySelector<HTMLButtonElement>(
        "[data-agent-pick] button",
      );
      expect(button).toBeTruthy();
      return button as HTMLButtonElement;
    });
    expect(trigger).toHaveAttribute("data-clay-component", "agentPicker");
    expect(trigger).toHaveAttribute("data-clay-slot", "trigger");
    expect(trigger).toHaveTextContent("Coding Agent");

    // Opening the menu lists exactly the server-listed types.
    act(() => {
      fireEvent.click(trigger);
    });
    const currentRow = await screen.findByRole("option", {
      name: /Coding Agent/,
    });
    void currentRow;
    expect(currentRow).toHaveAttribute("aria-selected", "true");
    const reviewerRow = screen.getByRole("option", { name: /Reviewer/ });
    expect(reviewerRow).toHaveAttribute("aria-selected", "false");
    // The menu names where more agent types come from and never fabricates
    // one the data root does not hold.
    expect(screen.getByText(/from ~\/\.clay\/agents\//)).toBeInTheDocument();
    expect(screen.queryByRole("option", { name: /Ghost/ })).toBeNull();
  });

  it("hands the picked type to the tab and resets the pending effort", async () => {
    const picked: Array<string | null> = [];
    const session = createDocumentSession({ send: async () => undefined });
    render(
      <CodingAgentPanel
        surface={surface}
        uiVersion={4}
        workspaceRoot="/tmp/ws"
        session={session}
        agentType="coding-agent"
        onPickAgent={(agent) => picked.push(agent)}
      />,
    );
    act(() => {
      session.handleEnvelope({
        kind: "event",
        data: {
          kind: "launcherEntries",
          // The server's ServerMessage shape: the listing rides under
          // `entries` (the same envelope the launcher parses).
          data: {
            clientId: 1,
            entries: {
              workspaces: [],
              pruned: 0,
              agents: [
                {
                  name: "coding-agent",
                  label: "Coding Agent",
                  configRoot: "~/.clay/agents/coding-agent",
                  skillCount: 3,
                },
                {
                  name: "reviewer",
                  label: "Reviewer",
                  configRoot: "~/.clay/agents/reviewer",
                  skillCount: 1,
                },
              ],
            },
          },
        },
      } as never);
    });
    const trigger = await waitFor(() => {
      const button = document.querySelector<HTMLButtonElement>(
        "[data-agent-pick] button",
      );
      expect(button).toBeTruthy();
      return button as HTMLButtonElement;
    });
    act(() => {
      fireEvent.click(trigger);
    });
    act(() => {
      fireEvent.click(screen.getByRole("option", { name: /Reviewer/ }));
    });
    expect(picked).toEqual(["reviewer"]);
    // The already-current row is a no-op: no redundant tab command.
    act(() => {
      fireEvent.click(trigger);
    });
    act(() => {
      fireEvent.click(screen.getByRole("option", { name: /Coding Agent/ }));
    });
    expect(picked).toEqual(["reviewer"]);
  });

  it("keeps the picker inert when there is no shell to switch through", async () => {
    render(
      <CodingAgentPanel
        surface={surface}
        uiVersion={4}
        workspaceRoot="/tmp/ws"
        agentType="coding-agent"
      />,
    );
    const trigger = await waitFor(() => {
      const button = document.querySelector<HTMLButtonElement>(
        "[data-agent-pick] button",
      );
      expect(button).toBeTruthy();
      return button as HTMLButtonElement;
    });
    expect(trigger).toBeDisabled();
  });
});

describe("Per-turn agent labels (plan 118 task 35)", () => {
  it("labels turns with their producer and notes the switch when the tab changes agent", async () => {
    render(
      <CodingAgentPanel
        surface={surface}
        uiVersion={4}
        workspaceRoot="/tmp/ws"
        agentType="reviewer"
      />,
    );
    const store = (await import("../agent/state")).agentSession;
    const release = store.start();
    try {
      harness.emit({
        type: "MESSAGES_SNAPSHOT",
        messages: [
          {
            id: "m0",
            role: "user",
            content: "first, as the coding agent",
            metadata: { agent: "coding-agent" },
          },
          {
            id: "m1",
            role: "assistant",
            content: "done, then switched",
            metadata: { agent: "reviewer" },
          },
          {
            id: "m2",
            role: "assistant",
            content: "and a second turn from the reviewer",
            metadata: { agent: "reviewer" },
          },
        ],
        clientId: 1,
      } as never);
      expect(
        await screen.findByText(/first, as the coding agent/),
      ).toBeInTheDocument();
      // The previous agent's turn keeps its label...
      expect(screen.getByText("Coding Agent")).toBeInTheDocument();
      expect(screen.getAllByText("Reviewer").length).toBeGreaterThan(0);
      // ...and exactly one switch note marks the boundary.
      expect(screen.getAllByText("Switched to Reviewer")).toHaveLength(1);
    } finally {
      release();
    }
  });

  it("shows no agent labels when one agent wrote the whole transcript", async () => {
    render(
      <CodingAgentPanel
        surface={surface}
        uiVersion={4}
        workspaceRoot="/tmp/ws"
        agentType="coding-agent"
      />,
    );
    const store = (await import("../agent/state")).agentSession;
    const release = store.start();
    try {
      harness.emit({
        type: "MESSAGES_SNAPSHOT",
        messages: [
          {
            id: "m0",
            role: "user",
            content: "only one agent here",
            metadata: { agent: "coding-agent" },
          },
        ],
        clientId: 1,
      } as never);
      expect(
        await screen.findByText(/only one agent here/),
      ).toBeInTheDocument();
      const log = screen.getByRole("log", { name: "Transcript" });
      expect(within(log).queryByText(/Switched to/)).toBeNull();
      // One agent in the transcript: the head keeps the generic role label
      // instead of naming the agent on every turn.
      expect(within(log).queryByText("Coding Agent")).toBeNull();
      expect(within(log).getByText("you")).toBeInTheDocument();
    } finally {
      release();
    }
  });
});
