// Design System adapter: projects one resolved Rust ActiveDesignSystem snapshot
// into CSS custom properties. Runs once per design-system activation / snapshot — never per frame.
//
// Naming rule:
// `button.primary.root.rest.backgroundColor` → `--clay-ds-button-primary-root-rest-background-color`
//
// Security & authority constraints:
// - Prohibits literal colors (hex, rgb, hsl) and package-owned color aliases.
// - Resolves color-role tokens to host-owned `--clay-*` CSS variables.
// - Validates bounds on all dimensions, radii, opacities, motion, blur, saturate.
// - Deterministic property generation with zero runtime style injections or raw CSS text.

import { tokenToCssName } from "./adapter";
import type {
  DesignSystemSnapshot,
  DesignSystemVariableValue,
  InnerHighlightDto,
  ShadowLayerDto,
} from "./types";

/**
 * Valid theme color roles permitted in recipe color fields.
 * Content themes are the sole color authority in Clay.
 */
export const VALID_THEME_COLOR_ROLES: ReadonlySet<string> = new Set([
  "surface.main",
  "surface.panel",
  "surface.overlay",
  "surface.scrim",
  "surface.control",
  "surface.list",
  "surface.selected",
  "surface.hover",
  "surface.active",
  "surface.disabled",
  "surface.badge",
  "surface.kbd",
  "surface.scrollbar",
  "surface.scrollbar.track",
  "text.primary",
  "text.muted",
  "text.disabled",
  "text.badge",
  "text.kbd",
  "accent.primary",
  "focus.ring",
  "border.hairline",
  "border.subtle",
  "border.strong",
  "border.focus",
  "border.kbd",
  "diagnostic.error",
  "diagnostic.warning",
  "diagnostic.success",
  "selection.background",
  "selection.foreground",
  "selection.inactive",
  "transparent",
]);

/**
 * Converts a dotted recipe property path to a kebab-cased CSS custom property name.
 * e.g. `button.primary.root.rest.backgroundColor` → `--clay-ds-button-primary-root-rest-background-color`
 */
export function recipeVariableToCssName(variableKey: string): string {
  const sanitized = variableKey
    .replace(/([a-z0-9])([A-Z])/g, "$1-$2")
    .replaceAll(".", "-")
    .toLowerCase();
  return `--clay-ds-${sanitized}`;
}

/**
 * Formats and validates a theme color role into a host CSS variable reference.
 * Returns `null` if the role is not in the allowed theme token catalog.
 */
export function formatColorRole(role: string): string | null {
  const trimmed = role.trim();
  if (!VALID_THEME_COLOR_ROLES.has(trimmed)) {
    return null;
  }
  if (trimmed === "transparent") {
    return "transparent";
  }
  return `var(${tokenToCssName(trimmed)})`;
}

/**
 * Formats a shadow layer stack into a standard CSS box-shadow value string.
 */
export function formatShadow(layers: readonly ShadowLayerDto[]): string | null {
  if (layers.length === 0) return "none";
  const rendered: string[] = [];
  for (const layer of layers) {
    if (
      !Number.isFinite(layer.x) ||
      !Number.isFinite(layer.y) ||
      !Number.isFinite(layer.blur) ||
      !Number.isFinite(layer.spread) ||
      !Number.isFinite(layer.opacity) ||
      layer.opacity < 0 ||
      layer.opacity > 1 ||
      layer.blur < 0 ||
      layer.blur > 64 ||
      layer.x < -32 ||
      layer.x > 32 ||
      layer.y < -32 ||
      layer.y > 32 ||
      layer.spread < -16 ||
      layer.spread > 16
    ) {
      return null;
    }
    const colorCss = formatColorRole(layer.color);
    if (!colorCss) return null;
    const colorWithOpacity =
      colorCss === "transparent" || layer.opacity === 1
        ? colorCss
        : `color-mix(in srgb, ${colorCss} ${Math.round(layer.opacity * 100)}%, transparent)`;
    rendered.push(
      `${layer.x}px ${layer.y}px ${layer.blur}px ${layer.spread}px ${colorWithOpacity}`,
    );
  }
  return rendered.join(", ");
}

/**
 * Formats an inner highlight rim into an inset CSS box-shadow value string.
 */
export function formatInnerHighlight(ih: InnerHighlightDto): string | null {
  if (
    !Number.isFinite(ih.opacity) ||
    !Number.isFinite(ih.width) ||
    ih.opacity < 0 ||
    ih.opacity > 1 ||
    ih.width < 1 ||
    ih.width > 4
  ) {
    return null;
  }
  const colorCss = formatColorRole(ih.color);
  if (!colorCss) return null;
  const colorWithOpacity =
    colorCss === "transparent" || ih.opacity === 1
      ? colorCss
      : `color-mix(in srgb, ${colorCss} ${Math.round(ih.opacity * 100)}%, transparent)`;
  return `inset 0 ${ih.width}px 0 0 ${colorWithOpacity}`;
}

/**
 * Formats and validates a transition timing curve preset.
 */
