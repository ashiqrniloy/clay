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
import { SduiRenderer } from "../sdui/renderer";
import { installSduiTree } from "../sdui/state";
import { workspaceRail } from "../shell/layout-state";
import { WorkspaceView } from "../routes/workspace";
import type { BootstrapDto } from "../bridge/types";
import { behaviorManifestFixture } from "../test/contract-fixtures";

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
    behaviorManifest: behaviorManifestFixture({ behaviorVersion: 1 }),
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
  it("keeps the file listing's rows in a scrolling box", () => {
    const css = readRepo("frontend/src/components/controls.module.css");
    expect(css).toMatch(
      /\.listFiltered\s*>\s*\.listBox\s*\{[^}]*overflow-y:\s*auto/s,
    );
  });

  // The path strip and the two alert rows above the canvas only exist in some
  // states, so the canvas is pinned to the last row: auto-placement would drop
  // it into an `auto` row and collapse the editor to its content height.
  it("keeps the editor canvas filling the last host row", () => {
    const css = readRepo("frontend/src/editor/editor.module.css");
    expect(css).toMatch(
      /\.host\s*\{[^}]*grid-template-rows:\s*auto auto auto minmax\(0,\s*1fr\)/s,
    );
    expect(css).toMatch(/\.column\s*\{[^}]*grid-row:\s*-2\s*\/\s*-1/s);
  });

  it("keeps the sidebar title row at one line, not a full-height row", () => {
    const { container } = render(
      <SduiRenderer
        state={installSduiTree({
          uiVersion: 6,
          rootId: 1,
          nodes: [
            { id: 1, kind: { flex: { direction: "row", children: [2, 4] } } },
            { id: 2, kind: { stack: { children: [7, 5] } } },
            {
              id: 3,
              kind: { label: { text: "Workspace · clay", icon: null } },
            },
            {
              id: 6,
              kind: {
                button: {
                  label: "Hide file browser",
                  icon: "disclosure.right",
                  action: {
                    commandId: "workspace.toggleFileBrowser",
                    source: { button: { nodeId: 6 } },
                    arguments: [],
                  },
                },
              },
            },
            { id: 7, kind: { flex: { direction: "row", children: [3, 6] } } },
            { id: 5, kind: { list: { items: [], filter: null } } },
            {
              id: 4,
              kind: {
                editorView: { binding: { documentId: 1, expectedVersion: 2 } },
              },
            },
          ],
        })}
        send={async () => undefined}
        editorSlot={<div>editor</div>}
      />,
    );
    const head = container.querySelector("div[class*='sidebarHead']");
    expect(head).not.toBeNull();
    const css = readRepo("frontend/src/sdui/renderer.module.css");
    // The head is a nested `.row`, so it must opt out of the root row's
    // `height: 100%` claim — otherwise it takes ~a sidebar of rows.
    expect(css).toMatch(/\.sidebarHead\s*\{[^}]*height:\s*auto/s);
    expect(css).toMatch(/\.sidebarHead\s*\{[^}]*flex:\s*none/s);
    // Its children must opt out of the generic row share the same way, so
    // the label takes the line and the `>` keeps its fixed size.
    expect(css).toMatch(/\.sidebarHead\s*>\s*\*\s*\{[^}]*flex:\s*none/s);
    // Long `Workspace · name · nested/dir` titles truncate on the shared line
    // instead of wrapping the head onto many rows (verified live: a long
    // title keeps the head at one 32px line).
    expect(css).toMatch(
      /\.sidebarHead\s*>\s*:first-child\s*\{[^}]*white-space:\s*nowrap/s,
    );
    expect(css).toMatch(
      /\.sidebarHead\s*>\s*:first-child\s*\{[^}]*text-overflow:\s*ellipsis/s,
    );
  });

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

  it("removes the workspace rail when the agent owns the right slot", () => {
    const active = session();
    render(
      <WorkspaceView session={active} showRail={false}>
        <ClayEditor session={active} />
      </WorkspaceView>,
    );

    expect(screen.queryByTestId("workspace-rail")).not.toBeInTheDocument();
    expect(document.querySelector("[data-rail]")).toHaveAttribute(
      "data-rail",
      "collapsed",
    );
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

    // Document canvas: full-bleed and left-aligned — no centred measure,
    // no side gutters on the column or the CodeMirror content.
    expect(editor).toMatch(/\.column\s*{[^}]*justify-content:\s*flex-start/s);
    expect(editor).toMatch(/\.column\s*{[^}]*padding:\s*0/s);
    expect(editor).not.toContain("92ch");
    expect(
      readRepo("frontend/src/editor/create-editor.ts").replace(
        /caret-color[^,}]*,?/g,
        "",
      ),
    ).not.toMatch(/\.cm-content[\s\S]{0,200}?padding/);
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
