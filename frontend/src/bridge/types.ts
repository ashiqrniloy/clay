// Webview bridge types.
//
// The contract's *shapes* are generated from the Rust DTO layer
// (`src-tauri/src/bridge/dto.rs` + `errors.rs`) into `./generated/bridge.ts` by
// `cargo test -p clay-desktop --features ts-bindings
// export_webview_contract_bindings` — never restate a generated shape here
// (plan 119 SC-1; `scripts/check.sh` fails on stale bindings).
//
// What stays hand-written, and why:
//
//  1. Branded ids (`MenuSessionId`, `DocumentId`): a TypeScript-only nominal
//     distinction; the wire carries plain numbers, and menu session ids cross
//     as strings because they can exceed the safe-integer range.
//  2. The client-event narrowing (`ShellEvent` and the payload types of the
//     families the shell consumes). `BridgeEnvelope`'s `event`/`routed` variants
//     are deliberately `ts(skip)`-ed on the Rust side: generating the full
//     `ClientConnectionEvent` union would drag the internal event graph
//     (server messages, agent frames, viewport patches) into the contract. The
//     shell narrows it to what it consumes and keeps an opaque catch-all for the
//     rest — a narrowing, not a copy.
//
// Naming contract (pinned by `src-tauri/tests/dto_roundtrips.rs` and the
// generated file): envelope kinds and event discriminants are camelCase
// strings; protocol payload fields are camelCase.

/** Branded id for menu session handles (string on the wire). */
export type MenuSessionId = string & {
  readonly __menuSessionId: unique symbol;
};

/** Branded document id (sequential counter, number on the wire). */
export type DocumentId = number & { readonly __documentId: unique symbol };

export const asMenuSessionId = (raw: string): MenuSessionId =>
  raw as MenuSessionId;
export const asDocumentId = (raw: number): DocumentId => raw as DocumentId;

// Generated contract (everything except the envelope, which this file composes).
export type {
  AutocompleteTrigger,
  BehaviorManifest,
  BehaviorScope,
  BlinkStyle,
  BootstrapDto,
  BridgeError,
  BridgeErrorCode,
  CaretShape,
  CaretStyle,
  CommandAuthority,
  CommandDeclaration,
  CommentContinuationRule,
  ComponentRecipeDto,
  DecorationKind,
  DecorationProvenance,
  DecorationSet,
  DecorationSpan,
  DecorationTarget,
  DesignSystemProvenanceDto,
  DesignSystemSnapshotDto,
  DesignSystemVariableValueDto,
  DiagnosticSet,
  DiagnosticSeverity,
  DiagnosticSpan,
  DocumentAccess,
  DocumentFontRole,
  DocumentRuntimeRenderState,
  DocumentTextHead,
  EditorBehaviorRules,
  EditorChrome,
  EditorLayoutRules,
  EditorStyleDto,
  ElectricCharacterRule,
  ElectricEffect,
  EnterRule,
  FontProfile,
  IconGeometryDto,
  IconPackSnapshotDto,
  IconPathDto,
  InitialDocumentDto,
  InlayHintPayload,
  InlayPlacement,
  InnerHighlightDto,
  KeyBindingContext,
  KeyBindingRule,
  KeyCode,
  KeyModifiers,
  KeyStroke,
  LigaturePolicy,
  LineMovementStyle,
  LockScope,
  Modifiers,
  MovementRules,
  PackageInputRouteContent,
  PackageOverlayDto,
  PackagePanelDto,
  PackageSurfaceDto,
  PackageUiProvenance,
  PackageUiSnapshotDto,
  PackageUiTrustDomain,
  PairRule,
  PairRuleContext,
  ParagraphStyle,
  RoutingPolicy,
  RuntimeDiagnostic,
  RuntimeSnapshotDto,
  SduiActionArgument,
  SduiActionIntent,
  SduiActionSource,
  SduiActionValue,
  SduiEditorBinding,
  SduiFlexDirection,
  SduiListFilter,
  SduiListItem,
  SduiNode,
  SduiNodeId,
  SduiNodeKind,
  SduiTree,
  ShadowLayerDto,
  TabMode,
  TabRule,
  TextByteRange,
  TextEditCapability,
  ThemeSnapshotDto,
  ThemeTokenValueDto,
  TokenType,
  TypographySnapshotDto,
  UiChoiceOption,
  UiChoicesSnapshot,
  UiTypographyHierarchy,
  WordSeparatorPolicy,
  WrapPolicy,
} from "./generated/bridge";
export type { JsonValue } from "./generated/serde_json/JsonValue";
export type { JsonValue as BridgeJsonValue } from "./generated/serde_json/JsonValue";

import type { BridgeEnvelope as GeneratedBridgeEnvelope } from "./generated/bridge";
import type { BridgeError, RuntimeDiagnostic } from "./generated/bridge";

