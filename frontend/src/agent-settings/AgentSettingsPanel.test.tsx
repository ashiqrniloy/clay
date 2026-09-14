// @vitest-environment jsdom
// Agent settings tab-body tests (plan 117, recomposed by plan 118): delivered
// files with provenance badges, size as data, open routing into the document
// pipeline, and the designed empty state. The tab frame (and its close)
// belongs to the tab strip, not this component.

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
  it("lists delivered files with provenance badges and sizes", () => {
    render(
      <AgentSettingsPanel files={files} loading={false} onOpen={vi.fn()} />,
    );
    const list = screen.getByLabelText("Agent files");
    expect(list.textContent).toContain("SYSTEM.md");
    expect(list.textContent).toContain("built-in");
    expect(list.textContent).toContain("skills/graft/SKILL.md");
    expect(list.textContent).toContain("edited");
    // Sizes are data: formatted, and read in the mono role.
    expect(list.textContent).toContain("12 B");
    expect(list.textContent).toContain("1.0 kB");
  });

  it("renders each file row as a list row (the shared row language)", () => {
    const { container } = render(
      <AgentSettingsPanel files={files} loading={false} onOpen={vi.fn()} />,
    );
    const rows = container.querySelectorAll(
      '[data-clay-component="list"][data-clay-slot="row"]',
    );
    expect(rows).toHaveLength(files.length);
  });

  it("selecting a file opens it into the document pipeline", () => {
    const onOpen = vi.fn();
    render(
      <AgentSettingsPanel files={files} loading={false} onOpen={onOpen} />,
    );
    fireEvent.click(screen.getByText("skills/graft/SKILL.md"));
    expect(onOpen).toHaveBeenCalledWith("skills/graft/SKILL.md");
  });

  it("communicates the loading state", () => {
    render(<AgentSettingsPanel files={null} loading={true} onOpen={vi.fn()} />);
    expect(screen.getByText("Loading…")).toBeInTheDocument();
  });

  it("renders the empty state as a designed state, not an empty box", () => {
    const { container } = render(
      <AgentSettingsPanel files={[]} loading={false} onOpen={vi.fn()} />,
    );
    const empty = container.querySelector("[data-agent-settings-empty]");
    expect(empty).not.toBeNull();
    expect(empty?.textContent).toContain("No agent files yet.");
    // The empty state explains where the files come from, rather than just
    // stating that there are none.
    expect(empty?.querySelectorAll("p")[1]?.textContent).toContain("SYSTEM.md");
  });

  it("states where the listing comes from, in the approved caption", () => {
    const { container } = render(
      <AgentSettingsPanel files={files} loading={false} onOpen={vi.fn()} />,
    );
    const caption = container.querySelector("[data-agent-settings-caption]");
    expect(caption?.textContent).toContain(".agents/skills/*/SKILL.md");
    expect(caption?.textContent).toContain("next daemon start");
  });
});
