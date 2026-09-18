// The composer's `/` palette surface (plan 124; plan 125 made it the shell's
// only transient surface): the sheet the lane draws as the menu of its field.
// These are the properties that make it the field's menu rather than a second
// window — the head echoes (with no input of its own, except the shielded
// credential stage), the server's selection is the row fill, the foot states
// the keys, mode and live count, and the empty state is the sheet's own key,
// not a card inside a card.
// @vitest-environment jsdom
import {
  cleanup,
  fireEvent,
  render,
  screen,
  within,
} from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { TransientMenuSnapshotDto } from "../bridge/types";

import {
  CommandPalette,
  paletteModeOf,
  SECONDARY_ACTION_BINDING,
} from "./CommandPalette";
import { paletteItems, paletteMenu } from "../test/palette-stub";

/** A picker stage's snapshot: the composer's palette with a presentation mode
 *  (protocol v32) and the server's own prompt as its name. */
function stageMenu(
  mode: "picker" | "secret" | "url" | "oauth",
  items: TransientMenuSnapshotDto["items"] = paletteItems,
  prompt = "Providers",
) {
  return paletteMenu(items, { mode, prompt });
}

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

  it("draws a picker stage with its prompt, its filter and its own keys", () => {
    // Plan 125: the same sheet carries every picker stage. The stage's prompt
    // is the micro-label (and the dialog's name), the field's echo stays — a
    // stage's filter is typed without a sigil — and the foot states the stage's
    // verb plus the walk back. Scope chips are the catalogue's vocabulary, so a
    // stage draws none even when its rows carry scopes.
    render(
      <CommandPalette
        menu={stageMenu("picker")}
        scope="all"
        onScope={vi.fn()}
        query="anth"
        onPick={vi.fn()}
      />,
    );
    const sheet = screen.getByRole("dialog", { name: "Providers" });
    expect(sheet).toBeVisible();
    expect(sheet.dataset.mode).toBe("picker");
    expect(within(sheet).getByText("Providers")).toBeVisible();
    expect(within(sheet).getByText("anth")).toBeVisible();
    expect(screen.queryByRole("group", { name: "Scope" })).toBeNull();
    expect(screen.getByText("choose")).toBeVisible();
    // Both walk-back keys are stated (Esc and Alt+←), so the verb is drawn
    // once per key.
    expect(screen.getAllByText("back")).toHaveLength(2);
    expect(screen.getByText("Alt+←")).toBeVisible();
  });

  it("names the stage's verb per mode and never draws the shield but once", () => {
    const modes: Array<[Parameters<typeof stageMenu>[0], string]> = [
      ["picker", "choose"],
      ["secret", "store"],
      ["url", "save"],
      ["oauth", "run"],
    ];
    for (const [mode, verb] of modes) {
      const view = render(
        <CommandPalette
          menu={stageMenu(mode, [
            {
              id: "row",
              label: "Row",
              detail: null,
              accessibilityLabel: "Row",
            },
          ])}
          scope="all"
          onScope={vi.fn()}
          query=""
          onPick={vi.fn()}
          onSecret={vi.fn()}
        />,
      );
      expect(screen.getByText(verb)).toBeVisible();
      // One stage owns an input: the credential's shield, which is a password
      // control (no implicit `textbox` role) and the only one here. A stage
      // with an empty filter shows the prompt line alone (no placeholder echo).
      expect(view.container.querySelectorAll("input")).toHaveLength(
        mode === "secret" ? 1 : 0,
      );
      view.unmount();
    }
  });

  it("shields the credential stage inside the sheet and keeps it out of the DOM", () => {
    // Security (plan 125): the value is typed into the sheet's own field, the
    // composer never holds it, and nothing renders it back as text or as an
    // attribute — the wire echo it may receive is the server's bullet mask.
    const onSecret = vi.fn();
    const { container } = render(
      <CommandPalette
        menu={stageMenu(
          "secret",
          [
            {
              id: "store_secret",
              label: "Store API key",
              detail: "Value is hidden. Enter stores it.",
              accessibilityLabel: "Store API key",
            },
          ],
          "API key (hidden)",
        )}
        scope="all"
        onScope={vi.fn()}
        query="•••"
        onPick={vi.fn()}
        onSecret={onSecret}
      />,
    );
    const shield = screen.getByLabelText("API key (hidden)", {
      selector: "input",
    }) as HTMLInputElement;
    expect(shield.getAttribute("type")).toBe("password");
    expect(shield.getAttribute("autocomplete")).toBe("off");
    expect(shield.getAttribute("spellcheck")).toBe("false");
    // The stage's echo is not drawn at all: the shield is what carries the
    // value, and the server's mask is never read back into the page.
    expect(screen.queryByText("•••")).toBeNull();
    fireEvent.change(shield, { target: { value: "sk-super-secret" } });
    expect(onSecret).toHaveBeenCalledWith("sk-super-secret");
    expect(container.textContent).not.toContain("sk-super-secret");
    for (const element of container.querySelectorAll("*")) {
      for (const attribute of Array.from(element.attributes)) {
        // The control the user typed into is the one place the value lives;
        // nothing else in the sheet (text, name, hint) carries it.
        if (element === shield && attribute.name === "value") continue;
        expect(attribute.value).not.toContain("sk-super-secret");
      }
    }
  });

  it("clears the shield when the stage changes or the sheet reopens", () => {
    const items = [
      {
        id: "store_secret",
        label: "Store API key",
        detail: null,
        accessibilityLabel: "Store API key",
      },
    ];
    const props = {
      scope: "all" as const,
      onScope: vi.fn(),
      query: "",
      onPick: vi.fn(),
      onSecret: vi.fn(),
    };
    const view = render(
      <CommandPalette menu={stageMenu("secret", items)} {...props} />,
    );
    fireEvent.change(
      screen.getByLabelText("Providers", { selector: "input" }),
      {
        target: { value: "typed-secret" },
      },
    );
    // A stage change inside one session (or a new session) starts the shield
    // empty: a credential outlives neither.
    view.rerender(
      <CommandPalette
        menu={stageMenu("picker", items, "Choose sign-in method")}
        {...props}
      />,
    );
    expect(
      screen.queryByLabelText("Providers", { selector: "input" }),
    ).toBeNull();
    view.rerender(
      <CommandPalette
        menu={paletteMenu(items, {
          mode: "secret",
          prompt: "Providers",
          sessionId: "9" as never,
        })}
        {...props}
      />,
    );
    expect(
      (
        screen.getByLabelText("Providers", {
          selector: "input",
        }) as HTMLInputElement
      ).value,
    ).toBe("");
  });

  it("states a row's secondary action from the row's own binding", () => {
    // The session list's rows declare `Alt+↵`; the sheet renders the chip and
    // names the verb its activation carries (delete instead of resume).
    render(
      <CommandPalette
        menu={stageMenu(
          "picker",
          [
            {
              id: "session:1",
              label: "Palette stage flows",
              detail: "/workspace",
              bindings: [SECONDARY_ACTION_BINDING],
              accessibilityLabel: "Palette stage flows",
            },
          ],
          "Sessions",
        )}
        scope="all"
        onScope={vi.fn()}
        query=""
        onPick={vi.fn()}
      />,
    );
    const row = screen.getByRole("option", { name: /Palette stage flows/ });
    expect(within(row).getByText(SECONDARY_ACTION_BINDING)).toBeVisible();
    expect(screen.getByText("resume")).toBeVisible();
    expect(screen.getByText("delete")).toBeVisible();
  });

  it("points the listbox at the selected option for list stages", () => {
    render(
      <CommandPalette
        menu={stageMenu("picker", paletteItems, "Providers")}
        scope="all"
        onScope={vi.fn()}
        query=""
        onPick={vi.fn()}
      />,
    );
    expect(
      screen.getByRole("listbox").getAttribute("aria-activedescendant"),
    ).toBe("pal-option-0");
    expect(screen.getAllByRole("option")[0]?.id).toBe("pal-option-0");
  });

  it("sends a stage's empty state back a stage, not out of the shell", () => {
    render(
      <CommandPalette
        menu={stageMenu("picker", [], "Providers")}
        scope="all"
        onScope={vi.fn()}
        query="zzz"
        onPick={vi.fn()}
      />,
    );
    expect(screen.getByTestId("command-palette")).toHaveTextContent(
      "goes back a stage",
    );
    expect(screen.getByTestId("command-palette")).not.toHaveTextContent(
      "dismisses the palette",
    );
  });

  it("falls back to a list stage for a mode the vocabulary does not know", () => {
    // The vocabulary is closed, and an unknown spelling must not be drawn as
    // the catalogue: it claims no sigil and no shield.
    expect(
      paletteModeOf(paletteMenu([], { mode: "future-mode" as never })),
    ).toBe("picker");
    expect(paletteModeOf(paletteMenu([]))).toBe("catalogue");
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
