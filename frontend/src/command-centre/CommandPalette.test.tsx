// The composer's `/` palette surface (plan 124): the sheet the lane draws as
// the menu of its field. These are the properties that make it the field's
// menu rather than a second window — no input of its own (the head echoes), the
// server's selection is the row fill, the foot states the keys and the live
// count, and the empty state is the sheet's own key, not a card inside a card.
// @vitest-environment jsdom
import {
  cleanup,
  fireEvent,
  render,
  screen,
  within,
} from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { CommandPalette } from "./CommandPalette";
import { paletteItems, paletteMenu } from "../test/palette-stub";

afterEach(cleanup);

describe("CommandPalette", () => {
  it("echoes the field as its query and owns no input", () => {
    render(
      <CommandPalette
        menu={paletteMenu(paletteItems)}
        scope="all"
        onScope={vi.fn()}
        query="com"
        onPick={vi.fn()}
      />,
    );
    // The sheet is the dialog the field names; it has no textbox at all.
    const sheet = screen.getByRole("dialog", { name: "Commands" });
    expect(sheet).toBeVisible();
    expect(screen.queryByRole("textbox")).toBeNull();
    // The head echoes the field's filter, sigil drawn by CSS.
    expect(screen.getByText("com")).toBeVisible();
    expect(screen.getByText("com").dataset.empty).toBe("false");
    // The rows carry the server's label + detail (routing, provenance) and its
    // own chips: chords from `bindings`, scope from the segment.
    expect(screen.getByRole("option", { name: /\/compact/ })).toBeVisible();
    expect(screen.getByText("client — built-in")).toBeVisible();
    expect(screen.getByText("Ctrl+X")).toBeVisible();
    expect(screen.getByText("Ctrl+P")).toBeVisible();
  });

  it("shows the field's placeholder echo while the filter is empty", () => {
    render(
      <CommandPalette
        menu={paletteMenu(paletteItems)}
        scope="all"
        onScope={vi.fn()}
        query=""
        onPick={vi.fn()}
      />,
    );
    const echo = screen.getByText("type a command");
    expect(echo.dataset.empty).toBe("true");
  });

  it("marks the server's selected row and announces the count", () => {
    render(
      <CommandPalette
        menu={paletteMenu(paletteItems, { selectedIndex: 1 })}
        scope="all"
        onScope={vi.fn()}
        query=""
        onPick={vi.fn()}
      />,
    );
    const rows = screen.getAllByRole("option");
    expect(rows).toHaveLength(2);
    expect(rows[0]?.getAttribute("aria-selected")).toBe("false");
    expect(rows[1]?.getAttribute("aria-selected")).toBe("true");
    expect(rows[1]?.dataset.selected).toBe("true");
    const count = screen.getByText("2 results");
    expect(count.tagName).toBe("OUTPUT");
    expect(count.getAttribute("aria-live")).toBe("polite");
  });

  it("states one result in the singular", () => {
    render(
      <CommandPalette
        menu={paletteMenu(paletteItems.slice(0, 1))}
        scope="all"
        onScope={vi.fn()}
        query=""
        onPick={vi.fn()}
      />,
    );
    expect(screen.getByText("1 result")).toBeVisible();
  });

  it("runs the row a click names", () => {
    const onPick = vi.fn();
    render(
      <CommandPalette
        menu={paletteMenu(paletteItems)}
        scope="all"
        onScope={vi.fn()}
        query=""
        onPick={onPick}
      />,
    );
    fireEvent.click(screen.getByRole("option", { name: /Toggle Agent Lane/ }));
    expect(onPick).toHaveBeenCalledWith(1);
  });

  it("renders the session's empty key inside the sheet, not as a nested card", () => {
    render(
      <CommandPalette
        menu={paletteMenu([], {
          status: { empty: { message: "No commands match this query" } },
        })}
        scope="all"
        onScope={vi.fn()}
        query="zzz"
        onPick={vi.fn()}
      />,
    );
    expect(screen.queryByRole("listbox")).toBeNull();
    const sheet = screen.getByTestId("command-palette");
    expect(sheet).toHaveTextContent("No commands match this query");
    expect(sheet).toHaveTextContent("Esc");
    // Two live regions, each saying something different: the session's reason
    // and the count (the `<output>` is implicitly a status).
    expect(screen.getAllByRole("status")).toHaveLength(2);
  });

  it("falls back to a no-results statement when the session sends none", () => {
    render(
      <CommandPalette
        menu={paletteMenu([])}
        scope="all"
        onScope={vi.fn()}
        query="zzz"
        onPick={vi.fn()}
      />,
    );
    expect(screen.getByTestId("command-palette")).toHaveTextContent(
      "No results",
    );
    // The count still speaks for the empty list (the live region is the news).
    expect(screen.getByText("0 results")).toBeVisible();
  });

  it("draws the scope segment from the rows' scopes and reports a chip", () => {
    // Plan 124: the chips are server data — a chip exists because the session's
    // rows carry that scope, and the sheet fabricates no scope of its own.
    const onScope = vi.fn();
    render(
      <CommandPalette
        menu={paletteMenu(paletteItems)}
        scope="all"
        onScope={onScope}
        query=""
        onPick={vi.fn()}
      />,
    );
    const segment = screen.getByRole("group", { name: "Scope" });
    expect(segment).toBeVisible();
    const chips = within(segment).getAllByRole("button");
    expect(chips.map((chip) => chip.textContent)).toEqual([
      "All",
      "Session",
      "Shell",
      "Files",
    ]);
    expect(
      within(segment).getByRole("button", { name: "All" }),
    ).toHaveAttribute("aria-pressed", "true");
    fireEvent.click(within(segment).getByRole("button", { name: "Shell" }));
    expect(onScope).toHaveBeenCalledWith("shell");
  });

  it("marks the active chip and keeps the segment while a query empties a scope", () => {
    render(
      <CommandPalette
        menu={paletteMenu([], {
          status: { empty: { message: "No commands match this query" } },
        })}
        scope="files"
        onScope={vi.fn()}
        query="zzz"
        onPick={vi.fn()}
      />,
    );
    expect(
      screen
        .getByRole("button", { name: "Files" })
        .getAttribute("aria-pressed"),
    ).toBe("true");
  });

  it("renders no scope control when the session's rows carry no scope", () => {
    // A path-browser session's rows are paths, not catalogue commands: it has
    // no scopes, so the sheet offers no chip it cannot fill (the approved
    // artifact's rule).
    render(
      <CommandPalette
        menu={paletteMenu([
          {
            id: "path.0",
            label: "src/",
            detail: null,
            accessibilityLabel: "src/",
          },
        ])}
        scope="all"
        onScope={vi.fn()}
        query=""
        onPick={vi.fn()}
      />,
    );
    expect(screen.queryByRole("group", { name: "Scope" })).toBeNull();
  });

  it("renders a row's chords as chips and none for an unbound row", () => {
    render(
      <CommandPalette
        menu={paletteMenu(paletteItems)}
        scope="all"
        onScope={vi.fn()}
        query=""
        onPick={vi.fn()}
      />,
    );
    const bound = screen.getByRole("option", { name: /Toggle Agent Lane/ });
    expect(within(bound).getByText("Ctrl+X")).toBeVisible();
    expect(within(bound).getByText("Ctrl+P")).toBeVisible();
    const unbound = screen.getByRole("option", { name: /\/compact/ });
    expect(within(unbound).queryByText("Ctrl")).toBeNull();
  });

  it("states the keyboard model in the foot", () => {
    render(
      <CommandPalette
        menu={paletteMenu(paletteItems)}
        scope="all"
        onScope={vi.fn()}
        query=""
        onPick={vi.fn()}
      />,
    );
    expect(screen.getByText("navigate")).toBeVisible();
    expect(screen.getByText("run")).toBeVisible();
    expect(screen.getByText("close")).toBeVisible();
  });
});
