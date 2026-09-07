import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";

import { recipeVariableToCssName } from "../theme/design-system-adapter";

// ============================================================================
// Types and Static Analysis Utilities
// ============================================================================

export interface PackageManifestRecipes {
  recipes: Record<string, Record<string, unknown>>;
  recipeKeys: string[];
  emittedVars: Set<string>;
}

export interface ComponentCssConsumption {
  allConsumedVars: Set<string>;
  consumedByFile: Map<string, Set<string>>;
}

/**
 * Recursively discovers all `*.module.css` files within a directory.
 */
export function scanCssModuleFiles(dir: string): string[] {
  const results: string[] = [];
  if (!fs.existsSync(dir)) return results;
  const entries = fs.readdirSync(dir, { withFileTypes: true });
  for (const entry of entries) {
    const fullPath = path.join(dir, entry.name);
    if (entry.isDirectory()) {
      results.push(...scanCssModuleFiles(fullPath));
    } else if (entry.isFile() && entry.name.endsWith(".module.css")) {
      results.push(fullPath);
    }
  }
  return results.sort();
}

/**
 * Extracts all `--clay-ds-*` CSS custom properties referenced in a CSS string.
 */
export function extractConsumedVariables(cssContent: string): Set<string> {
  const vars = new Set<string>();
  const matches = cssContent.matchAll(/--clay-ds-([a-z0-9-]+)/g);
  for (const m of matches) {
    const varName = m[1];
    if (varName) {
      vars.add(`--clay-ds-${varName}`);
    }
  }
  return vars;
}

/**
 * Extracts all `--clay-ds-*` CSS custom properties defined in tokens.css.
 */
export function extractFallbackVariables(
  tokensCssContent: string,
): Set<string> {
  const fallbacks = new Set<string>();
  const matches = tokensCssContent.matchAll(/^\s*(--clay-ds-[a-z0-9-]+)\s*:/gm);
  for (const m of matches) {
    const varName = m[1];
    if (varName) {
      fallbacks.add(varName);
    }
  }
  return fallbacks;
}

/**
 * Parses `clay.contributions.uiDesignSystem.recipes` from a package manifest
 * and computes all emitted `--clay-ds-*` CSS custom property names using the
 * canonical adapter naming rule (`recipeVariableToCssName`).
 */
export function extractPackageRecipes(
  packageJsonContent: string,
): PackageManifestRecipes {
  const parsed = JSON.parse(packageJsonContent);
  const recipes = parsed?.clay?.contributions?.uiDesignSystem?.recipes ?? {};
  const recipeKeys = Object.keys(recipes).sort();
  const emittedVars = new Set<string>();

  for (const [recipeKey, recipeObj] of Object.entries(recipes)) {
    if (!recipeObj || typeof recipeObj !== "object") continue;
    for (const prop of Object.keys(recipeObj)) {
      const varName = recipeVariableToCssName(`${recipeKey}.${prop}`);
      emittedVars.add(varName);
    }
  }

  return { recipes, recipeKeys, emittedVars };
}

/**
 * (a) Asserts that every package recipe key has at least one property consumed
 * by component CSS as `--clay-ds-<kebab-key>-<property>`.
 * Returns any unconsumed recipe keys.
 */
export function checkRecipeKeyCoverage(
  recipeKeys: readonly string[],
  consumedVars: ReadonlySet<string>,
): string[] {
  const unconsumed: string[] = [];
  for (const key of recipeKeys) {
    const prefix = recipeVariableToCssName(key);
    const hasConsumption = Array.from(consumedVars).some(
      (v) => v === prefix || v.startsWith(`${prefix}-`),
    );
    if (!hasConsumption) {
      unconsumed.push(key);
    }
  }
  return unconsumed.sort();
}

/**
 * (b) Asserts that every fallback `--clay-ds-*` variable defined in tokens.css
 * is consumed by at least one component CSS module.
 * Returns any unconsumed fallback variables.
 */
export function checkFallbackVariableCoverage(
  fallbackVars: ReadonlySet<string>,
  consumedVars: ReadonlySet<string>,
): string[] {
  const unconsumed: string[] = [];
  for (const fv of fallbackVars) {
    if (!consumedVars.has(fv)) {
      unconsumed.push(fv);
    }
  }
  return unconsumed.sort();
}

/**
 * (c) Asserts that every `--clay-ds-*` variable consumed by CSS has either a
 * host fallback in tokens.css or an emitted recipe property in shipped packages.
 * Returns any unbacked consumed variables.
 */
