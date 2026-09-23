// Verify the plan-125 palette stage-flow prototype and capture its review
// evidence.
//
// One Chrome process, every scene x theme x width in a loop. Each run is
// asserted: the theme really applied, the scene really is the one asked for,
// the sheet is the composer box's own width 6px above it, the sheet caps at
// min(52vh, 420px) and scrolls internally where the rows overflow, the stage
// prompt names the stage, the scope chips exist only for the catalogue, the
// secret stage really is a masked field whose value is bullets only (and never
// enters the composer draft), the foot says what `↵` does and offers the back
// affordance, the halo board shows both values side by side, the live sheet
// flips between them, the mentions menu shares the halo, `backdropBlur` stays
// 0 outside the scrim, and there is no console error, no non-file:// request
// and no horizontal overflow.
//
// Then it walks the keyboard path the design claims: `/` opens the catalogue,
// arrows move, Esc closes and stays closed until the draft changes, `@` opens
// mentions, Ctrl+X Ctrl+O opens the palette, `Esc`/`Alt+←` walk a stage trail
// back, `Alt+↵` deletes a session row, and a scope chip activates from the
// keyboard.
//
// Usage:
//   CHROME_PATH=... node design-artifacts/tools/capture-palette-stages.mjs
//   node design-artifacts/tools/capture-palette-stages.mjs --out=DIR --no-shots --quiet
//
// Exit status is non-zero if any assertion fails, so this is the gate the
// approval task rests on, not just a screenshot script.

import { spawn } from "node:child_process";
import { existsSync, mkdirSync, readdirSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const HERE = dirname(fileURLToPath(import.meta.url));
const PROTO = resolve(HERE, "..", "prototypes", "composer-palette-stages");
const arg = (name, fallback) => {
  const hit = process.argv.find((a) => a.startsWith(`--${name}=`));
  return hit ? hit.slice(name.length + 3) : fallback;
};
const PAGE = resolve(arg("page", join(PROTO, "palette-stages.html")));
const OUT = resolve(
  arg("out", join(HERE, "..", "screenshots", "composer-palette-stages")),
);
const SHOTS = !process.argv.includes("--no-shots");
const QUIET = process.argv.includes("--quiet");
// `--filter=` narrows the matrix to matching `scene__theme__width` labels
// (re-capturing a few frames after a visual fix must not re-run all 144).
const FILTER = arg("filter", null);
const PORT = 9379;
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

const THEMES = {
  "modus-operandi": { bg: "#f0f0f0", dark: false },
  "modus-vivendi": { bg: "#000000", dark: true },
  "gruvbox-material-dark": { bg: "#1d2021", dark: true },
  "gruvbox-material-light": { bg: "#f2e5bc", dark: false },
};
const WIDTHS = [
  { id: "wide", w: 1500, h: 1150, frame: "wide" },
  { id: "narrow", w: 1024, h: 1200, frame: "narrow" },
];

// scene -> the composition it claims. `palette` open|closed; `view` the shell
// view; `sidebar`/`inspector` the rail state (default expanded — `inspector`
// only shows in the agent view); `stage` the picker stage the sheet is in
// (null = catalogue); `prompt` the stage's question the head must show; `rows`
// a sanity bound.
const SCENES = [
  { name: "lane", view: "workspace", palette: false },
  { name: "lane-railed", view: "agent", palette: true, stage: null, includes: "/model" },
  { name: "lane-full", view: "agent", sidebar: "collapsed", inspector: "collapsed", palette: true, stage: null, rows: 12 },
  { name: "catalogue", view: "workspace", palette: true, stage: null, includes: "/model", rows: 2 },
  { name: "catalogue-empty", view: "workspace", palette: true, stage: null, rows: 12 },
  { name: "catalogue-no-match", view: "workspace", palette: true, stage: null, rows: 0, empty: true },
  { name: "catalogue-scopes", view: "workspace", palette: true, stage: null, scope: "files", rows: 1 },
  { name: "catalogue-chords", view: "workspace", palette: true, stage: null, scope: "shell", rows: 6 },
  { name: "stage-provider", view: "workspace", palette: true, stage: "provider", prompt: "Providers", rows: 1 },
  { name: "stage-auth", view: "workspace", palette: true, stage: "auth", prompt: "Choose sign-in method", rows: 3 },
  { name: "stage-secret", view: "workspace", palette: true, stage: "secret", prompt: "API key (hidden)", rows: 1, secret: true },
  { name: "stage-url", view: "workspace", palette: true, stage: "url", prompt: "API base URL", rows: 1, field: true },
  { name: "stage-oauth", view: "workspace", palette: true, stage: "oauth", prompt: "Authorize provider", rows: 2, code: true },
  { name: "stage-model", view: "workspace", palette: true, stage: "model", prompt: "Models", rows: 12, scroll: true },
  { name: "stage-session", view: "workspace", palette: true, stage: "session", prompt: "Sessions", rows: 8, secondary: "delete" },
  { name: "stage-back", view: "workspace", palette: true, stage: "secret", prompt: "API key (hidden)", rows: 1, secret: true, back: true },
  { name: "mentions", view: "workspace", palette: false, mentions: true },
  { name: "halo", view: "halo", palette: false, haloBoard: true },
];

function assert(failures, name, ok, detail) {
  if (!ok) failures.push(`${name}: ${detail}`);
  return Boolean(ok);
}

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
  const found = candidates.find((candidate) => candidate && existsSync(candidate));
  if (!found) throw new Error("no chrome binary found");
  return found;
}

