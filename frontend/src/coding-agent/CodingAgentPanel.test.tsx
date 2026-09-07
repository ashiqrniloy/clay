// @vitest-environment jsdom
// CodingAgentPanel surface tests (plan 108 task 8): provenance-exact host
// rendering for the bundled @clay/coding-agent split surface. Mirrors the
// ChatPanel test harness: the AG-UI relay is mocked; the stream seam and the
// Tauri bridge are the only outs.

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
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

import { CodingAgentPanel, groupModelsByProvider } from "./CodingAgentPanel";

/** Mirrors the panel's worker-dropdown clear value (plan 109 I8). */
const OM_WORKER_CLEAR_VALUE = "om-worker:none";
import { sendRequest } from "../bridge/client";
import { resetChatAgentForTests } from "../agent/state";
import { createDocumentSession } from "../editor/sync/session";
import type { BootstrapDto } from "../bridge/types";

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

  it("shows the full content of a selected card in Session Info (plan 109 I10)", async () => {
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
      fireEvent.click(
        screen.getByRole("button", { name: /shell exited 0/ }),
      );
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
    const store = (await import("../agent/state")).chatAgent;
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
    const store = (await import("../agent/state")).chatAgent;
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
    const store = (await import("../agent/state")).chatAgent;
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
      expect((await screen.findAllByText(/mock\/mini/)).length).toBeGreaterThan(0);
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
      expect(
        screen.getAllByText(
          (_, element) => element?.textContent === "/tmp/ws · git main",
        ).length,
      ).toBeGreaterThan(0);
      // Plan 109 R3: the strip reflects daemon-reported extensions.
      expect(screen.getByText("Extensions: wiki")).toBeInTheDocument();
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
    const store = (await import("../agent/state")).chatAgent;
    const release = store.start();
    try {
      harness.emit({
        type: "STATE_SNAPSHOT",
        snapshot: { branch: "", extensions: [], mcpServers: [] },
        clientId: 1,
      } as never);
      // Not a repo (or not yet read) → the `—` fallback; no extensions
      // reported → the segment is omitted, MCP stays.
      await screen.findByText(/MCP: none/);
      expect(
        screen.getAllByText(
          (_, element) => element?.textContent === "/tmp/ws · git —",
        ).length,
      ).toBeGreaterThan(0);
      expect(screen.queryByText(/^Extensions:/)).not.toBeInTheDocument();
      expect(screen.getByText(/MCP: none/)).toBeInTheDocument();
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
        effortChord={{ shift: true, ctrl: false, alt: false, meta: false, key: "Tab" }}
      />,
    );
    const store = (await import("../agent/state")).chatAgent;
    const release = store.start();
    try {
      harness.emit({
        type: "STATE_SNAPSHOT",
        snapshot: {
          provider: "mock",
          model: "mini",
          effortLevels: ["low", "medium", "high"],
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
        effortChord={{ shift: false, ctrl: true, alt: false, meta: false, key: "e" }}
      />,
    );
    const store = (await import("../agent/state")).chatAgent;
    const release = store.start();
    try {
      harness.emit({
        type: "STATE_SNAPSHOT",
        snapshot: {
          provider: "mock",
          model: "mini",
          effortLevels: ["low", "high"],
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
    const store = (await import("../agent/state")).chatAgent;
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
    const store = (await import("../agent/state")).chatAgent;
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
      expect(
        await screen.findByText("hello full content"),
      ).toBeInTheDocument();

      // Back restores the category list.
      fireEvent.click(screen.getByRole("button", { name: "Back" }));
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
    const store = (await import("../agent/state")).chatAgent;
    const release = store.start();
    try {
      harness.emit({
        type: "STATE_SNAPSHOT",
        snapshot: {
          sessionId: "s1",
          provider: "mock",
          model: "mini",
          models: [{ provider: "mock", model: "mini", displayName: "Mock demo" }],
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
              { id: "e1#1", kind: "observation", summary: "User prefers dark mode" },
              { id: "e2#1", kind: "reflection", summary: "User values dark mode" },
              { id: "e3#dropped", kind: "drop", summary: "Dropped 1 observation" },
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
        fireEvent.change(hiddenSelect!, { target: { value: "model:mock/mini" } });
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

  it("Files tab (plan 109 I6): shows the empty state and no workspace tree until a file is selected", () => {
    render(
      <CodingAgentPanel
        surface={surface}
        uiVersion={4}
        workspaceRoot="/tmp/ws"
      />,
    );
    expect(
      screen.getByText("Open a file to work on it alongside the agent."),
    ).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Open file" })).toBeInTheDocument();
    expect(screen.queryByTestId("clay-editor")).not.toBeInTheDocument();
    // The workspace browser tree is the shell file browser's sole instance
    // (left workspace tab); the Files tab no longer embeds it.
    expect(screen.queryByText(/Workspace browser/)).not.toBeInTheDocument();
  });

  it("Files tab (plan 109 I6): follows workspace selection into the shared editor view", () => {
    const session = createDocumentSession({ send: async () => undefined });
    session.installInitial(bootstrapDto);
    // Simulate the shell selection flow: the pane session opens the file
    // (store path is the selection state the editor view follows).
    session.store.update({ path: "notes/plan.md" });
    render(
      <CodingAgentPanel
        surface={surface}
        uiVersion={4}
        workspaceRoot="/tmp/ws"
        session={session}
      />,
    );
    expect(screen.getByTestId("clay-editor")).toBeInTheDocument();
    expect(screen.queryByText(/alongside the agent/)).not.toBeInTheDocument();
    // Selecting another file rebinds the same session — no second pipeline.
    session.store.update({ path: "src/lib.rs" });
    expect(screen.getByTestId("clay-editor")).toBeInTheDocument();
  });
});

/** Minimal bootstrap for a mounted document session (mirrors editor tests). */
const bootstrapDto = {
  clientId: 1,
  protocolVersion: 28,
  endpoint: "test",
  generation: 1,
  initialDocument: {
    documentId: 1,
    version: 1,
    head: { totalBytes: 4, firstChunk: "seed" },
    access: { editable: { leaseId: 1 } },
    workspaceRoot: "/tmp/ws",
  },
  behaviorManifest: {
    manifestId: "m",
    behaviorVersion: 2,
    commands: [],
    keymaps: [],
  },
} as unknown as BootstrapDto;
