// Visual + accessibility evidence for the persistent agent lane and the
// composer palette (plan 124 tasks 6–9, extended by plan 125 task 12), driven
// against the *shipped* React surfaces through the DEV fixture route.
//
// The live Tauri build is the primary review vehicle (real shell, real veil,
// four themes); this tool covers what only a seeded fixture can render:
// streaming/approval/error turns, the palette's empty and files-scope states,
// the `@` mention dropdown, and — plan 125 — every picker stage (list, auth
// methods, secret, URL, OAuth, a 40-row model list, and the session list with
// its `Alt+↵` secondary action) plus the halo the retired drop shadow became.
// It pins the geometry the approved artifact fixes (the sheet is the composer
// box's own width, 6px above it), the row fields that come from the server
// (scope chips, chord chips, stage modes), the shielded secret stage's
// security posture, and the run signal's motion count.
//
// Usage: npm run dev (default http://localhost:5199) then
//   node design-artifacts/tools/capture-lane-palette.mjs
// Evidence lands in code-reviews/screenshots/2026-09-18-plan125-palette/ by
// default (CLAY_REVIEW_OUT overrides; plan 124's fixture-layer.json stays in
// its own directory as that plan's frozen evidence).
import { spawn } from "node:child_process";
import { existsSync, mkdirSync, readdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";

const ROOT = "/home/arn/Projects/clay";
const OUT =
  process.env.CLAY_REVIEW_OUT ??
  join(ROOT, "code-reviews/screenshots/2026-09-18-plan125-palette");
const BASE = process.env.CLAY_DEV_URL ?? "http://localhost:1420";
const PORT = 9351;
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
  const found = candidates.find(
    (candidate) => candidate && existsSync(candidate),
  );
  if (!found) throw new Error("no chrome binary found");
  return found;
}

/** Subscribe to a CDP event (the client above only answers requests). */
function cdpEvents(cdp, method, handler) {
  cdp.on(method, handler);
}