function connect(url) {
  const ws = new WebSocket(url);
  let id = 0;
  const pending = new Map();
  ws.addEventListener("message", (event) => {
    const msg = JSON.parse(event.data);
    if (msg.id && pending.has(msg.id)) {
      const { resolve: done, reject } = pending.get(msg.id);
      pending.delete(msg.id);
      if (msg.error) reject(new Error(JSON.stringify(msg.error)));
      else done(msg.result);
    }
  });
  const ready = new Promise((done) => ws.addEventListener("open", done));
  return {
    ready,
    send(method, params = {}) {
      id += 1;
      ws.send(JSON.stringify({ id, method, params }));
      return new Promise((done, reject) => pending.set(id, { resolve: done, reject }));
    },
    close: () => ws.close(),
  };
}

const evaluate = async (cdp, expression) => {
  const { result, exceptionDetails } = await cdp.send("Runtime.evaluate", {
    expression,
    awaitPromise: true,
    returnByValue: true,
  });
  if (exceptionDetails) throw new Error(JSON.stringify(exceptionDetails));
  return result.value;
};

async function pressKey(cdp, key, code, extra = {}) {
  const base = { key, code, windowsVirtualKeyCode: extra.vk ?? 0 };
  await cdp.send("Input.dispatchKeyEvent", { type: "keyDown", ...base, ...extra });
  await cdp.send("Input.dispatchKeyEvent", { type: "keyUp", ...base, ...extra });
  await sleep(180);
}

async function typeText(cdp, text) {
  for (const char of text) {
    await cdp.send("Input.dispatchKeyEvent", {
      type: "keyDown",
      text: char,
      unmodifiedText: char,
      key: char,
    });
    await cdp.send("Input.dispatchKeyEvent", { type: "keyUp", key: char });
    await sleep(30);
  }
  await sleep(220);
}

