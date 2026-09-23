#!/usr/bin/env node
/**
 * Component-level conformance against the approved specimen (plan 118).
 *
 * Two passes, one report:
 *
 *   audit    (default, offline) — the approved specimen page and the shipped
 *            manifest must agree key for key, and the manifest must obey the
 *            approved language: radius ladder, single border weight per
 *            surface, transient-only shadows, blur only on the scrim/toast,
 *            motion only in the language tiers.
 *   browser  (`--browser <url>`) — drives headless Chrome over the DEV
 *            component gallery and the approved specimen, in the `@clay/core`
 *            baseline plus the four shipped themes, and asserts what the
 *            components *paint* is the recipe they declare and the specimen
 *            they were approved from. Writes screenshots + report.json.
 *
 * Usage:
 *   node design-artifacts/tools/verify-component-conformance.mjs
 *   node design-artifacts/tools/verify-component-conformance.mjs --browser http://localhost:5199
 *
 * The browser pass needs the app's dev server (`npm run dev -w frontend`);
 * the specimen side is opened straight from disk (`file://`).
 */

import { spawn } from "node:child_process";
import {
  existsSync,
  mkdirSync,
  readdirSync,
  readFileSync,
  writeFileSync,
} from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "../..");
const MANIFEST = join(ROOT, "packages/design-instrument/package.json");
const SPECIMEN_DIR = join(
  ROOT,
  "design-artifacts/approved/quiet-instrument-migration",
);
const SPECIMEN = join(SPECIMEN_DIR, "component-catalog.html");
/** The 142-key pre-migration baseline the approved specimen was frozen against. */
const REFERENCE_KEYS = join(
  ROOT,
  "tests/fixtures/design-system-reference-keys.txt",
);
const THEME_VALUES = join(SPECIMEN_DIR, "theme-values.json");
const EVIDENCE = join(
  ROOT,
  "design-artifacts/screenshots/quiet-instrument-component-conformance",
);

// ---------------------------------------------------------------- language facts

const RADIUS_LADDER = [0, 5, 8, 12, 16, 9999];
const STATE_VOCABULARY = [
  "rest",
  "hover",
  "active",
  "focus",
  "selected",
  "disabled",
  "invalid",
  // The agent picker's trigger is the one state that is not a 7-column canonical
  // cell: `expanded` describes an open dropdown, which the specimen shows as a
  // distinct cell rather than as `selected`.
  "expanded",
];
const MOTION_TIERS = [0, 150, 240, 620];
/** Families that carry elevation because they enter over the canvas. */
const TRANSIENT_FAMILIES = [
  "commandCentre",
  "dropdown",
  "editor", // the tooltip slot only
  "menu",
  "modal",
  "overlay",
  "panel", // the `transient` variant only
  "popover",
  "textInput", // the focus halo only
  "toast",
  "tooltip",
];
/** Blur belongs to the scrim and the toast; nothing else may blur (DESIGN.md §6). */
const BLUR_KEYS = ["modal.default.scrim.rest", "toast.default.root.rest"];
/** Kind-level hosts that are deliberately not recipe targets (components.md). */
const HOST_ONLY_KINDS = ["tabList", "table", "iconSlot", "editorView"];
/**
 * A 0 radius is the flushed-region allowance: full-bleed regions and layout
 * layers, which is exactly the set `src/shell/theme.rs` and the host CSS
 * invariants allow (a control may never be square).
 */
const FLUSH_KEYS = [
  "editor.default.container.rest",
  "empty.default.root.rest",
  "keyHint.default.keys.rest",
  "keyHint.default.root.rest",
  "keyHint.default.row.rest",
  "paneSplitTree.default.pane.rest",
  "paneSplitTree.default.pane.active",
  "tabBar.default.root.rest",
  "paneSplitTree.default.root.rest",
  "modal.default.scrim.rest",
  "modal.default.root.rest",
  "dropdown.default.root.rest",
  "fileBrowser.default.root.rest",
  "flex.default.root.rest",
  "grid.default.root.rest",
  "label.default.root.rest",
  "scroll.default.root.rest",
  "statusBar.default.root.rest",
];

/** The eight surfaces that enter over the canvas: 240ms spring-snappy. */
const ENTERING_KEYS = [
  "commandCentre.default.root.rest",
  "dropdown.default.popover.rest",
  "menu.default.root.rest",
  "modal.default.dialog.rest",
  "overlay.default.root.rest",
  "panel.transient.root.rest",
  "popover.default.root.rest",
  "toast.default.root.rest",
];

/** The ten families added after approval, with their key counts (plan 118 task 8). */
const POST_APPROVAL_FAMILIES = {
  agentPicker: 5,
  empty: 1,
  keyHint: 3,
  recentRow: 4,
  seg: 6,
  sessionRow: 3,
  statRow: 3,
  statusDot: 5,
  swatch: 4,
  toast: 1,
};

/**
 * Deviations seen while verifying, each with the reason it is accepted. Anything
 * not in this list fails the run: the list is the task's recorded deviation list,
 * so a new one has to be consciously added (see the plan outcome).
 */
const RECORDED_DEVIATIONS = [
  {
    id: "modal.entrance-animation",
    keys: ["modal.default.dialog.rest"],
    note: "the modal dialog carries the approved 240ms spring-snappy entrance animation, so `animationName` is not `none` on exactly that node (DESIGN.md §11).",
  },
  {
    id: "dropdown.root-popover.root-unconsumed",
    keys: [],
    note: "`dropdown.default.root.*` and `popover.default.root.*` have no host consumer: the shared popover class paints from the dropdown family's popover recipe. Recorded in the adoption backlog for the task that owns a standalone popover surface.",
  },
  {
    id: "editor.path-is-a-field",
    keys: [],
    note: "`editor.default.path.rest` describes a transparent text-only label; the host's open-path control is an input and paints from `textInput.default.input.*` (the task-17 finding).",
  },
];

/**
 * Deviations between the host and the approved specimen's *cell mapping* rather
 * than its values. Recorded with the reading that makes the run honest: these
 * cells compare two different things on the two sides.
 */
const RECORDED_CROSS_DEVIATIONS = [
  {
    id: "textInput.field-cell-is-the-composer-well",
    keys: ["textInput.default.field.rest", "textInput.default.input.rest"],
    note: "The specimen's `field` cell paints the composer well (12px radius + hairline) and its `input` cell is the text inside it; the contract declares `field` as the transparent layout wrapper and puts the single-line boundary (8px radius + hairline) on `input`, which is what DESIGN.md §11 describes (12 composer / 8 single line) and what the host paints. A multiline variant is the missing contract piece.",
  },
];

// ---------------------------------------------------------------- small helpers

