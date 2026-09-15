// Transcript list (plan 119 SC-4 / PF-2): the rendered conversation, its
// per-turn producer labels, and the memo that keeps the previous-agent scan at
// one pass per box list (the O(n²) liability the review flagged).
// @vitest-environment jsdom
import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

vi.mock("./transcript-model", async (importOriginal) => {
  const actual = await importOriginal<typeof import("./transcript-model")>();
  return { ...actual, previousAgents: vi.fn(actual.previousAgents) };
});

import { TranscriptList } from "./TranscriptList";
import { previousAgents, type TranscriptBox } from "./transcript-model";

afterEach(() => {
  cleanup();
  vi.mocked(previousAgents).mockClear();
});

const box = (id: string, agent?: string): TranscriptBox => ({
  id,
  kind: "assistant",
  label: "agent",
  content: `content ${id}`,
  ...(agent ? { agent } : {}),
});

describe("TranscriptList", () => {
  it("shows the empty state with the keyboard path when nothing ran", () => {
    render(<TranscriptList boxes={[]} selectedId="" onSelect={() => {}} />);
    expect(screen.getByText("No conversation yet.")).toBeTruthy();
    expect(screen.queryAllByRole("button")).toHaveLength(0);
  });

  it("labels one producer's turns with the role, not the agent", () => {
    render(
      <TranscriptList
        boxes={[box("a", "planner"), box("b", "planner")]}
        selectedId=""
        onSelect={() => {}}
      />,
    );
    expect(screen.queryAllByText("Planner")).toHaveLength(0);
    expect(document.querySelectorAll("[data-turn-switch]")).toHaveLength(0);
  });

  it("notes each switch and labels every turn once two producers ran", () => {
    render(
      <TranscriptList
        boxes={[
          box("a", "planner"),
          box("b", "planner"),
          box("c", "reviewer"),
          box("d"),
        ]}
        selectedId=""
        onSelect={() => {}}
      />,
    );
    // The switch note belongs to the first turn of the new agent only.
    const notes = [...document.querySelectorAll("[data-turn-switch]")];
    expect(notes).toHaveLength(1);
    expect(notes[0]?.textContent).toBe("Switched to Reviewer");
    // The stamped turn carries its producer in the head; the unstamped turn
    // falls back to the generic role label (never an invented producer).
    expect(screen.getAllByText("Reviewer")).toHaveLength(1);
    expect(screen.getAllByText("agent")).toHaveLength(1);
  });

  it("selects a turn through the callback (deselect on re-click)", () => {
    const onSelect = vi.fn();
    render(
      <TranscriptList
        boxes={[box("a"), box("b")]}
        selectedId="a"
        onSelect={onSelect}
      />,
    );
    expect(document.querySelectorAll('[aria-pressed="true"]')).toHaveLength(1);
    screen.getByRole("button", { name: /content a/ }).click();
    expect(onSelect).toHaveBeenCalledWith("");
  });

  it("PF-2: derives the previous producers once per box list, not per turn", () => {
    const boxes = Array.from({ length: 200 }, (_, index) =>
      box(`t${index}`, index % 40 === 0 ? `agent${index}` : undefined),
    );
    const { rerender } = render(
      <TranscriptList boxes={boxes} selectedId="" onSelect={() => {}} />,
    );
    expect(vi.mocked(previousAgents).mock.calls.length).toBe(1);
    expect(document.querySelectorAll("[data-turn-switch]")).toHaveLength(
      boxes.filter((entry) => entry.agent).length - 1,
    );
    // Same list identity, new selection: the memo must hold.
    rerender(
      <TranscriptList boxes={boxes} selectedId="t5" onSelect={() => {}} />,
    );
    expect(vi.mocked(previousAgents).mock.calls.length).toBe(1);
  });
});
