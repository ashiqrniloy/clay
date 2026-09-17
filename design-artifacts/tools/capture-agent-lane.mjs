// Verify the plan-124 lane/palette prototype and capture its review evidence.
//
// One Chrome process, every state x theme x width in a loop. Each run is
// asserted — the theme really applied, the scene really is the one asked for,
// the lane is present/hidden as claimed, the palette is anchored to the lane
// over a real 3px veil that leaves the lane above it, the composer shows the
// right Stop/disabled state and never a Send button, the lane's foot carries the
// no-provider notice and no run indicator (the window mark's dot is the window's
// run signal — one blinking dot, at 1.1s — while the agent view's state strip
// says the state in words and swaps its tone dot for the transcript's three bars
// while a turn is in flight), the title bar is the app's (mark,
// tab strip, spacer, window actions at the right edge, no window buttons),
// `backdropBlur == 0` holds on every
// content
// surface, no console error, no non-file:// request, no horizontal overflow and
// no element wider than the window frame — and the runs that are review
// evidence are saved as PNGs named <scene>__<theme>__<width>.png.
//
// Then it walks the keyboard path the design claims: Ctrl+X Ctrl+P toggles the
// lane, Ctrl+X Ctrl+O opens the palette, `/` opens it from the composer, `@`
// opens mentions, arrows move, Esc closes, and the palette's own scrim mirrors
// each change — including the mentions menu, which shares that veil. The walk
// is recorded in report.json alongside the matrix.
//
// Usage:
//   CHROME_PATH=... node design-artifacts/tools/capture-agent-lane.mjs
//   node design-artifacts/tools/capture-agent-lane.mjs --out=DIR --no-shots --quiet
//
// Exit status is non-zero if any assertion fails, so this is the gate the
// approval task rests on, not just a screenshot script.

import { spawn } from "node:child_process";
import {
  existsSync,
  mkdirSync,
  readdirSync,
  writeFileSync,
} from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const HERE = dirname(fileURLToPath(import.meta.url));
const PROTO = resolve(HERE, "..", "prototypes", "agent-lane-palette");
const arg = (name, fallback) => {
  const hit = process.argv.find((a) => a.startsWith(`--${name}=`));
  return hit ? hit.slice(name.length + 3) : fallback;
};
/* `--page=` points the same assertions at a frozen set, e.g.
   `approved/agent-lane-palette/lane-palette.html`, to prove the approved copy
   still behaves like the reviewed one. */
const PAGE = resolve(arg("page", join(PROTO, "lane-palette.html")));
const OUT = resolve(arg("out", join(HERE, "..", "screenshots", "agent-lane-palette")));
const SHOTS = !process.argv.includes("--no-shots");
const QUIET = process.argv.includes("--quiet");
const PORT = 9377;

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

// scene -> the composition it claims. lane open|hidden; palette open|closed;
// mode is the composer state; query is the composer's draft.
const SCENES = [
  { name: "idle", view: "workspace", lane: "open", mode: "idle", palette: false },
  { name: "agent", view: "agent", lane: "open", mode: "idle", palette: false },
  { name: "streaming", view: "agent", lane: "open", mode: "streaming", palette: false },
  { name: "approval", view: "agent", lane: "open", mode: "approval", palette: false },
  { name: "no-agent", view: "agent", lane: "open", mode: "no-agent", palette: false },
  { name: "disabled", view: "workspace", lane: "open", mode: "disabled", palette: false },
  { name: "hidden", view: "workspace", lane: "hidden", mode: "idle", palette: false },
  { name: "palette", view: "workspace", lane: "open", mode: "idle", palette: true, rows: 12 },
  { name: "palette-agent", view: "agent", lane: "open", mode: "idle", palette: true, rows: 12 },
  { name: "palette-empty", view: "workspace", lane: "open", mode: "idle", palette: true, rows: 0 },
  { name: "mention", view: "workspace", lane: "open", mode: "idle", palette: false, mention: true },
];

function findChrome() {
  const candidates = [
    process.env.CHROME_PATH,
    "/usr/bin/google-chrome",
    "/usr/bin/chromium",
    "/usr/bin/chromium-browser",
    join(process.env.HOME ?? "/root", ".cache/puppeteer/chrome/linux-152.0.7977.42/chrome-linux64/chrome"),
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
        join(cache, entry, "chrome-headless-shell-linux64/chrome-headless-shell"),
      );
    }
  }
  const found = candidates.find((p) => p && existsSync(p));
  if (!found) throw new Error("no Chrome/Chromium found");
  return found;
}

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

