// Decoration and diagnostic wire shapes are generated from the Rust DTO layer
// (plan 119 SC-1) — do not restate them. Only the event-only payloads below
// (folding, viewport patches, completion, language intelligence, keybindings)
// stay hand-written: they are the shell's narrowing of `ClientConnectionEvent`,
// which the contract generation deliberately excludes.
import type {
  DecorationProvenance as Provenance,
  DecorationSet,
  DiagnosticSet,
  TextByteRange as ByteRange,
} from "../../bridge/types";

export type {
  DecorationKind,
  DecorationProvenance as Provenance,
  DecorationSet,
  DecorationSpan,
  DecorationTarget,
  DiagnosticSet,
  DiagnosticSpan,
  TextByteRange as ByteRange,
  TokenType,
} from "../../bridge/types";

export interface FoldingRange extends ByteRange {
  label: string | null;
  provenance: Provenance;
}
export interface FoldingRangeSet {
  documentId: number;
  documentVersion: number;
  packagePrefix: string;
  ranges: FoldingRange[];
}

/** Protocol v29 atomic answer to one viewport render request. */
export interface ViewportRenderPatchDto {
  requestId: number;
  documentId: number;
  documentVersion: number;
  status: "complete" | "empty" | "rejected";
  reason?: string | null;
  coveredRanges: ByteRange[];
  decorations: DecorationSet[];
  diagnostics: DiagnosticSet[];
  folds: FoldingRangeSet[];
  traceId?: number | null;
}

export interface CompletionItemDto {
  label: string;
  insertText: string;
  detail: string;
  commitCharacters: string;
  textFormat: "plainText" | "snippet";
  provenance: Provenance;
}
export interface CompletionResultSet {
  requestId: number;
  clientId: number;
  documentId: number;
  documentVersion: number;
  behaviorVersion: number;
  providerGeneration: number;
  replacementRange: ByteRange;
  status: "ok" | "empty" | "timeout" | "providerError";
  items: CompletionItemDto[];
  provenance: Provenance;
}

export type LanguageFeature =
  "hover" | "goToDefinition" | "codeAction" | "signatureHelp";
export type TextLocation =
  | { openDocument: { documentId: number; range: ByteRange } }
  | {
      workspaceFile: {
        workspaceRootId: number;
        relativePath: string;
        range: ByteRange;
      };
    };
export type LanguagePayload =
  | { hover: { range: ByteRange | null; markdown: string } }
  | { goToDefinition: { locations: TextLocation[] } }
  | {
      codeAction: {
        actions: Array<{
          range: ByteRange;
          title: string;
          commandId: string | null;
          edit: unknown;
        }>;
      };
    }
  | {
      signatureHelp: {
        signatures: Array<{
          label: string;
          documentation: string;
          parameters: Array<{ label: string; documentation: string }>;
        }>;
        activeSignature: number | null;
        activeParameter: number | null;
      };
    };
export interface SelectionQueryResult {
  requestId: number;
  clientId: number;
  documentId: number;
  documentVersion: number;
  behaviorVersion: number;
  ranges: Array<{ start: number; end: number } | null>;
}

export interface LanguageResult {
  requestId: number;
  clientId: number;
  documentId: number;
  documentVersion: number;
  behaviorVersion: number;
  providerGeneration: number;
  feature: LanguageFeature;
  status: "ok" | "empty" | "timeout" | "providerError";
  payload: LanguagePayload;
  provenance: Provenance;
}

export interface KeyStrokeDto {
  key: string | { character: string };
  modifiers: {
    shift: boolean;
    control: boolean;
    alt: boolean;
    superKey: boolean;
  };
}
export interface KeyBindingDto {
  commandId: string;
  sequence: KeyStrokeDto[];
  context: "editorTextFocus" | "global";
  routingPolicy: unknown;
}

export interface BehaviorManifestDto {
  behaviorVersion: number;
  documentFontRole?: "monospace" | "proportional" | "inherit";
  keymaps?: KeyBindingDto[];
  editorRules?: {
    enter?: unknown;
    tab?: {
      mode?: "insertSpaces" | "insertTabCharacter";
      spacesPerTab?: number;
    };
    pairs?: Array<{ open: string; close: string }>;
    comments?: Array<{ linePrefix: string; continuePrefix: string }>;
    headingPrefixes?: string[];
    autocompleteTriggers?: Array<{ trigger: string }>;
    caretStyle?: unknown;
    chrome?: {
      gutter: boolean;
      activeLine: boolean;
      indentGuides: boolean;
      bracketMatch: boolean;
      inlayHints: boolean;
    } | null;
    layout?: { wrap: unknown } | null;
  };
}
