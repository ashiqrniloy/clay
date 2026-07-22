export type SetThemeOptions = {
    specifier: string;
};
export type ActiveThemeSummary = {
    specifier: string;
    overrideCount: number;
};
export type FontProfile = {
    families: string[];
    size: number;
};
export type TypographyConfiguration = {
    monospace: FontProfile;
    proportional: FontProfile;
    ui: FontProfile;
};
export type ActiveTypographySummary = {
    revision: number;
};
export declare function setTheme(options: SetThemeOptions | string): ActiveThemeSummary;
export declare function setTypography(options: TypographyConfiguration): ActiveTypographySummary;
