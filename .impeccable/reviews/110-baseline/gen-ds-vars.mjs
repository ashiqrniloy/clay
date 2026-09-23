import { readFileSync } from "node:fs";

const TIMING = {
  "linear": "linear",
  "ease-in": "cubic-bezier(0.4, 0, 1, 1)",
  "ease-out": "cubic-bezier(0, 0, 0.2, 1)",
  "ease-in-out": "cubic-bezier(0.4, 0, 0.2, 1)",
  "spring-subtle": "cubic-bezier(0.16, 1, 0.3, 1)",
  "spring-snappy": "cubic-bezier(0.2, 0.8, 0.2, 1)",
};
const TRANSFORM = {
  "none": "none",
  "press-subtle": "scale(0.98)",
  "press-shift-down": "translateY(1px)",
  "hover-lift": "translateY(-1px)",
};
const COLOR_PREFIXES = ["surface.", "text.", "accent.", "border.", "diagnostic.", "focus.", "selection."];
const SPACING_RE = /^spacing\./;
const PX_PROPS = new Set(["borderRadius","borderWidth","outlineWidth","outlineOffset","backdropBlur"]);

function kebab(s){return s.replace(/([a-z0-9])([A-Z])/g,"$1-$2").toLowerCase();}
function cssRole(role){return `var(--clay-${role.replaceAll(".","-")})`;}

function shadowCss(layers){
  if(!layers.length) return "none";
  return layers.map(l=>{
    const c = cssRole(l.colorRole ?? l.color);
    const col = l.opacity===1?c:`color-mix(in srgb, ${c} ${Math.round(l.opacity*100)}%, transparent)`;
    return `${l.x}px ${l.y}px ${l.blur}px ${l.spread}px ${col}`;
  }).join(", ");
}

const pkg = JSON.parse(readFileSync(process.argv[2],"utf8"));
const ds = pkg.clay.contributions.uiDesignSystem;
const entries = [];
for (const [key, recipe] of Object.entries(ds.recipes)) {
  const base = `--clay-ds-${key.replaceAll(".","-")}`;
  for (const [prop, v] of Object.entries(recipe)) {
    const name = `${base}-${kebab(prop)}`;
    let val = null;
    if (prop === "shadow") val = shadowCss(v);
    else if (prop === "transitionTiming") val = TIMING[v] ?? null;
    else if (prop === "transformPreset") val = TRANSFORM[v] ?? null;
    else if (prop === "transitionDuration") val = `${v}ms`;
    else if (prop === "backdropSaturate" || prop === "opacity" || prop === "backgroundOpacity") val = String(v);
    else if (typeof v === "string" && SPACING_RE.test(v)) val = cssRole(v);
    else if (typeof v === "string" && COLOR_PREFIXES.some(p=>v.startsWith(p))) val = cssRole(v);
    else if (typeof v === "string") val = v; // borderStyle etc.
    else if (typeof v === "number" && PX_PROPS.has(prop)) val = `${v}px`;
    else if (typeof v === "number") val = String(v);
    if (val !== null) entries.push([name, val]);
  }
}
entries.sort((a,b)=>a[0].localeCompare(b[0]));
console.log(JSON.stringify(entries));
