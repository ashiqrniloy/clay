// Bundle-size budget check (Plan 097 Phase 4/5). Run after `npm run build`.
// Startup shell excludes code-split editor, package, desktop-workflow, and
// chat (AG-UI) chunks. Total gzip includes every lazy renderer.

import { readFileSync, existsSync, readdirSync, statSync } from "node:fs";
import { join } from "node:path";
import { gzipSync } from "node:zlib";

const dist = "dist/assets";
const SHELL_BUDGET_GZIP_KB = 180;
const TOTAL_BUDGET_GZIP_KB = 400;

if (!existsSync(dist)) {
  console.error("dist/assets missing — run `npm run build` first");
  process.exit(1);
}

let totalGzip = 0;
let shellGzip = 0;
const rows = [];
for (const file of readdirSync(dist)) {
  const full = join(dist, file);
  if (!statSync(full).isFile()) continue;
  if (!/\.(js|css)$/.test(file)) continue;
  const bytes = readFileSync(full);
  const gz = gzipSync(bytes).length;
  totalGzip += gz;
  const editor = /codemirror|ClayEditor|create-editor|editor/i.test(file);
  const packageRenderer = /PackageWorkspace|registry|package-workspace/i.test(
    file,
  );
  const workflow = /CommandCentre|WorkspacePanes|controls/i.test(file);
  const chat = /ChatPanel|ag-ui|TauriClay|agent/i.test(file);
  if (!editor && !packageRenderer && !workflow && !chat) shellGzip += gz;
  rows.push([
    file,
    `${(gz / 1024).toFixed(1)} kB`,
    editor
      ? "editor"
      : packageRenderer
        ? "package"
        : workflow
          ? "workflow"
          : chat
            ? "chat"
            : "shell",
  ]);
}
for (const [file, size, lane] of rows) {
  console.log(`${file.padEnd(40)} ${size}  ${lane}`);
}
const totalKb = totalGzip / 1024;
const shellKb = shellGzip / 1024;
console.log(
  `shell gzip: ${shellKb.toFixed(1)} kB (budget ${SHELL_BUDGET_GZIP_KB} kB)`,
);
console.log(
  `total gzip: ${totalKb.toFixed(1)} kB (budget ${TOTAL_BUDGET_GZIP_KB} kB)`,
);
let failed = false;
if (shellKb > SHELL_BUDGET_GZIP_KB) {
  console.error("SHELL BUNDLE BUDGET EXCEEDED");
  failed = true;
}
if (totalKb > TOTAL_BUDGET_GZIP_KB) {
  console.error("TOTAL BUNDLE BUDGET EXCEEDED");
  failed = true;
}
if (failed) process.exit(1);