const px = (value) => Number.parseFloat(value ?? "0") || 0;
const isPxEqual = (a, b) => Math.abs(px(a) - px(b)) < 0.51;

function countShadowLayers(computed) {
  if (!computed || computed === "none") return 0;
  let depth = 0;
  let layers = 1;
  for (const ch of computed) {
    if (ch === "(") depth += 1;
    else if (ch === ")") depth -= 1;
    else if (ch === "," && depth === 0) layers += 1;
  }
  return layers;
}

function maxDurationMs(computed) {
  return Math.max(
    0,
    ...String(computed ?? "0s")
      .split(",")
      .map((part) => {
        const value = part.trim();
        return value.endsWith("ms")
          ? Number.parseFloat(value)
          : Number.parseFloat(value) * 1000;
      })
      .filter((n) => Number.isFinite(n)),
  );
}

/**
 * Browsers serialise the same colour as `rgb()`, `rgba()` or `color(srgb …)`
 * depending on how it was authored; compare channels, not spellings.
 */
function parseColor(value) {
  const text = String(value ?? "").trim();
  let match = /rgba?\(([^)]+)\)/.exec(text);
  if (match) {
    const parts = match[1]
      .split(/[\s,\/]+/)
      .filter(Boolean)
      .map(Number);
    return { r: parts[0], g: parts[1], b: parts[2], a: parts[3] ?? 1 };
  }
  match = /color\(srgb ([^)]+)\)/.exec(text);
  if (match) {
    const parts = match[1]
      .split(/[\s\/]+/)
      .filter(Boolean)
      .map(Number);
    return {
      r: parts[0] * 255,
      g: parts[1] * 255,
      b: parts[2] * 255,
      a: parts[3] ?? 1,
    };
  }
  return null;
}

function colorsEqual(a, b) {
  const left = parseColor(a);
  const right = parseColor(b);
  if (!left || !right) return a === b;
  return (
    Math.abs(left.r - right.r) < 1.5 &&
    Math.abs(left.g - right.g) < 1.5 &&
    Math.abs(left.b - right.b) < 1.5 &&
    Math.abs(left.a - right.a) < 0.005
  );
}

function normalizeTransform(value) {
  return String(value ?? "none")
    .replace(/\s+/g, " ")
    .trim();
}

function hexToRgb(hex) {
  const raw = String(hex).trim().replace("#", "");
  const full =
    raw.length <= 4
      ? raw
          .split("")
          .map((c) => c + c)
          .join("")
      : raw;
  const int = Number.parseInt(full.slice(0, 6), 16);
  return { r: (int >> 16) & 255, g: (int >> 8) & 255, b: int & 255 };
}

function contrastRatio(a, b) {
  const channel = (c) => {
    const s = c / 255;
    return s <= 0.04045 ? s / 12.92 : ((s + 0.055) / 1.055) ** 2.4;
  };
  const lum = (c) =>
    0.2126 * channel(c.r) + 0.7152 * channel(c.g) + 0.0722 * channel(c.b);
  const [hi, lo] = [lum(a), lum(b)].sort((x, y) => y - x);
  return (hi + 0.05) / (lo + 0.05);
}

// ---------------------------------------------------------------- reading inputs

function loadManifest() {
  const data = JSON.parse(readFileSync(MANIFEST, "utf8"));
  return data.clay.contributions.uiDesignSystem;
}

function loadSpecimen() {
  const html = readFileSync(SPECIMEN, "utf8");
  const keys = [...html.matchAll(/data-recipe="([^"]+)"/g)].map((m) => m[1]);
  const families = [
    ...html.matchAll(/data-family="([^"]+)" data-interactive="([^"]+)"/g),
  ].map((m) => ({ family: m[1], interactive: m[2] === "true" }));
  const absent = [...html.matchAll(/data-absent="([^"]+)"/g)].map((m) => m[1]);
  return { html, keys: new Set(keys), families, absent: new Set(absent) };
}

function parseKeys(recipes) {
  const byFamily = new Map();
  for (const key of Object.keys(recipes)) {
    const [component, variant, slot, state] = key.split(".");
    const family = `${component}.${variant}`;
    if (!byFamily.has(family))
      byFamily.set(family, { component, slots: new Map() });
    const entry = byFamily.get(family);
    if (!entry.slots.has(slot)) entry.slots.set(slot, []);
    entry.slots.get(slot).push(state);
  }
  return byFamily;
}

// ---------------------------------------------------------------- audit pass

