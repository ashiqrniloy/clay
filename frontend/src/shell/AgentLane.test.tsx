// @vitest-environment jsdom
// Agent lane tests (plan 124 task 6): the shell's persistent bottom section —
// the composer (no Send button; `↵` sends), the tab's agent controls inside the
// composer box, the approval strip, and the session-environment foot. The lane
// owns the send authority, so these are the tests the panel's composer/header
// suite became.

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
  act,
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
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

import { AgentLane } from "./AgentLane";
import { paletteItems, paletteMenu, paletteStub } from "../test/palette-stub";
import { agentLane } from "./layout-state";
import { sendRequest } from "../bridge/client";
import { createAgentSession } from "../agent/state";
import { createDocumentSession } from "../editor/sync/session";

beforeEach(() => {
  // Intents are asserted per test (the "every intent is declared" check in
  // particular), so no test inherits another's send history.
  vi.clearAllMocks();
});

afterEach(() => {
  cleanup();
});

/** The shipped action targets (`packages/coding-agent/package.json`): an intent
 *  outside this list is rejected server-side, and a dead button was the bug the
 *  closed allowlist was added for (plan 117 follow-up). */
const ACTION_TARGETS = [
  "coding-agent.profile",
  "coding-agent.close",
  "agent.clientOpenModelPicker",
  "agent.clientOpenSessionPicker",
  "documents.clientOpenFileDialog",
];

/** The lane in its normal state: a tab with an agent. The agent-less tab is
 *  its own explicit case (see below), not the default. */
function mount(
  store = createAgentSession({}),
  props: {
    agentType?: string | null;
    onPickAgent?: (id: string | null) => void;
  } = {},
) {
  const utils = render(
    <AgentLane
      store={store}
      uiVersion={4}
      workspaceRoot="/tmp/ws"
      agentType={
        props.agentType === undefined ? "coding-agent" : props.agentType
      }
      onPickAgent={props.onPickAgent ?? (() => undefined)}
      palette={paletteStub()}
    />,
  );
  return { store, ...utils };
}

function seedState(state: Record<string, unknown>) {
  harness.emit({
    type: "STATE_SNAPSHOT",
    snapshot: state,
    clientId: 1,
  } as never);
}

describe("AgentLane composition", () => {
  it("is the shell's bottom section: composer, agent controls, and environment foot", async () => {
    const { store } = mount();
    const release = store.start();
    try {
      expect(
        screen.getByRole("contentinfo", { name: "Agent lane" }),
      ).toBeInTheDocument();
      // The field shell carries the tab's agent controls as its second row and
      // the hint row that says `↵` sends (no Send button exists).
      const field = screen.getByLabelText("Message");
      expect(field.closest("form")).not.toBeNull();
      // The hint row is what teaches `↵` — because no Send button exists.
      expect(screen.getByText(/commands/)).toBeInTheDocument();
      expect(screen.queryByRole("button", { name: "Send" })).toBeNull();
      expect(screen.queryByRole("button", { name: "Close" })).toBeNull();
      // Session environment: workspace root, branch, MCP summary.
      expect(screen.getByText("/tmp/ws")).toBeInTheDocument();
      expect(screen.getByText("git —")).toBeInTheDocument();
      expect(await screen.findByText("MCP none")).toBeInTheDocument();
    } finally {
      release();
    }
  });

  it("routes a `/` draft into the tab's palette session (plan 124)", async () => {
    const { store } = mount();
    const release = store.start();
    try {
      // The daemon registry still rides STATE; the rows it used to complete now
      // come from the server's command catalogue through the palette session,
      // so the field asks for that session and the foot still states the
      // daemon's own environment (plan 109 R2/R3).
      seedState({
        provider: "mock",
        model: "mini",
        commands: [{ name: "/compact", description: "Compact it." }],
        branch: "main",
        extensions: ["wiki"],
      });
      const composer = await screen.findByLabelText("Message");
      fireEvent.change(composer, { target: { value: "/open" } });
      // No inline slash list is left: the palette owns that surface.
      expect(screen.queryByRole("listbox")).toBeNull();
      expect(await screen.findByText("git main")).toBeInTheDocument();
      expect(await screen.findByText("extensions wiki")).toBeInTheDocument();
    } finally {
      release();
    }
  });

  it("reveals a hidden lane when a palette session opens (plan 124)", async () => {
    // The palette is the composer's menu, so a window whose lane is hidden has
    // nowhere to draw it: the trigger and the chord reveal the lane first.
    agentLane.resetForTests();
    agentLane.setVisible(false);
    const store = createAgentSession({});
    const palette = paletteStub();
    const view = render(
      <AgentLane
        store={store}
        uiVersion={4}
        workspaceRoot="/tmp/ws"
        agentType="coding-agent"
        palette={palette}
      />,
    );
    expect(screen.getByLabelText("Agent lane")).not.toBeVisible();
    view.rerender(
      <AgentLane
        store={store}
        uiVersion={4}
        workspaceRoot="/tmp/ws"
        agentType="coding-agent"
        palette={paletteStub({ menu: paletteMenu(paletteItems) })}
      />,
    );
    expect(screen.getByLabelText("Agent lane")).toBeVisible();
    agentLane.resetForTests();
  });

  it("renders `git —` and omits the extension segment without daemon data (plan 109 R2/R3)", async () => {
    const { store } = mount();
    const release = store.start();
    try {
      seedState({ branch: "", extensions: [], mcpServers: [] });
      // Not a repo (or not yet read) → the `—` fallback; no extensions
      // reported → the segment is omitted; MCP stays.
      await screen.findByText("MCP none");
      expect(screen.getByText("/tmp/ws")).toBeInTheDocument();
      expect(screen.getByText("git —")).toBeInTheDocument();
      expect(screen.queryByText(/^extensions /)).not.toBeInTheDocument();
    } finally {
      release();
    }
  });
});

