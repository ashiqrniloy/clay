// @vitest-environment jsdom
import "@testing-library/jest-dom/vitest";
import { cleanup, render } from "@testing-library/react";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { afterEach, describe, expect, it, vi } from "vitest";

import {
  ClayButton,
  ClayDropdown,
  ClayList,
  ClayTabStrip,
  ClayText,
  ClayTextField,
} from "../components";
import { PackageComponent } from "./registry";
import type { PackageComponentNode } from "./types";

afterEach(cleanup);

// Plan 118 task 17: the SDUI host must not paint its own version of a catalog
// component. A manifest-driven node and its React counterpart therefore render
// the *same* recipe attributes and the same module classes — that is what makes
// their computed geometry identical under `@clay/core` and under the shipped
// package, since both read the same `--clay-ds-*` values.

const repoRoot = path.resolve(
  path.dirname(fileURLToPath(import.meta.url)),
  "../../..",
);
const readRepo = (relative: string) =>
  fs.readFileSync(path.join(repoRoot, relative), "utf8");

function contract(element: Element | null) {
  const node = element as HTMLElement | null;
  return {
    component: node?.getAttribute("data-clay-component") ?? null,
    slot: node?.getAttribute("data-clay-slot") ?? null,
    variant: node?.getAttribute("data-variant") ?? null,
    className: node?.className ?? null,
  };
}

function sdui(node: PackageComponentNode) {
  return render(<PackageComponent node={node} uiVersion={1} send={vi.fn()} />);
}

describe("SDUI ⇄ catalog parity (plan 118 task 17)", () => {
  it("renders a manifest button and its React counterpart with one contract", () => {
    const { container } = sdui({
      id: "pkg.action",
      kind: "button",
      label: "Run",
      action: { commandId: "pkg.run" },
      style: { variant: "primary" },
    });
    const sduiButton = contract(container.querySelector("button"));

    cleanup();
    const { container: react } = render(
      <ClayButton variant="primary">Run</ClayButton>,
    );
    const reactButton = contract(react.querySelector("button"));

    expect(sduiButton.component).toBe("button");
    expect(sduiButton.slot).toBe("root");
    // The class comes from `components/button.module.css` in both cases: the
    // SDUI host delegates paint to the catalog component instead of restyling it.
    expect(sduiButton.className).toBe(reactButton.className);
  });

  it("delegates every SDUI kind that has a catalog counterpart", () => {
    // label -> ClayText: the SDUI label is a text role, not a bespoke span.
    const labelRender = sdui({
      id: "pkg.label",
      kind: "label",
      text: "Ready",
      style: { typography: "typography.body" },
    });
    const reactText = render(<ClayText variant="body">Ready</ClayText>);
    expect(contract(labelRender.container.firstElementChild).className).toBe(
      contract(reactText.container.firstElementChild).className,
    );
    cleanup();

    // textInput -> ClayTextField (one ring per surface lives in the field).
    const inputRender = sdui({
      id: "pkg.field",
      kind: "textInput",
      title: "Path",
    });
    const reactField = render(
      <ClayTextField label="Path" value="" onChange={() => {}} />,
    );
    const sduiField = inputRender.container.querySelector("input");
    const reactInput = reactField.container.querySelector("input");
    expect(sduiField?.className).toBe(reactInput?.className);
    expect(
      inputRender.container
        .querySelector("[data-clay-component]")
        ?.getAttribute("data-clay-component"),
    ).toBe(
      reactField.container
        .querySelector("[data-clay-component]")
        ?.getAttribute("data-clay-component"),
    );
    cleanup();

    // list -> ClayList (rows carry the list family's own recipe).
    const listRender = sdui({
      id: "pkg.list",
      kind: "list",
      items: [{ id: "a", label: "Alpha" }],
    });
    const reactList = render(
      <ClayList
        ariaLabel="Alpha"
        items={[{ id: "a", title: "Alpha" }]}
        onAction={() => {}}
      />,
    );
    expect(contract(listRender.container.querySelector("li")).className).toBe(
      contract(reactList.container.querySelector("li")).className,
    );
    cleanup();

    // dropdown -> ClayDropdown (trigger + popover come from the catalog module).
    const dropdownRender = sdui({
      id: "pkg.select",
      kind: "dropdown",
      title: "Model",
      items: [{ id: "m1", label: "glm-5.3-flash", selected: true }],
    });
    const reactDropdown = render(
      <ClayDropdown
        label="Model"
        options={[{ id: "m1", label: "glm-5.3-flash" }]}
        selectedId="m1"
        onSelect={() => {}}
      />,
    );
    const sduiTrigger = contract(
      dropdownRender.container.querySelector("[data-clay-slot='trigger']"),
    );
    const reactTrigger = contract(
      reactDropdown.container.querySelector("[data-clay-slot='trigger']"),
    );
    expect(sduiTrigger).toEqual(reactTrigger);
    cleanup();

    // tabList -> ClayTabStrip.
    const tabsRender = sdui({
      id: "pkg.tabs",
      kind: "tabList",
      title: "Views",
      items: [{ id: "one", label: "One", selected: true }],
      children: [{ id: "pkg.tabs.body", kind: "label", text: "Panel" }],
    });
    const reactTabs = render(
      <ClayTabStrip
        ariaLabel="Views"
        tabs={[{ id: "one", label: "One", content: <span>Panel</span> }]}
      />,
    );
    expect(
      contract(tabsRender.container.querySelector("[role='tab']")).className,
    ).toBe(
      contract(reactTabs.container.querySelector("[role='tab']")).className,
    );
    cleanup();
  });

  it("paints SDUI containers from their own family, never a borrowed variant", () => {
    const registryCss = readRepo("frontend/src/sdui/registry.module.css");
    const rendererCss = readRepo("frontend/src/sdui/renderer.module.css");

    // An undirected flex node wears `flex.default`, not `flex.column`.
    for (const css of [registryCss, rendererCss]) {
      expect(css).toContain("--clay-ds-flex-default-root-rest-gap");
      expect(css).not.toMatch(/\.flexDefault[^}]*flex-column/);
    }
    // Overlay and portal are the SDUI layers' own recipes.
    expect(registryCss).toContain(
      "--clay-ds-overlay-default-root-rest-border-radius",
    );
    expect(registryCss).toContain(
      "--clay-ds-portal-default-root-rest-border-width",
    );
    // A panel title is the panel family's header, not a bare title role.
    expect(registryCss).toContain(
      "--clay-ds-panel-default-header-rest-border-color",
    );
  });

  it("consumes panel, flex and stack recipes identically in registry and renderer", () => {
    const registryCss = readRepo("frontend/src/sdui/registry.module.css");
    const rendererCss = readRepo("frontend/src/sdui/renderer.module.css");

    for (const prefix of [
      "--clay-ds-panel-default-root-rest-",
      "--clay-ds-flex-column-root-rest-",
      "--clay-ds-stack-default-root-rest-",
    ]) {
      expect(registryCss, `registry must consume ${prefix}`).toContain(prefix);
      expect(rendererCss, `renderer must consume ${prefix}`).toContain(prefix);
    }
  });
});