function connect(url) {
  const ws = new WebSocket(url);
  let id = 0;
  const pending = new Map();
  const listeners = new Map();
  ws.addEventListener("message", (event) => {
    const msg = JSON.parse(event.data);
    if (msg.id && pending.has(msg.id)) {
      const { resolve, reject } = pending.get(msg.id);
      pending.delete(msg.id);
      if (msg.error) reject(new Error(JSON.stringify(msg.error)));
      else resolve(msg.result);
      return;
    }
    if (msg.method && listeners.has(msg.method)) {
      for (const listener of listeners.get(msg.method))
        listener(msg.params ?? {});
    }
  });
  const ready = new Promise((resolve) => ws.addEventListener("open", resolve));
  return {
    ready,
    on(method, listener) {
      if (!listeners.has(method)) listeners.set(method, []);
      listeners.get(method).push(listener);
    },
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
  const { result, exceptionDetails } = await cdp.send("Runtime.evaluate", {
    expression,
    awaitPromise: true,
    returnByValue: true,
  });
  if (exceptionDetails) throw new Error(JSON.stringify(exceptionDetails));
  return result.value;
};

/** Type text as real key events so React's synthetic handlers see it. */
async function typeText(cdp, text) {
  for (const char of text) {
    await cdp.send("Input.dispatchKeyEvent", {
      type: "keyDown",
      text: char,
      unmodifiedText: char,
      key: char,
    });
    await cdp.send("Input.dispatchKeyEvent", { type: "keyUp", key: char });
    await sleep(40);
  }
  await sleep(350);
}

async function pressKey(cdp, key, code, extra = {}) {
  const base = { key, code, windowsVirtualKeyCode: extra.vk ?? 0 };
  await cdp.send("Input.dispatchKeyEvent", {
    type: "keyDown",
    ...base,
    ...extra,
  });
  await cdp.send("Input.dispatchKeyEvent", {
    type: "keyUp",
    ...base,
    ...extra,
  });
  await sleep(250);
}

const rect = `(el) => { if (!el) return null; const r = el.getBoundingClientRect(); return { x: Math.round(r.x), y: Math.round(r.y), w: Math.round(r.width), h: Math.round(r.height), bottom: Math.round(r.bottom), top: Math.round(r.top) }; }`;

const PROBE = `(() => {
  const rectOf = ${rect};
  /* Compare a declared CSS value with a computed one by round-tripping it. */
  const normalize = (value) => {
    if (!value) return value;
    const probe = document.createElement("span");
    probe.style.color = value;
    document.body.appendChild(probe);
    const resolved = getComputedStyle(probe).color;
    probe.remove();
    return resolved;
  };
  const sheet = document.querySelector("[data-testid='command-palette']");
  const shell = document.querySelector("[data-clay-component='textInput'][data-clay-slot='field']");
  const lane = document.querySelector("[aria-label='Agent lane']");
  const laneRect = rectOf(lane);
  const cs = sheet ? getComputedStyle(sheet) : null;
  const animations = [];
  for (const el of document.querySelectorAll("*")) {
    let node = el;
    let animated = false;
    while (node && node !== document.documentElement) {
      const style = getComputedStyle(node);
      if (style.animationName && style.animationName !== "none") {
        animated = true;
        break;
      }
      node = node.parentElement;
    }
    if (animated) {
      animations.push(
        (el.getAttribute("class") || el.tagName) +
          " :: " + getComputedStyle(el).animationName,
      );
      break;
    }
  }
  const ownAnimations = [...document.querySelectorAll("*")].filter((el) => {
    const style = getComputedStyle(el);
    return style.animationName && style.animationName !== "none";
  });
  const rows = [...document.querySelectorAll("[role='option']")];
  const approval = document.querySelector("[role='alertdialog']");
  return {
    fixture: document.querySelector("[data-fixture]")?.getAttribute("data-fixture") ?? null,
    lane: laneRect,
    laneText: lane ? lane.innerText.replace(/\\s+/g, " ").slice(0, 260) : null,
    laneFoot: lane ? (lane.querySelector("[class*='foot']")?.innerText ?? "").replace(/\\s+/g, " ") : null,
    laneSendButtons: lane ? [...lane.querySelectorAll("button")].map((b) => b.textContent).filter((t) => /send/i.test(t)) : [],
    laneFieldDisabled: (() => {
      const field = lane?.querySelector("[data-clay-component='textInput'][data-clay-slot='field'] input, " +
        "[data-clay-component='textInput'][data-clay-slot='field'] textarea");
      return field ? field.disabled : null;
    })(),
    laneFootNote: lane
      ? (lane.innerText
          .split("\\n")
          .find((line) => /no (?:provider configured|agent on this tab)/.test(line)) ?? null)
      : null,
    sheet: rectOf(sheet),
    sheetAttrs: sheet ? {
      role: sheet.getAttribute("role"),
      label: sheet.getAttribute("aria-label"),
      /* commandCentre has no component kind (it is host chrome), so the
       * recipe is proven by the paint matching its own variables. */
      recipeBackground: (() => {
        const declared = normalize(
          getComputedStyle(document.documentElement)
            .getPropertyValue("--clay-ds-command-centre-default-root-rest-background-color")
            .trim(),
        );
        return { declared, computed: getComputedStyle(sheet).backgroundColor };
      })(),
      recipeRadius: (() => {
        const declared = getComputedStyle(document.documentElement)
          .getPropertyValue("--clay-ds-command-centre-default-root-rest-border-radius")
          .trim();
        return { declared, computed: getComputedStyle(sheet).borderTopLeftRadius };
      })(),
    } : null,
    shell: rectOf(shell),
    gapAboveShell: sheet && shell
      ? Math.round(shell.getBoundingClientRect().top - sheet.getBoundingClientRect().bottom)
      : null,
    widthDeltaShell: sheet && shell
      ? Math.round(sheet.getBoundingClientRect().width - shell.getBoundingClientRect().width)
      : null,
    sheetPaint: cs ? {
      background: cs.backgroundColor,
      radius: cs.borderTopLeftRadius,
      maxHeight: cs.maxHeight,
      animation: cs.animationName + " " + cs.animationDuration,
      backdropBlur: null,
    } : null,
    /* Plan 125: the approved sheet shadow is the *halo* (two zero-offset
     * layers of the text role), not the retired elevation.overlay drop
     * shadow. Both the shipped recipe variable and the painted value are
     * recorded so a regression shows up as a diff, not a hunch. */
    halo: (() => {
      const declared = getComputedStyle(document.documentElement)
        .getPropertyValue("--clay-ds-command-centre-default-root-rest-shadow")
        .trim();
      return { declared, computed: cs ? cs.boxShadow : null };
    })(),
    /* The height cap and the internal scroll are the sheet's own contract:
     * a catalogue bigger than the cap scrolls inside it, it never grows. */
    list: (() => {
      const box = sheet?.querySelector("[class*='palList']");
      if (!box) return null;
      return {
        scrollHeight: box.scrollHeight,
        clientHeight: box.clientHeight,
        rows: box.querySelectorAll("[role='option']").length,
      };
    })(),
    /* The shielded stage's field lives *inside* the sheet and is the only
     * input the palette owns (DESIGN.md §13.11). */
    stageField: (() => {
      const field = sheet?.querySelector("input[type='password']");
      if (!field) return null;
      return {
        type: field.type,
        length: field.value.length,
        focused: document.activeElement === field,
        name: field.getAttribute("aria-label"),
      };
    })(),
    composerDisabled: (() => {
      const field = document.querySelector(
        "[data-clay-component='textInput'][data-clay-slot='field'] input, " +
          "[data-clay-component='textInput'][data-clay-slot='field'] textarea",
      );
      return field ? field.disabled : null;
    })(),
    composerDraft: (() => {
      const field = document.querySelector(
        "[aria-label='Agent lane'] textarea, [aria-label='Agent lane'] input",
      );
      return field ? field.value : null;
    })(),
    prompt: sheet?.querySelector("[class*='prompt']")?.innerText ?? null,
    foot: sheet?.querySelector("footer")?.innerText.replace(/\\s+/g, " ") ?? null,
    mode: sheet?.getAttribute("data-mode") ?? null,
    /* The @ mentions menu takes the same halo (plan 125) and stays a narrow
     * dropdown: no scrim, no sheet. */
    mentions: (() => {
      const menu = document.querySelector("[aria-label='Mentions']");
      if (!menu) return null;
      return {
        shadow: getComputedStyle(menu).boxShadow,
        declared: getComputedStyle(document.documentElement)
          .getPropertyValue("--clay-ds-menu-default-root-rest-shadow")
          .trim(),
        rect: rectOf(menu),
      };
    })(),
    veil: [...document.querySelectorAll("[data-clay-slot='scrim']")].map((n) => {
      const style = getComputedStyle(n);
      return {
        component: n.getAttribute("data-clay-component"),
        background: style.backgroundColor,
        backdropFilter: style.backdropFilter || style.webkitBackdropFilter,
        ariaHidden: n.getAttribute("aria-hidden"),
      };
    }),
    rows: rows.map((row) => ({
      text: row.innerText.replace(/\\s+/g, " "),
      selected: row.getAttribute("aria-selected"),
      kbd: [...row.querySelectorAll("kbd")].map((k) => k.textContent),
      scope: row.dataset.scope ?? null,
    })),
    count: sheet?.querySelector("output")?.textContent ?? null,
    scopes: [...document.querySelectorAll("[data-scope]")].map(
      (b) => b.textContent + (b.getAttribute("aria-pressed") === "true" ? "*" : ""),
    ),
    empty: sheet?.querySelector("div[role='status']")?.innerText ?? null,
    approval: approval ? {
      label: approval.getAttribute("aria-label"),
      text: approval.innerText.replace(/\\s+/g, " "),
      focus: document.activeElement?.textContent ?? null,
    } : null,
    workingBars: document.querySelectorAll("[class*='workingBars'] > span").length,
    stateStripText: document.querySelector("[class*='stateStrip']")?.innerText ?? null,
    animations: animations.slice(0, 8),
    ownAnimations: ownAnimations.map(
      (el) => (el.getAttribute("class") || el.tagName) + " :: " + getComputedStyle(el).animationName,
    ).slice(0, 8),
    tabMarkerAnimation: (() => {
      const marker = document.querySelector("[class*='tabMarker'], [class*='marker']");
      return marker ? getComputedStyle(marker).animationName : null;
    })(),
  };
})()`;

const SCENES = [
  { id: "palette-open", url: "/?fixture=command-centre", widths: [1500, 1024] },
  {
    id: "palette-empty",
    url: "/?fixture=command-centre-empty",
    widths: [1500, 1024],
  },
  { id: "palette-files", url: "/?fixture=path-browser", widths: [1500] },
  // Plan 125: every picker stage renders on the same sheet, each with its own
  // mode, prompt, and foot verb. A 40-row model list covers the height cap and
  // the internal scroll; the session list carries the `Alt+↵` secondary action.
  {
    id: "palette-stage-providers",
    url: "/?fixture=command-centre&stage=providers",
    widths: [1500, 1024],
  },
  {
    id: "palette-stage-auth",
    url: "/?fixture=command-centre&stage=auth",
    widths: [1500],
  },
  {
    id: "palette-stage-secret",
    url: "/?fixture=command-centre&stage=secret",
    widths: [1500, 1024],
    type: "sk-live-review-secret",
    typeTarget: "sheet",
  },
  {
    id: "palette-stage-url",
    url: "/?fixture=command-centre&stage=url",
    widths: [1500],
  },
  {
    id: "palette-stage-oauth",
    url: "/?fixture=command-centre&stage=oauth",
    widths: [1500, 1024],
  },
  {
    id: "palette-stage-models",
    url: "/?fixture=command-centre&stage=models",
    widths: [1500, 1024],
  },
  {
    id: "palette-stage-sessions",
    url: "/?fixture=command-centre&stage=sessions",
    widths: [1500],
  },
  {
    id: "lane-landing",
    url: "/?fixture=coding-agent&state=landing",
    widths: [1500, 1024],
  },
  {
    id: "lane-conversation",
    url: "/?fixture=coding-agent&state=conversation",
    widths: [1500, 1024],
  },
  {
    id: "lane-mentions",
    url: "/?fixture=coding-agent&state=conversation",
    type: "@",
    widths: [1500],
  },
  {
    id: "lane-streaming",
    url: "/?fixture=coding-agent&state=streaming",
    widths: [1500, 1024],
  },
  {
    id: "lane-approval",
    url: "/?fixture=coding-agent&state=approval",
    widths: [1500, 1024],
  },
  {
    id: "lane-error",
    url: "/?fixture=coding-agent&state=error",
    widths: [1500],
  },
];

/** The prompt and foot verb each stage's mode fixes (DESIGN.md §12). */
const STAGE_EXPECTATIONS = {
  "palette-stage-providers": {
    prompt: "Configure provider",
    mode: "picker",
    verb: "choose",
  },
  "palette-stage-auth": {
    prompt: "Choose sign-in method",
    mode: "picker",
    verb: "choose",
  },
  "palette-stage-secret": {
    prompt: "API key (hidden)",
    mode: "secret",
    verb: "store",
  },
  "palette-stage-url": { prompt: "API base URL", mode: "url", verb: "save" },
  "palette-stage-oauth": {
    prompt: "Authorize provider",
    mode: "oauth",
    verb: "run",
  },
  "palette-stage-models": { prompt: "Models", mode: "picker", verb: "choose" },
  "palette-stage-sessions": {
    prompt: "Sessions",
    mode: "picker",
    verb: "resume",
  },
};

function assert(checks, name, ok, detail) {
  checks.push({ scene: name, ok: Boolean(ok), detail: String(detail ?? "") });
}

/**
 * Split a CSS shadow list into layers, each `{ color, lengths }`, so a
 * declared recipe value and a browser-serialized computed value can be
 * compared by meaning (the two spell `0 0 14px -2px` vs
 * `rgba(...) 0px 0px 14px -2px`).
 */
function shadowLayers(value) {
  if (!value || value === "none") return [];
  const raw = [];
  let depth = 0;
  let current = "";
  for (const char of value) {
    if (char === "(") depth += 1;
    if (char === ")") depth -= 1;
    if (char === "," && depth === 0) {
      raw.push(current);
      current = "";
      continue;
    }
    current += char;
  }
  if (current.trim()) raw.push(current);
  return raw.map((layer) => {
    const colorRegex = /rgba?\([^)]*\)|color-mix\([^)]*\)|color\([^)]*\)/;
    const color = (layer.match(colorRegex) ?? [""])[0].replace(/\s+/g, "");
    const lengths = layer
      .replace(colorRegex, "")
      .trim()
      .split(/\s+/)
      .filter(Boolean)
      .map((length) => parseFloat(length));
    return { color, lengths };
  });
}

