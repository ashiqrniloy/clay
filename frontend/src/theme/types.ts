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
