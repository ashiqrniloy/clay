import { readFileSync, writeFileSync } from "node:fs";
import { resolve, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import { chromium } from "/home/arn/Projects/clay/clay-agent/node_modules/playwright-core/index.mjs";

const __dirname = dirname(fileURLToPath(import.meta.url));

// Plan 110 task 11 — final visual + accessibility review captures.
// Same methodology as 110-baseline (Vite dev server + headless Chromium CDP).

const FIXTURES = ["controls", "splits", "chat", "command-centre", "settings", "package-ui"];

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

const STYLES = { core: [], neobrutal: neoVars, glass: glassVars };

const COMBOS = [];
for (const [themeName, themeVars] of Object.entries(THEMES)) {
  for (const [styleName, styleVars] of Object.entries(STYLES)) {
    COMBOS.push({ themeName, themeVars, styleName, styleVars });
  }
}

async function applyVars(page, themeVars, styleVars) {
  await page.evaluate(({ tVars, sVars }) => {
    const toRemove = [];
    for (let i = 0; i < document.documentElement.style.length; i++) {
      const prop = document.documentElement.style[i];
      if (prop.startsWith("--clay-ds-") || prop.startsWith("--clay-surface-") || prop.startsWith("--clay-text-") || prop.startsWith("--clay-border-") || prop.startsWith("--clay-accent-") || prop.startsWith("--clay-focus") || prop.startsWith("--clay-diagnostic-")) {
        toRemove.push(prop);
      }
    }
    for (const prop of toRemove) {
      document.documentElement.style.removeProperty(prop);
    }
    for (const [k, v] of tVars) {
      document.documentElement.style.setProperty(k, v);
    }
    for (const [k, v] of sVars) {
      document.documentElement.style.setProperty(k, v);
    }
  }, { tVars: themeVars, sVars: styleVars });
}

const a11yFindings = {};

async function a11ySnapshot(page, label) {
  const cdp = await page.context().newCDPSession(page);
  const { nodes } = await cdp.send("Accessibility.getFullAXTree");
  a11yFindings[label] = nodes
    .filter((n) => n.role?.value && (n.name?.value || n.role.value === "dialog" || n.role.value === "listbox"))
    .map((n) => ({ role: n.role.value, name: (n.name?.value ?? "").slice(0, 60), focused: !!n.focused?.value, disabled: !!n.disabled?.value }));
  await cdp.detach();
}

async function main() {
  const executablePath = "/home/arn/.cache/puppeteer/chrome/linux-152.0.7977.42/chrome-linux64/chrome";
  console.log(`Starting headless Chrome via CDP: ${executablePath}...`);
  const browser = await chromium.launch({ executablePath, headless: true });
  const manifest = [];

  // ---------- Part A: base matrix (36 states, 1280x800) ----------
  {
    const page = await browser.newPage({ viewport: { width: 1280, height: 800 } });
    for (const fixture of FIXTURES) {
      await page.goto(`http://localhost:5199/?fixture=${fixture}`);
      await page.waitForSelector(`[data-fixture^="${fixture}"]`);
      await page.waitForTimeout(500);
      for (const combo of COMBOS) {
        const filename = `${fixture}__${combo.styleName}__${combo.themeName}.png`;
        await applyVars(page, combo.themeVars, combo.styleVars);
        await page.waitForTimeout(100);
        await page.screenshot({ path: resolve(__dirname, filename) });
        manifest.push({ fixture, style: combo.styleName, theme: combo.themeName, file: filename, viewport: "1280x800" });
      }
      console.log(`Part A: ${fixture} done`);
    }
    await page.close();
  }

  // ---------- Part B: controls interaction states (core/light + neobrutal/dark) ----------
  const interactionCombos = [
    COMBOS.find((c) => c.styleName === "core" && c.themeName === "modus-operandi"),
    COMBOS.find((c) => c.styleName === "neobrutal" && c.themeName === "modus-vivendi"),
  ];
  {
    const page = await browser.newPage({ viewport: { width: 1280, height: 800 } });
    for (const combo of interactionCombos) {
      await page.goto(`http://localhost:5199/?fixture=controls`);
      await page.waitForSelector(`[data-fixture^="controls"]`);
      await page.waitForTimeout(500);
      await applyVars(page, combo.themeVars, combo.styleVars);
      await page.waitForTimeout(100);
      const tag = `${combo.styleName}__${combo.themeName}`;

      // hover on Primary button
      await page.getByRole("button", { name: "Primary" }).hover();
      await page.waitForTimeout(150);
      await page.screenshot({ path: resolve(__dirname, `controls-hover__${tag}.png`) });
      manifest.push({ fixture: "controls", state: "hover", style: combo.styleName, theme: combo.themeName, file: `controls-hover__${tag}.png` });

      // focus-visible via keyboard (Tab lands on Default, then to Muted)
      await page.getByRole("button", { name: "Default" }).focus();
      await page.keyboard.press("Shift+Tab");
      await page.keyboard.press("Tab");
      await page.waitForTimeout(150);
      await page.screenshot({ path: resolve(__dirname, `controls-focus-visible__${tag}.png`) });
      const focusInfo = await page.evaluate(() => {
        const el = document.activeElement;
        return el ? `${el.tagName}:${el.textContent?.trim()}` : "none";
      });
      manifest.push({ fixture: "controls", state: "focus-visible", focused: focusInfo, style: combo.styleName, theme: combo.themeName, file: `controls-focus-visible__${tag}.png` });

      // open dropdown
      await page.getByRole("button", { name: /density/i }).click();
      await page.waitForTimeout(400);
      const listboxVisible = await page.getByRole("listbox").isVisible().catch(() => false);
      await page.screenshot({ path: resolve(__dirname, `controls-dropdown-open__${tag}.png`) });
      manifest.push({ fixture: "controls", state: "open-dropdown", listboxVisible, style: combo.styleName, theme: combo.themeName, file: `controls-dropdown-open__${tag}.png` });
      if (!listboxVisible) {
        const listCount = await page.getByRole("listbox").count();
        console.log(`  dropdown listbox count=${listCount}`);
      }
      await page.keyboard.press("Escape");
      await page.waitForTimeout(150);

      // collapse: defaultExpanded true -> click header to collapse
      await page.getByRole("button", { name: /section/i }).click();
      await page.waitForTimeout(200);
      await page.screenshot({ path: resolve(__dirname, `controls-collapse-collapsed__${tag}.png`) });
      manifest.push({ fixture: "controls", state: "expanded-collapse (collapsed)", style: combo.styleName, theme: combo.themeName, file: `controls-collapse-collapsed__${tag}.png` });
      await page.getByRole("button", { name: /section/i }).click();
      await page.waitForTimeout(200);

      // modal open
      await page.getByRole("button", { name: "Open modal" }).click();
      await page.waitForTimeout(250);
      const dialogVisible = await page.getByRole("dialog").isVisible().catch(() => false);
      await page.screenshot({ path: resolve(__dirname, `controls-modal-open__${tag}.png`) });
      manifest.push({ fixture: "controls", state: "modal-open", dialogVisible, style: combo.styleName, theme: combo.themeName, file: `controls-modal-open__${tag}.png` });
      await page.keyboard.press("Escape");
      await page.waitForTimeout(150);

      // invalid text field element shot
      const invalidField = page.locator("text=Value is not a valid workspace path").locator("..");
      if (await invalidField.count()) {
        await invalidField.screenshot({ path: resolve(__dirname, `controls-invalid-field__${tag}.png`) });
        manifest.push({ fixture: "controls", state: "invalid", style: combo.styleName, theme: combo.themeName, file: `controls-invalid-field__${tag}.png` });
      }

      console.log(`Part B: ${tag} done (listbox=${listboxVisible})`);
    }

    // a11y: settings fixture + controls fixture trees
    await page.goto(`http://localhost:5199/?fixture=settings`);
    await page.waitForSelector(`[data-fixture="settings"]`);
    await page.waitForTimeout(600);
    await a11ySnapshot(page, "settings");
    const settingsTabOrder = await page.evaluate(() => {
      const order = [];
      const walker = document.createTreeWalker(document.body, NodeFilter.SHOW_ELEMENT);
      while (walker.nextNode()) {
        const el = walker.currentNode;
        if (el.tabIndex >= 0 && (el.tagName === "BUTTON" || el.tagName === "INPUT" || el.tagName === "SELECT")) {
          order.push(`${el.tagName}:${(el.getAttribute("aria-label") || el.textContent || "").trim().slice(0, 40)}`);
        }
      }
      return order;
    });
    a11yFindings["settings-tab-order"] = settingsTabOrder;

    // keyboard flow: Tab through settings, capture focus visibility
    await applyVars(page, THEMES["modus-vivendi"], STYLES.neobrutal);
    await page.keyboard.press("Tab");
    await page.waitForTimeout(120);
    const focusedAfterTab = await page.evaluate(() => {
      const el = document.activeElement;
      const style = el ? getComputedStyle(el) : null;
      return el ? { tag: el.tagName, name: el.getAttribute("aria-label") || el.textContent?.trim().slice(0, 40), outline: style?.outlineStyle, outlineWidth: style?.outlineWidth } : null;
    });
    a11yFindings["settings-first-focus"] = focusedAfterTab;
    await page.screenshot({ path: resolve(__dirname, "settings-focus-visible__neobrutal__modus-vivendi.png") });
    manifest.push({ fixture: "settings", state: "focus-visible", focused: focusedAfterTab, file: "settings-focus-visible__neobrutal__modus-vivendi.png" });
    await page.close();
  }

  // ---------- Part C: splits narrow (900) + wide (1920) ----------
  {
    const page = await browser.newPage({ viewport: { width: 900, height: 700 } });
    for (const combo of COMBOS) {
      await page.goto(`http://localhost:5199/?fixture=splits`);
      await page.waitForSelector(`[data-fixture^="splits"]`);
      await page.waitForTimeout(600);
      await applyVars(page, combo.themeVars, combo.styleVars);
      await page.waitForTimeout(100);
      const filename = `splits-900__${combo.styleName}__${combo.themeName}.png`;
      await page.screenshot({ path: resolve(__dirname, filename) });
      manifest.push({ fixture: "splits", style: combo.styleName, theme: combo.themeName, file: filename, viewport: "900x700" });
    }
    await page.close();

    const wide = await browser.newPage({ viewport: { width: 1920, height: 1000 } });
    for (const combo of COMBOS) {
      await wide.goto(`http://localhost:5199/?fixture=splits`);
      await wide.waitForSelector(`[data-fixture^="splits"]`);
      await wide.waitForTimeout(600);
      await applyVars(wide, combo.themeVars, combo.styleVars);
      await wide.waitForTimeout(100);
      const filename = `splits-1920__${combo.styleName}__${combo.themeName}.png`;
      await wide.screenshot({ path: resolve(__dirname, filename) });
      manifest.push({ fixture: "splits", style: combo.styleName, theme: combo.themeName, file: filename, viewport: "1920x1000" });
    }
    await wide.close();
    console.log("Part C: splits viewports done");
  }

  await browser.close();
  writeFileSync(resolve(__dirname, "manifest.json"), JSON.stringify({ captures: manifest }, null, 2));
  writeFileSync(resolve(__dirname, "a11y-snapshots.json"), JSON.stringify(a11yFindings, null, 2));
  console.log(`\nDone: ${manifest.length} captures + a11y snapshots recorded.`);
}

main().catch((err) => {
  console.error("Capture failed:", err);
  process.exit(1);
});