/** Alpha of a computed `rgba(...)` layer; `rgb(...)` is fully opaque. */
function shadowAlpha(color) {
  const rgba = color.match(/rgba?\(([^)]*)\)/)?.[1].split(",") ?? [];
  if (rgba.length === 4) return Number(rgba[3]);
  // `color(srgb r g b / a)` — what Chromium serializes a resolved
  // `color-mix()` to (the halo's recipe value).
  const modern = color.match(/color\([^)]*\/([^)]*)\)/)?.[1];
  if (modern !== undefined) return Number(modern);
  // The shipped recipes spell translucency as `color-mix(in srgb, <color> N%,
  // transparent)`; its alpha is N/100.
  const mixed = color.match(/color-mix\([^)]*?([\d.]+)%[^)]*\)/)?.[1];
  return mixed === undefined ? 1 : Number(mixed) / 100;
}

/** A declared recipe shadow (`0 0 14px -2px color-mix(...)`) vs a computed
 *  one: compare layer count, offsets, blurs, and alpha, never the spelling. */
function sameShadow(declared, computed) {
  const a = shadowLayers(declared);
  const b = shadowLayers(computed);
  if (a.length === 0 || a.length !== b.length) return false;
  return a.every((layer, index) => {
    const other = b[index];
    const lengths = (value) =>
      value.lengths.filter((n) => Number.isFinite(n)).slice(0, 3);
    const [ax, ay, ablur] = lengths(layer);
    const [bx, by, bblur] = lengths(other);
    return (
      Math.abs(ax - bx) < 0.01 &&
      Math.abs(ay - by) < 0.01 &&
      Math.abs(ablur - bblur) < 0.01 &&
      Math.abs(shadowAlpha(layer.color) - shadowAlpha(other.color)) < 0.01
    );
  });
}

