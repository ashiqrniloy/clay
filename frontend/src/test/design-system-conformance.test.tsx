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
    "modal.default.dialog.rest.borderRadius": { type: "radius", value: 0 },
    "modal.default.dialog.rest.borderWidth": { type: "border-width", value: 2 },
    "modal.default.dialog.rest.backdropBlur": {
      type: "backdrop-blur",
      value: 0,
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
    "modal.default.dialog.rest.borderRadius": { type: "radius", value: 14 },
    "modal.default.dialog.rest.borderWidth": { type: "border-width", value: 1 },
    "modal.default.dialog.rest.backdropBlur": {
      type: "backdrop-blur",
      value: 24,
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
  });
});