function audit() {
  const ds = loadManifest();
  const recipes = ds.recipes;
  const specimen = loadSpecimen();
  const byFamily = parseKeys(recipes);
  const checks = [];
  const findings = [];
  const row = (id, ok, detail) => checks.push({ id, ok, detail });

  // 1. Coverage: the specimen is exactly the approved reference set minus chat.
  const reference = new Set(
    readFileSync(REFERENCE_KEYS, "utf8")
      .split("\n")
      .map((l) => l.trim())
      .filter((l) => l && !l.startsWith("#")),
  );
  const referenceWithoutChat = new Set(
    [...reference].filter((k) => !k.startsWith("chat.")),
  );
  const missingFromSpecimen = [...referenceWithoutChat].filter(
    (k) => !specimen.keys.has(k),
  );
  const extraInSpecimen = [...specimen.keys].filter(
    (k) => !referenceWithoutChat.has(k),
  );
  row(
    "coverage/specimen-is-the-approved-reference",
    missingFromSpecimen.length === 0 && extraInSpecimen.length === 0,
    `${specimen.keys.size} specimen keys = ${referenceWithoutChat.size} reference keys minus 12 chat` +
      (missingFromSpecimen.length
        ? `; missing ${missingFromSpecimen.join(", ")}`
        : "") +
      (extraInSpecimen.length ? `; stale ${extraInSpecimen.join(", ")}` : ""),
  );

  // 2. The manifest may only extend it by the recorded post-approval families.
  const additions = [...Object.keys(recipes)].filter(
    (k) => !specimen.keys.has(k),
  );
  const additionFamilies = {};
  for (const key of additions) {
    const family = key.split(".")[0];
    additionFamilies[family] = (additionFamilies[family] ?? 0) + 1;
  }
  const additionsMatch =
    JSON.stringify(
      Object.fromEntries(Object.entries(additionFamilies).sort()),
    ) ===
    JSON.stringify(
      Object.fromEntries(Object.entries(POST_APPROVAL_FAMILIES).sort()),
    );
  const staleManifest = [...Object.keys(recipes)].filter(
    (k) => !specimen.keys.has(k) && k.startsWith("chat."),
  );
  row(
    "coverage/additions-are-the-approved-target-ia-families",
    additionsMatch && additions.length === 35 && staleManifest.length === 0,
    `${additions.length} additions in ${Object.keys(additionFamilies).length} families: ${Object.entries(
      additionFamilies,
    )
      .map(([f, n]) => `${f}(${n})`)
      .join(" ")}`,
  );

  const kinds = new Set([...byFamily.values()].map((f) => f.component));
  const specimenKinds = new Set([...specimen.keys].map((k) => k.split(".")[0]));
  const kindGaps = [...kinds].filter((kind) => !specimenKinds.has(kind));
  row(
    "coverage/every-kind-covered",
    kindGaps.every((kind) => kind in POST_APPROVAL_FAMILIES),
    `${specimenKinds.size} kinds approved + ${kindGaps.length} added later (${kindGaps.join(", ") || "none"}); ${byFamily.size} families, ${Object.keys(recipes).length} keys`,
  );

  // 2. State completeness: every slot declares `rest`, and states stay in the vocabulary.
  const restless = [];
  const offVocabulary = [];
  const interactiveWithoutFocus = [];
  for (const [family, entry] of byFamily) {
    for (const [slot, states] of entry.slots) {
      if (!states.includes("rest")) restless.push(`${family}.${slot}`);
      for (const state of states)
        if (!STATE_VOCABULARY.includes(state))
          offVocabulary.push(`${family}.${slot}.${state}`);
      if (states.includes("hover") && !states.includes("focus")) {
        interactiveWithoutFocus.push(`${family}.${slot}`);
      }
    }
  }
  row(
    "states/every-slot-declares-rest",
    restless.length === 0,
    restless.join(", ") || "all slots",
  );
  row(
    "states/vocabulary-closed",
    offVocabulary.length === 0,
    offVocabulary.join(", ") || STATE_VOCABULARY.join("/"),
  );
  findings.push({
    id: "states/hover-without-focus",
    detail: `${interactiveWithoutFocus.length} slots hover without a focus recipe (row-like surfaces; reported, not a language breach): ${interactiveWithoutFocus.join(", ")}`,
  });

  // 3. Radius ladder membership, with 0 only on the flushed regions.
  const offLadder = [];
  const strayFlush = [];
  for (const [key, recipe] of Object.entries(recipes)) {
    if (recipe.borderRadius === undefined) continue;
    if (!RADIUS_LADDER.includes(recipe.borderRadius))
      offLadder.push(`${key}=${recipe.borderRadius}`);
    if (recipe.borderRadius === 0 && !FLUSH_KEYS.includes(key))
      strayFlush.push(key);
  }
  row(
    "geometry/radius-ladder",
    offLadder.length === 0,
    offLadder.join(", ") || RADIUS_LADDER.join("/"),
  );
  row(
    "geometry/zero-radius-is-flushed-only",
    strayFlush.length === 0,
    strayFlush.join(", ") || FLUSH_KEYS.length + " flush regions",
  );

  // 4. One hairline weight, and one weight per surface across states.
  const wide = [];
  const mixed = [];
  for (const [family, entry] of byFamily) {
    for (const [slot, states] of entry.slots) {
      const widths = new Set();
      for (const state of states) {
        const recipe = recipes[`${family}.${slot}.${state}`];
        if (recipe.borderWidth === undefined) continue;
        if (recipe.borderWidth > 1)
          wide.push(`${family}.${slot}.${state}=${recipe.borderWidth}`);
        widths.add(recipe.borderWidth);
      }
      const nonZero = [...widths].filter((w) => w > 0);
      if (nonZero.length > 1)
        mixed.push(`${family}.${slot}: ${nonZero.join("/")}`);
    }
  }
  row("borders/hairline-max", wide.length === 0, wide.join(", ") || "<= 1px");
  row(
    "borders/single-weight-per-surface",
    mixed.length === 0,
    mixed.join(", ") || "no surface changes weight between states",
  );

  // 5. Transient-only shadows, and no hard offsets or inner highlights.
  const strayShadow = [];
  const hardShadow = [];
  let shadowKeys = 0;
  for (const [key, recipe] of Object.entries(recipes)) {
    const layers = recipe.shadow ?? [];
    if (layers.length) {
      shadowKeys += 1;
      const family = key.split(".")[0];
      const variant = key.split(".")[1];
      const transient =
        TRANSIENT_FAMILIES.includes(family) &&
        (family !== "editor" || key.includes(".tooltip.")) &&
        (family !== "panel" || variant === "transient") &&
        (family !== "textInput" || key.includes(".focus"));
      if (!transient) strayShadow.push(key);
      for (const layer of layers) {
        // A hard *offset* shadow is the retired neobrutal press; the focus halo
        // (0/0 blur, positive spread, accent) is the approved state signal.
        if (layer.blur === 0 && (layer.x !== 0 || layer.y !== 0)) {
          hardShadow.push(`${key} ${JSON.stringify(layer)}`);
        }
      }
    }
    if ((recipe.innerHighlight ?? 0) !== 0)
      hardShadow.push(`${key} innerHighlight ${recipe.innerHighlight}`);
  }
  row(
    "shadows/transient-only",
    strayShadow.length === 0,
    `${shadowKeys} keys carry elevation` +
      (strayShadow.length ? `; stray: ${strayShadow.join(", ")}` : ""),
  );
  row(
    "shadows/no-hard-offset-or-inner-highlight",
    hardShadow.length === 0,
    hardShadow.join("; ") || "soft only",
  );

  // 6. Blur only where the language allows it.
  const strayBlur = [];
  for (const [key, recipe] of Object.entries(recipes)) {
    if (recipe.backdropBlur === undefined) continue;
    if (!BLUR_KEYS.includes(key))
      strayBlur.push(`${key}=${recipe.backdropBlur}`);
    if (recipe.backdropBlur > 8)
      strayBlur.push(`${key} blur ${recipe.backdropBlur} > 8`);
  }
  row(
    "material/blur-scrim-and-toast-only",
    strayBlur.length === 0,
    strayBlur.join(", ") || BLUR_KEYS.join(", "),
  );

  // 7. Motion stays on the tiers, and entering surfaces use the entering tier.
  const offTier = [];
  const enteringMismatch = [];
  for (const [key, recipe] of Object.entries(recipes)) {
    const duration = recipe.transitionDuration;
    if (duration === undefined) continue;
    if (!MOTION_TIERS.includes(duration)) offTier.push(`${key}=${duration}ms`);
    const springKeys = [];
    if (duration === 240 || recipe.transitionTiming === "spring-snappy")
      springKeys.push(key);
    for (const springKey of springKeys) {
      if (!ENTERING_KEYS.includes(springKey))
        enteringMismatch.push(`${springKey} springs`);
      else if (
        duration !== 240 ||
        recipe.transitionTiming !== "spring-snappy"
      ) {
        enteringMismatch.push(
          `${springKey}=${duration}ms ${recipe.transitionTiming}`,
        );
      }
    }
  }
  const enteringSeen = Object.keys(recipes).filter((k) =>
    ENTERING_KEYS.includes(k),
  );
  row(
    "motion/durations-on-tier",
    offTier.length === 0,
    offTier.join(", ") || MOTION_TIERS.join("/"),
  );
  row(
    "motion/entering-surfaces-use-the-entering-tier",
    enteringMismatch.length === 0 &&
      enteringSeen.length === ENTERING_KEYS.length,
    enteringMismatch.join(", ") ||
      `${ENTERING_KEYS.length} entering surfaces at 240ms spring-snappy`,
  );

  // 8. No filters or keyframe animation anywhere in the contract.
  const json = JSON.stringify(recipes);
  row(
    "material/no-filter-in-contract",
    !/"filter"/.test(json) && !/"animation/.test(json),
    "contract declares no filter/animation",
  );

  // 9. Host CSS adds no filter/blur of its own, and no animation beyond the ones
  // the language allows: the running-work pulse (DESIGN.md §7/§14) and a
  // surface's *entering tier* (its recipe's transition duration/timing driving
  // keyframes that only touch `opacity`/`transform`), plus the reduced-motion /
  // reduced-transparency fallbacks that remove them again. Keyframes that touch
  // paint-heavy properties are still a violation. Every stylesheet is scanned —
  // the command centre, the shell and the coding agent are where the plan-124
  // surfaces live, and a gate that only reads `components/` cannot see them.
  const cssOffenders = [];
  const cssDir = join(ROOT, "frontend/src");
  const scan = (dir) => {
    for (const entry of readdirSync(dir, { withFileTypes: true })) {
      const path = join(dir, entry.name);
      if (entry.isDirectory()) scan(path);
      // `tokens.css` *states* the fallback values rather than consuming them.
      else if (entry.name.endsWith(".css") && entry.name !== "tokens.css") {
        const text = readFileSync(path, "utf8");
        const pulseFrames = new Set();
        for (const match of text.matchAll(
          /@keyframes\s+([A-Za-z0-9_-]+)\s*\{([\s\S]*?)\n\}/g,
        )) {
          const body = match[2] ?? "";
          const properties = [...body.matchAll(/([a-z-]+)\s*:/g)].map((m) => m[1]);
          if (properties.every((property) => ["opacity", "transform"].includes(property))) {
            pulseFrames.add(match[1]);
          }
        }
        for (const [index, line] of text.split("\n").entries()) {
          // At-rules (`@supports not (backdrop-filter: …)`) gate the property,
          // they do not set it.
          if (line.trim().startsWith("@")) continue;
          const property = line.split(":")[0].trim();
          const value = line.split(":").slice(1).join(":").trim();
          // A recipe-driven blur is the scrim's and nothing else; `filter` has
          // no recipe at all, so any other use is a violation. `none` is the
          // accessibility fallback (global.css) *removing* blur, not adding it.
          const recipeBlur =
            /^\s*backdrop-filter\s*:/.test(line) &&
            /var\(--clay-ds-modal-default-scrim-rest-backdrop-blur\)/.test(
              value,
            );
          const blurOff = /^none\s*(!important)?;?$/.test(value);
          if (/\bfilter\s*:/.test(property) && (recipeBlur || blurOff)) continue;
          if (
            /\b(filter|backdrop-filter)\s*:/.test(line) &&
            !recipeBlur &&
            !blurOff
          ) {
            cssOffenders.push(
              `${path.slice(ROOT.length + 1)}:${index + 1} ${line.trim()}`,
            );
          }
          const animation = /\banimation(-name)?\s*:\s*([^;]+);/.exec(line);
          if (animation) {
            const value = animation[2] ?? "";
            const namedPulse = [...pulseFrames].some((frame) =>
              value.includes(frame),
            );
            if (!namedPulse && !/^none\s*(!important)?;?$/.test(value)) {
              cssOffenders.push(
                `${path.slice(ROOT.length + 1)}:${index + 1} ${line.trim()}`,
              );
            }
          }
        }
      }
    }
  };
  scan(cssDir);
  row(
    "material/no-filter-or-animation-in-host-css",
    cssOffenders.length === 0,
    cssOffenders.join("; ") || "every stylesheet clean (minus tokens.css)",
  );

  // 10. The specimen's own gaps are the manifest's gaps (no hidden omissions).
  row(
    "coverage/specimen-gaps-are-in-the-contract",
    [...specimen.absent].every((key) => !(key in recipes)),
    `${specimen.absent.size} cells explicitly absent`,
  );

  row(
    "deviations/recorded",
    true,
    `${RECORDED_DEVIATIONS.length} recorded: ${RECORDED_DEVIATIONS.map((d) => d.id).join(", ")}`,
  );
  return {
    checks,
    findings,
    stats: {
      keys: Object.keys(recipes).length,
      families: byFamily.size,
      kinds: kinds.size,
      specimenKeys: specimen.keys.size,
      specimenFamilies: specimen.families.length,
    },
  };
}

