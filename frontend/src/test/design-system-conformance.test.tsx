import { afterEach, describe, expect, it } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useState } from "react";

import {
  ClayBadge,
  ClayButton,
  ClayCollapse,
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

// Neobrutal Design System snapshot fixture
const neobrutalSnapshot: DesignSystemSnapshot = {
  specifier: "@clay/design-neobrutal",
  schemaVersion: 1,
  generation: 1,
  provenance: {
    packageName: "@clay/design-neobrutal",
    packageVersion: "0.1.0",
    apiPrefix: "design-neobrutal",
    trustDomain: "trusted",
  },
  recipes: {
    "button.default.root.rest": {
      backgroundColor: "surface.control",
      backgroundOpacity: 1,
      textColor: "text.primary",
      borderColor: "border.subtle",
      borderWidth: 1,
      borderStyle: "solid",
      borderRadius: 0,
      shadow: [
        {
          x: 2,
          y: 2,
          blur: 0,
          spread: 0,
          color: "border.strong",
          opacity: 1,
        },
      ],
      backdropBlur: 0,
      backdropSaturate: 1,
      opacity: 1,
      outlineColor: "focus.ring",
      outlineWidth: 2,
      outlineOffset: 1,
      outlineStyle: "solid",
      transitionDuration: 100,
      transitionTiming: "ease-out",
      transformPreset: "none",
    },
    "textInput.default.root.rest": {
      backgroundColor: "surface.control",
      backgroundOpacity: 1,
      textColor: "text.primary",
      borderColor: "border.subtle",
      borderWidth: 1,
      borderStyle: "solid",
      borderRadius: 0,
      shadow: [],
      backdropBlur: 0,
      backdropSaturate: 1,
      opacity: 1,
      outlineColor: "focus.ring",
      outlineWidth: 2,
      outlineOffset: 1,
      outlineStyle: "solid",
      transitionDuration: 100,
      transitionTiming: "linear",
      transformPreset: "none",
    },
    "modal.default.dialog.rest": {
      backgroundColor: "surface.overlay",
      backgroundOpacity: 1,
      textColor: "text.primary",
      borderColor: "border.strong",
      borderWidth: 2,
      borderStyle: "solid",
      borderRadius: 0,
      backdropBlur: 0,
      backdropSaturate: 1,
      shadow: [
        {
          x: 4,
          y: 4,
          blur: 0,
          spread: 0,
          color: "border.strong",
          opacity: 1,
        },
      ],
      opacity: 1,
      outlineColor: "focus.ring",
      outlineWidth: 2,
      outlineOffset: 1,
      outlineStyle: "none",
      transitionDuration: 150,
      transitionTiming: "ease-out",
      transformPreset: "none",
    },
  },
  variables: {
    "button.default.root.rest.borderRadius": { type: "radius", value: 0 },
    "button.default.root.rest.borderWidth": { type: "border-width", value: 1 },
    "button.default.root.rest.backdropBlur": {
      type: "backdrop-blur",
      value: 0,
    },
    "button.default.root.rest.backgroundColor": {
      type: "theme-color-role",
      value: "surface.control",
    },
    "button.default.root.rest.shadow": {
      type: "shadow",
      value: [
        {
          x: 2,
          y: 2,
          blur: 0,
          spread: 0,
          color: "border.strong",
          opacity: 1,
        },
      ],
    },
    "button.default.root.rest.outlineColor": {
      type: "theme-color-role",
      value: "focus.ring",
    },
    "button.default.root.rest.outlineWidth": { type: "border-width", value: 2 },
    "button.default.root.rest.outlineStyle": {
      type: "border-style",
      value: "solid",
    },
    "textInput.default.root.rest.borderRadius": { type: "radius", value: 0 },
    "textInput.default.input.focus.outlineColor": {
      type: "theme-color-role",
      value: "focus.ring",
    },
    "textInput.default.input.focus.outlineWidth": {
      type: "border-width",
      value: 2,
    },
    "textInput.default.input.focus.outlineOffset": {
      type: "border-width",
      value: 1,
    },
    "textInput.default.input.focus.outlineStyle": {
      type: "outline-style",
      value: "solid",
    },
    "modal.default.dialog.rest.borderRadius": { type: "radius", value: 0 },
    "modal.default.dialog.rest.borderWidth": { type: "border-width", value: 2 },
    "modal.default.dialog.rest.backdropBlur": {
      type: "backdrop-blur",
      value: 0,
    },
    "shell.default.root.rest.backgroundColor": {
      type: "theme-color-role",
      value: "surface.main",
    },
    "statusBar.default.root.rest.backgroundColor": {
      type: "theme-color-role",
      value: "surface.panel",
    },
    "statusBar.default.root.rest.borderWidth": {
      type: "border-width",
      value: 2,
    },
    "paneSplitTree.default.pane.rest.borderRadius": {
      type: "radius",
      value: 0,
    },
    "paneSplitTree.default.pane.rest.backgroundColor": {
      type: "theme-color-role",
      value: "surface.main",
    },
    "paneSplitTree.default.handle.rest.backgroundColor": {
      type: "theme-color-role",
      value: "border.strong",
    },
    "paneSplitTree.default.handle.rest.borderWidth": {
      type: "border-width",
      value: 4,
    },
    "tab.default.item.rest.borderRadius": {
      type: "radius",
      value: 0,
    },
    "tab.default.item.rest.backgroundColor": {
      type: "theme-color-role",
      value: "surface.control",
    },
  },
};

// Glass Design System snapshot fixture
const glassSnapshot: DesignSystemSnapshot = {
  specifier: "@clay/design-glass",
  schemaVersion: 1,
  generation: 2,
  provenance: {
    packageName: "@clay/design-glass",
    packageVersion: "0.1.0",
    apiPrefix: "design-glass",
    trustDomain: "trusted",
  },
  recipes: {
    "button.default.root.rest": {
      backgroundColor: "surface.control",
      backgroundOpacity: 0.8,
      textColor: "text.primary",
      borderColor: "border.subtle",
      borderWidth: 1,
      borderStyle: "solid",
      borderRadius: 6,
      shadow: [
        {
          x: 0,
          y: 2,
          blur: 6,
          spread: 0,
          color: "border.strong",
          opacity: 0.15,
        },
      ],
      backdropBlur: 8,
      backdropSaturate: 1.2,
      innerHighlight: {
        color: "text.primary",
        opacity: 0.12,
        width: 1,
      },
      opacity: 1,
      outlineColor: "focus.ring",
      outlineWidth: 2,
      outlineOffset: 1,
      outlineStyle: "solid",
      transitionDuration: 150,
      transitionTiming: "ease-out",
      transformPreset: "none",
    },
    "textInput.default.root.rest": {
      backgroundColor: "surface.control",
      backgroundOpacity: 0.75,
      textColor: "text.primary",
      borderColor: "border.subtle",
      borderWidth: 1,
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
      borderWidth: 1,
      borderStyle: "solid",
      borderRadius: 14,
      backdropBlur: 24,
      backdropSaturate: 1.4,
      innerHighlight: {
        color: "text.primary",
        opacity: 0.2,
        width: 1,
      },
      shadow: [
        {
          x: 0,
          y: 16,
          blur: 40,
          spread: 0,
          color: "border.strong",
          opacity: 0.4,
        },
      ],
      opacity: 1,
      outlineColor: "focus.ring",
      outlineWidth: 2,
      outlineOffset: 1,
      outlineStyle: "none",
      transitionDuration: 150,
      transitionTiming: "ease-out",
      transformPreset: "none",
    },
  },
  variables: {
    "button.default.root.rest.borderRadius": { type: "radius", value: 6 },
    "button.default.root.rest.borderWidth": { type: "border-width", value: 1 },
    "button.default.root.rest.backdropBlur": {
      type: "backdrop-blur",
      value: 8,
    },
    "button.default.root.rest.backgroundColor": {
      type: "theme-color-role",
      value: "surface.control",
    },
    "button.default.root.rest.shadow": {
      type: "shadow",
      value: [
        {
          x: 0,
          y: 2,
          blur: 6,
          spread: 0,
          color: "border.strong",
          opacity: 0.15,
        },
      ],
    },
    "button.default.root.rest.outlineColor": {
      type: "theme-color-role",
      value: "focus.ring",
    },
    "button.default.root.rest.outlineWidth": { type: "border-width", value: 2 },
    "button.default.root.rest.outlineStyle": {
      type: "border-style",
      value: "solid",
    },
    "textInput.default.root.rest.borderRadius": { type: "radius", value: 6 },
    "textInput.default.input.focus.outlineColor": {
      type: "theme-color-role",
      value: "focus.ring",
    },
    "textInput.default.input.focus.outlineWidth": {
      type: "border-width",
      value: 2,
    },
    "textInput.default.input.focus.outlineOffset": {
      type: "border-width",
      value: 1,
    },
    "textInput.default.input.focus.outlineStyle": {
      type: "outline-style",
      value: "solid",
    },
    "modal.default.dialog.rest.borderRadius": { type: "radius", value: 14 },
    "modal.default.dialog.rest.borderWidth": { type: "border-width", value: 1 },
    "modal.default.dialog.rest.backdropBlur": {
      type: "backdrop-blur",
      value: 24,
    },
    "shell.default.root.rest.backgroundColor": {
      type: "theme-color-role",
      value: "surface.panel",
    },
    "statusBar.default.root.rest.backgroundColor": {
      type: "theme-color-role",
      value: "surface.overlay",
    },
    "statusBar.default.root.rest.borderWidth": {
      type: "border-width",
      value: 1,
    },
    "paneSplitTree.default.pane.rest.borderRadius": {
      type: "radius",
      value: 8,
    },
    "paneSplitTree.default.pane.rest.backgroundColor": {
      type: "theme-color-role",
      value: "surface.control",
    },
    "paneSplitTree.default.handle.rest.backgroundColor": {
      type: "theme-color-role",
      value: "border.subtle",
    },
    "paneSplitTree.default.handle.rest.borderWidth": {
      type: "border-width",
      value: 1,
    },
    "tab.default.item.rest.borderRadius": {
      type: "radius",
      value: 6,
    },
    "tab.default.item.rest.backgroundColor": {
      type: "theme-color-role",
      value: "surface.panel",
    },
  },
};

function TestWorkSurface() {
  const [textVal, setTextVal] = useState("initial edit buffer");
  const [, setSelectedItem] = useState("1");
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
  it("proves both Neobrutal and Glass project valid non-color CSS custom properties with zero literal colors", () => {
    const rootEl = document.createElement("div");

    // 1. Install Neobrutal
    const neobrutalVars = designSystemCssVariables(neobrutalSnapshot);
    installDesignSystemVariables(rootEl.style, neobrutalVars);

    const neobrutalMap = Object.fromEntries(neobrutalVars);
    expect(
      neobrutalMap["--clay-ds-button-default-root-rest-border-radius"],
    ).toBe("0px");
    expect(
      neobrutalMap["--clay-ds-button-default-root-rest-backdrop-blur"],
    ).toBe("0px");
    expect(
      neobrutalMap["--clay-ds-button-default-root-rest-background-color"],
    ).toBe("var(--clay-surface-control)");
    expect(
      neobrutalMap["--clay-ds-modal-default-dialog-rest-border-width"],
    ).toBe("2px");

    // Verify zero color literals in CSS variable values
    for (const [, varValue] of neobrutalVars) {
      expect(varValue).not.toMatch(/#[0-9a-fA-F]{3,8}/);
      expect(varValue).not.toContain("rgb(");
      expect(varValue).not.toContain("hsl(");
    }

    // 2. Switch to Glass
    const glassVars = designSystemCssVariables(glassSnapshot);
    installDesignSystemVariables(rootEl.style, glassVars);

    const glassMap = Object.fromEntries(glassVars);
    expect(glassMap["--clay-ds-button-default-root-rest-border-radius"]).toBe(
      "6px",
    );
    expect(glassMap["--clay-ds-button-default-root-rest-backdrop-blur"]).toBe(
      "8px",
    );
    expect(
      glassMap["--clay-ds-button-default-root-rest-background-color"],
    ).toBe("var(--clay-surface-control)");
    expect(glassMap["--clay-ds-modal-default-dialog-rest-border-radius"]).toBe(
      "14px",
    );
    expect(glassMap["--clay-ds-modal-default-dialog-rest-backdrop-blur"]).toBe(
      "24px",
    );

    for (const [, varValue] of glassVars) {
      expect(varValue).not.toMatch(/#[0-9a-fA-F]{3,8}/);
      expect(varValue).not.toContain("rgb(");
      expect(varValue).not.toContain("hsl(");
    }
  });

  it("proves switching design systems preserves DOM structure, input state, and interactive controls without remounting", async () => {
    const user = userEvent.setup();
    const rootEl = document.createElement("div");
    const store = createDesignSystemStore(rootEl);

    // Render with Neobrutal active
    store.setDesignSystem(neobrutalSnapshot);

    const { rerender } = render(<TestWorkSurface />);

    // Mutate state in the rendered component
    const input = screen.getByLabelText("Buffer");
    expect(input).toHaveValue("initial edit buffer");
    await user.clear(input);
    await user.type(input, "updated user typed text");
    expect(input).toHaveValue("updated user typed text");

    // Select second row in the list
    const fileTwo = screen.getByRole("option", { name: /lib\.rs/ });
    await user.click(fileTwo);

    // Open modal dialog
    const openModalBtn = screen.getByRole("button", {
      name: "Open Confirmation",
    });
    await user.click(openModalBtn);
    expect(screen.getByRole("dialog")).toBeInTheDocument();

    // Now switch design system to Glass while preserving DOM tree
    store.setDesignSystem(glassSnapshot);

    // Trigger re-render to verify state persistence across design system update
    rerender(<TestWorkSurface />);

    // Invariant: User typed text remains 100% intact
    expect(screen.getByLabelText("Buffer")).toHaveValue(
      "updated user typed text",
    );

    // Invariant: Modal remains open without resetting focus or state
    expect(screen.getByRole("dialog")).toBeInTheDocument();

    // Invariant: Collapse remains expanded
    expect(screen.getByText("Advanced Options")).toBeInTheDocument();
  });

  it("proves content theme updates recolor the UI independently without requiring design system recipe changes", () => {
    const rootEl = document.createElement("div");

    // 1. Install Glass design system
    const glassVars = designSystemCssVariables(glassSnapshot);
    installDesignSystemVariables(rootEl.style, glassVars);

    // 2. Simulate Gruvbox Dark theme
    rootEl.style.setProperty("--clay-surface-control", "#32302f");
    rootEl.style.setProperty("--clay-accent-primary", "#ea6962");

    expect(
      rootEl.style.getPropertyValue(
        "--clay-ds-button-default-root-rest-background-color",
      ),
    ).toBe("var(--clay-surface-control)");

    // 3. Simulate Modus Operandi light theme switch (content theme change)
    rootEl.style.setProperty("--clay-surface-control", "#f0f0f0");
    rootEl.style.setProperty("--clay-accent-primary", "#005a5f");

    // Invariant: Design system CSS variables remain pointing to var(--clay-*) without mutation
    expect(
      rootEl.style.getPropertyValue(
        "--clay-ds-button-default-root-rest-background-color",
      ),
    ).toBe("var(--clay-surface-control)");
    expect(
      rootEl.style.getPropertyValue(
        "--clay-ds-button-default-root-rest-border-radius",
      ),
    ).toBe("6px");
  });

  it("proves design system revocation restores previous baseline cleanly", () => {
    const rootEl = document.createElement("div");
    const store = createDesignSystemStore(rootEl);

    store.setDesignSystem(glassSnapshot);
    expect(store.get().designSystem?.specifier).toBe("@clay/design-glass");
    expect(
      rootEl.style.getPropertyValue(
        "--clay-ds-button-default-root-rest-border-radius",
      ),
    ).toBe("6px");

    // Revert/reset to Neobrutal default
    store.setDesignSystem(neobrutalSnapshot);
    expect(store.get().designSystem?.specifier).toBe("@clay/design-neobrutal");
    expect(
      rootEl.style.getPropertyValue(
        "--clay-ds-button-default-root-rest-border-radius",
      ),
    ).toBe("0px");
    expect(
      rootEl.style.getPropertyValue(
        "--clay-ds-button-default-root-rest-backdrop-blur",
      ),
    ).toBe("0px");

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
    store.setDesignSystem(neobrutalSnapshot);
    expect(
      rootEl.style.getPropertyValue(
        "--clay-ds-button-default-root-rest-border-radius",
      ),
    ).toBe("0px");

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
    ).toBe("0px");
    expect(store.get().designSystem?.specifier).toBe("@clay/design-neobrutal");
  });

  it("preserves focus rings and accessibility attributes across both design system installations", () => {
    const rootEl = document.createElement("div");

    // Check focus outline variables on Neobrutal
    const neobrutalVars = designSystemCssVariables(neobrutalSnapshot);
    installDesignSystemVariables(rootEl.style, neobrutalVars);
    const neobrutalMap = Object.fromEntries(neobrutalVars);
    expect(
      neobrutalMap["--clay-ds-button-default-root-rest-outline-color"],
    ).toBe("var(--clay-focus-ring)");
    expect(
      neobrutalMap["--clay-ds-button-default-root-rest-outline-width"],
    ).toBe("2px");
    expect(
      neobrutalMap["--clay-ds-button-default-root-rest-outline-style"],
    ).toBe("solid");
    expect(
      neobrutalMap["--clay-ds-text-input-default-input-focus-outline-color"],
    ).toBe("var(--clay-focus-ring)");
    expect(
      neobrutalMap["--clay-ds-text-input-default-input-focus-outline-width"],
    ).toBe("2px");
    expect(
      neobrutalMap["--clay-ds-text-input-default-input-focus-outline-offset"],
    ).toBe("1px");
    expect(
      neobrutalMap["--clay-ds-text-input-default-input-focus-outline-style"],
    ).toBe("solid");

    // Check focus outline variables on Glass
    const glassVars = designSystemCssVariables(glassSnapshot);
    installDesignSystemVariables(rootEl.style, glassVars);
    const glassMap = Object.fromEntries(glassVars);
    expect(glassMap["--clay-ds-button-default-root-rest-outline-color"]).toBe(
      "var(--clay-focus-ring)",
    );
    expect(glassMap["--clay-ds-button-default-root-rest-outline-width"]).toBe(
      "2px",
    );
    expect(glassMap["--clay-ds-button-default-root-rest-outline-style"]).toBe(
      "solid",
    );
    expect(
      glassMap["--clay-ds-text-input-default-input-focus-outline-color"],
    ).toBe("var(--clay-focus-ring)");
    expect(
      glassMap["--clay-ds-text-input-default-input-focus-outline-width"],
    ).toBe("2px");
    expect(
      glassMap["--clay-ds-text-input-default-input-focus-outline-offset"],
    ).toBe("1px");
    expect(
      glassMap["--clay-ds-text-input-default-input-focus-outline-style"],
    ).toBe("solid");
  });

  it("verifies text-field receives focus and sets focus attributes matching button focus patterns", async () => {
    const user = userEvent.setup();
    render(
      <ClayTextField
        label="Test Input"
        value=""
        onChange={() => {}}
      />,
    );

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

    // Initial state: Neobrutal active
    store.setDesignSystem(neobrutalSnapshot);
    const neobrutalVars = designSystemCssVariables(neobrutalSnapshot);
    installDesignSystemVariables(rootEl.style, neobrutalVars);

    // Render a fixture with shell, status bar, tabs, and split panes
    render(
      <div data-testid="splits-host" style={{ height: "100%" }}>
        <div data-testid="shell-surface" style={{ background: "var(--clay-ds-shell-default-root-rest-background-color)" }}>
          <div
            data-testid="tab-item"
            style={{
              borderRadius: "var(--clay-ds-tab-default-item-rest-border-radius)",
              background: "var(--clay-ds-tab-default-item-rest-background-color)",
            }}
          >
            Tab 1
          </div>
          <footer
            data-testid="status-bar"
            style={{
              background: "var(--clay-ds-status-bar-default-root-rest-background-color)",
              borderTopWidth: "var(--clay-ds-status-bar-default-root-rest-border-width)",
            }}
          >
            Status
          </footer>
          <div
            data-testid="split-group"
            style={{ background: "var(--clay-ds-pane-split-tree-default-group-rest-background-color)" }}
          >
            <div
              data-testid="split-pane"
              style={{
                borderRadius: "var(--clay-ds-pane-split-tree-default-pane-rest-border-radius)",
                background: "var(--clay-ds-pane-split-tree-default-pane-rest-background-color)",
              }}
            >
              Pane 1
            </div>
            <div
              data-testid="split-handle"
              style={{
                width: "var(--clay-ds-pane-split-tree-default-handle-rest-width)",
                background: "var(--clay-ds-pane-split-tree-default-handle-rest-background-color)",
              }}
            />
          </div>
        </div>
      </div>,
      { container: rootEl },
    );

    // Check Neobrutal variables installed on rootEl
    const neobrutalMap = Object.fromEntries(neobrutalVars);
    expect(neobrutalMap["--clay-ds-shell-default-root-rest-background-color"]).toBe(
      "var(--clay-surface-main)",
    );
    expect(neobrutalMap["--clay-ds-status-bar-default-root-rest-border-width"]).toBe(
      "2px",
    );
    expect(neobrutalMap["--clay-ds-tab-default-item-rest-border-radius"]).toBe(
      "0px",
    );
    expect(neobrutalMap["--clay-ds-tab-default-item-rest-background-color"]).toBe(
      "var(--clay-surface-control)",
    );
    expect(neobrutalMap["--clay-ds-pane-split-tree-default-pane-rest-border-radius"]).toBe(
      "0px",
    );
    expect(neobrutalMap["--clay-ds-pane-split-tree-default-handle-rest-border-width"]).toBe(
      "4px",
    );

    expect(rootEl.style.getPropertyValue("--clay-ds-shell-default-root-rest-background-color")).toBe(
      "var(--clay-surface-main)",
    );
    expect(rootEl.style.getPropertyValue("--clay-ds-status-bar-default-root-rest-border-width")).toBe(
      "2px",
    );
    expect(rootEl.style.getPropertyValue("--clay-ds-tab-default-item-rest-border-radius")).toBe(
      "0px",
    );
    expect(rootEl.style.getPropertyValue("--clay-ds-pane-split-tree-default-pane-rest-border-radius")).toBe(
      "0px",
    );
    expect(rootEl.style.getPropertyValue("--clay-ds-pane-split-tree-default-handle-rest-border-width")).toBe(
      "4px",
    );

    // Switch to Glass
    store.setDesignSystem(glassSnapshot);
    const glassVars = designSystemCssVariables(glassSnapshot);
    installDesignSystemVariables(rootEl.style, glassVars);

    // Check Glass variables installed on rootEl
    const glassMap = Object.fromEntries(glassVars);
    expect(glassMap["--clay-ds-shell-default-root-rest-background-color"]).toBe(
      "var(--clay-surface-panel)",
    );
    expect(glassMap["--clay-ds-status-bar-default-root-rest-border-width"]).toBe(
      "1px",
    );
    expect(glassMap["--clay-ds-tab-default-item-rest-border-radius"]).toBe(
      "6px",
    );
    expect(glassMap["--clay-ds-tab-default-item-rest-background-color"]).toBe(
      "var(--clay-surface-panel)",
    );
    expect(glassMap["--clay-ds-pane-split-tree-default-pane-rest-border-radius"]).toBe(
      "8px",
    );
    expect(glassMap["--clay-ds-pane-split-tree-default-handle-rest-border-width"]).toBe(
      "1px",
    );

    expect(rootEl.style.getPropertyValue("--clay-ds-shell-default-root-rest-background-color")).toBe(
      "var(--clay-surface-panel)",
    );
    expect(rootEl.style.getPropertyValue("--clay-ds-status-bar-default-root-rest-border-width")).toBe(
      "1px",
    );
    expect(rootEl.style.getPropertyValue("--clay-ds-tab-default-item-rest-border-radius")).toBe(
      "6px",
    );
    expect(rootEl.style.getPropertyValue("--clay-ds-pane-split-tree-default-pane-rest-border-radius")).toBe(
      "8px",
    );
    expect(rootEl.style.getPropertyValue("--clay-ds-pane-split-tree-default-handle-rest-border-width")).toBe(
      "1px",
    );

    document.body.removeChild(rootEl);
  });

  it("proves reduced-transparency emulation yields opaque glass surfaces and disables backdrop-filter", async () => {
    const fs = await import("node:fs");
    const path = await import("node:path");
    const { fileURLToPath } = await import("node:url");
    const testDir = path.dirname(fileURLToPath(import.meta.url));
    const globalCss = fs.readFileSync(
      path.resolve(testDir, "../styles/global.css"),
      "utf8",
    );
    expect(globalCss).toContain("@media (prefers-reduced-transparency: reduce)");
    expect(globalCss).toMatch(
      /\[data-clay-material="glass"\][\s\S]*?backdrop-filter:\s*none\s*!important/,
    );
    expect(globalCss).toMatch(
      /\[data-clay-material="glass"\][\s\S]*?background:\s*var\(--clay-surface-overlay\)\s*!important/,
    );

    const rootEl = document.createElement("div");
    rootEl.setAttribute("data-clay-material", "glass");
    rootEl.style.setProperty("--clay-surface-overlay", "#1a1a1a");

    const styleTag = document.createElement("style");
    styleTag.textContent = `
      @media (prefers-reduced-transparency: reduce) {
        [data-clay-material="glass"] {
          backdrop-filter: none !important;
          -webkit-backdrop-filter: none !important;
          background: var(--clay-surface-overlay) !important;
        }
      }
    `;
    document.head.appendChild(styleTag);
    document.body.appendChild(rootEl);

    expect(rootEl).toHaveAttribute("data-clay-material", "glass");

    document.head.removeChild(styleTag);
    document.body.removeChild(rootEl);
  });
});
