import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";

// Plan 110 Task 9: core baseline hierarchy gates. Locks the host-fallback
// surface differentiation (a filled input well against ghost buttons, a
// borderless muted text action, transparent list rows) and the default typography
// steps (13px body, 15px/13 title, 12px/13 label) against silent regression.
//
// Plan 118 Task 16 migrated the baseline to Quiet Instrument: the pre-bootstrap
// block in styles/tokens.css is the projection of the shipped
// `@clay/design-instrument` recipes, so the values asserted here are the package's
// own values (the Rust suite asserts the block and the package agree exactly).

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
  if (value === undefined) {
    throw new Error(`${name} must exist in tokens.css`);
  }
  return value.replace(/\s+/g, "").trim();
}

describe("core baseline hierarchy (plan 110 task 9)", () => {
  const tokensCss = readRepo("frontend/src/styles/tokens.css");
  const recipes = JSON.parse(
    readRepo("packages/design-instrument/package.json"),
  ).clay.contributions.uiDesignSystem.recipes as Record<
    string,
    Record<string, string | number>
  >;
  const recipeValue = (key: string, field: string) => {
    const value = recipes[key]?.[field];
    expect(
      value,
      `${key}.${field} must be declared by the package`,
    ).toBeDefined();
    return value;
  };

  it("gives the input well a fill while a default button stays a hairline ghost", () => {
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
    // The field is a filled well; the default button and the dropdown trigger are
    // hairlines on the canvas (DESIGN.md §11: only a surface with a fill reads as
    // recessed, and §14 bans a second box inside one).
    expect(inputFill).toBe("var(--clay-surface-control)");
    expect(recipeValue("textInput.default.input.rest", "backgroundColor")).toBe(
      "surface.control",
    );
    expect(buttonFill).toBe("transparent");
    expect(triggerFill).toBe("transparent");
    expect(inputFill).not.toBe(buttonFill);
  });

  it("keeps the muted variant a text action (no border, no fill)", () => {
    const border = tokensCssValue(
      tokensCss,
      "--clay-ds-button-muted-root-rest-border-color",
    );
    expect(border).toBe("transparent");
    expect(recipeValue("button.muted.root.rest", "borderWidth")).toBe(0);
    const buttonCss = readRepo("frontend/src/components/button.module.css");
    const mutedStart = buttonCss.indexOf(".muted {");
    const mutedBlock = buttonCss.slice(
      mutedStart,
      buttonCss.indexOf("}", mutedStart),
    );
    expect(mutedBlock).toContain(
      "--clay-ds-button-muted-root-rest-border-color",
    );
  });

  it("rests list rows transparent between hairline separators", () => {
    const rowFill = tokensCssValue(
      tokensCss,
      "--clay-ds-list-default-row-rest-background-color",
    );
    expect(rowFill).toBe("transparent");
  });

  it("mirrors the baseline hierarchy in the shipped recipes the host block projects", () => {
    // The Rust core fallbacks are asserted against this same manifest by
    // `plan118_core_fallbacks_match_the_shipped_language`; here the point is that the
    // static block the browser paints before bootstrap carries the package's values.
    expect(recipeValue("textInput.default.input.rest", "backgroundColor")).toBe(
      "surface.control",
    );
    expect(recipeValue("textInput.default.input.rest", "borderRadius")).toBe(8);
    expect(recipeValue("button.default.root.rest", "backgroundColor")).toBe(
      "transparent",
    );
    expect(recipeValue("button.default.root.rest", "borderColor")).toBe(
      "border.hairline",
    );
    expect(recipeValue("button.default.root.rest", "borderRadius")).toBe(8);
    expect(
      recipeValue("dropdown.default.trigger.rest", "backgroundColor"),
    ).toBe("transparent");
    expect(recipeValue("button.muted.root.rest", "borderWidth")).toBe(0);
    for (const name of [
      "--clay-ds-text-input-default-input-rest-border-radius",
      "--clay-ds-button-default-root-rest-border-radius",
      "--clay-ds-panel-default-root-rest-border-radius",
      "--clay-ds-modal-default-dialog-rest-border-radius",
      "--clay-ds-badge-default-root-rest-border-radius",
    ]) {
      expect(tokensCssValue(tokensCss, name), `${name} radius ladder`).not.toBe(
        "0px",
      );
    }
  });

  it("defaults the ui profile to 13px with 15px titles and 12px labels", () => {
    // Plan 133 task 2 moved the typography contract out of `src/protocol/mod.rs`
    // into `src/protocol/typography.rs` (values unchanged).
    const protocol = readRepo("src/protocol/typography.rs");
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
