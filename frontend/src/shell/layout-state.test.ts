import { beforeEach, describe, expect, it } from "vitest";

import { agentInspector, agentLane, workspaceRail } from "./layout-state";

/**
 * The rail, the agent inspector and the agent lane are three surfaces with
 * three stores (plan 118 E2 / plan 124). Sharing one map between them made a
 * lane toggle close the workspace rail and the inspector at once — plan 125
 * defect D9, found by the manual-test run and pinned here.
 */
describe("layout-state visibility stores", () => {
  beforeEach(() => {
    workspaceRail.resetForTests();
    agentInspector.resetForTests();
    agentLane.resetForTests();
  });

  it("keeps each surface's visibility to itself", () => {
    agentLane.toggle();
    expect(agentLane.isVisible()).toBe(false);
    expect(workspaceRail.isVisible()).toBe(true);
    expect(agentInspector.isVisible()).toBe(true);

    workspaceRail.toggle();
    expect(workspaceRail.isVisible()).toBe(false);
    expect(agentLane.isVisible()).toBe(false);
    expect(agentInspector.isVisible()).toBe(true);

    agentLane.toggle();
    expect(agentLane.isVisible()).toBe(true);
    expect(workspaceRail.isVisible()).toBe(false);
  });

  it("notifies only the surface that changed", () => {
    let rail = 0;
    let lane = 0;
    const offRail = workspaceRail.subscribe(() => (rail += 1));
    const offLane = agentLane.subscribe(() => (lane += 1));

    agentLane.toggle();
    expect([rail, lane]).toEqual([0, 1]);

    offRail();
    offLane();
  });

  it("keeps per-tab values per surface", () => {
    workspaceRail.setActiveTab(1);
    agentLane.setActiveTab(1);
    agentLane.toggle();
    expect(agentLane.isVisible()).toBe(false);
    expect(workspaceRail.isVisible()).toBe(true);

    // A tab that recorded nothing starts visible on both surfaces.
    workspaceRail.setActiveTab(2);
    agentLane.setActiveTab(2);
    expect(workspaceRail.isVisible()).toBe(true);
    expect(agentLane.isVisible()).toBe(true);

    workspaceRail.setActiveTab(1);
    agentLane.setActiveTab(1);
    expect(workspaceRail.isVisible()).toBe(true);
    expect(agentLane.isVisible()).toBe(false);
  });
});
