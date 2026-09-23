// One-shot capture + geometry verification for the overlay family
// (Plan 118, "Adopt the approved composition in the Command Centre, Package
// Workspace and overlay family").
//
// Drives the dev app over CDP, injects each shipped theme's `--clay-*` roles
// the way `ResolvedUiTheme::base_color` does, and records the family's
// measurable claims: the palette sheet (one ring, bounded results, key-hint
// foot), the menu origins' popover surface, the modal's head/body/foot with
// hairline separations, the tooltip, and the reduced-motion /
// reduced-transparency fallbacks — with zero horizontal overflow.
import { spawn } from "node:child_process";
import {
  existsSync,
  mkdirSync,
  readFileSync,
  readdirSync,
  writeFileSync,
} from "node:fs";
import { join } from "node:path";

const ROOT = "/home/arn/Projects/clay";
const OUT = join(ROOT, "design-artifacts/screenshots/quiet-instrument-overlays");
const BASE = process.env.CLAY_DEV_URL ?? "http://localhost:5199";
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

// Mirrors src/shell/theme.rs BASE projection (subset the surfaces use).
const BASE_PROJECTION = {
  "surface.main": "shellBg",
  "surface.panel": "panelBg",
  "surface.list": "panelBg",
  "surface.overlay": "panelBg",
  "surface.control": "statusBg",
  "surface.badge": "statusBg",
  "surface.kbd": "statusBg",
  "surface.hover": "selection",
  "surface.active": "selection",
  "surface.selected": "selection",
  "surface.scrollbar": "scrollbar",
  "surface.scrollbar.track": "scrollbarTrack",
  "text.primary": "text",
  "text.disabled": "placeholder",
  "text.icon": "placeholder",
  "accent.primary": "caret",
  "focus.ring": "caret",
  "border.focus": "caret",
  "border.hairline": "scrollbar",
  "border.subtle": "scrollbar",
  "border.strong": "scrollbar",
  "diagnostic.error": "diagnosticError",
  "diagnostic.warning": "diagnosticWarning",
  "diagnostic.info": "diagnosticInfo",
};

const THEMES = [
  "theme-modus-operandi",
  "theme-modus-vivendi",
  "theme-gruvbox-material-dark",
  "theme-gruvbox-material-light",
];
const WIDTHS = [1500, 1024, 960];

function themeVariables(dir) {
  const manifest = JSON.parse(
    readFileSync(join(ROOT, "packages", dir, "package.json"), "utf8"),
  );
  const styles = Object.fromEntries(
    manifest.clay.contributions.textStyles.map((e) => [e.token, e.color]),
  );
  const vars = {};
  for (const [token, field] of Object.entries(BASE_PROJECTION)) {
    if (styles[field])
      vars[`--clay-${token.replaceAll(".", "-")}`] = styles[field];
  }
  for (const entry of manifest.clay.contributions.designTokens ?? []) {
    vars[`--clay-${entry.token.replaceAll(".", "-")}`] = entry.value;
  }
  return { name: dir.replace(/^theme-/, ""), vars };
}

function findChrome() {
  const candidates = [
    process.env.CHROME_PATH,
    "/usr/bin/google-chrome",
    "/usr/bin/chromium",
    "/usr/bin/chromium-browser",
  ];
  const cache = join(process.env.HOME ?? "/root", ".cache", "ms-playwright");
  if (existsSync(cache)) {
    for (const entry of readdirSync(cache)) {
      candidates.push(
        join(cache, entry, "chrome-linux64/chrome"),
        join(cache, entry, "chrome-linux/chrome"),
      );
    }
  }
  const found = candidates.find((p) => p && existsSync(p));
  if (!found) throw new Error("no Chrome/Chromium found");
  return found;
}

async function openCdp() {
  spawn(
    findChrome(),
    [
      "--headless=new",
      "--no-sandbox",
      "--disable-gpu",
      "--remote-debugging-port=9336",
      "--user-data-dir=/tmp/clay-overlays-capture-profile",
      "--no-first-run",
      "--no-default-browser-check",
      "--hide-scrollbars",
      "about:blank",
    ],
    { stdio: "ignore", detached: true },
  ).unref();
  for (let attempt = 0; attempt < 60; attempt += 1) {
    await sleep(250);
    try {
      const list = await (await fetch("http://127.0.0.1:9336/json/list")).json();
      const page = list.find((t) => t.type === "page");
      if (page?.webSocketDebuggerUrl) return page.webSocketDebuggerUrl;
    } catch {
      /* not up yet */
    }
  }
  throw new Error("chrome did not expose a debug target");
}

