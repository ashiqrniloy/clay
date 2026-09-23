/** A completion provider's declared item shape, as written in
 *  `clay.contributions.completionProviders` in `package.json`. This call never
 *  reads items from its options object (plan 136 task 8). */
export type CompletionProviderItem = string | {
    label: string;
    insertText: string;
    detail?: string;
    textFormat?: "plainText" | "snippet";
};
export type ServerRegisterCompletionProviderOptions = {
    handler?: never;
    callback?: never;
    complete?: never;
    function?: never;
    clientJavaScript?: never;
    nativeHandle?: never;
    rawOps?: never;
    module?: Record<string, unknown>;
    /** Plan 127 P1: the package-owned module that declares `exportName`,
     *  resolved with `import.meta.resolve("./provider.js")`. When present the
     *  provider runs on the domain's latency lane (module import), so a busy
     *  parse/analysis/config lane cannot delay completions. Omit it for an
     *  inline `module: {...}` handler, which stays on the general lane. */
    moduleSpecifier?: string;
    exportName?: string;
};
export declare function serverRegisterCompletionProvider(options: ServerRegisterCompletionProviderOptions): unknown;
export type ServerDisableCompletionOptions = {
    provider: string;
    packagePrefix?: never;
} | {
    provider?: never;
    packagePrefix: string;
};
export declare function serverDisableCompletion(options: ServerDisableCompletionOptions): unknown;
export type ServerListCompletionProvidersForTriggerOptions = {
    trigger: string;
};
export declare function serverListCompletionProvidersForTrigger(options: ServerListCompletionProvidersForTriggerOptions): unknown;
export type EditorRulesLike = {
    autocompleteTriggers?: Array<{
        trigger?: string;
    }>;
};
export declare function completionTriggerCharactersFromEditorRules(editorRules: EditorRulesLike): string[];
