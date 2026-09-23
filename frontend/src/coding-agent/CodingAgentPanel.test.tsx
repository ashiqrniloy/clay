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
import { createAgentSession } from "../agent/state";
import { createDocumentSession } from "../editor/sync/session";

beforeEach(() => {
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
  it("renders the approved composition: transcript, state strip, and inspector — and no composer", async () => {
    render(<CodingAgentPanel surface={surface} uiVersion={4} />);
    expect(
      screen.getByRole("region", { name: "Coding Agent" }),
    ).toBeInTheDocument();
    expect(screen.getByRole("log", { name: "Transcript" })).toBeInTheDocument();
    for (const tab of ["Files", "Memory", "Context"]) {
      expect(screen.getByRole("tab", { name: tab })).toBeInTheDocument();
    }
    // Plan 124: the composer, the agent controls, and the session-environment
    // foot belong to the shell's lane — the view draws none of them, and the
    // state strip is its last row (directly above the lane).
    expect(screen.queryByLabelText(/Message/)).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Send" })).toBeNull();
    expect(screen.queryByText("git —")).not.toBeInTheDocument();
    const strip = screen.getByText("Ready").closest("[role='status']");
    expect(strip?.parentElement?.lastElementChild).toBe(strip);
  });

  it("state strip: the working bars replace the tone dot exactly while a turn is in flight", async () => {
    const store = createAgentSession({});
    render(<CodingAgentPanel agent={store} surface={surface} uiVersion={4} />);
    const release = store.start();
    try {
      // At rest: the steady tone dot, no motion in the strip.
      await screen.findByText("Ready");
      expect(
        document.querySelector("[data-clay-component='statusDot']"),
      ).not.toBeNull();
      expect(
        document.querySelectorAll("[data-working-bars] span"),
      ).toHaveLength(0);

      harness.emit({
        type: "STATE_SNAPSHOT",
        snapshot: { provider: "mock", model: "mini" },
        clientId: 1,
      } as never);
      harness.emit({ type: "RUN_STARTED", threadId: "s", runId: "r" } as never);
      expect(await screen.findByText("Working")).toBeInTheDocument();
      // Three bars, decoration over the strip's own role="status" text, and no
      // dot: the run's motion is the bars and the window mark, nothing else.
      const bars = document.querySelector("[data-working-bars]");
      expect(bars).not.toBeNull();
      expect(bars?.getAttribute("aria-hidden")).toBe("true");
      expect(bars?.querySelectorAll("span")).toHaveLength(3);
      expect(
        document.querySelector("[data-clay-component='statusDot']"),
      ).toBeNull();

      harness.emit({
        type: "RUN_FINISHED",
        threadId: "s",
        runId: "r",
      } as never);
      await waitFor(() =>
        expect(
          document.querySelector("[data-clay-component='statusDot']"),
        ).not.toBeNull(),
      );
      expect(document.querySelector("[data-working-bars]")).toBeNull();
    } finally {
      release();
    }
  });

  it("renders transcript boxes from the AG-UI stream and colors them by type", async () => {
    const store = createAgentSession({});
    render(<CodingAgentPanel agent={store} surface={surface} uiVersion={4} />);
    // Seed via the shared relay seam (same flow the transport tests use): the
    // out-of-run snapshot path applies messages + state and notifies.
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
      // The Context tab is where the per-server detail lives (the lane's foot
      // carries the one-line summary, plan 124).
      fireEvent.click(screen.getByRole("tab", { name: "Context" }));
      expect(await screen.findByText("2 tools")).toBeInTheDocument();
    } finally {
      release();
    }
  });

  it("shows the full content of a selected card in Session Info (plan 109 I10)", async () => {
    const store = createAgentSession({});
    render(<CodingAgentPanel agent={store} surface={surface} uiVersion={4} />);
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
    const store = createAgentSession({});
    render(<CodingAgentPanel agent={store} surface={surface} uiVersion={4} />);
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
    const store = createAgentSession({});
    render(<CodingAgentPanel agent={store} surface={surface} uiVersion={4} />);
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
    const store = createAgentSession({});
    render(<CodingAgentPanel agent={store} surface={surface} uiVersion={4} />);
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
    const store = createAgentSession({});
    render(<CodingAgentPanel agent={store} surface={surface} uiVersion={4} />);
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
    const store = createAgentSession({});
    render(<CodingAgentPanel agent={store} surface={surface} uiVersion={4} />);
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
      const observationLabel = (await screen.findAllByText("Mock demo"))[0];
      expect(observationLabel).toBeTruthy();
      const observationTrigger = observationLabel?.closest("button");
      expect(observationTrigger).toBeTruthy();
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
        fireEvent.change(hiddenSelect as HTMLSelectElement, {
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

  it("agent tab labels keep text; no icon takeover (plan 112 T8)", () => {
    render(<CodingAgentPanel surface={surface} uiVersion={4} />);
    for (const tab of ["Files", "Memory", "Context"]) {
      const tabNode = screen.getByRole("tab", { name: tab });
      expect(tabNode.querySelector("svg")).toBeNull();
    }
    // The approval strip (whose Allow/Deny are text buttons too) lives in the
    // shell's lane since plan 124 — see AgentLane.test.tsx.
  });

  it("a store handed in by the host is never re-created here (plan 124)", async () => {
    // The shell's host resolves the tab's store and runs its bootstrap, because
    // the lane is mounted for every tab; the view must not create a second one
    // or re-ask for the tab's STATE.
    const store = createAgentSession({});
    render(<CodingAgentPanel agent={store} surface={surface} uiVersion={4} />);
    const asks = vi
      .mocked(sendRequest)
      .mock.calls.map(([payload]) => String(payload));
    expect(asks.some((payload) => payload.includes("listSessions"))).toBe(
      false,
    );
    expect(asks.some((payload) => payload.includes("tabState"))).toBe(false);
    // A standalone mount (no host store) does keep asking for it itself.
    cleanup();
    vi.mocked(sendRequest).mockClear();
    render(<CodingAgentPanel surface={surface} uiVersion={4} />);
    await waitFor(() => {
      const own = vi
        .mocked(sendRequest)
        .mock.calls.map(([payload]) => String(payload));
      expect(own.some((payload) => payload.includes("listSessions"))).toBe(
        true,
      );
      expect(own.some((payload) => payload.includes("tabState"))).toBe(true);
    });
  });

  it("Files tab (plan 118 task 36): shows the session-history empty state, never a file browser", () => {
    render(<CodingAgentPanel surface={surface} uiVersion={4} />);
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
    const store = createAgentSession({});
    const onOpen = vi.fn();
    render(
      <CodingAgentPanel
        agent={store}
        surface={surface}
        uiVersion={4}
        onOpenInWorkspace={onOpen}
      />,
    );
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
    const store = createAgentSession({});
    render(<CodingAgentPanel agent={store} surface={surface} uiVersion={4} />);
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
      <CodingAgentPanel surface={surface} uiVersion={4} session={session} />,
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
    const store = createAgentSession({});
    render(<CodingAgentPanel agent={store} surface={surface} uiVersion={4} />);
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
    const store = createAgentSession({});
    render(<CodingAgentPanel agent={store} surface={surface} uiVersion={4} />);
    const release = store.start();
    try {
      harness.emit({
        type: "STATE_SNAPSHOT",
        snapshot: { provider: "mock", model: "mini", mcpServers: [] },
        clientId: 1,
      } as never);
      // The state has landed (the panel's own strip reports it), so the
      // section's absence is the answer, not a slow render.
      await screen.findByText("Ready");
      fireEvent.click(screen.getByRole("tab", { name: "Context" }));
      expect(
        screen.queryByLabelText("MCP servers connected"),
      ).not.toBeInTheDocument();
    } finally {
      release();
    }
  });

  it("the Context tab carries the per-server MCP detail across transcript growth", async () => {
    const store = createAgentSession({});
    render(<CodingAgentPanel agent={store} surface={surface} uiVersion={4} />);
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
      fireEvent.click(screen.getByRole("tab", { name: "Context" }));
      const section = await screen.findByLabelText("MCP servers connected");
      expect(within(section).getByText("3 tools")).toBeInTheDocument();
      expect(
        within(section).getByText("hidden: connection closed"),
      ).toBeInTheDocument();

      // Messages arrive; the section persists (the lane's foot carries the
      // one-line summary — plan 124).
      harness.emit({
        type: "TEXT_MESSAGE_CONTENT",
        delta: "hello",
        messageId: "m1",
      } as never);
      expect(
        screen.getByLabelText("MCP servers connected"),
      ).toBeInTheDocument();
    } finally {
      release();
    }
  });
});

describe("Per-turn agent labels (plan 118 task 35)", () => {
  it("labels turns with their producer and notes the switch when the tab changes agent", async () => {
    const store = createAgentSession({});
    render(<CodingAgentPanel agent={store} surface={surface} uiVersion={4} />);
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
    const store = createAgentSession({});
    render(<CodingAgentPanel agent={store} surface={surface} uiVersion={4} />);
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