function connect(url) {
  const ws = new WebSocket(url);
  let id = 0;
  const pending = new Map();
  ws.addEventListener("message", (event) => {
    const msg = JSON.parse(event.data);
    if (msg.id && pending.has(msg.id)) {
      const { resolve, reject } = pending.get(msg.id);
      pending.delete(msg.id);
      if (msg.error) reject(new Error(JSON.stringify(msg.error)));
      else resolve(msg.result);
    }
  });
  const ready = new Promise((resolve) => ws.addEventListener("open", resolve));
  return {
    ready,
    send(method, params = {}) {
      id += 1;
      ws.send(JSON.stringify({ id, method, params }));
      return new Promise((resolve, reject) =>
        pending.set(id, { resolve, reject }),
      );
    },
    close: () => ws.close(),
  };
}

const HELPERS = `
  const cs = (el) => (el ? getComputedStyle(el) : null);
  const px = (v) => (v && v.endsWith("px") ? parseFloat(v) : v);
  const rect = (el) => {
    if (!el) return null;
    const r = el.getBoundingClientRect();
    return { x: Math.round(r.x), y: Math.round(r.y), w: Math.round(r.width), h: Math.round(r.height) };
  };
  const overflow = () =>
    document.documentElement.scrollWidth - document.documentElement.clientWidth;
`;

const MEASURE_PALETTE = `(() => {
  ${HELPERS}
  // Plan 125: the shell's one transient surface is the composer's palette
  // (data-testid="command-palette"), not the retired window sheet.
  const sheet = document.querySelector('[data-testid="command-palette"]');
  const input = sheet?.querySelector("input");
  const results = sheet?.querySelector('[class*="palList"]');
  const head = sheet?.querySelector('[class*="palHead"]');
  const foot = sheet?.querySelector('[class*="foot"]');
  const rows = [...(sheet?.querySelectorAll('[role="option"]') ?? [])];
  const kbds = [...(sheet?.querySelectorAll("kbd") ?? [])];
  const s = cs(sheet);
  input?.focus();
  const focused = cs(sheet);
  const active = cs(input);
  return {
    overflow: overflow(),
    sheet: rect(sheet),
    radius: px(s?.borderTopLeftRadius),
    border: px(s?.borderTopWidth) + " " + s?.borderTopStyle,
    background: s?.backgroundColor,
    shadow: s?.boxShadow === "none" ? "none" : "present",
    headBorderBottom: px(cs(head)?.borderBottomWidth),
    footBorderTop: px(cs(foot)?.borderTopWidth),
    resultsOverflow: cs(results)?.overflowY ?? null,
    resultsBounded:
      results != null &&
      cs(results).maxHeight !== "none" &&
      results.clientHeight <= results.parentElement.clientHeight + 1,
    rows: rows.length,
    rowRadius: rows.length ? px(cs(rows[0]).borderTopLeftRadius) : null,
    rowDetailMono: rows.length
      ? cs(rows[0].querySelector('[class*="rowDetail"], span:last-child'))
          ?.fontFamily?.includes("mono") ?? false
      : null,
    kbds: kbds.length,
    hints: [...(sheet?.querySelectorAll('[class*="hint"]') ?? [])]
      .map((h) => h.textContent.trim())
      .filter((text, index, all) => text && all.indexOf(text) === index),
    count: sheet?.querySelector("output")?.textContent ?? null,
    promptLabel: sheet?.querySelector('[class*="prompt"]')?.textContent ?? null,
    dialogName: document.querySelector('[role="dialog"]')?.getAttribute("aria-label") ?? null,
    heading: !!document.querySelector('[role="dialog"] h2'),
    focusRing: {
      borderColor: focused?.borderTopColor,
      ringExpected: cs(sheet)?.borderTopColor,
      sheetHalo: focused?.boxShadow === "none" ? "none" : "present",
      // The sheet declares no focus state of its own: the composer box it
      // answers to is the boundary (plan 124), so focusing anything inside it
      // must leave its border and shadow untouched.
      ringStable:
        s?.boxShadow === focused?.boxShadow &&
        s?.borderTopColor === focused?.borderTopColor,
      inputOutline: active?.outlineStyle,
      accent: getComputedStyle(document.documentElement)
        .getPropertyValue("--clay-accent-primary")
        .trim(),
    },
    listPresent: !!sheet?.querySelector('[role="listbox"]'),
    emptyText: sheet?.querySelector('[class*="empty"]')?.textContent ?? null,
    scopeControls:
      (sheet?.querySelectorAll('[role="radiogroup"]').length ?? 0) +
      (sheet?.querySelectorAll('[role="tablist"]').length ?? 0),
  };
})()`;

