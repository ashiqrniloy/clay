// Clay configuration facade.
//
// Configuration runs from `~/.config/clay/init.js` in Clay's constrained
// server-side JavaScript runtime. These APIs delegate to Clay-owned ops when
// the embedded runtime provides them; they do not grant network, shell,
// package, AI, workspace, WASM, or client-side JavaScript authority.

export interface ConfigurationModuleOptions {
  path: string;
}

export interface ConfigurationState {
  entryPoint: "~/.config/clay/init.js";
  loadedModules: string[];
  packageOptions?: PackageOptionResult[];
}

export type PackageOptionSource = "init-js" | "package-default" | "clay-default";

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

type ClayCoreOps = {
  op_clay_configuration_load_module?: (path: string) => string;
  op_clay_configuration_get_state?: () => string;
  op_clay_configuration_set_package_option?: (optionsJson: string) => string;
  op_clay_runtime_unavailable?: (api: string) => void;
};

declare const Deno: undefined | { core?: { ops?: ClayCoreOps } };

function configurationOps(): ClayCoreOps & {
  op_clay_configuration_load_module: (path: string) => string;
  op_clay_configuration_get_state: () => string;
  op_clay_configuration_set_package_option: (optionsJson: string) => string;
} {
  const ops = Deno?.core?.ops;
  if (
    typeof ops?.op_clay_configuration_load_module !== "function" ||
    typeof ops?.op_clay_configuration_get_state !== "function" ||
    typeof ops?.op_clay_configuration_set_package_option !== "function"
  ) {
    throw new Error("clay.configuration runtime ops are unavailable in this context");
  }
  return ops as ClayCoreOps & {
    op_clay_configuration_load_module: (path: string) => string;
    op_clay_configuration_get_state: () => string;
    op_clay_configuration_set_package_option: (optionsJson: string) => string;
  };
}

function plannedConfigurationApi(api: string): never {
  const unavailable = Deno?.core?.ops?.op_clay_runtime_unavailable;
  if (typeof unavailable === "function") {
    unavailable(api);
  }
  throw new Error(`${api} is planned; configuration setting validation is not implemented yet`);
}

export async function loadConfigurationModule(options: ConfigurationModuleOptions): Promise<void> {
  if (options === null || typeof options !== "object" || typeof options.path !== "string") {
    throw new Error("clay.configuration.invalid_module: loadConfigurationModule requires { path: string }");
  }
  const path = configurationOps().op_clay_configuration_load_module(options.path);
  await import(path);
}

export function getConfigurationState(): ConfigurationState {
  return JSON.parse(configurationOps().op_clay_configuration_get_state()) as ConfigurationState;
}

export function setPackageOption(options: PackageOptionDefinition): PackageOptionResult {
  return JSON.parse(configurationOps().op_clay_configuration_set_package_option(JSON.stringify(options ?? null))) as PackageOptionResult;
}

export function setModePreference(options: unknown): never {
  void options;
  return plannedConfigurationApi("clay.configuration.setModePreference");
}

export function setDecorationTheme(options: unknown): never {
  void options;
  return plannedConfigurationApi("clay.configuration.setDecorationTheme");
}

export function setParsePolicy(options: unknown): never {
  void options;
  return plannedConfigurationApi("clay.configuration.setParsePolicy");
}
