// Clay key binding facade skeleton.
//
// Key binding APIs are planned configuration-time server runtime APIs. They
// record user intent for Clay-owned commands by stable registry ID; they do not
// install JavaScript into the Rust client keypress hot path.

export type KeyBindingScope = "global" | "editor";
export type KeyBindingScopeFilter = "all" | KeyBindingScope;

export interface BindKeyOptions {
  key: string;
  command: string;
  scope?: KeyBindingScope;
  when?: string;
}

export interface KeyBindingRecord {
  key: string;
  command: string;
  scope: KeyBindingScope;
  when?: string;
}

function plannedApi(name: string): never {
  throw new Error(`${name} is planned; Clay JS runtime op wiring is not implemented yet`);
}

export function bindKey(key: string, command: string, options: Omit<BindKeyOptions, "key" | "command"> = {}): KeyBindingRecord {
  void key;
  void command;
  void options;
  plannedApi("clay.keybindings.bindKey");
}

export function unbindKey(key: string, options: Pick<BindKeyOptions, "scope" | "when"> = {}): void {
  void key;
  void options;
  plannedApi("clay.keybindings.unbindKey");
}

export function listKeyBindings(scope?: KeyBindingScopeFilter): KeyBindingRecord[] {
  void scope;
  plannedApi("clay.keybindings.listKeyBindings");
}
