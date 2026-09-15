// Composer (plan 119 SC-4): the input lane's pure state machine (chord match,
// effort cycle, @-token parse) plus the behaviours the panel relies on —
// completion embedding, the consumed-submit contract, and the one-shot file
// fetch.
// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import {
  chordMatches,
  Composer,
  mentionTokenOf,
  nextEffortLevel,
} from "./Composer";

afterEach(cleanup);

function mount(overrides: Partial<Parameters<typeof Composer>[0]> = {}) {
  const props = {
    configured: true,
    streaming: false,
    sessionId: "s1",
    command: vi.fn(),
    slashCommands: [
      { name: "/model", description: "pick a model" },
      { name: "/resume", description: "resume a session" },
    ],
    skills: [{ name: "wiki-init" }],
    files: ["src/editor/position-index.ts"],
    effortLevels: null,
    shownEffort: null,
    onEffortChange: vi.fn(),
    effortChord: null,
    onStop: vi.fn(),
    onClose: vi.fn(),
    onSubmit: vi.fn(() => true),
    ...overrides,
  };
  render(<Composer {...props} />);
  return props;
}

describe("composer model", () => {
  it("parses only a trailing @token, and only after a boundary", () => {
    expect(mentionTokenOf("@")).toEqual({ query: "", start: 0 });
    expect(mentionTokenOf("ask @src/ed")).toEqual({
      query: "src/ed",
      start: 4,
    });
    expect(mentionTokenOf("ask\n@file:a.ts")).toEqual({
      query: "file:a.ts",
      start: 4,
    });
    // A mid-word @ (an email) never opens the dropdown.
    expect(mentionTokenOf("mail me at a@b")).toBeNull();
    // A token that is not at the end is spent, not open.
    expect(mentionTokenOf("@done and more")).toBeNull();
    expect(mentionTokenOf("no mention here")).toBeNull();
  });

  it("cycles declared effort levels, wrapping from the declared top", () => {
    const levels = ["low", "medium", "high"];
    // No pending/active level: the cycle starts at the declared first level.
    expect(nextEffortLevel(levels, null)).toBe("low");
    expect(nextEffortLevel(levels, "low")).toBe("medium");
    expect(nextEffortLevel(levels, "high")).toBe("low");
    // A stale level the model no longer declares restarts from the top.
    expect(nextEffortLevel(levels, "bogus")).toBe("low");
    expect(nextEffortLevel(["only"], "only")).toBe("only");
  });

  it("matches the default Shift+Tab chord strictly", () => {
    expect(
      chordMatches({ key: "Tab", shiftKey: true, ctrlKey: false }, null),
    ).toBe(true);
    expect(chordMatches({ key: "Tab", shiftKey: false }, null)).toBe(false);
    expect(
      chordMatches({ key: "Tab", shiftKey: true, ctrlKey: true }, null),
    ).toBe(false);
    // A rebound chord is the whole match: Ctrl+Tab matches only itself.
    const chord = {
      shift: false,
      ctrl: true,
      alt: false,
      meta: false,
      key: "Tab",
    };
    expect(
      chordMatches({ key: "Tab", shiftKey: false, ctrlKey: true }, chord),
    ).toBe(true);
    expect(chordMatches({ key: "Tab", shiftKey: true }, chord)).toBe(false);
  });
});

describe("Composer", () => {
  it("offers slash completions and submits the highlighted command", () => {
    const props = mount();
    const field = screen.getByLabelText("Message");
    fireEvent.change(field, { target: { value: "/" } });
    expect(
      screen.getByRole("listbox", { name: "Slash commands" }),
    ).toBeTruthy();

    fireEvent.submit(field.closest("form") as HTMLFormElement);
    // The first completion is highlighted, so it wins over the typed "/".
    expect(props.onSubmit).toHaveBeenCalledWith("/model");
  });

  it("keeps the draft when the panel reports the text was not consumed", () => {
    const submit = vi.fn(() => false);
    mount({ onSubmit: submit });
    const field = screen.getByLabelText("Message") as HTMLInputElement;
    fireEvent.change(field, { target: { value: "hello" } });
    fireEvent.submit(field.closest("form") as HTMLFormElement);
    expect(submit).toHaveBeenCalledWith("hello");
    expect(field.value).toBe("hello");
  });

  it("clears the draft when the panel consumes the text", () => {
    const submit = vi.fn(() => true);
    mount({ onSubmit: submit });
    const field = screen.getByLabelText("Message") as HTMLInputElement;
    fireEvent.change(field, { target: { value: "hello" } });
    fireEvent.submit(field.closest("form") as HTMLFormElement);
    expect(field.value).toBe("");
  });

  it("embeds the picked @-mention and closes the dropdown", () => {
    mount();
    const field = screen.getByLabelText("Message") as HTMLInputElement;
    fireEvent.change(field, { target: { value: "read @wiki" } });
    expect(screen.getByRole("listbox", { name: "Mentions" })).toBeTruthy();
    fireEvent.keyDown(field, { key: "Tab", shiftKey: false });
    expect(field.value).toBe("read @skill:wiki-init ");
    expect(screen.queryByRole("listbox", { name: "Mentions" })).toBeNull();
  });

  it("fetches the workspace file listing once per session", () => {
    const command = vi.fn();
    mount({ command });
    const field = screen.getByLabelText("Message");
    fireEvent.change(field, { target: { value: "@" } });
    fireEvent.change(field, { target: { value: "@s" } });
    const fetches = command.mock.calls.filter(
      ([payload]) =>
        typeof payload === "object" &&
        payload !== null &&
        "workspaceFiles" in payload,
    );
    expect(fetches).toHaveLength(1);
    expect(fetches[0]?.[0]).toEqual({ workspaceFiles: { sessionId: "s1" } });
  });

  it("cycles effort on the chord and ignores it without declared levels", () => {
    const onEffortChange = vi.fn();
    const view = mount({
      effortLevels: ["low", "high"],
      shownEffort: "low",
      onEffortChange,
    });
    const field = screen.getByLabelText("Message");
    fireEvent.keyDown(field, { key: "Tab", shiftKey: true });
    expect(onEffortChange).toHaveBeenCalledWith("high");
    cleanup();
    mount({ effortLevels: null, onEffortChange: view.onEffortChange });
    fireEvent.keyDown(screen.getByLabelText("Message"), {
      key: "Tab",
      shiftKey: true,
    });
    expect(view.onEffortChange).toHaveBeenCalledTimes(1);
  });

  it("wires the Stop / Close actions to their lanes", () => {
    const onStop = vi.fn();
    const onClose = vi.fn();
    mount({ streaming: true, onStop, onClose });
    screen.getByRole("button", { name: "Stop" }).click();
    screen.getByRole("button", { name: "Close" }).click();
    expect(onStop).toHaveBeenCalledTimes(1);
    expect(onClose).toHaveBeenCalledTimes(1);
  });
});
