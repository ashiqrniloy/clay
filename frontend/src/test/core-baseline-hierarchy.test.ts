import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";

// Plan 110 Task 9: core baseline hierarchy gates. Locks the host-fallback
// surface differentiation (field fill vs control fill, ghost muted border,
// transparent list rows) and the default typography steps (13px body,
// 15px/13 title, 12px/13 label) against silent regression.

const repoRoot = path.resolve(
  path.dirname(fileURLToPath(import.meta.url)),
  "../../..",
);

function readRepo(relativePath: string): string {
  return fs.readFileSync(path.join(repoRoot, relativePath), "utf8");
}

function tokensCssValue(tokensCss: string, name: string): string {
  const match = tokensCss
    .replace(/\s+/g, " ")
    .match(new RegExp(`${name}:\\s*([^;]+);`));
  const value = match?.[1];
  expect(value, `${name} must exist in tokens.css`).toBeDefined();
  return value!.replace(/\s+/g, "").trim();
}

describe("core baseline hierarchy (plan 110 task 9)", () => {
  const tokensCss = readRepo("frontend/src/styles/tokens.css");

  it("gives fields a darker fill than button control fill under core fallback", () => {
    const inputFill = tokensCssValue(
      tokensCss,
      "--clay-ds-text-input-default-input-rest-background-color",
    );
    const triggerFill = tokensCssValue(
      tokensCss,
      "--clay-ds-dropdown-default-trigger-rest-background-color",
    );
    const buttonFill = tokensCssValue(
      tokensCss,
      "--clay-ds-button-default-root-rest-background-color",
    );
    expect(inputFill).toBe("var(--clay-surface-main)");
    expect(triggerFill).toBe("var(--clay-surface-main)");
    expect(buttonFill).toBe("var(--clay-surface-control)");
    expect(inputFill).not.toBe(buttonFill);
  });

  it("keeps muted buttons reading as controls via a ghost border", () => {
    const border = tokensCssValue(
      tokensCss,
      "--clay-ds-button-muted-root-rest-border-color",
    );
    expect(border).toBe("var(--clay-border-subtle)");
    const buttonCss = readRepo("frontend/src/components/button.module.css");
    const mutedStart = buttonCss.indexOf(".muted {");
    const mutedBlock = buttonCss.slice(
      mutedStart,
      buttonCss.indexOf("}", mutedStart),
    );
    expect(mutedBlock).toContain("var(--clay-border-subtle)");
  });

  it("rests list rows transparent between hairline separators", () => {
    const rowFill = tokensCssValue(
      tokensCss,
      "--clay-ds-list-default-row-rest-background-color",
    );
    expect(rowFill).toBe("transparent");
  });

  it("mirrors field/control differentiation in the Rust core fallbacks", () => {
    const designSystem = readRepo("src/shell/design_system.rs");
    const inputRest = designSystem.slice(
      designSystem.indexOf('RecipeKey::new("textInput", "default", "input"'),
      designSystem.indexOf(
        ");",
        designSystem.indexOf('RecipeKey::new("textInput", "default", "input"'),
      ),
    );
    expect(inputRest).toContain('"surface.main"');
    const components = designSystem.slice(
      designSystem.indexOf("let standard_components"),
      designSystem.indexOf(
        "];",
        designSystem.indexOf("let standard_components"),
      ),
    );
    expect(components).toContain('("dropdown", "surface.main"');
    expect(components).not.toContain('("dropdown", "surface.control"');
    // Muted buttons are ghost: transparent fill, subtle border.
    expect(designSystem).toContain(
      '("transparent", "text.muted", "border.subtle")',
    );
  });

  it("defaults the ui profile to 13px with 15px titles and 12px labels", () => {
    const protocol = readRepo("src/protocol/mod.rs");
    const defaultBlock = protocol.slice(
      protocol.indexOf("pub const DEFAULT: Self = Self {"),
      protocol.indexOf(
        "};",
        protocol.indexOf("pub const DEFAULT: Self = Self {"),
      ),
    );
    expect(defaultBlock).toContain("title: 15.0 / 13.0");
    expect(defaultBlock).toContain("detail: 12.0 / 13.0");
    const uiProfile = protocol.slice(
      protocol.indexOf("impl Default for ActiveTypography"),
    );
    expect(uiProfile).toMatch(/ui: FontProfile \{[\s\S]*?size: 13\.0/);
  });

  it("ships a host default ui font stack with a generic fallback", () => {
    const stack = tokensCssValue(tokensCss, "--clay-font-ui");
    expect(stack).toContain("system-ui");
    expect(stack.trim().endsWith("sans-serif")).toBe(true);
  });
});
