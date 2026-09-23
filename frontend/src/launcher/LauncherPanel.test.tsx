// @vitest-environment jsdom
// Launcher panel (plan 118 Part D): the start surface's interaction contract —
// rows come from the server listing, the primary action names what it will
// open and stays inert until something is picked, the keyboard path works, and
// both halves launch in order (folder first, then the agent in the new tab).
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { LauncherPanel } from "./LauncherPanel";
import type { DocumentSession } from "../editor/sync/session";

type FeatureEnvelope = { kind: string; data: unknown };

/** One bridge envelope carrying a `launcherEntries` feature event. */
function featureEvent(entries: unknown): FeatureEnvelope {
  return {
    kind: "event",
    data: { kind: "launcherEntries", data: { clientId: 1, entries } },
  };
}

function fakeSession(entries: unknown) {
  const listeners = new Set<(envelope: FeatureEnvelope) => void>();
  const calls = { list: 0, removed: [] as number[] };
  let current = entries;
  const session = {
    featureSnapshot: (): FeatureEnvelope[] => [],
    subscribeFeatures: (listener: (envelope: FeatureEnvelope) => void) => {
      listeners.add(listener);
      return () => listeners.delete(listener);
    },
    listLauncherEntries() {
      calls.list += 1;
      // Wire shape: the feature listener sees the outer bridge envelope whose
      // `data` is the server event (`{clientId, entries}` under `entries`).
      for (const listener of [...listeners]) listener(featureEvent(current));
    },
    removeLauncherRecent(index: number) {
      calls.removed.push(index);
    },
  };
  return {
    session: session as unknown as DocumentSession,
    calls,
    /** Push a server answer the way the bridge delivers one. */
    answer(next: unknown) {
      current = next;
      for (const listener of [...listeners]) listener(featureEvent(next));
    },
  };
}

const ENTRIES = {
  workspaces: [
    { name: "clay", root: "/home/arn/Projects/clay" },
    { name: "prism", root: "/home/arn/Projects/prism" },
  ],
  agents: [
    {
      name: "coding-agent",
      label: "Coding Agent",
      configRoot: "~/.clay/agents/coding-agent",
      skillCount: 3,
    },
  ],
  pruned: 0,
};

function mount(entries: unknown = ENTRIES) {
  const fake = fakeSession(entries);
  const onOpenWorkspace = vi.fn(async () => undefined);
  const onOpenFolder = vi.fn();
  const onPickAgent = vi.fn();
  render(
    <LauncherPanel
      session={fake.session}
      onOpenWorkspace={onOpenWorkspace}
      onOpenFolder={onOpenFolder}
      onPickAgent={onPickAgent}
    />,
  );
  return { ...fake, onOpenWorkspace, onOpenFolder, onPickAgent };
}

afterEach(cleanup);