const MEASURE_MODAL = `(() => {
  ${HELPERS}
  const dialog = document.querySelector('[role="dialog"]');
  const d = cs(dialog);
  const foot = dialog?.querySelector('[class*="foot"]');
  const title = dialog?.querySelector("h2");
  const scrim = dialog?.parentElement?.parentElement;
  const buttons = [...(foot?.querySelectorAll("button") ?? [])];
  return {
    overflow: overflow(),
    dialog: rect(dialog),
    radius: px(d?.borderTopLeftRadius),
    border: px(d?.borderTopWidth) + " " + d?.borderTopStyle,
    shadow: d?.boxShadow === "none" ? "none" : "present",
    background: d?.backgroundColor,
    transition: d?.transitionDuration,
    timing: d?.transitionTimingFunction,
    titleBorderBottom: px(cs(title)?.borderBottomWidth),
    titleSize: cs(title)?.fontSize,
    footBorderTop: px(cs(foot)?.borderTopWidth),
    footJustify: cs(foot)?.justifyContent,
    actions: buttons.map((b) => b.textContent.trim()),
    closeButton: !!dialog?.querySelector('[aria-label="Close"]'),
    scrimBackdrop: cs(scrim)?.backdropFilter ?? null,
    scrimBackground: cs(scrim)?.backgroundColor ?? null,
  };
})()`;

const MEASURE_TOOLTIP = `(() => {
  ${HELPERS}
  const tip = document.querySelector('[role="tooltip"]');
  const t = cs(tip);
  return {
    overflow: overflow(),
    tooltip: rect(tip),
    radius: px(t?.borderTopLeftRadius),
    shadow: t?.boxShadow === "none" ? "none" : "present",
    background: t?.backgroundColor,
    text: tip?.textContent ?? null,
  };
})()`;

const REDUCED_MOTION = `(() => {
  ${HELPERS}
  const dialog = document.querySelector('[role="dialog"]');
  const sheet = document.querySelector('[data-testid="command-palette"]');
  return {
    dialogTransition: cs(dialog)?.transitionDuration ?? null,
    dialogTransform: cs(dialog)?.transform ?? null,
    sheetTransition: cs(sheet)?.transitionDuration ?? null,
  };
})()`;

const REDUCED_TRANSPARENCY = `(() => {
  ${HELPERS}
  const scrim = document.querySelector('[role="dialog"]')?.parentElement?.parentElement;
  const dialog = document.querySelector('[role="dialog"]');
  const panel = document.querySelector('[data-clay-component="panel"]');
  const hasAlpha = (value) =>
    /rgba\(|\)\s*\/|\/\s*0?\.\d+/.test(value ?? "");
  return {
    scrimBackdrop: cs(scrim)?.backdropFilter ?? null,
    scrimBackground: cs(scrim)?.backgroundColor ?? null,
    scrimOpaque: !hasAlpha(cs(scrim)?.backgroundColor),
    dialogBackground: cs(dialog)?.backgroundColor ?? null,
    dialogOpaque: !hasAlpha(cs(dialog)?.backgroundColor),
    panelBackground: cs(panel)?.backgroundColor ?? null,
    panelOpaque: !hasAlpha(cs(panel)?.backgroundColor),
    panelBackdrop: cs(panel)?.backdropFilter ?? null,
  };
})()`;

async function injectTheme(cdp, theme) {
  await cdp.send("Runtime.evaluate", {
    expression: `(() => { const r = document.documentElement.style; ${Object.entries(
      theme.vars,
    )
      .map(
        ([name, value]) =>
          `r.setProperty(${JSON.stringify(name)}, ${JSON.stringify(value)});`,
      )
      .join("")} return true; })()`,
  });
}

async function shoot(cdp, name) {
  const shot = await cdp.send("Page.captureScreenshot", { format: "png" });
  writeFileSync(join(OUT, `${name}.png`), Buffer.from(shot.data, "base64"));
}

