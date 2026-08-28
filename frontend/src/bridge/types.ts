// Typed mirror of the Rust bridge surface (`src-tauri/src/bridge`).
//
// Naming contract (pinned by Rust tests in `tests/dto_roundtrips.rs`):
// - Envelope kinds and event discriminants are camelCase strings.
// - Protocol payload fields are camelCase (blanket serde attrs on every
//   protocol type).
// - Menu session ids cross as strings (they carry the server high bit);
//   other ids are sequential counters safe in JS numbers.

/** Branded id for menu session handles (string on the wire). */
export type MenuSessionId = string & {
  readonly __menuSessionId: unique symbol;
};

/** Branded document id (sequential counter, number on the wire). */
export type DocumentId = number & { readonly __documentId: unique symbol };

export const asMenuSessionId = (raw: string): MenuSessionId =>
  raw as MenuSessionId;
export const asDocumentId = (raw: number): DocumentId => raw as DocumentId;

// ------------------------------------------------------------- bootstrap

import type { ThemeSnapshot, TypographySnapshot } from "../theme/types";
import type { RuntimeSnapshot } from "../sdui/types";
export type {
  ThemeSnapshot,
  TypographySnapshot,
  ThemeTokenValue,
  FontProfile,
  TypographyHierarchy,
} from "../theme/types";

export interface DocumentTextHeadDto {
  totalBytes: number;
  firstChunk: string;
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

export interface DocumentChunkDto {
  documentId: DocumentId;
  documentVersion: number;
  offset: number;
  text: string;
}

export interface DocumentChunkRejectedDto {
  documentId: DocumentId;
  documentVersion: number;
  offset: number;
  reason: DocumentChunkRejectionDto;
}

export interface InitialDocumentDto {
  documentId: DocumentId;
  version: number;
  head: DocumentTextHeadDto;
  access: { readOnly?: null; editable?: { leaseId: number | null } };
  workspaceRoot: string;
}

export interface CommandEntryDto {
  id: string;
  title?: string;
  [key: string]: unknown;
}

export interface BootstrapDto {
  clientId: number;
  /** Present after the server binds this connection to a tab. */
  tabId?: number | null;
  protocolVersion: number;
  endpoint: string;
  generation: number;
  /** Developer-only profiling flag inherited from `--profile-perf`. */
  performanceProfile?: boolean;
  initialDocument: InitialDocumentDto;
  behaviorManifest: {
    manifestId: string;
    behaviorVersion: number;
    commands: CommandEntryDto[];
    keymaps: unknown[];
    // Additional manifest fields are inert data; the shell only counts them.
    [key: string]: unknown;
  };
  /** Fully resolved by the Rust bridge; the adapter only projects CSS vars. */
  activeTheme: ThemeSnapshot;
  activeTypography: TypographySnapshot;
}

// ------------------------------------------------------------ envelopes

/**
 * Validated client-layer events. Only the families the shell consumes today
 * are fully typed; the rest stay opaque but still flow. Extend this union as
 * React surfaces land (Phase 5+), never by loosening `BridgeEnvelope`.
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
  data: RuntimeDiagnosticDto;
}

export interface RuntimeDiagnosticDto {
  severity: "info" | "warning" | "error" | string;
  code: string;
  message: string;
}

export interface TransientMenuItemDto {
  id: string;
  label: string;
  detail: string | null;
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

/** Bridge-owned lifecycle notices. */
export interface DisconnectedNotice {
  kind: "disconnected";
  data: { reason: string; clientId?: number | null; tabId?: number | null };
}

export interface RoutedEvent {
  kind: "routed";
  data: {
    clientId: number;
    tabId: number | null;
    event: ShellEvent;
  };
}

export type BridgeEnvelope =
  | { kind: "event"; data: ShellEvent }
  | RoutedEvent
  /** Rust-resolved replacement for raw ActiveTheme pushes. */
  | { kind: "themeSnapshot"; data: ThemeSnapshot }
  | {
      kind: "runtimeSnapshot";
      data: {
        clientId: number;
        tabId: number | null;
        snapshot: RuntimeSnapshot;
      };
    }
  | DisconnectedNotice;

// ---------------------------------------------------------------- errors

export interface BridgeErrorDto {
  code:
    | "notConnected"
    | "busy"
    | "timeout"
    | "serverUnreachable"
    | "invalidRequest"
    | "requestTooLarge"
    | "forbidden"
    | "queueFull";
  message: string;
}
