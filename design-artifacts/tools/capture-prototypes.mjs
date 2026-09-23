// Verify the Quiet Instrument migration prototype set and capture its evidence
// (plan 118, task 6). One Chrome process, every page x theme x width, in a loop:
// each run is asserted (no console error, no remote request, no horizontal
// overflow or clipped box, the theme really applied, the page's states are in the
// DOM, the active scene is the only one visible) and the runs that are review
// evidence are saved as PNGs named <page>__<theme>__<width>__<state>.png.
//
// Usage:
//   CHROME=/usr/bin/google-chrome node design-artifacts/tools/capture-prototypes.mjs
//   node design-artifacts/tools/capture-prototypes.mjs --out=DIR --no-shots --quiet
//
// Exit status is non-zero if any assertion fails, so this is the gate task 7's
// approval rests on, not just a screenshot script.

import { spawn } from 'node:child_process';
import { existsSync, mkdirSync, readFileSync, rmSync, statSync, writeFileSync, readdirSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const HERE = dirname(fileURLToPath(import.meta.url));
const PROTO = resolve(HERE, '..', 'prototypes', 'quiet-instrument-migration');
const ROOT = resolve(HERE, '..', '..');
const arg = (name, fallback) => {
  const hit = process.argv.find((a) => a.startsWith(`--${name}=`));
  return hit ? hit.slice(name.length + 3) : fallback;
};
const OUT = resolve(arg('out', join(PROTO, 'screenshots')));
const SHOTS = !process.argv.includes('--no-shots');
const QUIET = process.argv.includes('--quiet');
const ONLY = arg('only', null); // --only=start:modus-vivendi:both

const THEMES = [
  'modus-operandi',
  'modus-vivendi',
  'gruvbox-material-dark',
  'gruvbox-material-light',
];
const WIDTHS = [
  [1500, 950],
  [1024, 800],
];

// Every page, the scenes its switcher offers, and the DOM evidence for the
// states its coverage-table row claims. `evidence` selectors must exist on the
// page; `visible` ones must also be painted in the default state.
const PAGES = [
  {
    id: 'start',
    scenes: ['default', 'workspace', 'agent', 'both', 'first-run'],
    implicitScene: 'default',
    // the launcher's states are real selection, not staged markup: `?scene=`
    // and the review bar's switcher drive the page's own code, so there is no
    // per-state block to find and the assertions below are the evidence.
    scriptScenes: true,
    evidence: ['.win', '[data-start-pane="workspace"]', '[data-start-pane="agent"]', '[data-start-open]', '[data-viewswitch]', '.start-actions', '.statusbar'],
    visible: ['.win', '[data-pick]', '[data-start-open]'],
    stateChecks: {
      default: [
        {
          what: 'nothing is picked and Open is inert',
          expression: `document.querySelector('[data-start-open]').disabled && document.querySelector('[data-start-sum]').textContent.trim() === 'Nothing picked yet'`,
        },
      ],
      workspace: [
        {
          what: 'one workspace is picked and Open names it',
          expression: `document.querySelector('[data-start-sum]').textContent.trim() === 'workspace clay' && document.querySelector('[data-start-open]').textContent.trim() === 'Open clay'`,
        },
      ],
      agent: [
        {
          what: 'one agent is picked and Open names it',
          expression: `document.querySelector('[data-start-open]').textContent.trim() === 'Open Coding Agent'`,
        },
      ],
      both: [
        {
          what: 'both are picked and Open names the pair',
          expression: `document.querySelector('[data-start-open]').textContent.trim() === 'Open clay + Coding Agent'`,
          actual: `document.querySelector('[data-start-open]').textContent.trim() + ' / ' + document.querySelector('[data-start-sum]').textContent.trim()`,
        },
      ],
      'first-run': [
        {
          what: 'both panes say nothing has been used yet',
          expression: `Array.from(document.querySelectorAll('[data-start-body]')).every((b) => b.dataset.empty === 'true' && b.querySelector('.rows').checkVisibility({ opacityProperty: true }) === false) && document.querySelectorAll('[data-start-body] .start-note').length === 2`,
        },
      ],
    },
    behaviour: [
      {
        what: 'picking a workspace row makes the tab openable',
        reset: `window.__v.esc()`,
        do: `window.__v.click('[data-pick="workspace"]')`,
        measure: `document.querySelector('[data-start-open]').disabled ? 'disabled' : document.querySelector('[data-start-open]').textContent.trim()`,
        expect: (before, after) => before === 'disabled' && after === 'Open clay',
      },
      {
        what: 'the workspace filter narrows the recents',
        when: `document.querySelectorAll('[data-start-body][data-empty="true"]').length === 0`,
        do: `window.__v.type('[data-filter-list="#start-ws-rows"]', 'prism')`,
        measure: `window.__v.rows('[data-start-pane="workspace"] [data-pick]')`,
        expect: (before, after) => after > 0 && after < before,
      },
    ],
    note: 'the launcher; its states are selections the reviewer can also make by hand',
  },
  {
    id: 'component-catalog',
    scenes: [],
    evidence: ['.cat-spec', '[data-recipe]', '[data-fx]', '.cat-keys-list'],
    note: 'every declared recipe key once; the catalog generator asserts the key set',
  },
  {
    id: 'shell',
    scenes: [],
    readyExtra: `document.querySelectorAll('[data-tree] [role="treeitem"], [data-tree] .tree-row').length > 0`,
    evidence: ['.win', '.titlebar', '.shell-tabbar .tab', '[data-focus-zone]', '.welcome.empty', '.statusbar', '[data-viewswitch]', '.shell-tab--add'],
    visible: ['.win', '.welcome.empty'],
    stateChecks: {
      default: [
        {
          what: 'tabs are workspaces, and the active tab holds two views',
          expression: `document.querySelectorAll('.shell-tabbar [role="tab"]').length === 3 && document.querySelector('.viewswitch [aria-selected="true"]')?.textContent.trim() === 'Workspace'`,
        },
      ],
    },
    note: 'focused pane is [data-focus-zone]; the dragged split is a catalog state',
  },
  {
    id: 'workspace',
    scenes: [],
    readyExtra: `document.querySelectorAll('[data-tree] [role="treeitem"], [data-tree] .tree-row').length > 0`,
    evidence: ['.win', '.side', '[data-tree]', '.editor', '.rail', '.field input[type="search"]'],
    visible: ['.win', '.editor'],
    behaviour: [
      {
        what: 'the tree filter narrows the file list',
        do: `window.__v.type('[data-filter-for="files"]', 'quiet')`,
        measure: `window.__v.rows('[data-tree] [role="treeitem"], [data-tree] .tree-row')`,
        expect: (before, after) => after > 0 && after < before,
      },
    ],
    note: 'the derived approved screen: filter is live, read-only/no-workspace are not built (finding 7)',
  },
  {
    id: 'agent-landing',
    scenes: ['landing', 'running', 'error', 'resumed', 'empty-session', 'disconnected'],
    implicitScene: 'landing',
    evidence: ['.win', '[data-transcript]', '.composer', '[data-tabs]', '[data-foot-mcp]', '[data-agent-pick]', '[data-session-files]', '[data-viewswitch]'],
    visible: ['.win', '.composer', '[data-agent-pick]', '[data-viewswitch]'],
    onLoad: [
      {
        what: 'the composer draws one focus ring, on the shell',
        expression: `(() => {
          const shell = document.querySelector('.composer .input-shell');
          const inner = shell.querySelector('.field-input');
          inner.focus();
          const shellEdge = getComputedStyle(shell);
          const innerEdge = getComputedStyle(inner);
          return shellEdge.boxShadow !== 'none' && shellEdge.borderTopColor !== getComputedStyle(document.body).color && innerEdge.outlineStyle === 'none';
        })()`,
      },
      {
        what: 'the composer spans its zone with the controls inside the field',
        expression: `(() => {
          const shell = document.querySelector('.composer .input-shell');
          const row = document.querySelector('.composer-row');
          const actions = shell.querySelector('.composer-actions');
          const inner = shell.querySelector('.field-input');
          return !!actions && shell.getBoundingClientRect().width >= row.getBoundingClientRect().width - 2
            && inner.getBoundingClientRect().left - shell.getBoundingClientRect().left < 16
            && actions.getBoundingClientRect().right <= shell.getBoundingClientRect().right + 1
            && inner.getBoundingClientRect().right <= actions.getBoundingClientRect().left + 1;
        })()`,
      },
    ],
    // the footer's transport row is the one place a scene changes something
    // outside its own block, so it is asserted per state rather than assumed
    stateChecks: {
      landing: [
        {
          what: 'the transport row reads connected',
          expression: `Array.from(document.querySelectorAll('[data-foot-mcp]')).filter((el) => el.checkVisibility({ opacityProperty: true })).length === 2 && !document.querySelector('[data-foot-off]').checkVisibility({ opacityProperty: true })`,
        },
      ],
      disconnected: [
        {
          what: 'the transport row reads unavailable',
          expression: `Array.from(document.querySelectorAll('[data-foot-mcp]')).filter((el) => el.checkVisibility({ opacityProperty: true })).length === 0 && document.querySelector('[data-foot-off]').checkVisibility({ opacityProperty: true })`,
        },
      ],
    },
    note: 'the agent view of a tab; cancelled is the running scene\'s Esc affordance, not a separate state',
  },
  {
    id: 'settings',
    scenes: [],
    evidence: ['.win', '.settings-col', '.collapse-head', "[data-fx='invalid-error']", '.field-input', '.settings-actions', '.settings-actions [disabled]'],
    visible: ['.win', '.settings-col'],
    note: 'invalid field and the not-yet-applied action bar are in the DOM',
  },
  {
    id: 'agent-settings',
    scenes: ['rest', 'empty', 'error'],
    implicitScene: 'rest',
    evidence: ['.win', '.agent-files-col', '.files-list', '.file-row', '.badge'],
    visible: ['.win', '.files-list'],
    honesty: 'agentFiles',
    note: 'the daemon\'s delivered layout; skill sizes are read from disk and checked below',
  },
  {
    id: 'command-centre',
    scenes: [],
    evidence: ['.app', '.cc-palette', '.cc-input', '.pal-list', '[data-cc-empty]'],
    visible: ['.app', '.cc-palette'],
    behaviour: [
      {
        what: 'a query with no match reveals the empty state',
        do: `window.__v.type('.cc-input', 'zzzznomatch')`,
        measure: `window.__v.shown('[data-cc-empty]')`,
        expect: (before, after) => !before && after,
      },
    ],
    note: 'no-result state is [data-cc-empty], revealed by typing — asserted, not staged',
  },
  {
    id: 'package-workspace',
    scenes: [],
    evidence: ['.app', '.pk-shell', '.pk-row', "[data-fx='selected-accent']", "[data-fx='invalid-error']", '.badge--warning'],
    visible: ['.app', '.pk-shell'],
    note: 'SDUI rows; the failed-verification card is the package error state',
  },
  {
    id: 'overlays',
    scenes: ['rest', 'reduced-transparency'],
    implicitScene: 'rest',
    evidence: ['.ov-stage', '.ov-modal', '.popover', '.tip', '.toast', '.ov-scrim'],
    visible: ['.ov-stage', '.ov-modal'],
    note: 'veils, popovers, tooltips, toasts and the reduced-transparency fallback; focus trap and dismissal are component behaviour (React Aria), not staged here',
  },
  {
    id: 'themes',
    scenes: [],
    evidence: ['.theme-panel', '.sw', '.sw-old', '.spec-row', '.ladder', '.verdict-ok'],
    visible: ['.theme-panel'],
    note: 'current vs proposed per theme, measured composited',
  },
];

function findChrome() {
  if (process.env.CHROME) return process.env.CHROME;
  const candidates = ['/usr/bin/google-chrome', '/usr/bin/chromium', '/usr/bin/chromium-browser'];
  const cache = join(process.env.HOME ?? '/root', '.cache', 'ms-playwright');
  if (existsSync(cache)) {
    for (const entry of readdirSync(cache)) {
      for (const bin of ['chrome-linux64/chrome', 'chrome-linux/chrome']) {
        candidates.push(join(cache, entry, bin));
      }
    }
  }
  const found = candidates.find((path) => existsSync(path));
  if (!found) throw new Error('no Chrome found; set CHROME=/path/to/chrome');
  return found;
}

const CHROME = findChrome();
const PORT = 9500 + Math.floor(Math.random() * 400);
const PROFILE = join('/tmp', `clay-proto-verify-${PORT}`);
const chrome = spawn(
  CHROME,
  [
    '--headless=new',
    '--no-sandbox',
    '--disable-gpu',
    '--hide-scrollbars',
    '--allow-file-access-from-files',
    '--force-device-scale-factor=1',
    '--disable-lcd-text',
    '--no-first-run',
    '--no-default-browser-check',
    `--user-data-dir=${PROFILE}`,
    `--remote-debugging-port=${PORT}`,
    'about:blank',
  ],
  { stdio: ['ignore', 'ignore', 'pipe'] },
);
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

async function browserSocket() {
  for (let i = 0; i < 150; i++) {
    try {
      const list = await (await fetch(`http://127.0.0.1:${PORT}/json/list`)).json();
      const page = list.find((t) => t.type === 'page');
      if (page) return page.webSocketDebuggerUrl;
    } catch {
      /* not up yet */
    }
    await sleep(100);
  }
  throw new Error('chrome did not start');
}

const ws = new WebSocket(await browserSocket());
await new Promise((r) => ws.addEventListener('open', r, { once: true }));

let seq = 0;
const pending = new Map();
let events = [];
ws.addEventListener('message', (raw) => {
  const msg = JSON.parse(raw.data);
  if (msg.id && pending.has(msg.id)) {
    const { resolve: done, reject } = pending.get(msg.id);
    pending.delete(msg.id);
    msg.error ? reject(new Error(JSON.stringify(msg.error))) : done(msg.result);
  } else if (msg.method) {
    events.push(msg);
  }
});
const send = (method, params = {}) =>
  new Promise((done, reject) => {
    const id = ++seq;
    pending.set(id, { resolve: done, reject });
    ws.send(JSON.stringify({ id, method, params }));
  });

await send('Page.enable');
await send('Runtime.enable');
await send('Log.enable');
await send('Network.enable');

const evaluate = async (expression) => {
  const { result, exceptionDetails } = await send('Runtime.evaluate', {
    expression,
    returnByValue: true,
    awaitPromise: true,
  });
  if (exceptionDetails) throw new Error(`${exceptionDetails.text}: ${exceptionDetails.exception?.description ?? ''} <- ${expression.slice(0, 90)}`);
  return result.value;
};

const PROBE = `(() => {
  const de = document.documentElement;
  const over = [];
  // checkVisibility honours opacity: a closed popover is positioned off-canvas
  // and invisible, and counting it as clipped hid the real thing twice
  const shown = (el) => el instanceof Element && el.checkVisibility({ opacityProperty: true, visibilityProperty: true });
  const rows = (sel) => Array.from(document.querySelectorAll(sel)).filter(shown).length;
  for (const el of document.querySelectorAll('*')) {
    if (!shown(el)) continue;
    if (el.closest('.tipwrap, .sr-only, [aria-hidden="true"]')) continue;
    const r = el.getBoundingClientRect();
    if (r.width === 0) continue;
    if (r.right > de.clientWidth + 1 || r.left < -1) over.push((el.className || el.tagName) + ' @' + Math.round(r.right));
  }
  const css = getComputedStyle(de);
  return {
    theme: de.dataset.theme || '',
    scene: de.dataset.scene || '',
    docOverflow: de.scrollWidth - de.clientWidth,
    bodyOverflow: document.body.scrollWidth - de.clientWidth,
    clipped: over.slice(0, 6),
    resource: performance.getEntriesByType('resource').map((r) => r.name),
    // resource timing is empty for file:// documents in this Chrome, which made
    // the "no remote request" assertion vacuous: scan the markup for anything
    // that is not a sibling file instead
    external: Array.from(document.querySelectorAll('[src], [href], [srcset], [data-src]'))
      .map((el) => el.getAttribute('src') || el.getAttribute('href') || el.getAttribute('srcset') || el.getAttribute('data-src'))
      // relative sibling paths are the point; only an absolute scheme is remote
      .filter((url) => url && /^[a-z][a-z0-9+.-]*:/i.test(url) && !/^(file:|data:|mailto:|about:)/i.test(url)),
    canvas: css.getPropertyValue('--c-surface').trim(),
    accent: css.getPropertyValue('--c-accent').trim(),
    rail: css.getPropertyValue('--hairline').trim(),
    treeRows: rows('[data-tree] [role="treeitem"], [data-tree] .tree-row'),
  };
})()`;

// Behaviour probes: the claim is that a live affordance does something, so the
// run types into it and measures the result rather than trusting a screenshot.
const HELPERS = `(() => {
  window.__v = {
    shown: (sel) => { const el = document.querySelector(sel); return !!el && el.checkVisibility({ opacityProperty: true, visibilityProperty: true }); },
    rows: (sel) => Array.from(document.querySelectorAll(sel)).filter((el) => el.checkVisibility({ opacityProperty: true, visibilityProperty: true })).length,
    click: (sel) => { const el = document.querySelector(sel); if (!el) return false; el.click(); return true; },
    esc: () => { document.body.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true })); return true; },
    type: (sel, text) => {
      const el = document.querySelector(sel);
      if (!el) return false;
      el.focus();
      el.value = text;
      el.dispatchEvent(new Event('input', { bubbles: true }));
      el.dispatchEvent(new KeyboardEvent('keydown', { key: 'a', bubbles: true }));
      return true;
    },
  };
  return true;
})()`;

const SCENE_PROBE = (scenes) =>
  `(() => {
    const shown = (el) => el instanceof Element && el.checkVisibility({ opacityProperty: true, visibilityProperty: true });
    return ${JSON.stringify(scenes)}.map((id) => {
      const block = document.querySelector('[data-scene="' + id + '"]');
      const others = Array.from(document.querySelectorAll('[data-scene]')).filter((el) => el.dataset.scene !== id);
      return { id, present: !!block, visible: shown(block), otherVisible: others.filter(shown).length };
    });
  })()`;

const EVIDENCE_PROBE = (evidence, visible) =>
  `(() => ({
    missing: ${JSON.stringify(evidence)}.filter((s) => !document.querySelector(s)),
    hidden: ${JSON.stringify(visible)}.filter((s) => {
      const el = document.querySelector(s);
      return !(el && el.checkVisibility({ opacityProperty: true, visibilityProperty: true }));
    }),
  }))()`;

// Honesty probe: the numbers an agent-files screen prints must be the numbers on
// disk, or the screen is presenting invented data as real.
const FILE_SIZE_PROBE = `(() => Array.from(document.querySelectorAll('.file-row')).map((row) => ({
  name: row.querySelector('.file-name')?.textContent.trim() ?? '',
  size: row.querySelector('.file-size')?.textContent.trim() ?? '',
})).filter((f) => f.size))()`;


const rows = [];
const failures = [];
let shots = 0;

const PAGE_IDS = () => PAGES.map((p) => p.id);

// A page x theme x width run. `state` is the scene (or 'default').
async function run(page, theme, [width, height], state, shot) {
  const query = [`theme=${theme}`, `width=${width}`];
  if (state !== 'default') query.push(`scene=${state}`);
  void state;
  const url = `file://${join(PROTO, `${page.id}.html`)}?${query.join('&')}`;
  await send('Emulation.setDeviceMetricsOverride', {
    width,
    height,
    deviceScaleFactor: 1,
    mobile: false,
  });
  events = [];
  await send('Page.navigate', { url });
  // Waiting for `Page.loadEventFired` is not enough and not safe: the event can
  // still belong to the previous document, and then every measurement below is
  // taken on the wrong page (it silently reported another page's clipped boxes
  // and an empty tree). Confirm the navigation committed first, then wait for
  // the document to be parsed, themed and styled.
  let committed = false;
  for (let i = 0; i < 120 && !committed; i++) {
    committed = await evaluate(`document.URL === ${JSON.stringify(url)}`);
    if (!committed) await sleep(40);
  }
  // Pages whose content is built by their own script say what "ready" means on
  // top of the generic gate: otherwise the tree (or a list) can still be empty
  // when a behaviour probe measures it, which made a passing run flaky.
  const baseReady = `document.readyState === 'complete' && document.documentElement.dataset.theme !== '' && getComputedStyle(document.documentElement).getPropertyValue('--hairline').trim() !== ''`;
  const pageReady = page.readyExtra ? `${baseReady} && (${page.readyExtra})` : baseReady;
  let ready = false;
  for (let i = 0; i < 80 && !ready; i++) {
    ready = await evaluate(pageReady);
    if (!ready) await sleep(60);
  }
  await sleep(120);

  const errors = events
    .filter((e) => e.method === 'Runtime.exceptionThrown')
    .map((e) => e.params.exceptionDetails.exception?.description ?? e.params.exceptionDetails.text)
    .concat(
      events
        .filter((e) => e.method === 'Runtime.consoleAPICalled' && ['error', 'assert'].includes(e.params.type))
        .map((e) => e.params.args.map((a) => a.value ?? a.description ?? '').join(' ')),
      events
        .filter((e) => e.method === 'Log.entryAdded' && e.params.entry.level === 'error')
        .map((e) => e.params.entry.text),
    )
    .filter((text) => !/favicon/i.test(text));

  const probe = await evaluate(PROBE);
  const isDefaultState = state === (page.implicitScene ?? 'default');
  const evidence = await evaluate(
    EVIDENCE_PROBE(page.evidence, isDefaultState ? page.visible ?? [] : []),
  );
  const scenes = await evaluate(SCENE_PROBE(page.scenes));

  // assertions about *the state as loaded* come before anything is driven, or a
  // behaviour probe's own mutations read as the state being wrong
  const stateProblems = [];
  for (const check of state === (page.implicitScene ?? 'default') ? page.onLoad ?? [] : []) {
    const ok = await evaluate(`Boolean(${check.expression})`);
    if (!ok) stateProblems.push(`layout: ${check.what}`);
  }
  for (const check of page.stateChecks?.[state] ?? []) {
    const ok = await evaluate(`Boolean(${check.expression})`);
    if (!ok) {
      const seen = check.actual ? await evaluate(check.actual) : null;
      stateProblems.push(`state (${state}): ${check.what}${seen === null ? '' : ` — saw ${JSON.stringify(seen)}`}`);
    }
  }

  // the evidence is the state as loaded: capture it before anything is driven,
  // or the PNG shows the probe's own mutations (a typed filter, a picked row)
  let file = null;
  if (shot && SHOTS) {
    const name = `${page.id}__${theme}__${width}x${height}__${state}.png`;
    const { data } = await send('Page.captureScreenshot', { format: 'png', fromSurface: true });
    mkdirSync(OUT, { recursive: true });
    writeFileSync(join(OUT, name), Buffer.from(data, 'base64'));
    file = name;
    shots += 1;
  }

  // a behaviour claim is verified by driving the affordance, not by looking
  const behaviour = [];
  if (page.behaviour?.length) {
    await evaluate(HELPERS);
    for (const item of page.behaviour) {
      // a behaviour probe starts from the state it needs, or is skipped there
      if (item.when && !(await evaluate(`Boolean(${item.when})`))) continue;
      if (item.reset) await evaluate(item.reset);
      const before = await evaluate(item.measure);
      await evaluate(item.do);
      await sleep(120);
      const after = await evaluate(item.measure);
      behaviour.push({ what: item.what, before, after, ok: item.expect(before, after) });
    }
  }

  // honesty: sizes printed for repository files must be the sizes on disk
  const honesty = [];
  if (page.honesty === 'agentFiles') {
    for (const row of await evaluate(FILE_SIZE_PROBE)) {
      if (!row.name.startsWith('skills/')) continue;
      const path = join(ROOT, '.agents', row.name.replace(/^skills\//, 'skills/'));
      const real = existsSync(path) ? humanSize(statSync(path).size) : null;
      honesty.push({ name: row.name, printed: row.size, disk: real, ok: real === row.size });
    }
  }
  const remote = probe.resource
    .filter((name) => !name.startsWith('file://'))
    .concat(probe.external.map((url) => `external reference: ${url}`));

  const problems = stateProblems.slice();
  if (!committed) problems.push('navigation never committed');
  if (!ready) problems.push('page never reached a parsed, themed, styled state');
  if (errors.length) problems.push(`console: ${errors.slice(0, 2).join(' | ')}`);
  if (remote.length) problems.push(`remote resource: ${remote.slice(0, 2).join(', ')}`);
  if (probe.theme !== theme) problems.push(`theme=${probe.theme} wants ${theme}`);
  if (probe.docOverflow > 1 || probe.bodyOverflow > 1)
    problems.push(`overflow ${probe.docOverflow}/${probe.bodyOverflow}px`);
  if (probe.clipped.length) problems.push(`clipped: ${probe.clipped.join(', ')}`);
  if (evidence.missing.length) problems.push(`missing ${evidence.missing.join(', ')}`);
  if (evidence.hidden.length) problems.push(`not painted: ${evidence.hidden.join(', ')}`);
  const implicit = page.implicitScene;
  const active = scenes.find((s) => s.id === state);
  if (page.scriptScenes) {
    // the page's states are its own code (the launcher picks rows): there is no
    // markup to find, so the per-state assertions below are the evidence
  } else if (active) {
    // the state has a block of its own (the default may be the page's own markup,
    // in which case there is nothing to assert beyond "no other scene is showing")
    if (!active.present) problems.push(`scene ${state} absent`);
    else if (!active.visible) problems.push(`scene ${state} not visible`);
    if (active.otherVisible) problems.push(`${active.otherVisible} other scene(s) visible`);
  } else if (scenes.some((s) => s.present && s.visible)) {
    problems.push('a scene block is visible in the default state');
  }
  if (!page.scriptScenes) {
    for (const scene of scenes) {
      if (!scene.present && scene.id !== implicit) problems.push(`scene ${scene.id} absent`);
    }
  }
  for (const item of behaviour) if (!item.ok) problems.push(`behaviour: ${item.what} (${item.before} -> ${item.after})`);
  for (const item of honesty) if (!item.ok) problems.push(`honesty: ${item.name} prints ${item.printed}, disk has ${item.disk}`);

  rows.push({
    page: page.id,
    state,
    theme,
    width,
    height,
    shot: file,
    errors: errors.length,
    remote: remote.length,
    overflow: Math.max(probe.docOverflow, probe.bodyOverflow),
    clipped: probe.clipped.length,
    canvas: probe.canvas,
    accent: probe.accent,
    treeRows: probe.treeRows,
    behaviour,
    honesty,
    problems,
  });
  if (problems.length) failures.push(`${page.id} ${theme} ${width}x${height} ${state}: ${problems.join('; ')}`);
}

// Every page x theme x width is asserted with a shot; the narrow matrix and the
// non-default scenes store a sample instead (documented in the README).
const NARROW_SHOT_THEME = 'gruvbox-material-light';
const SCENE_SHOT_THEMES = ['modus-vivendi', 'gruvbox-material-light'];

const wanted = (page, theme, state) => !ONLY || ONLY === `${page.id}:${theme}:${state}`;
for (const page of PAGES) {
  for (const theme of THEMES) {
    for (const [width, height] of WIDTHS) {
      const state = page.implicitScene ?? 'default';
      if (!wanted(page, theme, state)) continue;
      const shot = width === 1500 || theme === NARROW_SHOT_THEME;
      await run(page, theme, [width, height], state, shot);
    }
  }
  for (const state of page.scenes.filter((s) => s !== page.implicitScene)) {
    for (const theme of SCENE_SHOT_THEMES) {
      if (!wanted(page, theme, state)) continue;
      await run(page, theme, WIDTHS[0], state, true);
    }
  }
}

// Theme separation: the same page must paint a different canvas per theme.
for (const page of ONLY ? [] : PAGES) {
  const baseState = page.implicitScene ?? 'default';
  const canvases = new Set(
    rows.filter((r) => r.page === page.id && r.width === 1500 && r.state === baseState).map((r) => r.canvas),
  );
  if (canvases.size !== THEMES.length)
    failures.push(`${page.id}: ${canvases.size} distinct canvases, wants ${THEMES.length}`);
}

const report = {
  chrome: CHROME,
  pages: PAGE_IDS(),
  themes: THEMES,
  widths: WIDTHS.map(([w, h]) => `${w}x${h}`),
  runs: rows.length,
  shots,
  failures,
  results: rows,
};
if (SHOTS) {
  mkdirSync(OUT, { recursive: true });
  writeFileSync(join(OUT, 'report.json'), JSON.stringify(report, null, 1) + '\n');
}

if (!QUIET) {
  const byWidth = {};
  for (const r of rows) byWidth[`${r.width}x${r.height}`] = (byWidth[`${r.width}x${r.height}`] ?? 0) + 1;
  console.log(`chrome          ${CHROME}`);
  console.log(`runs            ${rows.length} (${Object.entries(byWidth).map(([k, v]) => `${k}: ${v}`).join(', ')})`);
  console.log(`screenshots     ${shots} -> ${OUT}`);
  console.log(`console errors  ${rows.reduce((n, r) => n + r.errors, 0)}`);
  console.log(`remote requests ${rows.reduce((n, r) => n + r.remote, 0)}`);
  console.log(`worst overflow  ${Math.max(...rows.map((r) => r.overflow))}px`);
  console.log(`clipped boxes   ${rows.reduce((n, r) => n + r.clipped, 0)}`);
  console.log(`assertions      ${rows.length - new Set(failures.map((f) => f.split(':')[0])).size}/${rows.length} runs clean`);
}
if (failures.length) {
  console.error(`\n${failures.length} FAILING RUNS`);
  for (const line of failures.slice(0, 30)) console.error(`  ${line}`);
}

ws.close();
chrome.kill();
// best effort: Chrome may still be flushing its profile on the way out
try {
  rmSync(PROFILE, { recursive: true, force: true, maxRetries: 3, retryDelay: 120 });
} catch {
  /* /tmp cleans itself up */
}
process.exit(failures.length ? 1 : 0);

// small helper mirroring the generator's size formatting
function humanSize(size) {
  if (size >= 1024 * 1024) return `${(size / (1024 * 1024)).toFixed(1)} MB`;
  if (size >= 1024) return `${(size / 1024).toFixed(1)} kB`;
  return `${size} B`;
}
void readFileSync;
