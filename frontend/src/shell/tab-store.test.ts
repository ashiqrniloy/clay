import { describe, expect, it } from "vitest";

import {
  applyRegistry,
  emptyTabs,
  markDirty,
  patchTab,
  removeTab,
  tabLabel,
  tabTitle,
  tabUncommitted,
  upsertTab,
  type ShellTabState,
} from "./tab-store";
import { tabsFromWindow, windowFromTabs, emptyLayout } from "./persist";

/** A committed tab record with the two-view fields filled in. */
function tab(overrides: Partial<ShellTabState>): ShellTabState {
  return {
    tabId: null,
    clientId: 1,
    workspaceRoot: "/tmp/notes",
    agent: null,
    view: "workspace",
    agentBusy: false,
    label: "notes",
    dirty: false,
    disconnected: false,
    ...overrides,
  };
}

describe("tab store", () => {
  it("labels a tab from the workspace basename", () => {
    expect(tabLabel("/tmp/notes")).toBe("notes");
    expect(tabLabel("C:\\\\Users\\\\clay\\\\proj\\\\")).toBe("proj");
  });

  it("labels the agent half and the uncommitted tab", () => {
    // Plan 118 task 33: an agent-only tab is named by its agent; a tab that
    // has picked nothing shows the approved faint "New tab".
    expect(tabLabel("", { type: "coding-agent", configRoot: "/tmp/a" })).toBe(
      "coding-agent",
    );
    expect(tabLabel("")).toBe("New tab");
    expect(
      tabLabel("/tmp/notes", { type: "coding-agent", configRoot: "" }),
    ).toBe("notes");
  });

  it("describes the tab in its tooltip: full path plus the agent", () => {
    expect(tabTitle("/tmp/notes")).toBe("/tmp/notes — one tab, two views");
    expect(
      tabTitle("/tmp/notes", { type: "coding-agent", configRoot: "/tmp/a" }),
    ).toBe("/tmp/notes — one tab, two views, agent: coding-agent");
    expect(tabTitle("")).toContain("No workspace picked");
  });

  it("patches one tab's identity and re-derives its label", () => {
    let state = upsertTab(emptyTabs(), tab({ clientId: 7 }));
    state = patchTab(state, 7, { view: "agent", agentBusy: true });
    expect(state.tabs[0]?.view).toBe("agent");
    expect(state.tabs[0]?.agentBusy).toBe(true);
    // Deriving on patch keeps one rule for the strip: agent-only renames.
    state = patchTab(state, 7, { workspaceRoot: "" });
    expect(state.tabs[0]?.label).toBe("New tab");
    state = patchTab(state, 7, {
      agent: { type: "coding-agent", configRoot: "/tmp/a" },
    });
    expect(state.tabs[0]?.label).toBe("coding-agent");
    expect(tabUncommitted(state.tabs[0]!)).toBe(false);
  });

  it("applies a newer registry and ignores a stale revision", () => {
    const seeded = upsertTab(emptyTabs(), tab({ dirty: true }));
    const next = applyRegistry(seeded, {
      revision: 3,
      active: 10,
      tabs: [
        { tabId: 10, clientId: 1, workspaceRoot: "/a" },
        { tabId: 11, clientId: 2, workspaceRoot: "/b" },
      ],
    });
    expect(next.tabs.map((tab) => tab.tabId)).toEqual([10, 11]);
    expect(next.activeClientId).toBe(1);
    expect(next.tabs[0]?.dirty).toBe(true);
    expect(
      applyRegistry(next, { revision: 1, active: 11, tabs: [] }).revision,
    ).toBe(3);
  });

  it("keeps the client's tab identity when the registry broadcasts", () => {
    // The server always has a root (configured root or cwd fallback), so its
    // snapshot would otherwise commit a launcher tab that has picked nothing.
    const seeded = upsertTab(
      emptyTabs(),
      tab({
        workspaceRoot: "",
        agent: { type: "coding-agent", configRoot: "" },
      }),
    );
    seeded.tabs[0]!.view = "agent";
    const next = applyRegistry(seeded, {
      revision: 4,
      active: 10,
      tabs: [{ tabId: 10, clientId: 1, workspaceRoot: "/home/dev/fallback" }],
    });
    expect(next.tabs[0]?.workspaceRoot).toBe("");
    expect(next.tabs[0]?.view).toBe("agent");
    expect(next.tabs[0]?.label).toBe("coding-agent");
  });

  it("does not let one tab's dirty flag leak onto another", () => {
    let state = upsertTab(
      emptyTabs(),
      tab({ clientId: 1, workspaceRoot: "/a" }),
    );
    state = upsertTab(
      state,
      tab({ clientId: 2, workspaceRoot: "/b", label: "b" }),
    );
    state = markDirty(state, 1, true);
    expect(state.tabs.find((tab) => tab.clientId === 2)?.dirty).toBe(false);
    state = removeTab(state, 1);
    expect(state.tabs).toHaveLength(1);
    expect(state.activeClientId).toBe(2);
  });
});