describe("editor and package-surface invariants (plan 118 task 17)", () => {
  const settingsCss = readRepo(
    "frontend/src/settings/settings-panel.module.css",
  );
  const agentSettingsCss = readRepo(
    "frontend/src/agent-settings/agent-settings.module.css",
  );
  const agentCss = readRepo(
    "frontend/src/coding-agent/coding-agent.module.css",
  );
  const editorCss = readRepo("frontend/src/editor/editor.module.css");
  const packageCss = readRepo(
    "frontend/src/packages/package-workspace.module.css",
  );
  const recipes = JSON.parse(
    readRepo("packages/design-instrument/package.json"),
  ).clay.contributions.uiDesignSystem.recipes as Record<
    string,
    Record<string, string | number>
  >;

  it("keeps the editor chrome free of blur and filters", () => {
    expect(editorCss).not.toMatch(/backdrop-filter/);
    expect(editorCss).not.toMatch(/\bblur\(/);
    expect(editorCss).not.toMatch(/\bfilter:/);
  });

  it("draws no radius on the canvas edge", () => {
    // The editor container is full-bleed: the package declares a 0 radius and
    // the host reads that value instead of hardcoding a literal.
    expect(recipes["editor.default.container.rest"]?.borderRadius).toBe(0);
    expect(editorCss).toContain(
      "border-radius: var(--clay-ds-editor-default-container-rest-border-radius)",
    );
    expect(editorCss).not.toMatch(/border-radius:\s*\d/);
  });

  it("keeps the document bar borderless with no line-number gutter", () => {
    // Full-bleed text: no gutter is painted, so the gutter recipe stays
    // declared but unconsumed while the chrome recipe declares
    // `borderStyle: none`.
    expect(recipes["editor.default.gutter.rest"]?.borderWidth).toBe(0);
    expect(recipes["editor.default.chrome.rest"]?.borderWidth).toBe(0);
    expect(editorCss).not.toContain("cm-gutters");
    expect(editorCss).not.toMatch(/\.chrome\s*{[^}]*border-bottom/);
  });

  it("separates package-workspace zones with the divider recipe only", () => {
    const hairlines = packageCss.match(
      /--clay-ds-divider-default-root-rest-border-color/g,
    );
    expect(hairlines?.length ?? 0).toBeGreaterThanOrEqual(5);
    // No zone paints its own colour for a boundary.
    expect(packageCss).not.toMatch(/border-(top|bottom|left|right):\s*\d/);
  });

  it("keeps the editor, package, agent and settings surfaces free of geometry literals", () => {
    for (const [name, css] of [
      ["editor.module.css", editorCss],
      ["package-workspace.module.css", packageCss],
      ["settings-panel.module.css", settingsCss],
      ["agent-settings.module.css", agentSettingsCss],
      ["coding-agent.module.css", agentCss],
    ] as const) {
      for (const match of css.matchAll(
        /^\s*(border[a-z-]*|box-shadow)\s*:\s*([^;]+);/gm,
      )) {
        const value = match[2] ?? "";
        expect(
          /\d+px/.test(value.replace(/--clay-[a-z-]+/g, "")),
          `${name}: \`${match[1]}: ${value.trim()}\` must not hardcode geometry`,
        ).toBe(false);
      }
    }
  });

  it("paints the Files tab's rows from the session-row family only (task 36)", () => {
    // The session files list is session history: the row recipe carries its
    // padding, radius, fill and focus ring, the mark is toned by the theme's
    // diagnostic roles, and no row paints a boundary of its own.
    for (const property of [
      "gap",
      "padding",
      "border-radius",
      "background-color",
      "text-color",
      "transition-duration",
      "transition-timing",
    ]) {
      expect(agentCss).toContain(
        `var(--clay-ds-session-row-default-root-rest-${property})`,
      );
    }
    expect(agentCss).toContain(
      "var(--clay-ds-session-row-default-root-hover-background-color)",
    );
    expect(agentCss).toContain(
      "var(--clay-ds-session-row-default-root-focus-outline-color)",
    );
    expect(agentCss).toContain("var(--clay-diagnostic-warning)");
    expect(agentCss).toContain("var(--clay-diagnostic-success)");
    expect(agentCss).toContain("var(--clay-diagnostic-error)");
    // The filter is the approved quiet field: the well keeps the boundary and
    // the accent halo, and the input inside draws no outline of its own.
    expect(agentCss).toMatch(
      /\.sessionFilterInput:focus,[\s\S]{0,120}outline:\s*none;/,
    );
    expect(agentCss).toContain(
      "var(--clay-ds-text-input-default-input-focus-shadow)",
    );
  });

  it("paints the settings panel as a fixed slot: no radius, no elevation", () => {
    // A fixed slot is not a floating surface (DESIGN.md §1/§11): the panel's
    // boundary is the slot's divider hairline, and it carries no shadow.
    expect(settingsCss).toMatch(/\.panel\s*\{[^}]*border-width:\s*0/s);
    expect(settingsCss).toMatch(/\.panel\s*\{[^}]*border-radius:\s*0/s);
    expect(settingsCss).not.toMatch(/box-shadow/);
    // The recipe's radius, hairline and opaque fill are spent where the panel
    // stops being a slot and becomes the narrow drawer.
    // Prettier reflows long `var()` arguments: compare on collapsed whitespace.
    const drawer = settingsCss
      .slice(settingsCss.indexOf("@media (max-width: 760px)"))
      .replace(/\s+/g, " ");
    expect(drawer).toMatch(
      /border-radius:\s*var\(\s*--clay-ds-settings-panel-default-panel-rest-border-radius\s*\)\s*0\s*0/,
    );
    expect(drawer).toMatch(
      /background:\s*var\(\s*--clay-ds-settings-panel-default-panel-rest-background-color\s*\)/,
    );
  });

  it("keeps the settings surfaces on their declared recipes", () => {
    // Slot chrome, rows, groups and actions all read the shipped recipes.
    for (const variable of [
      "--clay-ds-settings-panel-default-panel-rest-background-color",
      "--clay-ds-settings-panel-default-heading-rest-border-color",
      "--clay-ds-settings-panel-default-actions-rest-border-color",
    ]) {
      expect(settingsCss).toContain(variable);
    }
    // The agent Settings tab lists delivered files as list rows and states the
    // first-run case with the empty recipe.
    expect(agentSettingsCss).toContain(
      "--clay-ds-list-default-row-rest-border-radius",
    );
    expect(agentSettingsCss).toContain("--clay-ds-empty-default-root-rest-gap");
  });

  it("maps the SDUI scroll family in the host's scroll chrome", () => {
    const globalCss = readRepo("frontend/src/styles/global.css");
    expect(globalCss).toContain(
      "--clay-ds-scroll-default-scrollbar-thumb-rest-opacity",
    );
    expect(globalCss).toContain(
      "--clay-ds-scroll-default-scrollbar-thumb-hover-opacity",
    );
    expect(globalCss).toContain(
      "--clay-ds-scroll-default-scrollbar-thumb-active-opacity",
    );
    expect(globalCss).toContain(
      "--clay-ds-scroll-default-scrollbar-track-rest-background-color",
    );
    expect(globalCss).not.toMatch(/scrollbar[\s\S]{0,80}backdrop-filter/);
  });
});