/** React Aria ignores synthetic `.click()`; dispatch real pointer events. */
/** Real pointer hover (React Aria's tooltip opens on hover after its delay). */
async function hover(cdp, selector) {
  const { result } = await cdp.send("Runtime.evaluate", {
    expression: `(() => {
      const el = document.querySelector(${JSON.stringify(selector)});
      if (!el) return null;
      const r = el.getBoundingClientRect();
      return { x: r.x + r.width / 2, y: r.y + r.height / 2 };
    })()`,
    returnByValue: true,
  });
  if (!result?.value) return false;
  const { x, y } = result.value;
  for (const [dx, dy] of [
    [0, -40],
    [0, 0],
    [1, 1],
  ]) {
    await cdp.send("Input.dispatchMouseEvent", {
      type: "mouseMoved",
      x: x + dx,
      y: y + dy,
      buttons: 0,
      pointerType: "mouse",
    });
    await sleep(200);
  }
  return true;
}

/** Press the first button whose text contains `label` (fixture order varies). */
async function pressText(cdp, label) {
  await cdp.send("Runtime.evaluate", {
    expression: `(() => {
      const el = [...document.querySelectorAll("button")].find((b) =>
        (b.textContent ?? "").includes(${JSON.stringify(label)}),
      );
      if (!el) return false;
      const box = el.getBoundingClientRect();
      const opts = { bubbles: true, cancelable: true, clientX: box.x + box.width / 2, clientY: box.y + box.height / 2, pointerId: 1, pointerType: "mouse", isPrimary: true, button: 0 };
      el.dispatchEvent(new PointerEvent("pointerdown", opts));
      el.dispatchEvent(new PointerEvent("pointerup", opts));
      el.dispatchEvent(new MouseEvent("click", opts));
      return true;
    })()`,
    returnByValue: true,
  });
}

async function press(cdp, selector) {
  await cdp.send("Runtime.evaluate", {
    expression: `(() => {
      const el = document.querySelector(${JSON.stringify(selector)});
      if (!el) return false;
      const box = el.getBoundingClientRect();
      const opts = { bubbles: true, cancelable: true, clientX: box.x + box.width / 2, clientY: box.y + box.height / 2, pointerId: 1, pointerType: "mouse", isPrimary: true, button: 0 };
      el.dispatchEvent(new PointerEvent("pointerdown", opts));
      el.dispatchEvent(new PointerEvent("pointerup", opts));
      el.dispatchEvent(new MouseEvent("click", opts));
      return true;
    })()`,
    returnByValue: true,
  });
}

/** Evaluate a measurement expression and surface a thrown error as data. */
async function measure(cdp, expression) {
  const { result, exceptionDetails } = await cdp.send("Runtime.evaluate", {
    expression,
    returnByValue: true,
  });
  if (exceptionDetails) {
    return {
      error:
        exceptionDetails.exception?.description ??
        exceptionDetails.text ??
        "evaluation failed",
    };
  }
  return result.value ?? {};
}

async function scene(cdp, report, { name, url, width = 1500, expression, theme, probe }) {
  await cdp.send("Emulation.setDeviceMetricsOverride", {
    width,
    height: 950,
    deviceScaleFactor: 1,
    mobile: false,
  });
  await cdp.send("Page.navigate", { url: `${BASE}/${url}` });
  await sleep(1300);
  await injectTheme(cdp, theme);
  await sleep(350);
  if (probe) await probe();
  const measured = await measure(cdp, expression);
  await shoot(cdp, `${name}__${theme.name}${width === 1500 ? "" : `__${width}`}`);
  const row = { scene: name, theme: theme.name, width, ...measured };
  report.matrix.push(row);
  return row;
}