export function checkVariableProvenance(
  consumedVars: ReadonlySet<string>,
  fallbackVars: ReadonlySet<string>,
  packageEmittedVars: ReadonlySet<string>,
): string[] {
  const unbacked: string[] = [];
  for (const cv of consumedVars) {
    if (!fallbackVars.has(cv) && !packageEmittedVars.has(cv)) {
      unbacked.push(cv);
    }
  }
  return unbacked.sort();
}

/**
 * Known owning component CSS module relative paths.
 */
export const COMPONENT_CSS_OWNERSHIP: Record<string, string[]> = {
  button: ["components/button.module.css"],
  textInput: ["components/text-field.module.css"],
  dropdown: ["components/controls.module.css"],
  collapse: ["components/controls.module.css"],
  modal: ["components/modal.module.css"],
  tab: ["components/tab-strip.module.css"],
  tabBar: ["components/tab-strip.module.css"],
  chat: ["chat/chat.module.css"],
  chatPanel: ["chat/chat.module.css"],
  editor: ["editor/editor.module.css"],
  editorChrome: ["editor/editor.module.css"],
  commandCentre: ["command-centre/command-centre.module.css"],
  statusItem: ["app/layout/shell.module.css"],
  statusBar: [
    "app/layout/shell.module.css",
    "packages/package-workspace.module.css",
  ],
  shell: ["app/layout/shell.module.css"],
  paneSplitTree: [
    "shell/pane-tree.module.css",
    "coding-agent/coding-agent.module.css",
  ],
  fileBrowser: ["packages/package-workspace.module.css"],
  settingsPanel: ["settings/settings-panel.module.css"],
  divider: ["components/chrome.module.css"],
};

export interface UnconsumedPropertyViolation {
  recipeKey: string;
  property: string;
  cssVariable: string;
  owningFiles: string[];
}

/**
 * (d) Asserts that recipe property keys (e.g. outlineColor / outlineWidth on textInput.*.focus)
 * are consumed by the owning component CSS.
 */
export function checkOwningComponentPropertyCoverage(
  recipes: Record<string, Record<string, unknown>>,
  consumedByFile: Map<string, Set<string>>,
  allConsumedVars: ReadonlySet<string>,
): UnconsumedPropertyViolation[] {
  const violations: UnconsumedPropertyViolation[] = [];

  for (const [recipeKey, recipeObj] of Object.entries(recipes)) {
    if (!recipeObj || typeof recipeObj !== "object") continue;
    const componentKind = recipeKey.split(".")[0] ?? "";
    const owningFiles = COMPONENT_CSS_OWNERSHIP[componentKind];

    for (const prop of Object.keys(recipeObj)) {
      const cssVar = recipeVariableToCssName(`${recipeKey}.${prop}`);

      if (owningFiles && owningFiles.length > 0) {
        // Must be consumed in at least one owning component CSS file
        const consumedInOwner = owningFiles.some((f: string) =>
          consumedByFile.get(f)?.has(cssVar),
        );
        if (!consumedInOwner) {
          violations.push({
            recipeKey,
            property: prop,
            cssVariable: cssVar,
            owningFiles,
          });
        }
      } else {
        // Fall back to check across all CSS
        if (!allConsumedVars.has(cssVar)) {
          violations.push({
            recipeKey,
            property: prop,
            cssVariable: cssVar,
            owningFiles: [],
          });
        }
      }
    }
  }

  return violations;
}

// ============================================================================
// Repository Data Loader
// ============================================================================

function loadRepoData() {
  const repoRoot = fileURLToPath(new URL("../../../", import.meta.url));
  const srcDir = path.join(repoRoot, "frontend/src");
  const tokensPath = path.join(srcDir, "styles/tokens.css");
  const neobrutalPath = path.join(
    repoRoot,
    "packages/design-neobrutal/package.json",
  );
  const glassPath = path.join(repoRoot, "packages/design-glass/package.json");

  const cssFiles = scanCssModuleFiles(srcDir);
  const allConsumedVars = new Set<string>();
  const consumedByFile = new Map<string, Set<string>>();

  for (const file of cssFiles) {
    const content = fs.readFileSync(file, "utf8");
    const rel = path.relative(srcDir, file);
    const vars = extractConsumedVariables(content);
    consumedByFile.set(rel, vars);
    for (const v of vars) allConsumedVars.add(v);
  }

  const tokensContent = fs.readFileSync(tokensPath, "utf8");
  const fallbackVars = extractFallbackVariables(tokensContent);

  const neobrutalPkg = extractPackageRecipes(
    fs.readFileSync(neobrutalPath, "utf8"),
  );
  const glassPkg = extractPackageRecipes(fs.readFileSync(glassPath, "utf8"));

  const packageRecipeKeys = Array.from(
    new Set([...neobrutalPkg.recipeKeys, ...glassPkg.recipeKeys]),
  ).sort();

  const allPackageEmittedVars = new Set<string>([
    ...neobrutalPkg.emittedVars,
    ...glassPkg.emittedVars,
  ]);

  return {
    srcDir,
    cssFiles,
    allConsumedVars,
    consumedByFile,
    fallbackVars,
    neobrutalPkg,
    glassPkg,
    packageRecipeKeys,
    allPackageEmittedVars,
  };
}