describe("LauncherPanel", () => {
  it("lists the server's rows and asks for them once", async () => {
    const fake = mount();
    await waitFor(() => expect(fake.calls.list).toBe(1));
    expect(screen.getByText("clay")).toBeInTheDocument();
    expect(screen.getByText("/home/arn/Projects/clay")).toBeInTheDocument();
    expect(screen.getByText("Coding Agent")).toBeInTheDocument();
    expect(screen.getByText("3 skills")).toBeInTheDocument();
  });

  it("specifies the first-run state instead of fabricating rows", async () => {
    mount({ workspaces: [], agents: [], pruned: 0 });
    expect(
      await screen.findByText(/No workspace has been opened yet/),
    ).toBeInTheDocument();
    expect(
      screen.getByText(/Agents are folders under ~\/\.clay\/agents\//),
    ).toBeInTheDocument();
  });

  it("names what the primary action opens and stays inert until picked", async () => {
    const user = userEvent.setup();
    mount();
    const open = await screen.findByRole("button", { name: "Open" });
    expect(open).toBeDisabled();

    await user.click(screen.getByText("clay"));
    await user.keyboard("{Enter}");
    await waitFor(() =>
      expect(screen.getByRole("button", { name: "Open clay" })).toBeEnabled(),
    );

    await user.click(screen.getByText("Coding Agent"));
    await user.keyboard("{Enter}");
    await waitFor(() =>
      expect(
        screen.getByRole("button", { name: "Open clay + Coding Agent" }),
      ).toBeEnabled(),
    );

    await user.keyboard("{Escape}");
    await waitFor(() =>
      expect(screen.getByRole("button", { name: "Open" })).toBeDisabled(),
    );
  });

  it("moves with the arrows, picks with Enter, opens both with the modifier", async () => {
    const user = userEvent.setup();
    const fake = mount();
    await waitFor(() => expect(fake.calls.list).toBe(1));
    await user.click(screen.getByText("clay"));
    await user.keyboard("{ArrowDown}");
    await user.keyboard("{Enter}");
    await waitFor(() =>
      expect(screen.getByRole("button", { name: "Open prism" })).toBeEnabled(),
    );

    await user.keyboard("{Meta>}{Enter}{/Meta}");
    await waitFor(() =>
      expect(fake.onOpenWorkspace).toHaveBeenCalledWith(
        "/home/arn/Projects/prism",
      ),
    );
    expect(fake.onPickAgent).not.toHaveBeenCalled();
  });

  it("opens the folder first, then the agent in the new tab", async () => {
    const user = userEvent.setup();
    const fake = mount();
    await waitFor(() => expect(fake.calls.list).toBe(1));
    await user.click(screen.getByText("clay"));
    await user.keyboard("{Enter}");
    await user.click(screen.getByText("~/.clay/agents/coding-agent"));
    await user.keyboard("{Enter}");
    await user.click(
      await screen.findByRole("button", { name: "Open clay + Coding Agent" }),
    );
    await waitFor(() =>
      expect(fake.onOpenWorkspace).toHaveBeenCalledWith(
        "/home/arn/Projects/clay",
      ),
    );
    // The agent launch follows the folder open (the new tab is active by then).
    await waitFor(() => expect(fake.onPickAgent).toHaveBeenCalledTimes(1));
    // The identity handed to the tab is the agent's registry key + config root
    // (inert data), never a package specifier (plan 118 task 33 security).
    expect(fake.onPickAgent).toHaveBeenCalledWith({
      type: "coding-agent",
      configRoot: "~/.clay/agents/coding-agent",
    });
    expect(fake.onOpenWorkspace.mock.invocationCallOrder[0] ?? 0).toBeLessThan(
      fake.onPickAgent.mock.invocationCallOrder[0] ?? 0,
    );
  });

  it("reports a no-match state for a filter with no rows", async () => {
    const user = userEvent.setup();
    mount();
    await user.type(screen.getByLabelText("Filter workspaces"), "zzz");
    expect(screen.getByText(/No workspace matches/)).toBeInTheDocument();
    expect(screen.queryByText("clay")).toBeNull();
  });

  it("drops one recent by server-side index and renders the fresh listing", async () => {
    const user = userEvent.setup();
    const fake = mount();
    await waitFor(() => expect(fake.calls.list).toBe(1));
    await user.click(screen.getByText("clay"));
    await user.keyboard("{Backspace}");
    await waitFor(() => expect(fake.calls.removed).toEqual([0]));

    fake.answer({
      workspaces: [{ name: "prism", root: "/home/arn/Projects/prism" }],
      agents: [],
      pruned: 0,
    });
    await waitFor(() => expect(screen.queryByText("clay")).toBeNull());
    expect(screen.getByText("prism")).toBeInTheDocument();
  });

  it("runs the real folder dialog from the workspace pane foot", async () => {
    const user = userEvent.setup();
    const fake = mount();
    await user.click(screen.getByRole("button", { name: "Open folder…" }));
    expect(fake.onOpenFolder).toHaveBeenCalledTimes(1);
  });
});
