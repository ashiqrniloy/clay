// Clay application facade skeleton.
//
// Application lifecycle APIs are planned user-facing Clay JS facades. They do
// not call raw ops directly; future runtime wiring will route through explicit
// Clay op wrappers after permission and lifecycle validation exists.

export interface QuitOptions {
  force?: boolean;
}

export interface QuitResult {
  requested: boolean;
}

function plannedApi(name: string): never {
  throw new Error(`${name} is planned; Clay JS runtime op wiring is not implemented yet`);
}

export async function quit(options: QuitOptions = {}): Promise<QuitResult> {
  void options;
  plannedApi("clay.application.quit");
}
