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
export declare function bindKey(key: string, command: string, options?: Omit<BindKeyOptions, "key" | "command">): KeyBindingRecord;
export declare function unbindKey(key: string, options?: Pick<BindKeyOptions, "scope" | "when">): void;
export declare function listKeyBindings(scope?: KeyBindingScopeFilter): KeyBindingRecord[];
