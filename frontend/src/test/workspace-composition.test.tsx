// @vitest-environment jsdom
import "@testing-library/jest-dom/vitest";
import {
  act,
  cleanup,
  fireEvent,
  render,
  screen,
} from "@testing-library/react";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { ClayEditor } from "../editor/ClayEditor";
import { createDocumentSession } from "../editor/sync/session";
import { workspaceRail } from "../shell/layout-state";
import { WorkspaceView } from "../routes/workspace";
import type { BootstrapDto } from "../bridge/types";

afterEach(cleanup);
beforeEach(() => workspaceRail.setVisible(true));

const DOC = `# Notes

Fixture document.

## 26-08-12 01:15 — outline-entry-one

Body text.

## 26-08-13 09:40 — outline-entry-two

More body text.
`;

function session(doc = DOC) {
  const created = createDocumentSession({ send: async () => undefined });
  created.installInitial({
    clientId: 1,
    protocolVersion: 28,
    endpoint: "test",
    generation: 1,
    initialDocument: {
      documentId: 1,
      version: 7,
      head: { totalBytes: doc.length, firstChunk: doc },
      access: { editable: { leaseId: 1 } },
      workspaceRoot: "/tmp/clay",
    },
    behaviorManifest: {
      manifestId: "m",
      behaviorVersion: 1,
      commands: [],
      keymaps: [],
    },
  } as unknown as BootstrapDto);
  created.store.update({
    workspaceRootId: 1,
    path: "notes.png".replace("png", "md"),
  });
  return created;
}

const repoRoot = path.resolve(
  path.dirname(fileURLToPath(import.meta.url)),
  "../../..",
);
const readRepo = (relative: string) =>
  fs.readFileSync(path.join(repoRoot, relative), "utf8");