async function main() {
  mkdirSync(OUT, { recursive: true });
  const cdp = connect(await openCdp());
  await cdp.ready;
  await cdp.send("Page.enable");
  await cdp.send("Runtime.enable");
  const report = { matrix: [], probes: [], failures: [] };
  const themes = THEMES.map((dir) => themeVariables(dir));

  for (const theme of themes) {
    // 1. The palette, at three widths.
    for (const width of WIDTHS) {
      await scene(cdp, report, {
        name: "palette",
        url: "?fixture=command-centre",
        width,
        expression: MEASURE_PALETTE,
        theme,
      });
    }
    // 2. The empty result set.
    await scene(cdp, report, {
      name: "palette-empty",
      url: "?fixture=command-centre-empty",
      expression: MEASURE_PALETTE,
      theme,
    });
    // 3. The path scope (the server's own prompt).
    await scene(cdp, report, {
      name: "palette-path",
      url: "?fixture=path-browser",
      expression: MEASURE_PALETTE,
      theme,
    });
    // 4. The modal sheet: head, scrolling body, hairline actions foot.
    await scene(cdp, report, {
      name: "modal",
      url: "?fixture=controls",
      expression: MEASURE_MODAL,
      theme,
      probe: async () => {
        await pressText(cdp, "Open modal");
        await sleep(500);
      },
    });
    // 6. The tooltip on a real control (the editor chrome's Reload button).
    await scene(cdp, report, {
      name: "tooltip",
      url: "?fixture=editor",
      expression: MEASURE_TOOLTIP,
      theme,
      probe: async () => {
        await hover(cdp, '[aria-label="Reload"]');
        // React Aria opens the tooltip after its hover delay.
        await sleep(2200);
      },
    });
  }

  // Probes: reduced motion, then reduced transparency, on the palette+modal.
  await cdp.send("Emulation.setEmulatedMedia", {
    features: [{ name: "prefers-reduced-motion", value: "reduce" }],
  });
  await cdp.send("Page.navigate", { url: `${BASE}/?fixture=command-centre` });
  await sleep(1300);
  const motion = await measure(cdp, REDUCED_MOTION);
  await shoot(cdp, "reduced-motion");
  report.probes.push({ name: "prefers-reduced-motion", ...motion });

  await cdp.send("Emulation.setEmulatedMedia", { features: [] });
  await cdp.send("Page.navigate", { url: `${BASE}/?fixture=controls` });
  await sleep(1300);
  await pressText(cdp, "Open modal");
  await sleep(600);
  await cdp.send("Emulation.setEmulatedMedia", {
    features: [{ name: "prefers-reduced-transparency", value: "reduce" }],
  });
  await sleep(400);
  const transparency = await measure(cdp, REDUCED_TRANSPARENCY);
  await shoot(cdp, "reduced-transparency");
  report.probes.push({ name: "prefers-reduced-transparency", ...transparency });

/** `rgb(0, 49, 169)` and `#0031a9` are the same colour. */
function rgb(value) {
  const hex = /^#([0-9a-f]{6})$/i.exec(value ?? "");
  if (hex) {
    const int = parseInt(hex[1], 16);
    return `${(int >> 16) & 255},${(int >> 8) & 255},${int & 255}`;
  }
  const fn = /rgba?\(([^)]+)\)/.exec(value ?? "");
  return fn
    ? fn[1]
        .split(",")
        .slice(0, 3)
        .map((channel) => channel.trim())
        .join(",")
    : null;
}

  const failures = [];
  const palette = report.matrix.filter((r) => r.scene === "palette");
  for (const run of report.matrix) {
    if (run.error) failures.push(`${run.scene}/${run.theme}@${run.width}: ${run.error}`);
    if (run.overflow > 0) failures.push(`${run.scene}/${run.theme}@${run.width}: overflow ${run.overflow}`);
    if (run.width && run.sheet && run.sheet.x + run.sheet.w > run.width + 1)
      failures.push(`${run.scene}/${run.theme}@${run.width}: sheet clipped ${JSON.stringify(run.sheet)}`);
  }
  for (const run of palette) {
    if (run.radius !== 16)
      failures.push(`${run.theme}@${run.width}: palette radius ${run.radius}`);
    if (run.shadow !== "present")
      failures.push(`${run.theme}@${run.width}: palette has no overlay shadow`);
    if (run.border !== "1 solid")
      failures.push(`${run.theme}@${run.width}: palette border ${run.border}`);
    if (run.headBorderBottom !== 1 || run.footBorderTop !== 1)
      failures.push(
        `${run.theme}@${run.width}: head/foot hairlines ${run.headBorderBottom}/${run.footBorderTop}`,
      );
    if (run.resultsOverflow !== "auto")
      failures.push(`${run.theme}@${run.width}: row-list overflow ${run.resultsOverflow}`);
    if (run.rows < 1) failures.push(`${run.theme}@${run.width}: no rows`);
    if (run.rowRadius !== 8)
      failures.push(`${run.theme}@${run.width}: row radius ${run.rowRadius}`);
    if (run.kbds < 3)
      failures.push(`${run.theme}@${run.width}: key hints ${run.kbds}`);
    if (!run.count?.includes("result"))
      failures.push(`${run.theme}@${run.width}: count ${run.count}`);
    if (run.scopeControls !== 0)
      failures.push(`${run.theme}@${run.width}: fabricated scopes ${run.scopeControls}`);
    if (run.heading) failures.push(`${run.theme}@${run.width}: palette paints a second head`);
    if (!run.focusRing.ringStable)
      failures.push(
        `${run.theme}@${run.width}: the sheet drew a ring of its own (the composer box is the boundary)`,
      );
  }
  const empty = report.matrix.find((r) => r.scene === "palette-empty");
  if (!empty?.emptyText?.includes("No commands"))
    failures.push("empty scene: no empty message");
  if (empty?.listPresent) failures.push("empty scene: renders a list");
  for (const run of palette) {
    if (run.promptLabel !== "Commands")
      failures.push(`${run.theme}@${run.width}: prompt label ${run.promptLabel}`);
  }
  const path = report.matrix.find((r) => r.scene === "palette-path");
  if (path?.promptLabel !== "Browse workspace")
    failures.push(`path scope: prompt label ${path?.promptLabel}`);
  for (const run of report.matrix.filter((r) => r.scene === "modal")) {
    if (!run.dialog) failures.push(`${run.theme}: modal did not open`);
    if (run.radius !== 16) failures.push(`${run.theme}: dialog radius ${run.radius}`);
    if (run.shadow !== "present") failures.push(`${run.theme}: dialog has no overlay shadow`);
    if (run.titleBorderBottom !== 1)
      failures.push(`${run.theme}: modal head hairline ${run.titleBorderBottom}`);
    if (run.footBorderTop !== 1)
      failures.push(`${run.theme}: modal foot hairline ${run.footBorderTop}`);
    if (run.footJustify !== "space-between")
      failures.push(`${run.theme}: foot justify ${run.footJustify}`);
    if (run.actions?.[0] !== "Cancel")
      failures.push(`${run.theme}: foot order ${JSON.stringify(run.actions)}`);
    if (!run.closeButton) failures.push(`${run.theme}: no close affordance`);
    if (!/blur\(3px\)/.test(run.scrimBackdrop ?? ""))
      failures.push(`${run.theme}: scrim blur ${run.scrimBackdrop}`);
  }
  for (const run of report.matrix.filter((r) => r.scene === "tooltip")) {
    if (!run.tooltip) failures.push(`${run.theme}: no tooltip`);
    if (run.radius !== 8) failures.push(`${run.theme}: tooltip radius ${run.radius}`);
    if (run.shadow !== "present") failures.push(`${run.theme}: tooltip has no pop shadow`);
  }
  const motionProbe = report.probes.find((p) => p.name === "prefers-reduced-motion");
  const seconds = parseFloat(motionProbe?.sheetTransition ?? "1");
  if (!Number.isFinite(seconds) || seconds > 0.0001)
    failures.push(`reduced motion: palette transition ${motionProbe?.sheetTransition}`);
  const transparencyProbe = report.probes.find(
    (p) => p.name === "prefers-reduced-transparency",
  );
  if (transparencyProbe?.error)
    failures.push(`reduced transparency: ${transparencyProbe.error}`);
  if (/blur\((?!0px)/.test(transparencyProbe?.scrimBackdrop ?? ""))
    failures.push(`reduced transparency: scrim blur ${transparencyProbe?.scrimBackdrop}`);
  if (!transparencyProbe?.scrimOpaque)
    failures.push(`reduced transparency: scrim ${transparencyProbe?.scrimBackground}`);
  if (!transparencyProbe?.dialogOpaque)
    failures.push(`reduced transparency: dialog ${transparencyProbe?.dialogBackground}`);

  report.failures = failures;
  writeFileSync(join(OUT, "report.json"), `${JSON.stringify(report, null, 1)}\n`);
  console.log(JSON.stringify({ runs: report.matrix.length, failures }, null, 1));
  for (const run of report.matrix)
    console.log(
      `${run.scene.padEnd(13)} ${run.theme.padEnd(22)} ${String(run.width).padStart(4)}  ` +
        `radius ${run.radius ?? run.tooltip?.w} border ${run.border ?? "-"} shadow ${run.shadow ?? "-"} ` +
        `head/foot ${run.headBorderBottom ?? run.titleBorderBottom ?? "-"}/${run.footBorderTop ?? "-"} ` +
        `rows ${run.rows ?? "-"} kbds ${run.kbds ?? "-"} count ${run.count ?? run.text ?? run.actions?.join("|") ?? "-"} overflow ${run.overflow}`,
    );
  for (const probe of report.probes)
    console.log(`${probe.name}: ${JSON.stringify(probe)}`);
  cdp.close();
  process.exit(failures.length ? 1 : 0);
}

main().catch((error) => {
  console.error(error);
  process.exit(2);
});
