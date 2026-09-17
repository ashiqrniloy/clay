// Composer (plan 119 SC-4): the input lane's pure state machine (chord match,
// effort cycle, @-token parse) plus the behaviours the lane relies on — the
// @-mention embedding, the consumed-submit contract, the one-shot file fetch,
// and (plan 124) the `/` palette the field drives as its query.
// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import {
  chordMatches,
  Composer,
  mentionTokenOf,
  nextEffortLevel,
  paletteFilter,
} from "./Composer";
import { paletteItems, paletteMenu, paletteStub } from "../test/palette-stub";

afterEach(cleanup);

function mount(overrides: Partial<Parameters<typeof Composer>[0]> = {}) {
  const props = {
    agentless: false,
    streaming: false,
    sessionId: "s1",
    command: vi.fn(),
    palette: paletteStub(),
    skills: [{ name: "wiki-init" }],
    files: ["src/editor/position-index.ts"],
    effortLevels: null,
    shownEffort: null,
    onEffortChange: vi.fn(),
    effortChord: null,
    onStop: vi.fn(),
    onSubmit: vi.fn(() => true),
    ...overrides,
  };
  const view = render(<Composer {...props} />);
  return { ...props, rerender: view.rerender };
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
  it("routes a `/` draft to the palette: one open, then the field as its query", () => {
    const palette = paletteStub();
    mount({ palette });
    const field = screen.getByLabelText("Message") as HTMLTextAreaElement;
    fireEvent.change(field, { target: { value: "/" } });
    // The first slash asks for the session; the field already holds the query
    // (there is no inline slash list any more — plan 124).
    expect(palette.request).toHaveBeenCalledTimes(1);
    expect(palette.query).not.toHaveBeenCalled();
    expect(
      screen.queryByRole("listbox", { name: "Slash commands" }),
    ).toBeNull();
    expect(paletteFilter("/com")).toBe("com");
  });

  it("renders the session's rows in the lane-anchored sheet", () => {
    const palette = paletteStub({ menu: paletteMenu(paletteItems) });
    mount({ palette });
    // The sheet is the field's own menu (plan 124): it spans the composer box
    // and echoes the query; every row is a catalogue row.
    const sheet = screen.getByTestId("command-palette");
    expect(sheet.getAttribute("role")).toBe("dialog");
    expect(sheet.getAttribute("aria-label")).toBe("Commands");
    expect(screen.getByRole("option", { name: /\/compact/ })).toBeTruthy();
    // The chord is the row's own chip group now (plan 124), not text in the
    // detail line, and the sheet's segment is drawn from the rows' scopes.
    expect(screen.getByText("Ctrl+X")).toBeTruthy();
    expect(screen.getByText("Ctrl+P")).toBeTruthy();
    expect(screen.getByRole("button", { name: "Session" })).toBeTruthy();
    expect(screen.getByText("2 results")).toBeTruthy();
  });

  it("closes the palette when the slash goes away", () => {
    // Losing the sigil is losing the query: the session closes and the field
    // goes back to being a prompt (the artifact's own live-state rule). The
    // next `/` opens a session again — with the menu back to null, which is the
    // shell's state, so the reopen is asserted where that state lives.
    const palette = paletteStub({ menu: paletteMenu(paletteItems) });
    mount({ palette });
    const field = screen.getByLabelText("Message");
    fireEvent.change(field, { target: { value: "/c" } });
    expect(palette.query).toHaveBeenLastCalledWith("c", "all");
    fireEvent.change(field, { target: { value: "" } });
    expect(palette.cancel).toHaveBeenCalledTimes(1);
  });

  it("seeds the sigil and focuses the field when the session opens on its own", () => {
    // The titlebar trigger and `Ctrl+X Ctrl+O` open the session without the
    // field saying so: the field *is* the query, so it takes the sigil and the
    // focus (DESIGN.md §12).
    const palette = paletteStub({ menu: paletteMenu(paletteItems) });
    mount({ palette });
    const field = screen.getByLabelText("Message") as HTMLTextAreaElement;
    expect(field.value).toBe("/");
    expect(document.activeElement).toBe(field);
    // The catch-up filter for a trigger-opened session is empty.
    expect(palette.query).toHaveBeenCalledWith("", "all");
  });

  it("sends the chip with the filter it is filtering", () => {
    // Plan 124: the scope chip is part of the session's filter, not a local
    // view — the client never hides a row the server still selects over.
    const palette = paletteStub({ menu: paletteMenu(paletteItems) });
    mount({ palette });
    const field = screen.getByLabelText("Message") as HTMLTextAreaElement;
    fireEvent.change(field, { target: { value: "/comp" } });
    expect(palette.query).toHaveBeenLastCalledWith("comp", "all");
    fireEvent.click(screen.getByRole("button", { name: "Session" }));
    expect(palette.query).toHaveBeenLastCalledWith("comp", "session");
    // The chip is the sheet's state, so the segment shows the new selection.
    expect(
      screen
        .getByRole("button", { name: "Session" })
        .getAttribute("aria-pressed"),
    ).toBe("true");
  });

  it("starts a new session on the All chip", () => {
    // A fresh session begins unfiltered (the server starts it that way), so the
    // segment must not claim a scope the session is not applying.
    const palette = paletteStub({ menu: paletteMenu(paletteItems) });
    const props = mount({ palette });
    fireEvent.click(screen.getByRole("button", { name: "Shell" }));
    const { rerender, ...rest } = props;
    rerender(
      <Composer
        {...rest}
        palette={paletteStub({
          menu: paletteMenu(paletteItems, { sessionId: "9" as never }),
        })}
      />,
    );
    expect(
      screen.getByRole("button", { name: "All" }).getAttribute("aria-pressed"),
    ).toBe("true");
  });

  it("runs the selected row on Enter and clears the field, never sending it", () => {
    const palette = paletteStub({ menu: paletteMenu(paletteItems) });
    const props = mount({ palette });
    const field = screen.getByLabelText("Message") as HTMLTextAreaElement;
    fireEvent.change(field, { target: { value: "/" } });
    fireEvent.submit(field.closest("form") as HTMLFormElement);
    expect(palette.activate).toHaveBeenCalledTimes(1);
    expect(props.onSubmit).not.toHaveBeenCalled();
    expect(field.value).toBe("");
  });

  it("submits what was typed when the palette has no rows to run", () => {
    // A typed built-in still has a path: with nothing to activate, Enter falls
    // through to the lane (which intercepts `/model`, `/resume`).
    const palette = paletteStub({
      menu: paletteMenu([], { status: { empty: { message: "No match" } } }),
    });
    const props = mount({ palette });
    const field = screen.getByLabelText("Message") as HTMLTextAreaElement;
    fireEvent.change(field, { target: { value: "/model" } });
    fireEvent.submit(field.closest("form") as HTMLFormElement);
    expect(palette.activate).not.toHaveBeenCalled();
    expect(props.onSubmit).toHaveBeenCalledWith("/model");
  });

  it("moves the session's selection with the arrows and dismisses with Esc", () => {
    const palette = paletteStub({ menu: paletteMenu(paletteItems) });
    mount({ palette });
    const field = screen.getByLabelText("Message");
    fireEvent.keyDown(field, { key: "ArrowDown" });
    fireEvent.keyDown(field, { key: "ArrowUp" });
    expect(palette.move).toHaveBeenNthCalledWith(1, 1);
    expect(palette.move).toHaveBeenNthCalledWith(2, -1);
    fireEvent.keyDown(field, { key: "Escape" });
    expect(palette.cancel).toHaveBeenCalledTimes(1);
    // Esc keeps the draft (the artifact's own rule); the session is what closes.
    expect((field as HTMLTextAreaElement).value).toBe("/");
  });

  it("picks a row by click: the selection moves there, then it runs", () => {
    const palette = paletteStub({
      menu: paletteMenu(paletteItems, { selectedIndex: 0 }),
    });
    mount({ palette });
    fireEvent.click(screen.getByRole("option", { name: /Toggle Agent Lane/ }));
    expect(palette.move).toHaveBeenCalledWith(1);
    expect(palette.activate).toHaveBeenCalledTimes(1);
  });

  it("states what each mention row is: the skill's description, the file's directory", () => {
    mount({
      skills: [{ name: "wiki", description: "repo context graph" }],
      files: ["src/editor/position-index.ts", "README.md"],
    });
    const field = screen.getByLabelText("Message");
    fireEvent.change(field, { target: { value: "read @" } });
    // The approved artifact's row composition: name, then the muted detail
    // (skill description / file directory, `repository root` at the top).
    expect(
      screen.getByRole("option", { name: /@skill:wiki/ }),
    ).toHaveTextContent("repo context graph");
    expect(
      screen.getByRole("option", {
        name: /@file:src\/editor\/position-index.ts/,
      }),
    ).toHaveTextContent("src/editor");
    expect(
      screen.getByRole("option", { name: /@file:README.md/ }),
    ).toHaveTextContent("repository root");
  });

  it("reports the mentions menu up so the shell can veil the working area", () => {
    const onFieldMenuOpen = vi.fn();
    mount({ onFieldMenuOpen });
    const field = screen.getByLabelText("Message");
    fireEvent.change(field, { target: { value: "read @wiki" } });
    // `@` mentions are the inline list that is left — and they veil too.
    expect(onFieldMenuOpen).toHaveBeenLastCalledWith(true);
    fireEvent.change(field, { target: { value: "read " } });
    expect(onFieldMenuOpen).toHaveBeenLastCalledWith(false);
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

  it("wires Stop into the trailing slot and leaves it empty at rest", () => {
    // Plan 124: no Send button — `↵` sends (the hint row says so) — and Stop
    // takes the shell's trailing edge exactly while a run is live.
    const onStop = vi.fn();
    mount({ streaming: true, onStop });
    screen.getByRole("button", { name: "Stop" }).click();
    expect(onStop).toHaveBeenCalledTimes(1);
    cleanup();
    mount({ streaming: false });
    expect(screen.queryByRole("button", { name: "Stop" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Send" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Close" })).toBeNull();
  });

  it("keeps the field typeable with no provider and inert with no agent", () => {
    // Plan 124 review: a missing provider never blocks typing — the lane's foot
    // states why nothing will send. A tab with no agent at all is the one case
    // where the field is inert.
    mount({ agentless: false });
    const field = screen.getByLabelText("Message");
    expect(field).toHaveProperty("placeholder", "Ask, or type / or @");
    expect(field).toBeEnabled();
    cleanup();
    mount({ agentless: true });
    const inert = screen.getByLabelText("Message");
    expect(inert).toHaveProperty(
      "placeholder",
      "Attach an agent to this tab to send a prompt",
    );
    expect(inert).toBeDisabled();
  });

  it("renders the lane's agent-control toolbar inside the field shell", () => {
    mount({ toolbar: <span data-testid="lane-controls">controls</span> });
    const toolbar = screen.getByTestId("lane-controls");
    // The box stays the only boundary: the toolbar sits inside the field shell
    // (the element that paints `textInput`), not in a second bordered row.
    const shell = screen
      .getByLabelText("Message")
      .closest("[data-clay-component='textInput'][data-clay-slot='field']");
    expect(shell).not.toBeNull();
    expect(shell?.contains(toolbar)).toBe(true);
    expect(toolbar.closest("form")).not.toBeNull();
  });
});