const PROBE = `(() => {
  const rect = (el) => { if (!el) return null; const r = el.getBoundingClientRect(); return { x: Math.round(r.x), y: Math.round(r.y), w: Math.round(r.width), h: Math.round(r.height), top: Math.round(r.top), bottom: Math.round(r.bottom) }; };
  const visible = (el) => Boolean(el && el.getClientRects().length > 0);
  const sheet = document.querySelector('[data-palette]');
  const shell = document.querySelector('[data-shell]');
  const list = document.querySelector('[data-pal-list]');
  const field = document.querySelector('[data-pal-field-input]');
  const prompt = document.querySelector('[data-pal-prompt]');
  const scopes = document.querySelector('[data-pal-scopewrap]');
  const foot = document.querySelector('[data-pal-foot]');
  const mentions = document.querySelector('[data-mentions]');
  const scrim = document.querySelector('[data-scrim]');
  const draft = document.querySelector('[data-input]');
  const lane = document.querySelector('.lane');
  const main = document.querySelector('.pane.main');
  const side = document.querySelector('.side');
  const inspector = document.querySelector('.inspector');
  const composerForm = document.querySelector('.lane-composer');
  const shadow = (el) => (el ? getComputedStyle(el).boxShadow : null);
  const blur = (el) => (el ? getComputedStyle(el).backdropFilter : null);
  const rows = Array.prototype.map.call(document.querySelectorAll('.pal-item'), (row) => ({
    title: row.querySelector('.pal-title')?.textContent ?? '',
    detail: row.querySelector('.pal-detail')?.textContent ?? '',
    meta: row.querySelector('.pal-meta')?.textContent ?? '',
    secondary: row.querySelector('.pal-sec')?.textContent ?? '',
    chords: row.querySelectorAll('.kbd').length,
    active: row.dataset.active === 'true',
  }));
  return {
    theme: document.documentElement.dataset.theme,
    scene: document.documentElement.dataset.scene,
    frame: document.documentElement.dataset.frame,
    bodyBg: getComputedStyle(document.body).backgroundColor,
    view: document.querySelector('[data-app]').dataset.view,
    laneVisible: visible(document.querySelector('.lane')),
    /* The lane's width rule: it is the view pane's width, and a rail is never
       under it. lanePad is the composer's own inset inside the lane, so the
       sheet's relationship to the lane is measurable, not asserted. */
    app: rect(document.querySelector('[data-app]')),
    main: rect(main),
    lane: visible(lane) ? rect(lane) : null,
    side: visible(side) ? rect(side) : null,
    inspector: visible(inspector) ? rect(inspector) : null,
    lanePad: composerForm
      ? {
          left: Math.round(Number.parseFloat(getComputedStyle(composerForm).paddingLeft)),
          right: Math.round(Number.parseFloat(getComputedStyle(composerForm).paddingRight)),
        }
      : null,
    sheet: visible(sheet) ? rect(sheet) : null,
    sheetMode: sheet ? sheet.dataset.mode : null,
    sheetHalo: sheet ? sheet.dataset.halo : null,
    sheetShadow: shadow(sheet),
    sheetBlur: blur(sheet),
    sheetMaxHeight: sheet ? getComputedStyle(sheet).maxHeight : null,
    shell: rect(shell),
    gapAboveShell: sheet && shell ? Math.round(shell.getBoundingClientRect().top - sheet.getBoundingClientRect().bottom) : null,
    widthDeltaShell: sheet && shell ? Math.round(sheet.getBoundingClientRect().width - shell.getBoundingClientRect().width) : null,
    listScroll: list ? { scrollHeight: list.scrollHeight, clientHeight: list.clientHeight } : null,
    prompt: visible(prompt) ? prompt.textContent.trim() : null,
    promptSize: visible(prompt) ? getComputedStyle(prompt).letterSpacing : null,
    scopesVisible: visible(scopes),
    foot: foot ? foot.textContent.replace(/\\s+/g, ' ').trim() : '',
    field: {
      visible: visible(field),
      type: field ? field.type : null,
      value: field ? field.value : null,
      bulletsOnly: field ? /^\\u2022+$/.test(field.value) : null,
      ariaLabel: field ? field.getAttribute('aria-label') : null,
    },
    code: (() => {
      const block = document.querySelector('[data-pal-code]');
      if (!visible(block)) return null;
      return {
        user: document.querySelector('[data-pal-code-user]').textContent.trim(),
        uri: document.querySelector('[data-pal-code-uri]').textContent.trim(),
      };
    })(),
    draft: draft ? draft.value : null,
    mentions: visible(mentions) ? { ...rect(mentions), shadow: shadow(mentions), blur: blur(mentions) } : null,
    scrim: scrim ? { open: scrim.dataset.open, blur: blur(scrim), opacity: getComputedStyle(scrim).opacity } : null,
    empty: visible(document.querySelector('[data-pal-empty]')),
    rows,
    halo: Array.prototype.map.call(document.querySelectorAll('.halo-sheet'), (cell) => ({
      shadow: cell.dataset.shadow,
      computed: shadow(cell),
    })),
    sceneNote: (() => { const el = document.querySelector('[data-scene-note]'); return visible(el) ? el.textContent.trim() : null; })(),
    overflow: { scrollWidth: document.documentElement.scrollWidth, innerWidth: window.innerWidth },
    animating: Array.prototype.filter.call(document.querySelectorAll('.lane-palette, .completions'), (el) => getComputedStyle(el).animationName !== 'none').length,
    consoleErrors: window.__clayErrors ? window.__clayErrors.slice() : [],
    requests: window.__clayRequests ? window.__clayRequests.slice() : [],
  };
})()`;

