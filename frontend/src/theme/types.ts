// Frontend theme/types surface.
//
// Every shape here is generated from the Rust DTO layer
// (`src-tauri/src/bridge/dto.rs`) into `../bridge/generated/bridge.ts` — the
// names below are aliases kept for the frontend's own vocabulary, never
// restatements (plan 119 SC-1). Only the frontend-owned constants and the
// text-variant union stay declared here.

export type {
  ComponentRecipeDto,
  DesignSystemProvenanceDto,
  DesignSystemVariableValueDto as DesignSystemVariableValue,
  DesignSystemSnapshotDto as DesignSystemSnapshot,
  EditorStyleDto as EditorStyle,
  FontProfile,
  InnerHighlightDto,
  ShadowLayerDto,
  ThemeSnapshotDto as ThemeSnapshot,
  ThemeTokenValueDto as ThemeTokenValue,
  TypographySnapshotDto as TypographySnapshot,
  UiTypographyHierarchy as TypographyHierarchy,
} from "../bridge/types";

/** The seven semantic UI text variants, in catalog order. */
export const TEXT_VARIANTS = [
  "display",
  "title",
  "section",
  "body",
  "status",
  "detail",
  "caption",
] as const;

export type TextVariant = (typeof TEXT_VARIANTS)[number];

/** Native line-height multiplier shared with the Masonry client. */
export const LINE_HEIGHT_MULTIPLIER = 1.2;
