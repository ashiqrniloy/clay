// Plan 112 task 4: generate both first-party Phosphor icon packages and the
// host safety fallback from pinned, integrity-checked upstream SVG sources.
//
// Deterministic: same inputs (vendored SVGs + packages/icon-sources.json)
// produce byte-identical outputs. Run `node scripts/generate-icon-packs.mjs`
// to (re)generate; `--check` verifies no drift (used by tests/icon_packages.rs
// and CI). No network access, no dependencies.

import { createHash } from "node:crypto";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const sourcesPath = join(repoRoot, "packages/icon-sources.json");
const sources = JSON.parse(readFileSync(sourcesPath, "utf8"));

const RELEASE = sources.upstream.release;
const VENDOR_DIR = join(repoRoot, sources.vendoredDir);
const PACKS = [
  {
    weight: "regular",
    root: "packages/icons-phosphor-regular",
    name: "@clay/icons-phosphor-regular",
    apiPrefix: "icons-phosphor-regular",
    displayName: "Phosphor Regular",
    summary: "First-party Phosphor Regular icon pack (inert geometry data).",
  },
  {
    weight: "duotone",
    root: "packages/icons-phosphor-duotone",
    name: "@clay/icons-phosphor-duotone",
    apiPrefix: "icons-phosphor-duotone",
    displayName: "Phosphor Duotone",
    summary: "First-party Phosphor Duotone icon pack (inert geometry data).",
  },
];

// ---------------------------------------------------------------------------
// Upstream parsing. Phosphor assets are flat
// `<svg viewBox="0 0 256 256" fill="currentColor"><path d="..." opacity?/>...`
// Everything else (strokes, transforms, nested groups, URLs) is rejected so
// the converter can never smuggle unbounded structure into the record.
// ---------------------------------------------------------------------------

function verifyAndParse(relPath) {
  const bytes = readFileSync(join(VENDOR_DIR, relPath));
  const digest = createHash("sha256").update(bytes).digest("hex");
  const expected = sources.fileIntegrity[relPath];
  if (!expected) throw new Error(`unlisted source file: ${relPath}`);
  if (digest !== expected) {
    throw new Error(`checksum drift for ${relPath}: ${digest} != ${expected}`);
  }

  const svg = bytes.toString("utf8");
  const viewBox = svg.match(/viewBox="([^"]+)"/)?.[1];
  if (viewBox !== "0 0 256 256") {
    throw new Error(`${relPath}: unexpected viewBox ${JSON.stringify(viewBox)}`);
  }
  if (!/fill="currentColor"/.test(svg)) {
    throw new Error(`${relPath}: fill must be currentColor`);
  }
  const openTags = svg.match(/<(?!path\b|svg\b|\/)[a-zA-Z]/g) ?? [];
  if (openTags.length > 0) {
    throw new Error(`${relPath}: unsupported elements ${openTags.join(", ")}`);
  }

  const paths = [...svg.matchAll(/<path\b[^>]*>/g)].map((match) => {
    const tag = match[0];
    const d = tag.match(/\bd="([^"]+)"/)?.[1];
    if (!d) throw new Error(`${relPath}: path without d`);
    const opacityRaw = tag.match(/\bopacity="([^"]+)"/)?.[1];
    const opacity = opacityRaw === undefined ? undefined : Number(opacityRaw);
    if (opacity !== undefined && !(opacity >= 0 && opacity <= 1)) {
      throw new Error(`${relPath}: opacity out of range: ${opacityRaw}`);
    }
    if (/transform=|style=|class=|url\(|<script/i.test(tag)) {
      throw new Error(`${relPath}: prohibited path attribute`);
    }
    return opacity === undefined ? { d } : { d, opacity };
  });
  if (paths.length < 1 || paths.length > 8) {
    throw new Error(`${relPath}: ${paths.length} paths; schema allows 1..=8`);
  }
  return { viewBox: [0, 0, 256, 256], paths };
}

function buildIcons(weight) {
  return Object.entries(sources.semanticKeys).map(([key, asset]) => {
    const relPath =
      weight === "regular"
        ? `regular/${asset}.svg`
        : `duotone/${asset}-duotone.svg`;
    return { key, ...verifyAndParse(relPath) };
  });
}

// ---------------------------------------------------------------------------
// Emission. All output text is assembled from sorted, fixed-shape structures
// with JSON.stringify, so regeneration is byte-stable.
// ---------------------------------------------------------------------------

