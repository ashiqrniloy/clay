import { readFileSync, writeFileSync } from "node:fs";
import { resolve, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import { chromium } from "/home/arn/Projects/clay/clay-agent/node_modules/playwright-core/index.mjs";

const __dirname = dirname(fileURLToPath(import.meta.url));

const FIXTURES = [
  "controls",
  "splits",
  "chat",
  "command-centre",
  "settings",
  "package-ui",
];

const THEMES = {
  "modus-vivendi": [
    ["--clay-surface-main", "#100f17"],
    ["--clay-surface-panel", "#21202b"],
    ["--clay-surface-overlay", "#2a2838"],
    ["--clay-surface-scrim", "#000000"],
    ["--clay-surface-control", "#39354a"],
    ["--clay-surface-list", "#292835"],
    ["--clay-surface-selected", "#3d385c"],
    ["--clay-surface-hover", "#2d2b3d"],
    ["--clay-surface-active", "#343147"],
    ["--clay-surface-disabled", "#1b1a24"],
    ["--clay-text-primary", "#eeeaff"],
    ["--clay-text-muted", "#b9b2cf"],
    ["--clay-text-disabled", "#6f6a87"],
    ["--clay-accent-primary", "#7c6fff"],
    ["--clay-accent-muted", "#5951a0"],
    ["--clay-focus-ring", "#968aff"],
    ["--clay-border-hairline", "#282638"],
    ["--clay-border-subtle", "#2f2c40"],
    ["--clay-border-strong", "#3d385c"],
    ["--clay-border-focus", "#7c6fff"],
    ["--clay-border-kbd", "#2f2c40"],
    ["--clay-surface-badge", "#39354a"],
    ["--clay-text-badge", "#eeeaff"],
    ["--clay-surface-kbd", "#292835"],
    ["--clay-text-kbd", "#eeeaff"],
    ["--clay-surface-tooltip", "#2a2838"],
    ["--clay-text-tooltip", "#eeeaff"],
    ["--clay-text-icon", "#b9b2cf"],
    ["--clay-surface-scrollbar", "#6f6a87"],
    ["--clay-surface-scrollbar-track", "#21202b"],
    ["--clay-diagnostic-error", "#ff6b6b"],
    ["--clay-diagnostic-warning", "#ffc66b"],
    ["--clay-diagnostic-info", "#6bb2ff"],
    ["--clay-diagnostic-success", "#6bffb0"],
  ],
  "modus-operandi": [
    ["--clay-surface-main", "#ffffff"],
    ["--clay-surface-panel", "#f2f2f2"],
    ["--clay-surface-overlay", "#f2f2f2"],
    ["--clay-surface-scrim", "#000000"],
    ["--clay-surface-control", "#c8c8c8"],
    ["--clay-surface-list", "#f2f2f2"],
    ["--clay-surface-selected", "#c0deff"],
    ["--clay-surface-hover", "#dbe8f6"],
    ["--clay-surface-active", "#b2d4f5"],
    ["--clay-surface-disabled", "#e8e8e8"],
    ["--clay-text-primary", "#000000"],
    ["--clay-text-muted", "#595959"],
    ["--clay-text-disabled", "#8a8a8a"],
    ["--clay-accent-primary", "#000000"],
    ["--clay-accent-muted", "#595959"],
    ["--clay-focus-ring", "#000000"],
    ["--clay-border-hairline", "#d8d8d8"],
    ["--clay-border-subtle", "#c8c8c8"],
    ["--clay-border-strong", "#9f9f9f"],
    ["--clay-border-focus", "#000000"],
    ["--clay-border-kbd", "#595959"],
    ["--clay-surface-badge", "#c8c8c8"],
    ["--clay-text-badge", "#000000"],
    ["--clay-surface-kbd", "#c8c8c8"],
    ["--clay-text-kbd", "#000000"],
    ["--clay-surface-tooltip", "#f2f2f2"],
    ["--clay-text-tooltip", "#000000"],
    ["--clay-text-icon", "#595959"],
    ["--clay-surface-scrollbar", "#9f9f9f"],
    ["--clay-surface-scrollbar-track", "#f2f2f2"],
    ["--clay-diagnostic-error", "#a60000"],
    ["--clay-diagnostic-warning", "#884900"],
    ["--clay-diagnostic-info", "#005f5f"],
    ["--clay-diagnostic-success", "#005f5f"],
  ],
};

const neoVars = JSON.parse(readFileSync(resolve(__dirname, "neobrutal.json"), "utf8"));
const glassVars = JSON.parse(readFileSync(resolve(__dirname, "glass.json"), "utf8"));

const STYLES = {
  core: [],
  neobrutal: neoVars,
  glass: glassVars,
};

async function main() {
  const executablePath = "/home/arn/.cache/puppeteer/chrome/linux-152.0.7977.42/chrome-linux64/chrome";
  console.log(`Starting headless Chrome via CDP: ${executablePath}...`);
  const browser = await chromium.launch({
    executablePath,
    headless: true,
  });

  const page = await browser.newPage({
    viewport: { width: 1280, height: 800 },
  });

  const manifest = [];

  for (const fixture of FIXTURES) {
    console.log(`\nNavigating to fixture: ${fixture}...`);
    await page.goto(`http://localhost:5199/?fixture=${fixture}`);
    await page.waitForSelector(`[data-fixture^="${fixture}"]`);
    // Wait for lazy components / state to settle
    await page.waitForTimeout(500);

    for (const [themeName, themeVars] of Object.entries(THEMES)) {
      for (const [styleName, styleVars] of Object.entries(STYLES)) {
        const filename = `${fixture}__${styleName}__${themeName}.png`;
        const filepath = resolve(__dirname, filename);

        await page.evaluate(({ tVars, sVars }) => {
          // 1. Clear existing style custom properties
          const toRemove = [];
          for (let i = 0; i < document.documentElement.style.length; i++) {
            const prop = document.documentElement.style[i];
            if (prop.startsWith("--clay-ds-")) {
              toRemove.push(prop);
            }
          }
          for (const prop of toRemove) {
            document.documentElement.style.removeProperty(prop);
          }

          // 2. Set theme tokens
          for (const [k, v] of tVars) {
            document.documentElement.style.setProperty(k, v);
          }

          // 3. Set design-system recipe vars (if any)
          for (const [k, v] of sVars) {
            document.documentElement.style.setProperty(k, v);
          }
        }, { tVars: themeVars, sVars: styleVars });

        // Short pause for style recalculation & repaint
        await page.waitForTimeout(100);

        await page.screenshot({ path: filepath });
        console.log(`  Captured: ${filename}`);
        manifest.push({
          fixture,
          style: styleName,
          theme: themeName,
          file: filename,
        });
      }
    }
  }

  await browser.close();
  writeFileSync(resolve(__dirname, "manifest.json"), JSON.stringify(manifest, null, 2));
  console.log(`\nAll ${manifest.length} baseline screenshots captured and recorded in manifest.json!`);
}

main().catch((err) => {
  console.error("Capture failed:", err);
  process.exit(1);
});
