// Visual + behaviour evidence for the agent Files tab (Plan 118 task 36: the
// Files tab is the session's file history, not a second editor).
//
// Drives the real panel over CDP against the agent fixture's conversation
// state (dev server): the tab must list the session's own file records newest
// first on the `sessionRow` family, tone each mark by its role, filter by path
// and count what is visible. The view switch a row triggers is tab chrome and
// is covered by the shell-level tests (frontend/src/shell/WorkspacePanes.test.tsx).
//
// Usage: npm run dev (default http://localhost:5199) then
//   node design-artifacts/tools/capture-agent-files.mjs
// Evidence lands in design-artifacts/screenshots/quiet-instrument-agent-files/.
import { spawn } from "node:child_process";
import { existsSync, mkdirSync, readdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";

const ROOT = "/home/arn/Projects/clay";
const OUT = join(
  ROOT,
  "design-artifacts/screenshots/quiet-instrument-agent-files",
);
const BASE = process.env.CLAY_DEV_URL ?? "http://localhost:5199";
const PORT = 9341;
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
      "--user-data-dir=/tmp/clay-agent-files-capture-profile",
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

/// Reads the panel: rows (mark, basename, directory, role, paint), the filter's
/// effect on the list and the count chip, and the page's overflow.
const PROBE = `(async () => {
  const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
  const click = (el) => {
    el.dispatchEvent(new MouseEvent("pointerdown", { bubbles: true }));
    el.dispatchEvent(new MouseEvent("mousedown", { bubbles: true }));
    el.dispatchEvent(new MouseEvent("mouseup", { bubbles: true }));
    el.click();
  };
  const tabs = [...document.querySelectorAll('[role="tab"]')];
  const filesTab = tabs.find((t) => t.textContent.trim() === "Files");
  if (!filesTab) return { error: "no Files tab" };
  if (filesTab.getAttribute("aria-selected") !== "true") {
    click(filesTab);
    await sleep(120);
  }
  const listSelector = 'ul[aria-label="Files this session has touched"]';
  const rowSelector = listSelector + " button";
  const px = (v) => (v && v.endsWith("px") ? parseFloat(v) : v);
  const rect = (el) => {
    if (!el) return null;
    const r = el.getBoundingClientRect();
    return {
      x: Math.round(r.x),
      y: Math.round(r.y),
      w: Math.round(r.width),
      h: Math.round(r.height),
    };
  };
  const row = (el) => {
    const style = getComputedStyle(el);
    return {
      family:
        el.getAttribute("data-clay-component") +
        "/" +
        el.getAttribute("data-clay-slot"),
      role: el.dataset.role,
      mark: el.children[0]?.textContent ?? null,
      markColor: getComputedStyle(el.children[0]).color,
      name: el.children[1]?.children[0]?.textContent ?? null,
      dir: el.children[1]?.children[1]?.textContent ?? "",
      roleText: el.children[2]?.textContent ?? null,
      label: el.getAttribute("aria-label"),
      radius: px(style.borderTopLeftRadius),
      padding: px(style.paddingTop),
      background: style.backgroundColor,
      height: rect(el).h,
    };
  };
  const before = [...document.querySelectorAll(rowSelector)].map(row);
  const countBadge = () =>
    [...document.querySelectorAll('[data-clay-component="badge"]')].map(
      (el) => el.textContent,
    );
  // The filter is the approved quiet field: the well carries the boundary and
  // the accent halo, the input inside draws none.
  const filter = document.querySelector('input[aria-label="Filter session files"]');
  const filterPaint = filter
    ? (() => {
        const well = getComputedStyle(filter);
        return {
          borderWidth: px(well.borderTopWidth),
          radius: px(well.borderTopLeftRadius),
          outline: well.outlineStyle,
        };
      })()
    : null;
  let filtered = null;
  if (filter) {
    const setter = Object.getOwnPropertyDescriptor(
      window.HTMLInputElement.prototype,
      "value",
    ).set;
    setter.call(filter, "118-Quiet");
    filter.dispatchEvent(new Event("input", { bubbles: true }));
    await sleep(80);
    filtered = {
      visible: document.querySelectorAll(rowSelector).length,
      labels: [...document.querySelectorAll(rowSelector)].map((el) =>
        el.getAttribute("aria-label"),
      ),
      counts: countBadge(),
    };
    setter.call(filter, "");
    filter.dispatchEvent(new Event("input", { bubbles: true }));
    await sleep(80);
  }
  return {
    tabSelected: filesTab.getAttribute("aria-selected"),
    counts: countBadge(),
    rows: before,
    filterPaint,
    filtered,
    overflow:
      document.documentElement.scrollWidth -
      document.documentElement.clientWidth,
    inspector: rect(document.querySelector('aside[aria-label="Agent inspector"]')),
  };
})()`;

mkdirSync(OUT, { recursive: true });
const report = {
  base: BASE,
  viewport: { w: 1500, h: 950 },
  checks: {},
  probe: null,
};

const target = await openCdp();
const cdp = connect(target);
await cdp.ready;
await cdp.send("Page.enable");
await cdp.send("Page.navigate", {
  url: `${BASE}/?fixture=coding-agent&state=conversation`,
});
// The fixture boots the shell, then the agent panel's transcript: give the
// package load and the seeded rows time to land.
await sleep(2500);
report.probe = await evaluate(cdp, PROBE);

const screenshot = await cdp.send("Page.captureScreenshot", { format: "png" });
writeFileSync(
  join(OUT, "agent-files-1500.png"),
  Buffer.from(screenshot.data, "base64"),
);

const probe = report.probe;
const rows = probe.rows ?? [];
report.checks = {
  // The session's own records, newest first: the fixture seeds a write, an
  // edit and a read in transcript order.
  rowsAreSessionHistory:
    rows.length === 3 &&
    rows[0].role === "created" &&
    rows[0].mark === "A" &&
    rows[1].role === "modified" &&
    rows[1].mark === "M" &&
    rows[2].role === "read" &&
    rows[2].mark === "R",
  rowsUseSessionRowFamily: rows.every(
    (r) => r.family === "sessionRow/root",
  ),
  rowsPaintTheRecipe: rows.every(
    (r) => r.radius === 8 && r.padding === 8 && r.background === "rgba(0, 0, 0, 0)",
  ),
  basenameFirstWithDirectory:
    rows[0]?.name === "start.html" &&
    rows[0]?.dir === "design-artifacts/approved/quiet-instrument-migration" &&
    rows[2]?.name ===
      "118-Quiet-Instrument-Migration-Component-and-Surface-Adoption.md" &&
    rows[2]?.dir === "plans",
  rowsCarryTheirRoleWord: rows.every((r) => r.roleText === r.role),
  marksAreTonedByRole:
    rows[0]?.markColor !== rows[2]?.markColor &&
    rows[1]?.markColor !== rows[2]?.markColor,
  countMatchesTheRows: probe.counts?.includes(String(rows.length)),
  filterNarrowsTheList:
    probe.filtered?.visible === 1 &&
    probe.filtered?.labels?.[0]?.includes("118-Quiet-Instrument") &&
    probe.filtered?.counts?.includes("1"),
  filterIsAQuietFieldWithOneRing:
    probe.filterPaint?.borderWidth === 1 &&
    probe.filterPaint?.radius === 8 &&
    probe.filterPaint?.outline === "none",
  zeroHorizontalOverflow: probe.overflow === 0,
};
report.passed = Object.values(report.checks).every(Boolean);
writeFileSync(join(OUT, "report.json"), JSON.stringify(report, null, 2));
console.log(JSON.stringify(report, null, 2));
cdp.close();
process.exit(report.passed ? 0 : 1);