async function openCdp() {
  spawn(
    findChrome(),
    [
      "--headless=new",
      "--no-sandbox",
      "--disable-gpu",
      "--hide-scrollbars",
      `--remote-debugging-port=${PORT}`,
      "--user-data-dir=/tmp/clay-agent-lane-capture-profile",
      "--no-first-run",
      "--no-default-browser-check",
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

function connect(url, onEvent) {
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
      return;
    }
    if (msg.method && onEvent) onEvent(msg);
  });
  const ready = new Promise((resolve) => ws.addEventListener("open", resolve));
  return {
    ready,
    send(method, params = {}) {
      id += 1;
      ws.send(JSON.stringify({ id, method, params }));
      return new Promise((resolve, reject) => pending.set(id, { resolve, reject }));
    },
    close: () => ws.close(),
  };
}

const evaluate = async (cdp, expression) => {
  const result = await cdp.send("Runtime.evaluate", { expression, awaitPromise: true, returnByValue: true });
  if (result.exceptionDetails) throw new Error(JSON.stringify(result.exceptionDetails.exception));
  return result.result.value;
};

/// One run's measurements. Read-only: it never changes the page.
const PROBE = `(() => {
  const app = document.querySelector('[data-app]');
  const lane = document.querySelector('.lane');
  const shell = document.querySelector('[data-shell]');
  const pal = document.querySelector('[data-palette]');
  const palList = document.querySelector('[data-pal-list]');
  const palEmpty = document.querySelector('[data-pal-empty]');
  const scrim = document.querySelector('[data-scrim]');
  const input = document.querySelector('[data-input]');
  const mentions = document.querySelector('[data-mentions]');
  const frame = document.querySelector('.proto-frame');
  const rect = (el) => {
    if (!el) return null;
    const r = el.getBoundingClientRect();
    return { x: Math.round(r.x), y: Math.round(r.y), w: Math.round(r.width), h: Math.round(r.height), right: Math.round(r.right), bottom: Math.round(r.bottom) };
  };
  const px = (v) => (v && v.endsWith('px') ? parseFloat(v) : v);
  const root = getComputedStyle(document.documentElement);
  const scrimStyle = scrim ? getComputedStyle(scrim) : null;
  const contentBlur = ['.doc', '.rows', '.transcript', '.side'].map((sel) => {
    const el = document.querySelector(sel);
    return { sel, blur: el ? getComputedStyle(el).backdropFilter : 'missing' };
  });
  const frameRect = rect(frame);
  const controls = document.querySelector('[data-lane-controls]');
  const model = document.querySelector('[data-popover-toggle="lane-model"]');
  const effort = document.querySelector('[data-popover-toggle="lane-effort"]');
  const meter = document.querySelector('[data-meter]');
  const agentKind = document.querySelector('[data-popover-toggle="lane-agent"]');
  const horizontalFit = ['.titlebar', '.side', '.pane.main', '.inspector', '.lane', '.lane-palette', '.completions', '.statusbar']
    .map((sel) => {
      const el = document.querySelector(sel);
      if (!el || el.hidden || getComputedStyle(el).display === 'none') return null;
      const r = el.getBoundingClientRect();
      return { sel, left: Math.round(r.left), right: Math.round(r.right), overflow: Math.round(r.right) - frameRect.right };
    })
    .filter(Boolean);
  return {
    theme: document.documentElement.dataset.theme,
    scene: document.documentElement.dataset.scene,
    view: app.dataset.view,
    laneState: app.dataset.lane,
    mode: lane.dataset.mode,
    laneVisible: !lane.hidden && getComputedStyle(lane).display !== 'none',
    laneRect: rect(lane),
    laneZ: getComputedStyle(lane).zIndex,
    shellRect: rect(shell),
    /** Plan-124 review 2026-09-16: the tab's agent controls live in the lane. */
    laneControlsVisible: !controls.hidden && getComputedStyle(controls).display !== 'none',
    laneControlsMode: controls.dataset.mode,
    agentKindVisible: !agentKind.hidden,
    agentKindDisabled: agentKind.disabled,
    agentName: document.querySelector('[data-agent-name]').textContent,
    modelVisible: !model.hidden && getComputedStyle(model).display !== 'none',
    modelDisabled: model.disabled,
    modelText: document.querySelector('[data-model-value]').textContent,
    effortVisible: !effort.hidden && getComputedStyle(effort).display !== 'none',
    effortText: document.querySelector('[data-effort-value]').textContent,
    meterVisible: !meter.hidden && getComputedStyle(meter).display !== 'none',
    /** The shot that moved the model/effort pickers out of the agent view. */
    agentHeadPresent: Boolean(document.querySelector('.agent-head')),
    paletteOpen: !pal.hidden,
    paletteRect: rect(pal),
    paletteRows: document.querySelectorAll('[data-pal-list] .pal-item').length,
    paletteEmptyVisible: !palEmpty.hidden,
    paletteCount: document.querySelector('[data-pal-count]').textContent,
    paletteListVisibleRows: palList ? [...palList.querySelectorAll('.pal-item')].filter((el) => el.getBoundingClientRect().height > 0).length : 0,
    scrimOpen: scrim.dataset.open === 'true',
    scrimOpacity: scrimStyle ? px(scrimStyle.opacity) : null,
    scrimBlur: scrimStyle ? scrimStyle.backdropFilter : null,
    scrimFill: scrimStyle ? scrimStyle.backgroundColor : null,
    scrimAlpha: (() => {
      if (!scrimStyle) return null;
      const raw = scrimStyle.backgroundColor;
      const slash = raw.lastIndexOf('/');
      if (slash !== -1) return parseFloat(raw.slice(slash + 1));
      if (raw.indexOf('rgba') === 0) {
        const parts = raw.slice(5, -1).split(',');
        return parseFloat(parts[3]);
      }
      return 1;
    })(),
    scrimZ: scrimStyle ? scrimStyle.zIndex : null,
    mentionOpen: !mentions.hidden,
    approvalVisible: !document.querySelector('[data-approval]').hidden,
    /* Review 2026-09-17 (second round): no Send button anywhere — ↵ sends (the
       hint row says so) and Stop takes the slot while a run is live. The lane's
       foot carries no run indicator at all: the window mark's dot is the run's
       signal, and the agent view's state strip says it in words. */
    sendButtons: document.querySelectorAll('[data-send]').length,
    stopVisible: !document.querySelector('[data-stop]').hidden,
    footText: document.querySelector('.lane-foot').textContent,
    footRunMarkers: document.querySelectorAll('.lane-foot .lane-busy, .lane-foot .typing').length,
    brandBusy: document.querySelector('[data-brand]').dataset.busy,
    brandDotAnimation: (() => {
      const s = getComputedStyle(document.querySelector('[data-brand]'), '::before');
      return { name: s.animationName, duration: s.animationDuration };
    })(),
    tabAgentBusy: document.querySelector('[data-tab-agent]').dataset.busy,
    tabAgentAnimation: (() => {
      const s = getComputedStyle(document.querySelector('[data-tab-agent]'));
      return { name: s.animationName, duration: s.animationDuration };
    })(),
    stateDotAnimation: getComputedStyle(document.querySelector('[data-state-dot]')).animationName,
    /* Review 2026-09-17 (third round): the state strip's run cue is the
       transcript's three bars — in place of the tone dot — while a turn is in
       flight; the dot comes back when the turn ends. */
    stateStrip: (() => {
      const strip = document.querySelector('.state-strip');
      if (!strip) return { present: false };
      const dot = document.querySelector('[data-state-dot]');
      const bars = document.querySelector('[data-state-bars]');
      const first = bars ? bars.querySelector('i') : null;
      const anim = first ? getComputedStyle(first) : null;
      return {
        visible: strip.getClientRects().length > 0,
        dotVisible: Boolean(dot && dot.getClientRects().length > 0),
        barsVisible: Boolean(bars && bars.getClientRects().length > 0),
        bars: bars ? bars.querySelectorAll('i').length : 0,
        barsAnimation: anim ? { name: anim.animationName, duration: anim.animationDuration } : null,
      };
    })(),
    /* The top bar is the app's (app-shell.tsx): mark, tab strip hugging it,
       spacer, then the window actions — Control Center trigger, view switcher
       — at the right edge, and no window buttons. */
    titlebar: (() => {
      const actions = document.querySelector('.titlebar nav[aria-label="Application controls"]');
      const bar = document.querySelector('.titlebar').getBoundingClientRect();
      const viewSwitch = document.querySelector('[data-viewswitch]').getBoundingClientRect();
      const commands = document.querySelector('.titlebar [data-commands]').getBoundingClientRect();
      const brand = document.querySelector('.titlebar [data-brand]');
      return {
        brandText: brand ? brand.textContent.trim() : null,
        tabs: document.querySelectorAll('.titlebar .tb-tabs [role="tab"]').length,
        spacer: Boolean(document.querySelector('.titlebar .tb-spacer')),
        commandsInActions: Boolean(actions && actions.querySelector('[data-commands]')),
        viewSwitchInActions: Boolean(actions && actions.querySelector('[data-viewswitch]')),
        windowControls: document.querySelectorAll('.titlebar .win-btn, .titlebar .win-btns').length,
        rightGap: Math.round(bar.right - viewSwitch.right),
        commandsBeforeSwitch: Math.round(viewSwitch.left - commands.right),
      };
    })(),
    laneNoteVisible: !document.querySelector('[data-lane-note]').hidden,
    laneNoteText: document.querySelector('[data-lane-note]').textContent,
    conversationTurns: [...document.querySelectorAll('[data-conversation]')].filter((el) => !el.hidden).length,
    inputDisabled: input.disabled,
    inputPlaceholder: input.placeholder,
    tokenBg: root.getPropertyValue('--c-bg').trim(),
    tokenAccent: root.getPropertyValue('--c-accent').trim(),
    contentBlur,
    frameInViewport: frameRect.bottom <= window.innerHeight + 1,
    horizontalFit,
    overflow: document.documentElement.scrollWidth - document.documentElement.clientWidth,
  };
})()`;

