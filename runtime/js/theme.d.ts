export type SetThemeOptions = {
    specifier: string;
};
export type ActiveThemeSummary = {
    specifier: string;
    overrideCount: number;
    designTokenCount: number;
};
/**
 * OpenType ligature/feature policy for one font role (Plan 071 task 7).
 * All fields are optional; absent fields keep the ligature-on defaults.
 */
export type LigaturePolicy = {
    /** Standard ligatures (`liga` + `clig`). Default: true. */
    enableStandard?: boolean;
    /** Contextual alternates/ligatures (`calt`). Default: true. */
    enableContextual?: boolean;
    /** Extra OpenType feature tags to enable, e.g. ["ss01", "zero"]. Max 32. */
    discretionaryFeatures?: string[];
    /** CSS-font-features-format string escape hatch, e.g. "liga off, dlig". Max 256 bytes. */
    rawFeatures?: string;
    /** OpenType feature tags to force off, e.g. ["calt"]. Max 32. */
    disableFeatures?: string[];
};
export type FontProfile = {
    families: string[];
    size: number;
    /** Optional ligature/feature policy. Omission keeps standard+contextual ligatures enabled. */
    ligatures?: LigaturePolicy;
};
export type UiTypographyHierarchy = {
    display: number;
    title: number;
    section: number;
    body: number;
    status: number;
    detail: number;
    caption: number;
};
export type TypographyConfiguration = {
    monospace: FontProfile;
    proportional: FontProfile;
    ui: FontProfile;
    /** Optional bounded UI variant scale ratios. Omission uses Clay defaults. */
    hierarchy?: UiTypographyHierarchy;
};
export type ActiveTypographySummary = {
    revision: number;
};
export type Appearance = "light" | "dark" | "system";
export type SetAppearanceOptions = {
    appearance: Appearance;
};
export type SetAppearanceSummary = {
    appearance: Appearance;
    /** Canonical default theme specifier resolved for this appearance, or null
     * if an explicit theme is active or the canonical package is unavailable. */
    resolvedTheme: string | null;
};
export declare function setTheme(options: SetThemeOptions | string): ActiveThemeSummary;
export declare function setTypography(options: TypographyConfiguration): ActiveTypographySummary;
export declare function setAppearance(options: SetAppearanceOptions | Appearance): SetAppearanceSummary;
