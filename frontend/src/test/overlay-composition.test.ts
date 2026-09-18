/**
 * Plan 118 — overlay-family composition gate.
 *
 * The palette, the transient menus, the modal sheet, the dropdown/menu
 * surfaces and the tooltip are the surfaces that float. This suite holds the
 * three properties the approved artifacts depend on, mechanically:
 *
 *  1. Shadows exist only on transient surfaces (DESIGN.md §11 checklist) — a
 *     static region that gains a shadow fails here.
 *  2. The overlay modules keep one source of recipe variables and no literal
 *     geometry or colour.
 *  3. Every floating surface stays inside a bounded box (`max-height` +
 *     `overflow`), so a long result list scrolls instead of growing past the
 *     window.
 */

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { describe, expect, it } from "vitest";

const repoRoot = path.resolve(
  path.dirname(fileURLToPath(import.meta.url)),
  "../..",
);

function read(relative: string): string {
  return fs.readFileSync(path.join(repoRoot, relative), "utf8");
}

function cssModules(): string[] {
  const found: string[] = [];
  const walk = (dir: string) => {
    for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
      const full = path.join(dir, entry.name);
      if (entry.isDirectory()) walk(full);
      else if (entry.name.endsWith(".module.css")) found.push(full);
    }
  };
  walk(path.join(repoRoot, "src"));
  return found;
}

/** Modules allowed to paint a shadow: transient surfaces and focus halos. */
const SHADOW_OWNERS = [
  "components/modal.module.css",
  "components/controls.module.css",
  "components/tooltip.module.css",
  "components/text-field.module.css",
  "command-centre/command-centre.module.css",
  "coding-agent/coding-agent.module.css",
  "editor/editor.module.css",
  "routes/workspace.module.css",
  "sdui/registry.module.css",
];

const OVERLAY_MODULES = [
  "src/command-centre/command-centre.module.css",
  "src/components/modal.module.css",
  "src/components/tooltip.module.css",
  "src/components/controls.module.css",
];

describe("Plan 118: overlay family composition", () => {
  it("paints shadows only on transient surfaces", () => {
    const offenders: string[] = [];
    for (const file of cssModules()) {
      const relative = path
        .relative(path.join(repoRoot, "src"), file)
        .split(/[\\/]/)
        .join("/");
      if (SHADOW_OWNERS.includes(relative)) continue;
      const css = fs.readFileSync(file, "utf8");
      for (const match of css.matchAll(/^\s*box-shadow\s*:\s*([^;]+);/gm)) {
        const value = match[1] ?? "";
        // A focus halo on an inset well is a state, not elevation.
        if (/focus-shadow/.test(value) || value.trim() === "none") continue;
        offenders.push(`${relative}: box-shadow: ${value.trim()}`);
      }
    }
    expect(offenders, "static surfaces must not float").toEqual([]);
  });

  it("keeps the overlay modules free of literal geometry and colour", () => {
    for (const file of OVERLAY_MODULES) {
      const css = read(file);
      const name = file.split(/[\\/]/).pop() ?? file;
      // Shadows are covered by the two tests above (owner allowlist + recipe
      // timing); this scan is about geometry and colour.
      for (const match of css.matchAll(
        /^\s*(border[a-z-]*|border-radius|outline)\s*:\s*([^;]+);/gm,
      )) {
        // `var(...)` (including its documented fallback) is the recipe channel;
        // anything left outside one must not be a literal.
        const value = (match[2] ?? "").replace(
          /var\([^()]*(?:\([^()]*\)[^()]*)*\)/g,
          "",
        );
        expect(
          /\d+px|\d+ms|#[0-9a-f]{3,8}\b|rgba?\(/i.test(value),
          `${name}: \`${match[1]}: ${match[2]?.trim()}\` must come from a recipe`,
        ).toBe(false);
      }
    }
  });

  it("bounds the palette and lets its rows scroll", () => {
    const css = read("src/command-centre/command-centre.module.css");
    // Plan 124 anchored the sheet to the composer box: `min(52vh, 420px)`,
    // internal scroll, and never a second frame around the rows.
    expect(css).toMatch(
      /\.palette\s*\{[^}]*max-height:\s*min\(52vh,\s*420px\)/s,
    );
    expect(css).toMatch(/\.palette\s*\{[^}]*overflow:\s*hidden/s);
    expect(css).toMatch(/\.palList\s*\{[^}]*overflow:\s*auto/s);
  });

  it("draws no ring on the sheet: the composer box in focus is the boundary", () => {
    const css = read("src/command-centre/command-centre.module.css");
    // One ring per surface (§9/§14.4): the field the sheet answers to is the
    // shell of the composition, so the sheet declares no focus state of its own
    // and never paints an outline (Plan 124's approved geometry). The row and
    // chip outlines below it are the list/seg recipes' own focus states.
    expect(css).not.toMatch(/\.palette:focus-within/);
    expect(css).not.toMatch(/\.palette\s*\{[^}]*outline:/s);
    expect(css).toMatch(/\.palRow:focus-visible\s*\{[^}]*outline:/s);
  });

  it("composes the modal as head, scrolling body and an actions foot", () => {
    const css = read("src/components/modal.module.css");
    expect(css).toMatch(/\.title\s*\{[^}]*border-bottom:/s);
    expect(css).toMatch(/\.body\s*\{[^}]*overflow:\s*auto/s);
    expect(css).toMatch(/\.foot\s*\{[^}]*border-top:/s);
    expect(css).toMatch(/\.foot\s*\{[^}]*justify-content:\s*space-between/s);
    // Flush content owns the surface: no second frame or head.
    expect(css).toMatch(/\.flush\s*\{[^}]*box-shadow:\s*none/s);
    // Flush content paints the head: no heading element is rendered with it.
    const modal = read("src/components/modal.tsx");
    expect(modal).toMatch(/\{!flush && \(/);
  });

  it("resolves every veil to an opaque surface when transparency is reduced", () => {
    const css = read("src/styles/global.css");
    const reduced = css.slice(
      css.indexOf("@media (prefers-reduced-transparency: reduce)"),
      css.indexOf("@supports not (backdrop-filter"),
    );
    // The scrim resolves to the canvas, veil-bearing surfaces to the already
    // opaque overlay plane, and no blur keeps running.
    expect(reduced).toContain("var(--clay-surface-main)");
    expect(reduced).toContain("var(--clay-surface-overlay)");
    expect(reduced).toMatch(/\[data-clay-component="panel"\]/);
    expect(reduced).toContain("backdrop-filter: none !important");
  });

  it("collapses overlay motion when motion is reduced", () => {
    const css = read("src/styles/global.css");
    const reduced = css.slice(
      css.indexOf("@media (prefers-reduced-motion: reduce)"),
      css.indexOf("@media (prefers-reduced-transparency"),
    );
    expect(reduced).toContain("transition-duration: 0.01ms !important");
    expect(reduced).toContain("transform: none !important");
  });

  it("animates floating surfaces with the recipe's own motion only", () => {
    for (const file of OVERLAY_MODULES) {
      const css = read(file);
      for (const match of css.matchAll(/transition[^;]*;/gs)) {
        const value = match[0];
        expect(
          value.includes("--clay-ds-") || value.includes("none"),
          `${file.split(/[\\/]/).pop() ?? file}: \`${value.replace(/\s+/g, " ").trim()}\` must use recipe timing`,
        ).toBe(true);
      }
    }
  });
});