// ---------------------------------------------------------------- theme values

/**
 * Mirrors `ResolvedUiTheme::base_color` (src/shell/theme.rs) so the browser pass
 * paints the same `--clay-*` values the host would install for a theme: the
 * theme's `textStyles` palette, layered under its approved typed overrides.
 */
const BASE_PROJECTION = {
  "surface.main": "shellBg",
  "surface.panel": "panelBg",
  "surface.list": "panelBg",
  "surface.overlay": "panelBg",
  "surface.tooltip": "panelBg",
  "surface.disabled": "panelBg",
  "surface.control": "statusBg",
  "surface.badge": "statusBg",
  "surface.kbd": "statusBg",
  "surface.hover": "selection",
  "surface.active": "selection",
  "surface.selected": "selection",
  "surface.scrollbar": "scrollbar",
  "surface.scrollbar.track": "scrollbarTrack",
  "text.primary": "text",
  "text.tooltip": "text",
  "text.badge": "statusText",
  "text.kbd": "statusText",
  "text.disabled": "placeholder",
  "accent.muted": "placeholder",
  "text.icon": "placeholder",
  "border.kbd": "placeholder",
  // A theme may declare the accent and the border ladder itself; absent, the
  // projection keeps the caret / scrollbar stand-ins (plan 118 task E7), which is
  // what the list form says: first declared field wins.
  "accent.primary": ["accent", "caret"],
  "focus.ring": ["accent", "caret"],
  "border.focus": ["accent", "caret"],
  "border.hairline": ["borderHairline", "scrollbar"],
  "border.subtle": ["borderSubtle", "scrollbar"],
  "border.strong": ["borderStrong", "scrollbar"],
  "diagnostic.error": "diagnosticError",
  "diagnostic.warning": "diagnosticWarning",
  "diagnostic.info": "diagnosticInfo",
};

