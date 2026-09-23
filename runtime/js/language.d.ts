export type LanguageIntelligenceFeature = "hover" | "definition" | "goToDefinition" | "codeAction" | "signatureHelp";
export type LanguageIntelligenceProviderDeclaration = {
    id: string;
    modes?: string[];
    features: LanguageIntelligenceFeature[];
    priority?: number;
    exportName?: string;
    timeoutMs?: number;
    budgets?: {
        timeoutMs?: number;
    };
    handler?: never;
    callback?: never;
    function?: never;
    clientJavaScript?: never;
    nativeHandle?: never;
    rawOps?: never;
    executable?: never;
    process?: never;
    languageServer?: never;
};
export type ServerRegisterLanguageIntelligenceProviderOptions = {
    /** A pre-assembled provider declaration; equivalent to passing `id`,
     *  `modes`, `features`, `priority`, `exportName`, `timeoutMs`, and
     *  `budgets` at the top level. A nested `moduleSpecifier` is honored; an
     *  inline handler is only bound from the top-level `module` object. */
    provider?: LanguageIntelligenceProviderDeclaration;
    id?: string;
    modes?: string[];
    features?: LanguageIntelligenceFeature[];
    priority?: number;
    exportName?: string;
    timeoutMs?: number;
    module?: Record<string, unknown>;
    /** Plan 127 P1: the package-owned module that declares `exportName`,
     *  resolved with `import.meta.resolve("./provider.js")`. When present the
     *  provider runs on the domain's latency lane (module import), so a busy
     *  parse/analysis/config lane cannot delay language-intelligence requests.
     *  Omit it for an inline `module: {...}` handler, which stays on the
     *  general lane. */
    moduleSpecifier?: string;
    handler?: never;
    callback?: never;
    function?: never;
    clientJavaScript?: never;
    nativeHandle?: never;
    rawOps?: never;
    executable?: never;
    process?: never;
    languageServer?: never;
};
export type ServerRegisterDocumentAnalyzerOptions = {
    analyzer: {
        id: string;
        contribution: string;
        modes?: string[];
        moduleSpecifier: string;
        exportName?: string;
    };
};
export declare function serverRegisterDocumentAnalyzer(options: ServerRegisterDocumentAnalyzerOptions): unknown;
export declare function serverRegisterLanguageIntelligenceProvider(options: ServerRegisterLanguageIntelligenceProviderOptions): unknown;