mkdirSync(OUT, { recursive: true });
if (!existsSync(PAGE)) throw new Error(`prototype page missing: ${PAGE}`);
const chrome = spawn(
  findChrome(),
  [
    "--headless=new",
    `--remote-debugging-port=${PORT}`,
    "--no-first-run",
    "--no-default-browser-check",
    "--disable-gpu",
    "--hide-scrollbars",
    "--allow-file-access-from-files",
    "about:blank",
  ],
  { stdio: "ignore" },
);

let target = null;
for (let attempt = 0; attempt < 60 && !target; attempt += 1) {
  try {
    const response = await fetch(`http://127.0.0.1:${PORT}/json/list`);
    target = (await response.json()).find((entry) => entry.type === "page");
  } catch {
    /* not up yet */
  }
  if (!target) await sleep(200);
}
if (!target) {
  chrome.kill("SIGKILL");
  throw new Error("chrome did not expose a debug target");
}
const cdp = connect(target.webSocketDebuggerUrl);
await cdp.ready;
await cdp.send("Page.enable");
await cdp.send("Runtime.enable");
await cdp.send("Network.enable");
await cdp.send("Accessibility.enable");

const requests = [];
cdp.send("Network.setRequestInterception", { patterns: [] }).catch(() => {});
await cdp.send("Page.addScriptToEvaluateOnNewDocument", {
  source: `window.__clayErrors = []; window.__clayRequests = [];
    window.addEventListener('error', (e) => window.__clayErrors.push(String(e.message)));
    window.addEventListener('unhandledrejection', (e) => window.__clayErrors.push(String(e.reason)));
    const originalFetch = window.fetch;
    window.fetch = (...args) => { window.__clayRequests.push(String(args[0])); return originalFetch(...args); };`,
});

const url = (scene, theme, width) =>
  `file://${PAGE}?scene=${scene}&theme=${theme}&width=${width}`;

const failures = [];
const matrix = [];

