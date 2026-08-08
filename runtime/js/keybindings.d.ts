export type KeyBindingScope = "global" | "editor";
export type KeyBindingScopeFilter = "all" | KeyBindingScope;
export interface BindKeyOptions {
    key: string;
    command: string;
    scope?: KeyBindingScope;
    when?: string;
}
/** Table form: one call, one scope, a chord->command map. */
export interface KeyBindingTable {
    scope?: KeyBindingScope;
    bindings: Record<string, string>;
}
/** Table form for unbind: one scope, a list of chords. */
export interface KeyUnbindTable {
    scope?: KeyBindingScope;
    keys: string[];
}
export interface KeyBindingRecord {
    key: string;
    command: string;
    scope: KeyBindingScope;
    when?: string;
}
export declare function bindKey(key: string, command: string, options?: Omit<BindKeyOptions, "key" | "command">): KeyBindingRecord;
export declare function bindKey(table: KeyBindingTable): KeyBindingRecord[];
export declare function unbindKey(key: string, options?: Pick<BindKeyOptions, "scope" | "when">): void;
export declare function unbindKey(table: KeyUnbindTable): void;
export declare function listKeyBindings(scope?: KeyBindingScopeFilter): KeyBindingRecord[];