/**
 * The plan 125 halo: two layers, both with zero offset, blur 14px and 3px, and
 * a faint alpha — a drop shadow has an offset (the retired
 * `elevation.overlay` did), so this fails loudly if one comes back.
 */
function isHalo(value) {
  const layers = shadowLayers(value);
  if (layers.length !== 2) return false;
  const blurs = layers.map((layer) => layer.lengths[2]);
  if (!(Math.abs(blurs[0] - 14) < 0.01 && Math.abs(blurs[1] - 3) < 0.01))
    return false;
  return layers.every(
    (layer) =>
      layer.lengths[0] === 0 &&
      layer.lengths[1] === 0 &&
      shadowAlpha(layer.color) > 0 &&
      shadowAlpha(layer.color) <= 0.2,
  );
}

mkdirSync(OUT, { recursive: true });
const chrome = spawn(
  findChrome(),
  [
    "--headless=new",
    `--remote-debugging-port=${PORT}`,
    "--no-first-run",
    "--no-default-browser-check",
    "--disable-gpu",
    "--hide-scrollbars",
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
await cdp.send("Accessibility.enable");
await cdp.send("Log.enable");
// Console/perf hygiene: the review records every console message the shipped
// surfaces emit while a fixture runs, including typing, so a warning or an
// error introduced by the palette shows up as evidence rather than a hunch.
const consoleMessages = [];
cdpEvents(cdp, "Runtime.consoleAPICalled", (params) => {
  const text = (params.args ?? [])
    .map((arg) => arg.value ?? arg.description ?? "")
    .join(" ");
  consoleMessages.push({ level: params.type, text, scene: currentScene });
});
cdpEvents(cdp, "Log.entryAdded", (params) => {
  consoleMessages.push({
    level: params.entry?.level ?? "log",
    text: params.entry?.text ?? "",
    // The URL is what makes a resource failure classifiable: the message text
    // alone ("Failed to load resource") names no resource.
    url: params.entry?.url ?? null,
    scene: currentScene,
  });
});
cdpEvents(cdp, "Runtime.exceptionThrown", (params) => {
  const details = params.exceptionDetails ?? {};
  consoleMessages.push({
    level: "error",
    text: `${details.text ?? "exception"}: ${
      details.exception?.description ?? details.exception?.value ?? ""
    }`
      .replace(/\s+/g, " ")
      .slice(0, 400),
    scene: currentScene,
    url: details.url ?? null,
  });
});

const checks = [];
const evidence = [];
let currentScene = "startup";
for (const scene of SCENES) {
  currentScene = scene.id;
  for (const width of scene.widths) {
    await cdp.send("Emulation.setDeviceMetricsOverride", {
      width,
      height: 940,
      deviceScaleFactor: 2,
      mobile: false,
    });
    await cdp.send("Page.navigate", { url: `${BASE}${scene.url}` });
    await sleep(2200);
    if (scene.type) {
      const selector =
        scene.typeTarget === "sheet"
          ? "[data-testid='command-palette'] input[type='password']"
          : "[aria-label='Agent lane'] textarea, [aria-label='Agent lane'] input";
      const field = await evaluate(
        cdp,
        `(() => { const el = document.querySelector(${JSON.stringify(selector)}); if (!el) return false; el.focus(); return true; })()`,
      );
      if (!field) throw new Error(`${scene.id}: typing target not found`);
      await typeText(cdp, scene.type);
    }
    const probe = await evaluate(cdp, PROBE);
    if (scene.typeTarget === "sheet") {
      // The approved contract: the characters are never rendered, never
      // echoed, and never reach the composer's persisted draft. The shield's
      // own input is the only node allowed to carry the value.
      const leaked = await evaluate(
        cdp,
        `(() => { const secret = ${JSON.stringify(scene.type)};
          const shield = document.querySelector("[data-testid='command-palette'] input[type='password']");
          const offenders = [...document.querySelectorAll("*")]
            .filter((el) => el !== shield && [...el.attributes].some((a) => a.value.includes(secret)))
            .slice(0, 3)
            .map((el) => el.tagName.toLowerCase() + "[" + [...el.attributes].filter((a) => a.value.includes(secret)).map((a) => a.name).join(",") + "]");
          return {
            rendered: document.body.innerText.includes(secret),
            otherAttributes: offenders.length > 0,
            offenders,
            /* React mirrors a controlled input's value into the DOM's own
             * value attribute (true of every Clay text field, not just the
             * shield); recorded, not asserted, because the approved contract is
             * never-rendered / never-persisted / never-echoed. */
            shieldValueAttribute: shield ? shield.getAttribute("value") !== null : null,
          };
        })()`,
      );
      probe.secretLeak = leaked;
    }
    const dir = join(OUT, scene.id, String(width));
    mkdirSync(dir, { recursive: true });
    const shot = await cdp.send("Page.captureScreenshot", { format: "png" });
    writeFileSync(join(dir, "fixture.png"), Buffer.from(shot.data, "base64"));
    writeFileSync(join(dir, "probe.json"), JSON.stringify(probe, null, 2));
    const { nodes } = await cdp.send("Accessibility.getFullAXTree");
    const lines = [];
    const walk = (node, depth) => {
      const role = node.role?.value ?? "";
      const name = node.name?.value ?? "";
      if (role && !["none", "generic", "InlineTextBox"].includes(role)) {
        const properties = (node.properties ?? [])
          .filter((property) =>
            [
              "focused",
              "disabled",
              "selected",
              "expanded",
              "live",
              "modal",
            ].includes(property.name),
          )
          .map(
            (property) =>
              `${property.name}=${JSON.stringify(property.value?.value)}`,
          );
        lines.push(
          `${"  ".repeat(depth)}${role} "${name}"${properties.length ? " " + properties.join(" ") : ""}`,
        );
      }
      for (const childId of node.childIds ?? []) {
        const child = nodes.find((entry) => entry.nodeId === childId);
        if (child) walk(child, depth + 1);
      }
    };
    const root = nodes.find((node) => !node.parentId);
    if (root) walk(root, 0);
    writeFileSync(join(dir, "ax.txt"), lines.join("\n"));
    // Plan 125's accessibility contract, read from the same tree the platform
    // sees: the sheet is a dialog named by the session prompt, its rows are
    // options, and a shielded field never carries a value.
    const axPalette = nodes.find((node) => node.role?.value === "dialog");
    const axOptions = nodes.filter((node) => node.role?.value === "option");
    const axSecret =
      nodes.find(
        (node) =>
          ["textField", "textbox", "password"].includes(
            node.role?.value ?? "",
          ) && (node.name?.value ?? "") === "API key (hidden)",
      ) ?? null;
    evidence.push({ scene: scene.id, width, probe });
    console.log(
      `${scene.id} @${width}: sheet=${probe.sheet ? probe.sheet.w + "x" + probe.sheet.h : "none"} gap=${probe.gapAboveShell} rows=${probe.rows.length} anim=${probe.ownAnimations.length}`,
    );

    const label = `${scene.id}@${width}`;
    if (scene.id.startsWith("palette")) {
      assert(checks, label, probe.sheet, "palette sheet present");
      assert(
        checks,
        label,
        probe.widthDeltaShell === 0,
        `sheet matches the field width (delta ${probe.widthDeltaShell})`,
      );
      assert(
        checks,
        label,
        probe.gapAboveShell === 6,
        `sheet sits 6px above the field (measured ${probe.gapAboveShell})`,
      );
      assert(
        checks,
        label,
        probe.sheetAttrs?.recipeBackground.declared ===
          probe.sheetAttrs?.recipeBackground.computed,
        `sheet fill is the commandCentre recipe's own (${JSON.stringify(
          probe.sheetAttrs?.recipeBackground,
        )})`,
      );
      assert(
        checks,
        label,
        probe.sheetAttrs?.recipeRadius.declared !== "" &&
          probe.sheetAttrs?.recipeRadius.declared ===
            probe.sheetAttrs?.recipeRadius.computed,
        `sheet radius is the commandCentre recipe's own (${JSON.stringify(
          probe.sheetAttrs?.recipeRadius,
        )})`,
      );
      if (scene.id !== "palette-empty") {
        assert(checks, label, probe.rows.length > 0, "rows delivered");
        assert(
          checks,
          label,
          probe.count?.includes(String(probe.rows.length)) ?? false,
          `count echoes rows (${probe.count})`,
        );
      }
    }
    if (scene.id === "palette-open") {
      assert(
        checks,
        label,
        probe.scopes.join(",") === "All*,Session,Shell,Files",
        `scope segment ${probe.scopes.join(",")}`,
      );
      const chordRow = probe.rows.find((row) => row.kbd.length > 0);
      assert(
        checks,
        label,
        chordRow?.kbd.join(" ") === "Ctrl+X Ctrl+P",
        `chord chips from bindings (${chordRow?.kbd.join(" ")})`,
      );
      const bareRow = probe.rows.find((row) => row.text.startsWith("/compact"));
      assert(
        checks,
        label,
        bareRow && bareRow.kbd.length === 0,
        "a row without bindings draws no chord chip",
      );
      assert(
        checks,
        label,
        probe.ownAnimations.some((entry) => entry.includes("clay-palette-in")),
        `entrance animation ${probe.ownAnimations.join("; ")}`,
      );
    }
    if (scene.id === "palette-empty") {
      assert(
        checks,
        label,
        probe.empty && probe.empty.length > 0,
        `empty state text (${probe.empty})`,
      );
      assert(
        checks,
        label,
        probe.scopes.length === 0,
        "no scope segment without scope-bearing rows",
      );
    }
    if (scene.id === "palette-files") {
      assert(
        checks,
        label,
        probe.rows.length === 2 &&
          probe.rows.every((row) => /src\/|README\.md/.test(row.text)),
        `files-scope rows (${probe.rows.map((row) => row.text).join(" | ")})`,
      );
      assert(
        checks,
        label,
        probe.count?.includes("2") ?? false,
        `files count (${probe.count})`,
      );
    }
    if (scene.id === "lane-landing") {
      assert(
        checks,
        label,
        probe.laneFieldDisabled === false,
        `composer stays typeable without a provider (disabled=${probe.laneFieldDisabled})`,
      );
      assert(
        checks,
        label,
        probe.laneFootNote !== null,
        `no-provider note is text in the foot (${probe.laneFootNote})`,
      );
      assert(
        checks,
        label,
        probe.laneSendButtons.length === 0,
        "no Send button in the lane",
      );
    }
    if (scene.id === "lane-mentions") {
      assert(
        checks,
        label,
        probe.rows.length > 0,
        `mention dropdown rows (${probe.rows.length})`,
      );
      assert(checks, label, probe.mentions, "mentions menu is its own listbox");
      assert(
        checks,
        label,
        sameShadow(probe.mentions?.declared, probe.mentions?.shadow),
        `mentions take the recipe's own shadow (${JSON.stringify(probe.mentions?.shadow)} vs ${probe.mentions?.declared})`,
      );
      assert(
        checks,
        label,
        isHalo(probe.mentions?.shadow),
        `mentions shadow is the halo, not a drop shadow (${probe.mentions?.shadow})`,
      );
      assert(
        checks,
        label,
        probe.veil.length === 0,
        `the mentions menu stays a narrow dropdown: no scrim (${probe.veil.length})`,
      );
    }
    const stage = STAGE_EXPECTATIONS[scene.id];
    if (stage) {
      assert(
        checks,
        label,
        probe.mode === stage.mode,
        `stage mode ${probe.mode}`,
      );
      assert(
        checks,
        label,
        (probe.prompt ?? "").toLowerCase() === stage.prompt.toLowerCase(),
        `stage prompt ${JSON.stringify(probe.prompt)}`,
      );
      assert(
        checks,
        label,
        (probe.foot ?? "").toLowerCase().includes(stage.verb),
        `foot verb "${stage.verb}" (${probe.foot})`,
      );
      assert(
        checks,
        label,
        probe.rows.length > 0,
        `stage rows (${probe.rows.length})`,
      );
      // Plan 125: the shadow is the halo — zero offset, real blur, two layers.
      assert(
        checks,
        label,
        sameShadow(probe.halo?.declared, probe.halo?.computed),
        `sheet paints the recipe's own shadow (${JSON.stringify(probe.halo)})`,
      );
      assert(
        checks,
        label,
        isHalo(probe.halo?.computed),
        `sheet shadow is the halo, not a drop shadow (${probe.halo?.computed})`,
      );
      assert(
        checks,
        label,
        /14px/.test(probe.halo?.declared ?? "") &&
          /3px/.test(probe.halo?.declared ?? ""),
        `recipe declares both halo layers (${probe.halo?.declared})`,
      );
      // The veil is WorkspacePanes' own grid item (the fixture mounts the lane
      // alone), so its coverage is proven in the live pass, not here.
      assert(
        checks,
        label,
        axPalette?.name?.value === stage.prompt,
        `the sheet's accessible name is the session prompt (${axPalette?.name?.value})`,
      );
      assert(
        checks,
        label,
        axOptions.length > 0 && axOptions.length <= probe.rows.length,
        `rows are options in the AX tree (${axOptions.length}/${probe.rows.length})`,
      );
      assert(
        checks,
        label,
        axOptions.some((option) => (option.name?.value ?? "").length > 0),
        "the AX options carry their row labels",
      );
      if (scene.id === "palette-stage-secret") {
        assert(
          checks,
          label,
          axSecret !== null && /^•+$/.test(axSecret.value?.value ?? ""),
          `AT sees only the mask on the shield (${JSON.stringify(axSecret?.value?.value)})`,
        );
      }
    }
    if (scene.id === "palette-stage-secret") {
      assert(
        checks,
        label,
        probe.stageField,
        "shielded field inside the sheet",
      );
      assert(
        checks,
        label,
        probe.stageField?.type === "password",
        `the field is a password input (${probe.stageField?.type})`,
      );
      assert(
        checks,
        label,
        probe.stageField?.focused === true,
        "focus sits in the shielded field",
      );
      assert(
        checks,
        label,
        probe.stageField?.length === scene.type.length,
        `field holds the typed value privately (${probe.stageField?.length}/${scene.type.length})`,
      );
      assert(
        checks,
        label,
        probe.composerDisabled === true,
        `composer is inert during the secret stage (disabled=${probe.composerDisabled})`,
      );
      assert(
        checks,
        label,
        probe.composerDraft === "",
        `no secret in the composer draft (${JSON.stringify(probe.composerDraft)})`,
      );
      assert(
        checks,
        label,
        probe.secretLeak?.rendered === false,
        `the characters are never rendered (${JSON.stringify(probe.secretLeak)})`,
      );
      assert(
        checks,
        label,
        probe.secretLeak?.otherAttributes === false,
        `only the shield's own input carries the value (${JSON.stringify(probe.secretLeak)})`,
      );
    }
    if (scene.id === "palette-stage-models") {
      assert(
        checks,
        label,
        probe.list?.rows === 40,
        `40 model rows (${probe.list?.rows})`,
      );
      assert(
        checks,
        label,
        (probe.list?.scrollHeight ?? 0) > (probe.list?.clientHeight ?? 0),
        `the list scrolls inside the cap (${probe.list?.scrollHeight} > ${probe.list?.clientHeight})`,
      );
      assert(
        checks,
        label,
        /420px/.test(probe.sheetPaint?.maxHeight ?? ""),
        `sheet declares the 420px cap (${probe.sheetPaint?.maxHeight})`,
      );
    }
    if (scene.id === "palette-stage-sessions") {
      const chip = probe.rows.find((row) => row.kbd.length > 0);
      assert(
        checks,
        label,
        chip?.kbd.join(" ") === "Alt+↵",
        `session rows carry the secondary binding (${chip?.kbd.join(" ")})`,
      );
      assert(
        checks,
        label,
        /Alt\+/.test(probe.foot ?? ""),
        `foot spells the secondary action (${probe.foot})`,
      );
    }
    if (scene.id === "lane-streaming") {
      assert(
        checks,
        label,
        probe.workingBars === 3,
        `three working bars (${probe.workingBars})`,
      );
      const animated = new Set(
        probe.ownAnimations.map((entry) => entry.split(":: ")[1]),
      );
      assert(
        checks,
        label,
        animated.size === 1 && [...animated][0].includes("workingBar"),
        `one animation while running: ${[...animated].join(",")}`,
      );
      assert(
        checks,
        label,
        probe.stateStripText?.includes("Working") ?? false,
        `strip announces the run (${probe.stateStripText})`,
      );
      assert(
        checks,
        label,
        !/working|streaming/i.test(probe.laneFoot ?? ""),
        `lane foot carries no run cue (${probe.laneFoot})`,
      );
      for (const width of [1500, 1024]) {
        const dir = join(OUT, scene.id, String(width));
        if (!existsSync(join(dir, "probe.json"))) continue;
        const other = JSON.parse(
          (await import("node:fs")).readFileSync(
            join(dir, "probe.json"),
            "utf8",
          ),
        );
        assert(
          checks,
          `${scene.id}@${width}`,
          other.tabMarkerAnimation === "none" ||
            other.tabMarkerAnimation === null,
          `tab marker is not animated (${other.tabMarkerAnimation})`,
        );
      }
    }
    if (scene.id === "lane-approval") {
      assert(checks, label, probe.approval, "approval strip present");
      assert(
        checks,
        label,
        probe.approval?.label === "Tool approval",
        `approval label ${probe.approval?.label}`,
      );
      assert(
        checks,
        label,
        /Allow/.test(probe.approval?.focus ?? ""),
        `focus lands on the first action (${probe.approval?.focus})`,
      );
    }
    if (scene.id === "lane-conversation") {
      assert(
        checks,
        label,
        probe.laneText?.includes("Mock Mini") ?? false,
        "model trigger shows the configured model",
      );
      assert(
        checks,
        label,
        probe.laneSendButtons.length === 0,
        "no Send button in the lane",
      );
    }
  }
}

cdp.close();
chrome.kill("SIGKILL");
/* Fixture-harness noise, excluded by name so a *product* warning still fails
 * the run: (1) the DEV-only `detached()` warn for a refused fire-and-forget
 * call, and the unhandled rejection it logs, both because this route's
 * workspace `send` is an inert stub (`lib/detached.ts`, fixture route); (2)
 * React's render-phase-update error from the fixture seeding its session
 * snapshot while rendering its own store — the real shell receives envelopes
 * from the bridge asynchronously (plan 124 fixture, pre-existing). */
const FIXTURE_ONLY = [
  /clay: detached call failed/,
  /Uncaught \(in promise\): Object/,
  /Cannot update a component .* while rendering a different component/,
];
/* Chrome asks the dev server for `/favicon.ico` on its own; the DEV route ships
 * none, so the request 404s. Anchored on the URL (not the text) so a missing
 * product asset still fails the run. */
const BROWSER_ONLY_URLS = [/^http[^ ]*\/favicon\.ico$/];
const noisy = consoleMessages.filter(
  (message) =>
    ["error", "warning", "warn", "assert"].includes(String(message.level)) &&
    !FIXTURE_ONLY.some((pattern) => pattern.test(message.text)) &&
    !BROWSER_ONLY_URLS.some((pattern) =>
      pattern.test(String(message.url ?? "")),
    ),
);
for (const message of noisy) {
  assert(
    checks,
    `console@${message.scene}`,
    false,
    `${message.level}: ${message.text.slice(0, 200)}`,
  );
}
const failures = checks.filter((check) => !check.ok);
writeFileSync(
  join(OUT, "fixture-layer.json"),
  JSON.stringify(
    { checks, evidence, console: consoleMessages, failures: failures.length },
    null,
    2,
  ),
);
const fixtureOnly = consoleMessages.filter((message) =>
  FIXTURE_ONLY.some((pattern) => pattern.test(message.text)),
).length;
console.log(
  `console messages: ${consoleMessages.length} ` +
    `(fixture-harness: ${fixtureOnly}, product warnings/errors: ${noisy.length})`,
);
console.log(
  `checks: ${checks.length - failures.length}/${checks.length} passed`,
);
for (const failure of failures) {
  console.log(`FAIL ${failure.scene}: ${failure.detail}`);
}
process.exit(failures.length ? 1 : 0);