for (const scene of SCENES) {
  for (const [themeId, theme] of Object.entries(THEMES)) {
    for (const width of WIDTHS) {
      const label = `${scene.name}__${themeId}__${width.id}`;
      if (FILTER && !new RegExp(FILTER).test(label)) continue;
      await cdp.send("Emulation.setDeviceMetricsOverride", {
        width: width.w,
        height: width.h,
        deviceScaleFactor: 1,
        mobile: false,
      });
      // a fresh page each run: the scene, theme and width all come from the URL
      await cdp.send("Page.navigate", { url: url(scene.name, themeId, width.frame) });
      await sleep(700);
      const probe = await evaluate(cdp, PROBE);
      matrix.push({ label, probe });
      if (!QUIET) {
        console.log(
          `${label}: sheet=${probe.sheet ? probe.sheet.w + "x" + probe.sheet.h : "none"} rows=${probe.rows.length} mode=${probe.sheetMode} shadow=${probe.sheetHalo}`,
        );
      }

      assert(failures, label, probe.theme === themeId, `theme is ${probe.theme}`);
      assert(failures, label, probe.scene === scene.name, `scene is ${probe.scene}`);
      assert(failures, label, probe.view === scene.view, `view is ${probe.view}`);
      assert(failures, label, probe.laneVisible, "the lane is on screen");

      /* The lane's containment: the view pane's width, never a rail's, and the
         working area's whole width when no rail is showing (DESIGN §12 as
         amended by plan 125). */
      const laneW = probe.lane ? probe.lane.x + probe.lane.w : null;
      const mainW = probe.main ? probe.main.x + probe.main.w : null;
      assert(failures, label, Boolean(probe.lane) && Boolean(probe.main), "the lane and the view pane are measurable");
      if (probe.lane && probe.main) {
        assert(
          failures,
          label,
          Math.abs(probe.lane.x - probe.main.x) <= 1 && Math.abs(laneW - mainW) <= 1,
          `the lane is the view pane's width (lane ${probe.lane.x}..${laneW}, view ${probe.main.x}..${mainW})`,
        );
        for (const rail of ["side", "inspector"]) {
          const box = probe[rail];
          if (!box) continue;
          assert(
            failures,
            label,
            probe.lane.x >= box.x + box.w - 1 || laneW <= box.x + 1,
            `the lane never sits over the ${rail} rail (lane ${probe.lane.x}..${laneW}, ${rail} ${box.x}..${box.x + box.w})`,
          );
        }
      }
      const wantsSide = scene.sidebar !== "collapsed";
      const wantsInspector = scene.inspector !== "collapsed" && scene.view === "agent";
      assert(failures, label, Boolean(probe.side) === wantsSide, `files rail visible=${Boolean(probe.side)}, expected ${wantsSide}`);
      assert(
        failures,
        label,
        Boolean(probe.inspector) === wantsInspector,
        `agent rail visible=${Boolean(probe.inspector)}, expected ${wantsInspector}`,
      );
      if (probe.side && probe.main) {
        assert(failures, label, Math.abs(probe.main.x - (probe.side.x + probe.side.w)) <= 1, "the view pane starts where the files rail ends");
      }
      if (probe.inspector && probe.main) {
        assert(failures, label, Math.abs(mainW - probe.inspector.x) <= 1, "the view pane ends where the agent rail starts");
      }
      if (!wantsSide && !wantsInspector && probe.lane && probe.app) {
        assert(
          failures,
          label,
          Math.abs(probe.lane.x - probe.app.x) <= 1 && Math.abs(laneW - (probe.app.x + probe.app.w)) <= 1,
          `with no rail showing the lane takes the whole working area (lane ${probe.lane.x}..${laneW}, app ${probe.app.x}..${probe.app.x + probe.app.w})`,
        );
      }
      assert(failures, label, probe.consoleErrors.length === 0, `console errors: ${probe.consoleErrors.join(" | ")}`);
      assert(failures, label, probe.requests.length === 0, `network requests: ${probe.requests.join(" | ")}`);
      assert(
        failures,
        label,
        probe.overflow.scrollWidth <= probe.overflow.innerWidth + 1,
        `horizontal overflow ${probe.overflow.scrollWidth} > ${probe.overflow.innerWidth}`,
      );
      assert(failures, label, probe.sheetBlur === "none", `sheet backdrop-filter is ${probe.sheetBlur}`);
      assert(failures, label, probe.mentions ? probe.mentions.blur === "none" : true, "mentions menu has no blur of its own");
      if (probe.scrim && probe.scrim.open === "true") {
        assert(failures, label, /blur\(3px\)/.test(probe.scrim.blur), `scrim blur is ${probe.scrim.blur}`);
      }

      if (scene.palette) {
        assert(failures, label, Boolean(probe.sheet), "the sheet is on screen");
        assert(failures, label, probe.widthDeltaShell === 0, `sheet width matches the composer box (delta ${probe.widthDeltaShell})`);
        /* ...and the composer box is the lane's own inner width: the sheet
           matches the lane, inset by the lane's padding, so it can never
           cross into a rail. */
        assert(
          failures,
          label,
          probe.sheet.x >= probe.lane.x - 1 && probe.sheet.x + probe.sheet.w <= laneW + 1,
          `the sheet stays inside the lane (sheet ${probe.sheet.x}..${probe.sheet.x + probe.sheet.w}, lane ${probe.lane.x}..${laneW})`,
        );
        assert(
          failures,
          label,
          Math.abs(probe.sheet.x - (probe.lane.x + probe.lanePad.left)) <= 1 &&
            Math.abs(probe.sheet.x + probe.sheet.w - (laneW - probe.lanePad.right)) <= 1,
          `the sheet is the lane's inner width (inset ${probe.lanePad.left}/${probe.lanePad.right}, sheet ${probe.sheet.x}..${probe.sheet.x + probe.sheet.w})`,
        );
        assert(failures, label, probe.gapAboveShell === 6, `sheet sits 6px above the box (${probe.gapAboveShell})`);
        const cap = Number.parseFloat(probe.sheetMaxHeight.replace("px", ""));
        assert(failures, label, cap <= 420, `sheet cap is ${probe.sheetMaxHeight}`);
        assert(failures, label, probe.sheet.h <= cap + 1, `sheet height ${probe.sheet.h} within cap ${cap}`);
        assert(failures, label, !scene.stage || probe.scopesVisible === false, "stage sheets show no scope chips");
        if (!scene.stage) {
          assert(failures, label, probe.scopesVisible, "catalogue shows the scope chips");
          assert(failures, label, probe.prompt === null, "catalogue has no stage prompt");
          assert(failures, label, /run/.test(probe.foot) && /close/.test(probe.foot), `catalogue foot: ${probe.foot}`);
        } else {
          assert(failures, label, probe.scopesVisible === false, "stage sheets hide the scope chips");
          assert(failures, label, probe.prompt === scene.prompt, `stage prompt is ${JSON.stringify(probe.prompt)}`);
          assert(failures, label, /back/.test(probe.foot), `stage foot offers back: ${probe.foot}`);
        }
        if (scene.rows !== undefined) {
          assert(failures, label, probe.rows.length === scene.rows, `rows ${probe.rows.length} != ${scene.rows}`);
        }
        if (scene.includes) {
          assert(failures, label, probe.rows.some((row) => row.title === scene.includes), `rows include ${scene.includes}`);
        }
        if (scene.secret) {
          assert(failures, label, probe.field.visible && probe.field.type === "password", "the secret field is a masked field");
          assert(failures, label, probe.field.bulletsOnly === true, `secret field value is bullets only (${JSON.stringify(probe.field.value)})`);
          assert(failures, label, probe.draft === "", "the composer draft is not the secret transport");
          assert(failures, label, !probe.rows.some((row) => (row.title + row.detail + row.meta).includes("\u2022")), "no row repeats the masked value");
        }
        if (scene.field) {
          assert(failures, label, probe.field.visible && probe.field.type === "text", "the URL stage shows a visible field");
        }
        if (scene.code) {
          assert(failures, label, Boolean(probe.code), "the device-code block is on screen");
          assert(failures, label, /^[A-Z0-9-]{6,}$/.test(probe.code?.user ?? ""), `user code ${probe.code?.user}`);
          assert(failures, label, /^https:\/\//.test(probe.code?.uri ?? ""), `verification URI ${probe.code?.uri}`);
        }
        if (scene.scroll) {
          assert(failures, label, probe.listScroll.scrollHeight > probe.listScroll.clientHeight, `long list scrolls inside the sheet (${probe.listScroll.scrollHeight}/${probe.listScroll.clientHeight})`);
        }
        if (scene.secondary) {
          assert(failures, label, probe.rows.some((row) => row.secondary.includes(scene.secondary)), `a row carries the ${scene.secondary} hint`);
          assert(failures, label, /delete/.test(probe.foot), "the foot names the secondary action");
        }
        if (scene.empty) {
          assert(failures, label, probe.empty, "the empty state is shown");
          assert(failures, label, probe.rows.length === 0, "no rows survive the filter");
        }
        assert(failures, label, probe.scrim.open === "true", "the veil is up while the sheet is open");
        assert(
          failures,
          label,
          probe.sheet.bottom <= probe.shell.top && probe.sheet.top >= 0,
          `the sheet sits above the composer inside the frame (sheet ${probe.sheet.top}..${probe.sheet.bottom}, shell ${probe.shell.top})`,
        );
      } else {
        assert(failures, label, probe.sheet === null, "no sheet is open");
      }

      if (scene.mentions) {
        assert(failures, label, Boolean(probe.mentions), "the mentions menu is on screen");
        assert(failures, label, probe.scrim.open === "true", "the mentions menu shares the veil");
      }

      if (scene.haloBoard) {
        assert(failures, label, probe.halo.length === 4, `halo board has ${probe.halo.length} cells`);
        const current = probe.halo.filter((cell) => cell.shadow === "current");
        const halo = probe.halo.filter((cell) => cell.shadow === "halo");
        assert(failures, label, current.length === 2 && halo.length === 2, "both values are shown");
        assert(failures, label, halo.every((cell) => /0px 0px 14px -2px/.test(cell.computed)), `halo value: ${halo[0]?.computed}`);
        assert(failures, label, current.every((cell) => /0px 24px 60px|0px 14px 34px/.test(cell.computed)), `today's value: ${current[0]?.computed}`);
      }

      // the live sheet flips between the two values
      if (scene.palette) {
        assert(failures, label, /0px 0px 14px -2px/.test(probe.sheetShadow), `sheet wears the halo by default: ${probe.sheetShadow}`);
        const flipped = await evaluate(
          cdp,
          `(() => { document.querySelector('[data-set-halo="current"]').click(); return getComputedStyle(document.querySelector('[data-palette]')).boxShadow; })()`,
        );
        assert(failures, label, /0px 24px 60px/.test(flipped), `drop shadow returns on toggle: ${flipped}`);
        const back = await evaluate(
          cdp,
          `(() => { document.querySelector('[data-set-halo="halo"]').click(); return getComputedStyle(document.querySelector('[data-palette]')).boxShadow; })()`,
        );
        assert(failures, label, /0px 0px 14px -2px/.test(back), `halo returns on toggle: ${back}`);
      }

      if (SHOTS) {
        // the toggle above raises its own toast; a review frame shows the state,
        // not the review device that produced it
        await evaluate(cdp, `(() => { document.querySelectorAll('.toast').forEach((t) => t.remove()); return true; })()`);
        const shot = await cdp.send("Page.captureScreenshot", { format: "png" });
        writeFileSync(join(OUT, `${label}.png`), Buffer.from(shot.data, "base64"));
      }
    }
  }
}

/* ---------- the keyboard path ------------------------------------------- */

const keyboard = [];
const step = async (name, ok, detail = "") => {
  keyboard.push({ step: name, ok: Boolean(ok), detail: String(detail) });
  if (!ok) failures.push(`keyboard/${name}: ${detail}`);
};
const focusInput = () =>
  evaluate(cdp, `(() => { const el = document.querySelector('[data-input]'); el.focus(); return document.activeElement === el; })()`);

await cdp.send("Emulation.setDeviceMetricsOverride", { width: 1500, height: 1150, deviceScaleFactor: 1, mobile: false });
await cdp.send("Page.navigate", { url: url("lane", "gruvbox-material-dark", "wide") });
await sleep(700);
await focusInput();
// start from an empty draft: the `lane` scene carries prose in the field
await evaluate(cdp, `(() => { const el = document.querySelector('[data-input]'); el.value = ''; el.dispatchEvent(new Event('input', { bubbles: true })); return el.value; })()`);
await typeText(cdp, "/");
let probe = await evaluate(cdp, PROBE);
await step("slash opens the catalogue", probe.sheet !== null && probe.sheetMode === "catalogue", `sheet=${JSON.stringify(probe.sheet)} mode=${probe.sheetMode}`);

await typeText(cdp, "model");
probe = await evaluate(cdp, PROBE);
await step("typing filters inside the catalogue", probe.rows.length >= 1 && probe.rows.some((row) => row.title === "/model"), `rows=${probe.rows.length} titles=${probe.rows.map((row) => row.title).join("/")}`);

await pressKey(cdp, "ArrowDown", "ArrowDown", { vk: 40 });
const moved = await evaluate(cdp, `(() => document.querySelectorAll('.pal-item')[0]?.dataset.active)()`);
await step("ArrowDown moves the selection", moved === "false", `first row active=${moved}`);

await pressKey(cdp, "Escape", "Escape", { vk: 27 });
probe = await evaluate(cdp, PROBE);
await step("Esc closes the catalogue", probe.sheet === null, `sheet=${probe.sheet}`);

await typeText(cdp, "x");
probe = await evaluate(cdp, PROBE);
/* The artifact's Esc: the draft survives the dismissal and the next keystroke
   reopens the sheet with it (the shipped Composer rule). */
await step(
  "Esc keeps the draft; the next keystroke reopens",
  probe.draft === "/modelx" && probe.sheet !== null,
  `draft=${probe.draft} rows=${probe.rows.length}`,
);

await evaluate(cdp, `(() => { const el = document.querySelector('[data-input]'); el.value = ''; el.dispatchEvent(new Event('input', { bubbles: true })); return true; })()`);
await typeText(cdp, "/");
probe = await evaluate(cdp, PROBE);
await step("a fresh draft reopens it", probe.sheet !== null, `mode=${probe.sheetMode}`);

await pressKey(cdp, "Escape", "Escape", { vk: 27 });
await evaluate(cdp, `(() => { const el = document.querySelector('[data-input]'); el.value = '@'; el.dispatchEvent(new Event('input', { bubbles: true })); return true; })()`);
probe = await evaluate(cdp, PROBE);
await step("@ opens mentions", probe.mentions !== null, "mentions visible");

await pressKey(cdp, "Escape", "Escape", { vk: 27 });
probe = await evaluate(cdp, PROBE);
await step("Esc closes mentions", probe.mentions === null && probe.scrim.open === "false", `scrim=${probe.scrim.open}`);

// Ctrl+X Ctrl+O: the shipping chord
await cdp.send("Input.dispatchKeyEvent", { type: "keyDown", key: "x", code: "KeyX", modifiers: 2, windowsVirtualKeyCode: 88 });
await cdp.send("Input.dispatchKeyEvent", { type: "keyUp", key: "x", code: "KeyX", modifiers: 2, windowsVirtualKeyCode: 88 });
await sleep(200);
await cdp.send("Input.dispatchKeyEvent", { type: "keyDown", key: "o", code: "KeyO", modifiers: 2, windowsVirtualKeyCode: 79 });
await cdp.send("Input.dispatchKeyEvent", { type: "keyUp", key: "o", code: "KeyO", modifiers: 2, windowsVirtualKeyCode: 79 });
await sleep(300);
probe = await evaluate(cdp, PROBE);
await step("Ctrl+X Ctrl+O opens the catalogue", probe.sheet !== null && probe.sheetMode === "catalogue", `mode=${probe.sheetMode}`);

// the stage trail: Esc walks back one stage at a time
await cdp.send("Page.navigate", { url: url("stage-back", "gruvbox-material-dark", "wide") });
await sleep(600);
await focusInput();
probe = await evaluate(cdp, PROBE);
await step("depth 3 starts at the secret stage", probe.prompt === "API key (hidden)", `prompt=${probe.prompt}`);
await pressKey(cdp, "Escape", "Escape", { vk: 27 });
probe = await evaluate(cdp, PROBE);
await step("Esc walks back to the sign-in method", probe.prompt === "Choose sign-in method", `prompt=${probe.prompt}`);
await pressKey(cdp, "ArrowLeft", "ArrowLeft", { vk: 37, modifiers: 1 });
probe = await evaluate(cdp, PROBE);
await step("Alt+← walks back to the providers", probe.prompt === "Providers", `prompt=${probe.prompt}`);
await pressKey(cdp, "Escape", "Escape", { vk: 27 });
probe = await evaluate(cdp, PROBE);
await step("Esc at the first stage closes the sheet", probe.sheet === null, `sheet=${probe.sheet}`);

// the session secondary action
await cdp.send("Page.navigate", { url: url("stage-session", "gruvbox-material-dark", "wide") });
await sleep(600);
await focusInput();
const before = await evaluate(cdp, `(() => document.querySelectorAll('.pal-item').length)()`);
await cdp.send("Input.dispatchKeyEvent", { type: "keyDown", key: "Enter", code: "Enter", modifiers: 1, windowsVirtualKeyCode: 13 });
await cdp.send("Input.dispatchKeyEvent", { type: "keyUp", key: "Enter", code: "Enter", modifiers: 1, windowsVirtualKeyCode: 13 });
await sleep(300);
const after = await evaluate(cdp, `(() => document.querySelectorAll('.pal-item').length)()`);
await step("Alt+↵ deletes the selected session", after === before - 1, `rows ${before} -> ${after}`);

// a scope chip from the keyboard
await cdp.send("Page.navigate", { url: url("catalogue-empty", "gruvbox-material-dark", "wide") });
await sleep(600);
const chipOk = await evaluate(
  cdp,
  `(() => {
     const chip = document.querySelector('[data-pal-scope="files"]');
     chip.focus();
     return document.activeElement === chip;
   })()`,
);
await cdp.send("Input.dispatchKeyEvent", { type: "keyDown", key: "Enter", code: "Enter", text: "\r", unmodifiedText: "\r", windowsVirtualKeyCode: 13 });
await cdp.send("Input.dispatchKeyEvent", { type: "keyUp", key: "Enter", code: "Enter", windowsVirtualKeyCode: 13 });
await sleep(300);
probe = await evaluate(cdp, PROBE);
await step("a scope chip activates from the keyboard", chipOk && probe.rows.length === 1 && /Browse Filesystem/.test(probe.rows[0].title), `focus=${chipOk} rows=${probe.rows.length} titles=${probe.rows.map((r) => r.title).join('/')}`);

const report = {
  generatedAt: new Date().toISOString(),
  page: PAGE,
  themes: Object.keys(THEMES),
  widths: WIDTHS.map((width) => width.id),
  scenes: SCENES.map((scene) => scene.name),
  checks: matrix.length * 8,
  keyboard,
  failures,
  matrix,
};
writeFileSync(join(OUT, "report.json"), JSON.stringify(report, null, 2));

const total = matrix.length + keyboard.length;
if (failures.length) {
  console.log(`\nFAIL ${failures.length} of ${total}`);
  for (const failure of failures.slice(0, 30)) console.log(`  - ${failure}`);
  chrome.kill("SIGKILL");
  process.exit(1);
}
console.log(`\nPASS ${total} checks across ${matrix.length} matrix runs + ${keyboard.length} keyboard steps.`);
console.log(`evidence: ${OUT}`);
chrome.kill("SIGKILL");