const THEMES = [
  {
    specifier: "@clay/theme-modus-operandi",
    dir: "theme-modus-operandi",
    page: "modus-operandi",
  },
  {
    specifier: "@clay/theme-modus-vivendi",
    dir: "theme-modus-vivendi",
    page: "modus-vivendi",
  },
  {
    specifier: "@clay/theme-gruvbox-material-dark",
    dir: "theme-gruvbox-material-dark",
    page: "gruvbox-material-dark",
  },
  {
    specifier: "@clay/theme-gruvbox-material-light",
    dir: "theme-gruvbox-material-light",
    page: "gruvbox-material-light",
  },
];

function themeVariables({ dir, specifier }) {
  const manifest = JSON.parse(
    readFileSync(join(ROOT, "packages", dir, "package.json"), "utf8"),
  );
  const styles = Object.fromEntries(
    manifest.clay.contributions.textStyles.map((entry) => [
      entry.token,
      entry.color,
    ]),
  );
  const proposal = JSON.parse(readFileSync(THEME_VALUES, "utf8")).themes[
    specifier
  ];
  const variables = {};
  for (const [token, fields] of Object.entries(BASE_PROJECTION)) {
    const candidates = Array.isArray(fields) ? fields : [fields];
    const field = candidates.find((candidate) => styles[candidate]);
    if (field) variables[token] = styles[field];
  }
  const panel = hexToRgb(styles.panelBg ?? "#000000");
  const placeholder = styles.placeholder;
  const muted =
    placeholder && contrastRatio(hexToRgb(placeholder), panel) >= 4.5
      ? placeholder
      : styles.text;
  variables["text.muted"] = muted;
  for (const [token, value] of Object.entries(proposal?.tokens ?? {}))
    variables[token] = value;
  return variables;
}

// ---------------------------------------------------------------- browser pass

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

