import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { afterEach, describe, expect, it } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useState } from "react";

import {
  ClayBadge,
  ClayButton,
  ClayCollapse,
  ClayDropdown,
  ClayList,
  ClayModal,
  ClayText,
  ClayTextField,
} from "../components";
import { createDesignSystemStore } from "../state/design-system-store";
import {
  designSystemCssVariables,
  installDesignSystemVariables,
} from "../theme/design-system-adapter";
import type { DesignSystemSnapshot } from "../theme/types";

afterEach(cleanup);

// `@clay/design-instrument` (Quiet Instrument) snapshot fixture.
// Values mirror `packages/design-instrument/package.json`; the Rust conformance
// suite (`tests/package_ui_conformance.rs`) asserts them against the manifest, so
// this fixture only has to be faithful for the adapter/store invariants below.
const shippedSnapshot: DesignSystemSnapshot = {
  specifier: "@clay/design-instrument",
  schemaVersion: 1,
  generation: 1,
  provenance: {
    packageName: "@clay/design-instrument",
    packageVersion: "0.1.0",
    apiPrefix: "design-instrument",
    trustDomain: "trusted",
  },
  recipes: {
    "button.default.root.rest": {
      backgroundColor: "transparent",
      textColor: "text.primary",
      borderColor: "border.hairline",
      borderWidth: 1,
      borderStyle: "solid",
      borderRadius: 8,
      shadow: [],
      transitionDuration: 150,
      transitionTiming: "ease-out",
      transformPreset: "none",
    },
    "button.default.root.focus": {
      backgroundColor: "transparent",
      textColor: "text.primary",
      borderColor: "border.hairline",
      borderWidth: 1,
      borderStyle: "solid",
      borderRadius: 8,
      shadow: [],
      outlineColor: "focus.ring",
      outlineWidth: 2,
      outlineOffset: 2,
      outlineStyle: "solid",
      transitionDuration: 150,
      transitionTiming: "ease-out",
      transformPreset: "none",
    },
    "list.default.row.selected": {
      backgroundColor: "accent.primary",
      backgroundOpacity: 0.15,
      textColor: "text.primary",
      borderWidth: 0,
      borderRadius: 8,
      shadow: [],
      transitionDuration: 150,
      transitionTiming: "ease-out",
      transformPreset: "none",
    },
    "modal.default.dialog.rest": {
      backgroundColor: "surface.overlay",
      textColor: "text.primary",
      borderColor: "border.hairline",
      borderWidth: 1,
      borderStyle: "solid",
      borderRadius: 16,
      shadow: [
        {
          x: 0,
          y: 24,
          blur: 60,
          spread: -16,
          color: "text.primary",
          opacity: 0.42,
        },
        {
          x: 0,
          y: 2,
          blur: 10,
          spread: -4,
          color: "text.primary",
          opacity: 0.22,
        },
      ],
      backdropBlur: 0,
      backdropSaturate: 1,
      transitionDuration: 240,
      transitionTiming: "spring-snappy",
      transformPreset: "none",
    },
    "toast.default.root.rest": {
      backgroundColor: "surface.panel",
      backgroundOpacity: 0.88,
      textColor: "text.primary",
      borderColor: "border.hairline",
      borderWidth: 1,
      borderStyle: "solid",
      borderRadius: 9999,
      shadow: [],
      backdropBlur: 8,
      backdropSaturate: 1,
      transitionDuration: 240,
      transitionTiming: "spring-snappy",
      transformPreset: "none",
    },
  } as unknown as DesignSystemSnapshot["recipes"],
  variables: {
    "button.default.root.rest.backgroundColor": {
      type: "theme-color-role",
      value: "transparent",
    },
    "button.default.root.rest.borderColor": {
      type: "theme-color-role",
      value: "border.hairline",
    },
    "button.default.root.rest.borderRadius": { type: "radius", value: 8 },
    "button.default.root.rest.borderWidth": { type: "border-width", value: 1 },
    "button.default.root.rest.transitionDuration": {
      type: "motion-duration",
      value: 150,
    },
    "button.default.root.focus.outlineColor": {
      type: "theme-color-role",
      value: "focus.ring",
    },
    "button.default.root.focus.outlineWidth": {
      type: "border-width",
      value: 2,
    },
    "button.default.root.focus.outlineOffset": {
      type: "border-width",
      value: 2,
    },
    "button.default.root.focus.outlineStyle": {
      type: "outline-style",
      value: "solid",
    },
    "list.default.row.selected.backgroundColor": {
      type: "theme-color-role",
      value: "accent.primary",
    },
    "list.default.row.selected.backgroundOpacity": {
      type: "opacity",
      value: 0.15,
    },
    "list.default.row.selected.borderRadius": { type: "radius", value: 8 },
    "list.default.row.selected.borderWidth": {
      type: "border-width",
      value: 0,
    },
    "modal.default.dialog.rest.backgroundColor": {
      type: "theme-color-role",
      value: "surface.overlay",
    },
    "modal.default.dialog.rest.borderRadius": { type: "radius", value: 16 },
    "modal.default.dialog.rest.borderWidth": {
      type: "border-width",
      value: 1,
    },
    "modal.default.dialog.rest.shadow": {
      type: "shadow",
      value: [
        {
          x: 0,
          y: 24,
          blur: 60,
          spread: -16,
          color: "text.primary",
          opacity: 0.42,
        },
        {
          x: 0,
          y: 2,
          blur: 10,
          spread: -4,
          color: "text.primary",
          opacity: 0.22,
        },
      ],
    },
    "toast.default.root.rest.backdropBlur": {
      type: "backdrop-blur",
      value: 8,
    },
    "shell.default.root.rest.backgroundColor": {
      type: "theme-color-role",
      value: "surface.main",
    },
    "statusBar.default.root.rest.backgroundColor": {
      type: "theme-color-role",
      value: "transparent",
    },
    "statusBar.default.root.rest.borderWidth": {
      type: "border-width",
      value: 1,
    },
    "paneSplitTree.default.pane.rest.borderRadius": {
      type: "radius",
      value: 0,
    },
    "paneSplitTree.default.pane.rest.borderWidth": {
      type: "border-width",
      value: 0,
    },
    "paneSplitTree.default.handle.rest.backgroundColor": {
      type: "theme-color-role",
      value: "border.hairline",
    },
    "paneSplitTree.default.handle.rest.borderWidth": {
      type: "border-width",
      value: 0,
    },
    "tab.default.item.rest.borderRadius": { type: "radius", value: 9999 },
    "tab.default.item.rest.borderWidth": { type: "border-width", value: 0 },
    "tab.default.item.selected.backgroundColor": {
      type: "theme-color-role",
      value: "accent.primary",
    },
    "tab.default.item.selected.backgroundOpacity": {
      type: "opacity",
      value: 0.15,
    },
  },
};

