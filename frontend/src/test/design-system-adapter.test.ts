import { describe, expect, it, vi } from "vitest";

import { createDesignSystemStore } from "../state/design-system-store";
import {
  designSystemCssVariables,
  formatColorRole,
  formatInnerHighlight,
  formatShadow,
  formatTransformPreset,
  formatTransitionTiming,
  installDesignSystemVariables,
  recipeVariableToCssName,
  variableToCssValue,
} from "../theme/design-system-adapter";
import type { DesignSystemSnapshot } from "../theme/types";

const sampleSnapshot: DesignSystemSnapshot = {
  specifier: "@thirdparty/design-sample",
  schemaVersion: 1,
  generation: 4,
  provenance: {
    packageName: "@thirdparty/design-sample",
    packageVersion: "0.1.0",
    apiPrefix: "design-sample",
    trustDomain: "thirdParty",
  },
  recipes: {
    "button.primary.root.rest": {
      backgroundColor: "accent.primary",
      backgroundOpacity: 1,
      textColor: "text.primary",
      borderColor: "border.subtle",
      borderWidth: 1,
      borderStyle: "solid",
      borderRadius: 8,
      padding: null,
      gap: null,
      shadow: [
        {
          x: 0,
          y: 4,
          blur: 12,
          spread: 0,
          color: "surface.scrim",
          opacity: 0.25,
        },
      ],
      backdropBlur: 16,
      backdropSaturate: 1.4,
      innerHighlight: {
        color: "surface.overlay",
        opacity: 0.4,
        width: 1,
      },
      opacity: 1,
      outlineColor: "focus.ring",
      outlineWidth: 2,
      outlineOffset: 2,
      outlineStyle: "solid",
      transitionDuration: 150,
      transitionTiming: "ease-out",
      transformPreset: "hover-lift",
    },
  },
  variables: {
    "button.primary.root.rest.backgroundColor": {
      type: "theme-color-role",
      value: "accent.primary",
    },
    "button.primary.root.rest.backgroundOpacity": {
      type: "opacity",
      value: 1,
    },
    "button.primary.root.rest.borderRadius": {
      type: "radius",
      value: 8,
    },
    "button.primary.root.rest.borderWidth": {
      type: "border-width",
      value: 1,
    },
    "button.primary.root.rest.borderStyle": {
      type: "border-style",
      value: "solid",
    },
    "button.primary.root.rest.backdropBlur": {
      type: "backdrop-blur",
      value: 16,
    },
    "button.primary.root.rest.backdropSaturate": {
      type: "backdrop-saturate",
      value: 1.4,
    },
    "button.primary.root.rest.shadow": {
      type: "shadow",
      value: [
        {
          x: 0,
          y: 4,
          blur: 12,
          spread: 0,
          color: "surface.scrim",
          opacity: 0.25,
        },
      ],
    },
    "button.primary.root.rest.innerHighlight": {
      type: "inner-highlight",
      value: {
        color: "surface.overlay",
        opacity: 0.4,
        width: 1,
      },
    },
    "button.primary.root.rest.outlineStyle": {
      type: "outline-style",
      value: "solid",
    },
    "button.primary.root.rest.transitionDuration": {
      type: "motion-duration",
      value: 150,
    },
    "button.primary.root.rest.transitionTiming": {
      type: "transition-timing",
      value: "ease-out",
    },
    "button.primary.root.rest.transformPreset": {
      type: "transform-preset",
      value: "hover-lift",
    },
  },
};