describe("workspace composition (plan 118: shell and Workspace page)", () => {
  it("renders sidebar · editor · rail from real document metadata", () => {
    const active = session();
    render(
      <WorkspaceView session={active}>
        <ClayEditor session={active} />
      </WorkspaceView>,
    );

    const rail = screen.getByTestId("workspace-rail");
    expect(rail).toBeInTheDocument();
    // Facts are the open document's own metadata: nothing is fabricated.
    expect(rail).toHaveTextContent("notes.md");
    expect(rail).toHaveTextContent("v7");
    expect(rail).toHaveTextContent("clean");
    // Outline: every heading, entry-stamped headings split into time + title.
    const entries = rail.querySelectorAll("li button");
    expect(entries).toHaveLength(3);
    expect(entries[0]?.textContent).toContain("Notes");
    expect(entries[1]?.textContent).toContain("01:15");
    expect(entries[1]?.textContent).toContain("outline-entry-one");
    expect(rail.querySelector("dl")).toHaveTextContent("3");
  });

  it("toggles the rail without remounting the editor, from the rail and the shell", () => {
    const active = session();
    render(
      <WorkspaceView session={active}>
        <ClayEditor session={active} data-testid="editor-slot" />
      </WorkspaceView>,
    );
    const view = document.querySelector("[data-rail]");
    expect(view?.getAttribute("data-rail")).toBe("expanded");

    fireEvent.click(screen.getByRole("button", { name: "Hide outline" }));
    expect(screen.queryByTestId("workspace-rail")).not.toBeInTheDocument();
    expect(
      document.querySelector("[data-rail]")?.getAttribute("data-rail"),
    ).toBe("collapsed");

    // The same toggle the titlebar and the status hint run.
    act(() => workspaceRail.toggle());
    expect(screen.getByTestId("workspace-rail")).toBeInTheDocument();
  });

  it("spies on the editor viewport and lets an explicit jump keep its entry (plan 118 E2)", () => {
    vi.useFakeTimers();
    const active = session();
    let topLine = 0;
    const listeners = new Set<() => void>();
    active.topVisibleLine = () => topLine;
    active.onViewportChange = (listener) => {
      listeners.add(listener);
      return () => listeners.delete(listener);
    };
    render(
      <WorkspaceView session={active}>
        <ClayEditor session={active} />
      </WorkspaceView>,
    );
    const rail = screen.getByTestId("workspace-rail");
    const entries = [...rail.querySelectorAll("li button")];
    const label = () =>
      entries.find((entry) => entry.getAttribute("data-active") === "true")
        ?.textContent ?? "";
    /** The rail coalesces scrolls into one animation frame; fake timers run it. */
    const scrollTo = (line: number) => {
      topLine = line;
      act(() => {
        for (const listener of [...listeners]) listener();
        vi.advanceTimersByTime(20);
      });
    };

    // At the top of the document the first heading is the reached entry.
    expect(label()).toContain("Notes");
    // Scrolling to a later heading moves the mark with it.
    scrollTo(8);
    expect(label()).toContain("outline-entry-two");

    // An explicit jump locks the spy, so the jump's own scroll cannot move it.
    fireEvent.click(entries[0] as HTMLElement);
    expect(label()).toContain("Notes");
    scrollTo(8);
    expect(label()).toContain("Notes");

    // After the lock window a manual scroll takes the rail over again.
    act(() => vi.advanceTimersByTime(900));
    scrollTo(8);
    expect(label()).toContain("outline-entry-two");
    vi.useRealTimers();
  });

  it("keeps every document action in one bar and the path field on demand", () => {
    const active = session();
    render(<ClayEditor session={active} />);
    const chrome = document.querySelector('[data-clay-ds="editor.chrome"]');
    expect(chrome).not.toBeNull();
    for (const name of ["Open", "Undo", "Redo", "Reload", "Save", "Close"]) {
      expect(
        [...(chrome?.querySelectorAll("button") ?? [])].some((button) =>
          (button.getAttribute("aria-label") ?? button.textContent)?.includes(
            name,
          ),
        ),
      ).toBe(true);
    }
    // The relative-path control is revealed, not permanently visible.
    expect(
      screen.queryByLabelText("Open relative path"),
    ).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Open" }));
    const field = screen.getByLabelText("Open relative path");
    expect(field).toHaveFocus();
    fireEvent.keyDown(field, { key: "Escape" });
    expect(
      screen.queryByLabelText("Open relative path"),
    ).not.toBeInTheDocument();
  });

  it("animates nothing, blurs nothing and holds the measure (DESIGN.md §5–§7)", () => {
    const shell = readRepo("frontend/src/app/layout/shell.module.css");
    const workspace = readRepo("frontend/src/routes/workspace.module.css");
    const editor = readRepo("frontend/src/editor/editor.module.css");

    // Reading measure: the document column is 92ch and is centred.
    expect(editor).toContain("max-width: calc(92ch + 4rem)");
    expect(editor).toMatch(/\.column\s*{[^}]*justify-content: center/);
    // Rail width is host geometry with a token override, not a magic number.
    expect(workspace).toContain("--clay-dimension-rail-width, 340px");
    // Datum rows: micro-label tracking, mono values, list-family row geometry.
    expect(workspace).toContain("letter-spacing: 0.14em");
    expect(workspace).toContain(
      "--clay-ds-list-default-row-rest-border-radius",
    );
    // Chrome strips: host-owned heights, recipe-driven hairlines, mono data.
    expect(shell).toContain("--clay-dimension-titlebar-height, 40px");
    expect(shell).toContain("--clay-dimension-statusbar-height, 28px");
    expect(shell).toContain("font-variant-numeric: tabular-nums");
    expect(shell).toMatch(
      /\.footer\s*{[^}]*font-family: var\(--clay-font-monospace\)/,
    );
    // No blur, no filter, no animation on any of the three surfaces.
    for (const css of [shell, workspace, editor]) {
      expect(css).not.toMatch(/backdrop-filter/);
      expect(css).not.toMatch(/\bfilter:/);
      expect(css).not.toMatch(/@keyframes/);
    }
  });

  it("keeps the sidebar a flush region and the rail a transient drawer when narrow", () => {
    const workspace = readRepo("frontend/src/routes/workspace.module.css");
    // Narrow windows: the rail leaves the flow instead of crushing the measure.
    expect(workspace).toMatch(
      /@media \(max-width: 1000px\)[\s\S]*\.rail\s*{[^}]*position: fixed/,
    );
    // The workspace host paints the sidebar region (fileBrowser + divider); the
    // SDUI tree no longer wraps it in a panel.
    const fileBrowser = readRepo("src/shell/file_browser.rs");
    expect(fileBrowser).toMatch(/fn to_sdui_tree[\s\S]*SduiNodeKind::Stack/);
    expect(fileBrowser).not.toMatch(
      /fn to_sdui_tree[\s\S]{0,2000}?SduiNodeKind::Panel/,
    );
  });
});
