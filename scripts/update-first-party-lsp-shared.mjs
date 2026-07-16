#!/usr/bin/env node
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import path from "node:path";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const files = ["utf8.js", "framing.js", "positions.js", "mapping.js", "client.js", "typescript-language-server.js"];
const packages = ["lsp-rust", "lsp-typescript", "lsp-javascript", "lsp-markdown"];
const check = process.argv.includes("--check");
let stale = false;

for (const packageName of packages) {
  const destination = path.join(root, "packages", packageName, "dist", "shared");
  if (!check) await mkdir(destination, { recursive: true });
  for (const file of files) {
    const source = await readFile(path.join(root, "packages", "lsp-shared", file));
    const target = path.join(destination, file);
    if (check) {
      let current;
      try {
        current = await readFile(target);
      } catch {
        current = null;
      }
      if (!current?.equals(source)) {
        console.error(`stale: ${path.relative(root, target)}`);
        stale = true;
      }
    } else {
      await writeFile(target, source);
    }
  }
}

if (stale) {
  console.error("run: node scripts/update-first-party-lsp-shared.mjs");
  process.exitCode = 1;
}
