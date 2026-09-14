/**
 * Closed allowlists and helper functions for host-owned component, slot,
 * and variant data attributes.
 *
 * Invariant: Package contributions cannot inject arbitrary HTML attributes,
 * classes, or unvalidated variant names. All attributes applied to the DOM
 * must come from these closed catalog sets.
 */

export const KNOWN_COMPONENT_KINDS = [
  "button",
  "textInput",
  "dropdown",
  "list",
  "collapse",
  "modal",
  "panel",
  "label",
  "statusItem",
  "flex",
  "stack",
  "overlay",
  "portal",
  "scroll",
  "editorView",
  "tabList",
  "table",
  "badge",
  "kbd",
  "divider",
  "iconSlot",
  "tooltip",
  "statusDot",
  // View switcher (plan 118 task 33): the `seg` family is the segmented
  // control the switcher is — declared in the design system, consumed by the
  // shell titlebar.
  "seg",
  // Agent-type picker (plan 118 task 35): a dropdown whose *trigger* is the
  // agent view's title, so only the trigger slot takes this family.
  "agentPicker",
  // Session files (plan 118 task 36): the Files tab's row — the session's own
  // file history, one row per path it touched.
  "sessionRow",
] as const;

export type ComponentKind = (typeof KNOWN_COMPONENT_KINDS)[number];

export const KNOWN_SLOT_NAMES = [
  "root",
  "field",
  "label",
  "input",
  "description",
  "error",
  "trigger",
  "triggerLabel",
  "indicator",
  "popover",
  "list",
  "item",
  "row",
  "rowTitle",
  "rowDetail",
  "header",
  "title",
  "chevron",
  "body",
  "scrim",
  "dialog",
  "track",
  "thumb",
  "strip",
  "tab",
  "panel",
] as const;

export type SlotName = (typeof KNOWN_SLOT_NAMES)[number];

export const KNOWN_VARIANTS = [
  "default",
  "muted",
  "primary",
  "danger",
  "display",
  "title",
  "section",
  "body",
  "status",
  "detail",
  "caption",
  "fixed",
] as const;

export type VariantName = (typeof KNOWN_VARIANTS)[number];

export function sanitizeVariant(
  rawVariant?: string | null,
): VariantName | undefined {
  if (!rawVariant) return undefined;
  if ((KNOWN_VARIANTS as readonly string[]).includes(rawVariant)) {
    return rawVariant as VariantName;
  }
  return undefined;
}

export function sanitizeComponentKind(
  rawKind?: string | null,
): ComponentKind | undefined {
  if (!rawKind) return undefined;
  if ((KNOWN_COMPONENT_KINDS as readonly string[]).includes(rawKind)) {
    return rawKind as ComponentKind;
  }
  return undefined;
}

export function sanitizeSlot(rawSlot?: string | null): SlotName | undefined {
  if (!rawSlot) return undefined;
  if ((KNOWN_SLOT_NAMES as readonly string[]).includes(rawSlot)) {
    return rawSlot as SlotName;
  }
  return undefined;
}

export interface RecipeAttributes {
  "data-clay-component"?: ComponentKind;
  "data-clay-slot"?: SlotName;
  "data-variant"?: VariantName;
}

export function recipeAttributes(
  kind: ComponentKind,
  slot?: SlotName,
  rawVariant?: string | null,
): RecipeAttributes {
  const attrs: RecipeAttributes = {};
  const validKind = sanitizeComponentKind(kind);
  if (validKind) {
    attrs["data-clay-component"] = validKind;
  }
  if (slot) {
    const validSlot = sanitizeSlot(slot);
    if (validSlot) {
      attrs["data-clay-slot"] = validSlot;
    }
  }
  const validVariant = sanitizeVariant(rawVariant);
  if (validVariant) {
    attrs["data-variant"] = validVariant;
  }
  return attrs;
}