function findChrome() {
  const candidates = [
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
  return candidates.find((p) => existsSync(p));
}

async function connect(port) {
  const chrome = findChrome();
  if (!chrome) throw new Error("no Chrome/Chromium found");
  spawn(
    chrome,
    [
      "--headless=new",
      "--no-sandbox",
      "--disable-gpu",
      "--hide-scrollbars",
      "--no-first-run",
      "--force-device-scale-factor=1",
      `--user-data-dir=/tmp/clay-conformance-profile`,
      `--remote-debugging-port=${port}`,
      "about:blank",
    ],
    { stdio: "ignore", detached: true },
  ).unref();
  let url;
  for (let i = 0; i < 150; i += 1) {
    try {
      const list = await (
        await fetch(`http://127.0.0.1:${port}/json/list`)
      ).json();
      const page = list.find((t) => t.type === "page");
      if (page) {
        url = page.webSocketDebuggerUrl;
        break;
      }
    } catch {
      /* not up yet */
    }
    await sleep(100);
  }
  if (!url) throw new Error("chrome did not expose a debug target");
  const ws = new WebSocket(url);
  await new Promise((r) => ws.addEventListener("open", r, { once: true }));
  let seq = 0;
  const pending = new Map();
  ws.addEventListener("message", (raw) => {
    const message = JSON.parse(raw.data);
    if (message.id && pending.has(message.id)) {
      const { resolve, reject } = pending.get(message.id);
      pending.delete(message.id);
      if (message.error) reject(new Error(JSON.stringify(message.error)));
      else resolve(message.result);
    }
  });
  const send = (method, params = {}) =>
    new Promise((resolvePromise, reject) => {
      const id = ++seq;
      pending.set(id, { resolve: resolvePromise, reject });
      ws.send(JSON.stringify({ id, method, params }));
    });
  await send("Page.enable");
  await send("Runtime.enable");
  // Headless pages are unfocused, so `element.focus()` sets activeElement but
  // `:focus`/`:focus-visible` never match; the focus-state probes need both.
  await send("Emulation.setFocusEmulationEnabled", { enabled: true });
  await send("Page.bringToFront");
  await send("Emulation.setDeviceMetricsOverride", {
    width: 1280,
    height: 900,
    deviceScaleFactor: 1,
    mobile: false,
  });
  return { send, close: () => send("Browser.close").catch(() => {}) };
}

/** Reads component nodes and computes the properties the language controls. */
const READ_NODES = `(() => {
  const read = (el) => {
    const s = getComputedStyle(el);
    return {
      radius: s.borderTopLeftRadius,
      borderTop: s.borderTopWidth,
      borderLeft: s.borderLeftWidth,
      borderStyle: s.borderTopStyle,
      shadow: s.boxShadow,
      blur: s.backdropFilter,
      filter: s.filter,
      transition: Math.max(0, ...s.transitionDuration.split(',').map((v) => v.trim().endsWith('ms') ? parseFloat(v) : parseFloat(v) * 1000).filter(Number.isFinite)),
      timing: s.transitionTimingFunction,
      animation: s.animationName,
      outline: s.outlineWidth + ' ' + s.outlineStyle,
      outlineWidth: s.outlineWidth,
      outlineStyle: s.outlineStyle,
      outlineOffset: s.outlineOffset,
      transform: s.transform,
      background: s.backgroundColor,
      borderColor: s.borderTopColor,
      color: s.color,
      opacity: s.opacity,
    };
  };
  const nodes = [...document.querySelectorAll('[data-clay-component]')].map((el) => {
    const component = el.getAttribute('data-clay-component');
    const slot = el.getAttribute('data-clay-slot') || 'root';
    const variant = el.getAttribute('data-variant') || 'default';
    const states = ['hovered','pressed','focused','focusVisible','selected','disabled','invalid','expanded']
      .filter((name) => el.hasAttribute('data-' + name.replace(/[A-Z]/g, (c) => '-' + c.toLowerCase())));
    return { key: component + '.' + variant + '.' + slot + '.rest', component, slot, variant, states, ...read(el) };
  });
  // A specimen cell's attributes land on the cell's outer element, which is a
  // wrapper for some slots (a kbd inside its shell). Follow the single-child
  // chain while the outer element paints nothing, so both sides are compared on
  // the element the language actually styles.
  const paints = (values) =>
    parseFloat(values.radius) > 0 || parseFloat(values.borderLeft) > 0 || values.shadow !== 'none';
  const painter = (el) => {
    let node = el;
    let values = read(node);
    while (!paints(values) && node.children.length === 1 && node.firstElementChild) {
      node = node.firstElementChild;
      values = read(node);
    }
    return { values, painted: paints(values), wrapper: node !== el };
  };
  const recipes = [...document.querySelectorAll('[data-recipe]')].map((el) => {
    const found = painter(el);
    return { key: el.getAttribute('data-recipe'), ...found.values, painted: found.painted, wrapper: found.wrapper };
  });
  return { nodes, recipes, variables: {
    hover: getComputedStyle(document.documentElement).getPropertyValue('--clay-surface-hover').trim(),
    active: getComputedStyle(document.documentElement).getPropertyValue('--clay-surface-active').trim(),
    selected: getComputedStyle(document.documentElement).getPropertyValue('--clay-surface-selected').trim(),
    accent: getComputedStyle(document.documentElement).getPropertyValue('--clay-accent-primary').trim(),
    focus: getComputedStyle(document.documentElement).getPropertyValue('--clay-focus-ring').trim(),
    main: getComputedStyle(document.documentElement).getPropertyValue('--clay-surface-main').trim(),
    panel: getComputedStyle(document.documentElement).getPropertyValue('--clay-surface-panel').trim(),
  } };
})()`;

async function evaluate(send, expression) {
  const { result, exceptionDetails } = await send("Runtime.evaluate", {
    expression,
    returnByValue: true,
    awaitPromise: true,
  });
  if (exceptionDetails)
    throw new Error(exceptionDetails.text ?? JSON.stringify(exceptionDetails));
  return result.value;
}

async function shoot(send, name) {
  const { data } = await send("Page.captureScreenshot", { format: "png" });
  writeFileSync(join(EVIDENCE, `${name}.png`), Buffer.from(data, "base64"));
}

async function browserPass(baseUrl, auditResult) {
  const recipes = loadManifest().recipes;
  const specimen = loadSpecimen();
  mkdirSync(EVIDENCE, { recursive: true });
  const { send, close } = await connect(9644);
  const report = { base: baseUrl, themes: [], deviations: [] };
  const check = (id, ok, detail) => {
    report.deviations.push({ id, ok, detail });
    return ok;
  };

  try {
    for (const theme of [
      { specifier: "@clay/core", dir: null, page: null },
      ...THEMES,
    ]) {
      const variables = theme.dir ? themeVariables(theme) : null;
      const injection = variables
        ? Object.entries(variables)
            .map(
              ([token, value]) =>
                `r.style.setProperty('--clay-${token.replace(/\./g, "-")}', '${value}')`,
            )
            .join(";")
        : "";
      const entry = { theme: theme.specifier, app: null, specimen: null };

      // --- app side: the DEV component gallery under this theme's roles.
      await send("Page.navigate", { url: `${baseUrl}/?fixture=controls` });
      await sleep(1600);
      if (injection) {
        await evaluate(
          send,
          `(() => { const r = document.documentElement; ${injection}; return true })()`,
        );
        await sleep(250);
      }
      entry.app = await evaluate(send, READ_NODES);
      await shoot(send, `gallery-${theme.page ?? "core-baseline"}`);

      // --- specimen side: the approved page under its own theme switch.
      const specimenUrl = `${pathToFileURL(SPECIMEN).href}${theme.page ? `?theme=${theme.page}` : ""}`;
      await send("Page.navigate", { url: specimenUrl });
      await sleep(1500);
      entry.specimen = await evaluate(send, READ_NODES);
      await evaluate(
        send,
        `document.getElementById('sec-buttons')?.scrollIntoView({block:'start'}); true`,
      );
      await sleep(200);
      await shoot(send, `specimen-${theme.page ?? "default"}`);

      // --- compare what each app node paints against its declared recipe.
      const mismatches = [];
      const notes = [];
      const recordedKeys = new Set(
        RECORDED_DEVIATIONS.flatMap((d) => d.keys ?? []),
      );
      for (const node of entry.app.nodes) {
        if (HOST_ONLY_KINDS.includes(node.component)) continue;
        const recipe = recipes[node.key];
        if (!recipe) {
          // Undeclared host slots (`†` in the recipe matrix) and variant names
          // carried over from a shared component are expected; a component the
          // contract never declares is not.
          const declaredElsewhere =
            recipes[`${node.component}.default.${node.slot}.rest`];
          const anyKey = Object.keys(recipes).some((k) =>
            k.startsWith(`${node.component}.`),
          );
          if (!anyKey)
            mismatches.push(
              `${node.key}: the contract declares no ${node.component} recipe at all`,
            );
          else
            notes.push({
              kind: declaredElsewhere ? "variant" : "undeclaredSlot",
              key: node.key,
              detail: `${node.key}: ${declaredElsewhere ? "variant attr not in the contract" : "undeclared host slot († in the recipe matrix)"}`,
            });
          continue;
        }
        if (recordedKeys.has(node.key)) continue;
        if (
          recipe.borderRadius !== undefined &&
          !isPxEqual(node.radius, recipe.borderRadius)
        ) {
          mismatches.push(
            `${node.key}: radius ${node.radius} != recipe ${recipe.borderRadius}px`,
          );
        }
        // A recipe that declares no boundary is painted as a transparent hairline
        // by the shared host rules so variant boxes keep identical metrics; that
        // is not a boundary and the approved specimen paints it the same way.
        const transparent = /rgba?\(0,\s*0,\s*0,\s*0\)|\/\s*0\)/.test(
          node.borderColor ?? "",
        );
        if (
          recipe.borderWidth !== undefined &&
          !isPxEqual(node.borderLeft, recipe.borderWidth) &&
          !(recipe.borderWidth === 0 && transparent)
        ) {
          mismatches.push(
            `${node.key}: border ${node.borderLeft} != recipe ${recipe.borderWidth}px`,
          );
        }
        if (
          recipe.borderWidth === 0 &&
          px(node.borderLeft) > 0 &&
          transparent
        ) {
          notes.push({
            kind: "metricsOnly",
            key: node.key,
            detail: `${node.key}: 0-width recipe painted as a transparent hairline (metrics only)`,
          });
        }
        const wantedLayers = (recipe.shadow ?? []).length;
        const paintedLayers = countShadowLayers(node.shadow);
        if (paintedLayers !== wantedLayers) {
          mismatches.push(
            `${node.key}: ${paintedLayers} shadow layer(s) != recipe ${wantedLayers}`,
          );
        }
        const wantedDuration = recipe.transitionDuration ?? 0;
        if (wantedDuration > 0 && node.transition !== wantedDuration) {
          mismatches.push(
            `${node.key}: transition ${node.transition}ms != recipe ${wantedDuration}ms`,
          );
        }
        if (
          recipe.backdropBlur === undefined &&
          node.blur !== "none" &&
          node.blur !== ""
        ) {
          mismatches.push(
            `${node.key}: blur ${node.blur} without a blur recipe`,
          );
        }
        if (node.filter !== "none" && node.filter !== "") {
          mismatches.push(`${node.key}: filter ${node.filter}`);
        }
        if (
          node.animation !== "none" &&
          node.animation !== "" &&
          !(
            node.key === "modal.default.dialog.rest" &&
            RECORDED_DEVIATIONS.some((d) => d.id === "modal.entrance-animation")
          )
        ) {
          mismatches.push(`${node.key}: animation ${node.animation}`);
        }
        if (
          node.radius &&
          !RADIUS_LADDER.includes(px(node.radius)) &&
          px(node.radius) !== 0
        ) {
          mismatches.push(
            `${node.key}: painted radius ${node.radius} off the ladder`,
          );
        }
        if (px(node.borderLeft) > 1 || px(node.borderTop) > 1) {
          mismatches.push(`${node.key}: painted border > hairline`);
        }
      }
      entry.appMismatches = mismatches;
      entry.appNotes = [...new Set(notes)];

      // --- compare the specimen's own rendering of the same keys.
      const specimenByKey = new Map(
        entry.specimen.recipes.map((r) => [r.key, r]),
      );
      const crossChecked = [];
      for (const node of entry.app.nodes) {
        if (HOST_ONLY_KINDS.includes(node.component)) continue;
        const mirror = specimenByKey.get(node.key);
        if (!mirror) continue;
        if (!mirror.painted) continue; // the approved cell shows this slot as text/layout only
        if (RECORDED_CROSS_DEVIATIONS.some((d) => d.keys.includes(node.key)))
          continue;
        const agrees =
          isPxEqual(node.radius, mirror.radius) &&
          isPxEqual(node.borderLeft, mirror.borderLeft) &&
          countShadowLayers(node.shadow) === countShadowLayers(mirror.shadow);
        crossChecked.push({
          key: node.key,
          agrees,
          app: {
            radius: node.radius,
            border: node.borderLeft,
            shadow: countShadowLayers(node.shadow),
          },
          specimen: {
            radius: mirror.radius,
            border: mirror.borderLeft,
            shadow: countShadowLayers(mirror.shadow),
          },
        });
      }
      entry.crossChecked = crossChecked;
      entry.crossCompared = crossChecked.length;
      entry.crossAgreed = crossChecked.filter((c) => c.agrees).length;
      entry.crossMismatches = crossChecked
        .filter((c) => !c.agrees)
        .map(
          (c) =>
            `${c.key}: app ${JSON.stringify(c.app)} vs specimen ${JSON.stringify(c.specimen)}`,
        );
      report.themes.push(entry);
    }

    // --- interaction probes: a state must resolve to the role the recipe
    // declares, not merely to a plausible colour. Expectations are CSS
    // expressions so the page resolves them with the same variables the
    // component reads.
    await send("Page.navigate", { url: `${baseUrl}/?fixture=controls` });
    await sleep(1600);
    const before = await evaluate(send, READ_NODES);
    const probes = [];
    // Resolve an expectation through the page's own variables, so the comparison
    // is against the value the component reads, not against a re-typed literal.
    const resolveValue = (css, property) =>
      evaluate(
        send,
        `(() => { const d = document.createElement('div'); d.style.${property} = ${JSON.stringify(css)}; document.body.appendChild(d); const v = getComputedStyle(d).${property}; d.remove(); return v })()`,
      );
    const probe = async (name, expression, key, state, expectations) => {
      // Each probe starts from the rest state; the forced attributes are removed
      // again so a probe never measures the previous probe's state.
      await evaluate(
        send,
        `(() => { for (const el of document.querySelectorAll('[data-clay-component]')) for (const a of ['hovered','pressed','selected']) el.removeAttribute('data-' + a); return true })()`,
      );
      await evaluate(send, expression);
      // Long enough for the recipe's 150ms state transition to settle, so the
      // computed value is the destination and not an in-flight interpolation.
      await sleep(520);
      const after = await evaluate(send, READ_NODES);
      const node = after.nodes.find((n) => n.key === key);
      const checks = [];
      for (const [prop, css] of Object.entries(expectations)) {
        const property =
          prop === "background"
            ? "backgroundColor"
            : prop === "border"
              ? "borderColor"
              : prop;
        const expected = await resolveValue(css, property);
        const painted = prop === "border" ? node?.borderColor : node?.[prop];
        const ok =
          prop === "transform"
            ? normalizeTransform(painted) === normalizeTransform(expected)
            : prop === "outlineWidth" || prop === "outlineOffset"
              ? Math.abs(px(painted) - px(expected)) < 0.51
              : prop === "outlineStyle"
                ? String(painted) === String(expected)
                : colorsEqual(painted, expected);
        checks.push({ prop, css, painted, expected, ok });
      }
      probes.push({
        name,
        key,
        state,
        states: node?.states ?? [],
        radius: node?.radius,
        outline: `${node?.outline} offset ${node?.outlineOffset}`,
        shadowLayers: countShadowLayers(node?.shadow ?? "none"),
        checks,
      });
    };
    await probe(
      "button hover",
      `(() => { const b = document.querySelector('[data-fixture] [data-clay-component="button"][data-clay-slot="root"]'); b?.setAttribute('data-hovered',''); return true })()`,
      "button.default.root.rest",
      "hover",
      {
        background: "var(--clay-surface-hover)",
        border: "var(--clay-border-subtle)",
      },
    );
    await probe(
      "text input focus",
      `(() => { const i = document.querySelector('[data-clay-component="textInput"] input'); i?.focus(); return true })()`,
      "textInput.default.input.rest",
      "focus",
      {
        background: "var(--clay-surface-control)",
        border: "var(--clay-accent-primary)",
      },
    );
    await probe(
      "list row selection",
      `(() => { const r = document.querySelector('[data-clay-component="list"][data-clay-slot="row"]'); r?.setAttribute('data-selected',''); return true })()`,
      "list.default.row.rest",
      "selected",
      {
        background:
          "color-mix(in srgb, var(--clay-accent-primary) 15%, transparent)",
      },
    );
    await probe(
      "button focus",
      `(() => { const b = document.querySelector('[data-fixture] [data-clay-component="button"][data-clay-slot="root"]'); b?.focus(); return true })()`,
      "button.default.root.rest",
      "focus",
      {
        outlineWidth: "var(--clay-ds-button-default-root-focus-outline-width)",
        outlineStyle: "var(--clay-ds-button-default-root-focus-outline-style)",
        outlineOffset:
          "var(--clay-ds-button-default-root-focus-outline-offset)",
      },
    );
    await probe(
      "button press",
      `(() => { const b = document.querySelector('[data-fixture] [data-clay-component="button"][data-clay-slot="root"]'); b?.setAttribute('data-pressed',''); return true })()`,
      "button.default.root.rest",
      "pressed",
      {
        background: "var(--clay-surface-active)",
        transform: "var(--clay-ds-button-default-root-active-transform-preset)",
      },
    );
    report.probes = probes;
    report.probeFailures = probes.flatMap((p) =>
      p.checks
        .filter((c) => !c.ok)
        .map(
          (c) =>
            `${p.name} (${p.state}) ${c.prop}: ${c.painted} != ${c.expected} via ${c.css}`,
        ),
    );
    report.baselineNodeCount = before.nodes.length;

    // --- paint-cost invariants across the other fixtures: no node may filter,
    // blur where the contract does not, or animate.
    const paint = [];
    for (const fixture of [
      "states",
      "editor",
      "package-ui",
      "command-centre",
    ]) {
      await send("Page.navigate", { url: `${baseUrl}/?fixture=${fixture}` });
      await sleep(1500);
      const { nodes } = await evaluate(send, READ_NODES);
      for (const node of nodes) {
        if (node.filter !== "none" && node.filter !== "")
          paint.push(`${fixture} ${node.key}: filter ${node.filter}`);
        if (
          node.blur !== "none" &&
          node.blur !== "" &&
          !["modal.default.scrim.rest", "toast.default.root.rest"].includes(
            node.key,
          )
        ) {
          paint.push(`${fixture} ${node.key}: blur ${node.blur}`);
        }
        if (
          node.animation !== "none" &&
          node.animation !== "" &&
          node.key !== "modal.default.dialog.rest"
        ) {
          paint.push(`${fixture} ${node.key}: animation ${node.animation}`);
        }
      }
    }
    report.paintViolations = paint;
  } finally {
    close();
  }

  const auditFacts = auditResult.checks.filter((c) => !c.ok);
  const appMismatches = report.themes.flatMap((t) =>
    t.appMismatches.map((m) => `${t.theme}: ${m}`),
  );
  const crossMismatches = report.themes.flatMap((t) =>
    t.crossMismatches.map((m) => `${t.theme}: ${m}`),
  );
  report.summary = {
    themes: report.themes.length,
    appNodes: report.themes.reduce((n, t) => n + t.app.nodes.length, 0),
    specimenKeys: report.themes.reduce(
      (n, t) => n + t.specimen.recipes.length,
      0,
    ),
    crossChecked: report.themes.reduce((n, t) => n + t.crossChecked.length, 0),
    appMismatches,
    crossMismatches,
    undeclaredSlotKeys: [
      ...new Set(
        report.themes.flatMap((t) =>
          (t.appNotes ?? [])
            .filter((n) => n.kind !== "metricsOnly")
            .map((n) => n.key),
        ),
      ),
    ],
    metricsOnlyKeys: [
      ...new Set(
        report.themes.flatMap((t) =>
          (t.appNotes ?? [])
            .filter((n) => n.kind === "metricsOnly")
            .map((n) => n.key),
        ),
      ),
    ],
    paintViolations: report.paintViolations,
    probeFailures: report.probeFailures,
    auditFailures: auditFacts.map((c) => `${c.id}: ${c.detail}`),
  };

  // A recorded deviation that the run no longer sees should be removed, not left
  // to rot: report it as still-recorded so the list stays honest.
  report.recordedStillAbsent = RECORDED_DEVIATIONS.map((d) => d.id);
  writeFileSync(
    join(EVIDENCE, "report.json"),
    `${JSON.stringify(report, null, 2)}\n`,
  );
  return report;
}

