// Visual + geometry evidence for the tab's two-view chrome (Plan 118, "Model
// the tab as one workspace plus one agent, with two views").
//
// Drives the real shell over CDP (dev server, `/workspace`): the titlebar
// switcher must be present, consume the `seg` recipe family, be inert on an
// uncommitted tab with the approved reasons, and stay inside a 40px titlebar
// with zero horizontal overflow. The active/hover/focus paints are probed by
// forcing the attributes the component sets, exactly like the component
// conformance tool's state probes — so no fixture has to invent a live tab.
//
// Usage: npm run dev (default http://localhost:5199) then
//   node design-artifacts/tools/capture-tab-views.mjs
// Evidence lands in design-artifacts/screenshots/quiet-instrument-tab-views/.
import { spawn } from "node:child_process";
import {
  existsSync,
  mkdirSync,
  readdirSync,
  writeFileSync,
} from "node:fs";
import { join } from "node:path";

const ROOT = "/home/arn/Projects/clay";
const OUT = join(ROOT, "design-artifacts/screenshots/quiet-instrument-tab-views");
const BASE = process.env.CLAY_DEV_URL ?? "http://localhost:5199";
const PORT = 9337;
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

function findChrome() {
  const candidates = [
    process.env.CHROME_PATH,
    "/usr/bin/google-chrome",
    "/usr/bin/chromium",
    "/usr/bin/chromium-browser",
    join(
      process.env.HOME ?? "/root",
      ".cache/puppeteer/chrome/linux-152.0.7977.42/chrome-linux64/chrome",
    ),
  ];
  for (const cache of [
    join(process.env.HOME ?? "/root", ".cache/ms-playwright"),
    join(process.env.HOME ?? "/root", ".cache/puppeteer/chrome"),
  ]) {
    if (!existsSync(cache)) continue;
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
      `--remote-debugging-port=${PORT}`,
      "--user-data-dir=/tmp/clay-tab-views-capture-profile",
      "--no-first-run",
      "--no-default-browser-check",
      "--hide-scrollbars",
      "--window-size=1500,950",
      "about:blank",
    ],
    { stdio: "ignore", detached: true },
  ).unref();
  for (let attempt = 0; attempt < 60; attempt += 1) {
    await sleep(250);
    try {
      const list = await (await fetch(`http://127.0.0.1:${PORT}/json/list`)).json();
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

/// The switcher is a two-item `seg` in the titlebar; the probes read the real
/// element's computed paint for each state the component can be in.
const PROBE = `(async () => {
  const sw = document.querySelector("[data-viewswitch]");
  const items = sw ? [...sw.querySelectorAll("button")] : [];
  const first = items[0];
  const cs = (el) => (el ? getComputedStyle(el) : null);
  const px = (v) => (v && v.endsWith("px") ? parseFloat(v) : v);
  const rect = (el) => {
    if (!el) return null;
    const r = el.getBoundingClientRect();
    return { x: Math.round(r.x), y: Math.round(r.y), w: Math.round(r.width), h: Math.round(r.height) };
  };
  // State probes wait out the 150ms colour transition: reading immediately
  // returns the previous paint.
  const probe = (state) =>
    new Promise((resolve) => {
      if (!first) {
        resolve(null);
        return;
      }
      const prevSelected = first.getAttribute("aria-selected");
      const prevActive = first.dataset.active;
      const prevDisabled = first.disabled;
      first.disabled = state === "disabled";
      if (state === "disabled") first.setAttribute("disabled", "true");
      else first.removeAttribute("disabled");
      first.setAttribute("aria-selected", state === "selected" ? "true" : "false");
      first.dataset.active = state === "selected" ? "true" : "false";
      window.setTimeout(() => {
        const style = cs(first);
        const value = {
          background: style.backgroundColor,
          color: style.color,
          fill: style.getPropertyValue("--fill").trim(),
          fillOpacity: style.getPropertyValue("--fill-opacity").trim(),
          outlineWidth: px(style.outlineWidth),
          outlineOffset: px(style.outlineOffset),
          opacity: style.opacity,
        };
        first.setAttribute("aria-selected", prevSelected ?? "false");
        if (prevActive === undefined) delete first.dataset.active;
        else first.dataset.active = prevActive;
        if (prevDisabled) first.setAttribute("disabled", "true");
        else first.removeAttribute("disabled");
        resolve(value);
      }, 260);
    });
  // Hover is a CSS pseudo-class: probe it through the rule the host stylesheet
  // states, not by dispatching a synthetic pointer event.
  const hoverRule = [...document.styleSheets]
    .flatMap((sheet) => { try { return [...sheet.cssRules] } catch { return [] } })
    .find((rule) => rule.selectorText?.includes(":hover:not(:disabled)"));
  return {
    present: !!sw,
    dataViewswitch: sw?.dataset.viewswitch ?? null,
    segRoot: sw
      ? sw.getAttribute("data-clay-component") + "/" + sw.getAttribute("data-clay-slot")
      : null,
    itemRoles: items.map((item) => item.getAttribute("role")),
    items: items.map((item) => ({
      text: item.textContent,
      segAttribute:
        item.getAttribute("data-clay-component") + "/" + item.getAttribute("data-clay-slot"),
      disabled: item.disabled,
      selected: item.getAttribute("aria-selected"),
      title: item.title,
    })),
    rect: rect(sw),
    titlebar: rect(document.querySelector("header")),
    overflow: document.documentElement.scrollWidth - document.documentElement.clientWidth,
    paints: {
      rest: await probe("rest"),
      selected: await probe("selected"),
      disabled: await probe("disabled"),
    },
    hoverRuleDeclaresSurfaceHover: !!hoverRule,
  };
})()`;

const report = { base: BASE, viewport: { w: 1500, h: 950 }, checks: {}, probe: null };

const target = await openCdp();
const cdp = connect(target);
await cdp.ready;
await cdp.send("Page.enable");
await cdp.send("Page.navigate", { url: `${BASE}/workspace` });
await sleep(3000);

const result = await cdp.send("Runtime.evaluate", {
  expression: PROBE,
  returnByValue: true,
  awaitPromise: true,
});
report.probe = result.result.value;

const screenshot = await cdp.send("Page.captureScreenshot", { format: "png" });
mkdirSync(OUT, { recursive: true });
writeFileSync(join(OUT, "shell-tab-views-1500.png"), Buffer.from(screenshot.data, "base64"));

const probe = report.probe;
report.checks = {
  switcherPresent: probe.present === true,
  consumesSegRecipes:
    probe.segRoot === "seg/root" &&
    probe.items.every((item) => item.segAttribute === "seg/item"),
  approvedItemSemantics: probe.itemRoles.join(",") === "tab,tab",
  inertWhenUncommitted:
    probe.dataViewswitch === "empty" &&
    probe.items.every((item) => item.disabled) &&
    probe.items[0]?.title === "Pick a workspace first" &&
    probe.items[1]?.title === "Pick an agent first",
  titlebarHeight:
    probe.titlebar !== null && probe.titlebar.h >= 38 && probe.titlebar.h <= 44,
  noHorizontalOverflow: probe.overflow === 0,
  // The selected item paints the accent tint (background) and the accent text,
  // and takes the theme's role values, not a literal.
  selectedPaintIsAccentTint:
    probe.paints.selected?.fill !== probe.paints.rest?.fill &&
    probe.paints.selected?.fillOpacity === "0.15" &&
    probe.paints.selected?.background !== probe.paints.rest?.background &&
    probe.paints.selected?.color !== probe.paints.rest?.color,
  disabledPaintIsDimmed:
    Number(probe.paints.disabled?.opacity) <= 0.5 &&
    probe.paints.disabled?.color !== probe.paints.rest?.color,
  hoverRulePresent: probe.hoverRuleDeclaresSurfaceHover === true,
};
report.passed = Object.values(report.checks).every(Boolean);
writeFileSync(join(OUT, "report.json"), JSON.stringify(report, null, 2));
console.log(JSON.stringify(report, null, 2));
cdp.close();
process.exit(report.passed ? 0 : 1);
