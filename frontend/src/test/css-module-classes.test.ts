// CSS-module class resolution guard (plan 119 SC-4 UI review).
//
// `styles.someClass` that no longer exists in the imported module is a SILENT
// failure: the element renders unstyled and nothing throws. The SC-4 split
// moved markup between modules, and the review found the Files tab's empty
// state rendering its title and body flush together because its container class
// was never defined in the module it imported. This keeps every component's
// class references honest without a renderer.

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";

const sourceRoot = path.resolve(
  path.dirname(fileURLToPath(import.meta.url)),
  "..",
);

function sourceFiles(dir: string, found: string[] = []): string[] {
  for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
    const full = path.join(dir, entry.name);
    if (entry.isDirectory()) sourceFiles(full, found);
    else found.push(full);
  }
  return found;
}

describe("CSS module class references", () => {
  it("resolves every styles.X in the module it imports", () => {
    const missing: string[] = [];
    for (const file of sourceFiles(sourceRoot)) {
      if (!/\.[jt]sx?$/.test(file) || /\.test\.[jt]sx?$/.test(file)) continue;
      const text = fs.readFileSync(file, "utf8");
      const imports = [
        ...text.matchAll(
          /import\s+([A-Za-z_$][\w$]*)\s+from\s+"([^"]+\.module\.css)"/g,
        ),
      ];
      for (const [, alias, specifier] of imports) {
        if (alias === undefined || specifier === undefined) continue;
        const css = fs.readFileSync(
          path.resolve(path.dirname(file), specifier),
          "utf8",
        );
        const defined = new Set(
          [...css.matchAll(/\.(-?[_a-zA-Z][\w-]*)/g)].map((match) => match[1]),
        );
        const used = [
          ...text.matchAll(new RegExp(`\\b${alias}\\.([_a-zA-Z][\\w]*)`, "g")),
        ];
        for (const name of new Set(used.map((match) => match[1]))) {
          if (name === undefined) continue;
          if (!defined.has(name)) {
            missing.push(
              `${path.relative(sourceRoot, file)}: ${alias}.${name} is not in ${specifier}`,
            );
          }
        }
      }
    }
    expect(missing).toEqual([]);
  });
});