// A synthetic third-party system: only the crate-internal invariants
// (replacement without remounting, revocation, stale-key removal, adapter
// rejection) are exercised against it, so it deliberately declares geometry the
// shipped package must never have. Plan 118 task 9: the removed Neobrutal and
// Glass systems no longer appear in any test.
const thirdPartySnapshot: DesignSystemSnapshot = {
  specifier: "@thirdparty/design-sample",
  schemaVersion: 1,
  generation: 2,
  provenance: {
    packageName: "@thirdparty/design-sample",
    packageVersion: "0.1.0",
    apiPrefix: "design-sample",
    trustDomain: "thirdParty",
  },
  recipes: {
    "button.default.root.rest": {
      backgroundColor: "surface.overlay",
      backgroundOpacity: 0.9,
      textColor: "text.primary",
      borderColor: "border.subtle",
      borderWidth: 2,
      borderStyle: "solid",
      borderRadius: 6,
      shadow: [],
      backdropBlur: 8,
      backdropSaturate: 1.2,
      opacity: 1,
      outlineColor: "focus.ring",
      outlineWidth: 2,
      outlineOffset: 1,
      outlineStyle: "solid",
      transitionDuration: 150,
      transitionTiming: "ease-out",
      transformPreset: "none",
    },
    "modal.default.dialog.rest": {
      backgroundColor: "surface.overlay",
      backgroundOpacity: 0.85,
      textColor: "text.primary",
      borderColor: "border.strong",
      borderWidth: 2,
      borderStyle: "solid",
      borderRadius: 14,
      backdropBlur: 24,
      backdropSaturate: 1.4,
      shadow: [],
      opacity: 1,
      outlineColor: "focus.ring",
      outlineWidth: 2,
      outlineOffset: 1,
      outlineStyle: "none",
      transitionDuration: 150,
      transitionTiming: "ease-out",
      transformPreset: "none",
    },
  } as unknown as DesignSystemSnapshot["recipes"],
  variables: {
    "button.default.root.rest.borderRadius": { type: "radius", value: 6 },
    "button.default.root.rest.borderWidth": { type: "border-width", value: 2 },
    "button.default.root.rest.backdropBlur": {
      type: "backdrop-blur",
      value: 8,
    },
    "button.default.root.rest.backgroundColor": {
      type: "theme-color-role",
      value: "surface.overlay",
    },
    "button.default.root.rest.outlineColor": {
      type: "theme-color-role",
      value: "focus.ring",
    },
    "button.default.root.rest.outlineWidth": {
      type: "border-width",
      value: 2,
    },
    "button.default.root.rest.outlineOffset": {
      type: "border-width",
      value: 1,
    },
    "button.default.root.rest.outlineStyle": {
      type: "outline-style",
      value: "solid",
    },
    "modal.default.dialog.rest.borderRadius": { type: "radius", value: 14 },
    "modal.default.dialog.rest.borderWidth": { type: "border-width", value: 2 },
    "modal.default.dialog.rest.backdropBlur": {
      type: "backdrop-blur",
      value: 24,
    },
    "shell.default.root.rest.backgroundColor": {
      type: "theme-color-role",
      value: "surface.panel",
    },
    "statusBar.default.root.rest.borderWidth": {
      type: "border-width",
      value: 2,
    },
    "paneSplitTree.default.pane.rest.borderRadius": {
      type: "radius",
      value: 8,
    },
    "paneSplitTree.default.handle.rest.borderWidth": {
      type: "border-width",
      value: 1,
    },
    "tab.default.item.rest.borderRadius": { type: "radius", value: 6 },
    "tab.default.item.rest.backgroundColor": {
      type: "theme-color-role",
      value: "surface.panel",
    },
  },
};