const KEYBOARD = `(async () => {
  const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
  const app = document.querySelector('[data-app]');
  const input = document.querySelector('[data-input]');
  const pal = document.querySelector('[data-palette]');
  const scrim = document.querySelector('[data-scrim]');
  const mentions = document.querySelector('[data-mentions]');
  const chord = async (letter) => {
    document.dispatchEvent(new KeyboardEvent('keydown', { key: 'x', ctrlKey: true, bubbles: true }));
    document.dispatchEvent(new KeyboardEvent('keydown', { key: letter, ctrlKey: true, bubbles: true }));
    await sleep(80);
  };
  const type = async (value) => {
    const setter = Object.getOwnPropertyDescriptor(window.HTMLTextAreaElement.prototype, 'value').set;
    setter.call(input, value);
    input.dispatchEvent(new Event('input', { bubbles: true }));
    await sleep(80);
  };
  const key = async (k) => {
    input.dispatchEvent(new KeyboardEvent('keydown', { key: k, bubbles: true }));
    await sleep(80);
  };
  const steps = [];
  const record = (step) => steps.push({
    step,
    lane: app.dataset.lane,
    palette: !pal.hidden,
    mentions: !mentions.hidden,
    scrim: scrim.dataset.open,
    value: input.value,
    rows: document.querySelectorAll('[data-pal-list] .pal-item').length,
    empty: !document.querySelector('[data-pal-empty]').hidden,
    active: (document.querySelector('[data-pal-list] .pal-item[data-active="true"]') || {}).textContent || null,
    menu: [...document.querySelectorAll('.popover[data-open="true"]')].map((p) => p.dataset.popover).join(',') || null,
    menuItems: document.querySelectorAll('.popover[data-open="true"] [data-pick]').length,
    menuChecked: (document.querySelector('.popover[data-open="true"] [data-pick][aria-checked="true"]') || {}).textContent || null,
    agentName: document.querySelector('[data-agent-name]').textContent,
    effort: document.querySelector('[data-effort-value]').textContent,
  });
  const openMenu = async (name) => {
    document.querySelector('[data-popover-toggle="' + name + '"]').click();
    await sleep(120);
  };
  const pickItem = async (name, value) => {
    document.querySelector('[data-popover="' + name + '"] [data-value="' + value + '"]').click();
    await sleep(120);
  };
  record('start');
  await chord('p'); record('ctrl-x ctrl-p hides the lane');
  await chord('p'); record('ctrl-x ctrl-p shows the lane again');
  await chord('o'); record('ctrl-x ctrl-o opens the palette');
  await type('/zzz'); record('typing a query with no match shows the empty state');
  await type('/compact'); record('a matching query narrows the list');
  await key('Escape'); record('Esc closes the palette');
  await type('@'); record('@ opens the mention completions');
  await type(''); record('clearing the draft closes the completions');
  await openMenu('lane-effort'); record('the effort trigger opens its menu above itself');
  await pickItem('lane-effort', 'high'); record('picking a level writes it into the trigger');
  await openMenu('lane-agent'); record('the agent-type trigger opens its menu');
  await pickItem('lane-agent', 'Code Reviewer'); record('picking an agent type writes it into the lane');
  return { steps, finalLane: app.dataset.lane };
})()`;