describe("design system adapter: naming & formatting", () => {
  it("converts recipe variable keys deterministically to CSS custom property names", () => {
    expect(
      recipeVariableToCssName("button.primary.root.rest.backgroundColor"),
    ).toBe("--clay-ds-button-primary-root-rest-background-color");
    expect(
      recipeVariableToCssName("button.primary.root.rest.borderRadius"),
    ).toBe("--clay-ds-button-primary-root-rest-border-radius");
    expect(
      recipeVariableToCssName("modal.default.scrim.rest.backdropBlur"),
    ).toBe("--clay-ds-modal-default-scrim-rest-backdrop-blur");
    expect(
      recipeVariableToCssName("panel.default.root.rest.transitionDuration"),
    ).toBe("--clay-ds-panel-default-root-rest-transition-duration");
  });

  it("resolves theme color roles into host CSS variable references", () => {
    expect(formatColorRole("surface.control")).toBe(
      "var(--clay-surface-control)",
    );
    expect(formatColorRole("accent.primary")).toBe(
      "var(--clay-accent-primary)",
    );
    expect(formatColorRole("transparent")).toBe("transparent");
  });

  it("strictly rejects literal colors and invalid color roles", () => {
    expect(formatColorRole("#ff0000")).toBeNull();
    expect(formatColorRole("rgb(255, 0, 0)")).toBeNull();
    expect(formatColorRole("hsl(120, 100%, 50%)")).toBeNull();
    expect(formatColorRole("red")).toBeNull();
    expect(formatColorRole("unknown.custom.role")).toBeNull();
  });

  it("formats shadow stacks with color-mix for opacities", () => {
    expect(formatShadow([])).toBe("none");
    const shadow = formatShadow([
      {
        x: 0,
        y: 8,
        blur: 16,
        spread: 0,
        color: "surface.scrim",
        opacity: 0.35,
      },
    ]);
    expect(shadow).toBe(
      "0px 8px 16px 0px color-mix(in srgb, var(--clay-surface-scrim) 35%, transparent)",
    );
  });

  it("formats inner highlight rims with inset shadows", () => {
    const highlight = formatInnerHighlight({
      color: "surface.overlay",
      opacity: 0.5,
      width: 1,
    });
    expect(highlight).toBe(
      "inset 0 1px 0 0 color-mix(in srgb, var(--clay-surface-overlay) 50%, transparent)",
    );
  });

  it("formats transition timings and transform presets", () => {
    expect(formatTransitionTiming("ease-out")).toBe(
      "cubic-bezier(0, 0, 0.2, 1)",
    );
    expect(formatTransitionTiming("spring-snappy")).toBe(
      "cubic-bezier(0.2, 0.8, 0.2, 1)",
    );
    expect(formatTransitionTiming("invalid")).toBeNull();

    expect(formatTransformPreset("none")).toBe("none");
    expect(formatTransformPreset("press-subtle")).toBe("scale(0.98)");
    expect(formatTransformPreset("press-shift-down")).toBe("translateY(1px)");
    expect(formatTransformPreset("hover-lift")).toBe("translateY(-1px)");
    expect(formatTransformPreset("rotate(45deg)")).toBeNull();
  });

  it("validates and clamps scalar variable values", () => {
    expect(variableToCssValue({ type: "radius", value: 12 })).toBe("12px");
    expect(variableToCssValue({ type: "radius", value: -1 })).toBeNull();
    expect(variableToCssValue({ type: "radius", value: NaN })).toBeNull();

    expect(variableToCssValue({ type: "border-width", value: 2 })).toBe("2px");
    expect(variableToCssValue({ type: "border-width", value: 10 })).toBeNull();

    expect(variableToCssValue({ type: "opacity", value: 0.8 })).toBe("0.8");
    expect(variableToCssValue({ type: "opacity", value: 1.5 })).toBeNull();

    expect(variableToCssValue({ type: "backdrop-blur", value: 16 })).toBe(
      "16px",
    );
    expect(variableToCssValue({ type: "backdrop-blur", value: 64 })).toBeNull();

    expect(variableToCssValue({ type: "backdrop-saturate", value: 1.5 })).toBe(
      "1.5",
    );
    expect(
      variableToCssValue({ type: "backdrop-saturate", value: 0.5 }),
    ).toBeNull();

    expect(
      variableToCssValue({ type: "spacing-token", value: "spacing.md" }),
    ).toBe("var(--clay-spacing-md)");
    expect(
      variableToCssValue({ type: "spacing-token", value: "random-value" }),
    ).toBeNull();
  });
});

function createMockStyle(): {
  style: CSSStyleDeclaration;
  properties: Map<string, string>;
} {
  const properties = new Map<string, string>();
  const style = {
    setProperty: vi.fn((name: string, value: string) => {
      properties.set(name, value);
    }),
    removeProperty: vi.fn((name: string) => {
      properties.delete(name);
    }),
    getPropertyValue: (name: string) => properties.get(name) ?? "",
  } as unknown as CSSStyleDeclaration;
  return { style, properties };
}