export function formatTransitionTiming(timing: string): string | null {
  switch (timing) {
    case "linear":
      return "linear";
    case "ease-in":
      return "cubic-bezier(0.4, 0, 1, 1)";
    case "ease-out":
      return "cubic-bezier(0, 0, 0.2, 1)";
    case "ease-in-out":
      return "cubic-bezier(0.4, 0, 0.2, 1)";
    case "spring-subtle":
      return "cubic-bezier(0.16, 1, 0.3, 1)";
    case "spring-snappy":
      return "cubic-bezier(0.2, 0.8, 0.2, 1)";
    default:
      return null;
  }
}

/**
 * Formats and validates a transform preset.
 */
export function formatTransformPreset(preset: string): string | null {
  switch (preset) {
    case "none":
      return "none";
    case "press-subtle":
      return "scale(0.98)";
    case "press-shift-down":
      return "translateY(1px)";
    case "hover-lift":
      return "translateY(-1px)";
    default:
      return null;
  }
}

/**
 * Formats a single typed variable value to a safe CSS string.
 * Returns `null` if any validation or bounds check fails.
 */
export function variableToCssValue(
  value: DesignSystemVariableValue,
  key = "",
): string | null {
  switch (value.type) {
    case "theme-color-role":
      return formatColorRole(value.value);
    case "dimension":
      // Negative outline-offset is valid CSS (draws the outline inward);
      // every other dimension is a size and stays non-negative.
      return Number.isFinite(value.value) &&
        value.value >= (key.endsWith(".outlineOffset") ? -8192 : 0) &&
        value.value <= 8192
        ? `${value.value}px`
        : null;
    case "radius":
      return Number.isFinite(value.value) &&
        value.value >= 0 &&
        value.value <= 9999
        ? `${value.value}px`
        : null;
    case "border-width":
      return Number.isFinite(value.value) &&
        value.value >= 0 &&
        value.value <= 8
        ? `${value.value}px`
        : null;
    case "border-style":
      return ["none", "solid", "dashed", "dotted"].includes(value.value)
        ? value.value
        : null;
    case "spacing-token": {
      const trimmed = value.value.trim();
      if (trimmed.startsWith("spacing.")) {
        return `var(${tokenToCssName(trimmed)})`;
      }
      return null;
    }
    case "opacity":
      return Number.isFinite(value.value) &&
        value.value >= 0 &&
        value.value <= 1
        ? String(value.value)
        : null;
    case "backdrop-blur":
      return Number.isFinite(value.value) &&
        value.value >= 0 &&
        value.value <= 32
        ? `${value.value}px`
        : null;
    case "backdrop-saturate":
      return Number.isFinite(value.value) &&
        value.value >= 1 &&
        value.value <= 2
        ? String(value.value)
        : null;
    case "shadow":
      return formatShadow(value.value);
    case "inner-highlight":
      return formatInnerHighlight(value.value);
    case "outline-style":
      return ["none", "solid"].includes(value.value) ? value.value : null;
    case "motion-duration":
      return Number.isFinite(value.value) &&
        value.value >= 0 &&
        value.value <= 1000
        ? `${value.value}ms`
        : null;
    case "transition-timing":
      return formatTransitionTiming(value.value);
    case "transform-preset":
      return formatTransformPreset(value.value);
  }
}

/**
 * Validates and converts all variables from a DesignSystemSnapshot into a sorted list
 * of CSS custom property `[name, value]` tuples.
 *
 * Throws an Error if any variable is invalid, enforcing atomic rejection before DOM writes.
 */
export function designSystemCssVariables(
  snapshot: DesignSystemSnapshot,
): [string, string][] {
  if (snapshot.schemaVersion !== 1) {
    throw new Error(
      `Unsupported design system schema version: ${snapshot.schemaVersion}`,
    );
  }

  const variables: [string, string][] = [];

  for (const [key, value] of Object.entries(snapshot.variables)) {
    const cssName = recipeVariableToCssName(key);
    const cssVal = variableToCssValue(value, key);
    if (cssVal === null) {
      throw new Error(
        `Invalid design system variable value for \`${key}\`: ${JSON.stringify(value)}`,
      );
    }
    variables.push([cssName, cssVal]);
  }

  return variables.sort(([a], [b]) => a.localeCompare(b));
}

/**
 * Atomically updates CSS custom properties on a target element (e.g. document.documentElement).
 * Removes any properties that were in `prevKeys` but are not present in `nextVariables`.
 * Returns the updated Set of installed property names.
 */
export function installDesignSystemVariables(
  target: CSSStyleDeclaration,
  nextVariables: readonly [string, string][],
  prevKeys?: ReadonlySet<string>,
): Set<string> {
  const nextKeys = new Set<string>();

  for (const [name, value] of nextVariables) {
    target.setProperty(name, value);
    nextKeys.add(name);
  }

  if (prevKeys) {
    for (const oldKey of prevKeys) {
      if (!nextKeys.has(oldKey)) {
        target.removeProperty(oldKey);
      }
    }
  }

  return nextKeys;
}