describe("AgentLane composer", () => {
  it("keeps Shift+Tab a no-op when the model exposes no effort levels", () => {
    mount();
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
    const store = createAgentSession({});
    render(
      <AgentLane
        store={store}
        uiVersion={4}
        workspaceRoot="/tmp/ws"
        agentType="coding-agent"
        effortChord={{
          shift: true,
          ctrl: false,
          alt: false,
          meta: false,
          key: "Tab",
        }}
        palette={paletteStub()}
      />,
    );
    const release = store.start();
    try {
      seedState({
        provider: "mock",
        model: "mini",
        providers: [{ id: "mock", configured: true }],
        models: [
          {
            provider: "mock",
            model: "mini",
            displayName: "Mock demo",
            thinkingLevels: ["low", "medium", "high"],
          },
        ],
        effort: "medium",
      });
      // The cataloged dropdown appears for declared levels; the trigger reads
      // the active level.
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
    const store = createAgentSession({});
    render(
      <AgentLane
        store={store}
        uiVersion={4}
        workspaceRoot="/tmp/ws"
        agentType="coding-agent"
        effortChord={{
          shift: false,
          ctrl: true,
          alt: false,
          meta: false,
          key: "e",
        }}
        palette={paletteStub()}
      />,
    );
    const release = store.start();
    try {
      seedState({
        provider: "mock",
        model: "mini",
        providers: [{ id: "mock", configured: true }],
        models: [
          {
            provider: "mock",
            model: "mini",
            displayName: "Mock demo",
            thinkingLevels: ["low", "high"],
          },
        ],
      });
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
    mount();
    const composer = screen.getByLabelText(/Message/);
    fireEvent.change(composer, { target: { value: "/resume" } });
    fireEvent.submit(composer.closest("form") as HTMLFormElement);
    expect(sendRequest).toHaveBeenCalledWith(
      expect.stringContaining("clientOpenSessionPicker"),
    );
  });

  it("/model opens the daemon model picker instead of prompting", () => {
    mount();
    const composer = screen.getByLabelText(/Message/);
    fireEvent.change(composer, { target: { value: "/model" } });
    fireEvent.submit(composer.closest("form") as HTMLFormElement);
    expect(sendRequest).toHaveBeenCalledWith(
      expect.stringContaining("clientOpenModelPicker"),
    );
  });

  it("lane intents are declared action targets (plan 117 follow-up)", async () => {
    mount();
    const composer = screen.getByLabelText(/Message/);
    fireEvent.change(composer, { target: { value: "/model" } });
    fireEvent.submit(composer.closest("form") as HTMLFormElement);
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
      expect(ACTION_TARGETS).toContain(intent);
    }
  });

  it("sends a prompt on `↵` and steers while a run is live (no Send button)", async () => {
    const store = createAgentSession({});
    mount(store);
    const release = store.start();
    try {
      seedState({
        provider: "mock",
        model: "mini",
        providers: [{ id: "mock", configured: true }],
        models: [{ provider: "mock", model: "mini", displayName: "Mini" }],
      });
      const field = await screen.findByLabelText("Message");
      // The provider pair has to have landed: a submit without one is
      // deliberately ignored (the lane's foot says why).
      await screen.findAllByText("Mini");
      const sendPrompt = vi
        .spyOn(store.agent, "sendPrompt")
        .mockImplementation(() => {});
      const runTurn = vi.spyOn(store, "runTurn").mockResolvedValue(undefined);
      const steer = vi.spyOn(store.agent, "steer").mockImplementation(() => {});
      fireEvent.change(field, { target: { value: "hello" } });
      fireEvent.submit(field.closest("form") as HTMLFormElement);
      expect(sendPrompt).toHaveBeenCalledWith("hello", undefined);
      expect(runTurn).toHaveBeenCalledTimes(1);
      expect(field).toHaveProperty("value", "");

      // A live run turns the same field into the steering lane: the trailing
      // slot holds Stop and nothing else, and `↵` steers instead of prompting.
      harness.emit({ type: "RUN_STARTED", threadId: "s", runId: "r" } as never);
      await waitFor(() =>
        expect(
          screen.getByRole("button", { name: "Stop" }),
        ).toBeInTheDocument(),
      );
      expect(screen.queryByRole("button", { name: "Send" })).toBeNull();
      expect(screen.getByLabelText("Message")).toHaveProperty(
        "placeholder",
        "Steer the agent, or wait",
      );
      fireEvent.change(screen.getByLabelText("Message"), {
        target: { value: "also check the tests" },
      });
      fireEvent.submit(
        screen.getByLabelText("Message").closest("form") as HTMLFormElement,
      );
      expect(steer).toHaveBeenCalledWith("also check the tests");
      fireEvent.click(screen.getByRole("button", { name: "Stop" }));
      await waitFor(() =>
        expect(sendRequest).toHaveBeenCalledWith(
          expect.stringContaining("agent.cancel"),
        ),
      );
    } finally {
      release();
    }
  });

  it("keeps typing open with no provider and states the reason in the foot", async () => {
    // Plan 124 review: a missing provider gates sending, never typing — the
    // reason lives in the lane's own foot, in both views.
    const { store } = mount();
    const release = store.start();
    try {
      seedState({ provider: "", model: "", providers: [] });
      const field = screen.getByLabelText("Message");
      expect(field).toBeEnabled();
      expect(field).toHaveProperty("placeholder", "Ask, or type / or @");
      expect(
        await screen.findByText(
          "no provider configured · Settings → Providers",
        ),
      ).toBeInTheDocument();
      // The model trigger is the disabled gate, and it says why.
      const model = await screen.findByRole("button", {
        name: /Configure a provider/,
      });
      expect(model).toBeDisabled();
      // Nothing sends, and the draft survives (the foot says why).
      const runTurn = vi.spyOn(store, "runTurn").mockResolvedValue(undefined);
      fireEvent.change(field, { target: { value: "hello" } });
      fireEvent.submit(field.closest("form") as HTMLFormElement);
      expect(runTurn).not.toHaveBeenCalled();
      expect(field).toHaveProperty("value", "hello");
    } finally {
      release();
    }
  });

  it("offers the agent picker and keeps the field inert with no agent", async () => {
    const { store } = mount(createAgentSession({}), {
      agentType: null,
      onPickAgent: () => undefined,
    });
    const release = store.start();
    try {
      seedState({ provider: "mock", model: "mini" });
      const field = screen.getByLabelText("Message");
      expect(field).toBeDisabled();
      expect(field).toHaveProperty(
        "placeholder",
        "Attach an agent to this tab to send a prompt",
      );
      expect(
        screen.getByText("no agent on this tab · the lane keeps its place"),
      ).toBeInTheDocument();
      // The box's own controls row offers the agent picker only.
      const trigger = document.querySelector<HTMLButtonElement>(
        "[data-agent-pick] button",
      );
      expect(trigger).toBeTruthy();
      expect(trigger).toHaveTextContent("Attach an agent");
      // The model/effort/meter trio describes the agent's run: with no agent
      // there is nothing for them to describe (approved agent-less state).
      expect(screen.queryByRole("button", { name: /Model/ })).toBeNull();
      expect(screen.queryByRole("button", { name: "Effort" })).toBeNull();
      expect(screen.queryByTitle(/Context usage/)).toBeNull();
    } finally {
      release();
    }
  });

  it("reports the tab's run state up to the shell for the window mark", async () => {
    const onBusyChange = vi.fn();
    const store = createAgentSession({});
    render(
      <AgentLane
        store={store}
        uiVersion={4}
        workspaceRoot="/tmp/ws"
        onBusyChange={onBusyChange}
        palette={paletteStub()}
      />,
    );
    const release = store.start();
    try {
      await waitFor(() => expect(onBusyChange).toHaveBeenCalledWith(false));
      harness.emit({ type: "RUN_STARTED", threadId: "s", runId: "r" } as never);
      await waitFor(() => expect(onBusyChange).toHaveBeenCalledWith(true));
    } finally {
      release();
    }
  });

  it("renders the model dropdown with the active selection from inventory state", async () => {
    const { store } = mount();
    const release = store.start();
    try {
      seedState({
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
      });
      // Dropdown trigger shows the active model's display label; the grouped
      // list itself is the ClayDropdown catalog behavior (verified in the
      // grouping unit test). React Aria mirrors the selection into a hidden
      // native select, so the label may appear twice.
      expect((await screen.findAllByText("Mini")).length).toBeGreaterThan(0);
      // An unconfigured provider's models are never offered.
      fireEvent.click(screen.getByRole("button", { name: /Mini/ }));
      await screen.findByRole("option", { name: "Mini" });
      expect(screen.getByRole("option", { name: "big" })).toBeInTheDocument();
      expect(screen.queryByRole("option", { name: "hidden" })).toBeNull();
    } finally {
      release();
    }
  });

  it("effort dropdown is active from session start (plan 117): levels resolve from the model inventory before any prompt", async () => {
    const { store } = mount();
    const release = store.start();
    try {
      // Mount-time inventory STATE: models inventory, NO session and no
      // `effort` key — exactly what a fresh webview holds.
      seedState({
        provider: "mock",
        model: "mini",
        providers: [{ id: "mock", configured: true }],
        models: [
          {
            provider: "mock",
            model: "mini",
            displayName: "Mock demo",
            thinkingLevels: ["low", "medium", "high"],
          },
        ],
      });
      expect(await screen.findByText("Effort")).toBeInTheDocument();
    } finally {
      release();
    }
  });
});

describe("AgentLane agent picker", () => {
  const launcherEntries = {
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
  };

  function mountWithSession(
    overrides: { onPickAgent?: (id: string | null) => void } = {},
  ) {
    const session = createDocumentSession({ send: async () => undefined });
    const store = createAgentSession({});
    render(
      <AgentLane
        store={store}
        uiVersion={4}
        workspaceRoot="/tmp/ws"
        session={session}
        agentType="coding-agent"
        onPickAgent={overrides.onPickAgent ?? (() => undefined)}
        palette={paletteStub()}
      />,
    );
    act(() => {
      session.handleEnvelope({
        kind: "event",
        data: {
          kind: "launcherEntries",
          // The server's ServerMessage shape: the listing rides under
          // `entries` (the same envelope the launcher parses).
          data: { clientId: 1, entries: launcherEntries },
        },
      } as never);
    });
    return { store };
  }

  it("lists only the configured agents and marks the current one", async () => {
    mountWithSession();
    // The trigger consumes the agentPicker family (not the plain dropdown).
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

    act(() => {
      fireEvent.click(trigger);
    });
    const currentRow = await screen.findByRole("option", {
      name: /Coding Agent/,
    });
    expect(currentRow).toHaveAttribute("aria-selected", "true");
    const reviewerRow = screen.getByRole("option", { name: /Reviewer/ });
    expect(reviewerRow).toHaveAttribute("aria-selected", "false");
    // The menu names where more agent types come from and never fabricates one
    // the data root does not hold.
    expect(screen.getByText(/from ~\/\.clay\/agents\//)).toBeInTheDocument();
    expect(screen.queryByRole("option", { name: /Ghost/ })).toBeNull();
  });

  it("hands the picked type to the tab", async () => {
    const picked: Array<string | null> = [];
    mountWithSession({ onPickAgent: (agent) => picked.push(agent) });
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
    const store = createAgentSession({});
    render(
      <AgentLane
        store={store}
        uiVersion={4}
        workspaceRoot="/tmp/ws"
        agentType="coding-agent"
        palette={paletteStub()}
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

describe("AgentLane tool approval", () => {
  function mountSuspended() {
    const store = createAgentSession({});
    const release = store.start();
    render(
      <AgentLane
        store={store}
        uiVersion={4}
        workspaceRoot="/tmp/ws"
        agentType="coding-agent"
        palette={paletteStub()}
      />,
    );
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
    return { store, release };
  }

  it("keeps text buttons: Allow and Deny are never icon-only (plan 112 T8)", async () => {
    const { release } = mountSuspended();
    try {
      const allow = await screen.findByRole("button", { name: "Allow" });
      expect(allow.querySelector("svg")).toBeNull();
      expect(allow.textContent).toContain("Allow");
      const deny = screen.getByRole("button", { name: "Deny" });
      expect(deny.querySelector("svg")).toBeNull();
    } finally {
      release();
    }
  });

  it("Allow/Deny each send the runResume decision for that request (plan 119 SC-4)", async () => {
    // The decision payload is the security-relevant half of the strip; it rides
    // the tab's own command lane (the lane owns the send authority, plan 124).
    const { release } = mountSuspended();
    try {
      (await screen.findByRole("button", { name: "Allow" })).click();
      await waitFor(() =>
        expect(sendRequest).toHaveBeenCalledWith(
          expect.stringContaining('"runId":"r1"'),
        ),
      );
      const payload = String(
        vi.mocked(sendRequest).mock.calls.at(-1)?.[0] ?? "",
      );
      expect(payload).toContain("runResume");
      expect(payload).toContain("approvalId");
      expect(payload).toContain("req1");
      expect(payload).toContain("allow_once");
      // The strip clears optimistically; the resumed run re-announces.
      await waitFor(() =>
        expect(screen.queryByRole("button", { name: "Deny" })).toBeNull(),
      );
    } finally {
      release();
    }
  });

  it("takes focus while suspended and gives it back (plan 119 review F3)", async () => {
    // `alertdialog` promises the user is taken to the decision; announcing it
    // while focus stayed in the composer was the review's F3 finding.
    const store = createAgentSession({});
    const release = store.start();
    try {
      render(
        <AgentLane
          store={store}
          uiVersion={4}
          workspaceRoot="/tmp/ws"
          agentType="coding-agent"
          palette={paletteStub()}
        />,
      );
      const composer = screen.getByLabelText("Message");
      act(() => composer.focus());
      expect(document.activeElement).toBe(composer);
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
      await waitFor(() => expect(document.activeElement).toBe(allow));
      act(() => allow.click());
      await waitFor(() =>
        expect(screen.queryByRole("button", { name: "Deny" })).toBeNull(),
      );
      await waitFor(() => expect(document.activeElement).toBe(composer));
    } finally {
      release();
    }
  });
});