// Plan 118 task 21: the shipped choice set is the language plus the core
// baseline. `@clay/core` is "no adopted snapshot": the store clears every
// installed property and the host fallback block in `tokens.css` paints the
// same language, which is what the continuity and revocation invariants must
// hold across.
const SHIPPED_SYSTEMS = ["@clay/design-instrument", "@clay/core"] as const;
type ShippedSystem = (typeof SHIPPED_SYSTEMS)[number];

type DesignSystemStore = ReturnType<typeof createDesignSystemStore>;

/** The collapse header is the button the language puts `aria-expanded` on. */
function collapseHeader(): HTMLElement | null {
  return document.querySelector(
    '[data-clay-component="collapse"][data-clay-slot="header"]',
  );
}

function adoptSystem(store: DesignSystemStore, specifier: ShippedSystem): void {
  if (specifier === "@clay/core") {
    store.resetToFallback();
    return;
  }
  store.setDesignSystem(shippedSnapshot);
}

/** The shipped package's declared recipe values: the real contract the
 *  fallbacks must neutralize, not this file's fixture subset. */
function shippedRecipe(key: string): Record<string, unknown> {
  const repoRoot = path.resolve(
    path.dirname(fileURLToPath(import.meta.url)),
    "../../../",
  );
  const manifest = JSON.parse(
    fs.readFileSync(
      path.join(repoRoot, "packages/design-instrument/package.json"),
      "utf8",
    ),
  );
  const recipes = manifest?.clay?.contributions?.uiDesignSystem?.recipes ?? {};
  const recipe = recipes[key];
  if (!recipe) throw new Error(`manifest declares no recipe ${key}`);
  return recipe;
}

function TestWorkSurface() {
  const [textVal, setTextVal] = useState("initial edit buffer");
  const [selectedItem, setSelectedItem] = useState("1");
  const [modalOpen, setModalOpen] = useState(false);

  return (
    <div data-testid="workspace-root">
      <header data-testid="workspace-header">
        <ClayText variant="title">Clay Surface Test</ClayText>
        <ClayBadge>Online</ClayBadge>
      </header>
      <main data-testid="workspace-main">
        <ClayTextField
          label="Buffer"
          value={textVal}
          onChange={setTextVal}
          description="Live editing input"
        />
        <ClayList
          ariaLabel="Files"
          selectedId={selectedItem}
          items={[
            { id: "1", title: "main.rs" },
            { id: "2", title: "lib.rs" },
          ]}
          onSelect={setSelectedItem}
        />
        <ClayCollapse title="Advanced Options" defaultExpanded>
          <ClayButton variant="primary" onPress={() => setModalOpen(true)}>
            Open Confirmation
          </ClayButton>
        </ClayCollapse>
        <ClayModal
          title="Confirm Action"
          open={modalOpen}
          onClose={() => setModalOpen(false)}
        >
          <ClayText>Modal confirmation content</ClayText>
        </ClayModal>
      </main>
    </div>
  );
}