describe("layout.json v2 tab record (plan 118 task 33)", () => {
  it("round-trips the picked folder, the agent and the view", () => {
    const layout = {
      ...emptyLayout("/tmp/notes"),
      agent: { type: "coding-agent", configRoot: "/tmp/agents/coding-agent" },
      view: "agent" as const,
    };
    const persisted = windowFromTabs([layout], 0);
    expect(persisted.tabs[0]?.view).toBe("agent");
    expect(persisted.tabs[0]?.agent?.type).toBe("coding-agent");

    const parsed = tabsFromWindow(persisted);
    expect(parsed?.tabs[0]?.workspaceRoot).toBe("/tmp/notes");
    expect(parsed?.tabs[0]?.agent).toEqual({
      type: "coding-agent",
      configRoot: "/tmp/agents/coding-agent",
    });
    expect(parsed?.tabs[0]?.view).toBe("agent");
  });

  it("round-trips rail and inspector visibility per tab (plan 118 E2)", () => {
    const first = {
      ...emptyLayout("/tmp/one"),
      railVisible: false,
      inspectorVisible: true,
    };
    const second = {
      ...emptyLayout("/tmp/two"),
      view: "agent" as const,
      railVisible: true,
      inspectorVisible: false,
    };
    const persisted = windowFromTabs([first, second], 1);
    expect(persisted.tabs[0]?.railVisible).toBe(false);
    expect(persisted.tabs[1]?.inspectorVisible).toBe(false);

    const parsed = tabsFromWindow(persisted);
    expect(parsed?.tabs[0]?.railVisible).toBe(false);
    expect(parsed?.tabs[0]?.inspectorVisible).toBe(true);
    expect(parsed?.tabs[1]?.railVisible).toBe(true);
    expect(parsed?.tabs[1]?.inspectorVisible).toBe(false);
  });

  it("reads a document without the visibility fields as visible (plan 118 E2)", () => {
    const parsed = tabsFromWindow({
      version: 2,
      activeTab: 0,
      tabs: [{ workspaceRoot: "/tmp/x", splitTree: null, panes: {} }],
    });
    expect(parsed?.tabs[0]?.railVisible).toBe(true);
    expect(parsed?.tabs[0]?.inspectorVisible).toBe(true);
  });

  it("keeps an agent-only tab and drops a tab with neither half", () => {
    const raw = {
      version: 2,
      activeTab: 0,
      tabs: [
        {
          workspaceRoot: "",
          agent: { type: "coding-agent", configRoot: "" },
          view: "workspace",
          splitTree: null,
          panes: {},
        },
        { workspaceRoot: "", agent: null, view: "agent", panes: {} },
      ],
    };
    const parsed = tabsFromWindow(raw);
    expect(parsed?.tabs).toHaveLength(1);
    expect(parsed?.tabs[0]?.workspaceRoot).toBe("");
    expect(parsed?.tabs[0]?.agent?.type).toBe("coding-agent");
  });

  it("defaults a document without a view to the workspace view", () => {
    const parsed = tabsFromWindow({
      version: 2,
      activeTab: 0,
      tabs: [
        {
          workspaceRoot: "/tmp/x",
          splitTree: null,
          panes: {},
        },
      ],
    });
    expect(parsed?.tabs[0]?.view).toBe("workspace");
    expect(parsed?.tabs[0]?.agent).toBeNull();
  });
});
