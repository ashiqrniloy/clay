// @vitest-environment jsdom
// Agent settings tab-body tests (plan 117): delivered-file listing with
// provenance badges and open routing into the document pipeline. The tab
// frame (and its close) belongs to the tab strip, not this component.

import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";

import {
  AgentSettingsPanel,
  type AgentSettingsFileInfo,
} from "./AgentSettingsPanel";

afterEach(() => {
  cleanup();
});

const files: AgentSettingsFileInfo[] = [
  {
    name: "SYSTEM.md",
    displayPath: "/root/SYSTEM.md",
    sizeBytes: 12,
    modifiedMs: 1,
    edited: false,
  },
  {
    name: "skills/graft/SKILL.md",
    displayPath: "/root/skills/graft/SKILL.md",
    sizeBytes: 1024,
    modifiedMs: 2,
    edited: true,
  },
];

describe("AgentSettingsPanel", () => {
  it("lists delivered files with provenance badges", () => {
    render(
      <AgentSettingsPanel
        files={files}
        loading={false}
        onOpen={vi.fn()}
      />,
    );
    const list = screen.getByLabelText("Agent files");
    expect(list.textContent).toContain("SYSTEM.md");
    expect(list.textContent).toContain("built-in");
    expect(list.textContent).toContain("skills/graft/SKILL.md");
    expect(list.textContent).toContain("edited");
  });

  it("selecting a file opens it into the document pipeline", () => {
    const onOpen = vi.fn();
    render(
      <AgentSettingsPanel
        files={files}
        loading={false}
        onOpen={onOpen}
      />,
    );
    fireEvent.click(screen.getByText("skills/graft/SKILL.md"));
    expect(onOpen).toHaveBeenCalledWith("skills/graft/SKILL.md");
  });

  it("communicates the loading and empty states", () => {
    render(
      <AgentSettingsPanel
        files={null}
        loading={true}
        onOpen={vi.fn()}
      />,
    );
    expect(screen.getByText("Loading…")).toBeInTheDocument();

    cleanup();
    render(
      <AgentSettingsPanel
        files={[]}
        loading={false}
        onOpen={vi.fn()}
      />,
    );
    expect(screen.getByText("No agent files delivered yet.")).toBeInTheDocument();
  });
});