describe("UI Design System Conformance & Replacement Invariants", () => {
  it("proves the shipped system and a third-party system both project valid non-color CSS custom properties", () => {
    const rootEl = document.createElement("div");

    // 1. Install the shipped @clay/design-instrument snapshot (Quiet Instrument).
    const shippedVars = designSystemCssVariables(shippedSnapshot);
    installDesignSystemVariables(rootEl.style, shippedVars);

    const shippedMap = Object.fromEntries(shippedVars);
    // Radius ladder: 8px control, 16px surface; borders are hairlines.
    expect(shippedMap["--clay-ds-button-default-root-rest-border-radius"]).toBe(
      "8px",
    );
    expect(
      shippedMap["--clay-ds-modal-default-dialog-rest-border-radius"],
    ).toBe("16px");
    expect(shippedMap["--clay-ds-modal-default-dialog-rest-border-width"]).toBe(
      "1px",
    );
    // No blur anywhere except the scrim and the toast.
    expect(
      shippedMap["--clay-ds-button-default-root-rest-backdrop-blur"],
    ).toBeUndefined();
    expect(shippedMap["--clay-ds-toast-default-root-rest-backdrop-blur"]).toBe(
      "8px",
    );
    // Selection is a single accent fill signal, never an edge.
    expect(
      shippedMap["--clay-ds-list-default-row-selected-background-opacity"],
    ).toBe("0.15");
    expect(shippedMap["--clay-ds-list-default-row-selected-border-width"]).toBe(
      "0px",
    );
    // Focus rings sit at offset 2 and are host-owned.
    expect(
      shippedMap["--clay-ds-button-default-root-focus-outline-offset"],
    ).toBe("2px");
    expect(
      shippedMap["--clay-ds-button-default-root-focus-outline-color"],
    ).toBe("var(--clay-focus-ring)");

    // Verify zero color literals in CSS variable values
    for (const [, varValue] of shippedVars) {
      expect(varValue).not.toMatch(/#[0-9a-fA-F]{3,8}/);
      expect(varValue).not.toContain("rgb(");
      expect(varValue).not.toContain("hsl(");
    }

    // 2. Switch to a synthetic third-party system with deliberately different geometry.
    const thirdPartyVars = designSystemCssVariables(thirdPartySnapshot);
    installDesignSystemVariables(rootEl.style, thirdPartyVars);

    const thirdPartyMap = Object.fromEntries(thirdPartyVars);
    expect(
      thirdPartyMap["--clay-ds-button-default-root-rest-border-radius"],
    ).toBe("6px");
    expect(
      thirdPartyMap["--clay-ds-button-default-root-rest-backdrop-blur"],
    ).toBe("8px");
    expect(
      thirdPartyMap["--clay-ds-modal-default-dialog-rest-border-radius"],
    ).toBe("14px");

    for (const [, varValue] of thirdPartyVars) {
      expect(varValue).not.toMatch(/#[0-9a-fA-F]{3,8}/);
      expect(varValue).not.toContain("rgb(");
      expect(varValue).not.toContain("hsl(");
    }
  });

  it.each(SHIPPED_SYSTEMS)(
    "proves switching to %s preserves DOM structure, input state, focus, scroll and interactive controls without remounting",
    async (specifier) => {
      const user = userEvent.setup();
      const rootEl = document.createElement("div");
      const store = createDesignSystemStore(rootEl);

      // Render with the shipped language active.
      adoptSystem(store, "@clay/design-instrument");

      const { rerender } = render(<TestWorkSurface />);

      // Mutate state in the rendered component.
      const input = screen.getByLabelText("Buffer");
      expect(input).toHaveValue("initial edit buffer");
      await user.clear(input);
      await user.type(input, "updated user typed text");
      expect(input).toHaveValue("updated user typed text");
      await user.click(input);
      expect(input).toHaveFocus();

      // Select second row in the list.
      const fileTwo = screen.getByRole("option", { name: /lib\.rs/ });
      await user.click(fileTwo);

      // Open modal dialog.
      const openModalBtn = screen.getByRole("button", {
        name: "Open Confirmation",
      });
      await user.click(openModalBtn);
      const dialog = screen.getByRole("dialog");

      // A scroll offset that must survive the swap.
      const scroller = screen.getByTestId("workspace-main");
      scroller.style.overflow = "auto";
      scroller.scrollTop = 48;

      // Snapshot the DOM identity and the accessibility contract.
      const focused = document.activeElement;
      const before = {
        input,
        dialog,
        scroller,
        fileTwo,
        selected: fileTwo.getAttribute("aria-selected"),
        expanded: collapseHeader()?.getAttribute("aria-expanded") ?? null,
        badge: screen.getByText("Online"),
      };
      expect(before.selected).toBe("true");

      // Switch to the other shipped choice while keeping the DOM tree.
      adoptSystem(store, specifier);
      rerender(<TestWorkSurface />);

      // Invariant: the same nodes, not re-created equivalents.
      expect(screen.getByLabelText("Buffer")).toBe(before.input);
      expect(screen.getByRole("dialog")).toBe(before.dialog);
      expect(screen.getByTestId("workspace-main")).toBe(before.scroller);
      expect(screen.getByText("Online")).toBe(before.badge);

      // Invariant: typed text, focus, scroll and ARIA state are intact.
      expect(before.input).toHaveValue("updated user typed text");
      expect(document.activeElement).toBe(focused);
      expect(before.scroller.scrollTop).toBe(48);
      // The open modal hides the rest of the tree from the accessibility API,
      // so the selected row is asserted on the captured node.
      expect(before.fileTwo.getAttribute("aria-selected")).toBe(
        before.selected,
      );
      expect(collapseHeader()?.getAttribute("aria-expanded")).toBe(
        before.expanded,
      );
      // Interactive controls stay operable: the modal still closes.
      await user.click(screen.getByRole("button", { name: /Close/i }));
      expect(screen.queryByRole("dialog")).toBeNull();
    },
  );

  it("proves content theme updates recolor the UI independently without requiring design system recipe changes", () => {
    const rootEl = document.createElement("div");

    // 1. Install the shipped design system
    const shippedVars = designSystemCssVariables(shippedSnapshot);
    installDesignSystemVariables(rootEl.style, shippedVars);

    // 2. Simulate Gruvbox Dark theme
    rootEl.style.setProperty("--clay-surface-overlay", "#32302f");
    rootEl.style.setProperty("--clay-accent-primary", "#ea6962");

    expect(
      rootEl.style.getPropertyValue(
        "--clay-ds-modal-default-dialog-rest-background-color",
      ),
    ).toBe("var(--clay-surface-overlay)");

    // 3. Simulate Modus Operandi light theme switch (content theme change)
    rootEl.style.setProperty("--clay-surface-overlay", "#f0f0f0");
    rootEl.style.setProperty("--clay-accent-primary", "#005a5f");

    // Invariant: Design system CSS variables remain pointing to var(--clay-*) without mutation
    expect(
      rootEl.style.getPropertyValue(
        "--clay-ds-modal-default-dialog-rest-background-color",
      ),
    ).toBe("var(--clay-surface-overlay)");
    // Geometry comes from the design system, not from the theme: still 8px.
    expect(
      rootEl.style.getPropertyValue(
        "--clay-ds-button-default-root-rest-border-radius",
      ),
    ).toBe("8px");
  });

  it("proves design system revocation restores previous baseline cleanly", () => {
    const rootEl = document.createElement("div");
    const store = createDesignSystemStore(rootEl);

    store.setDesignSystem(thirdPartySnapshot);
    expect(store.get().designSystem?.specifier).toBe(
      "@thirdparty/design-sample",
    );
    expect(
      rootEl.style.getPropertyValue(
        "--clay-ds-button-default-root-rest-border-radius",
      ),
    ).toBe("6px");

    // Revert to the shipped system
    store.setDesignSystem(shippedSnapshot);
    expect(store.get().designSystem?.specifier).toBe("@clay/design-instrument");
    expect(
      rootEl.style.getPropertyValue(
        "--clay-ds-button-default-root-rest-border-radius",
      ),
    ).toBe("8px");
    expect(
      rootEl.style.getPropertyValue(
        "--clay-ds-button-default-root-rest-backdrop-blur",
      ),
    ).toBe("");

    // Reset to fallback
    store.resetToFallback();
    expect(store.get().designSystem).toBeNull();
    expect(
      rootEl.style.getPropertyValue(
        "--clay-ds-button-default-root-rest-border-radius",
      ),
    ).toBe("");
  });

  it("rejects malicious or out-of-bounds snapshot variables at the adapter boundary before modifying the DOM", () => {
    const rootEl = document.createElement("div");
    const store = createDesignSystemStore(rootEl);

    // Initial valid state
    store.setDesignSystem(shippedSnapshot);
    expect(
      rootEl.style.getPropertyValue(
        "--clay-ds-button-default-root-rest-border-radius",
      ),
    ).toBe("8px");

    // Invalid snapshot with unmapped color literal
    const maliciousSnapshot: DesignSystemSnapshot = {
      specifier: "@thirdparty/design-malicious",
      schemaVersion: 1,
      generation: 99,
      provenance: {
        packageName: "@thirdparty/design-malicious",
        packageVersion: "1.0.0",
        apiPrefix: "design-malicious",
        trustDomain: "thirdParty",
      },
      recipes: {},
      variables: {
        "button.default.root.rest.backgroundColor": {
          type: "theme-color-role",
          value: "javascript:alert(1)",
        },
      },
    };

    expect(() => store.setDesignSystem(maliciousSnapshot)).toThrowError(
      /Invalid design system variable value/,
    );

    // Invariant: DOM is untouched after throwing
    expect(
      rootEl.style.getPropertyValue(
        "--clay-ds-button-default-root-rest-border-radius",
      ),
    ).toBe("8px");
    expect(store.get().designSystem?.specifier).toBe("@clay/design-instrument");
  });

  it("preserves focus rings at offset 2 and host-owned focus roles on the shipped system", () => {
    const rootEl = document.createElement("div");

    // The shipped system draws one ring per surface: a 2px host-owned ring at
    // offset 2 on the focused element itself (`button.default.root.focus`).
    const shippedVars = designSystemCssVariables(shippedSnapshot);
    installDesignSystemVariables(rootEl.style, shippedVars);
    const shippedMap = Object.fromEntries(shippedVars);
    expect(
      shippedMap["--clay-ds-button-default-root-focus-outline-color"],
    ).toBe("var(--clay-focus-ring)");
    expect(
      shippedMap["--clay-ds-button-default-root-focus-outline-width"],
    ).toBe("2px");
    expect(
      shippedMap["--clay-ds-button-default-root-focus-outline-offset"],
    ).toBe("2px");
    expect(
      shippedMap["--clay-ds-button-default-root-focus-outline-style"],
    ).toBe("solid");

    // Rest state declares no ring — focus is not painted into every element.
    expect(
      shippedMap["--clay-ds-button-default-root-rest-outline-color"],
    ).toBeUndefined();
    // A text input's focus is a border plus a soft halo, never a second ring.
    expect(
      shippedMap["--clay-ds-text-input-default-input-focus-outline-color"],
    ).toBeUndefined();

    // Reinstall over an existing system: the ring survives a swap intact.
    installDesignSystemVariables(
      rootEl.style,
      designSystemCssVariables(thirdPartySnapshot),
    );
    installDesignSystemVariables(rootEl.style, shippedVars);
    expect(
      rootEl.style.getPropertyValue(
        "--clay-ds-button-default-root-focus-outline-offset",
      ),
    ).toBe("2px");
  });

  it("verifies text-field receives focus and sets focus attributes matching button focus patterns", async () => {
    const user = userEvent.setup();
    render(<ClayTextField label="Test Input" value="" onChange={() => {}} />);

    const input = screen.getByLabelText("Test Input");
    expect(input).not.toHaveFocus();
    await user.click(input);
    expect(input).toHaveFocus();
    expect(input).toHaveAttribute("data-focused", "true");
  });

  it("verifies switching design systems dynamically updates shell, status bar, and pane computed styles while splits fixture is mounted", () => {
    const rootEl = document.createElement("div");
    document.body.appendChild(rootEl);
    const store = createDesignSystemStore(rootEl);

    // Initial state: the shipped system is active
    store.setDesignSystem(shippedSnapshot);
    const shippedVars = designSystemCssVariables(shippedSnapshot);
    installDesignSystemVariables(rootEl.style, shippedVars);

    // Render a fixture with shell, status bar, tabs, and split panes
    render(
      <div data-testid="splits-host" style={{ height: "100%" }}>
        <div
          data-testid="shell-surface"
          style={{
            background:
              "var(--clay-ds-shell-default-root-rest-background-color)",
          }}
        >
          <div
            data-testid="tab-item"
            style={{
              borderRadius:
                "var(--clay-ds-tab-default-item-rest-border-radius)",
              background:
                "var(--clay-ds-tab-default-item-rest-background-color)",
            }}
          >
            Tab 1
          </div>
          <footer
            data-testid="status-bar"
            style={{
              background:
                "var(--clay-ds-status-bar-default-root-rest-background-color)",
              borderTopWidth:
                "var(--clay-ds-status-bar-default-root-rest-border-width)",
            }}
          >
            Status
          </footer>
          <div
            data-testid="split-group"
            style={{
              background:
                "var(--clay-ds-pane-split-tree-default-group-rest-background-color)",
            }}
          >
            <div
              data-testid="split-pane"
              style={{
                borderRadius:
                  "var(--clay-ds-pane-split-tree-default-pane-rest-border-radius)",
                background:
                  "var(--clay-ds-pane-split-tree-default-pane-rest-background-color)",
              }}
            >
              Pane 1
            </div>
            <div
              data-testid="split-handle"
              style={{
                width:
                  "var(--clay-ds-pane-split-tree-default-handle-rest-width)",
                background:
                  "var(--clay-ds-pane-split-tree-default-handle-rest-background-color)",
              }}
            />
          </div>
        </div>
      </div>,
      { container: rootEl },
    );

    // Check the shipped variables installed on rootEl: canvas chrome, hairline
    // status bar, pill tabs, edgeless panes.
    const shippedMap = Object.fromEntries(shippedVars);
    expect(
      shippedMap["--clay-ds-shell-default-root-rest-background-color"],
    ).toBe("var(--clay-surface-main)");
    expect(
      shippedMap["--clay-ds-status-bar-default-root-rest-border-width"],
    ).toBe("1px");
    expect(shippedMap["--clay-ds-tab-default-item-rest-border-radius"]).toBe(
      "9999px",
    );
    expect(
      shippedMap["--clay-ds-tab-default-item-rest-background-color"],
    ).toBeUndefined();
    expect(
      shippedMap["--clay-ds-pane-split-tree-default-pane-rest-border-radius"],
    ).toBe("0px");
    expect(
      shippedMap["--clay-ds-pane-split-tree-default-handle-rest-border-width"],
    ).toBe("0px");

    expect(
      rootEl.style.getPropertyValue(
        "--clay-ds-shell-default-root-rest-background-color",
      ),
    ).toBe("var(--clay-surface-main)");
    expect(
      rootEl.style.getPropertyValue(
        "--clay-ds-status-bar-default-root-rest-border-width",
      ),
    ).toBe("1px");
    expect(
      rootEl.style.getPropertyValue(
        "--clay-ds-tab-default-item-rest-border-radius",
      ),
    ).toBe("9999px");
    expect(
      rootEl.style.getPropertyValue(
        "--clay-ds-pane-split-tree-default-pane-rest-border-radius",
      ),
    ).toBe("0px");
    expect(
      rootEl.style.getPropertyValue(
        "--clay-ds-pane-split-tree-default-handle-rest-border-width",
      ),
    ).toBe("0px");

    // Switch to the third-party system: identical DOM, different recipes.
    store.setDesignSystem(thirdPartySnapshot);
    const thirdPartyVars = designSystemCssVariables(thirdPartySnapshot);
    installDesignSystemVariables(rootEl.style, thirdPartyVars);

    const thirdPartyMap = Object.fromEntries(thirdPartyVars);
    expect(
      thirdPartyMap["--clay-ds-shell-default-root-rest-background-color"],
    ).toBe("var(--clay-surface-panel)");
    expect(
      thirdPartyMap["--clay-ds-status-bar-default-root-rest-border-width"],
    ).toBe("2px");
    expect(thirdPartyMap["--clay-ds-tab-default-item-rest-border-radius"]).toBe(
      "6px",
    );
    expect(
      thirdPartyMap["--clay-ds-tab-default-item-rest-background-color"],
    ).toBe("var(--clay-surface-panel)");
    expect(
      thirdPartyMap[
        "--clay-ds-pane-split-tree-default-pane-rest-border-radius"
      ],
    ).toBe("8px");
    expect(
      thirdPartyMap[
        "--clay-ds-pane-split-tree-default-handle-rest-border-width"
      ],
    ).toBe("1px");

    expect(
      rootEl.style.getPropertyValue(
        "--clay-ds-shell-default-root-rest-background-color",
      ),
    ).toBe("var(--clay-surface-panel)");
    expect(
      rootEl.style.getPropertyValue(
        "--clay-ds-status-bar-default-root-rest-border-width",
      ),
    ).toBe("2px");
    expect(
      rootEl.style.getPropertyValue(
        "--clay-ds-tab-default-item-rest-border-radius",
      ),
    ).toBe("6px");
    expect(
      rootEl.style.getPropertyValue(
        "--clay-ds-pane-split-tree-default-pane-rest-border-radius",
      ),
    ).toBe("8px");
    expect(
      rootEl.style.getPropertyValue(
        "--clay-ds-pane-split-tree-default-handle-rest-border-width",
      ),
    ).toBe("1px");

    document.body.removeChild(rootEl);
  });

  it("collapses motion to instant, transform-free state changes under prefers-reduced-motion", () => {
    const testDir = path.dirname(fileURLToPath(import.meta.url));
    const globalCss = fs.readFileSync(
      path.resolve(testDir, "../styles/global.css"),
      "utf8",
    );

    // The language animates (150ms states, 240ms entering surfaces, 620ms
    // focus flash), so the reduced-motion fallback has real work to do.
    expect(shippedRecipe("button.default.root.rest").transitionDuration).toBe(
      150,
    );
    expect(shippedRecipe("modal.default.dialog.rest").transitionDuration).toBe(
      240,
    );

    // §13.6: instant state changes, no transforms, no pulse.
    const reducedMotion = globalCss.match(
      /@media \(prefers-reduced-motion: reduce\) \{[\s\S]*?\n\}/,
    )?.[0];
    expect(
      reducedMotion,
      "global.css has no reduced-motion layer",
    ).toBeTruthy();
    expect(reducedMotion).toContain("transition-duration: 0.01ms !important");
    expect(reducedMotion).toContain("animation-duration: 0.01ms !important");
    expect(reducedMotion).toContain("animation-iteration-count: 1 !important");
    expect(reducedMotion).toContain("transform: none !important");
  });

  it("resolves every veil to an opaque fill and drops backdrop blur under prefers-reduced-transparency", async () => {
    const testDir = path.dirname(fileURLToPath(import.meta.url));
    const globalCss = fs.readFileSync(
      path.resolve(testDir, "../styles/global.css"),
      "utf8",
    );

    // The translucent surfaces are the language's veil family: the scrim (blur
    // 3), the toast (blur 8) and any fill carrying an opacity below 1 (§6).
    expect(shippedRecipe("modal.default.scrim.rest").backgroundOpacity).toBe(
      0.5,
    );
    expect(shippedRecipe("modal.default.scrim.rest").backdropBlur).toBe(3);
    expect(shippedRecipe("toast.default.root.rest").backdropBlur).toBe(8);

    // The rule must target surfaces that exist in the DOM, by the attributes the
    // components actually emit. Read them off a rendered modal and an opened
    // dropdown instead of trusting the selector text.
    const user = userEvent.setup();
    const emitted = new Set<string>();
    const collect = () => {
      for (const el of document.querySelectorAll(
        "[data-clay-component][data-clay-slot]",
      )) {
        emitted.add(
          `[data-clay-component="${el.getAttribute("data-clay-component")}"][data-clay-slot="${el.getAttribute("data-clay-slot")}"]`,
        );
      }
    };

    const dropdown = render(
      <ClayDropdown
        label="Veil menu"
        options={[{ id: "1", label: "One" }]}
        selectedId={null}
        onSelect={() => {}}
      />,
    );
    collect();
    const trigger = document.querySelector(
      '[data-clay-component="dropdown"][data-clay-slot="trigger"]',
    );
    expect(trigger).not.toBeNull();
    await user.click(trigger as HTMLElement);
    collect();
    dropdown.unmount();

    const modal = render(
      <ClayModal title="Veil probe" open onClose={() => {}}>
        <ClayText>Scrim probe</ClayText>
      </ClayModal>,
    );
    collect();
    modal.unmount();
    expect(emitted).toContain(
      '[data-clay-component="modal"][data-clay-slot="scrim"]',
    );
    expect(emitted).toContain(
      '[data-clay-component="dropdown"][data-clay-slot="popover"]',
    );

    const veilRule = globalCss.match(
      /@media \(prefers-reduced-transparency: reduce\) \{[\s\S]*?\n\}/,
    )?.[0];
    expect(
      veilRule,
      "global.css has no reduced-transparency layer",
    ).toBeTruthy();
    // Coverage: the scrim and the popover are veils and must be covered.
    expect(veilRule).toContain(
      '[data-clay-component="modal"][data-clay-slot="scrim"]',
    );
    expect(veilRule).toContain(
      '[data-clay-component="dropdown"][data-clay-slot="popover"]',
    );
    // No dead selectors: every component/slot pair the rule names is rendered by
    // a real component, so the fallback cannot rot into prose.
    const rulePairs = [
      ...(veilRule ?? "").matchAll(
        /\[data-clay-component="([a-zA-Z]+)"\]\[data-clay-slot="([a-zA-Z]+)"\]/g,
      ),
    ].map((m) => `[data-clay-component="${m[1]}"][data-clay-slot="${m[2]}"]`);
    expect(rulePairs.length).toBeGreaterThanOrEqual(2);
    expect(rulePairs.filter((pair) => !emitted.has(pair))).toEqual([]);
    // §13.7: opaque fill + no blur — never a translucent `color-mix` fill.
    expect(veilRule).toContain("backdrop-filter: none !important");
    expect(veilRule).toMatch(
      /background: var\(--clay-surface-\w+\) !important/,
    );
    expect(veilRule).not.toContain("color-mix");

    // Retired vocabulary: the removed glass material is not a selector anywhere.
    expect(globalCss).not.toContain("data-clay-material");
  });
});
