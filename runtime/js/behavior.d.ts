export interface BehaviorManifestSummary {
    id: string;
    documentId?: string;
    version: number;
    clientFirstBehaviors: string[];
}
export interface BehaviorRoute {
    input: string;
    runtimePath: "client-first" | "server-first" | "background";
    apiId?: string;
}
export interface CodeEditingManifestOptions {
    indentSize: number;
    lineComment?: string;
    blockCommentStart?: string;
    blockCommentEnd?: string;
    enter?: {
        kind: "preserveLeadingWhitespace" | "insertNewlineOnly" | "continueLineMarkers" | "preserveFenceBodyIndent";
        markers?: string[];
        exitOnEmptyItem?: boolean;
        fenceMarkers?: string[];
    };
    pairs?: Array<{
        open: string;
        close: string;
    }>;
    electricOutdentCharacters?: string[];
    autocompleteTriggers?: string[];
    /** Declarative movement policy override (`editorRules.movement`). Absent
     * fields fall back to the code-editing defaults server-side. */
    movement?: {
        wordSeparators?: "code" | "prose" | { custom: string[] };
        treatUnderscoreAsWord?: boolean;
        camelCaseSubWord?: boolean;
        paragraphStyle?: "blankLine" | "blankLineOrWhitespace";
        stopAtEolWordEnd?: boolean;
        lineMovement?: "character" | "screenLine";
        stickyColumn?: boolean;
    };
    /** Declarative caret appearance override (`editorRules.caretStyle`).
     * Absent fields fall back to the editor default bar server-side. */
    caretStyle?: {
        shape?: "bar" | "line" | "block" | "underline";
        widthPx?: number;
        heightPct?: number;
        hollow?: boolean;
        blink?: "solid" | "blink" | "phase" | "smooth";
        smoothAnimationMs?: number;
        stopBlinkOnTyping?: boolean;
    };
}
export declare function getActiveBehaviorManifest(documentId?: string): Promise<BehaviorManifestSummary>;
export declare function listBehaviorRoutes(documentId?: string): Promise<BehaviorRoute[]>;
/**
 * Build a generic C-family code-editing behavior manifest from language-specific
 * parameters. The returned object is the `editorRules` shape accepted by
 * `clay:modes` registration/activation and by the server-side validator.
 *
 * The helper emits inert declarative rules only; it never produces executable
 * callbacks, client JavaScript, native handles, or raw authority fields.
 */
export declare function buildCodeEditingManifest(options: CodeEditingManifestOptions): Record<string, unknown>;
