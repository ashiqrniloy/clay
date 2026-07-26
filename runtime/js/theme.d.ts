export type SetThemeOptions = {
    specifier: string;
};
export type ActiveThemeSummary = {
    specifier: string;
    overrideCount: number;
    designTokenCount: number;
};
export type FontProfile = {
    families: string[];
    size: number;
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
