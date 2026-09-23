export interface ConfigurationModuleOptions {
    path: string;
    optional?: boolean;
}
export type ConfigurationModuleLoadResult =
    | { loaded: true }
    | { loaded: false; error: string };
export interface ConfigurationState {
    entryPoint: "~/.config/clay/init.js";
    loadedModules: string[];
    packageOptions?: PackageOptionResult[];
}
export type PackageOptionSource = "init-js" | "package-default" | "clay-default" | "ui-session";
export interface PackageOptionDefinition {
    packagePrefix: string;
    option: string;
    value: unknown;
    source?: PackageOptionSource;
}
export interface PackageOptionResult {
    registered: true;
    packagePrefix: string;
    option: string;
    value: unknown;
    source: PackageOptionSource;
    estimatedPayloadBytes: number;
}
export declare function loadConfigurationModule(options: ConfigurationModuleOptions): Promise<ConfigurationModuleLoadResult>;
export declare function getConfigurationState(): ConfigurationState;
export declare function setPackageOption(options: PackageOptionDefinition): PackageOptionResult;
export declare function setModePreference(options: unknown): never;
export declare function setDecorationTheme(options: unknown): never;
export declare function setParsePolicy(options: unknown): never;