// ============================================================================
// Test Suite
// ============================================================================

describe("Phase 20.7 / Plan 110 Task 2: Recipe-consumption drift & coverage", () => {
  // Gate toggle: `it.fails` indicates expected failures for unmerged tasks 3, 7, 8
  // Setting STRICT_DS_GATE=1 forces immediate raw failures.
  const redFirst = process.env.STRICT_DS_GATE === "1" ? it : it.fails;

  // --------------------------------------------------------------------------
  // 1. Synthetic Drift Regression Tests
  // --------------------------------------------------------------------------
  describe("Synthetic drift regression (rule verification with mock data)", () => {
    it("fails when a synthetic recipe key is absent from CSS consumption", () => {
      const mockRecipeKeys = [
        "button.default.root.rest",
        "phantomWidget.default.root.rest",
      ];
      const mockConsumedVars = new Set([
        "--clay-ds-button-default-root-rest-background-color",
      ]);

      const unconsumed = checkRecipeKeyCoverage(
        mockRecipeKeys,
        mockConsumedVars,
      );
      expect(unconsumed).toContain("phantomWidget.default.root.rest");
      expect(unconsumed).not.toContain("button.default.root.rest");
    });

    it("fails when a synthetic fallback variable in tokens.css is unconsumed", () => {
      const mockFallbackVars = new Set([
        "--clay-ds-button-default-root-rest-background-color",
        "--clay-ds-unused-speculative-control-background-color",
      ]);
      const mockConsumedVars = new Set([
        "--clay-ds-button-default-root-rest-background-color",
      ]);

      const unconsumed = checkFallbackVariableCoverage(
        mockFallbackVars,
        mockConsumedVars,
      );
      expect(unconsumed).toContain(
        "--clay-ds-unused-speculative-control-background-color",
      );
      expect(unconsumed).not.toContain(
        "--clay-ds-button-default-root-rest-background-color",
      );
    });

    it("fails when a synthetic consumed CSS variable has neither host fallback nor package recipe", () => {
      const mockConsumedVars = new Set([
        "--clay-ds-button-default-root-rest-background-color",
        "--clay-ds-rogue-component-custom-color",
      ]);
      const mockFallbackVars = new Set([
        "--clay-ds-button-default-root-rest-background-color",
      ]);
      const mockPackageEmittedVars = new Set<string>();

      const unbacked = checkVariableProvenance(
        mockConsumedVars,
        mockFallbackVars,
        mockPackageEmittedVars,
      );
      expect(unbacked).toContain("--clay-ds-rogue-component-custom-color");
      expect(unbacked).not.toContain(
        "--clay-ds-button-default-root-rest-background-color",
      );
    });

    it("fails when a synthetic recipe property key is unconsumed by the owning component CSS", () => {
      const mockRecipes = {
        "textInput.default.input.focus": {
          borderColor: "border.focus",
          outlineColor: "focus.ring",
          outlineWidth: 2.0,
        },
      };
      const mockConsumedByFile = new Map<string, Set<string>>([
        [
          "components/text-field.module.css",
          new Set(["--clay-ds-text-input-default-input-focus-border-color"]),
        ],
      ]);
      const mockAllConsumed = new Set([
        "--clay-ds-text-input-default-input-focus-border-color",
      ]);

      const violations = checkOwningComponentPropertyCoverage(
        mockRecipes,
        mockConsumedByFile,
        mockAllConsumed,
      );

      const deadOutlineColor = violations.find(
        (v) =>
          v.recipeKey === "textInput.default.input.focus" &&
          v.property === "outlineColor",
      );
      const deadOutlineWidth = violations.find(
        (v) =>
          v.recipeKey === "textInput.default.input.focus" &&
          v.property === "outlineWidth",
      );

      expect(deadOutlineColor).toBeDefined();
      expect(deadOutlineWidth).toBeDefined();
      expect(deadOutlineColor?.cssVariable).toBe(
        "--clay-ds-text-input-default-input-focus-outline-color",
      );
    });
  });

  // --------------------------------------------------------------------------
  // 2. Real-World Drift Detection Verification
  // --------------------------------------------------------------------------
  describe("Real-world drift detection (verifies that known repository drift is actively detected)", () => {
    it("confirms canonical tab.default.item.* recipes are declared in packages and consumed by CSS", () => {
      const repo = loadRepoData();
      const unconsumed = checkRecipeKeyCoverage(
        repo.packageRecipeKeys,
        repo.allConsumedVars,
      );

      // Legacy root keys are no longer present in packages
      expect(repo.packageRecipeKeys).not.toContain("tab.default.root.rest");
      // Canonical item keys are declared and consumed
      expect(repo.packageRecipeKeys).toContain("tab.default.item.rest");
      expect(unconsumed).not.toContain("tab.default.item.rest");
    });

    it("confirms tab.default.item.* consumed in CSS is fully backed by package recipes and host fallbacks", () => {
      const repo = loadRepoData();
      const unbacked = checkVariableProvenance(
        repo.allConsumedVars,
        repo.fallbackVars,
        repo.allPackageEmittedVars,
      );

      expect(unbacked).not.toContain(
        "--clay-ds-tab-default-item-hover-background-color",
      );
      expect(unbacked).not.toContain(
        "--clay-ds-tab-default-item-selected-background-color",
      );
    });

    it("confirms textInput.default.input.focus outline properties are actively consumed by text-field.module.css", () => {
      const repo = loadRepoData();
      const violations = checkOwningComponentPropertyCoverage(
        repo.neobrutalPkg.recipes,
        repo.consumedByFile,
        repo.allConsumedVars,
      );

      const deadTextInputOutline = violations.filter(
        (v) =>
          v.recipeKey === "textInput.default.input.focus" &&
          v.property.startsWith("outline"),
      );

      expect(deadTextInputOutline).toEqual([]);
    });

    it("confirms 0 unconsumed fallback variables in tokens.css after Task 3 cleanup", () => {
      const repo = loadRepoData();
      const unconsumedFallbacks = checkFallbackVariableCoverage(
        repo.fallbackVars,
        repo.allConsumedVars,
      );

      expect(unconsumedFallbacks).toEqual([]);
    });
  });

  // --------------------------------------------------------------------------
  // 3. Real-Data Zero-Drift Gates (Red-First; turns green as Tasks 3, 7, 8 land)
  // --------------------------------------------------------------------------
  describe("Real-data zero-drift gate (strict enforcement after tasks 3, 7, 8)", () => {
    redFirst(
      "asserts all package recipe keys are consumed by CSS (turns green when tasks 3 and 8 land)",
      () => {
        const repo = loadRepoData();
        const unconsumed = checkRecipeKeyCoverage(
          repo.packageRecipeKeys,
          repo.allConsumedVars,
        );
        expect(
          unconsumed,
          `Found ${unconsumed.length} unconsumed package recipe keys in CSS: ${unconsumed.join(", ")}`,
        ).toEqual([]);
      },
    );

    it(
      "asserts all tokens.css fallback variables are consumed by CSS (turns green when task 3 lands)",
      () => {
        const repo = loadRepoData();
        const unconsumed = checkFallbackVariableCoverage(
          repo.fallbackVars,
          repo.allConsumedVars,
        );
        expect(
          unconsumed,
          `Found ${unconsumed.length} dead fallback variables in tokens.css: ${unconsumed.join(", ")}`,
        ).toEqual([]);
      },
    );

    redFirst(
      "asserts all consumed CSS variables have host fallbacks or package recipes (turns green when task 3 lands)",
      () => {
        const repo = loadRepoData();
        const unbacked = checkVariableProvenance(
          repo.allConsumedVars,
          repo.fallbackVars,
          repo.allPackageEmittedVars,
        );
        expect(
          unbacked,
          `Found ${unbacked.length} consumed CSS variables without fallback or package recipe: ${unbacked.join(", ")}`,
        ).toEqual([]);
      },
    );

    it(
      "asserts textInput focus outline properties are consumed by owning component CSS",
      () => {
        const repo = loadRepoData();
        const violations = checkOwningComponentPropertyCoverage(
          repo.neobrutalPkg.recipes,
          repo.consumedByFile,
          repo.allConsumedVars,
        );
        const textInputViolations = violations.filter(
          (v) =>
            v.recipeKey === "textInput.default.input.focus" &&
            v.property.startsWith("outline"),
        );
        expect(
          textInputViolations,
          `textInput focus outline properties must be consumed by text-field.module.css`,
        ).toEqual([]);
      },
    );
  });
});
