export type TokenType =
  | "namespace"
  | "type"
  | "class"
  | "enum"
  | "interface"
  | "struct"
  | "typeParameter"
  | "parameter"
  | "variable"
  | "property"
  | "enumMember"
  | "event"
  | "function"
  | "method"
  | "macro"
  | "keyword"
  | "modifier"
  | "comment"
  | "string"
  | "number"
  | "regexp"
  | "operator"
  | "decorator"
  | "heading1"
  | "heading2"
  | "heading3"
  | "heading4"
  | "heading5"
  | "heading6"
  | "listItem"
  | "quote"
  | "codeBlock"
  | "codeSpan"
  | "link"
  | "paragraph";

export type DecorationKind =
  "syntax" | "semantic" | "diagnostic" | "searchMatch" | "link" | "inlayHint";

export interface ByteRange {
  byteStart: number;
  byteEnd: number;
}
export interface Provenance {
  packageName: string;
  packageVersion: string;
  packagePrefix: string;
}
export type DecorationTarget =
  | { workspacePath: { relativePath: string; range: ByteRange | null } }
  | { documentRange: { range: ByteRange } }
  | { displayOnly: { text: string } };

export interface DecorationSpan extends ByteRange {
  kind: DecorationKind;
  tokenType: TokenType;
  modifiers: number;
  scope: string | null;
  fontRole: "monospace" | "proportional" | "inherit" | null;
  priority: number;
  provenance: Provenance;
  target: DecorationTarget | null;
  inlay: { label: string; placement: "before" | "after" } | null;
}

export interface DecorationSet {
  documentId: number;
  documentVersion: number;
  packagePrefix: string;
  kind: DecorationKind;
  viewportByteStart: number;
  viewportByteEnd: number;
  spans: DecorationSpan[];
}

export interface DiagnosticSpan extends ByteRange {
  severity: "error" | "warning" | "info";
  code: string;
  message: string;
  source: string;
  provenance: Provenance;
}
export interface DiagnosticSet {
  documentId: number;
  documentVersion: number;
  viewportByteStart: number;
  viewportByteEnd: number;
  source: string;
  provenance: Provenance;
  spans: DiagnosticSpan[];
}

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
