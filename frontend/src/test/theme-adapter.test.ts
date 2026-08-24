import { describe, expect, it } from "vitest";

import {
  installVariables,
  themeCssVariables,
  tokenToCssName,
  typographyCssVariables,
  variantSize,
} from "../theme/adapter";
import type { ThemeSnapshot, TypographySnapshot } from "../theme/types";

const theme: ThemeSnapshot = {
  specifier: "@clay/theme-gruvbox-material-dark",
  densityScale: 0.875,
  editorStyles: {
    keyword: {
      color: "#c678ddff",
      background: null,
      bold: true,
      italic: false,
      underline: false,
      strike: false,
      scale: 1,
    },
  },
  tokens: {
    "surface.main": { type: "color", value: "#100f17" },
    "accent.primary": { type: "color", value: "#7c6fff" },
    "spacing.md": { type: "scalar", value: 16 },
    "motion.fast": { type: "scalar", value: 100 },
    "opacity.disabled": { type: "opacity", value: 0.55 },
    "z.modal": { type: "level", value: "modal" },
    "typography.title": { type: "variant", value: "title" },
  },
};

const typography: TypographySnapshot = {
  revision: 1,
  ui: {
    families: ["System Sans"],
    size: 13,
    ligatures: { enableStandard: false },
  },
  monospace: {
    families: ["Code Mono"],
    size: 12,
    ligatures: { enableStandard: true },
  },
  proportional: {
    families: ["Read Serif"],
    size: 14,
    ligatures: { enableStandard: true },
  },
  hierarchy: {
    display: 1.5,
    title: 1.08,
    section: 1,
    body: 1,
    status: 1,
    detail: 0.8,
    caption: 0.75,
  },
};

describe("token naming rule", () => {
  it("maps token names to --clay-* variables", () => {
    expect(tokenToCssName("surface.main")).toBe("--clay-surface-main");
    expect(tokenToCssName("surface.scrollbar.track")).toBe(
      "--clay-surface-scrollbar-track",
    );
    expect(tokenToCssName("dimension.overlay.centered.width")).toBe(
      "--clay-dimension-overlay-centered-width",
    );
  });
});

describe("theme adapter", () => {
  it("emits colors, levels, variants and opacity without units", () => {
    const vars = new Map(themeCssVariables(theme));
    expect(vars.get("--clay-surface-main")).toBe("#100f17");
    expect(vars.get("--clay-z-modal")).toBe("40");
    expect(vars.get("--clay-typography-title")).toBe("title");
    expect(vars.get("--clay-opacity-disabled")).toBe("0.55");
    expect(vars.get("--clay-editor-keyword-color")).toBe("#c678ddff");
    expect(vars.get("--clay-editor-keyword-scale")).toBe("1");
  });

  it("emits motion in ms and other scalars in px", () => {
    const vars = new Map(themeCssVariables(theme));
    expect(vars.get("--clay-motion-fast")).toBe("100ms");
    expect(vars.get("--clay-spacing-md")).toBe("14px"); // 16 × 0.875 density
  });

  it("pre-scales only the spacing rhythm with the density scale", () => {
    const scaled: ThemeSnapshot = {
      ...theme,
      densityScale: 1.125,
      tokens: {
        "spacing.sm": { type: "scalar", value: 12 },
        "radius.xs": { type: "scalar", value: 2 },
        "dimension.sidebar.default": { type: "scalar", value: 240 },
      },
    };
    const vars = new Map(themeCssVariables(scaled));
    expect(vars.get("--clay-spacing-sm")).toBe("13.5px");
    expect(vars.get("--clay-radius-xs")).toBe("2px");
    expect(vars.get("--clay-dimension-sidebar-default")).toBe("240px");
  });

  it("emits deterministic sorted output for snapshot stability", () => {
    const vars = themeCssVariables(theme);
    const names = vars.map(([name]) => name);
    expect([...names].sort()).toEqual(names);
  });
});

describe("typography adapter", () => {
  it("computes variant sizes once from role base × hierarchy scale", () => {
    const vars = new Map(typographyCssVariables(typography));
    expect(vars.get("--clay-font-ui")).toBe("'System Sans'");
    expect(vars.get("--clay-text-display-size")).toBe("19.5px"); // 13 × 1.5
    expect(variantSize(typography, "caption")).toBeCloseTo(9.75);
    expect(vars.get("--clay-font-feature-settings")).toContain('"liga" 0');
  });

  it("quotes multi-role family stacks safely", () => {
    const stacked: TypographySnapshot = {
      ...typography,
      ui: {
        families: ["A B", "C"],
        size: 10,
        ligatures: { enableStandard: true },
      },
    };
    const vars = new Map(typographyCssVariables(stacked));
    expect(vars.get("--clay-font-ui")).toBe("'A B', 'C'");
  });
});

describe("installVariables", () => {
  it("writes every variable onto the target style", () => {
    const written = new Map<string, string>();
    const fakeStyle = {
      setProperty(name: string, value: string) {
        written.set(name, value);
      },
    } as unknown as CSSStyleDeclaration;
    installVariables(fakeStyle, [
      ["--clay-surface-main", "#100f17"],
      ["--clay-motion-fast", "100ms"],
    ]);
    expect(written.get("--clay-surface-main")).toBe("#100f17");
    expect(written.get("--clay-motion-fast")).toBe("100ms");
  });
});
