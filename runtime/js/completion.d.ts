export type CompletionProviderItem = string | {
    label: string;
    insertText: string;
    detail?: string;
    textFormat?: "plainText" | "snippet";
};
export type ServerRegisterCompletionProviderOptions = {
    completionProvider?: unknown;
    contribution?: unknown;
    providerId?: string;
    triggerCharacters?: string[];
    triggers?: {
        characters?: string[];
        wordBoundary?: boolean;
    };
    wordBoundaryChars?: string[];
    items?: CompletionProviderItem[];
    priority?: number;
    exclusive?: boolean;
    timeoutMs?: number;
    maxItems?: number;
    handler?: never;
    callback?: never;
    complete?: never;
    function?: never;
    clientJavaScript?: never;
    nativeHandle?: never;
    rawOps?: never;
    module?: Record<string, unknown>;
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
