export type ServerRegisterSyntaxGrammarOptions = {
    syntaxGrammar?: unknown;
    contribution?: unknown;
    languageId?: string;
    filePatterns?: {
        extensions?: string[];
        fileNames?: string[];
    };
    grammar?: {
        kind: "tree-sitter-wasm";
        path: string;
        source?: string;
    };
    queries?: {
        highlights: string;
        locals?: string;
        injections?: string;
    };
    styleMap?: Record<string, string | {
        styleToken: string;
        fontRole?: "monospace" | "proportional";
        /** Optional capture priority 0-100; higher wins overlapping ranges. Default 70. */
        priority?: number;
    }>;
    budgets?: {
        timeoutMs?: number;
        maxWindowBytes?: number;
    };
    handler?: never;
    callback?: never;
    onParse?: never;
    function?: never;
    clientJavaScript?: never;
    nativeHandle?: never;
    rawOps?: never;
};
export type SyntaxEngineTierPreference = "native" | "wasm" | "javascript" | "js";
export declare function setSyntaxEnginePreference(target: string, tier: SyntaxEngineTierPreference): unknown;
export declare function serverRegisterSyntaxGrammar(options: ServerRegisterSyntaxGrammarOptions): unknown;
