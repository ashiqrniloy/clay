// Complete contract fixtures for tests (plan 119 SC-1).
//
// Contract shapes are generated from the Rust DTO layer, so a test that builds
// a `BehaviorManifest` or a theme snapshot by hand breaks whenever the real
// contract gains a field. These builders carry the canonical complete value and
// take only the fields a test actually cares about.

import type {
  BehaviorManifest,
  EditorBehaviorRules,
  FontProfile,
  ThemeSnapshotDto,
  TypographySnapshotDto,
} from "../bridge/types";

/** The empty editor rule set the Rust `BehaviorManifest` constructors start from. */
export const emptyEditorRules: EditorBehaviorRules = {
  textEdits: [],
  enter: "preserveLeadingWhitespace",
  tab: { mode: "insertSpaces", spacesPerTab: 2 },
  pairs: [],
  comments: [],
  headingPrefixes: [],
  electricCharacters: [],
  autocompleteTriggers: [],
  movement: {
    wordSeparators: "code",
    treatUnderscoreAsWord: true,
    camelCaseSubWord: true,
    paragraphStyle: "blankLine",
    stopAtEolWordEnd: false,
    lineMovement: "character",
    stickyColumn: true,
  },
  caretStyle: null,
  chrome: null,
  layout: null,
};

export const behaviorManifestFixture = (
  overrides: Partial<BehaviorManifest> = {},
): BehaviorManifest => ({
  manifestId: "default.text",
  behaviorVersion: 1,
  scope: "globalDefault",
  documentFontRole: "inherit",
  keymaps: [],
  commands: [],
  editorRules: emptyEditorRules,
  ...overrides,
});

export const fontProfileFixture = (
  overrides: Partial<FontProfile> = {},
): FontProfile => ({
  families: ["system-ui"],
  size: 13,
  ligatures: {
    enableStandard: true,
    enableContextual: true,
    discretionaryFeatures: [],
    rawFeatures: null,
    disableFeatures: [],
  },
  ...overrides,
});

export const themeSnapshotFixture = (
  overrides: Partial<ThemeSnapshotDto> = {},
): ThemeSnapshotDto => ({
  specifier: "@clay/default",
  tokens: {},
  editorStyles: {},
  densityScale: 1,
  ...overrides,
});

export const typographySnapshotFixture = (
  overrides: Partial<TypographySnapshotDto> = {},
): TypographySnapshotDto => ({
  revision: 1,
  monospace: fontProfileFixture({ families: ["Code Mono"] }),
  proportional: fontProfileFixture({ families: ["Read Serif"] }),
  ui: fontProfileFixture(),
  hierarchy: {
    display: 1.5,
    title: 1.25,
    section: 1.125,
    body: 1,
    status: 0.9,
    detail: 0.85,
    caption: 0.8,
  },
  ...overrides,
});