describe("design system adapter: snapshot conversion", () => {
  it("converts a valid snapshot into sorted CSS variable tuples", () => {
    const vars = designSystemCssVariables(sampleSnapshot);
    expect(vars.length).toBe(Object.keys(sampleSnapshot.variables).length);

    // Verify sorted order
    const keys = vars.map(([name]) => name);
    expect(keys).toEqual([...keys].sort());

    const map = new Map(vars);
    expect(map.get("--clay-ds-button-primary-root-rest-background-color")).toBe(
      "var(--clay-accent-primary)",
    );
    expect(map.get("--clay-ds-button-primary-root-rest-border-radius")).toBe(
      "8px",
    );
    expect(map.get("--clay-ds-button-primary-root-rest-backdrop-blur")).toBe(
      "16px",
    );
    expect(map.get("--clay-ds-button-primary-root-rest-transform-preset")).toBe(
      "translateY(-1px)",
    );
  });

  it("throws and rejects entire snapshot if any variable is invalid", () => {
    const invalidSnapshot: DesignSystemSnapshot = {
      ...sampleSnapshot,
      variables: {
        ...sampleSnapshot.variables,
        "button.primary.root.rest.backgroundColor": {
          type: "theme-color-role",
          value: "#ff0000", // Literal hex is prohibited
        },
      },
    };

    expect(() => designSystemCssVariables(invalidSnapshot)).toThrow(
      /Invalid design system variable value/,
    );
  });

  it("installDesignSystemVariables applies properties and tracks installed key set", () => {
    const { style, properties } = createMockStyle();
    const vars: [string, string][] = [
      ["--clay-ds-a", "1px"],
      ["--clay-ds-b", "2px"],
    ];
    const installed = installDesignSystemVariables(style, vars);
    expect(installed).toEqual(new Set(["--clay-ds-a", "--clay-ds-b"]));
    expect(properties.get("--clay-ds-a")).toBe("1px");
    expect(properties.get("--clay-ds-b")).toBe("2px");

    // Replace with single property, removing obsolete ones
    const nextVars: [string, string][] = [["--clay-ds-c", "3px"]];
    const nextInstalled = installDesignSystemVariables(
      style,
      nextVars,
      installed,
    );
    expect(nextInstalled).toEqual(new Set(["--clay-ds-c"]));
    expect(properties.has("--clay-ds-a")).toBe(false);
    expect(properties.has("--clay-ds-b")).toBe(false);
    expect(properties.get("--clay-ds-c")).toBe("3px");
  });
});