/** Previous name of the generated bridge error; kept for import compatibility. */
export type BridgeErrorDto = BridgeError;

/** Bridge-owned lifecycle notice (generated variant, named for consumers). */
export type DisconnectedNotice = Extract<
  GeneratedBridgeEnvelope,
  { kind: "disconnected" }
>;

// ------------------------------------------------- document chunk payloads

export interface DocumentChunkDto {
  documentId: DocumentId;
  documentVersion: number;
  offset: number;
  text: string;
}

export type DocumentChunkRejectionDto =
  | {
      invalidRequestSize: {
        requestedBytes: number;
        minimumBytes: number;
        maximumBytes: number;
      };
    }
  | "invalidOffset"
  | { staleVersion: { currentVersion: number } }
  | "unknownDocument";

export interface DocumentChunkRejectedDto {
  documentId: DocumentId;
  documentVersion: number;
  offset: number;
  reason: DocumentChunkRejectionDto;
}

// ------------------------------------------------------------ envelopes

/**
 * Validated client-layer events. Only the families the shell consumes today
 * are fully typed; the rest stay opaque but still flow. Extend this union as
 * React surfaces land, never by loosening `BridgeEnvelope`.
 */
export interface TabEntryDto {
  tabId: number;
  workspaceRoot: string;
  clientId: number;
}

export interface TabRegistryEvent {
  kind: "tabRegistry";
  data: {
    tabs: TabEntryDto[];
    active: number | null;
    revision: number;
  };
}

export interface RuntimeDiagnosticEvent {
  kind: "runtimeDiagnostic";
  data: RuntimeDiagnostic;
}

/** Previous name of the generated `RuntimeDiagnostic`; kept for consumers. */
export type RuntimeDiagnosticDto = RuntimeDiagnostic;

export interface TransientMenuItemDto {
  id: string;
  label: string;
  detail: string | null;
  /**
   * Plan 124: the row's scope tag, from the server's closed vocabulary
   * (`session` / `shell` / `files`). `null`/absent = the row belongs to no
   * scope and shows under `All` only (never a guessed one).
   */
  scope?: string | null;
  /**
   * Plan 124: the command's effective chords in the app's spelling
   * (`"Ctrl+X Ctrl+P"`) — the palette's per-row chips. Absent when unbound.
   */
  bindings?: string[];
  accessibilityLabel: string;
}

export interface TransientMenuSnapshotDto {
  sessionId: MenuSessionId;
  prompt: string;
  query: string;
  items: TransientMenuItemDto[];
  selectedIndex: number;
  status: "active" | { empty: { message: string } };
  focusPolicy: "modal" | "modeless";
  /**
   * Plan 125: the session's presentation mode, from the server's closed
   * vocabulary — `catalogue` (rows only), `path` (the directory mode),
   * `picker` (a picker stage), `secret` (the shielded field), `url`, `oauth`.
   * It picks the sheet's stage layout; the client never infers a stage from the
   * origin or the prompt. Absent = `catalogue`.
   */
  mode?: string | null;
  /**
   * `centered` is the retired window sheet's origin (plan 125): the wire value
   * stays decodable for older peers, but no producer sends it and this shell
   * draws no surface for it — every session it *does* send is the composer
   * palette (`commandPalette`) or a package overlay origin
   * (`contextMenu`/`menuBar`, drawn by the package UI renderer).
   */
  origin: "commandPalette" | "contextMenu" | "menuBar" | "centered";
}

export interface TransientMenuSnapshotEvent {
  kind: "transientMenuSnapshot";
  data: TransientMenuSnapshotDto;
}

export interface TransientMenuClosedEvent {
  kind: "transientMenuClosed";
  data: { sessionId: MenuSessionId };
}

export interface ShellClientCommandEvent {
  kind: "shellClientCommandRequest";
  data: { commandId: string };
}

export interface ServerErrorEvent {
  kind: "serverError";
  data: { code: string; message: string };
}

export type ShellEvent =
  | TabRegistryEvent
  | RuntimeDiagnosticEvent
  | TransientMenuSnapshotEvent
  | TransientMenuClosedEvent
  | ShellClientCommandEvent
  | ServerErrorEvent
  | { kind: string; data: unknown };

export interface RoutedEvent {
  kind: "routed";
  data: {
    clientId: number;
    tabId: number | null;
    event: ShellEvent;
  };
}

/**
 * Everything the webview can observe: the generated envelope (resolved theme
 * and runtime snapshots, disconnect notice) plus the narrowed client-event
 * variants the Rust side excludes from generation.
 */
export type BridgeEnvelope =
  GeneratedBridgeEnvelope | { kind: "event"; data: ShellEvent } | RoutedEvent;
