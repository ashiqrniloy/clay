import { describe, expect, it } from "vitest";

import {
  applyRegistry,
  emptyTabs,
  markDirty,
  removeTab,
  tabLabel,
  upsertTab,
} from "./tab-store";

describe("tab store", () => {
  it("labels a tab from the workspace basename", () => {
    expect(tabLabel("/tmp/notes")).toBe("notes");
    expect(tabLabel("C:\\\\Users\\\\clay\\\\proj\\\\")).toBe("proj");
  });

  it("applies a newer registry and ignores a stale revision", () => {
    const seeded = upsertTab(emptyTabs(), {
      tabId: null,
      clientId: 1,
      workspaceRoot: "/a",
      label: "a",
      dirty: true,
      disconnected: false,
    });
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

  it("does not let one tab's dirty flag leak onto another", () => {
    let state = upsertTab(emptyTabs(), {
      tabId: 1,
      clientId: 1,
      workspaceRoot: "/a",
      label: "a",
      dirty: false,
      disconnected: false,
    });
    state = upsertTab(state, {
      tabId: 2,
      clientId: 2,
      workspaceRoot: "/b",
      label: "b",
      dirty: false,
      disconnected: false,
    });
    state = markDirty(state, 1, true);
    expect(state.tabs.find((tab) => tab.clientId === 2)?.dirty).toBe(false);
    state = removeTab(state, 1);
    expect(state.tabs).toHaveLength(1);
    expect(state.activeClientId).toBe(2);
  });
});
