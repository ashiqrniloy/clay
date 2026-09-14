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
 * Recursively discovers every host stylesheet within a directory — CSS modules
 * and plain host CSS alike, minus `styles/tokens.css`, which *states* the
 * fallback values rather than consuming them.
 */
export function scanCssModuleFiles(dir: string): string[] {
  const results: string[] = [];
  if (!fs.existsSync(dir)) return results;
  const entries = fs.readdirSync(dir, { withFileTypes: true });
  for (const entry of entries) {
    const fullPath = path.join(dir, entry.name);
    if (entry.isDirectory()) {
      results.push(...scanCssModuleFiles(fullPath));
    } else if (
      entry.isFile() &&
      entry.name.endsWith(".css") &&
      entry.name !== "tokens.css"
    ) {
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
 *
 * The inventory of record is
 * `design-artifacts/prototypes/quiet-instrument-migration/README.md` (§2/§3/§4):
 * every package family names the module that must paint it. Families whose
 * surface does not exist yet (`recentRow` — the launcher is Part D) still name
 * their owner so the gate reports the missing consumer instead of silently
 * accepting a family with no home.
 */
export const COMPONENT_CSS_OWNERSHIP: Record<string, string[]> = {
  badge: ["components/chrome.module.css"],
  kbd: ["components/chrome.module.css"],
  divider: ["components/chrome.module.css"],
  tooltip: ["components/tooltip.module.css"],
  button: ["components/button.module.css"],
  textInput: ["components/text-field.module.css"],
  label: ["components/text.module.css"],
  dropdown: ["components/controls.module.css"],
  list: [
    "components/controls.module.css",
    "routes/workspace.module.css",
    // The agent inspector's Settings tab lists delivered config files as
    // `list.default.row`s (no second row language for a file listing).
    "agent-settings/agent-settings.module.css",
  ],
  collapse: ["components/controls.module.css"],
  seg: ["components/controls.module.css", "app/layout/shell.module.css"],
  menu: [
    // The command palette; the agent composer's completion menus paint the
    // same family from the coding-agent surface (Plan 118 task 22 will move
    // the palette onto it too).
    "command-centre/command-centre.module.css",
    "coding-agent/coding-agent.module.css",
  ],
  popover: [
    "components/controls.module.css",
    "command-centre/command-centre.module.css",
  ],
  modal: ["components/modal.module.css"],
  toast: ["components/toast.module.css"],
  empty: ["coding-agent/coding-agent.module.css"],
  tab: ["components/tab-strip.module.css"],
  tabBar: ["components/tab-strip.module.css"],
  editor: ["editor/editor.module.css"],
  editorChrome: ["editor/editor.module.css"],
  scroll: [
    "editor/editor.module.css",
    "sdui/registry.module.css",
    "styles/global.css",
  ],
  commandCentre: ["command-centre/command-centre.module.css"],
  statusItem: ["app/layout/shell.module.css"],
  statusDot: ["coding-agent/coding-agent.module.css"],
  statusBar: [
    "app/layout/shell.module.css",
    "packages/package-workspace.module.css",
  ],
  shell: ["app/layout/shell.module.css"],
  card: ["sdui/registry.module.css"],
  panel: [
    "sdui/registry.module.css",
    "sdui/renderer.module.css",
    "packages/package-workspace.module.css",
    "settings/settings-panel.module.css",
    "routes/workspace.module.css",
  ],
  overlay: ["sdui/registry.module.css", "sdui/renderer.module.css"],
  portal: ["sdui/registry.module.css", "sdui/renderer.module.css"],
  flex: ["sdui/registry.module.css", "sdui/renderer.module.css"],
  stack: ["sdui/registry.module.css", "sdui/renderer.module.css"],
  paneSplitTree: [
    "shell/pane-tree.module.css",
    "coding-agent/coding-agent.module.css",
  ],
  fileBrowser: ["packages/package-workspace.module.css"],
  settingsPanel: ["settings/settings-panel.module.css"],
  agentPicker: [
    "coding-agent/coding-agent.module.css",
    "components/controls.module.css",
  ],
  sessionRow: ["coding-agent/coding-agent.module.css"],
  statRow: ["coding-agent/coding-agent.module.css"],
  keyHint: [
    "coding-agent/coding-agent.module.css",
    "command-centre/command-centre.module.css",
  ],
  swatch: ["settings/settings-panel.module.css"],
  recentRow: ["routes/start.module.css"],
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

/**
 * (e) Asserts that a recipe key consumed by CSS is consumed by one of its
 * recorded owner modules (not only somewhere on the tree, which would let a
 * surface borrow another surface's recipe). Returns the misrouted keys.
 */
export function checkOwnerRouting(
  recipeKeys: readonly string[],
  consumedByFile: ReadonlyMap<string, Set<string>>,
  ownership: Record<string, string[]>,
): string[] {
  const misrouted: string[] = [];
  for (const key of recipeKeys) {
    const owners = ownership[key.split(".")[0] ?? ""];
    if (!owners) continue;
    const prefix = recipeVariableToCssName(key);
    const matches = (vars: Iterable<string>) =>
      [...vars].some((v) => v === prefix || v.startsWith(`${prefix}-`));
    const consumedSomewhere = [...consumedByFile.values()].some(matches);
    if (!consumedSomewhere) continue;
    const consumedByOwner = owners.some((file) =>
      matches(consumedByFile.get(file) ?? []),
    );
    if (!consumedByOwner)
      misrouted.push(`${key} (owners: ${owners.join(", ")})`);
  }
  return misrouted.sort();
}

// ============================================================================
// Repository Data Loader
// ============================================================================

function loadRepoData() {
  const repoRoot = fileURLToPath(new URL("../../../", import.meta.url));
  const srcDir = path.join(repoRoot, "frontend/src");
  const tokensPath = path.join(srcDir, "styles/tokens.css");
  // Plan 118 task 9: the shipped system is @clay/design-instrument. The removed
  // Neobrutal/Glass packages are gone, so the gate reads exactly one package.
  const shippedPath = path.join(
    repoRoot,
    "packages/design-instrument/package.json",
  );

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

  const shippedPkg = extractPackageRecipes(
    fs.readFileSync(shippedPath, "utf8"),
  );

  const packageRecipeKeys = Array.from(new Set(shippedPkg.recipeKeys)).sort();

  const allPackageEmittedVars = new Set<string>(shippedPkg.emittedVars);

  return {
    srcDir,
    cssFiles,
    allConsumedVars,
    consumedByFile,
    fallbackVars,
    shippedPkg,
    packageRecipeKeys,
    allPackageEmittedVars,
  };
}

// ============================================================================
// Test Suite
// ============================================================================

/**
 * Plan 118 task 21: the adoption backlog, recorded when the drift gate stopped
 * tolerating drift. Both lists are asserted by exact equality, so anything new
 * fails immediately and anything adopted must leave the recording — the two
 * host-CSS adoption tasks shrink them to empty and then delete the file.
 */
interface AdoptionBacklog {
  note: string;
  unconsumedRecipeKeys: string[];
  unbackedConsumedVariables: string[];
}

function loadAdoptionBacklog(): AdoptionBacklog {
  const path = fileURLToPath(
    new URL("./fixtures/design-system-adoption-backlog.json", import.meta.url),
  );
  return JSON.parse(fs.readFileSync(path, "utf8")) as AdoptionBacklog;
}

describe("Phase 20.7 / Plan 110 Task 2 / Plan 118 Task 21: recipe-consumption drift & coverage", () => {
  const backlog = loadAdoptionBacklog();

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
    it("fails when a synthetic recipe key is consumed outside its owner module", () => {
      const consumedByFile = new Map<string, Set<string>>([
        [
          "editor/editor.module.css",
          new Set(["--clay-ds-button-default-root-rest-border-color"]),
        ],
      ]);
      expect(
        checkOwnerRouting(["button.default.root.rest"], consumedByFile, {
          button: ["components/button.module.css"],
        }),
      ).toEqual([
        "button.default.root.rest (owners: components/button.module.css)",
      ]);
      // Consumed by its owner → no violation.
      expect(
        checkOwnerRouting(
          ["button.default.root.rest"],
          new Map([
            [
              "components/button.module.css",
              new Set(["--clay-ds-button-default-root-rest-border-color"]),
            ],
          ]),
          { button: ["components/button.module.css"] },
        ),
      ).toEqual([]);
    });
  });
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
        repo.shippedPkg.recipes,
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
  describe("Real-data zero-drift gate (hard since Plan 118 task 21)", () => {
    it("asserts every package recipe key is consumed by CSS or recorded in the adoption backlog", () => {
      const repo = loadRepoData();
      const unconsumed = checkRecipeKeyCoverage(
        repo.packageRecipeKeys,
        repo.allConsumedVars,
      );
      expect(
        unconsumed,
        `Found ${unconsumed.length} unconsumed package recipe keys in CSS: ${unconsumed.join(", ")}`,
      ).toEqual(backlog.unconsumedRecipeKeys);

      // The recording cannot contain keys the package does not declare: a
      // renamed or dropped recipe would otherwise stay "recorded" forever.
      const declared = new Set(repo.packageRecipeKeys);
      const phantom = backlog.unconsumedRecipeKeys.filter(
        (key) => !declared.has(key),
      );
      expect(phantom, "recorded keys that no package declares").toEqual([]);
      expect(backlog.unconsumedRecipeKeys).toEqual(
        [...backlog.unconsumedRecipeKeys].sort(),
      );
    });

    it("asserts all tokens.css fallback variables are consumed by CSS (turns green when task 3 lands)", () => {
      const repo = loadRepoData();
      const unconsumed = checkFallbackVariableCoverage(
        repo.fallbackVars,
        repo.allConsumedVars,
      );
      expect(
        unconsumed,
        `Found ${unconsumed.length} dead fallback variables in tokens.css: ${unconsumed.join(", ")}`,
      ).toEqual([]);
    });

    it("asserts every consumed CSS variable has a host fallback or package recipe, or is recorded in the adoption backlog", () => {
      const repo = loadRepoData();
      const unbacked = checkVariableProvenance(
        repo.allConsumedVars,
        repo.fallbackVars,
        repo.allPackageEmittedVars,
      );
      expect(
        unbacked,
        `Found ${unbacked.length} consumed CSS variables without fallback or package recipe: ${unbacked.join(", ")}`,
      ).toEqual(backlog.unbackedConsumedVariables);

      // The recording cannot name variables no module consumes: an unbacked
      // reference that was migrated away must leave the list.
      const live = backlog.unbackedConsumedVariables.filter((v) =>
        repo.allConsumedVars.has(v),
      );
      expect(live).toEqual(backlog.unbackedConsumedVariables);
      expect(backlog.unbackedConsumedVariables).toEqual(
        [...backlog.unbackedConsumedVariables].sort(),
      );
    });

    it("asserts textInput focus outline properties are consumed by owning component CSS", () => {
      const repo = loadRepoData();
      const violations = checkOwningComponentPropertyCoverage(
        repo.shippedPkg.recipes,
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
    });

    it("routes every consumed recipe key through its owning component CSS module", () => {
      const repo = loadRepoData();
      const misrouted = checkOwnerRouting(
        repo.packageRecipeKeys,
        repo.consumedByFile,
        COMPONENT_CSS_OWNERSHIP,
      );
      expect(
        misrouted,
        `consumed outside its recorded owner module: ${misrouted.join("; ")}`,
      ).toEqual([]);
    });

    it("names an owner module for every declared recipe family", () => {
      const repo = loadRepoData();
      const families = [
        ...new Set(
          repo.packageRecipeKeys.map((key) => key.split(".")[0] ?? ""),
        ),
      ].sort();
      const unmapped = families.filter((f) => !COMPONENT_CSS_OWNERSHIP[f]);
      expect(
        unmapped,
        `families with no recorded CSS owner: ${unmapped.join(", ")}`,
      ).toEqual([]);
    });
  });
});