mkdirSync(OUT, { recursive: true });

const consoleErrors = [];
const remoteRequests = [];
const onEvent = (msg) => {
  if (msg.method === "Runtime.consoleAPICalled" && msg.params.type === "error") {
    consoleErrors.push(msg.params.args.map((a) => a.value ?? a.description).join(" "));
  }
  if (msg.method === "Log.entryAdded" && msg.params.entry.level === "error") {
    consoleErrors.push(msg.params.entry.text);
  }
  if (msg.method === "Network.requestWillBeSent" && !/^(file|data|blob):/.test(msg.params.request.url)) {
    remoteRequests.push(msg.params.request.url);
  }
};

const target = await openCdp();
const cdp = connect(target, onEvent);
await cdp.ready;
await cdp.send("Page.enable");
await cdp.send("Runtime.enable");
await cdp.send("Log.enable");
await cdp.send("Network.enable");

const report = { page: "lane-palette.html", shots: [], runs: [], failures: [], keyboard: null };
const failures = [];
const fail = (where, what) => failures.push(`${where}: ${what}`);

for (const [theme, spec] of Object.entries(THEMES)) {
  for (const width of WIDTHS) {
    for (const scene of SCENES) {
      const where = `${scene.name}/${theme}/${width.id}`;
      consoleErrors.length = 0;
      remoteRequests.length = 0;
      await cdp.send("Emulation.setDeviceMetricsOverride", {
        width: width.w,
        height: width.h,
        deviceScaleFactor: 1,
        mobile: false,
      });
      const url = `file://${PAGE}?theme=${theme}&width=${width.frame}&scene=${scene.name}`;
      await cdp.send("Page.navigate", { url });
      await sleep(1100);
      const probe = await evaluate(cdp, PROBE);

      if (probe.theme !== theme) fail(where, `theme is ${probe.theme}`);
      if (probe.scene !== scene.name) fail(where, `scene is ${probe.scene}`);
      if (probe.view !== scene.view) fail(where, `view is ${probe.view}`);
      if (probe.laneState !== scene.lane) fail(where, `lane state is ${probe.laneState}`);
      if (probe.laneVisible !== (scene.lane === "open")) fail(where, `lane visibility ${probe.laneVisible}`);
      if (probe.mode !== scene.mode) fail(where, `composer mode is ${probe.mode}`);
      if (probe.tokenBg.toLowerCase() !== spec.bg) fail(where, `--c-bg ${probe.tokenBg} != ${spec.bg}`);
      if (probe.overflow > 0) fail(where, `horizontal overflow ${probe.overflow}px`);
      if (!probe.frameInViewport) fail(where, "the window frame is cut off by the viewport");
      for (const item of probe.horizontalFit) {
        if (item.overflow > 1) fail(where, `${item.sel} overflows the frame by ${item.overflow}px`);
      }
      for (const item of probe.contentBlur) {
        if (item.blur !== "none") fail(where, `backdropBlur on ${item.sel} is ${item.blur}`);
      }

      if (scene.palette || scene.mention) {
        if (probe.scrimOpen !== true) fail(where, `the veil is not open with the ${scene.palette ? "palette" : "mentions menu"}`);
        if (!probe.scrimBlur || !probe.scrimBlur.includes("blur(3px)")) fail(where, `veil blur is ${probe.scrimBlur}`);
        if (!(probe.scrimAlpha < 1)) fail(where, `veil fill is opaque (alpha ${probe.scrimAlpha})`);
        if (Number(probe.scrimZ) >= Number(probe.laneZ)) fail(where, `veil z ${probe.scrimZ} is not under the lane z ${probe.laneZ}`);
      }
      if (scene.palette) {
        if (!probe.paletteOpen) fail(where, "palette is not open");
        /* Review 2026-09-16: the sheet takes the composer's box, at the same
           6px the `@` completions keep above that same input. */
        if (probe.laneVisible && Math.abs(probe.paletteRect.bottom - (probe.shellRect.y - 6)) > 2) {
          fail(where, `palette bottom ${probe.paletteRect.bottom} is not 6px above the composer top ${probe.shellRect.y}`);
        }
        if (Math.abs(probe.paletteRect.left - probe.shellRect.left) > 2 || Math.abs(probe.paletteRect.right - probe.shellRect.right) > 2) {
          fail(where, `palette ${probe.paletteRect.left}..${probe.paletteRect.right} does not span the composer ${probe.shellRect.left}..${probe.shellRect.right}`);
        }
        if (probe.paletteRect.w < probe.shellRect.w - 2) {
          fail(where, `palette width ${probe.paletteRect.w} < composer width ${probe.shellRect.w}`);
        }
        if (scene.rows === 0 && !probe.paletteEmptyVisible) fail(where, "the empty state is not shown");
        if (scene.rows > 0 && probe.paletteRows !== scene.rows) fail(where, `palette rows ${probe.paletteRows} != ${scene.rows}`);
        if (probe.paletteRows === 0 && !probe.paletteEmptyVisible) fail(where, "no rows and no empty state");
      } else {
        if (probe.paletteOpen) fail(where, "palette is open in a closed scene");
        if (probe.scrimOpen && !scene.mention) fail(where, "the veil is open with both menus closed");
        if (probe.mentionOpen !== Boolean(scene.mention)) fail(where, `mention menu ${probe.mentionOpen}`);
      }

      if (probe.approvalVisible !== (scene.mode === "approval")) fail(where, "approval strip visibility");
      /* Review 2026-09-17: only an agent-less tab blocks typing; no provider
         keeps the composer live and says why nothing sends. */
      if (probe.inputDisabled !== (scene.mode === "no-agent")) fail(where, "composer disabled state");
      if (probe.sendButtons !== 0) fail(where, "a Send button is still rendered");
      /* Review 2026-09-16: the agent view has no header any more; the controls
         live in the lane for every view. */
      if (probe.agentHeadPresent) fail(where, "the agent view still carries an agent header");
      if (!probe.laneControlsVisible) fail(where, "the lane has no agent controls");
      if (probe.laneControlsMode !== scene.mode) fail(where, `lane controls mode ${probe.laneControlsMode}`);
      if (!probe.agentKindVisible) fail(where, "the agent-type picker is not in the lane");
      if (scene.mode === "no-agent") {
        if (probe.conversationTurns !== 0) fail(where, "a tab with no agent still shows a conversation");
        if (probe.agentName !== "Attach an agent") fail(where, `agent picker says ${probe.agentName} on an agent-less tab`);
        if (probe.modelVisible || probe.effortVisible || probe.meterVisible) {
          fail(where, "model/effort/meter show on a tab with no agent");
        }
      } else if (scene.mode === "disabled") {
        if (probe.conversationTurns !== 2) fail(where, `conversation turns ${probe.conversationTurns} != 2`);
        if (!probe.modelDisabled) fail(where, "the model picker is live with no provider configured");
        if (!/Configure a provider/.test(probe.modelText)) fail(where, `model picker says ${probe.modelText}`);
        if (probe.effortVisible || probe.meterVisible) fail(where, "effort/meter show with no provider");
        /* Review 2026-09-17: the notice takes the place of the disabled input. */
        if (!probe.laneNoteVisible) fail(where, "no provider: the lane says nothing");
        if (!/Settings → Providers/.test(probe.laneNoteText)) fail(where, `lane note says ${probe.laneNoteText}`);
        if (/Configure a provider/.test(probe.inputPlaceholder)) fail(where, "the no-provider notice is still in the placeholder");
      } else {
        if (probe.laneNoteVisible) fail(where, "the no-provider notice shows in a working lane");
        if (probe.conversationTurns !== 2) fail(where, `conversation turns ${probe.conversationTurns} != 2`);
        if (!probe.modelVisible || probe.modelDisabled) fail(where, "the model picker is not live");
        if (!/\//.test(probe.modelText)) fail(where, `model shows ${probe.modelText}, not provider/model`);
        if (!probe.effortVisible) fail(where, "the effort picker is not in the lane");
        if (probe.effortText !== "medium") fail(where, `effort shows ${probe.effortText}`);
        if (!probe.meterVisible) fail(where, "the context meter is not in the lane");
      }
      /* Review 2026-09-17 (second round): Stop replaces nothing at all in the
         foot — it fills the icon slot Send used to fill — and the foot says
         what the session's environment is, nothing about the run. */
      if (probe.stopVisible !== (scene.mode === "streaming")) fail(where, "Stop visibility");
      if (probe.footRunMarkers !== 0) fail(where, "the lane foot still draws a run indicator");
      if (/Working/.test(probe.footText)) fail(where, "the lane foot still says Working");

      /* Review 2026-09-17 (second round): the run's signal is the window mark's
         accent dot — pulsing at the tab's own 1.1s while a turn is in flight,
         a steady accent dot otherwise — and it is the window's only blinking
         element: the tab's agent word and the state strip's tone dot carry their
         state by colour alone. */
      const streamingScene = scene.mode === "streaming";
      if (probe.brandBusy !== String(streamingScene)) fail(where, `the mark's busy state is ${probe.brandBusy}`);
      if (streamingScene) {
        if (!probe.brandDotAnimation || probe.brandDotAnimation.name === "none") {
          fail(where, "the mark's dot does not pulse while the agent works");
        } else if (probe.brandDotAnimation.duration !== "1.1s") {
          fail(where, `the mark's pulse is ${probe.brandDotAnimation.duration}, not the tab's 1.1s`);
        }
      } else if (probe.brandDotAnimation && probe.brandDotAnimation.name !== "none") {
        fail(where, "the mark's dot pulses with no run in flight");
      }
      if (probe.tabAgentBusy !== String(streamingScene)) fail(where, `the tab marker's busy state is ${probe.tabAgentBusy}`);
      if (probe.tabAgentAnimation && probe.tabAgentAnimation.name !== "none") {
        fail(where, "the tab marker pulses as well as the mark");
      }
      if (probe.stateDotAnimation && probe.stateDotAnimation !== "none") {
        fail(where, "the state strip's dot pulses as well as the mark");
      }
      /* Review 2026-09-17 (third round): the strip's run cue is the bars, not the
         dot — working is drawn as motion in the words, and only while a turn is
         in flight (the strip exists in the agent view only). */
      if (probe.stateStrip.visible) {
        if (streamingScene) {
          if (!probe.stateStrip.barsVisible) {
            fail(where, "the state strip shows no run cue while the agent works");
          } else if (probe.stateStrip.bars !== 3) {
            fail(where, `the state strip's run cue draws ${probe.stateStrip.bars} bars, not three`);
          } else if (!probe.stateStrip.barsAnimation || probe.stateStrip.barsAnimation.name === "none") {
            fail(where, "the state strip's bars do not pulse");
          } else if (probe.stateStrip.barsAnimation.duration !== "1.1s") {
            fail(where, `the state strip's bars pulse at ${probe.stateStrip.barsAnimation.duration}, not 1.1s`);
          }
          if (probe.stateStrip.dotVisible) {
            fail(where, "the state strip still draws its tone dot while the bars run");
          }
        } else if (probe.stateStrip.barsVisible) {
          fail(where, "the state strip's bars run with no turn in flight");
        } else if (!probe.stateStrip.dotVisible) {
          fail(where, "the state strip lost its tone dot though no turn is in flight");
        }
      }

      /* The top bar is the app's, as implemented: mark, tab strip hugging it,
         spacer, then the window actions at the right edge. */
      if (probe.titlebar.brandText !== "Clay") fail(where, `the window mark reads ${probe.titlebar.brandText}`);
      if (probe.titlebar.tabs !== 1) fail(where, `the strip holds ${probe.titlebar.tabs} window tabs`);
      if (!probe.titlebar.spacer) fail(where, "the title bar has no spacer");
      if (!probe.titlebar.commandsInActions) fail(where, "the Control Center trigger is not in the window actions");
      if (!probe.titlebar.viewSwitchInActions) fail(where, "the view switcher is not in the window actions");
      if (probe.titlebar.windowControls !== 0) fail(where, "the title bar draws window buttons the app does not have");
      if (probe.titlebar.rightGap > 16) fail(where, `the view switcher sits ${probe.titlebar.rightGap}px from the bar's edge`);
      if (probe.titlebar.commandsBeforeSwitch < 0) fail(where, "the Control Center trigger overlaps the view switcher");
      if (probe.laneVisible && !probe.laneRect.w) fail(where, "the lane has no width");

      for (const message of consoleErrors) fail(where, `console error: ${message}`);
      for (const request of remoteRequests) fail(where, `non-file request: ${request}`);

      const save = SHOTS && (width.id === "wide" || theme === "modus-operandi" || theme === "modus-vivendi");
      let file = null;
      if (save) {
        file = `${scene.name}__${theme}__${width.id}.png`;
        const shot = await cdp.send("Page.captureScreenshot", { format: "png" });
        writeFileSync(join(OUT, file), Buffer.from(shot.data, "base64"));
        report.shots.push(file);
      }
      report.runs.push({ scene: scene.name, theme, width: width.id, file, probe });
      if (!QUIET) {
        process.stdout.write(
          `${failures.length ? "FAIL" : "ok  "} ${where} rows=${probe.paletteRows} lane=${probe.laneRect ? probe.laneRect.w + "x" + probe.laneRect.h : "hidden"}\n`,
        );
      }
    }
  }
}

// Keyboard-only pass: the live path, at 1500 wide, in the light and dark themes.
for (const theme of ["modus-operandi", "modus-vivendi"]) {
  await cdp.send("Emulation.setDeviceMetricsOverride", { width: 1500, height: 1150, deviceScaleFactor: 1, mobile: false });
  await cdp.send("Page.navigate", { url: `file://${PAGE}?theme=${theme}&width=wide&scene=idle` });
  await sleep(1100);
  const walk = await evaluate(cdp, KEYBOARD);
  report.keyboard = report.keyboard ?? {};
  report.keyboard[theme] = walk;
  const steps = walk.steps;
  const expect = (index, lane, palette, mentions, scrim) => {
    const s = steps[index];
    if (s.lane !== lane) fail(`keys/${theme}`, `${s.step}: lane ${s.lane} != ${lane}`);
    if (s.palette !== palette) fail(`keys/${theme}`, `${s.step}: palette ${s.palette} != ${palette}`);
    if (s.mentions !== mentions) fail(`keys/${theme}`, `${s.step}: mentions ${s.mentions} != ${mentions}`);
    if (s.scrim !== String(scrim)) fail(`keys/${theme}`, `${s.step}: veil ${s.scrim} != ${scrim}`);
  };
  expect(0, "open", false, false, false);
  expect(1, "hidden", false, false, false);
  expect(2, "open", false, false, false);
  expect(3, "open", true, false, true);
  expect(4, "open", true, false, true);
  expect(5, "open", true, false, true);
  expect(6, "open", false, false, false);
  expect(7, "open", false, true, true);
  expect(8, "open", false, false, false);
  if (steps[4].empty !== true) fail(`keys/${theme}`, "an unmatched query did not show the empty state");
  if (!(steps[5].rows >= 1)) fail(`keys/${theme}`, "a matching query showed no rows");
  if (steps[1].lane !== "hidden") fail(`keys/${theme}`, "Ctrl+X Ctrl+P did not hide the lane");

  /* Review 2026-09-16: the lane's pickers are real dropdowns. */
  const menus = [
    { index: 9, menu: "lane-effort", items: 3, checked: "medium", after: null },
    { index: 10, menu: null, items: 0, checked: null, after: "high" },
    { index: 11, menu: "lane-agent", items: 3, checked: "Coding Agent", after: null },
    { index: 12, menu: null, items: 0, checked: null, after: "Code Reviewer" },
  ];
  for (const step of menus) {
    const s = steps[step.index];
    if (s.menu !== step.menu) fail(`keys/${theme}`, `${s.step}: open menu ${s.menu} != ${step.menu}`);
    if (s.menuItems !== step.items) fail(`keys/${theme}`, `${s.step}: ${s.menuItems} options`);
    if (step.checked && !(s.menuChecked || "").includes(step.checked)) {
      fail(`keys/${theme}`, `${s.step}: checked option ${s.menuChecked} != ${step.checked}`);
    }
    if (step.after && steps[step.index].effort !== step.after && steps[step.index].agentName !== step.after) {
      fail(`keys/${theme}`, `${s.step}: pick did not reach the trigger`);
    }
  }

  /* Evidence for the menus themselves: the same lane, the effort menu open. */
  await evaluate(
    cdp,
    `(async () => { document.querySelector('[data-popover-toggle="lane-effort"]').click(); await new Promise((r) => setTimeout(r, 200)); return document.querySelector('[data-popover="lane-effort"]').dataset.open; })()`,
  );
  const openProbe = await evaluate(
    cdp,
    `document.querySelector('[data-popover="lane-effort"]').dataset.open === 'true' && document.querySelectorAll('[data-popover="lane-effort"] [data-pick]').length`,
  );
  if (openProbe !== 3) fail(`keys/${theme}`, `the effort menu did not reopen (${openProbe})`);
  const menuShot = `menu-effort__${theme}__wide.png`;
  const menuPng = await cdp.send("Page.captureScreenshot", { format: "png" });
  writeFileSync(join(OUT, menuShot), Buffer.from(menuPng.data, "base64"));
  report.shots.push(menuShot);
}

report.failures = failures;
writeFileSync(join(OUT, "report.json"), JSON.stringify(report, null, 2));
cdp.close();

if (!QUIET) {
  process.stdout.write(
    `\n${report.runs.length} runs, ${report.shots.length} PNGs, ${failures.length} failures\n`,
  );
}
if (failures.length) {
  for (const failure of failures.slice(0, 40)) process.stdout.write(`  - ${failure}\n`);
  process.exit(1);
}