function manifest(pack, icons) {
  const iconPack = {
    schemaVersion: 1,
    displayName: pack.displayName,
    icons,
  };
  const manifestValue = {
    name: pack.name,
    version: RELEASE.slice(1),
    type: "module",
    exports: { ".": "./dist/index.js", "./load": "./dist/load.js" },
    clay: {
      apiPrefix: pack.apiPrefix,
      entry: "./dist/index.js",
      loadEntry: "./dist/load.js",
      permissions: [],
      modes: [],
      docs: "./docs/index.md",
      performance: {
        estimatedManifestBytes: Buffer.byteLength(
          JSON.stringify(iconPack),
          "utf8",
        ),
        hotPathPolicy: "inert geometry data parsed at load; no hot-path JS",
      },
      contributions: { iconPack },
      extensionPoints: [
        {
          id: `${pack.apiPrefix}.icons`,
          version: 1,
          operations: ["replace"],
          contributionKinds: ["iconPack"],
          summary: `Derive from this pack to override ${pack.weight} icon geometry.`,
        },
      ],
    },
  };
  return `${JSON.stringify(manifestValue, null, 2)}\n`;
}

function inertModule(name, role) {
  return `// ${name} — ${role}.
//
// Inert data package: all icon geometry lives in package.json under
// \`clay.contributions.iconPack\` and is parsed/validated by Clay at load.
// These no-op modules satisfy the package manifest contract; nothing
// registers at runtime.
export {};
`;
}

function docs(pack, icons) {
  const rows = icons
    .map((icon) => `| \`${icon.key}\` | ${icon.paths.length} |`)
    .join("\n");
  return `# ${pack.name}

First-party **inert geometry-data** icon pack for Clay: the ${pack.displayName} style from [Phosphor Icons](https://github.com/phosphor-icons/core) ${RELEASE}, shipped as bounded normalized geometry under \`clay.contributions.iconPack\` in [package.json](../package.json).

This package carries **no executable authority**: \`dist/index.js\` and \`dist/load.js\` are no-ops, there are no permissions, no ops, and no raw SVG/CSS at runtime. Clay parses and bounds-checks the geometry (per-path payload, command count, coordinate ranges) at package-record assembly.

Select it explicitly after loading:

\`\`\`js
await clay.loadPackage("@clay/${pack.root.split("/")[1]}");
clay.setIconPack("@clay/${pack.root.split("/")[1]}");
\`\`\`

The bundled host fallback is generated from the Regular pack; this pack only takes effect through explicit selection, which never changes theme, design-system, or typography state.

## Semantic keys (${icons.length})

| Key | Paths |
|-----|-------|
${rows}

## Provenance and license

- Upstream: [phosphor-icons/core ${RELEASE}](https://github.com/phosphor-icons/core/tree/${RELEASE}), assets converted by \`scripts/generate-icon-packs.mjs\` from the vendored, sha256-pinned sources under \`packages/icon-sources/\`.
- License: MIT — ${sources.upstream.copyright}. See [LICENSE](../LICENSE).
`;
}

function license() {
  return `MIT License

${sources.upstream.copyright}

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
`;
}

function fallbackModule(icons) {
  // Generated host safety subset: the Regular geometry, so the UI renders
  // icons with zero packages loaded or when a selection fails (task 2 state
  // table). Data only — no library imports, no executable surface.
  const header = `// @generated by scripts/generate-icon-packs.mjs from packages/icon-sources.json (${RELEASE}). Do not edit.
// Host safety fallback: bounded normalized geometry for the ${icons.length} core semantic keys.

export interface FallbackIconPath {
  d: string;
  opacity?: number;
}

export interface FallbackIcon {
  viewBox: [number, number, number, number];
  paths: FallbackIconPath[];
}

export const FALLBACK_ICONS: Record<string, FallbackIcon> = `;
  return `${header}${JSON.stringify(
    Object.fromEntries(icons.map(({ key, ...geometry }) => [key, geometry])),
    null,
    2,
  )};\n`;
}

// ---------------------------------------------------------------------------
// Driver.
// ---------------------------------------------------------------------------

function renderAll() {
  const outputs = [];
  for (const pack of PACKS) {
    const icons = buildIcons(pack.weight);
    outputs.push([join(pack.root, "package.json"), manifest(pack, icons)]);
    outputs.push([
      join(pack.root, "dist/index.js"),
      inertModule(pack.name, "inert entry module"),
    ]);
    outputs.push([
      join(pack.root, "dist/load.js"),
      inertModule(pack.name, "inert load entry"),
    ]);
    outputs.push([join(pack.root, "docs/index.md"), docs(pack, icons)]);
    outputs.push([join(pack.root, "LICENSE"), license()]);
    if (pack.weight === "regular") {
      outputs.push([
        join("frontend/src/icons/fallback.generated.ts"),
        fallbackModule(icons),
      ]);
    }
  }
  return outputs;
}

const check = process.argv.includes("--check");
for (const [relativePath, content] of renderAll()) {
  const absolute = join(repoRoot, relativePath);
  if (check) {
    const existing = readFileSync(absolute, "utf8");
    if (existing !== content) {
      throw new Error(`drift detected: ${relativePath} differs from generation`);
    }
  } else {
    mkdirSync(dirname(absolute), { recursive: true });
    writeFileSync(absolute, content);
    console.log(`wrote ${relativePath}`);
  }
}
