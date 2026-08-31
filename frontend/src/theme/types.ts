// Typed mirrors of the Rust-resolved theme/typography projections
// (`src-tauri/src/bridge/dto.rs`). Raw theme overrides never cross the
// bridge; everything here is already validated and resolved Rust-side.

export type ThemeTokenValue =
  | { type: "color"; value: string }
  | { type: "scalar"; value: number }
  | { type: "opacity"; value: number }
  | { type: "level"; value: string }
  | { type: "variant"; value: string };

export interface EditorStyle {
  color: string;
  background: string | null;
  bold: boolean;
  italic: boolean;
  underline: boolean;
  strike: boolean;
  scale: number;
}

export interface ThemeSnapshot {
  specifier: string;
  /** Core token name → resolved typed value (e.g. `surface.main`). */
  tokens: Record<string, ThemeTokenValue>;
  /** Rust-resolved closed editor vocabulary. Optional only for old fixtures. */
  editorStyles?: Record<string, EditorStyle>;
  /** Spacing rhythm multiplier from the resolved density level. */
  densityScale: number;
}

export interface FontProfile {
  families: string[];
  size: number;
  ligatures: { enableStandard: boolean };
}

export interface TypographyHierarchy {
  display: number;
  title: number;
  section: number;
  body: number;
  status: number;
  detail: number;
  caption: number;
}

export interface TypographySnapshot {
  revision: number;
  monospace: FontProfile;
  proportional: FontProfile;
  ui: FontProfile;
  hierarchy: TypographyHierarchy;
}

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

export interface ShadowLayerDto {
  x: number;
  y: number;
  blur: number;
  spread: number;
  color: string;
  opacity: number;
}

export interface InnerHighlightDto {
  color: string;
  opacity: number;
  width: number;
}

export type DesignSystemVariableValue =
  | { type: "theme-color-role"; value: string }
  | { type: "dimension"; value: number }
  | { type: "radius"; value: number }
  | { type: "border-width"; value: number }
  | { type: "border-style"; value: string }
  | { type: "spacing-token"; value: string }
  | { type: "opacity"; value: number }
  | { type: "backdrop-blur"; value: number }
  | { type: "backdrop-saturate"; value: number }
  | { type: "shadow"; value: ShadowLayerDto[] }
  | { type: "inner-highlight"; value: InnerHighlightDto }
  | { type: "outline-style"; value: string }
  | { type: "motion-duration"; value: number }
  | { type: "transition-timing"; value: string }
  | { type: "transform-preset"; value: string };

export interface ComponentRecipeDto {
  backgroundColor: string;
  backgroundOpacity: number;
  textColor: string;
  borderColor: string;
  borderWidth: number;
  borderStyle: string;
  borderRadius: number;
  padding?: string;
  gap?: string;
  shadow: ShadowLayerDto[];
  backdropBlur: number;
  backdropSaturate: number;
  innerHighlight?: InnerHighlightDto;
  opacity: number;
  outlineColor: string;
  outlineWidth: number;
  outlineOffset: number;
  outlineStyle: string;
  transitionDuration: number;
  transitionTiming: string;
  transformPreset: string;
}

export interface DesignSystemProvenanceDto {
  packageName: string;
  packageVersion: string;
  apiPrefix: string;
  trustDomain: "trusted" | "thirdParty";
}

export interface DesignSystemSnapshot {
  specifier: string;
  schemaVersion: number;
  generation: number;
  provenance: DesignSystemProvenanceDto;
  recipes: Record<string, ComponentRecipeDto>;
  variables: Record<string, DesignSystemVariableValue>;
}
