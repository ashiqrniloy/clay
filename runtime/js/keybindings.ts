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

const ops = globalThis.Deno?.core?.ops;

function requireOps(): NonNullable<typeof ops> {
  if (!ops) {
    throw new Error("clay.keybindings.runtime_unavailable: Clay key binding APIs require the server runtime");
  }
  return ops;
}

export function bindKey(key: string, command: string, options: Omit<BindKeyOptions, "key" | "command"> = {}): KeyBindingRecord {
  return JSON.parse(requireOps().op_clay_keybindings_bind_key(key, command, JSON.stringify(options ?? {})));
}

export function unbindKey(key: string, options: Pick<BindKeyOptions, "scope" | "when"> = {}): void {
  requireOps().op_clay_keybindings_unbind_key(key, JSON.stringify(options ?? {}));
}

export function listKeyBindings(scope: KeyBindingScopeFilter = "all"): KeyBindingRecord[] {
  return JSON.parse(requireOps().op_clay_keybindings_list_key_bindings(scope));
}
