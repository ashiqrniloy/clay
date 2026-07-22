export type LanguageIntelligenceFeature = "hover" | "definition" | "goToDefinition" | "codeAction" | "signatureHelp";
export type LanguageIntelligenceProviderDeclaration = {
    id: string;
    modes?: string[];
    features: LanguageIntelligenceFeature[];
    priority?: number;
    module?: string;
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
    provider?: LanguageIntelligenceProviderDeclaration;
    id?: string;
    modes?: string[];
    features?: LanguageIntelligenceFeature[];
    priority?: number;
    exportName?: string;
    timeoutMs?: number;
    module?: Record<string, unknown>;
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
