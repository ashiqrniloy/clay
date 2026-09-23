// Theme adapter: projects one resolved Rust theme snapshot into CSS custom
// properties. Runs once per theme/typography install — never per frame.
//
// Naming rule (locked by docs/development/react-ui-catalog-mapping.md):
// `token.name.sub` → `--clay-token-name-sub`.

import {
  LINE_HEIGHT_MULTIPLIER,
  TEXT_VARIANTS,
  type TextVariant,
  type ThemeSnapshot,
  type ThemeTokenValue,
  type TypographySnapshot,
} from "./types";

/** `surface.scrollbar.track` → `--clay-surface-scrollbar-track`. */
export function tokenToCssName(token: string): string {
  return `--clay-${token.replaceAll(".", "-").toLowerCase()}`;
}

/** Catalog z-level names → CSS stacking integers (native sort order). */
function zIndex(level: string): string {
  switch (level) {
    case "base":
      return "0";
    case "panel":
      return "10";
    case "overlay":
      return "20";
    case "modal":
      return "40";
    case "tooltip":
      return "50";
    default:
      return "0";
  }
}

function cssValue(token: string, value: ThemeTokenValue): string | null {
  switch (value.type) {
    case "color":
    case "variant":
      return value.value;
    case "level":
      // z.* levels become numeric stacking contexts; other level domains
      // (density/elevation) keep their catalog names for diagnostics.
      if (token.startsWith("z.")) return zIndex(value.value);
      return value.value;
    case "opacity":
      return String(value.value);
    case "scalar":
      // Motion durations are milliseconds; every other scalar domain
      // (spacing/radius/dimension) is logical pixels.
      return token.startsWith("motion.")
        ? `${value.value}ms`
        : `${value.value}px`;
  }
}

/**
 * Core-token variables for one snapshot, with the density scale pre-applied
 * to the spacing rhythm (density never rescales dimensions or radii).
 */
export function themeCssVariables(theme: ThemeSnapshot): [string, string][] {
  const variables: [string, string][] = [];
  for (const [token, value] of Object.entries(theme.tokens)) {
    let emitted = value;
    if (
      value.type === "scalar" &&
      token.startsWith("spacing.") &&
      theme.densityScale !== 1
    ) {
      emitted = { type: "scalar", value: value.value * theme.densityScale };
    }
    const css = cssValue(token, emitted);
    if (css !== null) variables.push([tokenToCssName(token), css]);
  }
  for (const [token, style] of Object.entries(theme.editorStyles ?? {})) {
    const prefix = `--clay-editor-${token.toLowerCase()}`;
    variables.push([`${prefix}-color`, style.color]);
    if (style.background)
      variables.push([`${prefix}-background`, style.background]);
    variables.push([`${prefix}-scale`, String(style.scale)]);
    variables.push([`${prefix}-weight`, style.bold ? "bold" : "normal"]);
    variables.push([`${prefix}-style`, style.italic ? "italic" : "normal"]);
    variables.push([
      `${prefix}-decoration`,
      [style.underline && "underline", style.strike && "line-through"]
        .filter(Boolean)
        .join(" ") || "none",
    ]);
  }
  return variables.sort(([a], [b]) => a.localeCompare(b));
}

function familyStack(profile: { families: string[] }): string {
  return profile.families
    .map((family) => `'${family.replaceAll("'", "")}'`)
    .join(", ");
}

/**
 * Font-role and text-variant variables from the user-owned typography
 * snapshot. Variant sizes are computed once here: role base × hierarchy
 * scale; components consume finished sizes only.
 */
export function typographyCssVariables(
  typography: TypographySnapshot,
): [string, string][] {
  const variables: [string, string][] = [
    ["--clay-font-ui", familyStack(typography.ui)],
    ["--clay-font-monospace", familyStack(typography.monospace)],
    ["--clay-font-proportional", familyStack(typography.proportional)],
  ];
  for (const variant of TEXT_VARIANTS) {
    const scale = typography.hierarchy[variant];
    const size = typography.ui.size * scale;
    variables.push([`--clay-text-${variant}-size`, `${size}px`]);
    variables.push([
      `--clay-text-${variant}-line-height`,
      `${size * LINE_HEIGHT_MULTIPLIER}px`,
    ]);
  }
  // Ligature policy drives the UI font stack's OpenType feature setting.
  variables.push([
    "--clay-font-feature-settings",
    typography.ui.ligatures.enableStandard
      ? '"liga" 1, "clig" 1'
      : '"liga" 0, "clig" 0',
  ]);
  return variables;
}

/** Applies variables to an element style (app root in production). */
export function installVariables(
  target: CSSStyleDeclaration,
  variables: readonly [string, string][],
): void {
  for (const [name, value] of variables) target.setProperty(name, value);
}

/** Resolves the active text-variant size the adapter computed for `variant`. */
export function variantSize(
  typography: TypographySnapshot,
  variant: TextVariant,
): number {
  return typography.ui.size * typography.hierarchy[variant];
}