// ---------------------------------------------------------------- main

const args = process.argv.slice(2);
const browserIndex = args.indexOf("--browser");
const baseUrl =
  browserIndex >= 0
    ? (args[browserIndex + 1] ?? "http://localhost:5199")
    : null;

const result = audit();
const pad = (s, n) => String(s).padEnd(n);
console.log("Component-level conformance — plan 118\n");
console.log(
  `contract: ${result.stats.keys} keys · ${result.stats.families} families · ${result.stats.kinds} kinds · specimen ${result.stats.specimenKeys} keys / ${result.stats.specimenFamilies} families\n`,
);
for (const check of result.checks) {
  console.log(
    `${check.ok ? "ok  " : "FAIL"}  ${pad(check.id, 46)} ${check.detail}`,
  );
}
for (const finding of result.findings)
  console.log(`note        ${pad(finding.id, 46)} ${finding.detail}`);
const failed = result.checks.filter((c) => !c.ok);
console.log(
  `\naudit: ${result.checks.length - failed.length}/${result.checks.length} checks passed`,
);

if (!baseUrl) {
  process.exit(failed.length ? 1 : 0);
}

const report = await browserPass(baseUrl, result);
console.log("\nbrowser pass:");
for (const theme of report.themes) {
  console.log(
    `  ${pad(theme.theme, 32)} app ${pad(theme.app.nodes.length, 3)} nodes · specimen ${pad(theme.specimen.recipes.length, 3)} keys · compared ${pad(theme.crossCompared, 3)} (agreed ${theme.crossAgreed}) · app mismatches ${theme.appMismatches.length} · cross mismatches ${theme.crossMismatches.length}`,
  );
}
console.log(
  `  probes: ${report.probes.map((p) => `${p.name} [${p.states.join(",") || "no state attr"}] ${p.checks.filter((c) => c.ok).length}/${p.checks.length} roles, radius ${p.radius}, shadow ${p.shadowLayers}, outline ${p.outline}`).join("\n           ")}`,
);
console.log(`  paint violations: ${report.paintViolations.length}`);
console.log(
  `  undeclared host slots rendered († in the matrix): ${report.summary.undeclaredSlotKeys.length} — ${report.summary.undeclaredSlotKeys.join(", ")}`,
);
console.log(
  `  0-width recipes painted as transparent hairlines: ${report.summary.metricsOnlyKeys.length} — ${report.summary.metricsOnlyKeys.join(", ")}`,
);
for (const line of report.summary.appMismatches)
  console.log(`  APP    ${line}`);
for (const line of report.summary.crossMismatches)
  console.log(`  CROSS  ${line}`);
for (const line of report.paintViolations) console.log(`  PAINT  ${line}`);
for (const line of report.probeFailures) console.log(`  PROBE  ${line}`);
console.log(
  `  evidence: ${EVIDENCE.slice(ROOT.length + 1)}/report.json + ${report.themes.length * 2} screenshots`,
);

process.exit(
  failed.length ||
    report.summary.appMismatches.length ||
    report.paintViolations.length ||
    report.probeFailures.length
    ? 1
    : 0,
);
