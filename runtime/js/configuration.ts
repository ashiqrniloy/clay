// Clay configuration facade skeleton.
//
// Configuration is planned to run from `~/.config/clay/init.js` in a
// server-side JavaScript runtime. These APIs only describe the stable facade;
// they do not load user files or grant filesystem, network, shell, package, AI,
// workspace, or client-side JavaScript authority.

export interface ConfigurationModuleOptions {
  path: string;
}

export interface ConfigurationState {
  entryPoint: "~/.config/clay/init.js";
  loadedModules: string[];
}

function plannedApi(name: string): never {
  throw new Error(`${name} is planned; Clay JS runtime op wiring is not implemented yet`);
}

export async function loadConfigurationModule(options: ConfigurationModuleOptions): Promise<void> {
  void options;
  plannedApi("clay.configuration.loadConfigurationModule");
}

export function getConfigurationState(): ConfigurationState {
  plannedApi("clay.configuration.getConfigurationState");
}