describe("design system store & DOM installation", () => {
  it("installs variables atomically on the root element", () => {
    const { style, properties } = createMockStyle();
    const store = createDesignSystemStore({ style });

    let listenerCalls = 0;
    store.subscribe(() => {
      listenerCalls += 1;
    });

    store.setDesignSystem(sampleSnapshot);

    expect(listenerCalls).toBe(1);
    expect(store.get().designSystem?.specifier).toBe(
      "@thirdparty/design-sample",
    );
    expect(
      properties.get("--clay-ds-button-primary-root-rest-background-color"),
    ).toBe("var(--clay-accent-primary)");
    expect(
      properties.get("--clay-ds-button-primary-root-rest-border-radius"),
    ).toBe("8px");
  });

  it("removes stale keys when switching from larger to smaller recipe sets", () => {
    const { style, properties } = createMockStyle();
    const store = createDesignSystemStore({ style });

    store.setDesignSystem(sampleSnapshot);
    expect(
      properties.has("--clay-ds-button-primary-root-rest-backdrop-blur"),
    ).toBe(true);

    // Create a smaller snapshot with fewer variables
    const smallerSnapshot: DesignSystemSnapshot = {
      ...sampleSnapshot,
      generation: 5,
      variables: {
        "button.primary.root.rest.backgroundColor": {
          type: "theme-color-role",
          value: "accent.primary",
        },
      },
    };

    store.setDesignSystem(smallerSnapshot);

    expect(
      properties.has("--clay-ds-button-primary-root-rest-background-color"),
    ).toBe(true);
    expect(
      properties.has("--clay-ds-button-primary-root-rest-backdrop-blur"),
    ).toBe(false);
    expect(style.removeProperty).toHaveBeenCalledWith(
      "--clay-ds-button-primary-root-rest-backdrop-blur",
    );
  });

  it("is idempotent: identical revision causes no DOM writes or notifications", () => {
    const { style } = createMockStyle();
    const store = createDesignSystemStore({ style });

    store.setDesignSystem(sampleSnapshot);
    const initialCalls = vi.mocked(style.setProperty).mock.calls.length;

    let listenerCalls = 0;
    store.subscribe(() => {
      listenerCalls += 1;
    });

    // Re-set the identical snapshot
    store.setDesignSystem(sampleSnapshot);

    expect(listenerCalls).toBe(0);
    expect(vi.mocked(style.setProperty).mock.calls.length).toBe(initialCalls);
  });

  it("resets cleanly to fallback and removes all installed properties", () => {
    const { style, properties } = createMockStyle();
    const store = createDesignSystemStore({ style });

    store.setDesignSystem(sampleSnapshot);
    expect(properties.size).toBeGreaterThan(0);

    store.resetToFallback();
    expect(properties.size).toBe(0);
    expect(store.get().designSystem).toBeNull();
  });

  it("maps all catalog component matrix keys to valid host CSS variable names", () => {
    const componentKinds = [
      "button",
      "textInput",
      "modal",
      "panel",
      "dropdown",
      "list",
      "collapse",
      "badge",
      "kbd",
      "divider",
      "tab",
      "commandCentre",
      "settings",
      "editor",
    ];

    for (const kind of componentKinds) {
      const cssName = recipeVariableToCssName(
        `${kind}.default.root.rest.backgroundColor`,
      );
      expect(cssName).toMatch(/^--clay-ds-[a-z0-9-]+$/);
      expect(cssName).not.toContain("..");
      expect(cssName).not.toContain("_");
    }
  });

  it("ensures recipe color roles resolve through active-theme CSS indirection", () => {
    // When a design system specifies theme color roles, it emits `var(--clay-<role>)`
    // so that switching the content theme immediately recolors every component without
    // updating design system variables or component source.
    const variableMap = designSystemCssVariables(sampleSnapshot);
    const bgVar = variableMap.find(
      ([name]) =>
        name === "--clay-ds-button-primary-root-rest-background-color",
    );
    expect(bgVar).toBeDefined();
    expect(bgVar?.[1]).toBe("var(--clay-accent-primary)");
  });
});

describe("accessibility and effect bounds fallbacks", () => {
  it("rejects oversized blur values exceeding the 32px platform safety bound", () => {
    expect(variableToCssValue({ type: "backdrop-blur", value: 33 })).toBeNull();
    expect(variableToCssValue({ type: "backdrop-blur", value: 32 })).toBe(
      "32px",
    );
    expect(variableToCssValue({ type: "backdrop-blur", value: 0 })).toBe("0px");
  });

  it("rejects oversized motion durations exceeding the 1000ms safety bound", () => {
    expect(
      variableToCssValue({ type: "motion-duration", value: 1500 }),
    ).toBeNull();
    expect(variableToCssValue({ type: "motion-duration", value: 1000 })).toBe(
      "1000ms",
    );
    expect(variableToCssValue({ type: "motion-duration", value: 100 })).toBe(
      "100ms",
    );
  });

  it("rejects oversized border widths exceeding the 8px bound", () => {
    expect(variableToCssValue({ type: "border-width", value: 9 })).toBeNull();
    expect(variableToCssValue({ type: "border-width", value: 8 })).toBe("8px");
    expect(variableToCssValue({ type: "border-width", value: 1 })).toBe("1px");
  });

  it("projects veil material into a blur plus a theme-role fill the opener can make opaque", () => {
    // A veil's blur and its fill are separate projections: the blur is a length
    // the reduced-transparency layer switches off (§13.7), and the fill resolves
    // to a content-theme surface role, so the fallback stays opaque and themed.
    const blurCss = variableToCssValue({ type: "backdrop-blur", value: 3 });
    expect(blurCss).toBe("3px");
    expect(variableToCssValue({ type: "backdrop-blur", value: 0 })).toBe("0px");

    const solidFallback = formatColorRole("surface.overlay");
    expect(solidFallback).toBe("var(--clay-surface-overlay)");
    const scrimFallback = formatColorRole("surface.scrim");
    expect(scrimFallback).toBe("var(--clay-surface-scrim)");
  });
});
