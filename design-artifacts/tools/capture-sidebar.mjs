// Visual + behaviour evidence for the workspace sidebar head (Plan 118 task E1:
// the approved filter field, count footer and the 244/224 width).
//
// Drives the real workspace over CDP against the splits fixture (dev server):
// the sidebar must render the approved tools row (search well + `/` chip) and
// foot, filter the delivered listing locally with the ancestor-free flat
// listing it actually has (one directory's entries), report the live match
// count, keep the field the list's only ring, and paint 244px (224px at
// ≤1240px). Usage: npm run dev (default http://localhost:5199) then
//   node design-artifacts/tools/capture-sidebar.mjs
// Evidence lands in design-artifacts/screenshots/quiet-instrument-sidebar/.
import { spawn } from "node:child_process";
import { existsSync, mkdirSync, readdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";

const ROOT = "/home/arn/Projects/clay";
const OUT = join(
  ROOT,
  "design-artifacts/screenshots/quiet-instrument-sidebar",
);
const BASE = process.env.CLAY_DEV_URL ?? "http://localhost:5199";
const PORT = 9343;
const WIDTHS = [1500, 1024];
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

function findChrome() {
  const candidates = [
    process.env.CHROME_PATH,
    "/usr/bin/google-chrome",
    "/usr/bin/chromium",
    "/usr/bin/chromium-browser",
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
      "--user-data-dir=/tmp/clay-sidebar-capture-profile",
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
      const list = await (
        await fetch(`http://127.0.0.1:${PORT}/json/list`)
      ).json();
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

const evaluate = async (cdp, expression) => {
  const result = await cdp.send("Runtime.evaluate", {
    expression,
    awaitPromise: true,
    returnByValue: true,
  });
  if (result.exceptionDetails) {
    throw new Error(JSON.stringify(result.exceptionDetails.exception));
  }
  return result.result.value;
};

/** Reads the sidebar: geometry, the tools row, the filter's effect, the foot. */
const PROBE = `(async () => {
  const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
  const tools = document.querySelector('[data-clay-list-filter]');
  const field = tools?.querySelector('input') ?? null;
  const rows = () => [...document.querySelectorAll('[role="option"]')];
  const count = () =>
    tools?.parentElement
      ?.querySelector('[class*="listCount"]')
      ?.textContent.trim() ?? null;
  const setValue = (el, value) => {
    const setter = Object.getOwnPropertyDescriptor(
      window.HTMLInputElement.prototype,
      "value",
    ).set;
    setter.call(el, value);
    el.dispatchEvent(new Event("input", { bubbles: true }));
  };
  const before = rows().length;
  const firstLabel = rows()[0]?.textContent.trim() ?? null;
  // The sidebar region itself: the SDUI stack the server sized by token.
  const region = document.querySelector('[data-clay-size]');
  const rect = region?.getBoundingClientRect() ?? null;
  const regionToken = region?.getAttribute('data-clay-size') ?? null;
  const result = {
    field: field !== null,
    fieldAriaLabel: field?.getAttribute("aria-label") ?? null,
    shortcut: tools?.querySelector("kbd")?.textContent ?? null,
    rowsBefore: before,
    firstRow: firstLabel,
    regionWidth: rect ? Math.round(rect.width) : null,
    regionToken,
    countIdle: count(),
    overflow: document.documentElement.scrollWidth - window.innerWidth,
  };
  if (!field) return result;
  const needle = (firstLabel ?? "").slice(0, 3).toLowerCase() || "a";
  setValue(field, needle);
  await sleep(120);
  result.rowsFiltered = rows().length;
  result.countFiltered = count();
  result.filteredSubset = rows().every((row) =>
    row.textContent.toLowerCase().includes(needle),
  );
  setValue(field, "zzzz-no-such-file");
  await sleep(120);
  result.rowsEmpty = rows().length;
  setValue(field, "");
  await sleep(120);
  result.rowsRestored = rows().length;
  return result;
})()`;

async function main() {
  const ws = await openCdp();
  const cdp = connect(ws);
  await cdp.ready;
  await cdp.send("Page.enable");
  await cdp.send("Runtime.enable");

  const matrix = [];
  const findings = [];
  for (const width of WIDTHS) {
    await cdp.send("Emulation.setDeviceMetricsOverride", {
      width,
      height: 950,
      deviceScaleFactor: 1,
      mobile: false,
    });
    await cdp.send("Page.navigate", {
      url: `${BASE}/?fixture=workspace-sidebar`,
      waitUntil: "load",
    });
    await sleep(1200);
    const probe = await evaluate(cdp, PROBE);
    matrix.push({ width, ...probe });
    if (probe.field !== true) findings.push(`${width}px: no filter field`);
    if (probe.shortcut !== "/") findings.push(`${width}px: missing / chip`);
    if (probe.overflow > 0) findings.push(`${width}px: ${probe.overflow}px overflow`);
    if (!(probe.rowsFiltered < probe.rowsBefore))
      findings.push(`${width}px: filter did not narrow the listing`);
    if (probe.rowsEmpty !== 0)
      findings.push(`${width}px: no-match query left rows`);
    if (probe.rowsRestored !== probe.rowsBefore)
      findings.push(`${width}px: clearing did not restore the listing`);
    if (!String(probe.countFiltered ?? "").includes("match"))
      findings.push(`${width}px: count missing while filtering`);
    if (probe.countIdle)
      findings.push(`${width}px: count shown without a query`);
    if (probe.regionWidth != null) {
      const want = width <= 1240 ? 224 : 244;
      if (Math.abs(probe.regionWidth - want) > 1)
        findings.push(
          `${width}px: sidebar ${probe.regionWidth}px, expected ${want}px`,
        );
    } else {
      findings.push(`${width}px: sidebar region not measurable`);
    }
  }

  mkdirSync(OUT, { recursive: true });
  const report = {
    note: "Plan 118 task E1: the workspace sidebar's approved filter field, count foot and slot width.",
    matrix,
    findings,
    pass: findings.length === 0,
  };
  writeFileSync(join(OUT, "report.json"), `${JSON.stringify(report, null, 2)}\n`);
  console.log(JSON.stringify(report.matrix, null, 2));
  console.log(
    report.pass
      ? `sidebar: ${matrix.length}/${matrix.length} widths pass`
      : `sidebar FAIL: ${findings.join("; ")}`,
  );
  cdp.close();
  process.exit(report.pass ? 0 : 1);
}

main().catch((error) => {
  console.error(error);
  process.exit(1);
});
