// Visual + accessibility evidence for the persistent agent lane and the `/`
// palette (plan 124 tasks 6–9), driven against the *shipped* React surfaces
// through the DEV fixture route.
//
// The live Tauri build is the primary review vehicle (real shell, real veil,
// four themes); this tool covers what only a seeded fixture can render:
// streaming/approval/error turns, the palette's empty and files-scope states,
// and the `@` mention dropdown. It pins the geometry the approved artifact
// fixes (the sheet is the composer box's own width, 6px above it), the row
// fields that come from the server (scope chips, chord chips), and the run
// signal's motion count (the strip's working bars are the frame's only
// animation; the retired tab-marker pulse must not be back).
//
// Usage: npm run dev (default http://localhost:5199) then
//   node design-artifacts/tools/capture-lane-palette.mjs
// Evidence lands in code-reviews/screenshots/2026-09-17-plan124-agent-lane-palette/.
import { spawn } from "node:child_process";
import { existsSync, mkdirSync, readdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";

const ROOT = "/home/arn/Projects/clay";
const OUT =
  process.env.CLAY_REVIEW_OUT ??
  join(
    ROOT,
    "code-reviews/screenshots/2026-09-17-plan124-agent-lane-palette",
  );
const BASE = process.env.CLAY_DEV_URL ?? "http://localhost:5199";
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
  await cdp.send("Input.dispatchKeyEvent", { type: "keyDown", ...base, ...extra });
  await cdp.send("Input.dispatchKeyEvent", { type: "keyUp", ...base, ...extra });
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
  { id: "palette-empty", url: "/?fixture=command-centre-empty", widths: [1500, 1024] },
  { id: "palette-files", url: "/?fixture=path-browser", widths: [1500] },
  { id: "lane-landing", url: "/?fixture=coding-agent&state=landing", widths: [1500, 1024] },
  { id: "lane-conversation", url: "/?fixture=coding-agent&state=conversation", widths: [1500, 1024] },
  { id: "lane-mentions", url: "/?fixture=coding-agent&state=conversation", type: "@", widths: [1500] },
  { id: "lane-streaming", url: "/?fixture=coding-agent&state=streaming", widths: [1500, 1024] },
  { id: "lane-approval", url: "/?fixture=coding-agent&state=approval", widths: [1500, 1024] },
  { id: "lane-error", url: "/?fixture=coding-agent&state=error", widths: [1500] },
];

function assert(checks, name, ok, detail) {
  checks.push({ scene: name, ok: Boolean(ok), detail: String(detail ?? "") });
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

const checks = [];
const evidence = [];
for (const scene of SCENES) {
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
      const field = await evaluate(
        cdp,
        `(() => { const el = document.querySelector("[aria-label='Agent lane'] textarea, [aria-label='Agent lane'] input"); if (!el) return false; el.focus(); return true; })()`,
      );
      if (!field) throw new Error(`${scene.id}: lane field not found for typing`);
      await typeText(cdp, scene.type);
    }
    const probe = await evaluate(cdp, PROBE);
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
            ["focused", "disabled", "selected", "expanded", "live", "modal"].includes(
              property.name,
            ),
          )
          .map((property) => `${property.name}=${JSON.stringify(property.value?.value)}`);
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
      assert(checks, label, probe.empty && probe.empty.length > 0, `empty state text (${probe.empty})`);
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
    }
    if (scene.id === "lane-streaming") {
      assert(
        checks,
        label,
        probe.workingBars === 3,
        `three working bars (${probe.workingBars})`,
      );
      const animated = new Set(probe.ownAnimations.map((entry) => entry.split(":: ")[1]));
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
          (await import("node:fs")).readFileSync(join(dir, "probe.json"), "utf8"),
        );
        assert(
          checks,
          `${scene.id}@${width}`,
          other.tabMarkerAnimation === "none" || other.tabMarkerAnimation === null,
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
      assert(checks, label, probe.laneSendButtons.length === 0, "no Send button in the lane");
    }
  }
}

cdp.close();
chrome.kill("SIGKILL");
const failures = checks.filter((check) => !check.ok);
writeFileSync(
  join(OUT, "fixture-layer.json"),
  JSON.stringify({ checks, evidence, failures: failures.length }, null, 2),
);
console.log(`checks: ${checks.length - failures.length}/${checks.length} passed`);
for (const failure of failures) {
  console.log(`FAIL ${failure.scene}: ${failure.detail}`);
}
process.exit(failures.length ? 1 : 0);
